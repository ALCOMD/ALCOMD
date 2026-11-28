use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use alcomd_platform::{open_plain_file, resolve_plain_storage_directory};
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{Scope, assets, native};

const UNINSTALL_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\{3CA473A7-9CEA-4EB6-9949-803C3D0B8057}_is1";
const FROZEN_EXECUTABLE: &str = "ALCOMD3.exe";
const FROZEN_SHA256: &str = "523c9eeaf22a6ce00c1acc1f826e557756f5ff8bdc0fbcc75c6b2ce508c76dd7";
const MAX_FILE: u64 = 1024 * 1024 * 1024;
const MAX_TREE: u64 = 2 * MAX_FILE;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct Installation {
    pub root: PathBuf,
    pub recovery: PathBuf,
    executable_identity: Vec<u8>,
    uninstaller: PathBuf,
    uninstaller_sha256: String,
}

pub(super) fn detect(scope: Scope) -> anyhow::Result<Installation> {
    let value = |name| {
        native::registry_value(scope, UNINSTALL_KEY, name)?
            .context("the requested Windows scope has no confirmed v3 installation")
    };
    ensure!(
        value("DisplayVersion")? == "3.4.0",
        "the selected v3 installation is not the published version"
    );
    ensure!(
        native::registry_value(scope.other(), UNINSTALL_KEY, "Inno Setup: App Path")?.is_none(),
        "both v3 installation scopes are registered"
    );
    let registered = PathBuf::from(value("Inno Setup: App Path")?);
    let (root, _) = resolve_plain_storage_directory(&registered)?;
    if let Some(location) = native::registry_value(scope, UNINSTALL_KEY, "InstallLocation")? {
        let (location, _) = resolve_plain_storage_directory(Path::new(&location))?;
        ensure!(location == root, "the v3 install registration disagrees");
    }
    let recovery = root.join(FROZEN_EXECUTABLE);
    let (sha256, executable_identity, _) = file_evidence(&recovery)?;
    ensure!(
        sha256 == FROZEN_SHA256,
        "the v3 recovery executable changed"
    );
    let uninstall = value("UninstallString")?;
    let uninstaller = registered_uninstaller(&root, &uninstall)?;
    let (uninstaller_sha256, _, _) = file_evidence(&uninstaller)?;
    Ok(Installation {
        root,
        recovery,
        executable_identity,
        uninstaller,
        uninstaller_sha256,
    })
}

fn registered_uninstaller(root: &Path, command: &str) -> anyhow::Result<PathBuf> {
    let quoted = command
        .strip_prefix('"')
        .and_then(|command| command.strip_suffix('"'))
        .context("v3 uninstaller has an unexpected invocation")?;
    let path = Path::new(quoted);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("v3 uninstaller has no usable name")?;
    let lower = name.to_ascii_lowercase();
    let number = lower
        .strip_prefix("unins")
        .and_then(|name| name.strip_suffix(".exe"));
    ensure!(
        number.is_some_and(|number| {
            number.len() == 3 && number.bytes().all(|byte| byte.is_ascii_digit())
        }),
        "v3 uninstaller has an unexpected basename"
    );
    let (directory, _) =
        resolve_plain_storage_directory(path.parent().context("v3 uninstaller has no parent")?)?;
    ensure!(
        directory == root,
        "v3 uninstaller belongs to another installation"
    );
    Ok(directory.join(name))
}

impl Installation {
    pub(super) fn running(&self) -> bool {
        alcomd_platform::observe_processes()
            .processes()
            .iter()
            .filter_map(|process| process.executable())
            .filter(|path| {
                path.file_name()
                    .is_some_and(|name| name.eq_ignore_ascii_case(FROZEN_EXECUTABLE))
            })
            .any(|path| {
                open_plain_file(path)
                    .is_ok_and(|(_, identity)| identity == self.executable_identity)
            })
    }
}

pub(super) fn source(local: &Path) -> anyhow::Result<Option<PathBuf>> {
    let source = local.join("ALCOMD3");
    match source.symlink_metadata() {
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
        Ok(_) => Ok(Some(resolve_plain_storage_directory(&source)?.0)),
    }
}

pub(super) fn snapshot(root: &Path) -> anyhow::Result<String> {
    let mut files = BTreeMap::new();
    let mut bytes = 0;
    collect(root, root, 0, &mut files, &mut bytes)?;
    Ok(assets::format_digest(
        &Sha256::digest(serde_json::to_vec(&files)?).into(),
    ))
}

fn collect(
    root: &Path,
    directory: &Path,
    depth: usize,
    files: &mut BTreeMap<PathBuf, String>,
    bytes: &mut u64,
) -> anyhow::Result<()> {
    ensure!(depth < 32, "v3 data exceeds the migration path depth");
    resolve_plain_storage_directory(directory)?;
    for entry in fs::read_dir(directory)? {
        ensure!(
            files.len() < 65_536,
            "v3 data exceeds the migration file limit"
        );
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(root)?.to_owned();
        if entry.file_type()?.is_dir() {
            files.insert(relative, String::new());
            collect(root, &path, depth + 1, files, bytes)?;
        } else {
            let (digest, _, length) = file_evidence(&path)?;
            *bytes = bytes.checked_add(length).context("v3 data size overflow")?;
            ensure!(
                *bytes <= MAX_TREE,
                "v3 data exceeds the migration size limit"
            );
            files.insert(relative, digest);
        }
    }
    Ok(())
}

fn file_evidence(path: &Path) -> anyhow::Result<(String, Vec<u8>, u64)> {
    let (file, identity) = open_plain_file(path)?;
    let before = file.metadata()?;
    ensure!(
        before.len() <= MAX_FILE,
        "v3 file exceeds the migration size limit"
    );
    let digest = assets::hash(&file)?;
    let after = file.metadata()?;
    ensure!(
        before.len() == after.len() && before.modified()? == after.modified()?,
        "v3 file changed during fingerprinting"
    );
    Ok((digest, identity, after.len()))
}
