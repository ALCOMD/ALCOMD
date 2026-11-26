//! Explicit per-account handoff from the frozen Linux release.

use std::ffi::OsStr;
use std::fs::{self, DirBuilder};
use std::io::ErrorKind;
use std::os::unix::fs::DirBuilderExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use alcomd_platform::{
    CORE_EXECUTABLE_NAME, DaemonInstance, DataConfig, IpcConfig, LINUX_GUI_EXECUTABLE_NAME,
    PortableImage, check_linux_core_registration, file_identity_key, linux_installation_runtime,
    linux_portable_runtime, require_unprivileged_linux_installation, state_database_location,
    sync_directory, validate_installation_record_parent,
};
use alcomd_updater::decode_hex;
use alcomd_updater::distribution::{Target, verify_installed_release};
use anyhow::{Context, ensure};

use crate::{
    EXE_LIMIT, InstallScope, Journal, MigrationOutcome, MigrationRequest, PLAN_RECORD, Phase, Plan,
    SOURCE_VERSION, file_evidence, latest_phase, linux_portable, migrate_checked_v3,
};

// Installer SHA-256 values pinned in docs/baselines/source-lock.toml.
const V3_APPIMAGE_SHA256: &str = "846d150f4c1707664238283f62262dea560724f42bd524e4c1ec58147ed7b67a";
const V3_DEB_SHA256: &str = "0775e0208ccf77bb2f8a851d5ba83d8eb99494914ec35605d2a2b8de3bb57d6c";
const V3_DEB_PACKAGE: &str = "alcomd3";

/// Stages or commits migration from the original product, using an independently deployed v4.
pub fn bridge_linux_v3(
    request: &MigrationRequest,
    recovery_package: Option<&Path>,
) -> anyhow::Result<MigrationOutcome> {
    require_unprivileged_linux_installation()?;
    let data = DataConfig::default();
    let database = state_database_location(&data)?;
    let destination = database.parent().context("missing user data directory")?;
    let (workspace, _) = crate::journal::isolated_child(&request.work_directory)?;
    ensure!(
        request.destination == destination && workspace == migration_workspace(destination),
        "Linux migration requires this account's data directory and its designated sibling workspace"
    );

    let _instance = DaemonInstance::acquire(&IpcConfig::default())
        .context("close this account's Core before migrating")?;
    let (product, installed) = match request.install_scope {
        InstallScope::Appimage => {
            ensure!(
                recovery_package.is_none(),
                "AppImage recovery uses the original image"
            );
            (mounted_product()?, linux_portable_runtime(&data)?)
        }
        InstallScope::Deb => {
            let runtime = installed_deb(&data)?;
            (runtime.clone(), runtime)
        }
        _ => anyhow::bail!("Linux bridge requires the AppImage or DEB scope"),
    };
    check_linux_core_registration(&data, &installed)
        .context("a different installed Core owns this user account")?;
    let version = env!("CARGO_PKG_VERSION").parse()?;
    migrate_checked_v3(
        request,
        Some(&|bridge, plan| {
            ensure!(
                bridge.version() == &version,
                "the bridge targets a different product version"
            );
            let product = verify_installed_release(&product, Target::Linux, &version)?;
            product.verify_bridge_pair(bridge)?;
            match plan.install_scope {
                InstallScope::Appimage => {
                    ensure!(
                        plan.recovery_file.sha256 == decode_hex(V3_APPIMAGE_SHA256)?,
                        "the retained AppImage is not the published v3.4.0 recovery image"
                    );
                    let new_image = portable_origin()?;
                    ensure!(
                        file_identity_key(new_image.path())? != plan.recovery_file.identity,
                        "the new AppImage must be separate from the v3 recovery image"
                    );
                }
                InstallScope::Deb => {
                    let package =
                        recovery_package.context("the original v3 DEB must be retained")?;
                    let (package, _) = crate::journal::isolated_child(package)?;
                    ensure!(
                        !package.starts_with(&plan.workspace)
                            && !package.starts_with(&plan.destination)
                            && file_evidence(&package, EXE_LIMIT)?.sha256
                                == decode_hex(V3_DEB_SHA256)?,
                        "the retained DEB is not the published v3.4.0 recovery package"
                    );
                    installed_deb(&data)?;
                    let old_version = format!("{SOURCE_VERSION}-1");
                    check_dpkg_owner(
                        V3_DEB_PACKAGE,
                        &plan.recovery_executable,
                        Some(&old_version),
                    )?;
                }
                _ => unreachable!("the Linux scope has already been checked"),
            }
            prepare_workspace(&plan.workspace)
        }),
    )
}

fn migration_workspace(destination: &Path) -> PathBuf {
    destination.with_file_name(format!(".{CORE_EXECUTABLE_NAME}-v3-migration"))
}

/// Blocks normal first launch while the same account has a staged, uncommitted migration.
pub(super) fn check_startup(data: &DataConfig) -> anyhow::Result<()> {
    let database = state_database_location(data)?;
    let destination = database.parent().context("missing user data directory")?;
    let workspace = migration_workspace(destination);
    match fs::symlink_metadata(&workspace) {
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    }
    let plan_path = workspace.join(PLAN_RECORD);
    validate_installation_record_parent(&plan_path)?;
    let _plan = alcomd_platform::open_owned_installation_record(&plan_path)
        .context("finish or roll back this account's pending v3 migration before starting")?;
    let journal = Journal::inspect(&workspace)?;
    let plan = journal
        .read::<Plan>(PLAN_RECORD)?
        .context("missing Linux migration plan")?;
    ensure!(
        plan.workspace == workspace
            && plan.destination == destination
            && matches!(
                plan.install_scope,
                InstallScope::Appimage | InstallScope::Deb
            )
            && matches!(
                latest_phase(&journal)?,
                Some(Phase::Committed | Phase::RolledBack)
            ),
        "finish or roll back this account's pending v3 migration before starting"
    );
    Ok(())
}

fn prepare_workspace(workspace: &Path) -> anyhow::Result<()> {
    match DirBuilder::new().mode(0o700).create(workspace) {
        Ok(()) => sync_directory(
            workspace
                .parent()
                .context("missing migration workspace parent")?,
        )?,
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error).context("cannot create private migration workspace"),
    }
    validate_installation_record_parent(&workspace.join(PLAN_RECORD))
        .context("Linux migration workspace must be owned by this account and private")
}

/// Starts the normal desktop after the same caller received a durable commit outcome.
pub fn start_linux_bridge_desktop(scope: InstallScope) -> anyhow::Result<()> {
    match scope {
        InstallScope::Appimage => linux_portable::start_portable(&[]),
        InstallScope::Deb => {
            let runtime = installed_deb(&DataConfig::default())?;
            let version = env!("CARGO_PKG_VERSION").parse()?;
            verify_installed_release(&runtime, Target::Linux, &version)?;
            Err(Command::new(runtime.join(LINUX_GUI_EXECUTABLE_NAME))
                .env_remove("ALCOMD_MCP_CLIENT_CREDENTIAL")
                .exec())
            .context("installed DEB desktop could not start")
        }
        _ => anyhow::bail!("Linux bridge requires the AppImage or DEB scope"),
    }
}

fn mounted_product() -> anyhow::Result<PathBuf> {
    let mount = std::env::var_os("APPDIR").context("run the new AppImage with --bridge-v3")?;
    linux_portable::embedded_runtime(Path::new(&mount))
}

fn portable_origin() -> anyhow::Result<PortableImage> {
    let path = std::env::var_os("APPIMAGE").context("AppImage origin is unavailable")?;
    PortableImage::open(Path::new(&path)).context("new AppImage cannot be opened")
}

fn installed_deb(data: &DataConfig) -> anyhow::Result<PathBuf> {
    let executable = std::env::current_exe()?;
    let runtime = linux_installation_runtime(&executable, data)?;
    ensure!(
        runtime != linux_portable_runtime(data)?,
        "use Bootstrap installed by apt or dpkg"
    );
    check_dpkg_owner(CORE_EXECUTABLE_NAME, &executable, None)?;
    Ok(runtime)
}

fn check_dpkg_owner(
    package: &str,
    path: &Path,
    expected_version: Option<&str>,
) -> anyhow::Result<()> {
    // The closed package names and the kernel-resolved, absolute paths are passed as arguments.
    let owner = dpkg_query(&[OsStr::new("--search"), OsStr::new("--"), path.as_os_str()])?;
    ensure!(
        owner == format!("{package}: {}", path.display()),
        "file is not owned by its expected Debian package"
    );
    let status_format = "--showformat=${db:Status-Status} ${Version}";
    let status = dpkg_query(&[
        OsStr::new("--show"),
        OsStr::new(status_format),
        OsStr::new("--"),
        OsStr::new(package),
    ])?;
    ensure!(
        status
            .split_once(' ')
            .is_some_and(|(phase, version)| phase == "installed"
                && expected_version.is_none_or(|expected| version == expected)),
        "Debian package is not fully installed at the required version"
    );
    Ok(())
}

fn dpkg_query(args: &[&OsStr]) -> anyhow::Result<String> {
    let output = Command::new("/usr/bin/dpkg-query")
        .args(args)
        .env_clear()
        .env("LC_ALL", "C")
        .output()
        .context("could not query the Debian package manager")?;
    ensure!(
        output.status.success(),
        "Debian package manager could not verify the requested file or package"
    );
    Ok(String::from_utf8(output.stdout)?.trim_end().to_owned())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

    use uuid::Uuid;

    use super::*;

    #[test]
    fn prepare_only_accepts_a_private_workspace_and_leaves_other_paths_intact() {
        let root = std::env::temp_dir()
            .canonicalize()
            .expect("fixture parent")
            .join(format!(".alcomd-linux-bridge-test-{}", Uuid::new_v4()));
        DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .expect("fixture root");
        let workspace = root.join("migration workspace");
        prepare_workspace(&workspace).expect("create private workspace");
        prepare_workspace(&workspace).expect("reopen private workspace");
        assert_eq!(
            fs::metadata(&workspace)
                .expect("workspace metadata")
                .permissions()
                .mode()
                & 0o777,
            0o700
        );

        let foreign = root.join("shared directory");
        DirBuilder::new()
            .mode(0o700)
            .create(&foreign)
            .expect("fixture directory");
        fs::set_permissions(&foreign, fs::Permissions::from_mode(0o755))
            .expect("fixture permissions");
        let entry = foreign.join("preserve");
        fs::write(&entry, b"unrelated data").expect("fixture content");
        assert!(prepare_workspace(&foreign).is_err());
        let link = root.join("link");
        symlink(&workspace, &link).expect("fixture link");
        assert!(prepare_workspace(&link).is_err());
        assert_eq!(
            fs::read(entry).expect("unrelated fixture"),
            b"unrelated data"
        );
        assert_eq!(fs::read_link(link).expect("preserved link"), workspace);
        fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn staged_migration_blocks_startup_until_its_journal_records_rollback() {
        let root = std::env::temp_dir()
            .canonicalize()
            .expect("fixture parent")
            .join(format!(".alcomd-linux-bridge-test-{}", Uuid::new_v4()));
        DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .expect("fixture root");
        let destination = root.join(CORE_EXECUTABLE_NAME);
        let data = DataConfig::isolated(destination.clone());
        check_startup(&data).expect("unrelated first launch remains available");

        let workspace = migration_workspace(&destination);
        prepare_workspace(&workspace).expect("private bridge workspace");
        assert!(check_startup(&data).is_err());
        let executable = root.join("recovery-image");
        fs::write(&executable, b"isolated fixture").expect("recovery fixture");
        let plan = Plan {
            workspace: workspace.clone(),
            source_root: root.join("v3-data"),
            source_identity: vec![],
            recovery_executable: executable.clone(),
            recovery_file: file_evidence(&executable, EXE_LIMIT).expect("recovery evidence"),
            destination: destination.clone(),
            destination_parent_identity: vec![],
            payload_root: root.join("bridge-input"),
            payload_identity: vec![],
            payload_manifest_sha256: [0; 32],
            target_version: env!("CARGO_PKG_VERSION").to_owned(),
            channel: crate::Channel::Beta,
            install_scope: InstallScope::Appimage,
            credential_transfer_id: None,
            discord_consent_digest: None,
        };
        let journal = Journal::open(&workspace).expect("journal");
        journal.record(PLAN_RECORD, &plan).expect("durable plan");
        journal
            .record(&crate::phase_file(1), &Phase::Frozen)
            .expect("freeze record");
        drop(journal);
        assert!(check_startup(&data).is_err());
        assert!(!destination.exists());

        crate::rollback_v3(&workspace).expect("rollback uncommitted migration");
        check_startup(&data).expect("explicit rollback removes the startup interlock");
        assert!(!destination.exists());
        assert_eq!(
            fs::read(executable).expect("original recovery file"),
            b"isolated fixture"
        );
        fs::remove_dir_all(root).expect("remove fixture");
    }
}
