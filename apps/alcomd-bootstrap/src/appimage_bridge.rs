//! Standalone entry that authenticates the downloaded AppImage before executing it.

use std::fs::{self, OpenOptions};
use std::io::{self, ErrorKind, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use alcomd_platform::{
    DataConfig, PortableImage, file_identity_key, open_plain_file, publish_path_no_replace,
    require_unprivileged_linux_installation, resolve_plain_storage_directory,
    state_database_location, sync_directory,
};
use alcomd_updater::bridge::appimage::{
    AssetPins, HEALTH_REPORT, HEALTH_SIGNATURE, MIGRATION, RELEASE,
};
use alcomd_updater::distribution::verify_appimage_bridge_release;
use alcomd_updater::{Channel, decode_hex, hash_exact, verify_staged_payload};
use anyhow::{Context, ensure};
use clap::{Args, Parser, Subcommand};

use crate::{
    EXE_LIMIT, HealthReport, InstallScope, Journal, PLAN_RECORD, Phase, Plan, latest_phase,
    linux_bridge::{V3_APPIMAGE_SHA256, migration_workspace},
    read_platform_health, rollback_v3,
};

#[derive(Parser)]
#[command(
    version,
    about = "Migrate a frozen Linux AppImage installation to ALCOMD v4"
)]
struct Arguments {
    #[command(subcommand)]
    operation: Operation,
}

#[derive(Subcommand)]
enum Operation {
    /// Verify both releases and prepare the selected account's migration for review.
    Stage(Selection),
    /// Confirm the staged migration, commit it and start the new desktop.
    Commit(Selection),
    /// Discard this account's uncommitted AppImage migration state.
    Rollback,
}

#[derive(Args)]
struct Selection {
    /// Absolute path of the v3.4.0 user data directory.
    #[arg(long)]
    source_root: PathBuf,
    /// Absolute path of the original frozen AppImage.
    #[arg(long)]
    recovery_image: PathBuf,
    /// Absolute destination of the independently staged v4 AppImage.
    #[arg(long)]
    install_image: PathBuf,
    /// Release channel of the separately authenticated archive.
    #[arg(long, value_enum, default_value = "stable")]
    channel: Channel,
    /// Store this user's repository authentication in the current account's credential vault.
    #[arg(long)]
    transfer_credentials: bool,
    /// Consent to restore Discord preferences for this exact signed extension.
    #[arg(long)]
    discord_consent_digest: Option<String>,
}

/// Dispatches the separately distributed native launcher.
pub fn run() -> anyhow::Result<()> {
    let Arguments { operation } = Arguments::parse();
    require_unprivileged_linux_installation()?;
    let database = state_database_location(&DataConfig::default())?;
    let destination = database.parent().context("missing user data directory")?;
    let workspace = migration_workspace(destination);
    let (selection, commit) = match operation {
        Operation::Stage(selection) => (selection, false),
        Operation::Commit(selection) => (selection, true),
        Operation::Rollback => {
            let (_, phase) =
                prepared_migration(&workspace)?.context("no AppImage migration to roll back")?;
            ensure!(
                !matches!(phase, Some(Phase::CommitIntent(_) | Phase::Committed)),
                "migration reached its commit point; use commit to recover"
            );
            rollback_v3(&workspace)?;
            println!("Uncommitted AppImage migration staging discarded");
            return Ok(());
        }
    };
    let pins: AssetPins = serde_json::from_str(
        option_env!("ALCOMD_APPIMAGE_BRIDGE_PINS")
            .context("this launcher was not assembled for a verified release")?,
    )?;
    ensure!(
        selection.channel == pins.channel,
        "the selected channel does not match this standalone bridge"
    );
    let executable = std::env::current_exe()?;
    let (bundle, _) = resolve_plain_storage_directory(
        executable
            .parent()
            .context("missing bridge archive directory")?,
    )?;
    let bridge = verify_staged_payload(&bundle.join(MIGRATION), selection.channel)?;
    ensure!(
        bridge.version().to_string() == env!("CARGO_PKG_VERSION")
            && bridge.manifest_sha256() == pins.migration_manifest_sha256,
        "the signed migration payload is for a different standalone bridge"
    );
    verify_appimage_bridge_release(&bundle.join(RELEASE), pins.product_manifest_sha256, &bridge)?;
    let source_image = bundle.join(&pins.name);
    pins.verify_file(&source_image)?;

    let (source_root, _) = resolve_plain_storage_directory(&selection.source_root)?;
    let (recovery, _) = crate::journal::isolated_child(&selection.recovery_image)?;
    let (install, _) = crate::journal::isolated_child(&selection.install_image)?;
    ensure!(
        install != recovery
            && ![
                bundle.as_path(),
                source_root.as_path(),
                workspace.as_path(),
                destination
            ]
            .iter()
            .any(|root| install.starts_with(root)),
        "the v4 AppImage must be staged separately from the bridge, v3 and migration data"
    );
    let original = PortableImage::open(&recovery)?;
    ensure!(
        original.size() <= EXE_LIMIT
            && hash_exact(original.file().try_clone()?, original.size())?
                == decode_hex(V3_APPIMAGE_SHA256)?,
        "the selected recovery image is not the published v3.4.0 AppImage"
    );
    PortableImage::check_destination(&install)?;

    let health = HealthReport {
        report: bundle.join(HEALTH_REPORT),
        signature: bundle.join(HEALTH_SIGNATURE),
    };
    ensure!(
        read_platform_health(&health)?.matches_release(
            1,
            &bridge.version().to_string(),
            InstallScope::Appimage,
            bridge.manifest_sha256()
        ),
        "the signed health proof is for a different AppImage migration"
    );
    if let Some((plan, phase)) = prepared_migration(&workspace)? {
        ensure!(
            plan.destination == destination
                && plan.source_root == source_root
                && plan.recovery_executable == recovery
                && plan.payload_root == bridge.root()
                && plan.payload_manifest_sha256 == bridge.manifest_sha256(),
            "the existing migration belongs to different signed or local inputs"
        );
        if commit {
            ensure!(
                matches!(
                    phase,
                    Some(
                        Phase::NativeChecked(_)
                            | Phase::Healthy(_)
                            | Phase::CommitIntent(_)
                            | Phase::Committed
                    )
                ),
                "stage and review this migration before confirming it"
            );
        } else {
            ensure!(
                !matches!(
                    phase,
                    Some(Phase::CommitIntent(_) | Phase::Committed | Phase::RolledBack)
                ),
                "this migration is already past staging"
            );
        }
    } else {
        ensure!(
            !commit,
            "stage and review this migration before confirming it"
        );
    }
    let selected = if commit {
        stage_image(&pins, &source_image, &install)?;
        PortableImage::open_for_update(&install)?
    } else {
        match fs::symlink_metadata(&install) {
            Ok(_) => pins
                .verify_file(&install)
                .context("an existing v4 destination differs from the verified AppImage")?,
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        PortableImage::open(&source_image)?
    };
    ensure!(
        hash_exact(selected.file().try_clone()?, pins.bytes)? == pins.sha256,
        "the independently staged AppImage changed"
    );
    ensure!(
        file_identity_key(selected.path())? != file_identity_key(&recovery)?,
        "the selected old and new AppImages refer to the same file"
    );

    let mut command = launch_command(selected.path());
    command
        .arg("--bridge-v3")
        .arg("--source-root")
        .arg(source_root)
        .arg("--recovery-executable")
        .arg(recovery)
        .arg("--destination")
        .arg(destination)
        .arg("--work-directory")
        .arg(workspace)
        .arg("--staged-payload")
        .arg(bridge.root())
        .arg("--channel")
        .arg(match selection.channel {
            Channel::Stable => "stable",
            Channel::Beta => "beta",
        });
    if selection.transfer_credentials {
        command.arg("--transfer-credentials");
    }
    if let Some(digest) = selection.discord_consent_digest {
        command.arg("--discord-consent-digest").arg(digest);
    }
    if commit {
        command
            .arg("--health-report")
            .arg(health.report)
            .arg("--health-signature")
            .arg(health.signature);
    }
    Err(command.exec()).context("could not start the independently verified v4 AppImage")
}

fn prepared_migration(workspace: &Path) -> anyhow::Result<Option<(Plan, Option<Phase>)>> {
    match fs::symlink_metadata(workspace.join(PLAN_RECORD)) {
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    }
    let journal = Journal::inspect(workspace)?;
    let plan = journal
        .read::<Plan>(PLAN_RECORD)?
        .context("missing AppImage migration plan")?;
    ensure!(
        plan.workspace == workspace && plan.install_scope == InstallScope::Appimage,
        "the selected journal is not this account's AppImage migration"
    );
    Ok(Some((plan, latest_phase(&journal)?)))
}

fn launch_command(installed: &Path) -> Command {
    let mut command = Command::new(installed);
    for name in [
        "APPIMAGE",
        "APPDIR",
        "ARGV0",
        "OWD",
        "APPIMAGE_EXTRACT_AND_RUN",
        "ALCOMD_MCP_CLIENT_CREDENTIAL",
    ] {
        command.env_remove(name);
    }
    command
}

fn stage_image(asset: &AssetPins, source: &Path, destination: &Path) -> anyhow::Result<()> {
    match fs::symlink_metadata(destination) {
        Ok(_) => {
            return asset
                .verify_file(destination)
                .context("an existing v4 destination differs from the verified AppImage");
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let parent = destination
        .parent()
        .context("missing AppImage destination parent")?;
    let (_, parent_identity) = resolve_plain_storage_directory(parent)?;
    let staging_path = parent.join(format!(".alcomd-bridge-image-{}", uuid::Uuid::new_v4()));
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o700)
        .open(&staging_path)?;
    let stage = TemporaryImage(staging_path);
    let (source, _) = open_plain_file(source)?;
    ensure!(
        io::copy(
            &mut source.take(
                asset
                    .bytes
                    .checked_add(1)
                    .context("AppImage exceeds the copy limit")?
            ),
            &mut output
        )? == asset.bytes,
        "AppImage changed while copied"
    );
    output.sync_all()?;
    drop(output);
    asset.verify_file(&stage.0)?;
    ensure!(
        resolve_plain_storage_directory(parent)?.1 == parent_identity,
        "AppImage destination directory changed"
    );
    publish_path_no_replace(&stage.0, destination)?;
    sync_directory(parent)?;
    Ok(())
}

struct TemporaryImage(PathBuf);

impl Drop for TemporaryImage {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;

    use sha2::{Digest, Sha256};

    use super::*;

    #[test]
    fn publishes_only_the_pinned_image_and_leaves_foreign_files_intact() {
        let fixture = crate::tests::Fixture::create();
        let source = fixture.0.join("verified-source");
        let selected = fixture.0.join("staged image");
        let foreign = fixture.0.join("unrelated image");
        let link = fixture.0.join("existing link");
        let bytes = b"pinned AppImage fixture";
        fs::write(&source, bytes).expect("source fixture");
        fs::write(&foreign, b"preserve me").expect("foreign path");
        symlink(&foreign, &link).expect("foreign link");
        let asset = AssetPins {
            name: "fixture.AppImage".to_owned(),
            bytes: bytes.len() as u64,
            sha256: Sha256::digest(bytes).into(),
            product_manifest_sha256: [1; 32],
            migration_manifest_sha256: [2; 32],
            channel: Channel::Beta,
        };
        stage_image(&asset, &source, &selected).expect("publish verified image");
        stage_image(&asset, &source, &selected).expect("reopen exact image");
        assert_eq!(fs::read(&selected).expect("selected bytes"), bytes);
        assert!(stage_image(&asset, &source, &foreign).is_err());
        assert!(stage_image(&asset, &source, &link).is_err());
        assert_eq!(fs::read(&foreign).expect("foreign bytes"), b"preserve me");
        assert_eq!(fs::read_link(&link).expect("foreign link"), foreign);

        fs::write(&source, b"changed bytes").expect("tampered source fixture");
        let absent = fixture.0.join("no image");
        assert!(stage_image(&asset, &source, &absent).is_err());
        assert!(!absent.exists());
        assert_eq!(
            fs::read_dir(&fixture.0).expect("fixture contents").count(),
            4
        );
    }
}
