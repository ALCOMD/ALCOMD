use std::path::Path;

use alcomd_platform::{
    DaemonInstance, DataConfig, IpcConfig, open_owned_installation_record,
    resolve_plain_storage_directory, state_database_location, windows_process_is_elevated,
};
use alcomd_updater::distribution::{Target, format_digest, verify_installed_release};
use anyhow::{Context, ensure};

use crate::{
    InstallScope, Journal, PLAN_RECORD, Phase, Plan, latest_phase, phase_file, state_evidence,
};

use super::{WindowsInstallScope, update::check_machine_runtime};

/// Verifies the original account's committed migration against the exact inactive product.
pub fn verify_windows_bridge_commit(
    workspace: &Path,
    scope: WindowsInstallScope,
    product_manifest: &str,
) -> anyhow::Result<()> {
    ensure!(
        !windows_process_is_elevated()?,
        "Windows migration commitment must be checked by the originating account"
    );
    let _instance = DaemonInstance::acquire(&IpcConfig::default())
        .context("close Core before finishing the Windows installation")?;
    let database = state_database_location(&DataConfig::default())?;
    let (data, _) =
        resolve_plain_storage_directory(database.parent().context("missing data directory")?)?;
    let (workspace, _) = resolve_plain_storage_directory(workspace)?;
    ensure!(
        workspace != data && workspace.parent() == data.parent(),
        "the migration journal must belong to this account's data location"
    );
    let _plan_file = open_owned_installation_record(&workspace.join(PLAN_RECORD))?;
    let journal = Journal::open(&workspace)?;
    let plan = journal
        .read::<Plan>(PLAN_RECORD)?
        .context("missing Windows migration plan")?;
    let install_scope = match scope {
        WindowsInstallScope::CurrentUser => InstallScope::WindowsCurrentUser,
        WindowsInstallScope::AllUsers => InstallScope::WindowsAllUsers,
    };
    ensure!(
        plan.workspace == workspace
            && plan.destination == data
            && plan.install_scope == install_scope,
        "the migration commitment belongs to a different Windows installation"
    );
    ensure!(
        latest_phase(&journal)? == Some(Phase::Committed),
        "Windows migration data has not been committed"
    );
    let Some(Phase::CommitIntent(expected_data)) = journal.read(&phase_file(6))? else {
        anyhow::bail!("missing Windows migration data evidence");
    };
    ensure!(
        state_evidence(&data)? == expected_data,
        "committed Windows migration data changed"
    );

    let version = env!("CARGO_PKG_VERSION").parse()?;
    let executable = std::env::current_exe()?;
    let (stage, _) =
        resolve_plain_storage_directory(executable.parent().context("missing program staging")?)?;
    let product = verify_installed_release(&stage, Target::Windows, &version)?;
    if scope == WindowsInstallScope::AllUsers {
        check_machine_runtime(&stage, &product)?;
    }
    let migration = alcomd_updater::verify_staged_payload(&plan.payload_root, plan.channel)?;
    ensure!(
        format_digest(&product.manifest_sha256) == product_manifest
            && migration.version() == &version
            && migration.identity() == plan.payload_identity
            && migration.manifest_sha256() == plan.payload_manifest_sha256
            && product
                .files()
                .all(|path| { product.file_sha256(path).copied() == migration.file_sha256(path) }),
        "the Windows program and committed migration have different signed inputs"
    );
    journal.check()?;
    Ok(())
}
