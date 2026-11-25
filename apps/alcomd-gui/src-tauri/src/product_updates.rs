use std::ffi::OsString;
#[cfg(windows)]
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Mutex;

#[cfg(target_os = "linux")]
use alcomd_platform::DataConfig;
use alcomd_updater::{
    Channel,
    product_update::{
        AvailableUpdate, InstalledProductUpdater, StagedUpdate, UpdateFormat,
        desktop_update_failed, desktop_update_storage_location, prepare_desktop_update_storage,
    },
};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State, WebviewWindow, async_runtime};

#[derive(Default)]
pub(crate) struct DesktopUpdates(Mutex<Session>);

#[derive(Default)]
struct Session {
    activity: Activity,
    generation: u64,
    task: Option<async_runtime::JoinHandle<()>>,
}

#[derive(Default)]
enum Activity {
    #[default]
    Idle,
    Checking,
    Current,
    Available(AvailableUpdate),
    Downloading,
    Ready(StagedUpdate),
    Failed,
    Restarting,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum UpdateAction {
    Status,
    Check,
    Download,
    Cancel,
    Install,
    Resume,
}

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub(crate) enum UpdateStatus {
    Unsupported,
    Idle,
    Checking,
    Current,
    Available { version: String, bytes: u64 },
    Downloading,
    Ready { version: String },
    Failed,
    InstallFailed,
    Recovery,
    Restarting,
}

impl Activity {
    fn status(&self) -> UpdateStatus {
        match self {
            Self::Idle => UpdateStatus::Idle,
            Self::Checking => UpdateStatus::Checking,
            Self::Current => UpdateStatus::Current,
            Self::Available(offer) => UpdateStatus::Available {
                version: offer.verified().version().to_string(),
                bytes: offer.verified().asset().bytes(),
            },
            Self::Downloading => UpdateStatus::Downloading,
            Self::Ready(staged) => UpdateStatus::Ready {
                version: staged.verified.version().to_string(),
            },
            Self::Failed => UpdateStatus::Failed,
            Self::Restarting => UpdateStatus::Restarting,
        }
    }
}

struct Installation {
    bootstrap: PathBuf,
    format: UpdateFormat,
    recovery: bool,
}

const UPDATE_UNAVAILABLE: &str = "product_update_unavailable";
const UPDATE_BUSY: &str = "product_update_busy";

#[tauri::command]
pub(crate) fn gui_product_update(
    app: AppHandle,
    window: WebviewWindow,
    updates: State<'_, DesktopUpdates>,
    action: UpdateAction,
) -> Result<UpdateStatus, &'static str> {
    if window.label() != "main" {
        return Err(UPDATE_UNAVAILABLE);
    }
    let Some(installation) = installed().map_err(|_| UPDATE_UNAVAILABLE)? else {
        return Ok(UpdateStatus::Unsupported);
    };
    let mut session = updates.0.lock().map_err(|_| UPDATE_UNAVAILABLE)?;
    if matches!(session.activity, Activity::Restarting) {
        return Ok(UpdateStatus::Restarting);
    }
    if installation.recovery {
        if matches!(action, UpdateAction::Resume) {
            start_bootstrap(
                &installation,
                &["resume-installed-update".into(), "--desktop".into()],
            )
            .map_err(|_| UPDATE_UNAVAILABLE)?;
            session.activity = Activity::Restarting;
            app.exit(0);
            return Ok(UpdateStatus::Restarting);
        }
        return Ok(UpdateStatus::Recovery);
    }

    match action {
        UpdateAction::Status => {}
        UpdateAction::Cancel => {
            if let Some(task) = session.task.take() {
                session.generation = session.generation.wrapping_add(1);
                task.abort();
                session.activity = Activity::Idle;
            }
        }
        UpdateAction::Check => {
            let (updater, storage) = prepare(&installation)?;
            session.start(&app, Activity::Checking, async move {
                match updater.check(&storage).await {
                    Ok(Some(offer)) => Activity::Available(offer),
                    Ok(None) => Activity::Current,
                    Err(_) => Activity::Failed,
                }
            })?;
        }
        UpdateAction::Download => {
            let (updater, storage) = prepare(&installation)?;
            if !matches!(session.activity, Activity::Available(_)) {
                return Err(UPDATE_UNAVAILABLE);
            }
            let Activity::Available(offer) =
                std::mem::replace(&mut session.activity, Activity::Downloading)
            else {
                unreachable!("checked update activity");
            };
            session.start(&app, Activity::Downloading, async move {
                match updater.download(offer, &storage, &storage).await {
                    Ok(staged) => Activity::Ready(staged),
                    Err(_) => Activity::Failed,
                }
            })?;
        }
        UpdateAction::Install => {
            let Activity::Ready(staged) = &session.activity else {
                return Err(UPDATE_UNAVAILABLE);
            };
            let storage = desktop_update_storage_location().map_err(|_| UPDATE_UNAVAILABLE)?;
            let command = match installation.format {
                UpdateFormat::Inno => "apply-installed-update",
                UpdateFormat::Appimage => "apply-portable-update",
                UpdateFormat::Dmg => return Err(UPDATE_UNAVAILABLE),
            };
            let channel = match staged.verified.acceptance().stream.channel {
                Channel::Stable => "stable",
                Channel::Beta => "beta",
            };
            start_bootstrap(
                &installation,
                &[
                    command.into(),
                    "--desktop".into(),
                    "--channel".into(),
                    channel.into(),
                    "--envelope".into(),
                    staged.envelope_path.as_os_str().into(),
                    "--asset".into(),
                    staged.path.as_os_str().into(),
                    "--replay-directory".into(),
                    storage.into_os_string(),
                ],
            )
            .map_err(|_| UPDATE_UNAVAILABLE)?;
            session.activity = Activity::Restarting;
            app.exit(0);
        }
        UpdateAction::Resume => return Err(UPDATE_UNAVAILABLE),
    }
    if matches!(session.activity, Activity::Idle) && desktop_update_failed().unwrap_or(false) {
        return Ok(UpdateStatus::InstallFailed);
    }
    Ok(session.activity.status())
}

impl Session {
    fn start(
        &mut self,
        app: &AppHandle,
        initial: Activity,
        operation: impl Future<Output = Activity> + Send + 'static,
    ) -> Result<(), &'static str> {
        if self.task.is_some() {
            return Err(UPDATE_BUSY);
        }
        self.activity = initial;
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let app = app.clone();
        self.task = Some(async_runtime::spawn(async move {
            let completed = operation.await;
            let updates = app.state::<DesktopUpdates>();
            if let Ok(mut session) = updates.0.lock()
                && session.generation == generation
            {
                session.activity = completed;
                session.task = None;
            }
        }));
        Ok(())
    }
}

fn prepare(
    installation: &Installation,
) -> Result<(InstalledProductUpdater, PathBuf), &'static str> {
    let updater = InstalledProductUpdater::new(installation.format, env!("CARGO_PKG_VERSION"))
        .map_err(|_| UPDATE_UNAVAILABLE)?;
    let storage = prepare_desktop_update_storage().map_err(|_| UPDATE_UNAVAILABLE)?;
    Ok((updater, storage))
}

#[cfg(windows)]
fn installed() -> io::Result<Option<Installation>> {
    use alcomd_platform::{
        WINDOWS_BOOTSTRAP_EXECUTABLE_NAME, WINDOWS_UPDATE_INTENT_RECORD,
        open_owned_installation_record, open_windows_machine_installation_record,
        windows_current_user_installation_root, windows_installation_bootstrap,
        windows_installation_runtime, windows_machine_installation_root,
        windows_machine_runtime_bootstrap,
    };

    let entry = std::env::current_exe()?;
    let Ok(runtime) = windows_installation_runtime(&entry) else {
        return Ok(None);
    };
    let Some(root) = runtime.parent().and_then(std::path::Path::parent) else {
        return Ok(None);
    };
    let machine = windows_machine_installation_root().is_ok_and(|expected| expected == root);
    if !machine && !windows_current_user_installation_root().is_ok_and(|expected| expected == root)
    {
        return Ok(None);
    }
    let intent = root.join(WINDOWS_UPDATE_INTENT_RECORD);
    let recovery = match fs::symlink_metadata(&intent) {
        Ok(_) if machine => {
            open_windows_machine_installation_record(&intent)?;
            true
        }
        Ok(_) => {
            open_owned_installation_record(&intent)?;
            true
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => false,
        Err(error) => return Err(error),
    };
    let bootstrap = if recovery && machine {
        windows_machine_runtime_bootstrap(&runtime)?
    } else if recovery {
        let bootstrap = runtime.join(WINDOWS_BOOTSTRAP_EXECUTABLE_NAME);
        open_owned_installation_record(&bootstrap)?;
        bootstrap
    } else {
        let Some(bootstrap) = windows_installation_bootstrap(&entry)? else {
            return Ok(None);
        };
        bootstrap
    };
    Ok(Some(Installation {
        bootstrap,
        format: UpdateFormat::Inno,
        recovery,
    }))
}

#[cfg(target_os = "linux")]
fn installed() -> io::Result<Option<Installation>> {
    alcomd_platform::linux_portable_update_bootstrap(
        &std::env::current_exe()?,
        &DataConfig::default(),
    )
    .map(|bootstrap| {
        bootstrap.map(|bootstrap| Installation {
            bootstrap,
            format: UpdateFormat::Appimage,
            recovery: false,
        })
    })
}

#[cfg(not(any(windows, target_os = "linux")))]
fn installed() -> io::Result<Option<Installation>> {
    Ok(None)
}

fn start_bootstrap(installation: &Installation, arguments: &[OsString]) -> io::Result<()> {
    let working_directory = installation
        .bootstrap
        .parent()
        .ok_or_else(|| io::Error::from(io::ErrorKind::InvalidInput))?;
    let arguments = arguments.iter().map(OsString::as_os_str);
    #[cfg(windows)]
    return alcomd_platform::start_core_process(
        &installation.bootstrap,
        arguments,
        Some(working_directory),
        "ALCOMD_MCP_CLIENT_CREDENTIAL",
    );
    #[cfg(not(windows))]
    std::process::Command::new(&installation.bootstrap)
        .args(arguments)
        .current_dir(working_directory)
        .env_remove("ALCOMD_MCP_CLIENT_CREDENTIAL")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map(drop)
}
