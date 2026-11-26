use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

use alcomd_platform::{
    DataConfig, LINUX_BOOTSTRAP_EXECUTABLE_NAME, LINUX_CLI_EXECUTABLE_NAME,
    PORTABLE_CLI_LAUNCHER_NAME, PortableCliEntry, PortableCliEntryState,
    check_linux_core_registration, linux_portable_runtime, linux_portable_runtime_for_version,
    lock_linux_portable_cli, open_owned_installation_record, publish_path_no_replace,
    registered_core_executable, sync_directory, validate_installation_record_parent,
};
use alcomd_updater::{
    distribution::{VerifiedRuntime, format_digest, verify_portable_runtime},
    hash_exact,
};
use anyhow::{Context, ensure};
use serde::Serialize;

use super::RuntimeStage;

const LAUNCHER_DIRECTORY: &str = "cli-integration-v1";
const PROOF: &str = "bootstrap.sha256";
const LAUNCHER_LIMIT: u64 = 128 * 1024 * 1024;

#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum PortableCliAction {
    Status,
    Enable,
    Remove,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PortableCliStatus {
    Disabled,
    Enabled,
    NeedsRepair,
    Conflict,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum PrivateLauncher {
    Absent,
    Incomplete,
    Ready,
    Conflict,
}

/// Manages the optional shell command from the signed and registered AppImage runtime.
pub fn portable_cli(action: PortableCliAction) -> anyhow::Result<PortableCliStatus> {
    let data = DataConfig::default();
    let runtime = linux_portable_runtime(&data)?;
    let bootstrap = runtime.join(LINUX_BOOTSTRAP_EXECUTABLE_NAME);
    ensure!(
        std::env::current_exe()? == bootstrap && check_linux_core_registration(&data, &runtime)?,
        "CLI integration requires this account's installed AppImage"
    );
    let verified = verify_portable_runtime(&runtime, &env!("CARGO_PKG_VERSION").parse()?)?;
    let root = runtime
        .parent()
        .context("portable runtime root is missing")?;
    let _integration_lock = lock_linux_portable_cli(root)?;
    let private = root.join(LAUNCHER_DIRECTORY);
    let launcher = private.join(PORTABLE_CLI_LAUNCHER_NAME);
    let entry = PortableCliEntry::open(&launcher, false)?;
    let link = entry.state()?;
    if link == PortableCliEntryState::Conflict {
        return Ok(PortableCliStatus::Conflict);
    }
    let state = inspect_launcher(&private)?;
    match action {
        PortableCliAction::Status => Ok(match (link, state) {
            (PortableCliEntryState::Owned, PrivateLauncher::Ready)
                if active_portable_runtime(&data, &runtime).unwrap_or(false) =>
            {
                PortableCliStatus::Enabled
            }
            (PortableCliEntryState::Owned, _) => PortableCliStatus::NeedsRepair,
            (_, PrivateLauncher::Conflict) => PortableCliStatus::Conflict,
            _ => PortableCliStatus::Disabled,
        }),
        PortableCliAction::Enable => {
            if state == PrivateLauncher::Conflict {
                return Ok(if link == PortableCliEntryState::Owned {
                    PortableCliStatus::NeedsRepair
                } else {
                    PortableCliStatus::Conflict
                });
            }
            ensure!(
                active_portable_runtime(&data, &runtime)?,
                "reopen the AppImage from its current location before enabling CLI integration"
            );
            if state != PrivateLauncher::Ready {
                if state == PrivateLauncher::Incomplete {
                    remove_launcher(&private, state)?;
                }
                publish_launcher(root, &private, &bootstrap, &verified)?;
            }
            if link == PortableCliEntryState::Missing {
                let entry = PortableCliEntry::open(&launcher, true)?;
                match entry.create() {
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                        return Ok(PortableCliStatus::Conflict);
                    }
                    result => result?,
                }
            }
            Ok(PortableCliStatus::Enabled)
        }
        PortableCliAction::Remove => {
            if link == PortableCliEntryState::Owned && !entry.remove()? {
                return Ok(PortableCliStatus::Conflict);
            }
            if matches!(state, PrivateLauncher::Ready | PrivateLauncher::Incomplete) {
                remove_launcher(&private, state)?;
            }
            Ok(match state {
                PrivateLauncher::Conflict => PortableCliStatus::Conflict,
                _ => PortableCliStatus::Disabled,
            })
        }
    }
}

fn active_portable_runtime(data: &DataConfig, runtime: &Path) -> io::Result<bool> {
    Ok(registered_core_executable(data)?.is_some_and(|core| core.parent() == Some(runtime)))
}

/// The installed copy of Bootstrap dispatches to the current registered AppImage's CLI.
pub fn start_portable_cli_if_selected() -> anyhow::Result<()> {
    let executable = std::env::current_exe()?;
    if executable
        .file_name()
        .is_none_or(|name| name != PORTABLE_CLI_LAUNCHER_NAME)
    {
        return Ok(());
    }
    let data = DataConfig::default();
    let own_runtime = linux_portable_runtime(&data)?;
    let own_root = own_runtime
        .parent()
        .context("portable runtime root is missing")?;
    let expected = own_root
        .join(LAUNCHER_DIRECTORY)
        .join(PORTABLE_CLI_LAUNCHER_NAME);
    ensure!(
        executable == expected,
        "CLI integration does not belong to this AppImage account"
    );
    let core = registered_core_executable(&data)
        .context("reopen the AppImage from its current location to repair CLI integration")?
        .context("open the AppImage to initialize its CLI integration")?;
    let (runtime, version) = portable_cli_target(&data, &core)?;
    verify_portable_runtime(runtime, &version.parse()?)?;
    Err(Command::new(runtime.join(LINUX_CLI_EXECUTABLE_NAME))
        .args(std::env::args_os().skip(1))
        .exec())
    .context("installed portable CLI could not start")
}

fn portable_cli_target<'a>(
    data: &DataConfig,
    core: &'a Path,
) -> anyhow::Result<(&'a Path, &'a str)> {
    let runtime = core.parent().context("registered runtime is missing")?;
    let version = runtime
        .file_name()
        .and_then(|name| name.to_str())
        .context("registered runtime version is missing")?;
    ensure!(
        runtime == linux_portable_runtime_for_version(data, version)?,
        "CLI integration cannot use a different product installation"
    );
    Ok((runtime, version))
}

fn inspect_launcher(directory: &Path) -> anyhow::Result<PrivateLauncher> {
    match fs::symlink_metadata(directory) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(PrivateLauncher::Absent);
        }
        Err(error) => return Err(error.into()),
        Ok(info) if !info.is_dir() => return Ok(PrivateLauncher::Conflict),
        Ok(_) => {}
    }
    let proof = match open_owned_installation_record(&directory.join(PROOF)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            validate_installation_record_parent(&directory.join(PROOF))?;
            return Ok(if fs::read_dir(directory)?.next().is_none() {
                PrivateLauncher::Incomplete
            } else {
                PrivateLauncher::Conflict
            });
        }
        result => result?,
    };
    let mut digest = Vec::new();
    proof.take(66).read_to_end(&mut digest)?;
    let Some(digest) = digest.strip_suffix(b"\n") else {
        return Ok(PrivateLauncher::Conflict);
    };
    if digest.len() != 64
        || !digest
            .iter()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Ok(PrivateLauncher::Conflict);
    }
    for entry in fs::read_dir(directory)? {
        let name = entry?.file_name();
        if name != PROOF && name != PORTABLE_CLI_LAUNCHER_NAME {
            return Ok(PrivateLauncher::Conflict);
        }
    }
    let launcher = match open_owned_installation_record(&directory.join(PORTABLE_CLI_LAUNCHER_NAME))
    {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(PrivateLauncher::Incomplete);
        }
        result => result?,
    };
    let info = launcher.metadata()?;
    if info.len() > LAUNCHER_LIMIT
        || info.permissions().mode() & 0o6100 != 0o100
        || format_digest(&hash_exact(launcher, info.len())?).as_bytes() != digest
    {
        return Ok(PrivateLauncher::Conflict);
    }
    Ok(PrivateLauncher::Ready)
}

fn publish_launcher(
    root: &Path,
    destination: &Path,
    bootstrap: &Path,
    verified: &VerifiedRuntime,
) -> anyhow::Result<()> {
    let stage = RuntimeStage::new(root)?;
    let source = open_owned_installation_record(bootstrap)?;
    let size = source.metadata()?.len();
    ensure!(
        size <= LAUNCHER_LIMIT,
        "portable CLI launcher exceeds its size limit"
    );
    let path = stage.0.join(PORTABLE_CLI_LAUNCHER_NAME);
    let mut launcher = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o700)
        .open(&path)?;
    ensure!(
        io::copy(&mut source.take(size + 1), &mut launcher)? == size,
        "CLI launcher changed during copy"
    );
    launcher.sync_all()?;
    verified.verify_copy(LINUX_BOOTSTRAP_EXECUTABLE_NAME, &path)?;
    let digest = verified
        .file_sha256(LINUX_BOOTSTRAP_EXECUTABLE_NAME)
        .context("signed Bootstrap digest is missing")?;
    let mut proof = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(stage.0.join(PROOF))?;
    writeln!(proof, "{}", format_digest(digest))?;
    proof.sync_all()?;
    sync_directory(&stage.0)?;
    publish_path_no_replace(&stage.0, destination)?;
    sync_directory(root)?;
    Ok(())
}

fn remove_launcher(directory: &Path, previous: PrivateLauncher) -> anyhow::Result<()> {
    ensure!(
        inspect_launcher(directory)? == previous,
        "the portable CLI launcher changed"
    );
    if previous == PrivateLauncher::Ready {
        fs::remove_file(directory.join(PORTABLE_CLI_LAUNCHER_NAME))?;
    }
    match fs::remove_file(directory.join(PROOF)) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        result => result?,
    }
    fs::remove_dir(directory)?;
    sync_directory(
        directory
            .parent()
            .context("portable CLI launcher parent is missing")?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn stable_cli_dispatch_accepts_the_active_portable_version_and_excludes_system_packages() {
        let data = DataConfig::isolated(Path::new("/tmp/ALCOMD portable data").to_path_buf());
        let newer = Path::new("/tmp/ALCOMD portable data/runtime/4.0.0-beta.1/alcomd");
        let (runtime, version) = portable_cli_target(&data, newer).expect("new active release");
        assert_eq!(runtime, newer.parent().expect("runtime"));
        assert_eq!(version, "4.0.0-beta.1");
        for path in [
            "/usr/lib/alcomd/runtime/4.0.0-beta.1/alcomd",
            "/tmp/ALCOMD portable data/runtime/3.4.0/alcomd",
            "/tmp/ALCOMD portable data/runtime/4.0.0+unpublished/alcomd",
        ] {
            assert!(portable_cli_target(&data, Path::new(path)).is_err());
        }
    }

    #[test]
    fn private_launcher_ownership_requires_matching_bytes_and_preserves_replacements() {
        let stage = RuntimeStage::new(&std::env::temp_dir()).expect("fixture root");
        let private = stage.0.join(LAUNCHER_DIRECTORY);
        assert!(inspect_launcher(&private).expect("absent") == PrivateLauncher::Absent);
        super::super::private_directory(&private).expect("private fixture");
        assert!(
            inspect_launcher(&private).expect("empty after removal") == PrivateLauncher::Incomplete
        );
        let unknown = private.join("unknown");
        fs::write(&unknown, b"user file").expect("unowned file");
        assert!(
            inspect_launcher(&private).expect("unowned private directory")
                == PrivateLauncher::Conflict
        );
        fs::remove_file(unknown).expect("remove fixture");
        let digest = format_digest(&Sha256::digest(b"verified launcher"));
        fs::write(private.join(PROOF), format!("{digest}\n")).expect("launcher proof");
        assert!(inspect_launcher(&private).expect("missing file") == PrivateLauncher::Incomplete);
        let launcher = private.join(PORTABLE_CLI_LAUNCHER_NAME);
        fs::write(&launcher, b"verified launcher").expect("launcher");
        fs::set_permissions(&launcher, fs::Permissions::from_mode(0o700)).expect("executable mode");
        assert!(inspect_launcher(&private).expect("matching bytes") == PrivateLauncher::Ready);
        fs::write(&launcher, b"user replaced launcher").expect("different bytes");
        assert!(inspect_launcher(&private).expect("changed file") == PrivateLauncher::Conflict);
        assert!(remove_launcher(&private, PrivateLauncher::Ready).is_err());
        assert_eq!(
            fs::read(launcher).expect("preserved replacement"),
            b"user replaced launcher"
        );
    }
}
