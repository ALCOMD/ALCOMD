//! Authorization and bounded use cases for technical-log adapters.
use crate::{AccessContext, Permission, PrincipalId};

#[derive(Clone, Debug, Default)]
pub struct TechnicalLogQuery {
    pub search: Option<String>,
    pub levels: Option<Vec<String>>,
    pub target: Option<String>,
    pub diagnostic_id: Option<String>,
}
#[derive(Clone, Debug)]
pub struct TechnicalLogRead {
    pub cursor: Option<String>,
    pub limit: u32,
    pub follow: bool,
    pub query: TechnicalLogQuery,
}
#[derive(Clone, Debug)]
pub struct TechnicalLogHistory {
    pub cursor: Option<String>,
    pub before: Option<String>,
    pub limit: u32,
    pub query: TechnicalLogQuery,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TechnicalLogError {
    InvalidInput,
    PermissionDenied,
    CursorExpired,
    Unavailable,
}

pub trait TechnicalLogPort: Clone + Send + Sync + 'static {
    type ReadPage: Send;
    type HistoryPage: Send;
    type Health: Send;
    fn read(
        &self,
        request: TechnicalLogRead,
    ) -> impl std::future::Future<Output = Result<Self::ReadPage, TechnicalLogError>> + Send;
    fn history(
        &self,
        request: TechnicalLogHistory,
    ) -> impl std::future::Future<Output = Result<Self::HistoryPage, TechnicalLogError>> + Send;
    fn health(&self) -> Self::Health;
}
#[derive(Clone)]
pub struct TechnicalLogApplication<P> {
    port: P,
}
impl<P: TechnicalLogPort> TechnicalLogApplication<P> {
    pub fn new(port: P) -> Self {
        Self { port }
    }
    pub async fn read(
        &self,
        access: &AccessContext,
        request: TechnicalLogRead,
    ) -> Result<P::ReadPage, TechnicalLogError> {
        authorize(access)?;
        validate(&request.query, request.limit, request.cursor.as_deref())?;
        self.port.read(request).await
    }
    pub async fn history(
        &self,
        access: &AccessContext,
        request: TechnicalLogHistory,
    ) -> Result<P::HistoryPage, TechnicalLogError> {
        authorize(access)?;
        validate(&request.query, request.limit, request.cursor.as_deref())?;
        if request
            .before
            .as_ref()
            .is_some_and(|v| v.is_empty() || v.len() > 1024)
        {
            return Err(TechnicalLogError::InvalidInput);
        }
        self.port.history(request).await
    }
    pub fn health(&self, access: &AccessContext) -> Result<P::Health, TechnicalLogError> {
        authorize(access)?;
        Ok(self.port.health())
    }
}
fn authorize(access: &AccessContext) -> Result<(), TechnicalLogError> {
    if access.principal() != &PrincipalId::local_owner()
        || access.require(Permission::DiagnosticsRead).is_err()
    {
        return Err(TechnicalLogError::PermissionDenied);
    }
    Ok(())
}
fn validate(
    query: &TechnicalLogQuery,
    limit: u32,
    cursor: Option<&str>,
) -> Result<(), TechnicalLogError> {
    if !(1..=200).contains(&limit)
        || cursor.is_some_and(|v| v.is_empty() || v.len() > 1024)
        || query.search.as_ref().is_some_and(|v| v.len() > 4096)
        || query.target.as_ref().is_some_and(|v| v.len() > 128)
        || query.diagnostic_id.as_ref().is_some_and(|v| v.len() != 36)
        || query.levels.as_ref().is_some_and(|v| {
            v.len() > 5
                || v.iter().any(|level| {
                    !matches!(
                        level.as_str(),
                        "TRACE" | "DEBUG" | "INFO" | "WARN" | "ERROR"
                    )
                })
        })
    {
        return Err(TechnicalLogError::InvalidInput);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    #[derive(Clone)]
    struct Reader(Arc<AtomicUsize>);
    impl TechnicalLogPort for Reader {
        type ReadPage = ();
        type HistoryPage = ();
        type Health = ();
        async fn read(&self, _: TechnicalLogRead) -> Result<(), TechnicalLogError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        async fn history(&self, _: TechnicalLogHistory) -> Result<(), TechnicalLogError> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        fn health(&self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    #[tokio::test]
    async fn owner_and_bounds_are_checked_before_any_file_adapter_call() {
        let calls = Arc::new(AtomicUsize::new(0));
        let app = TechnicalLogApplication::new(Reader(calls.clone()));
        let request = || TechnicalLogRead {
            cursor: None,
            limit: 100,
            follow: false,
            query: TechnicalLogQuery::default(),
        };
        let foreign = AccessContext::new(
            PrincipalId::parse("extension:test").expect("valid technical-log test fixture"),
            [Permission::DiagnosticsRead],
        );
        assert_eq!(
            app.read(&foreign, request()).await,
            Err(TechnicalLogError::PermissionDenied)
        );
        let denied = AccessContext::new(PrincipalId::local_owner(), []);
        assert_eq!(
            app.health(&denied),
            Err(TechnicalLogError::PermissionDenied)
        );
        let mut bad = request();
        bad.limit = 201;
        assert_eq!(
            app.read(&AccessContext::local_owner(), bad).await,
            Err(TechnicalLogError::InvalidInput)
        );
        let mut bad = request();
        bad.query.levels = Some(vec!["unknown".into()]);
        assert_eq!(
            app.read(&AccessContext::local_owner(), bad).await,
            Err(TechnicalLogError::InvalidInput)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        app.read(&AccessContext::local_owner(), request())
            .await
            .expect("valid technical-log test fixture");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
