use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use alcomd_application::{
    ExtensionActivationKind, ExtensionInstanceLease, ExtensionInvocationKind,
    ExtensionProjectSummary, ExtensionRuntimePoll, ExtensionStartContext, ExtensionStopReason,
    ExtensionUiExport, ExtensionUiInvocationAuthority, M6Error, M6ErrorCode, M6HostApplication,
    M6RuntimeAdapter, M7RuntimeAdapter, OperationId, ProjectId, Revision,
};
use alcomd_extensions::{
    HOST_PROTOCOL_VERSION, HostMessage, HostMessageBody, HostStableError, RuntimeLimits,
    bootstrap_nonce, invocation_context_id, read_host_message, write_host_message,
};
use alcomd_store::StateStoreHandle;
use serde_json::{Value, json};

#[derive(Clone)]
pub(super) struct PlatformExtensionRuntime {
    inner: Arc<RuntimeInner>,
}

struct RuntimeInner {
    authority: M6HostApplication<StateStoreHandle>,
    daemon_epoch: String,
    host_executable: PathBuf,
    processes: Mutex<HashMap<String, HostProcess>>,
}

impl Drop for RuntimeInner {
    fn drop(&mut self) {
        if let Ok(processes) = self.processes.get_mut() {
            for process in processes.values_mut() {
                terminate(&mut process.child);
            }
            processes.clear();
        }
    }
}

struct HostProcess {
    child: Child,
    input: ChildStdin,
    output: Option<ChildStdout>,
    lease: ExtensionInstanceLease,
    daemon_sequence: u64,
    host_sequence: u64,
    background_active: bool,
}

struct RuntimeInvocation {
    kind: ExtensionInvocationKind,
    ui_authority: Option<ExtensionUiInvocationAuthority>,
}

impl PlatformExtensionRuntime {
    pub(super) fn new(store: StateStoreHandle) -> Result<Self, M6Error> {
        let executable = std::env::current_exe().map_err(|_| internal())?;
        let default_host = executable.with_file_name(format!(
            "alcomd-extension-host{}",
            std::env::consts::EXE_SUFFIX
        ));
        #[cfg(feature = "test-kill-gates")]
        let host_executable = std::env::var_os("ALCOMD_TEST_EXTENSION_HOST")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                if default_host.is_file() {
                    return default_host;
                }
                executable
                    .parent()
                    .and_then(Path::parent)
                    .map(|parent| {
                        parent.join(format!(
                            "alcomd-extension-host{}",
                            std::env::consts::EXE_SUFFIX
                        ))
                    })
                    .unwrap_or(default_host)
            });
        #[cfg(not(feature = "test-kill-gates"))]
        let host_executable = default_host;
        if !host_executable.is_absolute() {
            return Err(internal());
        }
        Ok(Self {
            inner: Arc::new(RuntimeInner {
                authority: M6HostApplication::new(store),
                daemon_epoch: OperationId::new().to_string(),
                host_executable,
                processes: Mutex::new(HashMap::new()),
            }),
        })
    }
}

impl M6RuntimeAdapter for PlatformExtensionRuntime {
    fn daemon_epoch(&self) -> String {
        self.inner.daemon_epoch.clone()
    }

    async fn start(&self, context: ExtensionStartContext) -> Result<(), M6Error> {
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || start_host(&inner, context, None))
            .await
            .map_err(|_| internal())?
    }

    async fn stop(&self, extension_id: String, reason: ExtensionStopReason) -> Result<(), M6Error> {
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || stop_host(&inner, &extension_id, reason))
            .await
            .map_err(|_| internal())?
    }

    async fn poll(&self) -> Result<ExtensionRuntimePoll, M6Error> {
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || poll_hosts(&inner))
            .await
            .map_err(|_| internal())?
    }

    async fn update_lease(&self, lease: ExtensionInstanceLease) -> Result<(), M6Error> {
        let mut processes = self.inner.processes.lock().map_err(|_| internal())?;
        let process = processes
            .get_mut(&lease.extension_id)
            .filter(|process| process.lease.instance_id == lease.instance_id)
            .ok_or_else(|| error(M6ErrorCode::InstanceStale))?;
        process.lease = lease;
        Ok(())
    }

    async fn shutdown(&self) -> Result<(), M6Error> {
        let extension_ids = self
            .inner
            .processes
            .lock()
            .map_err(|_| internal())?
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for extension_id in extension_ids {
            self.stop(extension_id, ExtensionStopReason::DaemonShutdown)
                .await?;
        }
        Ok(())
    }
}

impl M7RuntimeAdapter for PlatformExtensionRuntime {
    async fn start_interactive(
        &self,
        context: ExtensionStartContext,
        authority: ExtensionUiInvocationAuthority,
    ) -> Result<(), M6Error> {
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || start_host(&inner, context, Some(authority)))
            .await
            .map_err(|_| internal())?
    }

    async fn active_lease(
        &self,
        extension_id: String,
    ) -> Result<Option<ExtensionInstanceLease>, M6Error> {
        Ok(self
            .inner
            .processes
            .lock()
            .map_err(|_| internal())?
            .get(&extension_id)
            .map(|process| process.lease.clone()))
    }

    async fn background_active(&self, extension_id: String) -> Result<bool, M6Error> {
        Ok(self
            .inner
            .processes
            .lock()
            .map_err(|_| internal())?
            .get(&extension_id)
            .is_some_and(|process| process.background_active))
    }

    async fn invoke_ui(
        &self,
        lease: ExtensionInstanceLease,
        authority: ExtensionUiInvocationAuthority,
        export: ExtensionUiExport,
    ) -> Result<Option<Vec<u8>>, M6Error> {
        let remaining_ms = authority.deadline_ms.saturating_sub(now_ms()?);
        if remaining_ms == 0 {
            return Err(error(M6ErrorCode::ResourceLimit));
        }
        let timeout = Duration::from_millis(remaining_ms.min(2_000));
        let inner = Arc::clone(&self.inner);
        tokio::task::spawn_blocking(move || {
            let mut processes = inner.processes.lock().map_err(|_| internal())?;
            let process = processes
                .get_mut(&lease.extension_id)
                .filter(|process| process.lease == lease)
                .ok_or_else(|| error(M6ErrorCode::InstanceStale))?;
            let (name, input, expects_document) = match export {
                ExtensionUiExport::Open { session_id, locale } => (
                    "ui.open",
                    json!({"sessionId": session_id, "locale": locale}),
                    true,
                ),
                ExtensionUiExport::Refresh { session_id } => {
                    ("ui.refresh", json!({"sessionId": session_id}), true)
                }
                ExtensionUiExport::Dispatch { session_id, action } => {
                    let action = serde_json::from_slice::<Value>(&action)
                        .map_err(|_| error(M6ErrorCode::InvalidInput))?;
                    (
                        "ui.dispatch",
                        json!({"sessionId": session_id, "action": action}),
                        true,
                    )
                }
                ExtensionUiExport::Close { session_id } => {
                    ("ui.close", json!({"sessionId": session_id}), false)
                }
            };
            let result = invoke(
                &inner,
                process,
                name,
                input,
                timeout,
                RuntimeInvocation {
                    kind: authority.kind,
                    ui_authority: Some(authority),
                },
            )?;
            if expects_document {
                serde_json::to_vec(&result)
                    .map(Some)
                    .map_err(|_| internal())
            } else {
                Ok(None)
            }
        })
        .await
        .map_err(|_| internal())?
    }

    async fn terminate_host(
        &self,
        extension_id: String,
    ) -> Result<Option<ExtensionInstanceLease>, M6Error> {
        let process = self
            .inner
            .processes
            .lock()
            .map_err(|_| internal())?
            .remove(&extension_id);
        Ok(process.map(|mut process| {
            terminate(&mut process.child);
            process.lease
        }))
    }
}

fn poll_hosts(inner: &RuntimeInner) -> Result<ExtensionRuntimePoll, M6Error> {
    let mut processes = inner.processes.lock().map_err(|_| internal())?;
    let exited_ids = processes
        .iter_mut()
        .filter_map(|(extension_id, process)| match process.child.try_wait() {
            Ok(Some(_)) | Err(_) => Some(extension_id.clone()),
            Ok(None) => None,
        })
        .collect::<Vec<_>>();
    let exited = exited_ids
        .iter()
        .filter_map(|extension_id| processes.remove(extension_id))
        .map(|process| process.lease)
        .collect();
    let active = processes
        .values()
        .map(|process| process.lease.clone())
        .collect();
    Ok(ExtensionRuntimePoll { active, exited })
}

fn start_host(
    inner: &RuntimeInner,
    context: ExtensionStartContext,
    activation_authority: Option<ExtensionUiInvocationAuthority>,
) -> Result<(), M6Error> {
    let component = PathBuf::from(&context.component_path);
    validate_component(&component)?;
    let mut processes = inner.processes.lock().map_err(|_| internal())?;
    if let Some(mut existing) = processes.remove(&context.lease.extension_id) {
        terminate(&mut existing.child);
    }
    let mut child = Command::new(&inner.host_executable)
        .arg("--extension")
        .arg(&context.lease.extension_id)
        .arg("--component")
        .arg(&component)
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| error(M6ErrorCode::Crashed))?;
    let mut input = child.stdin.take().ok_or_else(internal)?;
    let output = child.stdout.take().ok_or_else(internal)?;
    let nonce = bootstrap_nonce();
    let bootstrap = HostMessage {
        protocol_version: HOST_PROTOCOL_VERSION,
        daemon_epoch: context.lease.daemon_epoch.clone(),
        instance_id: context.lease.instance_id.clone(),
        lifecycle_generation: context.lease.lifecycle_generation.get(),
        sequence: 1,
        body: HostMessageBody::Bootstrap {
            nonce: nonce.clone(),
            lease_id: context.lease.lease_id.clone(),
            extension_id: context.lease.extension_id.clone(),
            api_world: "alcomd:extension/extension-v1@1.0.0".to_owned(),
            limits: RuntimeLimits::default(),
        },
    };
    if write_host_message(&mut input, &bootstrap).is_err() {
        terminate(&mut child);
        return Err(error(M6ErrorCode::Crashed));
    }
    let (ready, output) = read_with_timeout(output, Duration::from_millis(5_000), &mut child)?;
    if !bound(&ready, &context.lease, 1)
        || !matches!(ready.body, HostMessageBody::Ready { nonce: value } if value == nonce)
    {
        terminate(&mut child);
        return Err(error(M6ErrorCode::Crashed));
    }
    let mut process = HostProcess {
        child,
        input,
        output: Some(output),
        lease: context.lease,
        daemon_sequence: 1,
        host_sequence: 1,
        background_active: context.activation_kind == ExtensionActivationKind::Background,
    };
    let activation_kind = match context.activation_kind {
        ExtensionActivationKind::Background => "background",
        ExtensionActivationKind::InteractiveUi => "interactive_ui",
    };
    let activation = json!({
        "extensionId": process.lease.extension_id,
        "instanceId": process.lease.instance_id,
        "apiMajor": 1,
        "lifecycleGeneration": process.lease.lifecycle_generation.get(),
        "kind": activation_kind
    });
    if let Err(error) = invoke(
        inner,
        &mut process,
        "activate",
        activation,
        Duration::from_millis(5_000),
        RuntimeInvocation {
            kind: match context.activation_kind {
                ExtensionActivationKind::Background => ExtensionInvocationKind::Background,
                ExtensionActivationKind::InteractiveUi => {
                    ExtensionInvocationKind::InteractiveUiRender
                }
            },
            ui_authority: activation_authority,
        },
    ) {
        terminate(&mut process.child);
        return Err(error);
    }
    processes.insert(process.lease.extension_id.clone(), process);
    Ok(())
}

fn stop_host(
    inner: &RuntimeInner,
    extension_id: &str,
    reason: ExtensionStopReason,
) -> Result<(), M6Error> {
    let mut process = {
        let mut processes = inner.processes.lock().map_err(|_| internal())?;
        processes.remove(extension_id)
    };
    let Some(mut process) = process.take() else {
        return Ok(());
    };
    let reason = match reason {
        ExtensionStopReason::Disabled => "disabled",
        ExtensionStopReason::PermissionRevoked => "permission_revoked",
        ExtensionStopReason::LeaseExpired => "lease_expired",
        ExtensionStopReason::DaemonShutdown => "daemon_shutdown",
        ExtensionStopReason::Uninstalling => "uninstalling",
        ExtensionStopReason::InteractiveUiIdle => "interactive_ui_idle",
    };
    process.daemon_sequence = process
        .daemon_sequence
        .checked_add(1)
        .ok_or_else(internal)?;
    let _ = write_host_message(
        &mut process.input,
        &HostMessage {
            protocol_version: HOST_PROTOCOL_VERSION,
            daemon_epoch: process.lease.daemon_epoch.clone(),
            instance_id: process.lease.instance_id.clone(),
            lifecycle_generation: process.lease.lifecycle_generation.get(),
            sequence: process.daemon_sequence,
            body: HostMessageBody::RevokeLease {
                lease_id: process.lease.lease_id.clone(),
            },
        },
    );
    let _ = invoke(
        inner,
        &mut process,
        "deactivate",
        json!({"reason": reason}),
        Duration::from_millis(2_000),
        RuntimeInvocation {
            kind: if reason == "interactive_ui_idle" {
                ExtensionInvocationKind::InteractiveUiClose
            } else {
                ExtensionInvocationKind::Background
            },
            ui_authority: None,
        },
    );
    process.daemon_sequence = process
        .daemon_sequence
        .checked_add(1)
        .ok_or_else(internal)?;
    let _ = write_host_message(
        &mut process.input,
        &HostMessage {
            protocol_version: HOST_PROTOCOL_VERSION,
            daemon_epoch: process.lease.daemon_epoch.clone(),
            instance_id: process.lease.instance_id.clone(),
            lifecycle_generation: process.lease.lifecycle_generation.get(),
            sequence: process.daemon_sequence,
            body: HostMessageBody::Shutdown,
        },
    );
    wait_or_kill(&mut process.child, Duration::from_millis(2_000));
    Ok(())
}

fn invoke(
    inner: &RuntimeInner,
    process: &mut HostProcess,
    export: &str,
    input: Value,
    timeout: Duration,
    invocation: RuntimeInvocation,
) -> Result<Value, M6Error> {
    process.daemon_sequence = process
        .daemon_sequence
        .checked_add(1)
        .ok_or_else(internal)?;
    let request_id = format!("export-{}", process.daemon_sequence);
    let context_id = invocation_context_id();
    write_host_message(
        &mut process.input,
        &HostMessage {
            protocol_version: HOST_PROTOCOL_VERSION,
            daemon_epoch: process.lease.daemon_epoch.clone(),
            instance_id: process.lease.instance_id.clone(),
            lifecycle_generation: process.lease.lifecycle_generation.get(),
            sequence: process.daemon_sequence,
            body: HostMessageBody::InvokeExport {
                request_id: request_id.clone(),
                invocation_context_id: context_id.clone(),
                export: export.to_owned(),
                input,
            },
        },
    )
    .map_err(|_| error(M6ErrorCode::Crashed))?;
    let deadline = Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            terminate(&mut process.child);
            return Err(error(M6ErrorCode::ResourceLimit));
        }
        let output = process.output.take().ok_or_else(internal)?;
        let (message, returned) = read_with_timeout(output, remaining, &mut process.child)?;
        process.output = Some(returned);
        if !bound(&message, &process.lease, process.host_sequence + 1) {
            terminate(&mut process.child);
            return Err(error(M6ErrorCode::Crashed));
        }
        process.host_sequence = message.sequence;
        match message.body {
            HostMessageBody::CapabilityCall {
                call_id,
                invocation_context_id: returned_context_id,
                lease_id,
                capability,
                input,
            } => {
                if returned_context_id != context_id || lease_id != process.lease.lease_id {
                    terminate(&mut process.child);
                    return Err(error(M6ErrorCode::Crashed));
                }
                process.daemon_sequence = process
                    .daemon_sequence
                    .checked_add(1)
                    .ok_or_else(internal)?;
                let (result, response_error) = if invocation_cancelled(&invocation) {
                    (None, Some(cancelled_error()))
                } else {
                    match route_capability(
                        &inner.authority,
                        process.lease.clone(),
                        &invocation,
                        capability.as_str(),
                        input,
                    ) {
                        Ok(result) => (Some(result), None),
                        Err(error) => (None, Some(stable_error(error))),
                    }
                };
                write_host_message(
                    &mut process.input,
                    &HostMessage {
                        protocol_version: HOST_PROTOCOL_VERSION,
                        daemon_epoch: process.lease.daemon_epoch.clone(),
                        instance_id: process.lease.instance_id.clone(),
                        lifecycle_generation: process.lease.lifecycle_generation.get(),
                        sequence: process.daemon_sequence,
                        body: HostMessageBody::CapabilityResult {
                            call_id,
                            result,
                            error: response_error,
                        },
                    },
                )
                .map_err(|_| error(M6ErrorCode::Crashed))?;
            }
            HostMessageBody::ExportResult {
                request_id: response_id,
                result: Some(result),
                error: None,
            } if response_id == request_id => return Ok(result),
            HostMessageBody::ExportResult {
                request_id: response_id,
                result: None,
                error: Some(error),
            } if response_id == request_id => return Err(host_error(error)),
            _ => {
                terminate(&mut process.child);
                return Err(error(M6ErrorCode::Crashed));
            }
        }
    }
}

fn route_capability(
    authority: &M6HostApplication<StateStoreHandle>,
    lease: ExtensionInstanceLease,
    invocation: &RuntimeInvocation,
    capability: &str,
    input: Value,
) -> Result<Value, M6Error> {
    if !capability_allowed(invocation.kind, capability) {
        return Err(error(M6ErrorCode::PermissionDenied));
    }
    let runtime = tokio::runtime::Handle::current();
    match capability {
        "host-projects.get-summary" => {
            let project_id = input
                .get("projectId")
                .and_then(Value::as_str)
                .ok_or_else(|| error(M6ErrorCode::InvalidInput))?;
            ProjectId::parse(project_id).map_err(|_| error(M6ErrorCode::InvalidInput))?;
            if invocation.kind != ExtensionInvocationKind::Background {
                invocation
                    .ui_authority
                    .as_ref()
                    .ok_or_else(|| error(M6ErrorCode::ScopeDenied))?
                    .client_access
                    .require_project_read_scope(project_id)
                    .map_err(|_| error(M6ErrorCode::ScopeDenied))?;
            }
            let value = runtime.block_on(authority.project_summary(
                lease,
                project_id.to_owned(),
                now_ms()?,
            ))?;
            Ok(json!({"summary": project_summary(value)}))
        }
        "host-data.get" => {
            require_ui_data_authority(invocation, &lease)?;
            let key = input_string(&input, "key")?;
            let value = runtime.block_on(authority.data_get(lease, key, now_ms()?))?;
            Ok(json!({"value": value.map(|value| json!({
                "bytes": value.value,
                "keyRevision": value.key_revision.get(),
                "namespaceRevision": value.namespace_revision.get()
            }))}))
        }
        "host-data.set" => {
            require_ui_data_authority(invocation, &lease)?;
            let key = input_string(&input, "key")?;
            let value = input_bytes(&input, "value")?;
            let expected = input
                .get("expectedKeyRevision")
                .and_then(Value::as_u64)
                .map(|value| Revision::new(value).ok_or_else(|| error(M6ErrorCode::InvalidInput)))
                .transpose()?;
            let result =
                runtime.block_on(authority.data_set(lease, key, value, expected, now_ms()?))?;
            Ok(json!({
                "keyRevision": result.key_revision.get(),
                "namespaceRevision": result.namespace_revision.get()
            }))
        }
        "host-data.delete" => {
            require_ui_data_authority(invocation, &lease)?;
            let key = input_string(&input, "key")?;
            let expected = input
                .get("expectedKeyRevision")
                .and_then(Value::as_u64)
                .and_then(Revision::new)
                .ok_or_else(|| error(M6ErrorCode::InvalidInput))?;
            let result =
                runtime.block_on(authority.data_delete(lease, key, expected, now_ms()?))?;
            Ok(json!({
                "keyRevision": result.key_revision.get(),
                "namespaceRevision": result.namespace_revision.get()
            }))
        }
        _ => Err(error(M6ErrorCode::PermissionDenied)),
    }
}

fn capability_allowed(kind: ExtensionInvocationKind, capability: &str) -> bool {
    match kind {
        ExtensionInvocationKind::Background => matches!(
            capability,
            "host-projects.get-summary" | "host-data.get" | "host-data.set" | "host-data.delete"
        ),
        ExtensionInvocationKind::InteractiveUiRender => {
            matches!(capability, "host-projects.get-summary" | "host-data.get")
        }
        ExtensionInvocationKind::InteractiveUiAction => matches!(
            capability,
            "host-projects.get-summary" | "host-data.get" | "host-data.set" | "host-data.delete"
        ),
        ExtensionInvocationKind::InteractiveUiClose => false,
    }
}

fn invocation_cancelled(invocation: &RuntimeInvocation) -> bool {
    invocation.ui_authority.as_ref().is_some_and(|authority| {
        !authority.is_current() || now_ms().map_or(true, |now_ms| now_ms >= authority.deadline_ms)
    })
}

fn require_ui_data_authority(
    invocation: &RuntimeInvocation,
    lease: &ExtensionInstanceLease,
) -> Result<(), M6Error> {
    if invocation.kind != ExtensionInvocationKind::Background {
        invocation
            .ui_authority
            .as_ref()
            .ok_or_else(|| error(M6ErrorCode::ScopeDenied))?
            .client_access
            .require_extension_ui_scope(&lease.extension_id)
            .map_err(|_| error(M6ErrorCode::ScopeDenied))?;
    }
    Ok(())
}

fn read_with_timeout(
    mut output: ChildStdout,
    timeout: Duration,
    child: &mut Child,
) -> Result<(HostMessage, ChildStdout), M6Error> {
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let result = read_host_message(&mut output);
        let _ = sender.send((result, output));
    });
    match receiver.recv_timeout(timeout) {
        Ok((Ok(message), output)) => Ok((message, output)),
        Ok((Err(_), _)) => {
            terminate(child);
            Err(error(M6ErrorCode::Crashed))
        }
        Err(_) => {
            terminate(child);
            Err(error(M6ErrorCode::ResourceLimit))
        }
    }
}

fn bound(message: &HostMessage, lease: &ExtensionInstanceLease, sequence: u64) -> bool {
    message.protocol_version == HOST_PROTOCOL_VERSION
        && message.daemon_epoch == lease.daemon_epoch
        && message.instance_id == lease.instance_id
        && message.lifecycle_generation == lease.lifecycle_generation.get()
        && message.sequence == sequence
}

fn stable_error(error: M6Error) -> HostStableError {
    let code = match error.code() {
        M6ErrorCode::PermissionDenied => "extension_permission_denied",
        M6ErrorCode::ScopeDenied => "extension_scope_denied",
        M6ErrorCode::InstanceStale => "extension_instance_stale",
        M6ErrorCode::ResourceLimit | M6ErrorCode::InvalidInput => "extension_resource_limit",
        M6ErrorCode::ProjectNotFound | M6ErrorCode::NotInstalled => "project_not_found",
        M6ErrorCode::RevisionConflict => "revision_conflict",
        M6ErrorCode::DataQuotaExceeded => "extension_data_quota_exceeded",
        _ => "internal_error",
    };
    HostStableError {
        code: code.to_owned(),
        diagnostic_id: (code == "internal_error").then(|| OperationId::new().to_string()),
    }
}

fn cancelled_error() -> HostStableError {
    HostStableError {
        code: "cancelled".to_owned(),
        diagnostic_id: None,
    }
}

fn host_error(source: HostStableError) -> M6Error {
    error(match source.code.as_str() {
        "extension_permission_denied" => M6ErrorCode::PermissionDenied,
        "extension_scope_denied" => M6ErrorCode::ScopeDenied,
        "extension_instance_stale" => M6ErrorCode::InstanceStale,
        "extension_resource_limit" => M6ErrorCode::ResourceLimit,
        "project_not_found" => M6ErrorCode::ProjectNotFound,
        "revision_conflict" => M6ErrorCode::RevisionConflict,
        "extension_data_quota_exceeded" => M6ErrorCode::DataQuotaExceeded,
        _ => M6ErrorCode::Crashed,
    })
}

fn project_summary(value: ExtensionProjectSummary) -> Value {
    json!({
        "projectId": value.project_id,
        "displayName": value.display_name,
        "kind": value.kind,
        "unityVersion": value.unity_version,
        "revision": value.revision.get()
    })
}

fn input_string(input: &Value, field: &str) -> Result<String, M6Error> {
    input
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| error(M6ErrorCode::InvalidInput))
}

fn input_bytes(input: &Value, field: &str) -> Result<Vec<u8>, M6Error> {
    input
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| error(M6ErrorCode::InvalidInput))?
        .iter()
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| u8::try_from(value).ok())
                .ok_or_else(|| error(M6ErrorCode::InvalidInput))
        })
        .collect()
}

fn validate_component(path: &Path) -> Result<(), M6Error> {
    if !path.is_absolute()
        || path.file_name().and_then(|value| value.to_str()) != Some("extension.wasm")
    {
        return Err(error(M6ErrorCode::PackageInvalid));
    }
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| error(M6ErrorCode::PackageInvalid))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(error(M6ErrorCode::PackageInvalid));
    }
    Ok(())
}

fn wait_or_kill(child: &mut Child, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if child.try_wait().ok().flatten().is_some() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    terminate(child);
}

fn terminate(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn now_ms() -> Result<u64, M6Error> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| internal())?;
    u64::try_from(duration.as_millis()).map_err(|_| internal())
}

fn error(code: M6ErrorCode) -> M6Error {
    M6Error::new(code)
}

fn internal() -> M6Error {
    error(M6ErrorCode::Internal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_message_binding_rejects_forged_or_stale_authority() {
        let lease = ExtensionInstanceLease {
            lease_id: "00000000-0000-4000-8000-000000000001".to_owned(),
            extension_id: "dev.example.fixture".to_owned(),
            instance_id: "00000000-0000-4000-8000-000000000002".to_owned(),
            principal_id: alcomd_application::PrincipalId::parse("extension-instance:test")
                .expect("principal"),
            publisher_fingerprint: format!("ed25519-sha256:{}", "a".repeat(64)),
            grant_revision: Revision::INITIAL,
            lifecycle_generation: Revision::INITIAL,
            daemon_epoch: "00000000-0000-4000-8000-000000000003".to_owned(),
            expires_at_ms: 60_000,
        };
        let message = HostMessage {
            protocol_version: HOST_PROTOCOL_VERSION,
            daemon_epoch: lease.daemon_epoch.clone(),
            instance_id: lease.instance_id.clone(),
            lifecycle_generation: lease.lifecycle_generation.get(),
            sequence: 7,
            body: HostMessageBody::Ready {
                nonce: "bounded-nonce".to_owned(),
            },
        };
        assert!(bound(&message, &lease, 7));

        let mut wrong_protocol = message.clone();
        wrong_protocol.protocol_version += 1;
        let mut wrong_epoch = message.clone();
        wrong_epoch.daemon_epoch = "00000000-0000-4000-8000-000000000099".to_owned();
        let mut wrong_instance = message.clone();
        wrong_instance.instance_id = "00000000-0000-4000-8000-000000000098".to_owned();
        let mut wrong_generation = message.clone();
        wrong_generation.lifecycle_generation += 1;
        let mut replayed_sequence = message;
        replayed_sequence.sequence -= 1;

        for forged in [
            wrong_protocol,
            wrong_epoch,
            wrong_instance,
            wrong_generation,
            replayed_sequence,
        ] {
            assert!(!bound(&forged, &lease, 7));
        }
    }

    #[test]
    fn portable_ui_capability_matrix_is_explicit_and_closed() {
        for capability in [
            "host-projects.get-summary",
            "host-data.get",
            "host-data.set",
            "host-data.delete",
        ] {
            assert!(capability_allowed(
                ExtensionInvocationKind::Background,
                capability
            ));
        }
        assert!(capability_allowed(
            ExtensionInvocationKind::InteractiveUiRender,
            "host-projects.get-summary"
        ));
        assert!(capability_allowed(
            ExtensionInvocationKind::InteractiveUiRender,
            "host-data.get"
        ));
        assert!(!capability_allowed(
            ExtensionInvocationKind::InteractiveUiRender,
            "host-data.set"
        ));
        assert!(!capability_allowed(
            ExtensionInvocationKind::InteractiveUiRender,
            "host-data.delete"
        ));
        assert!(capability_allowed(
            ExtensionInvocationKind::InteractiveUiAction,
            "host-data.set"
        ));
        assert!(capability_allowed(
            ExtensionInvocationKind::InteractiveUiAction,
            "host-data.delete"
        ));
        assert!(!capability_allowed(
            ExtensionInvocationKind::InteractiveUiClose,
            "host-data.get"
        ));
        assert!(!capability_allowed(
            ExtensionInvocationKind::InteractiveUiAction,
            "future-capability"
        ));
    }
}
