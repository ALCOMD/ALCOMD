use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::{fs, io};

use alcomd_platform::{
    CORE_EXECUTABLE_NAME, DaemonInstance, DataConfig, IpcConfig, check_windows_core_registration,
    open_owned_installation_record, remove_windows_core_registration,
    resolve_plain_storage_directory, start_core_process, sync_directory,
    validate_installation_record_parent, windows_current_user_installation_root,
    windows_installation_runtime,
};
use alcomd_updater::{
    Channel,
    distribution::{
        Candidate, MANIFEST_NAME, Target, WINDOWS_UPDATE_STAGE_PREFIX, format_digest,
        verify_installed_release, verify_release,
    },
    product_update::{UpdateFormat, UpdateStream, verify_saved_product_update},
};
use anyhow::{Context, ensure};

/// Verifies the installed signed payload and lets the current user's Core initialize its state.
pub fn register_installed_core(
    source_commit: String,
    data_dir: Option<&Path>,
) -> anyhow::Result<()> {
    let runtime = verified_runtime(source_commit)?;
    let config = data_config(data_dir);
    if check_windows_core_registration(&config, &runtime)? {
        return Ok(());
    }

    let mut args = Vec::with_capacity(3);
    if let Some(data_dir) = data_dir {
        args.extend([OsStr::new("--data-dir"), data_dir.as_os_str()]);
    }
    args.push(OsStr::new("register-installation"));
    super::run_command(&runtime.join(CORE_EXECUTABLE_NAME), &runtime, &args)
        .context("Core installation initialization failed")?;
    ensure!(
        check_windows_core_registration(&config, &runtime)?,
        "Core exited before publishing the expected installation registration"
    );
    Ok(())
}

/// Uses the same Core lock as startup and leaves the current user's state in place.
pub fn unregister_installed_core(
    source_commit: String,
    data_dir: Option<&Path>,
) -> anyhow::Result<()> {
    let runtime = verified_runtime(source_commit)?;
    let _instance = DaemonInstance::acquire(&IpcConfig::default())
        .context("stop Core before unregistering it")?;
    remove_windows_core_registration(&data_config(data_dir), &runtime)?;
    Ok(())
}

/// Starts and registers Core for the current user after verifying the installed runtime.
pub fn start_installed_core() -> anyhow::Result<()> {
    let runtime = windows_installation_runtime(&std::env::current_exe()?)?;
    verify_installed_release(
        &runtime,
        Target::Windows,
        &env!("CARGO_PKG_VERSION").parse()?,
    )
    .context("Windows installed product payload did not verify")?;
    start_core_process(
        &runtime.join(CORE_EXECUTABLE_NAME),
        [OsStr::new("register-installation"), OsStr::new("--serve")],
        Some(&runtime),
        "ALCOMD_MCP_CLIENT_CREDENTIAL",
    )
    .context("installed Core could not start")
}

/// Verifies and extracts a complete Inno update without changing active ownership.
pub fn stage_installed_update(
    envelope: &Path,
    asset: &Path,
    replay_directory: &Path,
    channel: Channel,
) -> anyhow::Result<PathBuf> {
    let installed = windows_installation_runtime(&std::env::current_exe()?)?;
    let root = windows_current_user_installation_root()
        .context("a current-user Windows installation is required")?;
    ensure!(
        installed.parent().and_then(Path::parent) == Some(root.as_path()),
        "a current-user Windows installation is required"
    );
    let version = env!("CARGO_PKG_VERSION").parse()?;
    verify_installed_release(&installed, Target::Windows, &version)
        .context("installed Windows payload did not verify")?;

    // The protected file handle excludes writes and deletion through the launch.
    let _asset = open_owned_installation_record(asset)?;
    let update = verify_saved_product_update(
        envelope,
        asset,
        replay_directory,
        UpdateStream {
            channel,
            target: Target::Windows,
            format: UpdateFormat::Inno,
        },
        &version,
    )?;
    ensure!(
        asset.file_name().and_then(OsStr::to_str) == Some(update.asset().name()),
        "product update asset needs its signed installer filename"
    );
    let digest = update.payload_manifest_sha256();
    let digest_text = format_digest(&digest);
    let name = root
        .file_name()
        .and_then(OsStr::to_str)
        .context("Windows installation root needs a plain name")?;
    let staged = root.with_file_name(format!(".{name}{WINDOWS_UPDATE_STAGE_PREFIX}{digest_text}"));
    match fs::symlink_metadata(&staged) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let request = format!("/ALCOMDStage={digest_text}");
            // Inno exits during initialization after extracting to the exact staging root.
            super::run_command_with_exit_code(
                asset,
                &installed,
                &[
                    OsStr::new("/CURRENTUSER"),
                    OsStr::new("/VERYSILENT"),
                    OsStr::new("/SUPPRESSMSGBOXES"),
                    OsStr::new("/SP-"),
                    OsStr::new(&request),
                ],
                1,
            )
            .context("could not extract the Windows product update")?;
        }
        Err(error) => return Err(error.into()),
    }
    let (staged, _) = resolve_plain_storage_directory(&staged)?;
    validate_installation_record_parent(&staged.join(MANIFEST_NAME))?;
    let verified = verify_installed_release(&staged, Target::Windows, update.version())
        .context("staged Windows update payload did not verify")?;
    ensure!(
        verified.manifest_sha256 == digest,
        "staged Windows update has a different payload digest"
    );
    sync_directory(
        staged
            .parent()
            .context("staged Windows update needs a parent directory")?,
    )?;

    let _instance = DaemonInstance::acquire(&IpcConfig::default())
        .context("close the current user's Core before checking the staged update")?;
    super::run_command(
        &staged.join(CORE_EXECUTABLE_NAME),
        &staged,
        &[OsStr::new("check-installation-data")],
    )
    .context("the staged Windows update could not read the current data layout")?;
    Ok(staged)
}

fn verified_runtime(source_commit: String) -> anyhow::Result<PathBuf> {
    let runtime = windows_installation_runtime(&std::env::current_exe()?)?;
    verify_release(
        &runtime,
        &Candidate {
            target: Target::Windows,
            version: env!("CARGO_PKG_VERSION").parse()?,
            source_commit,
        },
    )
    .context("Windows installed product payload did not verify")?;
    Ok(runtime)
}

fn data_config(path: Option<&Path>) -> DataConfig {
    path.map(|path| DataConfig::isolated(path.to_path_buf()))
        .unwrap_or_default()
}
