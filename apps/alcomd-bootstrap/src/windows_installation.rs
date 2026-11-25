use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use alcomd_platform::{
    CORE_EXECUTABLE_NAME, DaemonInstance, DataConfig, IpcConfig, check_windows_core_registration,
    remove_windows_core_registration, resolve_plain_storage_directory, start_core_process,
    windows_installation_runtime,
};
use alcomd_updater::distribution::{
    Candidate, Target, VerifiedRuntime, verify_installed_release, verify_release,
};
use anyhow::{Context, ensure};

mod update;
pub use update::{apply_installed_update, resume_installed_update, stage_installed_update};

/// Verifies the installed signed payload and lets the current user's Core initialize its state.
pub fn register_installed_core(
    source_commit: String,
    data_dir: Option<&Path>,
) -> anyhow::Result<()> {
    let (runtime, _) = verified_runtime(source_commit)?;
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
    let (runtime, verified) = verified_runtime(source_commit)?;
    let _instance = DaemonInstance::acquire(&IpcConfig::default())
        .context("stop Core before unregistering it")?;
    verify_uninstall_program_files(&runtime, &verified)?;
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

fn verified_runtime(source_commit: String) -> anyhow::Result<(PathBuf, VerifiedRuntime)> {
    let runtime = windows_installation_runtime(&std::env::current_exe()?)?;
    let verified = verify_release(
        &runtime,
        &Candidate {
            target: Target::Windows,
            version: env!("CARGO_PKG_VERSION").parse()?,
            source_commit,
        },
    )
    .context("Windows installed product payload did not verify")?;
    Ok((runtime, verified))
}

fn verify_uninstall_program_files(
    runtime: &Path,
    verified: &VerifiedRuntime,
) -> anyhow::Result<()> {
    let versions = runtime.parent().context("Windows product runtime parent")?;
    let root = versions.parent().context("Windows installation root")?;
    update::verify_stable_cli(root, verified)
        .context("Windows stable entry point changed; the installation was kept")?;
    // Stock Inno appends new entries to the original uninstall log. Before it can
    // follow that log, verify all surviving versioned payloads in the same root.
    for entry in fs::read_dir(versions)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Ok(version) = name.parse() else {
            continue;
        };
        let path = entry.path();
        if path != runtime {
            let (path, _) = resolve_plain_storage_directory(&path)?;
            verify_installed_release(&path, Target::Windows, &version)
                .context("a retained Windows runtime changed; the installation was kept")?;
        }
    }
    Ok(())
}

fn data_config(path: Option<&Path>) -> DataConfig {
    path.map(|path| DataConfig::isolated(path.to_path_buf()))
        .unwrap_or_default()
}
