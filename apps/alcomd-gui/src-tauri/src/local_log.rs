//! GUI-owned system log adapter; never accepts a caller filesystem path.
use alcomd_logging::{LogQuery, LogSource, LoggingRuntime};
use alcomd_protocol as rpc;
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};
use tauri::{Manager, State};
use tracing_subscriber::prelude::*;
type AbortRead = Box<dyn Fn() + Send>;
pub(crate) struct LocalLogState {
    pub source: LogSource,
    runtime: Mutex<Option<LoggingRuntime>>,
    pending: Mutex<Option<(u64, AbortRead)>>,
    sequence: AtomicU64,
}
fn lock<T>(v: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    v.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}
pub(crate) fn initialize(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    // A failed path resolution is represented by a degraded source, without a fallback into arbitrary storage.
    let directory = app
        .path()
        .app_log_dir()
        .map(|p| p.join("gui"))
        .unwrap_or_default();
    let runtime = LoggingRuntime::start(directory, "gui");
    let source = runtime.source();
    let filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        tracing_subscriber::EnvFilter::new("warn,alcomd_gui_lib=info,alcomd_gui=info")
    });
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .json()
            .with_current_span(false)
            .with_span_list(true)
            .with_writer(source.writer())
            .with_filter(filter),
    );
    subscriber.try_init()?;
    tracing::info!(
        log_source = "gui",
        session_id = source.session_id(),
        "GUI logging initialized"
    );
    app.manage(LocalLogState {
        source,
        runtime: Mutex::new(Some(runtime)),
        pending: Mutex::new(None),
        sequence: AtomicU64::new(0),
    });
    Ok(())
}
pub(crate) fn shutdown(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<LocalLogState>() {
        if let Some((_, abort)) = lock(&state.pending).take() {
            abort();
        }
        if let Some(runtime) = lock(&state.runtime).take() {
            let done = std::sync::mpsc::sync_channel(1);
            let _ = std::thread::Builder::new()
                .name("gui-log-shutdown".into())
                .spawn(move || {
                    let _ = done.0.send(runtime.shutdown());
                });
            let _ = done.1.recv_timeout(std::time::Duration::from_secs(2));
        }
    }
}
pub(crate) fn cancel_reads(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<LocalLogState>()
        && let Some((_, abort)) = lock(&state.pending).take()
    {
        abort();
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Some(state) = app.try_state::<super::diagnostic_log::DiagnosticLogClient>() {
            state.cancel().await;
        }
    });
}
pub(crate) fn record_native_failure(
    error: impl std::fmt::Display,
    message: &'static str,
) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    tracing::error!(target:"alcomd_gui::system",diagnostic_id=%id,error=%error,"{message}");
    id
}
pub(crate) fn record_native_error(error: &dyn std::error::Error, message: &'static str) -> String {
    let mut reasons = Vec::new();
    let mut current = Some(error);
    while let Some(reason) = current {
        if reasons.len() == 32 {
            reasons.push("[error chain limit reached]".into());
            break;
        }
        let text = reason.to_string();
        if reasons.last() != Some(&text) {
            reasons.push(text);
        }
        current = reason.source();
    }
    record_native_failure(reasons.join("\ncaused by: "), message)
}
fn query(q: rpc::DiagnosticLogQuery) -> LogQuery {
    LogQuery {
        search: q.search,
        levels: q.levels,
        target: q.target,
        diagnostic_id: q.diagnostic_id,
    }
}
fn convert<T: serde::de::DeserializeOwned>(v: impl serde::Serialize) -> Result<T, rpc::RpcError> {
    serde_json::to_value(v)
        .ok()
        .and_then(|v| serde_json::from_value(v).ok())
        .ok_or_else(super::daemon_unavailable)
}
fn failure(e: alcomd_logging::LogError) -> rpc::RpcError {
    let (code, message) = match e {
        alcomd_logging::LogError::InvalidInput => {
            ("invalid_params", "The log parameters are invalid.")
        }
        alcomd_logging::LogError::CursorExpired => (
            "log_cursor_expired",
            "The requested log position is no longer retained.",
        ),
        alcomd_logging::LogError::Unavailable => (
            "log_unavailable",
            "The GUI technical logs could not be read.",
        ),
    };
    rpc::RpcError {
        code: code.into(),
        message: message.into(),
        diagnostic_id: None,
        data: None,
    }
}
#[tauri::command]
pub(crate) async fn gui_local_log_read(
    state: State<'_, LocalLogState>,
    params: rpc::DiagnosticLogReadParams,
) -> Result<rpc::DiagnosticLogReadResult, rpc::RpcError> {
    let (id, task) = {
        let mut pending = lock(&state.pending);
        if pending.is_some() {
            return Err(failure(alcomd_logging::LogError::InvalidInput));
        }
        let id = state.sequence.fetch_add(1, Ordering::Relaxed);
        let source = state.source.clone();
        let task = tauri::async_runtime::spawn(async move {
            source
                .read(
                    params.cursor,
                    params.limit.unwrap_or(100),
                    params.follow,
                    query(params.query),
                )
                .await
        });
        let abort = task.inner().abort_handle();
        *pending = Some((id, Box::new(move || abort.abort())));
        (id, task)
    };
    let result = task
        .await
        .map_err(|_| failure(alcomd_logging::LogError::Unavailable));
    let mut pending = lock(&state.pending);
    if pending.as_ref().is_some_and(|(current, _)| *current == id) {
        *pending = None;
    }
    convert(result?.map_err(failure)?)
}
#[tauri::command]
pub(crate) async fn gui_local_log_history(
    state: State<'_, LocalLogState>,
    params: rpc::DiagnosticLogHistoryParams,
) -> Result<rpc::DiagnosticLogHistoryResult, rpc::RpcError> {
    convert(
        state
            .source
            .history(
                params.cursor,
                params.before,
                params.limit.unwrap_or(100),
                query(params.query),
            )
            .await
            .map_err(failure)?,
    )
}
#[tauri::command]
pub(crate) fn gui_local_log_health(
    state: State<'_, LocalLogState>,
) -> Result<rpc::DiagnosticLogHealth, rpc::RpcError> {
    convert(state.source.health())
}
#[tauri::command]
pub(crate) fn gui_local_log_cancel(state: State<'_, LocalLogState>) {
    if let Some((_, abort)) = lock(&state.pending).take() {
        abort();
    }
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FrontendException {
    name: String,
    message: String,
    stack: Option<String>,
    location: Option<String>,
}
fn selected_text(text: &str) -> String {
    // URL credentials and query strings are excluded at this frontend exception boundary.
    text.split_inclusive(char::is_whitespace)
        .map(|part| {
            let content = part.trim_end_matches(char::is_whitespace);
            let whitespace = &part[content.len()..];
            let part = content;
            if let Some(at) = part.find("://") {
                let (prefix, tail) = part.split_at(at + 3);
                let tail = tail.split(['?', '#']).next().unwrap_or("");
                let slash = tail.find('/').unwrap_or(tail.len());
                let authority = tail[..slash].rsplit('@').next().unwrap_or("");
                format!("{prefix}{authority}{}{whitespace}", &tail[slash..])
            } else {
                format!("{part}{whitespace}")
            }
        })
        .collect::<String>()
}
#[tauri::command]
pub(crate) fn gui_report_frontend_error(
    _state: State<'_, LocalLogState>,
    error: FrontendException,
) -> Result<(), rpc::RpcError> {
    record_frontend_exception(error)
}
fn record_frontend_exception(error: FrontendException) -> Result<(), rpc::RpcError> {
    if error.name.is_empty()
        || error.name.len() > 128
        || error.message.len() > 4096
        || error.stack.as_ref().is_some_and(|s| s.len() > 16384)
        || error.location.as_ref().is_some_and(|s| s.len() > 2048)
        || matches!(error.name.as_str(), "RpcError" | "CoreRpcError")
    {
        return Err(failure(alcomd_logging::LogError::InvalidInput));
    }
    let all = format!(
        "{} {} {} {}",
        error.name,
        error.message,
        error.stack.as_deref().unwrap_or(""),
        error.location.as_deref().unwrap_or("")
    );
    let lower = all.to_ascii_lowercase();
    if [
        "authorization:",
        "bearer ",
        "password=",
        "token=",
        "secret=",
        "api_key=",
    ]
    .iter()
    .any(|s| lower.contains(s))
    {
        tracing::error!(target:"alcomd_gui::frontend","frontend exception details withheld because they contain credential-like data");
    } else {
        tracing::error!(target:"alcomd_gui::frontend",name=%error.name,error=%selected_text(&error.message),stack=%selected_text(error.stack.as_deref().unwrap_or("")),location=%selected_text(error.location.as_deref().unwrap_or("")),"unhandled frontend exception");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_transport_retries_keep_causes_and_remote_business_errors_are_not_logged_again() {
        tauri::async_runtime::block_on(async {
            let parent = std::env::temp_dir()
                .canonicalize()
                .expect("temporary parent");
            let directory =
                parent.join(format!("alcomd-gui-transport-log-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&directory).expect("isolated GUI source");
            let runtime = LoggingRuntime::start(&directory, "gui");
            let source = runtime.source();
            let subscriber = tracing_subscriber::registry().with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_writer(source.writer()),
            );
            let errors = tracing::subscriber::with_default(subscriber, || {
                let attempt = || {
                    super::super::client_error(alcomd_client::ClientError::Transport(
                        std::io::Error::new(
                            std::io::ErrorKind::PermissionDenied,
                            "endpoint access denied: C:\\用户\\ALCOMD",
                        ),
                    ))
                };
                let first = attempt();
                let second = attempt();
                let remote = rpc::RpcError::extension("extension_not_found");
                assert_eq!(
                    super::super::client_error(alcomd_client::ClientError::Remote(remote.clone())),
                    remote
                );
                (first, second)
            });
            assert_ne!(errors.0.diagnostic_id, errors.1.diagnostic_id);
            assert!(runtime.shutdown().drained);
            let page = source
                .history(None, None, 100, LogQuery::default())
                .await
                .expect("GUI file records");
            assert_eq!(page.items.len(), 2);
            assert!(page.items.iter().all(|item| {
                item.record["fields"]["error"]
                    .as_str()
                    .expect("explicit cause chain")
                    .contains("C:\\用户\\ALCOMD")
            }));
            assert!(
                directory
                    .canonicalize()
                    .expect("owned directory")
                    .starts_with(&parent)
            );
            drop(source);
            std::fs::remove_dir_all(directory).expect("remove isolated GUI logs");
        });
    }
    #[test]
    fn selected_frontend_fields_preserve_whitespace_paths_and_strip_url_credentials() {
        let input = "at C:\\项目 A\\app.ts:1\nfetch https://alice:password@example.invalid/api?token=secret#part\nnext";
        let out = selected_text(input);
        assert!(out.contains("C:\\项目 A\\app.ts:1\n"));
        assert!(out.contains("https://example.invalid/api\nnext"));
        assert!(!out.contains("password"));
        assert!(!out.contains("token="));
    }
    #[test]
    fn frontend_error_input_is_closed_and_rejects_business_errors_and_large_payloads() {
        assert!(
            serde_json::from_value::<FrontendException>(
                serde_json::json!({"name":"Error","message":"x","response":{"private":true}})
            )
            .is_err()
        );
        assert!(
            record_frontend_exception(FrontendException {
                name: "RpcError".into(),
                message: "Core already owns this failure".into(),
                stack: None,
                location: None
            })
            .is_err()
        );
        assert!(
            record_frontend_exception(FrontendException {
                name: "Error".into(),
                message: "x".repeat(4097),
                stack: None,
                location: None
            })
            .is_err()
        );
    }
    #[test]
    fn real_renderer_fault_is_written_once_and_readable_without_core() {
        let parent = std::env::temp_dir()
            .canonicalize()
            .expect("valid isolated GUI log fixture");
        let dir = parent.join(format!(
            "alcomd-native-log-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("valid isolated GUI log fixture")
                .as_nanos()
        ));
        std::fs::create_dir(&dir).expect("valid isolated GUI log fixture");
        let runtime = LoggingRuntime::start(&dir, "gui");
        let source = runtime.source();
        let subscriber = tracing_subscriber::registry().with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_writer(source.writer()),
        );
        tracing::subscriber::with_default(subscriber, || {
            record_frontend_exception(FrontendException {
                name: "TypeError".into(),
                message: "unexpected renderer failure".into(),
                stack: Some("at C:\\项目 A\\app.ts:1".into()),
                location: None,
            })
            .expect("valid isolated GUI log fixture")
        });
        let report = runtime.shutdown();
        assert!(report.drained && report.flushed);
        let page =
            tauri::async_runtime::block_on(source.history(None, None, 100, LogQuery::default()))
                .expect("valid isolated GUI log fixture");
        assert_eq!(page.items.len(), 1);
        assert_eq!(
            page.items[0].record["fields"]["message"],
            "unhandled frontend exception"
        );
        assert_eq!(
            page.items[0].record["fields"]["error"],
            "unexpected renderer failure"
        );
        assert!(
            page.items[0].record["fields"]["stack"]
                .as_str()
                .expect("valid isolated GUI log fixture")
                .contains("项目 A")
        );
        assert!(
            dir.canonicalize()
                .expect("valid isolated GUI log fixture")
                .starts_with(&parent)
        );
        drop(source);
        std::fs::remove_dir_all(dir).expect("valid isolated GUI log fixture");
    }
}
