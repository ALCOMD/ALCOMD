use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use alcomd_application::{
    BackupArchiveEvidence, BackupCancellation, BackupCompression, BackupCreateRequest,
    M5BackupAdapter, M5BackupError, M5BackupErrorCode, OperationId, ProjectRecord, PublishedBackup,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive};

use crate::{ArchiveLimits, preflight_archive_with_limits};

#[derive(Clone, Debug)]
pub struct BackupEngine {
    partial: PathBuf,
    objects: PathBuf,
    limits: ArchiveLimits,
}

#[derive(Clone, Debug)]
pub struct BackupInventory {
    project: ProjectRecord,
    entries: Vec<InventoryEntry>,
    fingerprint: [u8; 32],
    excluded_packages: Vec<ExcludedPackage>,
}

#[derive(Clone, Debug)]
struct InventoryEntry {
    relative: String,
    path: PathBuf,
    kind: EntryKind,
    identity: Vec<u8>,
    bytes: u64,
    modified_ns: u128,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EntryKind {
    Directory,
    File,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExcludedPackage {
    package_id: String,
    version: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BackupManifest<'a> {
    format_version: u32,
    created_at_ms: u64,
    compression_mode: BackupCompression,
    exclude_vpm_packages: bool,
    source_project_revision: u64,
    source_project_fingerprint: String,
    unity_version: &'a str,
    excluded_packages: &'a [ExcludedPackage],
    packages_require_resolve: bool,
}

impl BackupEngine {
    pub fn new(root: PathBuf) -> Result<Self, M5BackupError> {
        Self::with_limits(root, ArchiveLimits::backup())
    }

    pub fn with_limits(root: PathBuf, limits: ArchiveLimits) -> Result<Self, M5BackupError> {
        let partial = root.join("partial");
        let objects = root.join("objects");
        for directory in [&root, &partial, &objects] {
            std::fs::create_dir_all(directory).map_err(|_| internal())?;
            let metadata = std::fs::symlink_metadata(directory).map_err(|_| internal())?;
            if !metadata.is_dir() || is_link_or_reparse(&metadata) {
                return Err(error(M5BackupErrorCode::BackupSourceUnsafe));
            }
        }
        Ok(Self {
            partial,
            objects,
            limits,
        })
    }
}

impl M5BackupAdapter for BackupEngine {
    type Inventory = BackupInventory;

    async fn inventory(
        &self,
        project: ProjectRecord,
        request: BackupCreateRequest,
    ) -> Result<Self::Inventory, M5BackupError> {
        let limits = self.limits;
        tokio::task::spawn_blocking(move || build_inventory(project, &request, limits))
            .await
            .map_err(|_| internal())?
    }

    async fn archive(
        &self,
        operation_id: OperationId,
        request: BackupCreateRequest,
        inventory: Self::Inventory,
        cancellation: BackupCancellation,
    ) -> Result<BackupArchiveEvidence, M5BackupError> {
        let path = self.partial.join(format!("{operation_id}.zip.part"));
        let limits = self.limits;
        tokio::task::spawn_blocking(move || {
            if path.exists() {
                let metadata = std::fs::symlink_metadata(&path).map_err(|_| internal())?;
                if !metadata.is_file() || is_link_or_reparse(&metadata) {
                    return Err(error(M5BackupErrorCode::RecoveryRequired));
                }
                std::fs::remove_file(&path).map_err(|_| internal())?;
            }
            let result = write_archive(&path, &request, &inventory, &cancellation, limits);
            if result.is_err() {
                let _ = std::fs::remove_file(&path);
            }
            result
        })
        .await
        .map_err(|_| internal())?
    }

    async fn publish_or_recover(
        &self,
        operation_id: OperationId,
        request: BackupCreateRequest,
        evidence: BackupArchiveEvidence,
    ) -> Result<PublishedBackup, M5BackupError> {
        let partial = self.partial.join(format!("{operation_id}.zip.part"));
        let final_path = self.objects.join(format!("{}.zip", request.backup_id));
        let objects = self.objects.clone();
        let limits = self.limits;
        tokio::task::spawn_blocking(move || {
            if final_path.exists() {
                verify_archive(&final_path, &evidence, limits)?;
                return published(&final_path, request.backup_id);
            }
            verify_archive(&partial, &evidence, limits)?;
            match std::fs::rename(&partial, &final_path) {
                Ok(()) => {}
                Err(_) if final_path.exists() => {
                    verify_archive(&final_path, &evidence, limits)?;
                    std::fs::remove_file(&partial).map_err(|_| internal())?;
                }
                Err(_) => return Err(internal()),
            }
            alcomd_platform::sync_directory(&objects).map_err(|_| internal())?;
            verify_archive(&final_path, &evidence, limits)?;
            published(&final_path, request.backup_id)
        })
        .await
        .map_err(|_| internal())?
    }

    async fn discard_partial(&self, operation_id: OperationId) -> Result<(), M5BackupError> {
        let path = self.partial.join(format!("{operation_id}.zip.part"));
        tokio::task::spawn_blocking(move || match std::fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() && !is_link_or_reparse(&metadata) => {
                std::fs::remove_file(path).map_err(|_| internal())
            }
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(error(M5BackupErrorCode::RecoveryRequired)),
        })
        .await
        .map_err(|_| internal())?
    }
}

fn build_inventory(
    project: ProjectRecord,
    request: &BackupCreateRequest,
    limits: ArchiveLimits,
) -> Result<BackupInventory, M5BackupError> {
    let root = PathBuf::from(&project.observation.root_path);
    if !root.is_absolute()
        || alcomd_platform::file_identity_key(&root).map_err(|_| unsafe_source())?
            != project.observation.path_identity_key
    {
        return Err(unsafe_source());
    }
    let root_metadata = std::fs::symlink_metadata(&root).map_err(|_| unsafe_source())?;
    if !root_metadata.is_dir() || is_link_or_reparse(&root_metadata) {
        return Err(unsafe_source());
    }
    let excluded_packages = if request.exclude_vpm_packages {
        validated_exclusions(&root, &project)?
    } else {
        Vec::new()
    };
    let excluded_ids = excluded_packages
        .iter()
        .map(|value| value.package_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut entries = Vec::new();
    walk(&root, &root, &excluded_ids, &mut entries, limits)?;
    entries.sort_by(|left, right| left.relative.cmp(&right.relative));
    if entries.len().saturating_add(2) > limits.entries {
        return Err(limit());
    }
    let mut collision = BTreeSet::new();
    let mut total = 0_u64;
    let mut hasher = Sha256::new();
    for entry in &entries {
        if entry.relative.len() > limits.normalized_path_bytes
            || entry.relative.split('/').count() > limits.path_depth
            || !collision.insert(entry.relative.nfc().collect::<String>().to_lowercase())
        {
            return Err(limit());
        }
        total = total.checked_add(entry.bytes).ok_or_else(limit)?;
        if entry.bytes > limits.entry_bytes || total > limits.total_uncompressed_bytes {
            return Err(limit());
        }
        hash_field(&mut hasher, entry.relative.as_bytes())?;
        hasher.update([match entry.kind {
            EntryKind::Directory => 0,
            EntryKind::File => 1,
        }]);
        hash_field(&mut hasher, &entry.identity)?;
        hasher.update(entry.bytes.to_le_bytes());
        hasher.update(entry.modified_ns.to_le_bytes());
    }
    for manifest in ["Packages/vpm-manifest.json", "Packages/manifest.json"] {
        let bytes = std::fs::read(root.join(manifest)).map_err(|_| unsafe_source())?;
        if bytes.len() > 4 * 1024 * 1024 {
            return Err(limit());
        }
        hash_field(&mut hasher, manifest.as_bytes())?;
        hash_field(&mut hasher, &bytes)?;
    }
    Ok(BackupInventory {
        project,
        entries,
        fingerprint: hasher.finalize().into(),
        excluded_packages,
    })
}

fn walk(
    root: &Path,
    path: &Path,
    excluded_packages: &BTreeSet<&str>,
    entries: &mut Vec<InventoryEntry>,
    limits: ArchiveLimits,
) -> Result<(), M5BackupError> {
    let mut children = std::fs::read_dir(path)
        .map_err(|_| unsafe_source())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| unsafe_source())?;
    children.sort_by_key(std::fs::DirEntry::file_name);
    for child in children {
        let child_path = child.path();
        let relative = normalized_relative(root, &child_path)?;
        let metadata = std::fs::symlink_metadata(&child_path).map_err(|_| unsafe_source())?;
        if is_link_or_reparse(&metadata) {
            return Err(unsafe_source());
        }
        if metadata.is_dir() {
            if excluded_by_profile(&relative, excluded_packages) {
                if preserve_library_scene(&relative) {
                    let mut direct = std::fs::read_dir(&child_path)
                        .map_err(|_| unsafe_source())?
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|_| unsafe_source())?;
                    direct.sort_by_key(std::fs::DirEntry::file_name);
                    for candidate in direct {
                        if candidate.file_name().to_str().is_some_and(|name| {
                            name.eq_ignore_ascii_case("LastSceneManagerSetup.txt")
                        }) {
                            add_file(root, &candidate.path(), entries)?;
                        }
                    }
                }
                continue;
            }
            entries.push(inventory_entry(
                &child_path,
                relative,
                EntryKind::Directory,
                &metadata,
            )?);
            if entries.len() > limits.entries {
                return Err(limit());
            }
            walk(root, &child_path, excluded_packages, entries, limits)?;
        } else if metadata.is_file() {
            verify_single_link(&child_path, &metadata)?;
            entries.push(inventory_entry(
                &child_path,
                relative,
                EntryKind::File,
                &metadata,
            )?);
        } else {
            return Err(unsafe_source());
        }
    }
    Ok(())
}

fn add_file(
    root: &Path,
    path: &Path,
    entries: &mut Vec<InventoryEntry>,
) -> Result<(), M5BackupError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| unsafe_source())?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) {
        return Err(unsafe_source());
    }
    verify_single_link(path, &metadata)?;
    entries.push(inventory_entry(
        path,
        normalized_relative(root, path)?,
        EntryKind::File,
        &metadata,
    )?);
    Ok(())
}

fn inventory_entry(
    path: &Path,
    relative: String,
    kind: EntryKind,
    metadata: &std::fs::Metadata,
) -> Result<InventoryEntry, M5BackupError> {
    let modified_ns = metadata
        .modified()
        .map_err(|_| unsafe_source())?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| unsafe_source())?
        .as_nanos();
    Ok(InventoryEntry {
        relative,
        path: path.to_path_buf(),
        kind,
        identity: alcomd_platform::file_identity_key(path).map_err(|_| unsafe_source())?,
        bytes: if kind == EntryKind::File {
            metadata.len()
        } else {
            0
        },
        modified_ns,
    })
}

fn write_archive(
    path: &Path,
    request: &BackupCreateRequest,
    inventory: &BackupInventory,
    cancellation: &BackupCancellation,
    limits: ArchiveLimits,
) -> Result<BackupArchiveEvidence, M5BackupError> {
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| internal())?;
    let method = match request.compression_mode {
        BackupCompression::Store => CompressionMethod::Stored,
        BackupCompression::Fast | BackupCompression::Maximum => CompressionMethod::Deflated,
    };
    let level = match request.compression_mode {
        BackupCompression::Store => None,
        BackupCompression::Fast => Some(1),
        BackupCompression::Maximum => Some(9),
    };
    let file_options = SimpleFileOptions::default()
        .compression_method(method)
        .compression_level(level)
        .unix_permissions(0o600);
    let dir_options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .unix_permissions(0o700);
    let manifest = BackupManifest {
        format_version: 1,
        created_at_ms: request.created_at_ms,
        compression_mode: request.compression_mode,
        exclude_vpm_packages: request.exclude_vpm_packages,
        source_project_revision: inventory.project.revision.get(),
        source_project_fingerprint: hex(&inventory.fingerprint),
        unity_version: &inventory.project.observation.unity_version,
        excluded_packages: &inventory.excluded_packages,
        packages_require_resolve: true,
    };
    let manifest = serde_json::to_vec(&manifest).map_err(|_| internal())?;
    let mut zip = zip::ZipWriter::new(BoundedFile {
        file: output,
        limit: limits.archive_bytes,
    });
    zip.start_file("backup.json", file_options)
        .map_err(|_| internal())?;
    zip.write_all(&manifest).map_err(|_| internal())?;
    zip.add_directory("project/", dir_options)
        .map_err(|_| internal())?;
    let mut buffer = [0_u8; 64 * 1024];
    let mut archiving_gate_reached = false;
    for entry in &inventory.entries {
        if cancellation.cancelled() {
            return Err(error(M5BackupErrorCode::RecoveryRequired));
        }
        let name = format!(
            "project/{}{}",
            entry.relative,
            if entry.kind == EntryKind::Directory {
                "/"
            } else {
                ""
            }
        );
        if entry.kind == EntryKind::Directory {
            zip.add_directory(name, dir_options)
                .map_err(|_| internal())?;
            continue;
        }
        let before = evidence_for(&entry.path)?;
        if before != (entry.identity.clone(), entry.bytes, entry.modified_ns) {
            return Err(changed());
        }
        zip.start_file(name, file_options).map_err(|_| internal())?;
        let mut input = File::open(&entry.path).map_err(|_| unsafe_source())?;
        let mut remaining = entry.bytes;
        while remaining > 0 {
            if cancellation.cancelled() {
                return Err(error(M5BackupErrorCode::RecoveryRequired));
            }
            let maximum = remaining.min(buffer.len() as u64) as usize;
            let read = input.read(&mut buffer[..maximum]).map_err(|_| changed())?;
            if read == 0 {
                return Err(changed());
            }
            zip.write_all(&buffer[..read]).map_err(|_| internal())?;
            remaining -= read as u64;
            if !archiving_gate_reached {
                archiving_gate_reached = true;
                kill_gate("archiving");
            }
        }
        if input.read(&mut buffer[..1]).map_err(|_| changed())? != 0
            || evidence_for(&entry.path)? != before
        {
            return Err(changed());
        }
    }
    let output = zip.finish().map_err(|_| limit())?;
    output.file.sync_all().map_err(|_| internal())?;
    let final_inventory = build_inventory(inventory.project.clone(), request, limits)?;
    if final_inventory.fingerprint != inventory.fingerprint
        || final_inventory.excluded_packages != inventory.excluded_packages
    {
        return Err(changed());
    }
    validate_v1(path, limits)?;
    let metadata = std::fs::metadata(path).map_err(|_| internal())?;
    if metadata.len() > limits.archive_bytes {
        return Err(limit());
    }
    Ok(BackupArchiveEvidence {
        archive_sha256: hash_file(path)?,
        archive_bytes: metadata.len(),
        source_project_fingerprint: inventory.fingerprint,
    })
}

fn validate_v1(path: &Path, limits: ArchiveLimits) -> Result<(), M5BackupError> {
    let preflight = preflight_archive_with_limits(path, limits)
        .map_err(|_| error(M5BackupErrorCode::BackupIntegrityMismatch))?;
    if preflight.entries.len() < 2 {
        return Err(error(M5BackupErrorCode::BackupIntegrityMismatch));
    }
    let file = File::open(path).map_err(|_| internal())?;
    let mut archive =
        ZipArchive::new(file).map_err(|_| error(M5BackupErrorCode::BackupIntegrityMismatch))?;
    let mut roots = BTreeSet::new();
    for index in 0..archive.len() {
        let name = archive
            .by_index(index)
            .map_err(|_| internal())?
            .name()
            .to_owned();
        roots.insert(name.split('/').next().unwrap_or_default().to_owned());
    }
    if roots != BTreeSet::from(["backup.json".to_owned(), "project".to_owned()]) {
        return Err(error(M5BackupErrorCode::BackupIntegrityMismatch));
    }
    let mut manifest = archive
        .by_name("backup.json")
        .map_err(|_| error(M5BackupErrorCode::BackupIntegrityMismatch))?;
    let mut bytes = Vec::new();
    manifest.read_to_end(&mut bytes).map_err(|_| internal())?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| error(M5BackupErrorCode::BackupIntegrityMismatch))?;
    let object = value
        .as_object()
        .ok_or_else(|| error(M5BackupErrorCode::BackupIntegrityMismatch))?;
    let expected = BTreeSet::from([
        "formatVersion",
        "createdAtMs",
        "compressionMode",
        "excludeVpmPackages",
        "sourceProjectRevision",
        "sourceProjectFingerprint",
        "unityVersion",
        "excludedPackages",
        "packagesRequireResolve",
    ]);
    if object.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected
        || object
            .get("formatVersion")
            .and_then(serde_json::Value::as_u64)
            != Some(1)
        || object
            .get("packagesRequireResolve")
            .and_then(serde_json::Value::as_bool)
            != Some(true)
    {
        return Err(error(M5BackupErrorCode::BackupIntegrityMismatch));
    }
    Ok(())
}

fn verify_archive(
    path: &Path,
    evidence: &BackupArchiveEvidence,
    limits: ArchiveLimits,
) -> Result<(), M5BackupError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| error(M5BackupErrorCode::RecoveryRequired))?;
    if !metadata.is_file()
        || is_link_or_reparse(&metadata)
        || metadata.len() != evidence.archive_bytes
        || hash_file(path)? != evidence.archive_sha256
    {
        return Err(error(M5BackupErrorCode::BackupIntegrityMismatch));
    }
    validate_v1(path, limits)
}

fn published(
    path: &Path,
    backup_id: alcomd_application::BackupId,
) -> Result<PublishedBackup, M5BackupError> {
    Ok(PublishedBackup {
        archive_locator: format!("backup-v1:{backup_id}"),
        file_identity_key: alcomd_platform::file_identity_key(path).map_err(|_| internal())?,
    })
}

fn validated_exclusions(
    root: &Path,
    project: &ProjectRecord,
) -> Result<Vec<ExcludedPackage>, M5BackupError> {
    let manifest_path = root.join("Packages/vpm-manifest.json");
    let manifest_bytes = std::fs::read(&manifest_path).map_err(|_| unsafe_source())?;
    if manifest_bytes.len() > 4 * 1024 * 1024 {
        return Err(limit());
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&manifest_bytes).map_err(|_| unsafe_source())?;
    let locked = manifest
        .get("locked")
        .and_then(serde_json::Value::as_object)
        .ok_or_else(unsafe_source)?;
    let current = locked
        .iter()
        .map(|(package_id, value)| {
            let version = value
                .get("version")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(unsafe_source)?;
            Ok((package_id.clone(), version.to_owned()))
        })
        .collect::<Result<BTreeMap<_, _>, M5BackupError>>()?;
    let expected = project
        .observation
        .locked_dependencies
        .iter()
        .map(|value| (value.package_id.clone(), value.value.clone()))
        .collect::<BTreeMap<_, _>>();
    if current != expected {
        return Err(error(M5BackupErrorCode::ProjectChangedDuringBackup));
    }
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for locked in &project.observation.locked_dependencies {
        if !valid_package_id(&locked.package_id) || !seen.insert(locked.package_id.clone()) {
            return Err(unsafe_source());
        }
        let path = root.join("Packages").join(&locked.package_id);
        let metadata = std::fs::symlink_metadata(&path).map_err(|_| unsafe_source())?;
        if !metadata.is_dir() || is_link_or_reparse(&metadata) {
            return Err(unsafe_source());
        }
        result.push(ExcludedPackage {
            package_id: locked.package_id.clone(),
            version: locked.value.clone(),
        });
    }
    result.sort_by(|left, right| left.package_id.cmp(&right.package_id));
    Ok(result)
}

fn excluded_by_profile(relative: &str, packages: &BTreeSet<&str>) -> bool {
    let parts = relative.split('/').collect::<Vec<_>>();
    if parts.iter().any(|part| part.eq_ignore_ascii_case(".git")) {
        return true;
    }
    if parts.len() == 1 {
        let root = parts[0];
        if ["Logs", "Obj", "Temp"]
            .iter()
            .any(|excluded| root.eq_ignore_ascii_case(excluded))
            || root
                .get(..7)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("Library"))
        {
            return true;
        }
    }
    parts.len() == 2 && parts[0] == "Packages" && packages.contains(parts[1])
}

fn preserve_library_scene(relative: &str) -> bool {
    !relative.contains('/')
        && relative
            .get(..7)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("Library"))
}

fn valid_package_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 214
        && value.split(['.', '_', '-']).all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
}

fn normalized_relative(root: &Path, path: &Path) -> Result<String, M5BackupError> {
    let relative = path.strip_prefix(root).map_err(|_| unsafe_source())?;
    let value = relative
        .to_str()
        .ok_or_else(unsafe_source)?
        .replace('\\', "/");
    if value.is_empty()
        || value.starts_with('/')
        || value
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        return Err(unsafe_source());
    }
    Ok(value)
}

fn evidence_for(path: &Path) -> Result<(Vec<u8>, u64, u128), M5BackupError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| changed())?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) {
        return Err(unsafe_source());
    }
    verify_single_link(path, &metadata)?;
    let modified = metadata
        .modified()
        .map_err(|_| changed())?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| changed())?
        .as_nanos();
    Ok((
        alcomd_platform::file_identity_key(path).map_err(|_| changed())?,
        metadata.len(),
        modified,
    ))
}

fn hash_file(path: &Path) -> Result<[u8; 32], M5BackupError> {
    let mut file = File::open(path).map_err(|_| internal())?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|_| internal())?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(hash.finalize().into())
}

fn hash_field(hash: &mut Sha256, bytes: &[u8]) -> Result<(), M5BackupError> {
    hash.update(
        u32::try_from(bytes.len())
            .map_err(|_| limit())?
            .to_le_bytes(),
    );
    hash.update(bytes);
    Ok(())
}

struct BoundedFile {
    file: File,
    limit: u64,
}

impl Write for BoundedFile {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let position = self.file.stream_position()?;
        let requested = u64::try_from(buffer.len())
            .ok()
            .and_then(|length| position.checked_add(length))
            .ok_or_else(|| std::io::Error::other("Backup archive quota exceeded"))?;
        if requested > self.limit {
            return Err(std::io::Error::other("Backup archive quota exceeded"));
        }
        self.file.write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

impl Seek for BoundedFile {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.file.seek(position)
    }
}

#[cfg(unix)]
fn verify_single_link(_: &Path, metadata: &std::fs::Metadata) -> Result<(), M5BackupError> {
    use std::os::unix::fs::MetadataExt;
    if metadata.nlink() == 1 {
        Ok(())
    } else {
        Err(unsafe_source())
    }
}

#[cfg(windows)]
fn verify_single_link(path: &Path, _: &std::fs::Metadata) -> Result<(), M5BackupError> {
    match alcomd_platform::file_link_count(path) {
        Ok(1) => Ok(()),
        _ => Err(unsafe_source()),
    }
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
}

#[cfg(unix)]
fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(feature = "test-kill-gates")]
fn kill_gate(phase: &str) {
    if std::env::var("ALCOMD_TEST_BACKUP_KILL_GATE").as_deref() == Ok(phase) {
        let signal = std::env::var_os("ALCOMD_TEST_BACKUP_KILL_SIGNAL")
            .expect("Backup kill gate signal path");
        std::fs::write(signal, phase).expect("write Backup kill gate signal");
        loop {
            std::thread::park();
        }
    }
}

#[cfg(not(feature = "test-kill-gates"))]
fn kill_gate(_: &str) {}

fn hex(value: &[u8; 32]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}
const fn error(code: M5BackupErrorCode) -> M5BackupError {
    M5BackupError::new(code)
}
const fn internal() -> M5BackupError {
    error(M5BackupErrorCode::Internal)
}
const fn unsafe_source() -> M5BackupError {
    error(M5BackupErrorCode::BackupSourceUnsafe)
}
const fn limit() -> M5BackupError {
    error(M5BackupErrorCode::BackupLimitExceeded)
}
const fn changed() -> M5BackupError {
    error(M5BackupErrorCode::ProjectChangedDuringBackup)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alcomd_application::{
        DependencyIdentity, ManifestState, ProjectId, ProjectObservation, ProjectType, Revision,
    };
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(1);

    fn fixture() -> (PathBuf, ProjectRecord) {
        let root = std::env::temp_dir().join(format!(
            "alcomd-backup-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        for directory in [
            "Assets/Empty",
            "ProjectSettings",
            "Packages/com.example.locked",
            "Packages/com.example.embedded",
            "LibraryCache",
            "UserSettings",
            ".idea",
        ] {
            std::fs::create_dir_all(root.join(directory)).expect("directory");
        }
        std::fs::write(root.join("Assets/source.txt"), b"source").expect("source");
        std::fs::write(
            root.join("ProjectSettings/ProjectVersion.txt"),
            b"m_EditorVersion: 2022.3.22f1\n",
        )
        .expect("version");
        std::fs::write(root.join("Packages/manifest.json"), b"{}").expect("manifest");
        std::fs::write(
            root.join("Packages/vpm-manifest.json"),
            b"{\"dependencies\":{\"com.example.locked\":\"1.0.0\"},\"locked\":{\"com.example.locked\":{\"version\":\"1.0.0\"}}}",
        )
        .expect("vpm");
        std::fs::write(root.join("Packages/com.example.locked/package.json"), b"{}")
            .expect("locked");
        std::fs::write(
            root.join("Packages/com.example.embedded/package.json"),
            b"{}",
        )
        .expect("embedded");
        std::fs::write(
            root.join("LibraryCache/LastSceneManagerSetup.txt"),
            b"scene",
        )
        .expect("scene");
        let record = ProjectRecord {
            project_id: ProjectId::new(),
            observation: ProjectObservation {
                root_path: root.to_string_lossy().into_owned(),
                path_identity_key: alcomd_platform::file_identity_key(&root).expect("identity"),
                project_type: ProjectType::VpmStarter,
                unity_version: "2022.3.22f1".to_owned(),
                unity_revision: None,
                vpm_manifest: ManifestState::Valid,
                upm_manifest: ManifestState::Valid,
                direct_dependencies: Vec::new(),
                locked_dependencies: vec![DependencyIdentity {
                    package_id: "com.example.locked".to_owned(),
                    value: "1.0.0".to_owned(),
                }],
                issues: Vec::new(),
                observed_at_ms: 1,
            },
            revision: Revision::INITIAL,
            registered_at_ms: 1,
        };
        (root, record)
    }

    #[test]
    fn inventory_profile_is_precise_deterministic_and_rejects_hard_links() {
        let (root, project) = fixture();
        let request = BackupCreateRequest {
            backup_id: Default::default(),
            project_id: project.project_id,
            expected_revision: Revision::INITIAL,
            compression_mode: BackupCompression::Fast,
            exclude_vpm_packages: true,
            created_at_ms: 1,
        };
        let first =
            build_inventory(project.clone(), &request, ArchiveLimits::backup()).expect("inventory");
        let second = build_inventory(project.clone(), &request, ArchiveLimits::backup())
            .expect("inventory again");
        assert_eq!(first.fingerprint, second.fingerprint);
        let names = first
            .entries
            .iter()
            .map(|entry| entry.relative.as_str())
            .collect::<BTreeSet<_>>();
        assert!(names.contains("Assets/Empty"));
        assert!(names.contains("LibraryCache/LastSceneManagerSetup.txt"));
        assert!(names.contains("Packages/com.example.embedded/package.json"));
        assert!(
            !names
                .iter()
                .any(|name| name.starts_with("Packages/com.example.locked/"))
        );
        let link = root.join("Assets/link.txt");
        std::fs::hard_link(root.join("Assets/source.txt"), &link).expect("hard link");
        assert_eq!(
            build_inventory(project, &request, ArchiveLimits::backup())
                .expect_err("unsafe")
                .code(),
            M5BackupErrorCode::BackupSourceUnsafe
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[tokio::test]
    async fn all_compression_modes_publish_valid_v1_and_cancellation_cleans_partial() {
        for mode in [
            BackupCompression::Store,
            BackupCompression::Fast,
            BackupCompression::Maximum,
        ] {
            let (root, project) = fixture();
            let store = root.parent().expect("parent").join(format!(
                "backup-store-{}",
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            let engine = BackupEngine::new(store.clone()).expect("engine");
            let request = BackupCreateRequest {
                backup_id: Default::default(),
                project_id: project.project_id,
                expected_revision: Revision::INITIAL,
                compression_mode: mode,
                exclude_vpm_packages: true,
                created_at_ms: 1,
            };
            let inventory = engine
                .inventory(project.clone(), request.clone())
                .await
                .expect("inventory");
            let operation_id = OperationId::new();
            let evidence = engine
                .archive(
                    operation_id,
                    request.clone(),
                    inventory,
                    BackupCancellation::default(),
                )
                .await
                .expect("archive");
            let published = engine
                .publish_or_recover(operation_id, request.clone(), evidence.clone())
                .await
                .expect("publish");
            assert_eq!(
                published.archive_locator,
                format!("backup-v1:{}", request.backup_id)
            );
            verify_archive(
                &store
                    .join("objects")
                    .join(format!("{}.zip", request.backup_id)),
                &evidence,
                ArchiveLimits::backup(),
            )
            .expect("valid published archive");

            let cancelled_request = BackupCreateRequest {
                backup_id: Default::default(),
                ..request
            };
            let inventory = engine
                .inventory(project, cancelled_request.clone())
                .await
                .expect("cancel inventory");
            let cancellation = BackupCancellation::default();
            cancellation.cancel();
            let cancelled_operation = OperationId::new();
            assert_eq!(
                engine
                    .archive(
                        cancelled_operation,
                        cancelled_request,
                        inventory,
                        cancellation,
                    )
                    .await
                    .expect_err("cancelled archive")
                    .code(),
                M5BackupErrorCode::RecoveryRequired
            );
            assert!(
                !store
                    .join("partial")
                    .join(format!("{cancelled_operation}.zip.part"))
                    .exists()
            );
            std::fs::remove_dir_all(root).expect("project cleanup");
            std::fs::remove_dir_all(store).expect("store cleanup");
        }
    }

    #[test]
    fn injected_small_quota_and_locked_manifest_mismatch_fail_closed() {
        let (root, mut project) = fixture();
        let request = BackupCreateRequest {
            backup_id: Default::default(),
            project_id: project.project_id,
            expected_revision: Revision::INITIAL,
            compression_mode: BackupCompression::Fast,
            exclude_vpm_packages: true,
            created_at_ms: 1,
        };
        let mut limits = ArchiveLimits::backup();
        limits.entry_bytes = 1;
        assert_eq!(
            build_inventory(project.clone(), &request, limits)
                .expect_err("small quota")
                .code(),
            M5BackupErrorCode::BackupLimitExceeded
        );
        project.observation.locked_dependencies[0].value = "2.0.0".to_owned();
        assert_eq!(
            build_inventory(project, &request, ArchiveLimits::backup())
                .expect_err("locked mismatch")
                .code(),
            M5BackupErrorCode::ProjectChangedDuringBackup
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
