use std::io::{ErrorKind, Read};
use std::path::Path;

use alcomd_platform::{
    DaemonInstance, DataConfig, IpcConfig, WINDOWS_BRIDGE_EMPTY_PROFILE_PROOF,
    open_owned_installation_record, resolve_plain_storage_directory, state_database_location,
    windows_empty_bridge_activation_proof, windows_process_is_elevated,
};
use alcomd_updater::distribution::{Target, format_digest, verify_installed_release};
use anyhow::{Context, ensure};

use crate::{
    InstallScope, Journal, PLAN_RECORD, Phase, Plan, latest_phase, phase_file, state_evidence,
};

use super::{WindowsInstallScope, update::check_machine_runtime};

/// Verifies the original account's migration or empty-profile consent for the inactive product.
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
    let data = database.parent().context("missing data directory")?;
    let (parent, _) =
        resolve_plain_storage_directory(data.parent().context("missing data directory parent")?)?;
    let data = parent.join(data.file_name().context("missing data directory name")?);
    let (workspace, _) = resolve_plain_storage_directory(workspace)?;
    ensure!(
        workspace != data && workspace.parent() == data.parent(),
        "the migration journal must belong to this account's data location"
    );
    let version = env!("CARGO_PKG_VERSION").parse()?;
    let executable = std::env::current_exe()?;
    let (stage, _) =
        resolve_plain_storage_directory(executable.parent().context("missing program staging")?)?;
    let product = verify_installed_release(&stage, Target::Windows, &version)?;
    ensure!(
        format_digest(&product.manifest_sha256) == product_manifest,
        "the requested Windows product does not match the verified stage"
    );
    if scope == WindowsInstallScope::AllUsers {
        check_machine_runtime(&stage, &product)?;
    }
    let empty_proof = workspace.join(WINDOWS_BRIDGE_EMPTY_PROFILE_PROOF);
    if !path_is_missing(&empty_proof)? {
        return verify_empty_profile(&empty_proof, &data, scope, product_manifest);
    }

    let (data, _) = resolve_plain_storage_directory(&data)?;
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

    let migration = alcomd_updater::verify_staged_payload(&plan.payload_root, plan.channel)?;
    ensure!(
        migration.version() == &version
            && migration.identity() == plan.payload_identity
            && migration.manifest_sha256() == plan.payload_manifest_sha256,
        "the Windows program and committed migration have different signed inputs"
    );
    product.verify_bridge_pair(&migration)?;
    journal.check()?;
    Ok(())
}

fn verify_empty_profile(
    proof: &Path,
    data: &Path,
    scope: WindowsInstallScope,
    product_manifest: &str,
) -> anyhow::Result<()> {
    ensure!(
        scope == WindowsInstallScope::AllUsers && path_is_missing(data)?,
        "empty-profile activation requires an uninitialized account and a machine bridge"
    );
    let expected = windows_empty_bridge_activation_proof(product_manifest)?;
    let mut actual = Vec::new();
    open_owned_installation_record(proof)?
        .take((expected.len() + 1) as u64)
        .read_to_end(&mut actual)?;
    ensure!(
        actual == expected.as_bytes(),
        "empty-profile approval does not match this account and product"
    );
    Ok(())
}

fn path_is_missing(path: &Path) -> std::io::Result<bool> {
    match path.symlink_metadata() {
        Ok(_) => Ok(false),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(true),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn empty_profile_proof_cannot_replace_data_or_authorize_another_candidate() -> anyhow::Result<()>
    {
        let root = alcomd_platform::local_app_data_directory()?
            .join(format!("alcomd-empty-bridge-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root)?;
        let proof = root.join(WINDOWS_BRIDGE_EMPTY_PROFILE_PROOF);
        let data = root.join("data");
        let manifest = "a".repeat(64);
        fs::write(&proof, windows_empty_bridge_activation_proof(&manifest)?)?;
        verify_empty_profile(&proof, &data, WindowsInstallScope::AllUsers, &manifest)?;
        assert!(
            verify_empty_profile(
                &proof,
                &data,
                WindowsInstallScope::AllUsers,
                &"b".repeat(64)
            )
            .is_err()
        );
        assert!(
            verify_empty_profile(&proof, &data, WindowsInstallScope::CurrentUser, &manifest)
                .is_err()
        );
        fs::create_dir(&data)?;
        assert!(
            verify_empty_profile(&proof, &data, WindowsInstallScope::AllUsers, &manifest).is_err()
        );
        fs::remove_dir(data)?;
        fs::remove_file(proof)?;
        fs::remove_dir(root)?;
        Ok(())
    }
}
