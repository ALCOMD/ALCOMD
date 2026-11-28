//! Per-account startup interlock for a machine installation activated through a bridge.

use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::windows::core_installation::InstallationTrust;
use crate::windows::file_identity_from_file;
use crate::{
    DaemonInstance, DataConfig, IpcConfig, WINDOWS_BRIDGE_ACCOUNT_RECORD, state_database_location,
    windows_process_is_elevated,
};

use super::{hold_windows_installation, installation_trust, publish_owned_record, runtime_root};

const PROFILE_APPROVAL: &str = ".machine-bridge-account-v1";
const MARKER: &[u8] = b"1\n";

/// Original-account consent stored beside the native migration journal.
pub const WINDOWS_BRIDGE_EMPTY_PROFILE_PROOF: &str = "empty-profile-v1";

/// Binds consent for a machine bridge without local legacy data to its candidate and origin SID.
pub fn windows_empty_bridge_activation_proof(product_manifest: &str) -> io::Result<String> {
    if product_manifest.len() != 64
        || !product_manifest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let sid = crate::windows_current_account_sid()?;
    Ok(format!("{product_manifest}\n{sid}\n"))
}

/// Requires an existing store or explicit empty-profile approval in this account.
pub fn check_windows_bridge_profile_for_startup(
    executable: &Path,
    data: &DataConfig,
) -> io::Result<()> {
    let executable = fs::canonicalize(executable)?;
    let Some(runtime) = executable.parent() else {
        return Ok(());
    };
    let Ok(root) = runtime_root(runtime) else {
        return Ok(());
    };
    let Some(identity) = machine_bridge_identity(root)? else {
        return Ok(());
    };
    let database = state_database_location(data)?;
    require_account_store(&database, &identity)
}

/// Authorizes a fresh local profile after the original account explicitly selected it.
pub fn approve_windows_empty_bridge_profile(runtime: &Path) -> io::Result<()> {
    let root = runtime_root(runtime)?;
    if windows_process_is_elevated()? || installation_trust(root)? != InstallationTrust::Machine {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "empty profile approval requires the original unelevated account",
        ));
    }
    let _lease = hold_windows_installation(&runtime.join(super::CORE_EXECUTABLE_NAME))?
        .ok_or_else(super::invalid_registration)?;
    let Some(identity) = machine_bridge_identity(root)? else {
        return Ok(());
    };
    let database = state_database_location(&DataConfig::default())?;
    let _account = DaemonInstance::acquire(&IpcConfig::default()).map_err(io::Error::other)?;
    if open_profile_file(&database)?.is_some() {
        return Ok(());
    }
    let record = profile_approval(&database)?;
    let previous = match InstallationTrust::User.open_record(&record) {
        Ok(file) => Some(file_identity_from_file(&file)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    publish_owned_record(&record, &identity, previous, InstallationTrust::User)
}

fn machine_bridge_identity(root: &Path) -> io::Result<Option<[u8; 24]>> {
    if installation_trust(root)? != InstallationTrust::Machine {
        return Ok(None);
    }
    let file =
        match InstallationTrust::Machine.open_record(&root.join(WINDOWS_BRIDGE_ACCOUNT_RECORD)) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
    let mut bytes = Vec::new();
    (&file).take(3).read_to_end(&mut bytes)?;
    if bytes != MARKER {
        return Err(super::invalid_registration());
    }
    Ok(Some(file_identity_from_file(&file)?.to_key()))
}

fn require_account_store(database: &Path, identity: &[u8; 24]) -> io::Result<()> {
    if open_profile_file(database)?.is_some() {
        return Ok(());
    }
    let record = profile_approval(database)?;
    if let Some(file) = open_profile_file(&record)? {
        let mut bytes = Vec::new();
        file.take(25).read_to_end(&mut bytes)?;
        if bytes.as_slice() == identity {
            return Ok(());
        }
    }
    Err(io::Error::new(
        io::ErrorKind::WouldBlock,
        "complete the Windows migration bridge for this account before starting Core",
    ))
}

fn open_profile_file(path: &Path) -> io::Result<Option<fs::File>> {
    match path.symlink_metadata() {
        Ok(_) => InstallationTrust::User.open_record(path).map(Some),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn profile_approval(database: &Path) -> io::Result<PathBuf> {
    let parent = database
        .parent()
        .and_then(Path::parent)
        .ok_or_else(super::invalid_registration)?;
    Ok(parent.join(PROFILE_APPROVAL))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_account_needs_approval_for_this_installation() -> io::Result<()> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        let root = crate::local_app_data_directory()?.join(format!(
            "alcomd-bridge-profile-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir(&root)?;
        let root = root.canonicalize()?;
        let directory = root.join("data");
        let database = directory.join("state.db");
        let approval = profile_approval(&database)?;
        let identity = [3; 24];
        let result = (|| {
            assert_eq!(
                require_account_store(&database, &identity)
                    .expect_err("a fresh account may not initialize")
                    .kind(),
                io::ErrorKind::WouldBlock
            );
            assert!(!directory.exists());
            fs::write(&approval, [4; 24])?;
            assert_eq!(
                require_account_store(&database, &identity)
                    .expect_err("approval for another installation does not apply")
                    .kind(),
                io::ErrorKind::WouldBlock
            );
            fs::write(&approval, identity)?;
            require_account_store(&database, &identity)?;
            fs::remove_file(&approval)?;
            fs::create_dir(&directory)?;
            fs::write(&database, b"existing account store")?;
            require_account_store(&database, &identity)?;
            Ok(())
        })();
        for file in [database, approval] {
            if file.exists() {
                fs::remove_file(file)?;
            }
        }
        if directory.exists() {
            fs::remove_dir(directory)?;
        }
        fs::remove_dir(root)?;
        result
    }
}
