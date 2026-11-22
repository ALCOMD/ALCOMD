//! Acceptance must precede the first external effect. A restarted Core never reissues that effect.
use crate::{IdempotencyKey, OperationId, Permission, PrincipalId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, OnceLock},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalMethod {
    SettingsUpdate,
    OpenHub,
    ExportRepositories,
}

impl ExternalMethod {
    pub fn name(self) -> &'static str {
        match self {
            Self::SettingsUpdate => "settings.update",
            Self::OpenHub => "unity.hub.open",
            Self::ExportRepositories => "repositories.exportListFile",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "settings.update" => Some(Self::SettingsUpdate),
            "unity.hub.open" => Some(Self::OpenHub),
            "repositories.exportListFile" => Some(Self::ExportRepositories),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct ExternalRequest {
    pub method: ExternalMethod,
    pub caller: PrincipalId,
    pub key: IdempotencyKey,
    pub fingerprint: String,
    pub permissions: Vec<Permission>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ExternalReceipt {
    Result(Value),
    Error(Value),
    OutcomeUnknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExternalAdmission {
    Ready,
    Accepted(OperationId),
    Replayed(ExternalReceipt),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalError {
    Denied,
    Conflict,
    Unavailable,
}

pub trait ExternalWriteStore: Clone + Send + Sync + 'static {
    fn external_admit(
        &self,
        request: ExternalRequest,
        reserve: bool,
        now: u64,
    ) -> impl Future<Output = Result<ExternalAdmission, ExternalError>> + Send;
    fn external_finish(
        &self,
        request: ExternalRequest,
        id: OperationId,
        receipt: ExternalReceipt,
        now: u64,
    ) -> impl Future<Output = Result<(), ExternalError>> + Send;
}

type Claim = dyn Fn() -> Pin<Box<dyn Future<Output = Result<ExternalAdmission, ExternalError>> + Send>>
    + Send
    + Sync;

#[derive(Clone)]
pub struct ExternalAcceptance {
    claim: Arc<Claim>,
    pub accepted: Arc<OnceLock<OperationId>>,
    observed: Arc<Mutex<Option<Result<ExternalReceipt, ExternalError>>>>,
}
tokio::task_local! { static ACTIVE: ExternalAcceptance; }

impl ExternalAcceptance {
    pub fn new(store: impl ExternalWriteStore, request: ExternalRequest) -> Self {
        let authentication = crate::workflow_authentication();
        Self {
            accepted: Default::default(),
            observed: Default::default(),
            claim: Arc::new(move || {
                let store = store.clone();
                let request = request.clone();
                let auth = authentication.clone();
                Box::pin(async move {
                    let now = crate::unix_time_ms().map_err(|_| ExternalError::Unavailable)?;
                    let future = store.external_admit(request, true, now);
                    match auth {
                        Some(auth) => crate::with_workflow_authentication(auth, future).await,
                        None => future.await,
                    }
                })
            }),
        }
    }
    pub async fn scope<F: Future>(&self, work: F) -> F::Output {
        ACTIVE.scope(self.clone(), work).await
    }
    pub fn current() -> Option<Self> {
        ACTIVE.try_with(Clone::clone).ok()
    }
    pub fn observed(&self) -> Option<Result<ExternalReceipt, ExternalError>> {
        self.observed.lock().expect("external receipt").clone()
    }
    /// The RPC adapter returns stored results directly. A losing concurrent invocation never performs an effect.
    pub async fn claim(&self) -> bool {
        let outcome = match (self.claim)().await {
            Ok(ExternalAdmission::Accepted(id)) => return self.accepted.set(id).is_ok(),
            Ok(ExternalAdmission::Replayed(receipt)) => Ok(receipt),
            Ok(ExternalAdmission::Ready) => Err(ExternalError::Unavailable),
            Err(error) => Err(error),
        };
        *self.observed.lock().expect("external receipt") = Some(outcome);
        false
    }
    pub fn blocking_claim(&self) -> bool {
        tokio::runtime::Handle::current().block_on(self.claim())
    }
}
