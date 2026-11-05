//! Private technical causes never form part of a business RPC DTO.
use std::{error::Error, fmt, sync::Arc};
#[derive(Clone, Debug, Default)]
pub struct TechnicalErrorContext {
    pub path: Option<String>,
    cause: Option<Arc<dyn Error + Send + Sync>>,
    pub diagnostic_id: Option<String>,
}
impl PartialEq for TechnicalErrorContext {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
            && self.cause.as_ref().map(ToString::to_string)
                == other.cause.as_ref().map(ToString::to_string)
    }
}
impl Eq for TechnicalErrorContext {}
impl TechnicalErrorContext {
    pub fn with_source(mut self, error: impl Error + Send + Sync + 'static) -> Self {
        self.cause = Some(Arc::new(error));
        self
    }
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        if self.path.is_none() {
            self.path = Some(path.into());
        }
        self
    }
    pub fn with_reason(mut self, message: impl Into<String>) -> Self {
        self.cause = Some(Arc::new(TechnicalMessage {
            message: message.into(),
            cause: self.cause.take(),
        }));
        self
    }
    pub fn source_error(&self) -> Option<&(dyn Error + 'static)> {
        self.cause.as_deref().map(|e| e as &(dyn Error + 'static))
    }
}
#[derive(Debug)]
pub struct TechnicalMessage {
    message: String,
    cause: Option<Arc<dyn Error + Send + Sync>>,
}
impl fmt::Display for TechnicalMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}
impl Error for TechnicalMessage {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause.as_deref().map(|v| v as &(dyn Error + 'static))
    }
}

/// The execution owner records before reducing the error to a public code.
pub fn record_technical_failure(
    error: &(dyn Error + 'static),
    path: Option<&str>,
    operation_id: Option<crate::OperationId>,
    message: &'static str,
) -> String {
    let id = crate::OperationId::new().to_string();
    let chain = technical_error_chain(error);
    tracing::error!(diagnostic_id=%id,path,operation_id=operation_id.map(|v|v.to_string()),error=%chain,"{message}");
    id
}
pub fn technical_error_chain(error: &(dyn Error + 'static)) -> String {
    let mut text = error.to_string();
    let mut previous = text.clone();
    let mut source = error.source();
    let mut depth = 0;
    while let Some(cause) = source {
        if depth == 32 || text.len() > 128 * 1024 {
            text.push_str("\n[cause chain exceeds capture limit]");
            break;
        }
        let next = cause.to_string();
        if next != previous {
            text.push_str("\ncaused by: ");
            text.push_str(&next);
        }
        previous = next;
        source = cause.source();
        depth += 1;
    }
    text
}
