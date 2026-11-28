use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use alcomd_platform::{
    CORE_EXECUTABLE_NAME, DaemonInstance, DataConfig, IpcConfig, WINDOWS_BOOTSTRAP_EXECUTABLE_NAME,
    WINDOWS_CLI_EXECUTABLE_NAME, WINDOWS_GUI_EXECUTABLE_NAME, WINDOWS_UPDATE_INTENT_RECORD,
    WindowsInstallationUpdate, check_windows_core_registration,
    create_windows_machine_installation_directory, create_windows_machine_installation_record,
    hold_windows_installation, open_owned_installation_record,
    open_windows_machine_installation_record, prepare_windows_machine_update,
    publish_path_no_replace, receive_windows_machine_update, resolve_plain_storage_directory,
    start_core_process, start_elevated_windows_program, sync_directory,
    validate_installation_record_parent, validate_windows_machine_installation_record_parent,
    windows_available_storage, windows_current_user_installation_root,
    windows_installation_bootstrap, windows_installation_runtime,
    windows_machine_installation_root, windows_process_is_elevated,
};
use alcomd_updater::{
    Channel,
    distribution::{
        MANIFEST_NAME, SIGNATURE_NAME, Target, VerifiedRuntime, WINDOWS_CRT_FILE,
        WINDOWS_UPDATE_STAGE_PREFIX, format_digest, verify_installed_release,
    },
    product_update::{
        AcceptedUpdate, MAX_UPDATE_RESPONSE_BYTES, VerifiedUpdate,
        accept_windows_machine_product_update, accept_windows_product_update,
        record_desktop_update_failure, reverify_interrupted_windows_update,
        verify_fresh_windows_product_update,
    },
};
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const ENVELOPE: &str = "envelope.json";
const ACCEPTANCE: &str = "accepted.json";
const INSTALLED_RECEIPT: &str = "installed-v1";
const PRIVATE_RECORD_LIMIT: usize = 4096;
const INSTALLED_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Lets the closing desktop and its Core disconnect before entering maintenance.
pub fn with_windows_desktop_restart(
    operation: impl FnOnce() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let installation = Installation::installed()?;
    let result = wait_for_idle_core().and_then(|()| operation());
    if record_desktop_update_failure(result.is_err()).is_err() {
        eprintln!("Windows desktop update could not store its restart outcome");
    }
    let relaunch = relaunch_desktop(&installation);
    result.and(relaunch)
}

fn wait_for_idle_core() -> anyhow::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(18);
    loop {
        match DaemonInstance::acquire(&IpcConfig::default()) {
            Ok(_) => return Ok(()),
            Err(alcomd_platform::BindError::AlreadyRunning) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(150));
            }
            Err(error) => {
                return Err(error).context("close the current user's Core before updating");
            }
        }
    }
}

fn relaunch_desktop(installation: &Installation) -> anyhow::Result<()> {
    let cli = installation
        .root
        .join("bin")
        .join(WINDOWS_CLI_EXECUTABLE_NAME);
    let bootstrap =
        windows_installation_bootstrap(&cli)?.context("active Windows runtime is unavailable")?;
    let runtime = bootstrap
        .parent()
        .context("active Windows runtime is unavailable")?;
    let version = runtime
        .file_name()
        .and_then(OsStr::to_str)
        .context("invalid active Windows runtime")?;
    let verified = verify_runtime(runtime, version)?;
    if installation.scope == Scope::Machine {
        check_machine_runtime(runtime, &verified)?;
    }
    start_core_process(
        &runtime.join(WINDOWS_GUI_EXECUTABLE_NAME),
        [],
        Some(runtime),
        "ALCOMD_MCP_CLIENT_CREDENTIAL",
    )
    .context("could not reopen the installed desktop")
}

/// Extracts a verified update and checks the user's data layout without opening it for writes.
pub fn stage_installed_update(
    envelope: &Path,
    asset: &Path,
    replay_directory: &Path,
    channel: Channel,
) -> anyhow::Result<PathBuf> {
    let paths = Installation::installed()?;
    ensure!(
        paths.scope == Scope::CurrentUser,
        "a current-user Windows installation is required"
    );
    let _installation = hold_windows_installation(&paths.executable)
        .context("the current installation is undergoing maintenance")?
        .context("a current-user Windows installation is required")?;
    verify_runtime(&paths.runtime, INSTALLED_VERSION)?;
    let _asset = open_owned_installation_record(asset)?;
    let bytes = paths
        .scope
        .read_record(envelope, MAX_UPDATE_RESPONSE_BYTES)?;
    let (update, _) = accept_update(&bytes, asset, replay_directory, channel)?;
    let _instance = DaemonInstance::acquire(&IpcConfig::default())
        .context("close the current user's Core before staging the update")?;
    paths.prepare(&update, asset)
}

/// Verifies and installs a newer product in the existing Windows scope.
pub fn apply_installed_update(
    envelope: &Path,
    asset: &Path,
    replay_directory: &Path,
    channel: Channel,
) -> anyhow::Result<()> {
    let paths = Installation::installed()?;
    ensure_absent_intent(&paths)?;
    if paths.scope == Scope::Machine {
        return request_machine_update(&paths, envelope, asset, channel);
    }
    let previous = verify_runtime(&paths.runtime, INSTALLED_VERSION)?;
    let mut asset_handle = open_owned_installation_record(asset)?;
    let bytes = paths
        .scope
        .read_record(envelope, MAX_UPDATE_RESPONSE_BYTES)?;
    let (update, accepted_at) = accept_update(&bytes, asset, replay_directory, channel)?;
    let maintenance =
        WindowsInstallationUpdate::acquire(&paths.executable, &update.version().to_string())
            .context("close the current user's Core before updating the installation")?;
    ensure_absent_intent(&paths)?;
    ensure!(
        verify_runtime(&paths.runtime, INSTALLED_VERSION)?.manifest_sha256
            == previous.manifest_sha256,
        "installed Windows runtime changed before maintenance"
    );
    ensure!(
        !exists(maintenance.target_runtime())?,
        "the Windows update target already exists"
    );
    verify_stable_cli(&paths.root, &previous)?;
    check_windows_core_registration(&DataConfig::default(), &paths.runtime)
        .context("the existing Core registration does not belong to this installation")?;

    let staged = paths.prepare(&update, asset)?;
    let transaction = transaction_path(&staged)?;
    let intent = Intent {
        previous_version: INSTALLED_VERSION.to_owned(),
        previous_digest: format_digest(&previous.manifest_sha256),
        target_version: update.version().to_string(),
        target_digest: format_digest(&update.payload_manifest_sha256()),
    };
    retain_transaction(
        paths.scope,
        &transaction,
        &bytes,
        &mut asset_handle,
        &update,
        Acceptance {
            receipt: update.acceptance(),
            accepted_at,
        },
    )?;
    publish_record(paths.scope, &paths.intent(), &intent.encode())?;
    finish_update(&paths, maintenance, &intent)
}

/// Continues the exact committed Windows update using the retained signed inputs.
pub fn resume_installed_update() -> anyhow::Result<()> {
    let paths = Installation::installed()?;
    if paths.scope == Scope::Machine {
        paths.require_machine_program()?;
        paths
            .scope
            .read_record(&paths.intent(), PRIVATE_RECORD_LIMIT)?;
        start_elevated_windows_program(
            &paths.executable,
            &paths.runtime,
            &[OsStr::new("resume-machine-update-worker")],
        )?
        .wait()?;
        return Ok(());
    }
    resume_update(&paths)
}

/// Privileged entry point. Machine maintenance never opens profile data.
pub fn resume_machine_update_worker() -> anyhow::Result<()> {
    resume_update(&Installation::elevated_machine()?)
}

fn resume_update(paths: &Installation) -> anyhow::Result<()> {
    let intent = Intent::parse(
        &paths
            .scope
            .read_record(&paths.intent(), PRIVATE_RECORD_LIMIT)?,
    )?;
    if INSTALLED_VERSION == intent.target_version {
        let previous = paths.root.join("runtime").join(&intent.previous_version);
        let runtime = verify_runtime(&previous, &intent.previous_version)?;
        ensure!(
            format_digest(&runtime.manifest_sha256) == intent.previous_digest,
            "the previous Windows runtime does not match the update intent"
        );
        let operation = match paths.scope {
            Scope::CurrentUser => "resume-installed-update",
            Scope::Machine => {
                check_machine_runtime(&previous, &runtime)?;
                "resume-machine-update-worker"
            }
        };
        super::super::run_command(
            &previous.join(WINDOWS_BOOTSTRAP_EXECUTABLE_NAME),
            &previous,
            &[OsStr::new(operation)],
        )
        .context("the previous Windows Bootstrap could not resume the update")?;
        return Ok(());
    }
    ensure!(
        INSTALLED_VERSION == intent.previous_version,
        "this Windows Bootstrap does not belong to the interrupted update"
    );
    let maintenance = paths.maintenance(&intent.target_version)?;
    finish_update(paths, maintenance, &intent)
}

fn finish_update(
    paths: &Installation,
    mut maintenance: WindowsInstallationUpdate,
    intent: &Intent,
) -> anyhow::Result<()> {
    let encoded_intent = intent.encode();
    ensure!(
        paths
            .scope
            .read_record(&paths.intent(), PRIVATE_RECORD_LIMIT)?
            == encoded_intent,
        "Windows update intent changed"
    );
    let previous = verify_runtime(&paths.runtime, INSTALLED_VERSION)?;
    ensure!(
        format_digest(&previous.manifest_sha256) == intent.previous_digest,
        "the previous Windows runtime does not match the update intent"
    );
    let staged = paths.stage_path(&intent.target_digest)?;
    let transaction = transaction_path(&staged)?;
    let update = retained_update(paths.scope, &transaction)?;
    ensure!(
        update.version().to_string() == intent.target_version
            && format_digest(&update.payload_manifest_sha256()) == intent.target_digest,
        "the retained Windows update does not match the update intent"
    );
    let asset = transaction.join(update.asset().name());
    let _asset = paths.scope.open_record(&asset)?;
    update.asset().verify_file(&asset)?;
    paths.verify_staged(&staged, &update)?;
    paths.preflight(&staged, &intent.target_version)?;

    let receipt = transaction.join(INSTALLED_RECEIPT);
    let expected_receipt = format!("{}\n", intent.target_digest);
    if !receipt_matches(paths.scope, &receipt, expected_receipt.as_bytes())? {
        let digest = format!("/ALCOMDCommit={}", intent.target_digest);
        let version = format!("/ALCOMDPrevious={}", intent.previous_version);
        let previous = format!("/ALCOMDPreviousDigest={}", intent.previous_digest);
        super::super::run_command(
            &asset,
            &paths.runtime,
            &paths.scope.setup_arguments([&digest, &version, &previous]),
        )
        .context("Windows setup did not finish; resume the same committed update")?;
        ensure!(
            receipt_matches(paths.scope, &receipt, expected_receipt.as_bytes())?,
            "Windows setup did not publish its completion receipt"
        );
    }
    maintenance.lock_target()?;
    let target = verify_runtime(maintenance.target_runtime(), &intent.target_version)?;
    ensure!(
        target.manifest_sha256 == update.payload_manifest_sha256(),
        "the installed Windows runtime does not match the verified update"
    );
    verify_stable_cli(&paths.root, &target)?;
    if paths.scope == Scope::CurrentUser {
        maintenance.publish_core_registration(&DataConfig::default())?;
    } else {
        check_machine_runtime(maintenance.target_runtime(), &target)?;
        paths
            .scope
            .open_record(&paths.root.join("bin").join(WINDOWS_CLI_EXECUTABLE_NAME))?;
        paths
            .scope
            .open_record(&paths.root.join("bin").join(WINDOWS_CRT_FILE))?;
    }
    maintenance.activate()?;
    maintenance.finish(&encoded_intent)?;
    Ok(())
}

fn request_machine_update(
    paths: &Installation,
    envelope: &Path,
    asset: &Path,
    channel: Channel,
) -> anyhow::Result<()> {
    paths.require_machine_program()?;
    let bytes = Scope::CurrentUser.read_record(envelope, MAX_UPDATE_RESPONSE_BYTES)?;
    let (update, _) =
        verify_fresh_windows_product_update(&bytes, channel, &INSTALLED_VERSION.parse()?)?;
    check_asset_name(asset, &update)?;
    let input = open_owned_installation_record(asset)?;
    update.asset().verify_file(asset)?;
    let transfer = Uuid::new_v4().simple().to_string();
    let output = prepare_windows_machine_update(&transfer)?;
    let pid = std::process::id().to_string();
    let channel = match channel {
        Channel::Stable => "stable",
        Channel::Beta => "beta",
    };
    let worker = start_elevated_windows_program(
        &paths.executable,
        &paths.runtime,
        &[
            OsStr::new("apply-machine-update-worker"),
            OsStr::new("--transfer"),
            OsStr::new(&transfer),
            OsStr::new("--requester-pid"),
            OsStr::new(&pid),
            OsStr::new("--channel"),
            OsStr::new(channel),
        ],
    )
    .context("could not elevate Windows all-users update maintenance")?;
    let transfer = output.send(&worker, &bytes, input, update.asset().bytes());
    let transferred = transfer.is_ok();
    worker.wait_for_machine_update(transfer).map_err(|error| {
        let message = if error.kind() == io::ErrorKind::StorageFull {
            "there is insufficient free space for the all-users Windows update"
        } else if transferred {
            "the all-users update has not completed"
        } else {
            "could not transfer the signed Windows update to machine maintenance"
        };
        anyhow::Error::new(error).context(message)
    })
}

/// Receives only bounded signed data from the initiating process in the local session.
pub fn apply_machine_update_worker(
    transfer: &str,
    requester_pid: u32,
    channel: Channel,
) -> anyhow::Result<()> {
    let paths = Installation::elevated_machine()?;
    ensure_absent_intent(&paths)?;
    let mut input = receive_windows_machine_update(transfer, requester_pid)?;
    let mut length = [0_u8; 4];
    input.read_exact(&mut length)?;
    let length = usize::try_from(u32::from_le_bytes(length))?;
    ensure!(
        (1..=MAX_UPDATE_RESPONSE_BYTES).contains(&length),
        "invalid machine update declaration length"
    );
    let mut bytes = vec![0; length];
    input.read_exact(&mut bytes)?;
    let (update, accepted_at) =
        verify_fresh_windows_product_update(&bytes, channel, &INSTALLED_VERSION.parse()?)?;
    let maintenance = paths.maintenance(&update.version().to_string())?;
    ensure_absent_intent(&paths)?;
    ensure!(
        !exists(maintenance.target_runtime())?,
        "the Windows update target already exists"
    );
    let previous = paths.require_machine_program()?;
    verify_stable_cli(&paths.root, &previous)?;
    let target_digest = format_digest(&update.payload_manifest_sha256());
    let staged = paths.stage_path(&target_digest)?;
    let transaction = transaction_path(&staged)?;
    let mut input = input.take(update.asset().bytes());
    retain_transaction(
        paths.scope,
        &transaction,
        &bytes,
        &mut input,
        &update,
        Acceptance {
            receipt: update.acceptance(),
            accepted_at,
        },
    )?;
    // An interrupted pre-intent attempt can already have retained the signed
    // asset. Consume the remainder so the requesting process finishes sending.
    let remaining = input.limit();
    ensure!(
        io::copy(&mut input, &mut io::sink())? == remaining,
        "incomplete machine update transfer"
    );
    let asset = transaction.join(update.asset().name());
    let _asset = paths.scope.open_record(&asset)?;
    let replay = paths.machine_replay_directory()?;
    let accepted = accept_windows_machine_product_update(
        &bytes,
        &asset,
        &replay,
        channel,
        &INSTALLED_VERSION.parse()?,
    )?;
    ensure!(
        accepted.acceptance() == update.acceptance(),
        "machine update acceptance changed"
    );
    paths.prepare(&update, &asset)?;
    let intent = Intent {
        previous_version: INSTALLED_VERSION.to_owned(),
        previous_digest: format_digest(&previous.manifest_sha256),
        target_version: update.version().to_string(),
        target_digest,
    };
    publish_record(paths.scope, &paths.intent(), &intent.encode())?;
    finish_update(&paths, maintenance, &intent)
}

struct Installation {
    executable: PathBuf,
    runtime: PathBuf,
    root: PathBuf,
    scope: Scope,
}

impl Installation {
    fn installed() -> anyhow::Result<Self> {
        let executable = std::env::current_exe()?;
        let runtime = windows_installation_runtime(&executable)?;
        let root = runtime
            .parent()
            .and_then(Path::parent)
            .context("missing Windows installation root")?;
        let scope =
            if windows_current_user_installation_root().is_ok_and(|expected| expected == root) {
                Scope::CurrentUser
            } else {
                ensure!(
                    windows_machine_installation_root()? == root,
                    "unsupported Windows installation root"
                );
                Scope::Machine
            };
        let root = root.to_path_buf();
        Ok(Self {
            executable,
            runtime,
            root,
            scope,
        })
    }

    fn elevated_machine() -> anyhow::Result<Self> {
        let paths = Self::installed()?;
        ensure!(
            paths.scope == Scope::Machine && windows_process_is_elevated()?,
            "an elevated all-users Windows installation is required"
        );
        paths.require_machine_program()?;
        Ok(paths)
    }

    fn require_machine_program(&self) -> anyhow::Result<VerifiedRuntime> {
        self.scope.open_record(&self.executable)?;
        let verified = verify_runtime(&self.runtime, INSTALLED_VERSION)?;
        check_machine_runtime(&self.runtime, &verified)?;
        Ok(verified)
    }

    fn maintenance(&self, target: &str) -> anyhow::Result<WindowsInstallationUpdate> {
        match self.scope {
            Scope::CurrentUser => WindowsInstallationUpdate::acquire(&self.executable, target)
                .context("close the current user's Core before updating the installation"),
            Scope::Machine => WindowsInstallationUpdate::acquire_machine(&self.executable, target)
                .context("close Core in all signed-in accounts before updating the installation"),
        }
    }

    fn intent(&self) -> PathBuf {
        self.root.join(WINDOWS_UPDATE_INTENT_RECORD)
    }

    fn machine_replay_directory(&self) -> anyhow::Result<PathBuf> {
        let name = self
            .root
            .file_name()
            .and_then(OsStr::to_str)
            .context("Windows installation name")?;
        let path = self
            .root
            .with_file_name(format!(".{name}.update-replay-v1"));
        match create_windows_machine_installation_directory(&path) {
            Ok(()) => sync_directory(path.parent().context("Windows update replay parent")?)?,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        self.scope.validate_parent(&path.join("record"))?;
        Ok(path)
    }

    fn stage_path(&self, digest: &str) -> anyhow::Result<PathBuf> {
        let name = self
            .root
            .file_name()
            .and_then(OsStr::to_str)
            .context("Windows installation root needs a plain name")?;
        Ok(self
            .root
            .with_file_name(format!(".{name}{WINDOWS_UPDATE_STAGE_PREFIX}{digest}")))
    }

    fn prepare(&self, update: &VerifiedUpdate, asset: &Path) -> anyhow::Result<PathBuf> {
        let digest = format_digest(&update.payload_manifest_sha256());
        let staged = self.stage_path(&digest)?;
        if !exists(&staged)? {
            let request = format!("/ALCOMDStage={digest}");
            // Staging exits Inno before it can enter the normal installation flow.
            super::super::run_command_with_exit_code(
                asset,
                &self.runtime,
                &self.scope.setup_arguments([&request]),
                1,
            )
            .context("could not extract the Windows product update")?;
            if !exists(&staged)? {
                // Inno's staging-only path uses the same exit status whether it publishes
                // the payload or stops before publication. Activation also needs to retain
                // the complete signed installer on this volume.
                let installer_bytes = update.asset().bytes();
                if let Ok(available) = windows_available_storage(&self.root)
                    && available < installer_bytes
                {
                    return Err(io::Error::from(io::ErrorKind::StorageFull)).with_context(|| format!(
                        "could not stage the Windows product update: insufficient free space on the installation volume ({available} bytes available, signed installer is {installer_bytes} bytes)"
                    ));
                }
                anyhow::bail!("the installer did not publish the Windows product update stage");
            }
        }
        self.verify_staged(&staged, update)?;
        sync_directory(staged.parent().context("Windows update staging parent")?)?;
        self.preflight(&staged, &update.version().to_string())?;
        Ok(staged)
    }

    fn verify_staged(&self, staged: &Path, update: &VerifiedUpdate) -> anyhow::Result<()> {
        self.scope.validate_parent(&staged.join(MANIFEST_NAME))?;
        let verified = verify_runtime(staged, &update.version().to_string())?;
        ensure!(
            verified.manifest_sha256 == update.payload_manifest_sha256(),
            "staged Windows update has a different payload digest"
        );
        if self.scope == Scope::Machine {
            check_machine_runtime(staged, &verified)?;
        }
        Ok(())
    }

    fn preflight(&self, staged: &Path, version: &str) -> anyhow::Result<()> {
        let core = staged.join(CORE_EXECUTABLE_NAME);
        if self.scope == Scope::Machine {
            let version_output =
                super::super::run_command(&core, staged, &[OsStr::new("--version")])?;
            ensure!(
                std::str::from_utf8(&version_output)?.trim_end() == format!("alcomd {version}"),
                "the staged machine Core reported a different version"
            );
        } else {
            super::super::run_command(&core, staged, &[OsStr::new("check-installation-data")])
                .context("the staged Windows update could not read the current data layout")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    CurrentUser,
    Machine,
}

impl Scope {
    fn setup_arguments<const N: usize>(self, options: [&str; N]) -> Vec<&OsStr> {
        let scope: &[&str] = match self {
            Self::CurrentUser => &["/CURRENTUSER"],
            Self::Machine => &["/ALLUSERS", "/ALCOMDScope=all-users"],
        };
        scope
            .iter()
            .copied()
            .chain(["/VERYSILENT", "/SUPPRESSMSGBOXES", "/SP-", "/NORESTART"])
            .chain(options)
            .map(OsStr::new)
            .collect()
    }

    fn validate_parent(self, path: &Path) -> io::Result<()> {
        match self {
            Self::CurrentUser => validate_installation_record_parent(path),
            Self::Machine => validate_windows_machine_installation_record_parent(path),
        }
    }

    fn open_record(self, path: &Path) -> io::Result<File> {
        match self {
            Self::CurrentUser => open_owned_installation_record(path),
            Self::Machine => open_windows_machine_installation_record(path),
        }
    }

    fn read_record(self, path: &Path, limit: usize) -> anyhow::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        self.open_record(path)?
            .take(u64::try_from(limit)? + 1)
            .read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= limit, "Windows update record is too large");
        Ok(bytes)
    }

    fn create_directory(self, path: &Path) -> io::Result<()> {
        match self {
            Self::CurrentUser => fs::create_dir(path),
            Self::Machine => create_windows_machine_installation_directory(path),
        }
    }

    fn new_record(self, path: &Path) -> io::Result<File> {
        match self {
            Self::CurrentUser => {
                self.validate_parent(path)?;
                OpenOptions::new().create_new(true).write(true).open(path)
            }
            Self::Machine => create_windows_machine_installation_record(path),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Acceptance {
    receipt: AcceptedUpdate,
    accepted_at: i64,
}

struct Intent {
    previous_version: String,
    previous_digest: String,
    target_version: String,
    target_digest: String,
}

impl Intent {
    fn encode(&self) -> Vec<u8> {
        format!(
            "{}|{}|{}|{}\n",
            self.previous_version, self.previous_digest, self.target_version, self.target_digest,
        )
        .into_bytes()
    }

    fn parse(bytes: &[u8]) -> anyhow::Result<Self> {
        let text = std::str::from_utf8(bytes)?;
        let mut fields = text
            .strip_suffix('\n')
            .context("invalid Windows update intent")?
            .split('|');
        let mut version = || -> anyhow::Result<(String, String)> {
            let version = fields.next().context("missing Windows update version")?;
            let digest = fields.next().context("missing Windows update digest")?;
            ensure!(
                (1..=96).contains(&version.len())
                    && version.as_bytes()[0].is_ascii_digit()
                    && version
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b".-+".contains(&b))
                    && digest.len() == 64
                    && digest
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "invalid Windows update intent"
            );
            Ok((version.to_owned(), digest.to_owned()))
        };
        let (previous_version, previous_digest) = version()?;
        let (target_version, target_digest) = version()?;
        ensure!(
            fields.next().is_none(),
            "unexpected Windows update intent data"
        );
        Ok(Self {
            previous_version,
            previous_digest,
            target_version,
            target_digest,
        })
    }
}

fn accept_update(
    bytes: &[u8],
    asset: &Path,
    replay_directory: &Path,
    channel: Channel,
) -> anyhow::Result<(VerifiedUpdate, i64)> {
    let (update, time) = accept_windows_product_update(
        bytes,
        asset,
        replay_directory,
        channel,
        &INSTALLED_VERSION.parse()?,
    )?;
    check_asset_name(asset, &update)?;
    Ok((update, time))
}

fn check_asset_name(asset: &Path, update: &VerifiedUpdate) -> anyhow::Result<()> {
    ensure!(
        asset.file_name().and_then(OsStr::to_str) == Some(update.asset().name()),
        "product update asset needs its signed installer filename"
    );
    Ok(())
}

fn transaction_path(staged: &Path) -> anyhow::Result<PathBuf> {
    let mut name = staged
        .file_name()
        .context("Windows update stage needs a name")?
        .to_os_string();
    name.push(".transaction");
    Ok(staged.with_file_name(name))
}

fn retain_transaction(
    scope: Scope,
    transaction: &Path,
    bytes: &[u8],
    asset: &mut impl Read,
    update: &VerifiedUpdate,
    acceptance: Acceptance,
) -> anyhow::Result<()> {
    if !exists(transaction)? {
        let parent = transaction
            .parent()
            .context("Windows update transaction parent")?;
        let temporary = parent.join(format!(".update-retain-{}", Uuid::new_v4()));
        scope.create_directory(&temporary)?;
        let installer = temporary.join(update.asset().name());
        let envelope = temporary.join(ENVELOPE);
        let receipt = temporary.join(ACCEPTANCE);
        let result = (|| -> anyhow::Result<()> {
            let mut copy = scope.new_record(&installer)?;
            ensure!(
                io::copy(&mut asset.take(update.asset().bytes() + 1), &mut copy)?
                    == update.asset().bytes(),
                "product update installer length changed"
            );
            copy.sync_all()?;
            drop(copy);
            update.asset().verify_file(&installer)?;
            write_record(scope, &envelope, bytes)?;
            write_record(scope, &receipt, &serde_json::to_vec(&acceptance)?)?;
            sync_directory(&temporary)?;
            publish_path_no_replace(&temporary, transaction)?;
            sync_directory(parent)?;
            Ok(())
        })();
        if result.is_err() {
            for path in [installer, envelope, receipt] {
                let _ = fs::remove_file(path);
            }
            let _ = fs::remove_dir(&temporary);
        }
        result?;
    }
    let retained = retained_update(scope, transaction)?;
    ensure!(
        retained.acceptance() == acceptance.receipt,
        "Windows update staging already holds a different accepted update"
    );
    let installer = transaction.join(retained.asset().name());
    let _installer = scope.open_record(&installer)?;
    retained.asset().verify_file(&installer)?;
    Ok(())
}

fn retained_update(scope: Scope, transaction: &Path) -> anyhow::Result<VerifiedUpdate> {
    let acceptance: Acceptance = serde_json::from_slice(
        &scope.read_record(&transaction.join(ACCEPTANCE), PRIVATE_RECORD_LIMIT)?,
    )?;
    reverify_interrupted_windows_update(
        &scope.read_record(&transaction.join(ENVELOPE), MAX_UPDATE_RESPONSE_BYTES)?,
        acceptance.receipt,
        acceptance.accepted_at,
        &INSTALLED_VERSION.parse()?,
    )
}

fn verify_runtime(path: &Path, version: &str) -> anyhow::Result<VerifiedRuntime> {
    let (path, _) = resolve_plain_storage_directory(path)?;
    verify_installed_release(&path, Target::Windows, &version.parse()?)
        .context("Windows product payload did not verify")
}

pub(super) fn check_machine_runtime(path: &Path, runtime: &VerifiedRuntime) -> anyhow::Result<()> {
    for name in runtime.files().chain([MANIFEST_NAME, SIGNATURE_NAME]) {
        open_windows_machine_installation_record(&path.join(name))?;
    }
    Ok(())
}

pub(super) fn verify_stable_cli(root: &Path, runtime: &VerifiedRuntime) -> anyhow::Result<()> {
    let bin = root.join("bin");
    runtime.verify_copy(
        WINDOWS_CLI_EXECUTABLE_NAME,
        &bin.join(WINDOWS_CLI_EXECUTABLE_NAME),
    )?;
    runtime.verify_copy(WINDOWS_CRT_FILE, &bin.join(WINDOWS_CRT_FILE))?;
    Ok(())
}

fn ensure_absent_intent(paths: &Installation) -> anyhow::Result<()> {
    ensure!(
        !exists(&paths.intent())?,
        "a Windows update was committed; use resume-installed-update"
    );
    Ok(())
}

fn exists(path: &Path) -> io::Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn receipt_matches(scope: Scope, path: &Path, expected: &[u8]) -> anyhow::Result<bool> {
    if !exists(path)? {
        return Ok(false);
    }
    ensure!(
        scope.read_record(path, PRIVATE_RECORD_LIMIT)? == expected,
        "Windows setup completion receipt changed"
    );
    Ok(true)
}

fn write_record(scope: Scope, path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = scope.new_record(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn publish_record(scope: Scope, path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let temporary = path.with_extension(format!("partial-{}", Uuid::new_v4()));
    let mut record = scope.new_record(&temporary)?;
    let result = (|| -> anyhow::Result<()> {
        record.write_all(bytes)?;
        record.sync_all()?;
        drop(record);
        ensure!(
            scope.read_record(&temporary, PRIVATE_RECORD_LIMIT)? == bytes,
            "Windows update intent changed before publication"
        );
        publish_path_no_replace(&temporary, path)?;
        sync_directory(path.parent().context("Windows update record parent")?)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intent_requires_exact_fields_and_safe_version_components() {
        let text = format!(
            "4.0.0-alpha.0|{}|4.0.0-alpha.1|{}\n",
            "a".repeat(64),
            "b".repeat(64)
        );
        assert_eq!(
            Intent::parse(text.as_bytes())
                .expect("valid intent")
                .encode(),
            text.as_bytes()
        );
        for malformed in [
            text.replace("-alpha.0", "/../x"),
            text.replace("4.0.0-alpha.1", ".."),
            text.replace('b', "B"),
            text.replace('\n', "\r\n"),
            text.replace('\n', "|extra\n"),
            text.trim_end().to_owned(),
        ] {
            assert!(Intent::parse(malformed.as_bytes()).is_err());
        }
    }
}
