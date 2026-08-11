use discord_rich_presence::{DiscordIpc, DiscordIpcClient, activity};
use std::path::Path;
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;
use sysinfo::System;
use vrc_get_vpm::version::UnityVersion;

const REFRESH_INTERVAL: Duration = Duration::from_secs(15);
const DISCORD_TEXT_MAX_CHARS: usize = 128;

pub struct DiscordPresenceState {
    application_id: Option<&'static str>,
    worker: Mutex<Option<PresenceWorker>>,
}

struct PresenceWorker {
    stop: Sender<()>,
    thread: JoinHandle<()>,
}

impl DiscordPresenceState {
    pub fn new() -> Self {
        Self {
            application_id: crate::alcomd3_config::discord_application_id(),
            worker: Mutex::new(None),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        if enabled {
            self.start();
        } else {
            self.stop();
        }
    }

    pub fn shutdown(&self) {
        self.stop();
    }

    fn start(&self) {
        let Some(application_id) = self.application_id else {
            log::warn!(
                gui_toast = false;
                "Unity Discord status extension is enabled, but discordApplicationId is not configured"
            );
            return;
        };

        let mut worker = self.worker.lock().unwrap();
        if worker.is_some() {
            return;
        }

        let (stop, stop_receiver) = mpsc::channel();
        match std::thread::Builder::new()
            .name("discord-presence".to_string())
            .spawn(move || run_presence_worker(application_id, stop_receiver))
        {
            Ok(thread) => {
                *worker = Some(PresenceWorker { stop, thread });
            }
            Err(error) => {
                log::error!(gui_toast = false; "failed to start Unity Discord status worker: {error}");
            }
        }
    }

    fn stop(&self) {
        let Some(worker) = self.worker.lock().unwrap().take() else {
            return;
        };
        let _ = worker.stop.send(());
        if worker.thread.join().is_err() {
            log::error!(gui_toast = false; "Unity Discord status worker panicked while stopping");
        }
    }
}

fn run_presence_worker(application_id: &'static str, stop: Receiver<()>) {
    let mut system = System::new();
    let mut client = None;
    let mut connected_once = false;
    let mut unavailable_logged = false;

    loop {
        match stop.try_recv() {
            Ok(()) | Err(mpsc::TryRecvError::Disconnected) => {
                close_client(&mut client, true);
                return;
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }

        if let Some(unity_activity) = detect_unity_activity(&mut system) {
            if client.is_none() {
                let mut new_client = DiscordIpcClient::new(application_id);
                match new_client.connect() {
                    Ok(()) => {
                        if !connected_once {
                            log::info!(gui_toast = false; "connected Unity Discord status extension");
                            connected_once = true;
                        }
                        unavailable_logged = false;
                        client = Some(new_client);
                    }
                    Err(error) => {
                        if !unavailable_logged {
                            log::debug!("Discord desktop client is unavailable: {error}");
                            unavailable_logged = true;
                        }
                    }
                }
            }

            if let Some(connected_client) = client.as_mut()
                && let Err(error) = set_activity(connected_client, &unity_activity)
            {
                if !unavailable_logged {
                    log::debug!("failed to publish Unity Discord status: {error}");
                }
                unavailable_logged = true;
                close_client(&mut client, false);
            }
        } else {
            close_client(&mut client, true);
        }

        match stop.recv_timeout(REFRESH_INTERVAL) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => {
                close_client(&mut client, true);
                return;
            }
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct UnityActivity {
    project_name: String,
    unity_version: Option<String>,
    editor_count: usize,
    started_at: i64,
}

fn detect_unity_activity(system: &mut System) -> Option<UnityActivity> {
    let processes = crate::unity_process::refresh_unity_processes(system);
    unity_activity_from_processes(&processes)
}

fn unity_activity_from_processes(
    processes: &[crate::unity_process::UnityProcess],
) -> Option<UnityActivity> {
    let process = processes
        .iter()
        .max_by_key(|process| (process.started_at, process.process_id))?;
    let project_name = process
        .project_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Untitled Project".to_string());

    Some(UnityActivity {
        project_name: truncate_discord_text(&project_name),
        unity_version: read_unity_version(&process.project_path),
        editor_count: processes.len(),
        started_at: i64::try_from(process.started_at).unwrap_or(i64::MAX),
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
    unity_activity: &UnityActivity,
) -> Result<(), discord_rich_presence::error::Error> {
    client.set_activity(build_activity(unity_activity))
}

fn build_activity(unity_activity: &UnityActivity) -> activity::Activity<'static> {
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
        .timestamps(activity::Timestamps::new().start(unity_activity.started_at))
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
        };

        state.set_enabled(false);

        assert!(state.worker.lock().unwrap().is_none());
    }

    #[test]
    fn missing_application_id_does_not_start_a_worker() {
        let state = DiscordPresenceState {
            application_id: None,
            worker: Mutex::new(None),
        };

        state.set_enabled(true);

        assert!(state.worker.lock().unwrap().is_none());
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
        assert_eq!(activity.started_at, 2000);
    }

    #[test]
    fn unity_activity_is_absent_without_a_unity_editor() {
        assert_eq!(unity_activity_from_processes(&[]), None);
    }

    #[test]
    fn discord_payload_describes_unity_instead_of_alcomd3() {
        let unity_activity = UnityActivity {
            project_name: "Newer World".to_string(),
            unity_version: Some("2022.3.22f1".to_string()),
            editor_count: 2,
            started_at: 2000,
        };

        let payload = serde_json::to_value(build_activity(&unity_activity)).unwrap();

        assert_eq!(payload["name"], "Unity");
        assert_eq!(payload["details"], "Editing Newer World");
        assert_eq!(payload["state"], "Unity 2022.3.22f1 · 2 editors open");
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
