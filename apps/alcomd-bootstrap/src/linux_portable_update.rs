use std::ffi::{OsStr, OsString};
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{FileExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use alcomd_platform::{
    BindError, CORE_EXECUTABLE_NAME, DaemonInstance, DataConfig, IpcConfig,
    LINUX_BOOTSTRAP_EXECUTABLE_NAME, PORTABLE_ORIGIN_FILE, PortableImage, PortableOrigin,
    check_linux_core_registration, check_linux_portable_update_registration,
    linux_filesystem_is_full, linux_portable_runtime, linux_portable_runtime_for_version,
    open_owned_installation_record, open_plain_file, publish_linux_portable_update_registration,
    publish_path_no_replace, sync_directory,
};
use alcomd_updater::{
    Channel,
    distribution::{
        Target, VerifiedRuntime, format_digest, verify_installed_release, verify_portable_runtime,
    },
    hash_exact,
    product_update::{
        AcceptedUpdate, MAX_UPDATE_RESPONSE_BYTES, VerifiedUpdate, accept_portable_product_update,
        record_desktop_update_failure, reverify_interrupted_portable_update,
    },
};
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{RuntimeStage, copy_runtime, private_directory, runtime_in_image, write_origin};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const INTENT_FILE: &str = "appimage-update-intent-v1.json";
const INTENT_LIMIT: u64 = 2 * MAX_UPDATE_RESPONSE_BYTES as u64 + 32 * 1024;
const STAGED_IMAGE: &str = "image.AppImage";
const DESKTOP_IDLE_WAIT: Duration = Duration::from_secs(10);

#[derive(Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Intent {
    schema: u32,
    transaction: Uuid,
    previous_version: String,
    previous_origin: PortableOrigin,
    candidate_origin: PortableOrigin,
    envelope: String,
    acceptance: AcceptedUpdate,
    accepted_at: i64,
}

/// Replaces an initialized user's AppImage using a signed product declaration and asset.
pub fn apply_portable_update(
    envelope: &Path,
    asset: &Path,
    replay_directory: &Path,
    channel: Channel,
    desktop: bool,
) -> anyhow::Result<()> {
    let data = DataConfig::default();
    let result = apply_update(&data, envelope, asset, replay_directory, channel, desktop);
    finish_desktop(&data, result, desktop)
}

fn apply_update(
    data: &DataConfig,
    envelope: &Path,
    asset: &Path,
    replay_directory: &Path,
    channel: Channel,
    wait_for_core: bool,
) -> anyhow::Result<PathBuf> {
    let _lock = lock_core(wait_for_core)?;
    let (installed, verified) = own_installed_runtime(data)?;
    ensure!(
        check_linux_core_registration(data, &installed)?,
        "this account has not initialized this AppImage"
    );
    let root = installed
        .parent()
        .context("portable runtime root is missing")?;
    let intent_path = root.join(INTENT_FILE);
    ensure!(
        read_intent(&intent_path)?.is_none(),
        "an AppImage update requires recovery"
    );
    let previous_origin = PortableOrigin::read(&installed)?;
    previous_origin.revalidate()?;
    let image = PortableImage::open_for_update(previous_origin.image_path())?;
    require_digest(&image, previous_origin.image_sha256())?;
    {
        let (_stage, _, extracted) = extract_image(root, &image, VERSION)?;
        ensure!(
            extracted.manifest_sha256 == verified.manifest_sha256,
            "the original AppImage does not contain this installed release"
        );
    }

    let envelope = read_envelope(envelope)?;
    let (update, accepted_at) = accept_portable_product_update(
        envelope.as_bytes(),
        asset,
        replay_directory,
        channel,
        &VERSION.parse()?,
    )?;
    let transaction = Uuid::new_v4();
    let parent = image
        .path()
        .parent()
        .context("AppImage parent is missing")?;
    let mut stage = ImageStage::new(parent, transaction)?;
    let candidate = stage.copy_asset(asset, &update, image.file().metadata()?.permissions())?;
    let candidate_origin = PortableOrigin::bind(
        &candidate,
        &format_digest(&update.asset().sha256()),
        &format_digest(&update.payload_manifest_sha256()),
    )?;
    let selected = linux_portable_runtime_for_version(data, &update.version().to_string())?;
    match fs::symlink_metadata(&selected) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
        Ok(_) => anyhow::bail!("update target runtime already exists"),
    }
    let prepared = prepare_runtime(root, &candidate, &update)?;
    let intent = Intent {
        schema: 1,
        transaction,
        previous_version: VERSION.to_owned(),
        previous_origin,
        candidate_origin,
        envelope,
        acceptance: update.acceptance(),
        accepted_at,
    };
    publish_intent(&intent_path, &intent, &mut stage)?;
    match finish_update(data, &intent_path, &intent, &verified, Some(prepared)) {
        Ok(path) => Ok(path),
        Err(error) => {
            rollback_before_exchange(data, &intent_path, &intent, &verified)?;
            Err(error)
        }
    }
}

/// Recovers an accepted update using only the journal and the two retained images.
pub fn resume_portable_update(desktop: bool) -> anyhow::Result<()> {
    let data = DataConfig::default();
    let result = resume_update(&data, desktop);
    finish_desktop(&data, result, desktop)
}

fn resume_update(data: &DataConfig, wait_for_core: bool) -> anyhow::Result<PathBuf> {
    let _lock = lock_core(wait_for_core)?;
    let (installed, verified) = own_installed_runtime(data)?;
    let intent_path = installed.with_file_name(INTENT_FILE);
    let intent = read_intent(&intent_path)?.context("no interrupted AppImage update exists")?;
    match finish_update(data, &intent_path, &intent, &verified, None) {
        Ok(path) => Ok(path),
        Err(error) => {
            rollback_before_exchange(data, &intent_path, &intent, &verified)?;
            Err(error)
        }
    }
}

fn finish_desktop(
    data: &DataConfig,
    result: anyhow::Result<PathBuf>,
    desktop: bool,
) -> anyhow::Result<()> {
    if !desktop {
        return result.map(drop);
    }
    match result {
        Ok(path) => {
            let _ = record_desktop_update_failure(false);
            let result = relaunch_image(&path, &[]);
            if result.is_err() {
                let _ = record_desktop_update_failure(true);
            }
            result
        }
        Err(error) => {
            if let Ok((runtime, _)) = own_installed_runtime(data) {
                let _ = record_desktop_update_failure(true);
                if let Ok(path) = restartable_previous_image(data, &runtime) {
                    eprintln!("AppImage update did not complete; reopening the installed release");
                    return relaunch_image(&path, &[]);
                }
            }
            Err(error)
        }
    }
}

fn restartable_previous_image(data: &DataConfig, runtime: &Path) -> anyhow::Result<PathBuf> {
    ensure!(
        read_intent(&runtime.with_file_name(INTENT_FILE))?.is_none()
            && check_linux_core_registration(data, runtime)?,
        "AppImage update requires recovery"
    );
    let origin = PortableOrigin::read(runtime)?;
    origin.revalidate()?;
    Ok(origin.image_path().to_path_buf())
}

/// A normal launch from the recorded image path can complete a pending replacement.
pub(super) fn resume_from_image(
    data: &DataConfig,
    image: &PortableImage,
    verified: &VerifiedRuntime,
) -> anyhow::Result<bool> {
    let intent_path = linux_portable_runtime(data)?.with_file_name(INTENT_FILE);
    if read_intent(&intent_path)?.is_none() {
        return Ok(false);
    }
    let _lock = lock_core(false)?;
    let Some(intent) = read_intent(&intent_path)? else {
        return Ok(false);
    };
    ensure!(
        image.path() == intent.previous_origin.image_path(),
        "launch the AppImage from its original path to recover the update"
    );
    match finish_update(data, &intent_path, &intent, verified, None) {
        Ok(_) => {
            let _ = record_desktop_update_failure(false);
            Ok(true)
        }
        Err(_) if rollback_before_exchange(data, &intent_path, &intent, verified)? => {
            let _ = record_desktop_update_failure(true);
            eprintln!("AppImage update could not be applied; the previous release was kept");
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

pub(super) fn relaunch_image(path: &Path, arguments: &[OsString]) -> anyhow::Result<()> {
    Err(Command::new(path)
        .args(arguments)
        .env_remove("ALCOMD_MCP_CLIENT_CREDENTIAL")
        .env_remove("APPDIR")
        .env_remove("APPIMAGE")
        .env_remove("ARGV0")
        .env_remove("OWD")
        .exec())
    .context("updated AppImage could not start")
}

fn lock_core(wait: bool) -> anyhow::Result<DaemonInstance> {
    let start = Instant::now();
    loop {
        match DaemonInstance::acquire(&IpcConfig::default()) {
            Ok(instance) => return Ok(instance),
            Err(BindError::AlreadyRunning) if wait && start.elapsed() < DESKTOP_IDLE_WAIT => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => {
                return Err(error)
                    .context("close this account's Core before updating the AppImage");
            }
        }
    }
}

fn own_installed_runtime(data: &DataConfig) -> anyhow::Result<(PathBuf, VerifiedRuntime)> {
    let runtime = linux_portable_runtime(data)?;
    ensure!(
        std::env::current_exe()? == runtime.join(LINUX_BOOTSTRAP_EXECUTABLE_NAME),
        "this Bootstrap does not belong to an installed AppImage"
    );
    let verified = verify_portable_runtime(&runtime, &VERSION.parse()?)?;
    Ok((runtime, verified))
}

fn read_envelope(path: &Path) -> anyhow::Result<String> {
    let (file, _) = open_plain_file(path)?;
    let mut bytes = Vec::new();
    file.take(MAX_UPDATE_RESPONSE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= MAX_UPDATE_RESPONSE_BYTES,
        "update envelope is too large"
    );
    String::from_utf8(bytes).context("update envelope must be UTF-8")
}

fn read_intent(path: &Path) -> anyhow::Result<Option<Intent>> {
    let file = match open_owned_installation_record(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.take(INTENT_LIMIT + 1).read_to_end(&mut bytes)?;
    ensure!(
        u64::try_from(bytes.len())? <= INTENT_LIMIT,
        "AppImage update journal is too large"
    );
    let intent: Intent =
        serde_json::from_slice(&bytes).context("invalid AppImage update journal")?;
    ensure!(
        intent.schema == 1 && intent.envelope.len() <= MAX_UPDATE_RESPONSE_BYTES,
        "invalid AppImage update journal"
    );
    Ok(Some(intent))
}

fn publish_intent(
    path: &Path,
    intent: &Intent,
    image_stage: &mut ImageStage,
) -> anyhow::Result<()> {
    let parent = path.parent().context("AppImage journal root is missing")?;
    let stage = RuntimeStage::new(parent)?;
    let record = stage.0.join(INTENT_FILE);
    let bytes = serde_json::to_vec(intent)?;
    ensure!(
        u64::try_from(bytes.len())? <= INTENT_LIMIT,
        "AppImage update journal is too large"
    );
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&record)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    sync_directory(&stage.0)?;
    publish_path_no_replace(&record, path)?;
    image_stage.retained = true;
    sync_directory(parent)?;
    Ok(())
}

fn finish_update(
    data: &DataConfig,
    intent_path: &Path,
    intent: &Intent,
    entry: &VerifiedRuntime,
    prepared: Option<RuntimeStage>,
) -> anyhow::Result<PathBuf> {
    let previous_version = intent.previous_version.parse()?;
    let update = reverify_interrupted_portable_update(
        intent.envelope.as_bytes(),
        intent.acceptance,
        intent.accepted_at,
        &previous_version,
    )?;
    let selected_version = update.version().to_string();
    let previous = linux_portable_runtime_for_version(data, &intent.previous_version)?;
    let selected = linux_portable_runtime_for_version(data, &selected_version)?;
    let selected_registration =
        check_linux_portable_update_registration(data, &previous, &selected)?;
    verify_portable_runtime(&previous, &previous_version)?;
    ensure!(
        PortableOrigin::read(&previous)? == intent.previous_origin,
        "previous AppImage origin changed during update"
    );
    let image_path = intent.previous_origin.image_path();
    let image_parent = image_path.parent().context("AppImage parent is missing")?;
    let candidate_path = image_stage_directory(image_parent, intent.transaction).join(STAGED_IMAGE);
    let payload_digest = format_digest(&update.payload_manifest_sha256());
    let candidate_hash = format_digest(&update.asset().sha256());
    let entry_hash = format_digest(&entry.manifest_sha256);
    ensure!(
        intent.candidate_origin.image_path() == candidate_path
            && intent.candidate_origin.image_sha256() == candidate_hash
            && intent.candidate_origin.payload_manifest_sha256() == payload_digest
            && ((VERSION == intent.previous_version
                && entry_hash == intent.previous_origin.payload_manifest_sha256())
                || (VERSION == selected_version && entry_hash == payload_digest)),
        "AppImage update journal does not belong to this signed release"
    );

    let image = PortableImage::open_for_update(image_path)?;
    let staged_image = PortableImage::open_for_update(&candidate_path)?;
    let awaiting_exchange = intent.previous_origin.identifies_image(&image)
        && intent.candidate_origin.identifies_image(&staged_image);
    let candidate = if awaiting_exchange {
        ensure!(
            !selected_registration,
            "AppImage registration changed before replacement"
        );
        intent.previous_origin.revalidate()?;
        intent.candidate_origin.revalidate()?;
        require_digest(&image, intent.previous_origin.image_sha256())?;
        &staged_image
    } else {
        ensure!(
            intent.candidate_origin.identifies_image(&image)
                && intent.previous_origin.identifies_image(&staged_image),
            "retained AppImages changed during update"
        );
        require_digest(&staged_image, intent.previous_origin.image_sha256())?;
        &image
    };
    ensure!(
        candidate.size() == update.asset().bytes(),
        "update AppImage size changed"
    );
    require_digest(candidate, &candidate_hash)?;

    let root = selected
        .parent()
        .context("portable runtime root is missing")?;
    let prepared = match fs::symlink_metadata(&selected) {
        Ok(_) => {
            ensure!(!awaiting_exchange, "update target runtime already exists");
            verify_selected_runtime(&selected, &update)?;
            None
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            ensure!(
                !selected_registration,
                "registered update runtime is missing"
            );
            Some(match prepared {
                Some(prepared) => prepared,
                None => prepare_runtime(root, candidate, &update)?,
            })
        }
        Err(error) => return Err(error.into()),
    };
    if awaiting_exchange {
        image.exchange_for_update(&staged_image)?;
    }
    let active = PortableImage::open_for_update(image_path)?;
    ensure!(
        intent.candidate_origin.identifies_image(&active),
        "updated AppImage identity changed"
    );
    let origin = PortableOrigin::bind(&active, &candidate_hash, &payload_digest)?;
    if let Some(prepared) = prepared {
        let runtime = prepared.0.join("runtime");
        write_origin(&runtime.join(PORTABLE_ORIGIN_FILE), &origin)?;
        verify_selected_runtime(&runtime, &update)?;
        sync_directory(&runtime)?;
        sync_directory(&prepared.0)?;
        publish_path_no_replace(&runtime, &selected)?;
        sync_directory(root)?;
    } else {
        ensure!(
            PortableOrigin::read(&selected)? == origin,
            "updated runtime origin changed"
        );
    }
    origin.revalidate()?;
    publish_linux_portable_update_registration(data, &previous, &selected)?;
    clear_intent(intent_path, intent)?;
    Ok(image_path.to_path_buf())
}

fn clear_intent(path: &Path, intent: &Intent) -> anyhow::Result<()> {
    ensure!(
        read_intent(path)?.as_ref() == Some(intent),
        "AppImage update journal changed"
    );
    fs::remove_file(path)?;
    sync_directory(path.parent().context("AppImage journal root is missing")?)?;
    Ok(())
}

fn rollback_before_exchange(
    data: &DataConfig,
    intent_path: &Path,
    intent: &Intent,
    entry: &VerifiedRuntime,
) -> anyhow::Result<bool> {
    let source = linux_portable_runtime(data)?;
    if VERSION != intent.previous_version
        || format_digest(&entry.manifest_sha256) != intent.previous_origin.payload_manifest_sha256()
        || intent.previous_origin.revalidate().is_err()
        || !check_linux_core_registration(data, &source)?
        || PortableOrigin::read(&source)? != intent.previous_origin
    {
        return Ok(false);
    }
    clear_intent(intent_path, intent)?;
    // Only remove this transaction's candidate if the original inode and metadata survived.
    let parent = intent
        .previous_origin
        .image_path()
        .parent()
        .context("AppImage parent is missing")?;
    let stage = image_stage_directory(parent, intent.transaction);
    let staged = stage.join(STAGED_IMAGE);
    if intent.candidate_origin.image_path() == staged
        && intent.candidate_origin.revalidate().is_ok()
    {
        let cleanup = || -> io::Result<()> {
            fs::remove_file(&staged)?;
            sync_directory(&stage)?;
            fs::remove_dir(&stage)?;
            sync_directory(parent)
        };
        if cleanup().is_err() {
            eprintln!("AppImage update staging could not be fully removed");
        }
    }
    Ok(true)
}

fn prepare_runtime(
    root: &Path,
    candidate: &PortableImage,
    update: &VerifiedUpdate,
) -> anyhow::Result<RuntimeStage> {
    let (stage, extracted, verified) =
        extract_image(root, candidate, &update.version().to_string())?;
    ensure!(
        verified.manifest_sha256 == update.payload_manifest_sha256(),
        "AppImage payload differs from its signed update declaration"
    );
    super::super::run_command(
        &extracted.join(CORE_EXECUTABLE_NAME),
        &extracted,
        &[OsStr::new("check-installation-data")],
    )
    .context("the next AppImage Core could not read the installed data")?;
    let runtime = stage.0.join("runtime");
    private_directory(&runtime)?;
    copy_runtime(&extracted, &runtime, &verified)?;
    Ok(stage)
}

fn extract_image(
    root: &Path,
    image: &PortableImage,
    version: &str,
) -> anyhow::Result<(RuntimeStage, PathBuf, VerifiedRuntime)> {
    let mut header = [0; 20];
    image.file().read_exact_at(&mut header, 0)?;
    ensure!(
        &header[..6] == b"\x7fELF\x02\x01"
            && &header[8..11] == b"AI\x02"
            && header[18..20] == [62, 0],
        "file is not a Linux x86-64 AppImage"
    );
    let stage = RuntimeStage::new(root)?;
    let parsed_version = version.parse()?;
    let extracted = runtime_in_image(&stage.0.join("squashfs-root"), version);
    let verified =
        super::super::run_command(image.path(), &stage.0, &[OsStr::new("--appimage-extract")])
            .context("could not extract the signed AppImage")
            .and_then(|_| {
                verify_installed_release(&extracted, Target::Linux, &parsed_version)
                    .context("could not verify the extracted AppImage runtime")
            })
            .map_err(|error| {
                // The Type 2 extractor can exit successfully after an incomplete write.
                if linux_filesystem_is_full(&stage.0).unwrap_or(false) {
                    std::io::Error::from(std::io::ErrorKind::StorageFull).into()
                } else {
                    error
                }
            })
            .context("could not prepare the AppImage runtime")?;
    Ok((stage, extracted, verified))
}

fn verify_selected_runtime(runtime: &Path, update: &VerifiedUpdate) -> anyhow::Result<()> {
    let verified = verify_portable_runtime(runtime, update.version())?;
    ensure!(
        verified.manifest_sha256 == update.payload_manifest_sha256(),
        "the installed AppImage payload differs from the accepted update"
    );
    Ok(())
}

fn require_digest(image: &PortableImage, expected: &str) -> anyhow::Result<()> {
    ensure!(
        format_digest(&hash_exact(image.file().try_clone()?, image.size())?) == expected,
        "AppImage digest does not match the verified release"
    );
    Ok(())
}

fn image_stage_directory(parent: &Path, transaction: Uuid) -> PathBuf {
    parent.join(format!(".alcomd-appimage-{transaction}"))
}

struct ImageStage {
    path: PathBuf,
    retained: bool,
}

impl ImageStage {
    fn new(parent: &Path, transaction: Uuid) -> io::Result<Self> {
        let path = image_stage_directory(parent, transaction);
        private_directory(&path)?;
        Ok(Self {
            path,
            retained: false,
        })
    }

    fn copy_asset(
        &self,
        source: &Path,
        update: &VerifiedUpdate,
        permissions: fs::Permissions,
    ) -> anyhow::Result<PortableImage> {
        let path = self.path.join(STAGED_IMAGE);
        let (source, _) = open_plain_file(source)?;
        let expected_size = update.asset().bytes();
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o700)
            .open(&path)?;
        ensure!(
            io::copy(&mut source.take(expected_size + 1), &mut output)? == expected_size,
            "update AppImage size changed while staging"
        );
        output.set_permissions(fs::Permissions::from_mode(permissions.mode() & 0o777))?;
        output.sync_all()?;
        drop(output);
        sync_directory(&self.path)?;
        sync_directory(
            self.path
                .parent()
                .context("AppImage staging parent is missing")?,
        )?;
        let image = PortableImage::open_for_update(&path)?;
        require_digest(&image, &format_digest(&update.asset().sha256()))?;
        Ok(image)
    }
}

impl Drop for ImageStage {
    fn drop(&mut self) {
        if !self.retained {
            let _ = fs::remove_file(self.path.join(STAGED_IMAGE));
            let _ = fs::remove_dir(&self.path);
        }
    }
}
