use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use alcomd_platform::{
    CORE_EXECUTABLE_NAME, DaemonInstance, DataConfig, IpcConfig, WINDOWS_BOOTSTRAP_EXECUTABLE_NAME,
    WINDOWS_CLI_EXECUTABLE_NAME, WINDOWS_UPDATE_INTENT_RECORD, WindowsInstallationUpdate,
    check_windows_core_registration, hold_windows_installation, open_owned_installation_record,
    publish_path_no_replace, resolve_plain_storage_directory, sync_directory,
    validate_installation_record_parent, windows_current_user_installation_root,
    windows_installation_runtime,
};
use alcomd_updater::{
    Channel,
    distribution::{
        MANIFEST_NAME, Target, VerifiedRuntime, WINDOWS_CRT_FILE, WINDOWS_UPDATE_STAGE_PREFIX,
        format_digest, verify_installed_release,
    },
    product_update::{
        AcceptedUpdate, MAX_UPDATE_RESPONSE_BYTES, VerifiedUpdate, accept_windows_product_update,
        reverify_interrupted_windows_update,
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

/// Extracts a verified update and checks the user's data layout without opening it for writes.
pub fn stage_installed_update(
    envelope: &Path,
    asset: &Path,
    replay_directory: &Path,
    channel: Channel,
) -> anyhow::Result<PathBuf> {
    let paths = Installation::current()?;
    let _installation = hold_windows_installation(&paths.executable)
        .context("the current installation is undergoing maintenance")?
        .context("a current-user Windows installation is required")?;
    verify_runtime(&paths.runtime, INSTALLED_VERSION)?;
    let _asset = open_owned_installation_record(asset)?;
    let bytes = read_record(envelope, MAX_UPDATE_RESPONSE_BYTES)?;
    let (update, _) = accept_update(&bytes, asset, replay_directory, channel)?;
    let _instance = DaemonInstance::acquire(&IpcConfig::default())
        .context("close the current user's Core before staging the update")?;
    paths.prepare(&update, asset)
}

/// Verifies and installs a newer product in the existing current-user scope.
pub fn apply_installed_update(
    envelope: &Path,
    asset: &Path,
    replay_directory: &Path,
    channel: Channel,
) -> anyhow::Result<()> {
    let paths = Installation::current()?;
    ensure_absent_intent(&paths)?;
    let previous = verify_runtime(&paths.runtime, INSTALLED_VERSION)?;
    let mut asset_handle = open_owned_installation_record(asset)?;
    let bytes = read_record(envelope, MAX_UPDATE_RESPONSE_BYTES)?;
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
        &transaction,
        &bytes,
        &mut asset_handle,
        &update,
        Acceptance {
            receipt: update.acceptance(),
            accepted_at,
        },
    )?;
    publish_record(&paths.intent(), &intent.encode())?;
    finish_update(&paths, maintenance, &intent)
}

/// Continues the exact committed Windows update using the retained signed inputs.
pub fn resume_installed_update() -> anyhow::Result<()> {
    let paths = Installation::current()?;
    let intent = Intent::parse(&read_record(&paths.intent(), PRIVATE_RECORD_LIMIT)?)?;
    if INSTALLED_VERSION == intent.target_version {
        let previous = paths.root.join("runtime").join(&intent.previous_version);
        let runtime = verify_runtime(&previous, &intent.previous_version)?;
        ensure!(
            format_digest(&runtime.manifest_sha256) == intent.previous_digest,
            "the previous Windows runtime does not match the update intent"
        );
        super::super::run_command(
            &previous.join(WINDOWS_BOOTSTRAP_EXECUTABLE_NAME),
            &previous,
            &[OsStr::new("resume-installed-update")],
        )
        .context("the previous Windows Bootstrap could not resume the update")?;
        return Ok(());
    }
    ensure!(
        INSTALLED_VERSION == intent.previous_version,
        "this Windows Bootstrap does not belong to the interrupted update"
    );
    let maintenance = WindowsInstallationUpdate::acquire(&paths.executable, &intent.target_version)
        .context("close the current user's Core before resuming the update")?;
    finish_update(&paths, maintenance, &intent)
}

fn finish_update(
    paths: &Installation,
    mut maintenance: WindowsInstallationUpdate,
    intent: &Intent,
) -> anyhow::Result<()> {
    let encoded_intent = intent.encode();
    ensure!(
        read_record(&paths.intent(), PRIVATE_RECORD_LIMIT)? == encoded_intent,
        "Windows update intent changed"
    );
    let previous = verify_runtime(&paths.runtime, INSTALLED_VERSION)?;
    ensure!(
        format_digest(&previous.manifest_sha256) == intent.previous_digest,
        "the previous Windows runtime does not match the update intent"
    );
    let staged = paths.stage_path(&intent.target_digest)?;
    let transaction = transaction_path(&staged)?;
    let update = retained_update(&transaction)?;
    ensure!(
        update.version().to_string() == intent.target_version
            && format_digest(&update.payload_manifest_sha256()) == intent.target_digest,
        "the retained Windows update does not match the update intent"
    );
    let asset = transaction.join(update.asset().name());
    let _asset = open_owned_installation_record(&asset)?;
    update.asset().verify_file(&asset)?;
    verify_staged(&staged, &update)?;
    preflight(&staged)?;

    let receipt = transaction.join(INSTALLED_RECEIPT);
    let expected_receipt = format!("{}\n", intent.target_digest);
    if !receipt_matches(&receipt, expected_receipt.as_bytes())? {
        let digest = format!("/ALCOMDCommit={}", intent.target_digest);
        let version = format!("/ALCOMDPrevious={}", intent.previous_version);
        let previous = format!("/ALCOMDPreviousDigest={}", intent.previous_digest);
        super::super::run_command(
            &asset,
            &paths.runtime,
            &setup_arguments([&digest, &version, &previous]),
        )
        .context("Windows setup did not finish; resume the same committed update")?;
        ensure!(
            receipt_matches(&receipt, expected_receipt.as_bytes())?,
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
    maintenance.publish_core_registration(&DataConfig::default())?;
    maintenance.activate()?;
    maintenance.finish(&encoded_intent)?;
    Ok(())
}

struct Installation {
    executable: PathBuf,
    runtime: PathBuf,
    root: PathBuf,
}

impl Installation {
    fn current() -> anyhow::Result<Self> {
        let executable = std::env::current_exe()?;
        let runtime = windows_installation_runtime(&executable)?;
        let root = windows_current_user_installation_root()
            .context("a current-user Windows installation is required")?;
        ensure!(
            runtime.parent().and_then(Path::parent) == Some(root.as_path()),
            "a current-user Windows installation is required"
        );
        Ok(Self {
            executable,
            runtime,
            root,
        })
    }

    fn intent(&self) -> PathBuf {
        self.root.join(WINDOWS_UPDATE_INTENT_RECORD)
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
                &setup_arguments([&request]),
                1,
            )
            .context("could not extract the Windows product update")?;
        }
        verify_staged(&staged, update)?;
        sync_directory(staged.parent().context("Windows update staging parent")?)?;
        preflight(&staged)?;
        Ok(staged)
    }
}

fn setup_arguments<const N: usize>(options: [&str; N]) -> Vec<&OsStr> {
    [
        "/CURRENTUSER",
        "/VERYSILENT",
        "/SUPPRESSMSGBOXES",
        "/SP-",
        "/NORESTART",
    ]
    .into_iter()
    .chain(options)
    .map(OsStr::new)
    .collect()
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
    ensure!(
        asset.file_name().and_then(OsStr::to_str) == Some(update.asset().name()),
        "product update asset needs its signed installer filename"
    );
    Ok((update, time))
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
    transaction: &Path,
    bytes: &[u8],
    asset: &mut File,
    update: &VerifiedUpdate,
    acceptance: Acceptance,
) -> anyhow::Result<()> {
    if !exists(transaction)? {
        let parent = transaction
            .parent()
            .context("Windows update transaction parent")?;
        let temporary = parent.join(format!(".update-retain-{}", Uuid::new_v4()));
        fs::create_dir(&temporary)?;
        let installer = temporary.join(update.asset().name());
        let envelope = temporary.join(ENVELOPE);
        let receipt = temporary.join(ACCEPTANCE);
        let result = (|| -> anyhow::Result<()> {
            let mut copy = new_record(&installer)?;
            ensure!(
                io::copy(&mut asset.take(update.asset().bytes() + 1), &mut copy)?
                    == update.asset().bytes(),
                "product update installer length changed"
            );
            copy.sync_all()?;
            drop(copy);
            update.asset().verify_file(&installer)?;
            write_record(&envelope, bytes)?;
            write_record(&receipt, &serde_json::to_vec(&acceptance)?)?;
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
    let retained = retained_update(transaction)?;
    ensure!(
        retained.acceptance() == acceptance.receipt,
        "Windows update staging already holds a different accepted update"
    );
    let installer = transaction.join(retained.asset().name());
    let _installer = open_owned_installation_record(&installer)?;
    retained.asset().verify_file(&installer)?;
    Ok(())
}

fn retained_update(transaction: &Path) -> anyhow::Result<VerifiedUpdate> {
    let acceptance: Acceptance = serde_json::from_slice(&read_record(
        &transaction.join(ACCEPTANCE),
        PRIVATE_RECORD_LIMIT,
    )?)?;
    reverify_interrupted_windows_update(
        &read_record(&transaction.join(ENVELOPE), MAX_UPDATE_RESPONSE_BYTES)?,
        acceptance.receipt,
        acceptance.accepted_at,
        &INSTALLED_VERSION.parse()?,
    )
}

fn preflight(staged: &Path) -> anyhow::Result<()> {
    super::super::run_command(
        &staged.join(CORE_EXECUTABLE_NAME),
        staged,
        &[OsStr::new("check-installation-data")],
    )
    .context("the staged Windows update could not read the current data layout")?;
    Ok(())
}

fn verify_staged(staged: &Path, update: &VerifiedUpdate) -> anyhow::Result<()> {
    validate_installation_record_parent(&staged.join(MANIFEST_NAME))?;
    let verified = verify_runtime(staged, &update.version().to_string())?;
    ensure!(
        verified.manifest_sha256 == update.payload_manifest_sha256(),
        "staged Windows update has a different payload digest"
    );
    Ok(())
}

fn verify_runtime(path: &Path, version: &str) -> anyhow::Result<VerifiedRuntime> {
    let (path, _) = resolve_plain_storage_directory(path)?;
    verify_installed_release(&path, Target::Windows, &version.parse()?)
        .context("Windows product payload did not verify")
}

fn verify_stable_cli(root: &Path, runtime: &VerifiedRuntime) -> anyhow::Result<()> {
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

fn receipt_matches(path: &Path, expected: &[u8]) -> anyhow::Result<bool> {
    if !exists(path)? {
        return Ok(false);
    }
    ensure!(
        read_record(path, PRIVATE_RECORD_LIMIT)? == expected,
        "Windows setup completion receipt changed"
    );
    Ok(true)
}

fn read_record(path: &Path, limit: usize) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    open_owned_installation_record(path)?
        .take(u64::try_from(limit)? + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= limit, "Windows update record is too large");
    Ok(bytes)
}

fn new_record(path: &Path) -> io::Result<File> {
    validate_installation_record_parent(path)?;
    OpenOptions::new().create_new(true).write(true).open(path)
}

fn write_record(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = new_record(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn publish_record(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let temporary = path.with_extension(format!("partial-{}", Uuid::new_v4()));
    let mut record = new_record(&temporary)?;
    let result = (|| -> anyhow::Result<()> {
        record.write_all(bytes)?;
        record.sync_all()?;
        drop(record);
        ensure!(
            read_record(&temporary, PRIVATE_RECORD_LIMIT)? == bytes,
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
