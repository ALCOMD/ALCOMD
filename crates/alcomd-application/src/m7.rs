//! M7 Portable UI application use cases and daemon-memory-only session state.

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::Mutex;

use crate::{
    AccessContext, ExtensionActivationKind, ExtensionDesiredState, ExtensionInstanceLease,
    ExtensionQuarantineState, ExtensionRecord, ExtensionStartContext, ExtensionStopReason,
    ExtensionUiProtocol, M6Application, M6Error, M6ErrorCode, M6PackageAdapter, M6RuntimeAdapter,
    M6Store, OperationId, Permission,
};

pub const UI_IDLE_TIMEOUT_MS: u64 = 300_000;
pub const UI_ABSOLUTE_TIMEOUT_MS: u64 = 3_600_000;
pub const UI_HOST_IDLE_STOP_MS: u64 = 5_000;
const MAX_SESSIONS_PER_EXTENSION: usize = 8;
const MAX_SESSIONS_PER_CONNECTION: usize = 16;
const MAX_SESSIONS_TOTAL: usize = 128;
const MAX_REPLAY_ENTRIES: usize = 64;
const RATE_CAPACITY_MILLI_TOKENS: u64 = 10_000;
const RATE_TOKEN_COST: u64 = 1_000;
const CLIENT_VIOLATION_WINDOW_MS: u64 = 60_000;
const CLIENT_VIOLATION_CLOSE_THRESHOLD: usize = 3;
const MAX_STALE_SESSION_EVIDENCE: usize = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum M7ErrorCode {
    InvalidInput,
    PermissionDenied,
    NotInstalled,
    NotEnabled,
    Quarantined,
    UiNotAvailable,
    UiProtocolUnsupported,
    SessionNotFound,
    SessionStale,
    SnapshotStale,
    DocumentInvalid,
    ActionInvalid,
    LimitExceeded,
    ExtensionPermissionDenied,
    ExtensionScopeDenied,
    InstanceStale,
    Crashed,
    ResourceLimit,
    StoreUnavailable,
    Internal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct M7Error {
    code: M7ErrorCode,
}

impl M7Error {
    #[must_use]
    pub const fn new(code: M7ErrorCode) -> Self {
        Self { code }
    }

    #[must_use]
    pub const fn code(&self) -> M7ErrorCode {
        self.code
    }
}

impl fmt::Display for M7Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Portable UI request failed: {:?}", self.code)
    }
}

impl std::error::Error for M7Error {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExtensionInvocationKind {
    Background,
    InteractiveUiRender,
    InteractiveUiAction,
    InteractiveUiClose,
}

impl ExtensionInvocationKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::InteractiveUiRender => "interactive-ui-render",
            Self::InteractiveUiAction => "interactive-ui-action",
            Self::InteractiveUiClose => "interactive-ui-close",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExtensionUiInvocationAuthority {
    pub kind: ExtensionInvocationKind,
    pub client_access: AccessContext,
    pub client_connection_id: String,
    pub client_instance_id: String,
    pub session_id: String,
    pub snapshot_revision: u64,
    pub deadline_ms: u64,
    current: Arc<AtomicBool>,
}

impl ExtensionUiInvocationAuthority {
    #[must_use]
    pub fn is_current(&self) -> bool {
        self.current.load(Ordering::Acquire)
    }
}

#[derive(Clone, Debug)]
pub enum ExtensionUiExport {
    Open { session_id: String, locale: String },
    Refresh { session_id: String },
    Dispatch { session_id: String, action: Vec<u8> },
    Close { session_id: String },
}

pub trait M7RuntimeAdapter: M6RuntimeAdapter {
    fn start_interactive(
        &self,
        context: ExtensionStartContext,
        authority: ExtensionUiInvocationAuthority,
    ) -> impl Future<Output = Result<(), M6Error>> + Send;

    fn active_lease(
        &self,
        extension_id: String,
    ) -> impl Future<Output = Result<Option<ExtensionInstanceLease>, M6Error>> + Send;

    fn background_active(
        &self,
        extension_id: String,
    ) -> impl Future<Output = Result<bool, M6Error>> + Send;

    fn invoke_ui(
        &self,
        lease: ExtensionInstanceLease,
        authority: ExtensionUiInvocationAuthority,
        export: ExtensionUiExport,
    ) -> impl Future<Output = Result<Option<Vec<u8>>, M6Error>> + Send;

    fn terminate_host(
        &self,
        extension_id: String,
    ) -> impl Future<Output = Result<Option<ExtensionInstanceLease>, M6Error>> + Send;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum M7ValidationFailure {
    Invalid,
    LimitExceeded,
}

pub trait M7UiValidator: Clone + Send + Sync + 'static {
    fn validate_document(&self, document: &[u8]) -> Result<(), M7ValidationFailure>;
    fn validate_action(&self, document: &[u8], action: &[u8]) -> Result<(), M7ValidationFailure>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct M7UiSessionInfo {
    pub session_id: String,
    pub extension_id: String,
    pub locale: String,
    pub idle_timeout_ms: u64,
    pub absolute_timeout_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct M7UiSnapshot {
    pub session_id: String,
    pub snapshot_revision: u64,
    pub document: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct M7UiOpenResult {
    pub session: M7UiSessionInfo,
    pub snapshot: M7UiSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct M7UiDispatchResult {
    pub snapshot: M7UiSnapshot,
    pub replayed: bool,
}

#[derive(Clone)]
pub struct M7Application<S: M6Store, P: M6PackageAdapter, R: M7RuntimeAdapter, V: M7UiValidator> {
    core: M6Application<S, P, R>,
    validator: V,
    sessions: Arc<Mutex<UiSessionCoordinator>>,
    host_start: Arc<Mutex<()>>,
}

impl<S: M6Store, P: M6PackageAdapter, R: M7RuntimeAdapter, V: M7UiValidator>
    M7Application<S, P, R, V>
{
    #[must_use]
    pub fn new(core: M6Application<S, P, R>, validator: V) -> Self {
        Self {
            core,
            validator,
            sessions: Arc::new(Mutex::new(UiSessionCoordinator::default())),
            host_start: Arc::new(Mutex::new(())),
        }
    }

    pub async fn open(
        &self,
        access: &AccessContext,
        connection_id: String,
        client_instance_id: String,
        extension_id: String,
        locale: String,
        now_ms: u64,
    ) -> Result<M7UiOpenResult, M7Error> {
        require_ui_access(access, &extension_id)?;
        let locale =
            canonical_locale(&locale).ok_or_else(|| M7Error::new(M7ErrorCode::InvalidInput))?;
        let record = self
            .core
            .store
            .get_extension(access.principal().clone(), extension_id.clone())
            .await
            .map_err(map_m6)?;
        validate_open_record(&record)?;

        let session_id = OperationId::new().to_string();
        let absolute_deadline_ms = now_ms
            .checked_add(UI_ABSOLUTE_TIMEOUT_MS)
            .ok_or_else(internal)?;
        let idle_deadline_ms = now_ms
            .checked_add(UI_IDLE_TIMEOUT_MS)
            .ok_or_else(internal)?;
        {
            let mut coordinator = self.sessions.lock().await;
            coordinator.reserve(
                access,
                &connection_id,
                &client_instance_id,
                &record,
                &session_id,
                &locale,
                now_ms,
                idle_deadline_ms,
                absolute_deadline_ms,
            )?;
        }

        let activation_authority = {
            let coordinator = self.sessions.lock().await;
            coordinator.invocation(
                &session_id,
                access,
                &connection_id,
                ExtensionInvocationKind::InteractiveUiRender,
                0,
                now_ms,
            )?
        };
        let lease = match self
            .ensure_interactive_host(access, record.clone(), activation_authority, now_ms)
            .await
        {
            Ok(lease) => lease,
            Err(error) => {
                self.mark_session_stale(&session_id).await;
                return Err(error);
            }
        };
        {
            let mut coordinator = self.sessions.lock().await;
            coordinator.bind_lease(&session_id, &lease)?;
        }
        let authority = {
            let coordinator = self.sessions.lock().await;
            coordinator.invocation(
                &session_id,
                access,
                &connection_id,
                ExtensionInvocationKind::InteractiveUiRender,
                0,
                now_ms,
            )?
        };
        let document = match self
            .core
            .runtime
            .invoke_ui(
                lease.clone(),
                authority,
                ExtensionUiExport::Open {
                    session_id: session_id.clone(),
                    locale: locale.clone(),
                },
            )
            .await
        {
            Ok(Some(document)) => document,
            Ok(None) => {
                self.mark_session_stale(&session_id).await;
                return Err(internal());
            }
            Err(error) => {
                return Err(self
                    .handle_runtime_error(&session_id, lease, error, now_ms)
                    .await);
            }
        };
        if let Err(error) = self.validator.validate_document(&document) {
            self.reject_guest_document(&session_id, lease, error, now_ms)
                .await;
            return Err(validation_error(error));
        }

        let snapshot = {
            let mut coordinator = self.sessions.lock().await;
            coordinator.commit_open(&session_id, document, now_ms)?
        };
        Ok(M7UiOpenResult {
            session: M7UiSessionInfo {
                session_id,
                extension_id,
                locale,
                idle_timeout_ms: UI_IDLE_TIMEOUT_MS,
                absolute_timeout_ms: UI_ABSOLUTE_TIMEOUT_MS,
            },
            snapshot,
        })
    }

    pub async fn refresh(
        &self,
        access: &AccessContext,
        connection_id: &str,
        session_id: &str,
        expected_snapshot_revision: u64,
        now_ms: u64,
    ) -> Result<M7UiSnapshot, M7Error> {
        let call = self
            .begin_call(
                access,
                connection_id,
                session_id,
                expected_snapshot_revision,
                ExtensionInvocationKind::InteractiveUiRender,
                now_ms,
            )
            .await?;
        let document = match self
            .core
            .runtime
            .invoke_ui(
                call.lease.clone(),
                call.authority,
                ExtensionUiExport::Refresh {
                    session_id: session_id.to_owned(),
                },
            )
            .await
        {
            Ok(Some(document)) => document,
            Ok(None) => {
                self.finish_failed_call(session_id).await;
                return Err(internal());
            }
            Err(error) => {
                return Err(self
                    .handle_runtime_error(session_id, call.lease, error, now_ms)
                    .await);
            }
        };
        if let Err(error) = self.validator.validate_document(&document) {
            self.reject_guest_document(session_id, call.lease, error, now_ms)
                .await;
            return Err(validation_error(error));
        }
        let mut coordinator = self.sessions.lock().await;
        coordinator.commit_refresh(session_id, document, now_ms)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn dispatch(
        &self,
        access: &AccessContext,
        connection_id: &str,
        session_id: &str,
        expected_snapshot_revision: u64,
        sequence: u64,
        request_id: String,
        action: Vec<u8>,
        fingerprint: [u8; 32],
        now_ms: u64,
    ) -> Result<M7UiDispatchResult, M7Error> {
        if request_id.is_empty()
            || request_id.len() > 64
            || !request_id.bytes().all(|byte| (b'!'..=b'~').contains(&byte))
            || sequence == 0
            || sequence > i64::MAX as u64
        {
            return Err(self
                .reject_client_request(
                    access,
                    connection_id,
                    session_id,
                    M7ErrorCode::ActionInvalid,
                    now_ms,
                )
                .await);
        }
        let prepared = {
            let mut coordinator = self.sessions.lock().await;
            match coordinator.replay(
                access,
                connection_id,
                session_id,
                expected_snapshot_revision,
                sequence,
                &request_id,
                fingerprint,
                now_ms,
            ) {
                Ok(Some(result)) => return Ok(result),
                Ok(None) => {
                    let document = coordinator.current_document(session_id)?.to_vec();
                    self.validator.validate_action(&document, &action)
                }
                Err(error) => Err(match error.code() {
                    M7ErrorCode::ActionInvalid => M7ValidationFailure::Invalid,
                    M7ErrorCode::LimitExceeded => M7ValidationFailure::LimitExceeded,
                    _ => return Err(error),
                }),
            }
        };
        if let Err(failure) = prepared {
            let code = action_validation_error(failure).code();
            return Err(self
                .reject_client_request(access, connection_id, session_id, code, now_ms)
                .await);
        }
        let call = match self
            .begin_call(
                access,
                connection_id,
                session_id,
                expected_snapshot_revision,
                ExtensionInvocationKind::InteractiveUiAction,
                now_ms,
            )
            .await
        {
            Ok(call) => call,
            Err(error)
                if matches!(
                    error.code(),
                    M7ErrorCode::ActionInvalid | M7ErrorCode::LimitExceeded
                ) =>
            {
                return Err(self
                    .reject_client_request(access, connection_id, session_id, error.code(), now_ms)
                    .await);
            }
            Err(error) => return Err(error),
        };
        let document = match self
            .core
            .runtime
            .invoke_ui(
                call.lease.clone(),
                call.authority,
                ExtensionUiExport::Dispatch {
                    session_id: session_id.to_owned(),
                    action,
                },
            )
            .await
        {
            Ok(Some(document)) => document,
            Ok(None) => {
                self.finish_failed_call(session_id).await;
                return Err(internal());
            }
            Err(error) => {
                return Err(self
                    .handle_runtime_error(session_id, call.lease, error, now_ms)
                    .await);
            }
        };
        if let Err(error) = self.validator.validate_document(&document) {
            self.reject_guest_document(session_id, call.lease, error, now_ms)
                .await;
            return Err(validation_error(error));
        }
        let mut coordinator = self.sessions.lock().await;
        coordinator.commit_dispatch(
            session_id,
            expected_snapshot_revision,
            sequence,
            request_id,
            fingerprint,
            document,
            now_ms,
        )
    }

    pub async fn close(
        &self,
        access: &AccessContext,
        connection_id: &str,
        session_id: &str,
        now_ms: u64,
    ) -> bool {
        let session = {
            let mut coordinator = self.sessions.lock().await;
            coordinator.begin_close(access, connection_id, session_id, now_ms)
        };
        let Some(session) = session else {
            return false;
        };
        self.close_reserved_session(session, now_ms).await;
        true
    }

    pub async fn reject_client_request(
        &self,
        access: &AccessContext,
        connection_id: &str,
        session_id: &str,
        code: M7ErrorCode,
        now_ms: u64,
    ) -> M7Error {
        let closing = if matches!(
            code,
            M7ErrorCode::ActionInvalid | M7ErrorCode::LimitExceeded
        ) {
            let mut coordinator = self.sessions.lock().await;
            coordinator.record_client_violation(access, connection_id, session_id, now_ms)
        } else {
            None
        };
        if let Some(session) = closing {
            self.close_reserved_session(session, now_ms).await;
        }
        M7Error::new(code)
    }

    pub async fn close_connection(&self, connection_id: &str, now_ms: u64) {
        let sessions = {
            let mut coordinator = self.sessions.lock().await;
            coordinator.begin_close_connection(connection_id)
        };
        for session in sessions {
            self.close_reserved_session(session, now_ms).await;
        }
    }

    pub async fn maintain(&self, now_ms: u64) {
        let (expired, candidates) = {
            let mut coordinator = self.sessions.lock().await;
            let expired = coordinator.begin_close_expired(now_ms);
            let candidates = coordinator.maintenance_candidates();
            (expired, candidates)
        };
        for session in expired {
            self.close_reserved_session(session, now_ms).await;
        }
        let mut stale_ids = HashSet::new();
        for (session_id, extension_id, lease) in candidates {
            let active = self
                .core
                .runtime
                .active_lease(extension_id)
                .await
                .ok()
                .flatten();
            if active.as_ref() != Some(&lease) {
                stale_ids.insert(session_id);
            }
        }
        if !stale_ids.is_empty() {
            let mut coordinator = self.sessions.lock().await;
            for session_id in stale_ids {
                coordinator.mark_stale(&session_id);
            }
        }
    }

    async fn begin_call(
        &self,
        access: &AccessContext,
        connection_id: &str,
        session_id: &str,
        expected_snapshot_revision: u64,
        kind: ExtensionInvocationKind,
        now_ms: u64,
    ) -> Result<PreparedCall, M7Error> {
        let session = {
            let mut coordinator = self.sessions.lock().await;
            coordinator.prepare_call(
                access,
                connection_id,
                session_id,
                expected_snapshot_revision,
                now_ms,
            )?
        };
        let record = match self
            .core
            .store
            .get_extension(access.principal().clone(), session.extension_id.clone())
            .await
        {
            Ok(record) => record,
            Err(error) if error.code() == M6ErrorCode::NotInstalled => {
                self.mark_session_stale(session_id).await;
                return Err(M7Error::new(M7ErrorCode::SessionStale));
            }
            Err(error) => {
                self.finish_failed_call(session_id).await;
                return Err(map_m6(error));
            }
        };
        if let Err(error) = validate_current_session(&session, &record) {
            self.mark_session_stale(session_id).await;
            return Err(error);
        }
        let active = match self
            .core
            .runtime
            .active_lease(session.extension_id.clone())
            .await
        {
            Ok(active) => active,
            Err(error) => {
                self.finish_failed_call(session_id).await;
                return Err(map_m6(error));
            }
        };
        if active.as_ref() != Some(&session.lease) {
            self.mark_session_stale(session_id).await;
            return Err(M7Error::new(M7ErrorCode::SessionStale));
        }
        let authority = session.invocation(access.clone(), kind, now_ms)?;
        Ok(PreparedCall {
            lease: session.lease,
            authority,
        })
    }

    async fn ensure_interactive_host(
        &self,
        access: &AccessContext,
        record: ExtensionRecord,
        activation_authority: ExtensionUiInvocationAuthority,
        now_ms: u64,
    ) -> Result<ExtensionInstanceLease, M7Error> {
        let _host_start = self.host_start.lock().await;
        if let Some(lease) = self
            .core
            .runtime
            .active_lease(record.extension_id.clone())
            .await
            .map_err(map_m6)?
        {
            validate_lease(&record, &lease)?;
            return Ok(lease);
        }
        let locator = self
            .core
            .store
            .live_package_locator(access.principal().clone(), record.extension_id.clone())
            .await
            .map_err(map_m6)?;
        self.core
            .packages
            .verify_installed(record.clone(), locator)
            .await
            .map_err(map_m6)?;
        let mut context: ExtensionStartContext = self
            .core
            .store
            .prepare_instance(
                access.principal().clone(),
                record.extension_id.clone(),
                self.core.runtime.daemon_epoch(),
                now_ms,
            )
            .await
            .map_err(map_m6)?;
        context.activation_kind = ExtensionActivationKind::InteractiveUi;
        let lease = context.lease.clone();
        if let Err(error) = self
            .core
            .runtime
            .start_interactive(context, activation_authority)
            .await
        {
            self.record_crash(lease, "interactive_ui_start_failed", now_ms)
                .await;
            return Err(map_m6(error));
        }
        self.core
            .store
            .mark_instance_running(lease.clone(), now_ms)
            .await
            .map_err(map_m6)?;
        Ok(lease)
    }

    async fn close_reserved_session(&self, session: UiSession, now_ms: u64) {
        let authority = session
            .invocation(
                session.client_access.clone(),
                ExtensionInvocationKind::InteractiveUiClose,
                now_ms,
            )
            .ok();
        if let Some(authority) = authority {
            let result = self
                .core
                .runtime
                .invoke_ui(
                    session.lease.clone(),
                    authority,
                    ExtensionUiExport::Close {
                        session_id: session.session_id.clone(),
                    },
                )
                .await;
            if result
                .as_ref()
                .is_err_and(|error| error.code() == M6ErrorCode::Crashed)
            {
                self.record_crash(session.lease.clone(), "interactive_ui_close_failed", now_ms)
                    .await;
            }
        }
        {
            let mut coordinator = self.sessions.lock().await;
            coordinator.finish_close(&session.session_id);
        }
        let has_sessions = {
            let coordinator = self.sessions.lock().await;
            coordinator.has_extension_sessions(&session.extension_id)
        };
        if has_sessions {
            return;
        }
        let background_active = self
            .core
            .runtime
            .background_active(session.extension_id.clone())
            .await
            .unwrap_or(true);
        if !background_active {
            let _ = self
                .core
                .runtime
                .stop(
                    session.extension_id.clone(),
                    ExtensionStopReason::InteractiveUiIdle,
                )
                .await;
            let _ = self
                .core
                .store
                .mark_instance_stopped(session.extension_id, now_ms)
                .await;
        }
    }

    async fn reject_guest_document(
        &self,
        session_id: &str,
        lease: ExtensionInstanceLease,
        _failure: M7ValidationFailure,
        now_ms: u64,
    ) {
        self.mark_extension_sessions_stale(&lease.extension_id)
            .await;
        let _ = self
            .core
            .runtime
            .terminate_host(lease.extension_id.clone())
            .await;
        self.record_crash(lease, "portable_ui_document_invalid", now_ms)
            .await;
        self.mark_session_stale(session_id).await;
    }

    async fn record_crash(&self, lease: ExtensionInstanceLease, reason: &str, now_ms: u64) {
        if let Ok(decision) = self
            .core
            .store
            .record_instance_crash(lease, reason.to_owned(), now_ms)
            .await
            && let Some(delay_ms) = decision.restart_delay_ms
        {
            self.core.schedule_restart(decision.extension_id, delay_ms);
        }
    }

    async fn finish_failed_call(&self, session_id: &str) {
        let mut coordinator = self.sessions.lock().await;
        coordinator.finish_failed_call(session_id);
    }

    async fn handle_runtime_error(
        &self,
        session_id: &str,
        lease: ExtensionInstanceLease,
        error: M6Error,
        now_ms: u64,
    ) -> M7Error {
        if matches!(
            error.code(),
            M6ErrorCode::Crashed | M6ErrorCode::ResourceLimit
        ) {
            self.mark_extension_sessions_stale(&lease.extension_id)
                .await;
            let _ = self
                .core
                .runtime
                .terminate_host(lease.extension_id.clone())
                .await;
            self.record_crash(lease, "portable_ui_guest_failed", now_ms)
                .await;
        } else {
            self.finish_failed_call(session_id).await;
        }
        map_m6(error)
    }

    async fn mark_session_stale(&self, session_id: &str) {
        let mut coordinator = self.sessions.lock().await;
        coordinator.mark_stale(session_id);
    }

    async fn mark_extension_sessions_stale(&self, extension_id: &str) {
        let mut coordinator = self.sessions.lock().await;
        coordinator.mark_extension_stale(extension_id);
    }
}

struct PreparedCall {
    lease: ExtensionInstanceLease,
    authority: ExtensionUiInvocationAuthority,
}

#[derive(Default)]
struct UiSessionCoordinator {
    sessions: HashMap<String, UiSession>,
    stale: VecDeque<StaleSession>,
}

#[derive(Clone)]
struct UiSession {
    session_id: String,
    client_access: AccessContext,
    client_principal_id: String,
    client_connection_id: String,
    client_instance_id: String,
    extension_id: String,
    extension_principal_id: String,
    extension_instance_id: String,
    package_digest: [u8; 32],
    grant_revision: u64,
    lifecycle_generation: u64,
    snapshot_revision: u64,
    locale: String,
    created_at_ms: u64,
    absolute_deadline_ms: u64,
    idle_deadline_ms: u64,
    next_sequence: u64,
    replay: VecDeque<ReplayEntry>,
    current_document: Vec<u8>,
    lease: ExtensionInstanceLease,
    state: UiSessionState,
    in_flight: bool,
    rate_tokens: u64,
    rate_updated_at_ms: u64,
    invocation_current: Arc<AtomicBool>,
    client_violations: VecDeque<u64>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum UiSessionState {
    Opening,
    Active,
    Closing,
}

#[derive(Clone)]
struct ReplayEntry {
    sequence: u64,
    request_id: String,
    expected_snapshot_revision: u64,
    fingerprint: [u8; 32],
    resulting_snapshot_revision: u64,
    resulting_document: Vec<u8>,
}

struct StaleSession {
    session_id: String,
    client_principal_id: String,
    client_connection_id: String,
}

impl UiSessionCoordinator {
    #[allow(clippy::too_many_arguments)]
    fn reserve(
        &mut self,
        access: &AccessContext,
        connection_id: &str,
        client_instance_id: &str,
        record: &ExtensionRecord,
        session_id: &str,
        locale: &str,
        now_ms: u64,
        idle_deadline_ms: u64,
        absolute_deadline_ms: u64,
    ) -> Result<(), M7Error> {
        if self.sessions.len() >= MAX_SESSIONS_TOTAL
            || self
                .sessions
                .values()
                .filter(|session| session.extension_id == record.extension_id)
                .count()
                >= MAX_SESSIONS_PER_EXTENSION
            || self
                .sessions
                .values()
                .filter(|session| session.client_connection_id == connection_id)
                .count()
                >= MAX_SESSIONS_PER_CONNECTION
        {
            return Err(M7Error::new(M7ErrorCode::LimitExceeded));
        }
        let placeholder = ExtensionInstanceLease {
            lease_id: String::new(),
            extension_id: record.extension_id.clone(),
            instance_id: String::new(),
            principal_id: access.principal().clone(),
            publisher_fingerprint: record.publisher_fingerprint.clone(),
            grant_revision: record.grant_revision,
            lifecycle_generation: record.lifecycle_generation,
            daemon_epoch: String::new(),
            expires_at_ms: 0,
        };
        self.sessions.insert(
            session_id.to_owned(),
            UiSession {
                session_id: session_id.to_owned(),
                client_access: access.clone(),
                client_principal_id: access.principal().as_str().to_owned(),
                client_connection_id: connection_id.to_owned(),
                client_instance_id: client_instance_id.to_owned(),
                extension_id: record.extension_id.clone(),
                extension_principal_id: String::new(),
                extension_instance_id: String::new(),
                package_digest: record.package_digest,
                grant_revision: record.grant_revision.get(),
                lifecycle_generation: record.lifecycle_generation.get(),
                snapshot_revision: 0,
                locale: locale.to_owned(),
                created_at_ms: now_ms,
                absolute_deadline_ms,
                idle_deadline_ms,
                next_sequence: 1,
                replay: VecDeque::new(),
                current_document: Vec::new(),
                lease: placeholder,
                state: UiSessionState::Opening,
                in_flight: true,
                rate_tokens: RATE_CAPACITY_MILLI_TOKENS,
                rate_updated_at_ms: now_ms,
                invocation_current: Arc::new(AtomicBool::new(true)),
                client_violations: VecDeque::new(),
            },
        );
        Ok(())
    }

    fn bind_lease(
        &mut self,
        session_id: &str,
        lease: &ExtensionInstanceLease,
    ) -> Result<(), M7Error> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| M7Error::new(M7ErrorCode::SessionStale))?;
        session.extension_principal_id = lease.principal_id.as_str().to_owned();
        session.extension_instance_id = lease.instance_id.clone();
        session.lease = lease.clone();
        Ok(())
    }

    fn invocation(
        &self,
        session_id: &str,
        access: &AccessContext,
        connection_id: &str,
        kind: ExtensionInvocationKind,
        snapshot_revision: u64,
        now_ms: u64,
    ) -> Result<ExtensionUiInvocationAuthority, M7Error> {
        let session = self
            .sessions
            .get(session_id)
            .filter(|session| {
                session.client_principal_id == access.principal().as_str()
                    && session.client_connection_id == connection_id
                    && session.state != UiSessionState::Closing
            })
            .ok_or_else(|| M7Error::new(M7ErrorCode::SessionStale))?;
        session
            .invocation(access.clone(), kind, now_ms)
            .map(|mut value| {
                value.snapshot_revision = snapshot_revision;
                value
            })
    }

    fn commit_open(
        &mut self,
        session_id: &str,
        document: Vec<u8>,
        now_ms: u64,
    ) -> Result<M7UiSnapshot, M7Error> {
        let session = self
            .sessions
            .get_mut(session_id)
            .filter(|session| session.state == UiSessionState::Opening)
            .ok_or_else(|| M7Error::new(M7ErrorCode::SessionStale))?;
        session.snapshot_revision = 1;
        session.current_document = document;
        session.in_flight = false;
        session.state = UiSessionState::Active;
        session.idle_deadline_ms = now_ms
            .checked_add(UI_IDLE_TIMEOUT_MS)
            .ok_or_else(internal)?;
        Ok(session.snapshot())
    }

    fn prepare_call(
        &mut self,
        access: &AccessContext,
        connection_id: &str,
        session_id: &str,
        expected_snapshot_revision: u64,
        now_ms: u64,
    ) -> Result<UiSession, M7Error> {
        let session = self.owned_session_mut(access, connection_id, session_id)?;
        session.check_live(now_ms)?;
        require_ui_access(access, &session.extension_id)?;
        if session.in_flight {
            return Err(M7Error::new(M7ErrorCode::ActionInvalid));
        }
        if expected_snapshot_revision != session.snapshot_revision {
            return Err(M7Error::new(M7ErrorCode::SnapshotStale));
        }
        session.consume_rate(now_ms)?;
        session.in_flight = true;
        Ok(session.clone())
    }

    #[allow(clippy::too_many_arguments)]
    fn replay(
        &mut self,
        access: &AccessContext,
        connection_id: &str,
        session_id: &str,
        expected_snapshot_revision: u64,
        sequence: u64,
        request_id: &str,
        fingerprint: [u8; 32],
        now_ms: u64,
    ) -> Result<Option<M7UiDispatchResult>, M7Error> {
        let session = self.owned_session_mut(access, connection_id, session_id)?;
        session.check_live(now_ms)?;
        require_ui_access(access, &session.extension_id)?;
        if let Some(entry) = session
            .replay
            .iter()
            .find(|entry| entry.request_id == request_id || entry.sequence == sequence)
            .cloned()
        {
            let exact = entry.request_id == request_id
                && entry.sequence == sequence
                && entry.expected_snapshot_revision == expected_snapshot_revision
                && entry.fingerprint == fingerprint;
            if !exact {
                return Err(M7Error::new(M7ErrorCode::ActionInvalid));
            }
            if entry.resulting_snapshot_revision != session.snapshot_revision {
                return Err(M7Error::new(M7ErrorCode::SnapshotStale));
            }
            session.consume_rate(now_ms)?;
            return Ok(Some(M7UiDispatchResult {
                snapshot: M7UiSnapshot {
                    session_id: session.session_id.clone(),
                    snapshot_revision: entry.resulting_snapshot_revision,
                    document: entry.resulting_document.clone(),
                },
                replayed: true,
            }));
        }
        if sequence != session.next_sequence {
            return Err(M7Error::new(M7ErrorCode::ActionInvalid));
        }
        if expected_snapshot_revision != session.snapshot_revision {
            return Err(M7Error::new(M7ErrorCode::SnapshotStale));
        }
        Ok(None)
    }

    fn current_document(&self, session_id: &str) -> Result<&[u8], M7Error> {
        self.sessions
            .get(session_id)
            .map(|session| session.current_document.as_slice())
            .ok_or_else(|| M7Error::new(M7ErrorCode::SessionNotFound))
    }

    fn commit_refresh(
        &mut self,
        session_id: &str,
        document: Vec<u8>,
        now_ms: u64,
    ) -> Result<M7UiSnapshot, M7Error> {
        let session = self
            .sessions
            .get_mut(session_id)
            .filter(|session| session.state == UiSessionState::Active)
            .ok_or_else(|| M7Error::new(M7ErrorCode::SessionStale))?;
        let revision = session
            .snapshot_revision
            .checked_add(1)
            .ok_or_else(internal)?;
        session.snapshot_revision = revision;
        session.current_document = document;
        session.in_flight = false;
        session.idle_deadline_ms = now_ms
            .checked_add(UI_IDLE_TIMEOUT_MS)
            .ok_or_else(internal)?;
        Ok(session.snapshot())
    }

    #[allow(clippy::too_many_arguments)]
    fn commit_dispatch(
        &mut self,
        session_id: &str,
        expected_snapshot_revision: u64,
        sequence: u64,
        request_id: String,
        fingerprint: [u8; 32],
        document: Vec<u8>,
        now_ms: u64,
    ) -> Result<M7UiDispatchResult, M7Error> {
        let session = self
            .sessions
            .get_mut(session_id)
            .filter(|session| session.state == UiSessionState::Active)
            .ok_or_else(|| M7Error::new(M7ErrorCode::SessionStale))?;
        if session.snapshot_revision != expected_snapshot_revision
            || session.next_sequence != sequence
        {
            return Err(M7Error::new(M7ErrorCode::SessionStale));
        }
        let revision = session
            .snapshot_revision
            .checked_add(1)
            .ok_or_else(internal)?;
        session.next_sequence = session.next_sequence.checked_add(1).ok_or_else(internal)?;
        session.snapshot_revision = revision;
        session.current_document = document.clone();
        session.in_flight = false;
        session.idle_deadline_ms = now_ms
            .checked_add(UI_IDLE_TIMEOUT_MS)
            .ok_or_else(internal)?;
        session.replay.push_back(ReplayEntry {
            sequence,
            request_id,
            expected_snapshot_revision,
            fingerprint,
            resulting_snapshot_revision: revision,
            resulting_document: document,
        });
        while session.replay.len() > MAX_REPLAY_ENTRIES {
            session.replay.pop_front();
        }
        Ok(M7UiDispatchResult {
            snapshot: session.snapshot(),
            replayed: false,
        })
    }

    fn begin_close(
        &mut self,
        access: &AccessContext,
        connection_id: &str,
        session_id: &str,
        _now_ms: u64,
    ) -> Option<UiSession> {
        let session = self.sessions.get_mut(session_id)?;
        if session.client_principal_id != access.principal().as_str()
            || session.client_connection_id != connection_id
            || session.state != UiSessionState::Active
        {
            return None;
        }
        session.state = UiSessionState::Closing;
        session.invocation_current.store(false, Ordering::Release);
        session.replay.clear();
        Some(session.clone())
    }

    fn begin_close_connection(&mut self, connection_id: &str) -> Vec<UiSession> {
        self.sessions
            .values_mut()
            .filter(|session| {
                session.client_connection_id == connection_id
                    && session.state == UiSessionState::Active
            })
            .map(|session| {
                session.state = UiSessionState::Closing;
                session.invocation_current.store(false, Ordering::Release);
                session.replay.clear();
                session.clone()
            })
            .collect()
    }

    fn begin_close_expired(&mut self, now_ms: u64) -> Vec<UiSession> {
        self.sessions
            .values_mut()
            .filter(|session| {
                session.state == UiSessionState::Active
                    && (now_ms >= session.idle_deadline_ms
                        || now_ms >= session.absolute_deadline_ms)
            })
            .map(|session| {
                session.state = UiSessionState::Closing;
                session.invocation_current.store(false, Ordering::Release);
                session.replay.clear();
                session.clone()
            })
            .collect()
    }

    fn record_client_violation(
        &mut self,
        access: &AccessContext,
        connection_id: &str,
        session_id: &str,
        now_ms: u64,
    ) -> Option<UiSession> {
        let session = self
            .owned_session_mut(access, connection_id, session_id)
            .ok()?;
        while session
            .client_violations
            .front()
            .is_some_and(|occurred_at_ms| {
                now_ms.saturating_sub(*occurred_at_ms) >= CLIENT_VIOLATION_WINDOW_MS
            })
        {
            session.client_violations.pop_front();
        }
        session.client_violations.push_back(now_ms);
        if session.client_violations.len() < CLIENT_VIOLATION_CLOSE_THRESHOLD {
            return None;
        }
        session.state = UiSessionState::Closing;
        session.invocation_current.store(false, Ordering::Release);
        session.replay.clear();
        Some(session.clone())
    }

    fn finish_close(&mut self, session_id: &str) {
        self.mark_stale(session_id);
    }

    fn finish_failed_call(&mut self, session_id: &str) {
        if let Some(session) = self.sessions.get_mut(session_id)
            && session.state == UiSessionState::Active
        {
            session.in_flight = false;
        }
    }

    fn mark_stale(&mut self, session_id: &str) {
        if let Some(mut session) = self.sessions.remove(session_id) {
            session.invocation_current.store(false, Ordering::Release);
            session.replay.clear();
            session.current_document.clear();
            session.client_violations.clear();
            self.stale.push_back(StaleSession {
                session_id: session.session_id,
                client_principal_id: session.client_principal_id,
                client_connection_id: session.client_connection_id,
            });
            while self.stale.len() > MAX_STALE_SESSION_EVIDENCE {
                self.stale.pop_front();
            }
        }
    }

    fn mark_extension_stale(&mut self, extension_id: &str) {
        let ids = self
            .sessions
            .values()
            .filter(|session| session.extension_id == extension_id)
            .map(|session| session.session_id.clone())
            .collect::<Vec<_>>();
        for session_id in ids {
            self.mark_stale(&session_id);
        }
    }

    fn maintenance_candidates(&self) -> Vec<(String, String, ExtensionInstanceLease)> {
        self.sessions
            .values()
            .filter(|session| session.state == UiSessionState::Active)
            .map(|session| {
                (
                    session.session_id.clone(),
                    session.extension_id.clone(),
                    session.lease.clone(),
                )
            })
            .collect()
    }

    fn has_extension_sessions(&self, extension_id: &str) -> bool {
        self.sessions.values().any(|session| {
            session.extension_id == extension_id && session.state != UiSessionState::Closing
        })
    }

    #[cfg(test)]
    fn owned_session(
        &self,
        access: &AccessContext,
        connection_id: &str,
        session_id: &str,
    ) -> Result<&UiSession, M7Error> {
        if let Some(session) = self.sessions.get(session_id) {
            if session.client_principal_id == access.principal().as_str()
                && session.client_connection_id == connection_id
            {
                return if session.state == UiSessionState::Active {
                    Ok(session)
                } else {
                    Err(M7Error::new(M7ErrorCode::SessionStale))
                };
            }
            return Err(M7Error::new(M7ErrorCode::SessionNotFound));
        }
        if self.stale.iter().any(|session| {
            session.session_id == session_id
                && session.client_principal_id == access.principal().as_str()
                && session.client_connection_id == connection_id
        }) {
            Err(M7Error::new(M7ErrorCode::SessionStale))
        } else {
            Err(M7Error::new(M7ErrorCode::SessionNotFound))
        }
    }

    fn owned_session_mut(
        &mut self,
        access: &AccessContext,
        connection_id: &str,
        session_id: &str,
    ) -> Result<&mut UiSession, M7Error> {
        let stale_owned = self.stale.iter().any(|session| {
            session.session_id == session_id
                && session.client_principal_id == access.principal().as_str()
                && session.client_connection_id == connection_id
        });
        match self.sessions.get_mut(session_id) {
            Some(session)
                if session.client_principal_id == access.principal().as_str()
                    && session.client_connection_id == connection_id =>
            {
                if session.state == UiSessionState::Active {
                    Ok(session)
                } else {
                    Err(M7Error::new(M7ErrorCode::SessionStale))
                }
            }
            Some(_) => Err(M7Error::new(M7ErrorCode::SessionNotFound)),
            None if stale_owned => Err(M7Error::new(M7ErrorCode::SessionStale)),
            None => Err(M7Error::new(M7ErrorCode::SessionNotFound)),
        }
    }
}

impl UiSession {
    fn snapshot(&self) -> M7UiSnapshot {
        M7UiSnapshot {
            session_id: self.session_id.clone(),
            snapshot_revision: self.snapshot_revision,
            document: self.current_document.clone(),
        }
    }

    fn invocation(
        &self,
        client_access: AccessContext,
        kind: ExtensionInvocationKind,
        now_ms: u64,
    ) -> Result<ExtensionUiInvocationAuthority, M7Error> {
        let deadline_ms = now_ms
            .checked_add(2_000)
            .map(|value| {
                value
                    .min(self.absolute_deadline_ms)
                    .min(self.idle_deadline_ms)
            })
            .ok_or_else(internal)?;
        Ok(ExtensionUiInvocationAuthority {
            kind,
            client_access,
            client_connection_id: self.client_connection_id.clone(),
            client_instance_id: self.client_instance_id.clone(),
            session_id: self.session_id.clone(),
            snapshot_revision: self.snapshot_revision,
            deadline_ms,
            current: Arc::clone(&self.invocation_current),
        })
    }

    fn check_live(&self, now_ms: u64) -> Result<(), M7Error> {
        if self.locale.is_empty()
            || self.created_at_ms >= self.absolute_deadline_ms
            || now_ms >= self.idle_deadline_ms
            || now_ms >= self.absolute_deadline_ms
        {
            Err(M7Error::new(M7ErrorCode::SessionStale))
        } else {
            Ok(())
        }
    }

    fn consume_rate(&mut self, now_ms: u64) -> Result<(), M7Error> {
        let elapsed = now_ms.saturating_sub(self.rate_updated_at_ms);
        self.rate_tokens = self
            .rate_tokens
            .saturating_add(elapsed)
            .min(RATE_CAPACITY_MILLI_TOKENS);
        self.rate_updated_at_ms = now_ms;
        if self.rate_tokens < RATE_TOKEN_COST {
            return Err(M7Error::new(M7ErrorCode::LimitExceeded));
        }
        self.rate_tokens -= RATE_TOKEN_COST;
        Ok(())
    }
}

fn require_ui_access(access: &AccessContext, extension_id: &str) -> Result<(), M7Error> {
    access
        .require(Permission::ExtensionsRead)
        .and_then(|()| access.require_extension_ui_scope(extension_id))
        .map_err(|_| M7Error::new(M7ErrorCode::PermissionDenied))
}

fn validate_open_record(record: &ExtensionRecord) -> Result<(), M7Error> {
    if record.desired_state != ExtensionDesiredState::Enabled {
        return Err(M7Error::new(M7ErrorCode::NotEnabled));
    }
    if record.quarantine_state != ExtensionQuarantineState::Clear {
        return Err(M7Error::new(M7ErrorCode::Quarantined));
    }
    match record.ui_protocol {
        Some(ExtensionUiProtocol::PortableV1) => Ok(()),
        None => Err(M7Error::new(M7ErrorCode::UiNotAvailable)),
    }
}

fn validate_current_session(session: &UiSession, record: &ExtensionRecord) -> Result<(), M7Error> {
    if record.desired_state != ExtensionDesiredState::Enabled
        || record.quarantine_state != ExtensionQuarantineState::Clear
        || record.ui_protocol != Some(ExtensionUiProtocol::PortableV1)
        || session.package_digest != record.package_digest
        || session.grant_revision != record.grant_revision.get()
        || session.lifecycle_generation != record.lifecycle_generation.get()
        || session.extension_id != record.extension_id
    {
        return Err(M7Error::new(M7ErrorCode::SessionStale));
    }
    validate_lease(record, &session.lease)
}

fn validate_lease(record: &ExtensionRecord, lease: &ExtensionInstanceLease) -> Result<(), M7Error> {
    if lease.extension_id != record.extension_id
        || lease.grant_revision != record.grant_revision
        || lease.lifecycle_generation != record.lifecycle_generation
    {
        Err(M7Error::new(M7ErrorCode::SessionStale))
    } else {
        Ok(())
    }
}

fn canonical_locale(value: &str) -> Option<String> {
    if !(2..=15).contains(&value.len()) || !value.is_ascii() {
        return None;
    }
    let parts = value.split('-').collect::<Vec<_>>();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    let language = parts[0];
    if !(2..=3).contains(&language.len()) || !language.bytes().all(|byte| byte.is_ascii_lowercase())
    {
        return None;
    }
    let mut index = 1;
    if let Some(script) = parts.get(index)
        && script.len() == 4
        && script.as_bytes()[0].is_ascii_uppercase()
        && script.as_bytes()[1..].iter().all(u8::is_ascii_lowercase)
    {
        index += 1;
    }
    if let Some(region) = parts.get(index)
        && ((region.len() == 2 && region.bytes().all(|byte| byte.is_ascii_uppercase()))
            || (region.len() == 3 && region.bytes().all(|byte| byte.is_ascii_digit())))
    {
        index += 1;
    }
    (index == parts.len()).then(|| value.to_owned())
}

fn validation_error(failure: M7ValidationFailure) -> M7Error {
    match failure {
        M7ValidationFailure::Invalid => M7Error::new(M7ErrorCode::DocumentInvalid),
        M7ValidationFailure::LimitExceeded => M7Error::new(M7ErrorCode::LimitExceeded),
    }
}

fn action_validation_error(failure: M7ValidationFailure) -> M7Error {
    match failure {
        M7ValidationFailure::Invalid => M7Error::new(M7ErrorCode::ActionInvalid),
        M7ValidationFailure::LimitExceeded => M7Error::new(M7ErrorCode::LimitExceeded),
    }
}

fn map_m6(error: M6Error) -> M7Error {
    let code = match error.code() {
        M6ErrorCode::InvalidInput => M7ErrorCode::InvalidInput,
        M6ErrorCode::PermissionDenied => M7ErrorCode::ExtensionPermissionDenied,
        M6ErrorCode::NotInstalled => M7ErrorCode::NotInstalled,
        M6ErrorCode::ScopeDenied => M7ErrorCode::ExtensionScopeDenied,
        M6ErrorCode::InstanceStale => M7ErrorCode::InstanceStale,
        M6ErrorCode::ResourceLimit | M6ErrorCode::DataQuotaExceeded => M7ErrorCode::ResourceLimit,
        M6ErrorCode::Crashed => M7ErrorCode::Crashed,
        M6ErrorCode::Quarantined => M7ErrorCode::Quarantined,
        M6ErrorCode::StoreUnavailable => M7ErrorCode::StoreUnavailable,
        M6ErrorCode::ManifestInvalid
        | M6ErrorCode::PackageInvalid
        | M6ErrorCode::PackageUntrusted
        | M6ErrorCode::PublisherConfirmationRequired
        | M6ErrorCode::SignatureInvalid
        | M6ErrorCode::AlreadyInstalled
        | M6ErrorCode::ProjectNotFound
        | M6ErrorCode::ApiUnsupported
        | M6ErrorCode::PlanStale
        | M6ErrorCode::DataOwnerMismatch
        | M6ErrorCode::RecoveryRequired
        | M6ErrorCode::RevisionConflict
        | M6ErrorCode::IdempotencyConflict
        | M6ErrorCode::Internal => M7ErrorCode::Internal,
    };
    M7Error::new(code)
}

fn internal() -> M7Error {
    M7Error::new(M7ErrorCode::Internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_subset_is_canonical_and_bounded() {
        assert_eq!(canonical_locale("en"), Some("en".to_owned()));
        assert_eq!(
            canonical_locale("zh-Hans-CN"),
            Some("zh-Hans-CN".to_owned())
        );
        assert_eq!(canonical_locale("en-US"), Some("en-US".to_owned()));
        assert_eq!(canonical_locale("EN-us"), None);
        assert_eq!(canonical_locale("en-US-private"), None);
    }

    #[test]
    fn exact_scopes_do_not_expand_each_other() {
        let access = AccessContext::new(
            crate::PrincipalId::parse("client:test").expect("principal"),
            [Permission::ProjectsRead, Permission::ExtensionsUiUse],
        )
        .with_project_read_scopes(["project-a".to_owned()])
        .with_extension_ui_scopes(["dev.example.a".to_owned()]);
        assert!(access.require_project_read_scope("project-a").is_ok());
        assert!(access.require_project_read_scope("project-b").is_err());
        assert!(access.require_extension_ui_scope("dev.example.a").is_ok());
        assert!(access.require_extension_ui_scope("dev.example.b").is_err());
    }

    fn extension_record() -> ExtensionRecord {
        ExtensionRecord {
            extension_id: "dev.example.fixture".to_owned(),
            version: "1.0.0".to_owned(),
            api_major: 1,
            package_digest: [7; 32],
            publisher_fingerprint: format!("ed25519-sha256:{}", "a".repeat(64)),
            trust_decision: crate::ExtensionTrustDecision::Official,
            desired_state: ExtensionDesiredState::Enabled,
            quarantine_state: ExtensionQuarantineState::Clear,
            runtime_state: crate::ExtensionRuntimeState::Running,
            grant_revision: crate::Revision::INITIAL,
            lifecycle_generation: crate::Revision::INITIAL,
            revision: crate::Revision::INITIAL,
            ui_protocol: Some(ExtensionUiProtocol::PortableV1),
        }
    }

    fn lease() -> ExtensionInstanceLease {
        ExtensionInstanceLease {
            lease_id: "00000000-0000-4000-8000-000000000001".to_owned(),
            extension_id: "dev.example.fixture".to_owned(),
            instance_id: "00000000-0000-4000-8000-000000000002".to_owned(),
            principal_id: crate::PrincipalId::parse("extension-instance:test").expect("principal"),
            publisher_fingerprint: format!("ed25519-sha256:{}", "a".repeat(64)),
            grant_revision: crate::Revision::INITIAL,
            lifecycle_generation: crate::Revision::INITIAL,
            daemon_epoch: "00000000-0000-4000-8000-000000000003".to_owned(),
            expires_at_ms: 60_000,
        }
    }

    fn open_session() -> (UiSessionCoordinator, AccessContext) {
        let access = AccessContext::local_owner();
        let mut coordinator = UiSessionCoordinator::default();
        coordinator
            .reserve(
                &access,
                "connection-a",
                "client-a",
                &extension_record(),
                "00000000-0000-4000-8000-000000000004",
                "en-US",
                1_000,
                301_000,
                3_601_000,
            )
            .expect("reserve");
        coordinator
            .bind_lease("00000000-0000-4000-8000-000000000004", &lease())
            .expect("bind");
        coordinator
            .commit_open(
                "00000000-0000-4000-8000-000000000004",
                b"document-1".to_vec(),
                1_000,
            )
            .expect("commit open");
        (coordinator, access)
    }

    #[test]
    fn replay_is_current_only_and_guest_call_is_not_reserved_twice() {
        let (mut coordinator, access) = open_session();
        let session_id = "00000000-0000-4000-8000-000000000004";
        assert!(
            coordinator
                .replay(
                    &access,
                    "connection-a",
                    session_id,
                    1,
                    1,
                    "request-1",
                    [9; 32],
                    2_000,
                )
                .expect("new request")
                .is_none()
        );
        coordinator
            .prepare_call(&access, "connection-a", session_id, 1, 2_000)
            .expect("prepare");
        coordinator
            .commit_dispatch(
                session_id,
                1,
                1,
                "request-1".to_owned(),
                [9; 32],
                b"document-2".to_vec(),
                2_000,
            )
            .expect("dispatch");
        let replay = coordinator
            .replay(
                &access,
                "connection-a",
                session_id,
                1,
                1,
                "request-1",
                [9; 32],
                2_001,
            )
            .expect("replay")
            .expect("cached");
        assert!(replay.replayed);
        assert_eq!(replay.snapshot.document, b"document-2");

        coordinator
            .commit_refresh(session_id, b"document-3".to_vec(), 2_002)
            .expect("refresh");
        assert_eq!(
            coordinator
                .replay(
                    &access,
                    "connection-a",
                    session_id,
                    1,
                    1,
                    "request-1",
                    [9; 32],
                    2_003,
                )
                .expect_err("historical replay")
                .code(),
            M7ErrorCode::SnapshotStale
        );
    }

    #[test]
    fn close_marks_stale_before_cleanup_and_releases_payloads() {
        let (mut coordinator, access) = open_session();
        let session_id = "00000000-0000-4000-8000-000000000004";
        let authority = coordinator
            .invocation(
                session_id,
                &access,
                "connection-a",
                ExtensionInvocationKind::InteractiveUiAction,
                1,
                2_000,
            )
            .expect("current authority");
        assert!(authority.is_current());
        let closing = coordinator
            .begin_close(&access, "connection-a", session_id, 2_000)
            .expect("owned session");
        assert!(!authority.is_current());
        assert!(closing.replay.is_empty());
        assert_eq!(
            coordinator
                .prepare_call(&access, "connection-a", session_id, 1, 2_001)
                .err()
                .expect("closing rejects calls")
                .code(),
            M7ErrorCode::SessionStale
        );
        coordinator.finish_close(session_id);
        assert!(!coordinator.sessions.contains_key(session_id));
        assert_eq!(
            coordinator
                .owned_session(&access, "connection-a", session_id)
                .err()
                .expect("closed session")
                .code(),
            M7ErrorCode::SessionStale
        );
        assert_eq!(
            coordinator
                .owned_session(&access, "connection-b", session_id)
                .err()
                .expect("other connection")
                .code(),
            M7ErrorCode::SessionNotFound
        );
    }

    #[test]
    fn expired_sessions_are_reserved_for_close_before_maintenance_checks() {
        let (mut coordinator, access) = open_session();
        let session_id = "00000000-0000-4000-8000-000000000004";
        let expired = coordinator.begin_close_expired(301_000);
        assert_eq!(expired.len(), 1);
        assert!(expired[0].replay.is_empty());
        assert!(coordinator.maintenance_candidates().is_empty());
        assert_eq!(
            coordinator
                .prepare_call(&access, "connection-a", session_id, 1, 301_000)
                .err()
                .expect("expired session rejects calls")
                .code(),
            M7ErrorCode::SessionStale
        );
    }

    #[test]
    fn third_client_violation_closes_only_the_owned_session() {
        let (mut coordinator, access) = open_session();
        let session_id = "00000000-0000-4000-8000-000000000004";
        assert!(
            coordinator
                .record_client_violation(&access, "connection-a", session_id, 2_000)
                .is_none()
        );
        assert!(
            coordinator
                .record_client_violation(&access, "connection-a", session_id, 2_001)
                .is_none()
        );
        let closing = coordinator
            .record_client_violation(&access, "connection-a", session_id, 2_002)
            .expect("third violation closes");
        assert!(closing.replay.is_empty());
        assert!(!closing.invocation_current.load(Ordering::Acquire));
        assert_eq!(
            coordinator
                .owned_session(&access, "connection-a", session_id)
                .err()
                .expect("session is closing")
                .code(),
            M7ErrorCode::SessionStale
        );
    }

    #[test]
    fn established_session_maps_lifecycle_and_digest_changes_to_stale() {
        let mut session_record = extension_record();
        let (coordinator, _) = open_session();
        let current = coordinator
            .sessions
            .get("00000000-0000-4000-8000-000000000004")
            .expect("session");
        assert_eq!(validate_current_session(current, &session_record), Ok(()));
        session_record.desired_state = ExtensionDesiredState::InstalledDisabled;
        assert_eq!(
            validate_current_session(current, &session_record)
                .expect_err("disabled session")
                .code(),
            M7ErrorCode::SessionStale
        );
        session_record.desired_state = ExtensionDesiredState::Enabled;
        session_record.package_digest = [8; 32];
        assert_eq!(
            validate_current_session(current, &session_record)
                .expect_err("digest change")
                .code(),
            M7ErrorCode::SessionStale
        );
    }

    #[test]
    fn session_limits_enforce_extension_connection_and_daemon_caps() {
        let access = AccessContext::local_owner();
        let mut per_extension = UiSessionCoordinator::default();
        let record = extension_record();
        for index in 0..MAX_SESSIONS_PER_EXTENSION {
            reserve_opening(
                &mut per_extension,
                &access,
                &record,
                &format!("extension-connection-{index}"),
                &format!("extension-session-{index}"),
            )
            .expect("within extension cap");
        }
        assert_eq!(
            reserve_opening(
                &mut per_extension,
                &access,
                &record,
                "extension-connection-overflow",
                "extension-session-overflow",
            )
            .expect_err("extension session cap")
            .code(),
            M7ErrorCode::LimitExceeded
        );

        let mut per_connection = UiSessionCoordinator::default();
        for index in 0..MAX_SESSIONS_PER_CONNECTION {
            let mut record = extension_record();
            record.extension_id = format!("dev.example.connection-{index}");
            reserve_opening(
                &mut per_connection,
                &access,
                &record,
                "shared-connection",
                &format!("connection-session-{index}"),
            )
            .expect("within connection cap");
        }
        let mut record = extension_record();
        record.extension_id = "dev.example.connection-overflow".to_owned();
        assert_eq!(
            reserve_opening(
                &mut per_connection,
                &access,
                &record,
                "shared-connection",
                "connection-session-overflow",
            )
            .expect_err("connection session cap")
            .code(),
            M7ErrorCode::LimitExceeded
        );

        let mut total = UiSessionCoordinator::default();
        for index in 0..MAX_SESSIONS_TOTAL {
            let mut record = extension_record();
            record.extension_id = format!("dev.example.total-{index}");
            reserve_opening(
                &mut total,
                &access,
                &record,
                &format!("total-connection-{index}"),
                &format!("total-session-{index}"),
            )
            .expect("within daemon cap");
        }
        let mut record = extension_record();
        record.extension_id = "dev.example.total-overflow".to_owned();
        assert_eq!(
            reserve_opening(
                &mut total,
                &access,
                &record,
                "total-connection-overflow",
                "total-session-overflow",
            )
            .expect_err("daemon session cap")
            .code(),
            M7ErrorCode::LimitExceeded
        );
    }

    #[test]
    fn rate_limit_refills_and_single_in_flight_prevents_reordering() {
        let (mut coordinator, access) = open_session();
        let session_id = "00000000-0000-4000-8000-000000000004";
        coordinator
            .prepare_call(&access, "connection-a", session_id, 1, 2_000)
            .expect("first call");
        assert_eq!(
            coordinator
                .prepare_call(&access, "connection-a", session_id, 1, 2_000)
                .err()
                .expect("second in-flight call")
                .code(),
            M7ErrorCode::ActionInvalid
        );
        coordinator
            .commit_refresh(session_id, b"document-2".to_vec(), 2_000)
            .expect("complete first call");

        for revision in 2..=10 {
            coordinator
                .prepare_call(&access, "connection-a", session_id, revision, 2_000)
                .expect("within burst");
            coordinator
                .commit_refresh(
                    session_id,
                    format!("document-{}", revision + 1).into_bytes(),
                    2_000,
                )
                .expect("commit bounded refresh");
        }
        assert_eq!(
            coordinator
                .prepare_call(&access, "connection-a", session_id, 11, 2_000)
                .err()
                .expect("burst exhausted")
                .code(),
            M7ErrorCode::LimitExceeded
        );
        coordinator
            .prepare_call(&access, "connection-a", session_id, 11, 3_000)
            .expect("one token refilled after one second");
    }

    #[test]
    fn idle_absolute_and_revision_overflow_fail_closed() {
        let (mut idle, access) = open_session();
        let session_id = "00000000-0000-4000-8000-000000000004";
        assert_eq!(
            idle.prepare_call(&access, "connection-a", session_id, 1, 301_000)
                .err()
                .expect("idle timeout")
                .code(),
            M7ErrorCode::SessionStale
        );

        let (mut absolute, access) = open_session();
        absolute
            .sessions
            .get_mut(session_id)
            .expect("session")
            .idle_deadline_ms = 4_000_000;
        assert_eq!(
            absolute
                .prepare_call(&access, "connection-a", session_id, 1, 3_601_000)
                .err()
                .expect("absolute timeout")
                .code(),
            M7ErrorCode::SessionStale
        );

        let (mut overflow, _) = open_session();
        overflow
            .sessions
            .get_mut(session_id)
            .expect("session")
            .snapshot_revision = u64::MAX;
        assert_eq!(
            overflow
                .commit_refresh(session_id, b"never-published".to_vec(), 2_000)
                .expect_err("revision overflow")
                .code(),
            M7ErrorCode::Internal
        );
        assert_eq!(
            overflow
                .sessions
                .get(session_id)
                .expect("session retained")
                .snapshot_revision,
            u64::MAX
        );
    }

    #[test]
    fn sensitive_documents_and_replay_payloads_are_inaccessible_after_close() {
        let (mut coordinator, access) = open_session();
        let session_id = "00000000-0000-4000-8000-000000000004";
        let sensitive = b"m7-sensitive-form-value".to_vec();
        coordinator
            .prepare_call(&access, "connection-a", session_id, 1, 2_000)
            .expect("prepare sensitive result");
        coordinator
            .commit_dispatch(
                session_id,
                1,
                1,
                "request-sensitive".to_owned(),
                [4; 32],
                sensitive.clone(),
                2_000,
            )
            .expect("commit in-memory result");
        let closing = coordinator
            .begin_close(&access, "connection-a", session_id, 2_001)
            .expect("close session");
        assert!(closing.replay.is_empty());
        coordinator.finish_close(session_id);
        assert!(!coordinator.sessions.contains_key(session_id));
        assert!(coordinator.stale.iter().all(|entry| {
            !entry.session_id.contains("m7-sensitive-form-value")
                && !entry
                    .client_principal_id
                    .contains("m7-sensitive-form-value")
                && !entry
                    .client_connection_id
                    .contains("m7-sensitive-form-value")
        }));
        let error = coordinator
            .current_document(session_id)
            .expect_err("closed payload unavailable");
        assert!(!format!("{error:?} {error}").contains("m7-sensitive-form-value"));
    }

    fn reserve_opening(
        coordinator: &mut UiSessionCoordinator,
        access: &AccessContext,
        record: &ExtensionRecord,
        connection_id: &str,
        session_id: &str,
    ) -> Result<(), M7Error> {
        coordinator.reserve(
            access,
            connection_id,
            "client-instance",
            record,
            session_id,
            "en-US",
            1_000,
            301_000,
            3_601_000,
        )
    }
}
