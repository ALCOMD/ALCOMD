use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use alcomd_platform::{
    CORE_EXECUTABLE_NAME, DaemonInstance, DataConfig, IpcConfig, check_windows_core_registration,
    open_windows_machine_installation_record, remove_windows_core_registration,
    resolve_plain_storage_directory, start_core_process, windows_installation_runtime,
    windows_machine_installation_root, windows_process_is_elevated,
};
use alcomd_updater::distribution::{
    Candidate, Target, VerifiedRuntime, verify_installed_release, verify_release,
};
use anyhow::{Context, ensure};

mod bridge;
mod scope;
mod update;
pub use bridge::verify_windows_bridge_commit;
pub use scope::{
    ScopeDestinationAction, ScopeSourceAction, WindowsInstallScope, WindowsScopePlanArguments,
    complete_windows_scope_account, coordinate_windows_scope_destination,
    coordinate_windows_scope_source, remove_retired_windows_scope,
    request_windows_machine_scope_source, verify_retired_windows_scope_uninstall,
    windows_scope_plan_fingerprint,
};
pub use update::{
    apply_installed_update, apply_machine_update_worker, resume_installed_update,
    resume_machine_update_worker, stage_installed_update, with_windows_desktop_restart,
};

/// Verifies the installed signed payload and lets the current user's Core initialize its state.
pub fn register_installed_core(
    source_commit: String,
    data_dir: Option<&Path>,
) -> anyhow::Result<()> {
    let (runtime, verified) = verified_runtime(source_commit)?;
    let config = data_config(data_dir);
    if data_dir.is_none() {
        scope::rebind_scope_registration(&runtime, &verified)?;
    }
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
    verify_uninstall_program_files(&runtime, &verified, false)?;
    remove_windows_core_registration(&data_config(data_dir), &runtime)?;
    Ok(())
}

/// Confirms that elevated Inno can remove the machine-owned program without touching a profile.
pub fn verify_installed_machine_uninstall(source_commit: String) -> anyhow::Result<()> {
    ensure!(
        windows_process_is_elevated()?,
        "machine uninstall requires elevation"
    );
    let (runtime, verified) = verified_runtime(source_commit)?;
    let root = windows_machine_installation_root()?;
    ensure!(
        runtime.parent().and_then(Path::parent) == Some(root.as_path()),
        "machine uninstall requires the protected installation root"
    );
    verify_uninstall_program_files(&runtime, &verified, true)
}

/// Starts and registers Core for the current user after verifying the installed runtime.
pub fn start_installed_core() -> anyhow::Result<()> {
    let runtime = windows_installation_runtime(&std::env::current_exe()?)?;
    let verified = verify_installed_release(
        &runtime,
        Target::Windows,
        &env!("CARGO_PKG_VERSION").parse()?,
    )
    .context("Windows installed product payload did not verify")?;
    scope::rebind_scope_registration(&runtime, &verified)?;
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
    machine: bool,
) -> anyhow::Result<()> {
    let versions = runtime.parent().context("Windows product runtime parent")?;
    let root = versions.parent().context("Windows installation root")?;
    update::verify_stable_cli(root, verified)
        .context("Windows stable entry point changed; the installation was kept")?;
    if machine {
        update::check_machine_runtime(runtime, verified)?;
        for name in [
            alcomd_platform::WINDOWS_CLI_EXECUTABLE_NAME,
            alcomd_updater::distribution::WINDOWS_CRT_FILE,
        ] {
            open_windows_machine_installation_record(&root.join("bin").join(name))?;
        }
    }
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
            let verified = verify_installed_release(&path, Target::Windows, &version)
                .context("a retained Windows runtime changed; the installation was kept")?;
            if machine {
                update::check_machine_runtime(&path, &verified)?;
            }
        }
    }
    Ok(())
}

fn data_config(path: Option<&Path>) -> DataConfig {
    path.map(|path| DataConfig::isolated(path.to_path_buf()))
        .unwrap_or_default()
}
