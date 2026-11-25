use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use alcomd_platform::{
    CORE_EXECUTABLE_NAME, DaemonInstance, DataConfig, IpcConfig, check_windows_core_registration,
    remove_windows_core_registration, start_core_process, windows_installation_runtime,
};
use alcomd_updater::distribution::{Candidate, Target, verify_installed_release, verify_release};
use anyhow::{Context, ensure};

mod update;
pub use update::{apply_installed_update, resume_installed_update, stage_installed_update};

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
