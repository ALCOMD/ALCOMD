//! Local desktop window affordances. Project paths are resolved by Core.

use alcomd_protocol::RpcError;
use serde::{Deserialize, Serialize};
#[cfg(windows)]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tauri::State;

use super::GuiClientState;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Target {
    project_id: String,
    revision: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Snapshot {
    observing: bool,
    version: u32,
    states: Vec<(String, &'static str)>,
}

#[derive(Default)]
pub(crate) struct UnityWindows {
    #[cfg(windows)]
    active: std::sync::Mutex<Option<Active>>,
}

#[cfg(windows)]
struct Active {
    token: String,
    projects: Vec<String>,
    monitor: Option<alcomd_platform::UnityWindowMonitor>,
    waiting: Arc<AtomicBool>,
}

#[cfg(windows)]
struct WindowRead(Arc<AtomicBool>);

#[cfg(windows)]
impl Drop for WindowRead {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl UnityWindows {
    pub(crate) fn reset(&self) {
        #[cfg(windows)]
        {
            self.lock().take();
        }
    }

    pub(crate) fn refresh(&self) {
        #[cfg(windows)]
        if let Some(monitor) = self
            .lock()
            .as_ref()
            .and_then(|active| active.monitor.as_ref())
        {
            monitor.refresh();
        }
    }

    #[cfg(windows)]
    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Active>> {
        self.active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(windows)]
fn snapshot(projects: &[String], value: alcomd_platform::UnityWindowSnapshot) -> Snapshot {
    use alcomd_platform::UnityWindowState;
    Snapshot {
        observing: true,
        version: value.version,
        states: projects
            .iter()
            .cloned()
            .zip(value.states)
            .map(|(id, state)| {
                let label = match state {
                    UnityWindowState::Closed => "closed",
                    UnityWindowState::Opening => "opening",
                    UnityWindowState::Open => "open",
                    UnityWindowState::Ready => "ready",
                    UnityWindowState::Unavailable => "unavailable",
                };
                (id, label)
            })
            .collect(),
    }
}

#[tauri::command]
pub(crate) async fn gui_unity_windows_start(
    windows: State<'_, UnityWindows>,
    token: String,
    targets: Vec<Target>,
) -> Result<Snapshot, RpcError> {
    if uuid::Uuid::parse_str(&token).is_err()
        || targets.is_empty()
        || targets.len() > 128
        || targets.iter().any(|target| {
            uuid::Uuid::parse_str(&target.project_id).is_err() || target.revision == 0
        })
    {
        return Err(RpcError::invalid_request());
    }
    #[cfg(not(windows))]
    {
        let _ = windows;
        Ok(Snapshot {
            observing: false,
            version: 0,
            states: Vec::new(),
        })
    }
    #[cfg(windows)]
    {
        {
            *windows.lock() = Some(Active {
                token: token.clone(),
                projects: Vec::new(),
                monitor: None,
                waiting: Arc::default(),
            });
        }
        // A separate command connection keeps this finite registration work
        // from delaying launch, cancellation, and other interactive requests.
        let mut client = None;
        super::connect_if_needed(&mut client).await?;
        let mut authorized = Vec::new();
        for target in targets {
            if !windows
                .lock()
                .as_ref()
                .is_some_and(|active| active.token == token)
            {
                return Err(closed());
            }
            let Some(connection) = client.as_mut() else {
                return Err(super::daemon_unavailable());
            };
            let result = connection.project_get(target.project_id.clone()).await;
            let Ok(result) = super::finish_call(&mut client, result) else {
                continue;
            };
            if result.project.revision == Some(target.revision) {
                authorized.push((target.project_id, result.project.root_path));
            }
        }
        let started = tauri::async_runtime::spawn_blocking(move || {
            let (projects, roots): (Vec<_>, Vec<_>) = authorized
                .into_iter()
                .filter_map(|(id, path)| {
                    super::validated_registered_project_root(&path)
                        .ok()
                        .map(|root| (id, root))
                })
                .unzip();
            if roots.is_empty() {
                return Ok(None);
            }
            alcomd_platform::UnityWindowMonitor::start(roots)
                .map(|monitor| Some((projects, monitor)))
        })
        .await
        .map_err(|_| closed())?
        .map_err(|_| closed())?;
        let Some((projects, monitor)) = started else {
            gui_unity_windows_stop(windows, token);
            return Ok(Snapshot {
                observing: false,
                version: 0,
                states: Vec::new(),
            });
        };
        let value = snapshot(&projects, monitor.reader().snapshot());
        let mut active = windows.lock();
        if !active.as_ref().is_some_and(|active| active.token == token) {
            return Err(closed());
        }
        *active = Some(Active {
            token,
            projects,
            monitor: Some(monitor),
            waiting: Arc::default(),
        });
        Ok(value)
    }
}

#[tauri::command]
pub(crate) async fn gui_unity_windows_next(
    windows: State<'_, UnityWindows>,
    token: String,
    after: u32,
) -> Result<Snapshot, RpcError> {
    #[cfg(not(windows))]
    {
        let _ = (windows, token, after);
        Err(closed())
    }
    #[cfg(windows)]
    {
        let (projects, mut reader, _read) = {
            let active = windows.lock();
            let active = active
                .as_ref()
                .filter(|active| active.token == token)
                .ok_or_else(closed)?;
            let monitor = active.monitor.as_ref().ok_or_else(closed)?;
            if active.waiting.swap(true, Ordering::AcqRel) {
                return Err(RpcError::invalid_request());
            }
            (
                active.projects.clone(),
                monitor.reader(),
                WindowRead(active.waiting.clone()),
            )
        };
        let value = reader.changed(after).await.ok_or_else(closed)?;
        if !windows
            .lock()
            .as_ref()
            .is_some_and(|active| active.token == token)
        {
            return Err(closed());
        }
        Ok(snapshot(&projects, value))
    }
}

#[tauri::command]
pub(crate) fn gui_unity_windows_stop(windows: State<'_, UnityWindows>, token: String) {
    #[cfg(not(windows))]
    let _ = (windows, token);
    #[cfg(windows)]
    {
        let mut active = windows.lock();
        if active.as_ref().is_some_and(|active| active.token == token) {
            active.take();
        }
    }
}

#[tauri::command]
pub(crate) async fn gui_focus_project_unity(
    state: State<'_, GuiClientState>,
    windows: State<'_, UnityWindows>,
    project_id: String,
    revision: u64,
) -> Result<(), RpcError> {
    #[cfg(not(windows))]
    {
        let _ = (state, windows, project_id, revision);
        Err(RpcError::extension("unity_window_observation_unavailable"))
    }
    #[cfg(windows)]
    {
        use alcomd_platform::UnityWindowFocus;
        use alcomd_protocol::UnityWriterStateKind;
        let path = {
            let mut client = state.reads.lock().await;
            super::connect_if_needed(&mut client).await?;
            let connection = client.as_mut().ok_or_else(super::daemon_unavailable)?;
            let result = connection.project_get(project_id.clone()).await;
            let project = super::finish_call(&mut client, result)?.project;
            if project.revision != Some(revision) {
                return Err(RpcError::extension("resource_revision_conflict"));
            }
            let connection = client.as_mut().ok_or_else(super::daemon_unavailable)?;
            let result = connection.unity_writer_state(project_id).await;
            let observed = super::finish_call(&mut client, result)?;
            if !matches!(
                observed.state,
                UnityWriterStateKind::RunningConfirmed | UnityWriterStateKind::RunningSuspected
            ) {
                windows.refresh();
                return Err(RpcError::extension("unity_window_not_found"));
            }
            project.root_path
        };
        let result = tauri::async_runtime::spawn_blocking(move || {
            let root = super::validated_registered_project_root(&path)?;
            alcomd_platform::focus_unity_window(&root)
                .map_err(|_| RpcError::extension("unity_window_focus_failed"))
        })
        .await
        .map_err(|_| RpcError::extension("unity_window_focus_failed"))??;
        windows.refresh();
        match result {
            UnityWindowFocus::Activated => Ok(()),
            UnityWindowFocus::WindowNotFound => Err(RpcError::extension("unity_window_not_found")),
            UnityWindowFocus::Rejected => Err(RpcError::extension("unity_window_focus_rejected")),
        }
    }
}

fn closed() -> RpcError {
    RpcError::extension("unity_window_observation_unavailable")
}
