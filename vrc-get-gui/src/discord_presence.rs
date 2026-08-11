use discord_rich_presence::{DiscordIpc, DiscordIpcClient, activity};
use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;
use sysinfo::System;
use vrc_get_vpm::version::UnityVersion;

const REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const DISCORD_TEXT_MAX_CHARS: usize = 128;
const DISCORD_LARGE_IMAGE_KEY: &str = "unity";
const DISCORD_SMALL_IMAGE_KEY: &str = "alcomd3";

pub struct DiscordPresenceState {
    application_id: Option<&'static str>,
    worker: Mutex<Option<PresenceWorker>>,
    runtime: Arc<Mutex<PresenceRuntime>>,
    sharing_enabled: Arc<AtomicBool>,
}

struct PresenceWorker {
    command: Sender<PresenceWorkerCommand>,
    thread: JoinHandle<()>,
}

enum PresenceWorkerCommand {
    Stop,
    Refresh,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct PresenceRuntime {
    worker_running: bool,
    discord_connected: bool,
    activity: Option<UnityDiscordActivity>,
}

#[derive(Clone, Debug, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UnityDiscordStatus {
    pub enabled: bool,
    pub sharing_enabled: bool,
    pub application_configured: bool,
    pub worker_running: bool,
    pub discord_connected: bool,
    pub activity: Option<UnityDiscordActivity>,
}

#[derive(Clone, Debug, PartialEq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UnityDiscordActivity {
    pub project_name: String,
    pub unity_version: Option<String>,
    pub editor_count: u32,
    pub started_at: f64,
}

impl DiscordPresenceState {
    pub fn new() -> Self {
        Self {
            application_id: crate::alcomd3_config::discord_application_id(),
            worker: Mutex::new(None),
            runtime: Arc::new(Mutex::new(PresenceRuntime::default())),
            sharing_enabled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn status(&self, enabled: bool) -> UnityDiscordStatus {
        let runtime = self.runtime.lock().unwrap().clone();
        UnityDiscordStatus {
            enabled,
            sharing_enabled: self.sharing_enabled.load(Ordering::Relaxed),
            application_configured: self.application_id.is_some(),
            worker_running: runtime.worker_running,
            discord_connected: runtime.discord_connected,
            activity: runtime.activity,
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        if enabled {
            self.start();
        } else {
            self.stop();
        }
    }

    pub fn set_sharing_enabled(&self, enabled: bool) {
        self.sharing_enabled.store(enabled, Ordering::Relaxed);
        let worker = self.worker.lock().unwrap();
        if enabled && self.application_id.is_none() && worker.is_some() {
            log::warn!(
                gui_toast = false;
                "Unity Discord sharing is enabled, but discordApplicationId is not configured"
            );
        }
        if let Some(worker) = worker.as_ref() {
            let _ = worker.command.send(PresenceWorkerCommand::Refresh);
        }
    }

    pub fn shutdown(&self) {
        self.stop();
    }

    fn start(&self) {
        if self.sharing_enabled.load(Ordering::Relaxed) && self.application_id.is_none() {
            log::warn!(
                gui_toast = false;
                "Unity Discord sharing is enabled, but discordApplicationId is not configured"
            );
        }

        let mut worker = self.worker.lock().unwrap();
        if worker.is_some() {
            return;
        }

        let (command, command_receiver) = mpsc::channel();
        let runtime = Arc::clone(&self.runtime);
        let sharing_enabled = Arc::clone(&self.sharing_enabled);
        let application_id = self.application_id;
        match std::thread::Builder::new()
            .name("discord-presence".to_string())
            .spawn(move || {
                run_presence_worker(application_id, command_receiver, runtime, sharing_enabled)
            }) {
            Ok(thread) => {
                *worker = Some(PresenceWorker { command, thread });
                self.runtime.lock().unwrap().worker_running = true;
            }
            Err(error) => {
                log::error!(gui_toast = false; "failed to start Unity Discord status worker: {error}");
            }
        }
    }

    fn stop(&self) {
        let Some(worker) = self.worker.lock().unwrap().take() else {
            *self.runtime.lock().unwrap() = PresenceRuntime::default();
            return;
        };
        let _ = worker.command.send(PresenceWorkerCommand::Stop);
        if worker.thread.join().is_err() {
            log::error!(gui_toast = false; "Unity Discord status worker panicked while stopping");
        }
        *self.runtime.lock().unwrap() = PresenceRuntime::default();
    }
}

fn run_presence_worker(
    application_id: Option<&'static str>,
    command: Receiver<PresenceWorkerCommand>,
    runtime: Arc<Mutex<PresenceRuntime>>,
    sharing_enabled: Arc<AtomicBool>,
) {
    let mut system = System::new();
    let mut client = None;
    let mut connected_once = false;
    let mut unavailable_logged = false;

    loop {
        match command.try_recv() {
            Ok(PresenceWorkerCommand::Stop) | Err(mpsc::TryRecvError::Disconnected) => {
                close_client(&mut client, true);
                *runtime.lock().unwrap() = PresenceRuntime::default();
                return;
            }
            Ok(PresenceWorkerCommand::Refresh) => {}
            Err(mpsc::TryRecvError::Empty) => {}
        }

        if let Some(unity_activity) = detect_unity_activity(&mut system) {
            runtime.lock().unwrap().activity = Some(unity_activity.clone());
            if !sharing_enabled.load(Ordering::Relaxed) {
                close_client(&mut client, true);
                runtime.lock().unwrap().discord_connected = false;
            } else if client.is_none()
                && let Some(application_id) = application_id
            {
                let mut new_client = DiscordIpcClient::new(application_id);
                match new_client.connect() {
                    Ok(()) => {
                        if !connected_once {
                            log::info!(gui_toast = false; "connected Unity Discord status extension");
                            connected_once = true;
                        }
                        unavailable_logged = false;
                        client = Some(new_client);
                        runtime.lock().unwrap().discord_connected = true;
                    }
                    Err(error) => {
                        runtime.lock().unwrap().discord_connected = false;
                        if !unavailable_logged {
                            log::debug!("Discord desktop client is unavailable: {error}");
                            unavailable_logged = true;
                        }
                    }
                }
            }

            if sharing_enabled.load(Ordering::Relaxed)
                && let Some(connected_client) = client.as_mut()
                && let Err(error) = set_activity(connected_client, &unity_activity)
            {
                if !unavailable_logged {
                    log::debug!("failed to publish Unity Discord status: {error}");
                }
                unavailable_logged = true;
                close_client(&mut client, false);
                runtime.lock().unwrap().discord_connected = false;
            }
        } else {
            close_client(&mut client, true);
            let mut runtime = runtime.lock().unwrap();
            runtime.discord_connected = false;
            runtime.activity = None;
        }

        match command.recv_timeout(REFRESH_INTERVAL) {
            Ok(PresenceWorkerCommand::Stop) | Err(RecvTimeoutError::Disconnected) => {
                close_client(&mut client, true);
                *runtime.lock().unwrap() = PresenceRuntime::default();
                return;
            }
            Ok(PresenceWorkerCommand::Refresh) => {}
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

fn close_client(client: &mut Option<DiscordIpcClient>, clear_activity: bool) {
    if let Some(mut client) = client.take() {
        if clear_activity {
            let _ = client.clear_activity();
        }
        let _ = client.close();
    }
}

fn detect_unity_activity(system: &mut System) -> Option<UnityDiscordActivity> {
    let processes = crate::unity_process::refresh_unity_processes(system);
    unity_activity_from_processes(&processes)
}

fn unity_activity_from_processes(
    processes: &[crate::unity_process::UnityProcess],
) -> Option<UnityDiscordActivity> {
    let process = processes
        .iter()
        .max_by_key(|process| (process.started_at, process.process_id))?;
    let project_name = process
        .project_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Untitled Project".to_string());

    Some(UnityDiscordActivity {
        project_name: truncate_discord_text(&project_name),
        unity_version: read_unity_version(&process.project_path),
        editor_count: u32::try_from(processes.len()).unwrap_or(u32::MAX),
        started_at: process.started_at as f64,
    })
}

fn read_unity_version(project_path: &Path) -> Option<String> {
    let source = std::fs::read_to_string(
        project_path
            .join("ProjectSettings")
            .join("ProjectVersion.txt"),
    )
    .ok()?;
    let version = source
        .lines()
        .find_map(|line| line.trim().strip_prefix("m_EditorVersion:").map(str::trim))?;
    UnityVersion::parse(version).map(|version| version.to_string())
}

fn truncate_discord_text(text: &str) -> String {
    if text.chars().count() <= DISCORD_TEXT_MAX_CHARS {
        return text.to_string();
    }

    let mut truncated = text
        .chars()
        .take(DISCORD_TEXT_MAX_CHARS - 1)
        .collect::<String>();
    truncated.push('…');
    truncated
}

fn set_activity(
    client: &mut DiscordIpcClient,
    unity_activity: &UnityDiscordActivity,
) -> Result<(), discord_rich_presence::error::Error> {
    client.set_activity(build_activity(unity_activity))
}

fn build_activity(unity_activity: &UnityDiscordActivity) -> activity::Activity<'static> {
    let details = truncate_discord_text(&format!("Editing {}", unity_activity.project_name));
    let editor = unity_activity.unity_version.as_deref().map_or_else(
        || "Unity Editor".to_string(),
        |version| format!("Unity {version}"),
    );
    let state = if unity_activity.editor_count > 1 {
        format!("{editor} · {} editors open", unity_activity.editor_count)
    } else {
        editor
    };

    activity::Activity::new()
        .name("Unity")
        .details(details)
        .state(truncate_discord_text(&state))
        .assets(
            activity::Assets::new()
                .large_image(DISCORD_LARGE_IMAGE_KEY)
                .large_text("Unity Editor")
                .small_image(DISCORD_SMALL_IMAGE_KEY)
                .small_text("Shared by ALCOMD3"),
        )
        .timestamps(activity::Timestamps::new().start(unity_activity.started_at as i64))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn disabled_state_stops_without_starting_a_worker() {
        let state = DiscordPresenceState {
            application_id: Some("123456789012345678"),
            worker: Mutex::new(None),
            runtime: Arc::new(Mutex::new(PresenceRuntime::default())),
            sharing_enabled: Arc::new(AtomicBool::new(false)),
        };

        state.set_enabled(false);

        assert!(state.worker.lock().unwrap().is_none());
    }

    #[test]
    fn missing_application_id_still_starts_unity_detection() {
        let state = DiscordPresenceState {
            application_id: None,
            worker: Mutex::new(None),
            runtime: Arc::new(Mutex::new(PresenceRuntime::default())),
            sharing_enabled: Arc::new(AtomicBool::new(false)),
        };

        state.set_enabled(true);

        assert!(state.worker.lock().unwrap().is_some());
        assert!(state.status(true).worker_running);
        assert!(!state.status(true).application_configured);
        assert!(!state.status(true).sharing_enabled);
        state.set_enabled(false);
    }

    #[test]
    fn sharing_can_change_without_stopping_unity_detection() {
        let state = DiscordPresenceState {
            application_id: None,
            worker: Mutex::new(None),
            runtime: Arc::new(Mutex::new(PresenceRuntime::default())),
            sharing_enabled: Arc::new(AtomicBool::new(false)),
        };

        state.set_enabled(true);
        state.set_sharing_enabled(true);
        assert!(state.status(true).sharing_enabled);
        assert!(state.status(true).worker_running);

        state.set_sharing_enabled(false);
        assert!(!state.status(true).sharing_enabled);
        assert!(state.status(true).worker_running);
        state.set_enabled(false);
    }

    #[test]
    fn unity_activity_uses_the_most_recent_editor_and_project_metadata() {
        let temporary_directory = tempfile::tempdir().unwrap();
        let older_project = temporary_directory.path().join("Older Avatar");
        let newer_project = temporary_directory.path().join("Newer World");
        std::fs::create_dir_all(newer_project.join("ProjectSettings")).unwrap();
        std::fs::write(
            newer_project.join("ProjectSettings/ProjectVersion.txt"),
            "m_EditorVersion: 2022.3.22f1\n",
        )
        .unwrap();
        let processes = vec![
            crate::unity_process::UnityProcess {
                project_path: older_project,
                process_id: 100,
                started_at: 1000,
            },
            crate::unity_process::UnityProcess {
                project_path: newer_project,
                process_id: 200,
                started_at: 2000,
            },
        ];

        let activity = unity_activity_from_processes(&processes).unwrap();

        assert_eq!(activity.project_name, "Newer World");
        assert_eq!(activity.unity_version.as_deref(), Some("2022.3.22f1"));
        assert_eq!(activity.editor_count, 2);
        assert_eq!(activity.started_at, 2000.0);
    }

    #[test]
    fn unity_activity_is_absent_without_a_unity_editor() {
        assert_eq!(unity_activity_from_processes(&[]), None);
    }

    #[test]
    fn discord_payload_describes_unity_instead_of_alcomd3() {
        let unity_activity = UnityDiscordActivity {
            project_name: "Newer World".to_string(),
            unity_version: Some("2022.3.22f1".to_string()),
            editor_count: 2,
            started_at: 2000.0,
        };

        let payload = serde_json::to_value(build_activity(&unity_activity)).unwrap();

        assert_eq!(payload["name"], "Unity");
        assert_eq!(payload["details"], "Editing Newer World");
        assert_eq!(payload["state"], "Unity 2022.3.22f1 · 2 editors open");
        assert_eq!(payload["assets"]["large_image"], "unity");
        assert_eq!(payload["assets"]["large_text"], "Unity Editor");
        assert_eq!(payload["assets"]["small_image"], "alcomd3");
        assert_eq!(payload["assets"]["small_text"], "Shared by ALCOMD3");
        assert_eq!(payload["timestamps"]["start"], 2000);
        assert!(payload.get("buttons").is_none());
    }

    #[test]
    fn project_names_are_truncated_on_unicode_boundaries() {
        let long_name = "界".repeat(DISCORD_TEXT_MAX_CHARS + 10);

        let truncated = truncate_discord_text(&long_name);

        assert_eq!(truncated.chars().count(), DISCORD_TEXT_MAX_CHARS);
        assert!(truncated.ends_with('…'));
    }

    #[test]
    fn missing_project_name_uses_a_neutral_fallback() {
        let processes = vec![crate::unity_process::UnityProcess {
            project_path: PathBuf::from("/"),
            process_id: 100,
            started_at: 1000,
        }];

        let activity = unity_activity_from_processes(&processes).unwrap();

        assert_eq!(activity.project_name, "Untitled Project");
    }
}
