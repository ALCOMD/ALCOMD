use super::{
    M7UnityMigrationError, M7UnityMigrationErrorCode, OperationId, error, is_reparse,
    read_regular_file, sync_parent, write_new_synced_file,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
};

const MARKER: &str = "VRCProjectUpgradeMarker.json";
const MAX_SCENES: usize = 16_384;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct UpgradeMarker {
    upgraded_from: SourceVersion,
    wizard_completed: bool,
    scene_list: Vec<String>,
    upgraded_scenes: Option<Vec<String>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
struct SourceVersion {
    full: String,
    major: u32,
    minor: u32,
    patch: u32,
}

fn changed() -> M7UnityMigrationError {
    error(M7UnityMigrationErrorCode::SourceChanged)
}
fn io_error(cause: io::Error) -> M7UnityMigrationError {
    changed().with_source(cause)
}

pub(super) fn collect(root: &Path, source: &str) -> Result<UpgradeMarker, M7UnityMigrationError> {
    let patch = match source {
        "2022.3.6f1" => 6,
        "2022.3.22f1" => 22,
        _ => return Err(changed()),
    };
    let mut dirs = vec![root.join("Assets")];
    let mut scenes = BTreeSet::new();
    while let Some(dir) = dirs.pop() {
        let metadata = fs::symlink_metadata(&dir).map_err(io_error)?;
        if !metadata.is_dir() || is_reparse(&metadata) || metadata.file_type().is_symlink() {
            return Err(changed());
        }
        for entry in fs::read_dir(&dir).map_err(io_error)? {
            let path = entry.map_err(io_error)?.path();
            let metadata = fs::symlink_metadata(&path).map_err(io_error)?;
            if metadata.file_type().is_symlink() || is_reparse(&metadata) {
                return Err(changed());
            }
            if metadata.is_dir() {
                dirs.push(path);
            } else if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("unity"))
            {
                let mut meta = path.into_os_string();
                meta.push(".meta");
                let bytes = read_regular_file(Path::new(&meta), 64 * 1024)?;
                let text =
                    std::str::from_utf8(&bytes).map_err(|cause| changed().with_source(cause))?;
                let mut guids = text
                    .lines()
                    .filter_map(|line| line.strip_prefix("guid:"))
                    .map(str::trim);
                let guid = guids
                    .next()
                    .filter(|v| v.len() == 32 && v.bytes().all(|b| b.is_ascii_hexdigit()))
                    .ok_or_else(changed)?;
                if guids.next().is_some()
                    || !scenes.insert(guid.to_ascii_lowercase())
                    || scenes.len() > MAX_SCENES
                {
                    return Err(changed());
                }
            }
        }
    }
    Ok(UpgradeMarker {
        upgraded_from: SourceVersion {
            full: source.into(),
            major: 2022,
            minor: 3,
            patch,
        },
        wizard_completed: false,
        scene_list: scenes.into_iter().collect(),
        upgraded_scenes: None,
    })
}

fn library(root: &Path) -> Result<Option<PathBuf>, M7UnityMigrationError> {
    let path = root.join("Library");
    match fs::symlink_metadata(&path) {
        Ok(m) if m.is_dir() && !m.file_type().is_symlink() && !is_reparse(&m) => Ok(Some(path)),
        Ok(_) => Err(changed()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io_error(e)),
    }
}

pub(super) fn check_destination(
    root: &Path,
    expected: &UpgradeMarker,
) -> Result<bool, M7UnityMigrationError> {
    let Some(dir) = library(root)? else {
        return Ok(false);
    };
    let target = dir.join(MARKER);
    match fs::symlink_metadata(&target) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(io_error(e)),
        Ok(_) => {
            let data = read_regular_file(&target, 4 * 1024 * 1024)?;
            if serde_json::from_slice::<UpgradeMarker>(&data).ok().as_ref() != Some(expected) {
                return Err(changed());
            }
            Ok(true)
        }
    }
}

pub(super) fn publish(
    root: &Path,
    marker: &UpgradeMarker,
    id: OperationId,
) -> Result<(), M7UnityMigrationError> {
    if check_destination(root, marker)? {
        return Ok(());
    }
    if library(root)?.is_none() {
        fs::create_dir(root.join("Library")).map_err(io_error)?;
        sync_parent(&root.join("Library"))?;
    }
    let dir = library(root)?.ok_or_else(changed)?;
    let target = dir.join(MARKER);
    let temp = dir.join(format!(".alcomd-{id}-upgrade-marker.part"));
    let bytes = serde_json::to_vec(marker).map_err(|cause| changed().with_source(cause))?;
    match fs::symlink_metadata(&temp) {
        Ok(_) => {
            let partial = read_regular_file(&temp, bytes.len() as u64)?;
            if !bytes.starts_with(&partial) {
                return Err(changed());
            }
            fs::remove_file(&temp).map_err(io_error)?;
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(io_error(e)),
    }
    write_new_synced_file(&temp, &bytes)?;
    alcomd_platform::publish_path_no_replace(&temp, &target).map_err(io_error)?;
    sync_parent(&target)
}
