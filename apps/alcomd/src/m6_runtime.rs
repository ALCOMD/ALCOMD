use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use alcomd_application::{
    ExtensionInstanceLease, ExtensionProjectSummary, ExtensionRuntimePoll, ExtensionStartContext,
    ExtensionStopReason, M6Error, M6ErrorCode, M6HostApplication, M6RuntimeAdapter, OperationId,
    ProjectId, Revision,
};
use alcomd_extensions::{
    HOST_PROTOCOL_VERSION, HostMessage, HostMessageBody, HostStableError, RuntimeLimits,
    bootstrap_nonce, read_host_message, write_host_message,
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
        tokio::task::spawn_blocking(move || start_host(&inner, context))
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

fn start_host(inner: &RuntimeInner, context: ExtensionStartContext) -> Result<(), M6Error> {
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
    };
    let activation = json!({
        "extensionId": process.lease.extension_id,
        "instanceId": process.lease.instance_id,
        "apiMajor": 1,
        "lifecycleGeneration": process.lease.lifecycle_generation.get()
    });
    if let Err(error) = invoke(
        inner,
        &mut process,
        "activate",
        activation,
        Duration::from_millis(5_000),
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
        ExtensionStopReason::DaemonShutdown => "daemon_shutdown",
        ExtensionStopReason::Uninstalling => "uninstalling",
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
) -> Result<(), M6Error> {
    process.daemon_sequence = process
        .daemon_sequence
        .checked_add(1)
        .ok_or_else(internal)?;
    let request_id = format!("export-{}", process.daemon_sequence);
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
                lease_id,
                capability,
                input,
            } => {
                if lease_id != process.lease.lease_id {
                    terminate(&mut process.child);
                    return Err(error(M6ErrorCode::InstanceStale));
                }
                let response = route_capability(
                    &inner.authority,
                    process.lease.clone(),
                    capability.as_str(),
                    input,
                );
                process.daemon_sequence = process
                    .daemon_sequence
                    .checked_add(1)
                    .ok_or_else(internal)?;
                let (result, response_error) = match response {
                    Ok(result) => (Some(result), None),
                    Err(error) => (None, Some(stable_error(error))),
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
                result: Some(_),
                error: None,
            } if response_id == request_id => return Ok(()),
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
    capability: &str,
    input: Value,
) -> Result<Value, M6Error> {
    let runtime = tokio::runtime::Handle::current();
    match capability {
        "host-projects.get-summary" => {
            let project_id = input
                .get("projectId")
                .and_then(Value::as_str)
                .ok_or_else(|| error(M6ErrorCode::InvalidInput))?;
            ProjectId::parse(project_id).map_err(|_| error(M6ErrorCode::InvalidInput))?;
            let value = runtime.block_on(authority.project_summary(
                lease,
                project_id.to_owned(),
                now_ms()?,
            ))?;
            Ok(json!({"summary": project_summary(value)}))
        }
        "host-data.get" => {
            let key = input_string(&input, "key")?;
            let value = runtime.block_on(authority.data_get(lease, key, now_ms()?))?;
            Ok(json!({"value": value.map(|value| json!({
                "bytes": value.value,
                "keyRevision": value.key_revision.get(),
                "namespaceRevision": value.namespace_revision.get()
            }))}))
        }
        "host-data.set" => {
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
