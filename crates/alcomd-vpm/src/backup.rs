use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use alcomd_application::{
    BackupArchiveEvidence, BackupCancellation, BackupCompression, BackupCreateRequest,
    BackupDestination, BackupRestoreRequest, M5BackupAdapter, M5BackupError, M5BackupErrorCode,
    OperationId, ProjectId, ProjectRecord, ResourceKey, RestoredProject, StagedBackupRestore,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive};

use crate::{
    ArchiveError, ArchiveErrorCode, ArchiveLimits, ArchivePreflight, preflight_archive_with_limits,
};

#[derive(Clone, Debug)]
pub struct BackupEngine {
    root: PathBuf,
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExcludedPackage {
    package_id: String,
    version: String,
}

impl BackupEngine {
    pub fn new(root: PathBuf) -> Result<Self, M5BackupError> {
        Self::with_limits(root, ArchiveLimits::backup())
    }

    pub fn with_limits(root: PathBuf, limits: ArchiveLimits) -> Result<Self, M5BackupError> {
        if !root.is_absolute() || root.parent().is_none() {
            return Err(restore_unsafe());
        }
        Ok(Self { root, limits })
    }
}

fn validate_destination(destination: &BackupDestination) -> Result<PathBuf, M5BackupError> {
    let (directory, identity) =
        alcomd_platform::resolve_plain_storage_directory(Path::new(&destination.directory))
            .map_err(|cause| error(M5BackupErrorCode::RecoveryRequired).with_source(cause))?;
    if identity != destination.identity || path_text(&directory)? != destination.directory {
        return Err(error(M5BackupErrorCode::RecoveryRequired));
    }
    Ok(directory)
}

fn partial_path(directory: &Path, operation_id: OperationId) -> PathBuf {
    directory.join(format!(".alcomd-{operation_id}.zip.part"))
}

fn discard_backup_partial(path: &Path) -> Result<(), M5BackupError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !is_link_or_reparse(&metadata) => {
            std::fs::remove_file(path).map_err(|cause| {
                internal()
                    .with_source(cause)
                    .with_path(path.display().to_string())
            })
        }
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err(error(M5BackupErrorCode::RecoveryRequired)),
    }
}

fn valid_archive_file_name(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
        && validate_restore_leaf(name).is_ok()
        && !name.starts_with(".alcomd-")
}

fn archive_file_name(project: &ProjectRecord, created_at_ms: u64) -> Result<String, M5BackupError> {
    let project_name = alcomd_application::project_display_name(&project.observation.root_path)
        .ok_or_else(unsafe_source)?;
    let mut name = String::new();
    for character in project_name.chars() {
        let character = if character.is_control()
            || matches!(
                character,
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
            ) {
            '_'
        } else {
            character
        };
        if name.len() + character.len_utf8() > 160 {
            break;
        }
        name.push(character);
    }
    let name = name.trim_end_matches([' ', '.']);
    let name = if name.is_empty() { "Project" } else { name };
    let timestamp = i64::try_from(created_at_ms)
        .ok()
        .and_then(chrono::DateTime::from_timestamp_millis)
        .ok_or_else(internal)?
        .with_timezone(&chrono::Local);
    Ok(format!(
        "{name}-{}.zip",
        timestamp.format("%Y-%m-%dT%H-%M-%S%.3f")
    ))
}

impl M5BackupAdapter for BackupEngine {
    type Inventory = BackupInventory;

    async fn prepare_destination(
        &self,
        directory: Option<String>,
        destination_path: Option<String>,
        create_default_directory: bool,
    ) -> Result<BackupDestination, M5BackupError> {
        let root = self.root.clone();
        tokio::task::spawn_blocking(move || {
            let (directory, file_name) = if let Some(value) = destination_path {
                let path = Path::new(&value);
                let file_name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .filter(|name| valid_archive_file_name(name))
                    .ok_or_else(restore_unsafe)?;
                if !path.is_absolute()
                    || path.file_name()
                        != path.components().next_back().map(|part| part.as_os_str())
                {
                    return Err(restore_unsafe());
                }
                (
                    path.parent().ok_or_else(restore_unsafe)?.to_owned(),
                    Some(file_name.to_owned()),
                )
            } else {
                (
                    PathBuf::from(directory.unwrap_or_else(|| root.to_string_lossy().into_owned())),
                    None,
                )
            };
            if file_name.is_none()
                && create_default_directory
                && !directory
                    .try_exists()
                    .map_err(|cause| restore_unsafe().with_source(cause))?
            {
                // Disallow links in existing ancestors before creating the product-selected root.
                let mut ancestor = directory.as_path();
                while !ancestor
                    .try_exists()
                    .map_err(|cause| restore_unsafe().with_source(cause))?
                {
                    ancestor = ancestor.parent().ok_or_else(restore_unsafe)?;
                }
                alcomd_platform::resolve_plain_storage_directory(ancestor)
                    .map_err(|cause| restore_unsafe().with_source(cause))?;
                alcomd_platform::prepare_storage_directory(&directory)
                    .map_err(|cause| restore_unsafe().with_source(cause))?;
            }
            let (path, identity) = alcomd_platform::resolve_plain_storage_directory(&directory)
                .map_err(|cause| {
                    restore_unsafe()
                        .with_source(cause)
                        .with_path(directory.display().to_string())
                })?;
            if let Some(file_name) = &file_name {
                ensure_restore_absent(&path.join(file_name))?;
            }
            Ok(BackupDestination {
                directory: path_text(&path)?,
                identity,
                file_name,
            })
        })
        .await
        .map_err(|cause| internal().with_source(cause))?
    }

    fn backup_resource(&self, destination: &BackupDestination) -> Option<ResourceKey> {
        destination
            .file_name
            .as_ref()
            .map(|file_name| target_resource(&destination.identity, file_name))
    }

    async fn inventory(
        &self,
        project: ProjectRecord,
        request: BackupCreateRequest,
    ) -> Result<Self::Inventory, M5BackupError> {
        let limits = self.limits;
        tokio::task::spawn_blocking(move || {
            let directory = validate_destination(&request.destination)?;
            let (source, _) = alcomd_platform::resolve_plain_storage_directory(Path::new(
                &project.observation.root_path,
            ))
            .map_err(|cause| unsafe_source().with_source(cause))?;
            if directory.starts_with(source) {
                return Err(unsafe_source());
            }
            ensure_restore_absent(&directory.join(output_name(&request, &project)?))?;
            build_inventory(project, &request, limits)
        })
        .await
        .map_err(|cause| internal().with_source(cause))?
    }

    async fn archive(
        &self,
        operation_id: OperationId,
        request: BackupCreateRequest,
        inventory: Self::Inventory,
        cancellation: BackupCancellation,
    ) -> Result<BackupArchiveEvidence, M5BackupError> {
        let limits = self.limits;
        tokio::task::spawn_blocking(move || {
            let directory = validate_destination(&request.destination)?;
            let path = partial_path(&directory, operation_id);
            discard_backup_partial(&path)?;
            let result = write_archive(&path, &request, &inventory, &cancellation, limits);
            if result.is_err() {
                let _ = std::fs::remove_file(&path);
            }
            result
        })
        .await
        .map_err(|cause| internal().with_source(cause))?
    }

    async fn publish_or_recover(
        &self,
        operation_id: OperationId,
        request: BackupCreateRequest,
        evidence: BackupArchiveEvidence,
    ) -> Result<String, M5BackupError> {
        let limits = self.limits;
        tokio::task::spawn_blocking(move || {
            let directory = validate_destination(&request.destination)?;
            if !valid_archive_file_name(&evidence.archive_file_name) {
                return Err(integrity());
            }
            let partial = partial_path(&directory, operation_id);
            let final_path = directory.join(&evidence.archive_file_name);
            if std::fs::symlink_metadata(&partial).is_ok() {
                verify_archive(&partial, &evidence, limits)?;
                validate_destination(&request.destination)?;
                if let Err(cause) = alcomd_platform::publish_path_no_replace(&partial, &final_path)
                {
                    if std::fs::symlink_metadata(&final_path).is_ok() {
                        discard_backup_partial(&partial)?;
                        return Err(
                            error(M5BackupErrorCode::BackupRestoreTargetExists).with_source(cause)
                        );
                    }
                    return Err(error(M5BackupErrorCode::RecoveryRequired).with_source(cause));
                }
            }
            alcomd_platform::sync_directory(&directory)
                .map_err(|cause| error(M5BackupErrorCode::RecoveryRequired).with_source(cause))?;
            verify_archive(&final_path, &evidence, limits)?;
            Ok(evidence.archive_file_name)
        })
        .await
        .map_err(|cause| internal().with_source(cause))?
    }

    async fn discard_partial(
        &self,
        operation_id: OperationId,
        request: BackupCreateRequest,
    ) -> Result<(), M5BackupError> {
        tokio::task::spawn_blocking(move || {
            let directory = validate_destination(&request.destination)?;
            discard_backup_partial(&partial_path(&directory, operation_id))
        })
        .await
        .map_err(|cause| internal().with_source(cause))?
    }

    async fn prepare_restore(
        &self,
        archive_path: String,
        target: PathBuf,
    ) -> Result<BackupRestoreRequest, M5BackupError> {
        let engine = self.clone();
        tokio::task::spawn_blocking(move || prepare_restore(&engine, &archive_path, &target))
            .await
            .map_err(|cause| internal().with_source(cause))?
    }

    fn restore_resource(&self, request: &BackupRestoreRequest) -> ResourceKey {
        target_resource(&request.target_parent_identity, &request.target_name)
    }

    async fn stage_restore(
        &self,
        operation_id: OperationId,
        request: BackupRestoreRequest,
        cancellation: BackupCancellation,
    ) -> Result<StagedBackupRestore, M5BackupError> {
        let limits = self.limits;
        tokio::task::spawn_blocking(move || {
            stage_restore(operation_id, request, limits, &cancellation)
        })
        .await
        .map_err(|cause| internal().with_source(cause))?
    }

    async fn discard_staged_restore(
        &self,
        operation_id: OperationId,
        request: BackupRestoreRequest,
    ) -> Result<(), M5BackupError> {
        tokio::task::spawn_blocking(move || discard_staged_restore(operation_id, &request))
            .await
            .map_err(|cause| internal().with_source(cause))?
    }

    async fn publish_restore(
        &self,
        staged: StagedBackupRestore,
    ) -> Result<RestoredProject, M5BackupError> {
        tokio::task::spawn_blocking(move || publish_restore(staged))
            .await
            .map_err(|cause| internal().with_source(cause))?
    }

    async fn validate_published_restore(
        &self,
        operation_id: OperationId,
        request: BackupRestoreRequest,
    ) -> Result<RestoredProject, M5BackupError> {
        tokio::task::spawn_blocking(move || validate_published_restore(operation_id, &request))
            .await
            .map_err(|cause| internal().with_source(cause))?
    }

    async fn finalize_restore(
        &self,
        operation_id: OperationId,
        request: BackupRestoreRequest,
    ) -> Result<(), M5BackupError> {
        tokio::task::spawn_blocking(move || finalize_restore(operation_id, &request))
            .await
            .map_err(|cause| internal().with_source(cause))?
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RestoreOwnerMarker {
    operation_id: OperationId,
    project_id: ProjectId,
    staging_identity: Vec<u8>,
    publication: Option<RestorePublication>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RestorePublication {
    directory_identity: Vec<u8>,
    project_fingerprint: [u8; 32],
}

fn target_resource(identity: &[u8], name: &str) -> ResourceKey {
    ResourceKey::ProjectCreate {
        parent_identity_sha256: digest(identity),
        target_leaf: name.to_owned(),
    }
}

fn output_name(
    request: &BackupCreateRequest,
    project: &ProjectRecord,
) -> Result<String, M5BackupError> {
    let name = if let Some(name) = &request.destination.file_name {
        name.clone()
    } else {
        archive_file_name(project, request.created_at_ms)?
    };
    if !valid_archive_file_name(&name) {
        return Err(restore_unsafe());
    }
    Ok(name)
}

fn prepare_restore(
    engine: &BackupEngine,
    archive_path: &str,
    target: &Path,
) -> Result<BackupRestoreRequest, M5BackupError> {
    let target_name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(restore_unsafe)?;
    validate_restore_leaf(target_name)?;
    if target_name.starts_with(".alcomd-") {
        return Err(restore_unsafe());
    }
    let (target_parent, target_parent_identity) = alcomd_platform::resolve_plain_storage_directory(
        target.parent().ok_or_else(restore_unsafe)?,
    )
    .map_err(|cause| {
        restore_unsafe()
            .with_source(cause)
            .with_path(target.display().to_string())
    })?;
    let data_root = engine.root.parent().ok_or_else(restore_unsafe)?;
    let (data_root, _) = alcomd_platform::resolve_plain_storage_directory(data_root)
        .map_err(|cause| restore_unsafe().with_source(cause))?;
    if target_parent.starts_with(data_root) || has_unity_project_ancestor(&target_parent) {
        return Err(restore_unsafe());
    }
    ensure_restore_absent(&target_parent.join(target_name))?;

    let archive = Path::new(archive_path);
    let name = archive
        .file_name()
        .and_then(|leaf| leaf.to_str())
        .filter(|leaf| valid_archive_file_name(leaf))
        .ok_or_else(unsafe_source)?;
    let (archive_parent, archive_parent_identity) =
        alcomd_platform::resolve_plain_storage_directory(
            archive.parent().ok_or_else(unsafe_source)?,
        )
        .map_err(|cause| archive_path_error(archive, cause))?;
    let archive_path = archive_parent.join(name);
    let (mut file, archive_file_identity, metadata) =
        open_restore_source(&archive_path, engine.limits)?;
    let archive_directory_fingerprint = archive_directory_fingerprint(&mut file, engine.limits)?;
    let archive_modified_at = metadata
        .modified()
        .map_err(|cause| stale().with_source(cause))?;
    let request = BackupRestoreRequest {
        project_id: ProjectId::new(),
        archive_path,
        archive_parent_identity,
        archive_directory_fingerprint,
        archive_file_identity,
        archive_bytes: metadata.len(),
        archive_modified_at,
        target_parent,
        target_parent_identity,
        target_name: target_name.to_owned(),
    };
    verify_restore_source(&request)?;
    verify_restore_parent(&request)?;
    Ok(request)
}

fn validate_project_layout(archive: &ArchivePreflight) -> Result<(), ArchiveErrorCode> {
    let mut assets = false;
    let mut version = false;
    let mut manifest = false;
    for entry in &archive.entries {
        let path = &entry.relative_path;
        assets |= path.starts_with("Assets") && (path != Path::new("Assets") || entry.directory);
        version |= path == Path::new("ProjectSettings/ProjectVersion.txt") && !entry.directory;
        manifest |= path == Path::new("Packages/manifest.json") && !entry.directory;
    }
    if assets && version && manifest {
        Ok(())
    } else {
        Err(ArchiveErrorCode::Invalid)
    }
}

fn open_restore_source(
    path: &Path,
    limits: ArchiveLimits,
) -> Result<(File, Vec<u8>, std::fs::Metadata), M5BackupError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|cause| archive_path_error(path, cause))?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) {
        return Err(unsafe_source().with_path(path.display().to_string()));
    }
    verify_single_link(path, &metadata)?;
    if metadata.len() > limits.archive_bytes {
        return Err(limit());
    }
    let identity = alcomd_platform::file_identity_key(path)
        .map_err(|cause| archive_path_error(path, cause))?;
    let (file, opened_identity) =
        alcomd_platform::open_plain_file(path).map_err(|cause| archive_path_error(path, cause))?;
    let opened = file
        .metadata()
        .map_err(|cause| archive_path_error(path, cause))?;
    if !opened.is_file()
        || is_link_or_reparse(&opened)
        || opened.len() != metadata.len()
        || opened_identity != identity
        || alcomd_platform::file_identity_key(path)
            .map_err(|cause| archive_path_error(path, cause))?
            != identity
    {
        return Err(stale());
    }
    verify_single_link(path, &opened)?;
    Ok((file, identity, opened))
}

fn verify_restore_source(request: &BackupRestoreRequest) -> Result<(), M5BackupError> {
    let (parent, identity) = alcomd_platform::resolve_plain_storage_directory(
        request.archive_path.parent().ok_or_else(stale)?,
    )
    .map_err(|cause| stale().with_source(cause))?;
    let metadata = std::fs::symlink_metadata(&request.archive_path)
        .map_err(|cause| stale().with_source(cause))?;
    if request.archive_path.parent() != Some(parent.as_path())
        || identity != request.archive_parent_identity
        || !metadata.is_file()
        || is_link_or_reparse(&metadata)
        || metadata.len() != request.archive_bytes
        || metadata
            .modified()
            .map_err(|cause| stale().with_source(cause))?
            != request.archive_modified_at
        || alcomd_platform::file_identity_key(&request.archive_path)
            .map_err(|cause| stale().with_source(cause))?
            != request.archive_file_identity
    {
        return Err(stale());
    }
    verify_single_link(&request.archive_path, &metadata)
}

fn archive_path_error(path: &Path, cause: std::io::Error) -> M5BackupError {
    let code = if cause.kind() == std::io::ErrorKind::NotFound {
        M5BackupErrorCode::BackupNotFound
    } else {
        M5BackupErrorCode::BackupSourceUnsafe
    };
    error(code)
        .with_source(cause)
        .with_path(path.display().to_string())
}

fn archive_error(cause: ArchiveError) -> M5BackupError {
    let code = match cause.code() {
        ArchiveErrorCode::QuotaExceeded => M5BackupErrorCode::BackupLimitExceeded,
        ArchiveErrorCode::Io => M5BackupErrorCode::BackupIntegrityMismatch,
        _ => M5BackupErrorCode::BackupInvalidArchive,
    };
    error(code).with_source(cause)
}

fn archive_directory_fingerprint(
    file: &mut File,
    limits: ArchiveLimits,
) -> Result<[u8; 32], M5BackupError> {
    file.rewind()
        .map_err(|cause| integrity().with_source(cause))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|cause| error(M5BackupErrorCode::BackupInvalidArchive).with_source(cause))?;
    let preflight = crate::archive::preflight_zip(&mut archive, limits).map_err(archive_error)?;
    validate_project_layout(&preflight)
        .map_err(|_| error(M5BackupErrorCode::BackupInvalidArchive))?;
    let mut hash = Sha256::new();
    for planned in &preflight.entries {
        let entry = archive
            .by_index(planned.index)
            .map_err(|cause| integrity().with_source(cause))?;
        hash_field(&mut hash, entry.name_raw())?;
        hash.update([u8::from(planned.directory)]);
        hash.update(planned.uncompressed_size.to_le_bytes());
        hash.update(planned.compressed_size.to_le_bytes());
        hash.update(entry.crc32().to_le_bytes());
    }
    Ok(hash.finalize().into())
}

fn stage_restore(
    operation_id: OperationId,
    request: BackupRestoreRequest,
    limits: ArchiveLimits,
    cancellation: &BackupCancellation,
) -> Result<StagedBackupRestore, M5BackupError> {
    verify_restore_parent(&request)?;
    let mut staged = StagedBackupRestore {
        staging_root: request
            .target_parent
            .join(format!(".alcomd-restore-{operation_id}")),
        project_root: request
            .target_parent
            .join(format!(".alcomd-restore-{operation_id}"))
            .join("project"),
        target_root: request.target_parent.join(&request.target_name),
        owner_sidecar: request
            .target_parent
            .join(format!(".alcomd-restore-{operation_id}.owner.json")),
        request,
        already_published: false,
        project_fingerprint: [0; 32],
    };
    if staged
        .target_root
        .try_exists()
        .map_err(|cause| recovery().with_source(cause))?
    {
        if !staged
            .owner_sidecar
            .try_exists()
            .map_err(|cause| recovery().with_source(cause))?
        {
            return Err(error(M5BackupErrorCode::BackupRestoreTargetExists));
        }
        let restored = validate_published_restore(operation_id, &staged.request)?;
        staged.project_fingerprint = restored.project_fingerprint;
        staged.already_published = true;
        return Ok(staged);
    }
    discard_staged_restore(operation_id, &staged.request)?;
    verify_restore_source(&staged.request)?;
    let (mut file, identity, metadata) = open_restore_source(&staged.request.archive_path, limits)?;
    if identity != staged.request.archive_file_identity
        || metadata.len() != staged.request.archive_bytes
        || metadata
            .modified()
            .map_err(|cause| stale().with_source(cause))?
            != staged.request.archive_modified_at
        || archive_directory_fingerprint(&mut file, limits)?
            != staged.request.archive_directory_fingerprint
    {
        return Err(stale());
    }
    let archive_sha256 = hash_restore_file(&mut file, Some(cancellation))?;
    verify_restore_parent(&staged.request)?;
    // An empty directory may be discarded if the process stops before writing ownership.
    // No project content is extracted until the staging identity is durable.
    std::fs::create_dir(&staged.staging_root).map_err(|cause| recovery().with_source(cause))?;
    let staging_identity = alcomd_platform::file_identity_key(&staged.staging_root)
        .map_err(|cause| recovery().with_source(cause))?;
    let result = (|| {
        write_restore_owner(
            &staged.owner_sidecar,
            operation_id,
            &staged.request,
            staging_identity,
            None,
        )?;
        std::fs::create_dir(&staged.project_root).map_err(|cause| recovery().with_source(cause))?;
        crate::archive::extract_archive_from_file(
            &mut file,
            &staged.project_root,
            limits,
            validate_project_layout,
            || cancellation.cancelled(),
        )
        .map_err(archive_error)?;
        if hash_restore_file(&mut file, Some(cancellation))? != archive_sha256
            || archive_directory_fingerprint(&mut file, limits)?
                != staged.request.archive_directory_fingerprint
        {
            return Err(stale());
        }
        verify_restore_source(&staged.request)?;
        validate_restore_project(&staged.project_root)?;
        tree_fingerprint(&staged.project_root, Some(cancellation))
    })();
    staged.project_fingerprint = match result {
        Ok(fingerprint) => fingerprint,
        Err(source) => {
            discard_staged_restore(operation_id, &staged.request)?;
            return Err(source);
        }
    };
    Ok(staged)
}

fn discard_staged_restore(
    operation_id: OperationId,
    request: &BackupRestoreRequest,
) -> Result<(), M5BackupError> {
    verify_restore_parent(request)?;
    let staging = request
        .target_parent
        .join(format!(".alcomd-restore-{operation_id}"));
    remove_restore_staging(
        &staging,
        &staging.with_extension("owner.json"),
        operation_id,
        request,
    )
}

fn publish_restore(staged: StagedBackupRestore) -> Result<RestoredProject, M5BackupError> {
    let marker = read_restore_owner(&staged.owner_sidecar)?;
    let operation_id = marker.operation_id;
    validate_restore_owner(&staged.owner_sidecar, operation_id, &staged.request)?;
    if !staged.already_published {
        verify_restore_parent(&staged.request)?;
        if let Err(source) = ensure_restore_absent(&staged.target_root) {
            if source.code() == M5BackupErrorCode::BackupRestoreTargetExists {
                discard_staged_restore(operation_id, &staged.request)?;
            }
            return Err(source);
        }
        let identity = alcomd_platform::file_identity_key(&staged.project_root)
            .map_err(|cause| recovery().with_source(cause))?;
        if alcomd_platform::file_identity_key(&staged.staging_root)
            .map_err(|cause| recovery().with_source(cause))?
            != marker.staging_identity
            || tree_fingerprint(&staged.project_root, None)? != staged.project_fingerprint
        {
            return Err(recovery());
        }
        let publication_marker = staged.owner_sidecar.with_extension("publish.json");
        write_restore_owner(
            &publication_marker,
            operation_id,
            &staged.request,
            marker.staging_identity,
            Some(RestorePublication {
                directory_identity: identity,
                project_fingerprint: staged.project_fingerprint,
            }),
        )?;
        if let Err(cause) =
            alcomd_platform::publish_path_no_replace(&staged.project_root, &staged.target_root)
        {
            if std::fs::symlink_metadata(&staged.target_root).is_ok() {
                discard_staged_restore(operation_id, &staged.request)?;
                return Err(error(M5BackupErrorCode::BackupRestoreTargetExists).with_source(cause));
            }
            return Err(recovery().with_source(cause));
        }
        alcomd_platform::sync_directory(&staged.request.target_parent)
            .map_err(|cause| recovery().with_source(cause))?;
    }
    validate_published_restore(operation_id, &staged.request)
}

fn validate_published_restore(
    operation_id: OperationId,
    request: &BackupRestoreRequest,
) -> Result<RestoredProject, M5BackupError> {
    verify_restore_parent(request)?;
    let target = request.target_parent.join(&request.target_name);
    let owner = request
        .target_parent
        .join(format!(".alcomd-restore-{operation_id}.owner.json"));
    let publication_marker = owner.with_extension("publish.json");
    let identity = alcomd_platform::file_identity_key(&target)
        .map_err(|cause| recovery().with_source(cause))?;
    let published = validate_restore_owner(&publication_marker, operation_id, request)?
        .publication
        .ok_or_else(recovery)?;
    if published.directory_identity != identity {
        return Err(recovery());
    }
    let fingerprint =
        tree_fingerprint(&target, None).map_err(|cause| recovery().with_source(cause))?;
    if fingerprint != published.project_fingerprint {
        return Err(recovery());
    }
    let observation = validate_restore_project(&target)?;
    if observation.path_identity_key != identity {
        return Err(recovery());
    }
    Ok(RestoredProject {
        project_id: request.project_id,
        observation,
        project_fingerprint: fingerprint,
    })
}

fn finalize_restore(
    operation_id: OperationId,
    request: &BackupRestoreRequest,
) -> Result<(), M5BackupError> {
    // The registration already committed. Cleanup only removes proven operation-owned
    // files and can safely resume even if Unity has since modified the registered project.
    discard_staged_restore(operation_id, request)
}

fn validate_restore_project(
    root: &Path,
) -> Result<alcomd_application::ProjectObservation, M5BackupError> {
    let (root, identity) = alcomd_platform::resolve_plain_storage_directory(root)
        .map_err(|cause| recovery().with_source(cause))?;
    let reader = crate::VpmReader::new().map_err(|cause| {
        internal()
            .with_source(cause)
            .with_path(root.display().to_string())
    })?;
    tokio::runtime::Handle::current()
        .block_on(reader.inspect_project_root(root.clone(), identity))
        .map_err(|cause| {
            error(M5BackupErrorCode::BackupInvalidArchive)
                .with_source(cause)
                .with_path(root.display().to_string())
        })
}

fn verify_restore_parent(request: &BackupRestoreRequest) -> Result<(), M5BackupError> {
    let (parent, identity) =
        alcomd_platform::resolve_plain_storage_directory(&request.target_parent)
            .map_err(|cause| restore_unsafe().with_source(cause))?;
    if parent != request.target_parent || identity != request.target_parent_identity {
        return Err(restore_unsafe());
    }
    Ok(())
}

fn tree_fingerprint(
    root: &Path,
    cancellation: Option<&BackupCancellation>,
) -> Result<[u8; 32], M5BackupError> {
    let metadata =
        std::fs::symlink_metadata(root).map_err(|cause| recovery().with_source(cause))?;
    if !metadata.is_dir() || is_link_or_reparse(&metadata) {
        return Err(recovery());
    }
    let mut entries = Vec::new();
    collect_tree_entries(root, root, &mut entries, cancellation)?;
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = Sha256::new();
    for (relative, directory, content) in entries {
        hash_field(&mut hasher, relative.as_bytes())?;
        hasher.update([u8::from(directory)]);
        hasher.update(content);
    }
    Ok(hasher.finalize().into())
}

fn collect_tree_entries(
    root: &Path,
    directory: &Path,
    entries: &mut Vec<(String, bool, [u8; 32])>,
    cancellation: Option<&BackupCancellation>,
) -> Result<(), M5BackupError> {
    let mut children = std::fs::read_dir(directory)
        .map_err(|cause| recovery().with_source(cause))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|cause| recovery().with_source(cause))?;
    children.sort_by_key(std::fs::DirEntry::file_name);
    for child in children {
        if cancellation.is_some_and(BackupCancellation::cancelled) {
            return Err(integrity());
        }
        let path = child.path();
        let metadata =
            std::fs::symlink_metadata(&path).map_err(|cause| recovery().with_source(cause))?;
        if is_link_or_reparse(&metadata) || (!metadata.is_dir() && !metadata.is_file()) {
            return Err(recovery());
        }
        let relative = normalized_relative(root, &path)?;
        if metadata.is_dir() {
            entries.push((relative, true, digest(&[])));
            collect_tree_entries(root, &path, entries, cancellation)?;
        } else {
            let (mut file, _) = alcomd_platform::open_plain_file(&path)
                .map_err(|cause| recovery().with_source(cause))?;
            entries.push((relative, false, hash_restore_file(&mut file, cancellation)?));
        }
    }
    Ok(())
}

fn validate_restore_leaf(value: &str) -> Result<(), M5BackupError> {
    let stem = value
        .split('.')
        .next()
        .unwrap_or(value)
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|suffix| suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9'));
    if value.is_empty()
        || value.len() > 255
        || matches!(value, "." | "..")
        || value.ends_with([' ', '.'])
        || value.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '<' | '>' | '"' | '/' | '\\' | ':' | '|' | '?' | '*'
                )
        })
        || reserved
    {
        return Err(error(M5BackupErrorCode::InvalidInput));
    }
    Ok(())
}

fn has_unity_project_ancestor(path: &Path) -> bool {
    path.ancestors().any(|ancestor| {
        ancestor
            .join("ProjectSettings/ProjectVersion.txt")
            .is_file()
            && ancestor.join("Packages/manifest.json").is_file()
    })
}

fn ensure_restore_absent(path: &Path) -> Result<(), M5BackupError> {
    match std::fs::symlink_metadata(path) {
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(error(M5BackupErrorCode::BackupRestoreTargetExists)),
        Err(cause) => Err(restore_unsafe().with_source(cause)),
    }
}

fn write_restore_owner(
    path: &Path,
    operation_id: OperationId,
    request: &BackupRestoreRequest,
    staging_identity: Vec<u8>,
    publication: Option<RestorePublication>,
) -> Result<(), M5BackupError> {
    let bytes = serde_json::to_vec(&RestoreOwnerMarker {
        operation_id,
        project_id: request.project_id,
        staging_identity,
        publication,
    })
    .map_err(|cause| {
        internal()
            .with_source(cause)
            .with_path(path.display().to_string())
    })?;
    let partial = path.with_extension("partial");
    discard_backup_partial(&partial)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&partial)
        .map_err(|cause| {
            recovery()
                .with_source(cause)
                .with_path(path.display().to_string())
        })?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|cause| {
            recovery()
                .with_source(cause)
                .with_path(path.display().to_string())
        })?;
    drop(file);
    alcomd_platform::publish_path_no_replace(&partial, path).map_err(|cause| {
        recovery()
            .with_source(cause)
            .with_path(path.display().to_string())
    })?;
    alcomd_platform::sync_directory(path.parent().ok_or_else(recovery)?).map_err(|cause| {
        recovery()
            .with_source(cause)
            .with_path(path.display().to_string())
    })
}

fn read_restore_owner(path: &Path) -> Result<RestoreOwnerMarker, M5BackupError> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|cause| recovery().with_source(cause))?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) || metadata.len() > 4_096 {
        return Err(recovery());
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|cause| recovery().with_source(cause))?)
        .map_err(|cause| recovery().with_source(cause))
}

fn validate_restore_owner(
    path: &Path,
    operation_id: OperationId,
    request: &BackupRestoreRequest,
) -> Result<RestoreOwnerMarker, M5BackupError> {
    let marker = read_restore_owner(path)?;
    if marker.operation_id != operation_id || marker.project_id != request.project_id {
        return Err(recovery());
    }
    Ok(marker)
}

fn remove_restore_staging(
    staging: &Path,
    sidecar: &Path,
    operation_id: OperationId,
    request: &BackupRestoreRequest,
) -> Result<(), M5BackupError> {
    let publication = sidecar.with_extension("publish.json");
    let marker = match std::fs::symlink_metadata(sidecar) {
        Ok(_) => Some(validate_restore_owner(sidecar, operation_id, request)?),
        Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => None,
        Err(cause) => return Err(recovery().with_source(cause)),
    };
    match std::fs::symlink_metadata(staging) {
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
        Ok(metadata) if metadata.is_dir() && !is_link_or_reparse(&metadata) => {
            if let Some(marker) = &marker {
                if alcomd_platform::file_identity_key(staging)
                    .map_err(|cause| recovery().with_source(cause))?
                    != marker.staging_identity
                {
                    return Err(recovery());
                }
                std::fs::remove_dir_all(staging).map_err(|cause| recovery().with_source(cause))?;
            } else {
                std::fs::remove_dir(staging).map_err(|cause| recovery().with_source(cause))?;
            }
        }
        _ => return Err(recovery()),
    }
    if publication
        .try_exists()
        .map_err(|cause| recovery().with_source(cause))?
    {
        validate_restore_owner(&publication, operation_id, request)?;
        std::fs::remove_file(&publication).map_err(|cause| recovery().with_source(cause))?;
    }
    if marker.is_some() {
        std::fs::remove_file(sidecar).map_err(|cause| recovery().with_source(cause))?;
    }
    discard_backup_partial(&sidecar.with_extension("partial"))?;
    discard_backup_partial(&publication.with_extension("partial"))?;
    alcomd_platform::sync_directory(&request.target_parent)
        .map_err(|cause| recovery().with_source(cause))
}

fn path_text(path: &Path) -> Result<String, M5BackupError> {
    path.to_str().map(str::to_owned).ok_or_else(restore_unsafe)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn integrity() -> M5BackupError {
    error(M5BackupErrorCode::BackupIntegrityMismatch)
}

fn stale() -> M5BackupError {
    error(M5BackupErrorCode::BackupSourceChanged)
}

fn restore_unsafe() -> M5BackupError {
    error(M5BackupErrorCode::BackupRestoreTargetUnsafe)
}

fn recovery() -> M5BackupError {
    error(M5BackupErrorCode::BackupRestoreRecoveryRequired)
}

fn build_inventory(
    project: ProjectRecord,
    request: &BackupCreateRequest,
    limits: ArchiveLimits,
) -> Result<BackupInventory, M5BackupError> {
    let root = PathBuf::from(&project.observation.root_path);
    if !root.is_absolute()
        || alcomd_platform::file_identity_key(&root)
            .map_err(|cause| unsafe_source().with_source(cause))?
            != project.observation.path_identity_key
    {
        return Err(unsafe_source());
    }
    let root_metadata =
        std::fs::symlink_metadata(&root).map_err(|cause| unsafe_source().with_source(cause))?;
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
    walk(
        &root,
        &root,
        &excluded_ids,
        request.include_library,
        &mut entries,
        limits,
    )?;
    entries.sort_by(|left, right| left.relative.cmp(&right.relative));
    if entries.len() > limits.entries {
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
        let bytes = read_project_manifest(&root.join(manifest))?;
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
    include_library: bool,
    entries: &mut Vec<InventoryEntry>,
    limits: ArchiveLimits,
) -> Result<(), M5BackupError> {
    let mut children = std::fs::read_dir(path)
        .map_err(|cause| unsafe_source().with_source(cause))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|cause| unsafe_source().with_source(cause))?;
    children.sort_by_key(std::fs::DirEntry::file_name);
    for child in children {
        let child_path = child.path();
        let relative = normalized_relative(root, &child_path)?;
        let metadata = std::fs::symlink_metadata(&child_path)
            .map_err(|cause| unsafe_source().with_source(cause))?;
        if is_link_or_reparse(&metadata) {
            return Err(unsafe_source());
        }
        if metadata.is_dir() {
            if excluded_by_profile(&relative, excluded_packages, include_library) {
                if preserve_library_scene(&relative) {
                    let mut direct = std::fs::read_dir(&child_path)
                        .map_err(|cause| unsafe_source().with_source(cause))?
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|cause| unsafe_source().with_source(cause))?;
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
            walk(
                root,
                &child_path,
                excluded_packages,
                include_library,
                entries,
                limits,
            )?;
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
    let metadata =
        std::fs::symlink_metadata(path).map_err(|cause| unsafe_source().with_source(cause))?;
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
        .map_err(|cause| {
            unsafe_source()
                .with_source(cause)
                .with_path(path.display().to_string())
        })?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|cause| {
            unsafe_source()
                .with_source(cause)
                .with_path(path.display().to_string())
        })?
        .as_nanos();
    Ok(InventoryEntry {
        relative,
        path: path.to_path_buf(),
        kind,
        identity: alcomd_platform::file_identity_key(path).map_err(|cause| {
            unsafe_source()
                .with_source(cause)
                .with_path(path.display().to_string())
        })?,
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
        .map_err(|cause| {
            internal()
                .with_source(cause)
                .with_path(path.display().to_string())
        })?;
    let method = match request.compression_mode {
        BackupCompression::Store => CompressionMethod::Stored,
        BackupCompression::Fast
        | BackupCompression::Level3
        | BackupCompression::Level5
        | BackupCompression::Level7
        | BackupCompression::Maximum => CompressionMethod::Deflated,
    };
    let level = match request.compression_mode {
        BackupCompression::Store => None,
        BackupCompression::Fast => Some(1),
        BackupCompression::Level3 => Some(3),
        BackupCompression::Level5 => Some(5),
        BackupCompression::Level7 => Some(7),
        BackupCompression::Maximum => Some(9),
    };
    let file_options = SimpleFileOptions::default()
        .compression_method(method)
        .compression_level(level)
        .unix_permissions(0o600);
    let dir_options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .unix_permissions(0o700);
    let mut zip = zip::ZipWriter::new(BoundedFile {
        file: output,
        limit: limits.archive_bytes,
    });
    let mut buffer = [0_u8; 64 * 1024];
    let mut archiving_gate_reached = false;
    for entry in &inventory.entries {
        if cancellation.cancelled() {
            return Err(error(M5BackupErrorCode::RecoveryRequired));
        }
        let name = format!(
            "{}{}",
            entry.relative,
            if entry.kind == EntryKind::Directory {
                "/"
            } else {
                ""
            }
        );
        if entry.kind == EntryKind::Directory {
            zip.add_directory(name, dir_options).map_err(|cause| {
                internal()
                    .with_source(cause)
                    .with_path(path.display().to_string())
            })?;
            continue;
        }
        let before = evidence_for(&entry.path)?;
        if before != (entry.identity.clone(), entry.bytes, entry.modified_ns) {
            return Err(changed());
        }
        zip.start_file(
            name,
            file_options.large_file(entry.bytes >= u64::from(u32::MAX)),
        )
        .map_err(|cause| {
            internal()
                .with_source(cause)
                .with_path(path.display().to_string())
        })?;
        let (mut input, opened_identity) =
            alcomd_platform::open_plain_file(&entry.path).map_err(|cause| {
                changed()
                    .with_source(cause)
                    .with_path(entry.path.display().to_string())
            })?;
        if opened_identity != entry.identity {
            return Err(changed());
        }
        let mut remaining = entry.bytes;
        while remaining > 0 {
            if cancellation.cancelled() {
                return Err(error(M5BackupErrorCode::RecoveryRequired));
            }
            let maximum = remaining.min(buffer.len() as u64) as usize;
            let read = input
                .read(&mut buffer[..maximum])
                .map_err(|cause| changed().with_source(cause))?;
            if read == 0 {
                return Err(changed());
            }
            zip.write_all(&buffer[..read]).map_err(|cause| {
                internal()
                    .with_source(cause)
                    .with_path(path.display().to_string())
            })?;
            remaining -= read as u64;
            if !archiving_gate_reached {
                archiving_gate_reached = true;
                kill_gate("archiving");
            }
        }
        if input
            .read(&mut buffer[..1])
            .map_err(|cause| changed().with_source(cause))?
            != 0
            || evidence_for(&entry.path)? != before
        {
            return Err(changed());
        }
    }
    let output = zip.finish().map_err(|cause| limit().with_source(cause))?;
    output.file.sync_all().map_err(|cause| {
        internal()
            .with_source(cause)
            .with_path(path.display().to_string())
    })?;
    let final_inventory = build_inventory(inventory.project.clone(), request, limits)?;
    if final_inventory.fingerprint != inventory.fingerprint
        || final_inventory.excluded_packages != inventory.excluded_packages
    {
        return Err(changed());
    }
    validate_flat_archive(path, limits)?;
    let metadata = std::fs::metadata(path).map_err(|cause| {
        internal()
            .with_source(cause)
            .with_path(path.display().to_string())
    })?;
    if metadata.len() > limits.archive_bytes {
        return Err(limit());
    }
    Ok(BackupArchiveEvidence {
        archive_sha256: hash_file(path)?,
        archive_bytes: metadata.len(),
        source_project_fingerprint: inventory.fingerprint,
        archive_file_name: output_name(request, &inventory.project)?,
        archive_file_identity: alcomd_platform::file_identity_key(path)
            .map_err(|cause| internal().with_source(cause))?,
    })
}

fn validate_flat_archive(path: &Path, limits: ArchiveLimits) -> Result<(), M5BackupError> {
    let preflight = preflight_archive_with_limits(path, limits).map_err(archive_error)?;
    validate_project_layout(&preflight).map_err(|_| error(M5BackupErrorCode::BackupInvalidArchive))
}

fn verify_archive(
    path: &Path,
    evidence: &BackupArchiveEvidence,
    limits: ArchiveLimits,
) -> Result<(), M5BackupError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|cause| {
        error(M5BackupErrorCode::RecoveryRequired)
            .with_source(cause)
            .with_path(path.display().to_string())
    })?;
    if !metadata.is_file()
        || is_link_or_reparse(&metadata)
        || metadata.len() != evidence.archive_bytes
        || alcomd_platform::file_identity_key(path)
            .map_err(|cause| integrity().with_source(cause))?
            != evidence.archive_file_identity
        || hash_file(path)? != evidence.archive_sha256
    {
        return Err(error(M5BackupErrorCode::BackupIntegrityMismatch));
    }
    validate_flat_archive(path, limits)
}

fn read_project_manifest(path: &Path) -> Result<Vec<u8>, M5BackupError> {
    const MANIFEST_LIMIT: u64 = 4 * 1024 * 1024;
    alcomd_platform::resolve_plain_storage_directory(path.parent().ok_or_else(unsafe_source)?)
        .map_err(|cause| unsafe_source().with_source(cause))?;
    let (file, identity) = alcomd_platform::open_plain_file(path).map_err(|cause| {
        unsafe_source()
            .with_source(cause)
            .with_path(path.display().to_string())
    })?;
    let metadata = file
        .metadata()
        .map_err(|cause| unsafe_source().with_source(cause))?;
    verify_single_link(path, &metadata)?;
    if metadata.len() > MANIFEST_LIMIT {
        return Err(limit());
    }
    let mut bytes = Vec::new();
    file.take(MANIFEST_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|cause| unsafe_source().with_source(cause))?;
    if bytes.len() as u64 > MANIFEST_LIMIT {
        return Err(limit());
    }
    if alcomd_platform::file_identity_key(path).map_err(|cause| changed().with_source(cause))?
        != identity
    {
        return Err(changed());
    }
    Ok(bytes)
}

fn validated_exclusions(
    root: &Path,
    project: &ProjectRecord,
) -> Result<Vec<ExcludedPackage>, M5BackupError> {
    let manifest_bytes = read_project_manifest(&root.join("Packages/vpm-manifest.json"))?;
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes).map_err(|cause| {
        unsafe_source()
            .with_source(cause)
            .with_path(root.display().to_string())
    })?;
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
        let metadata = std::fs::symlink_metadata(&path).map_err(|cause| {
            unsafe_source()
                .with_source(cause)
                .with_path(root.display().to_string())
        })?;
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

fn excluded_by_profile(relative: &str, packages: &BTreeSet<&str>, include_library: bool) -> bool {
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
                && !(include_library && root.eq_ignore_ascii_case("Library"))
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
    let relative = path
        .strip_prefix(root)
        .map_err(|cause| unsafe_source().with_source(cause))?;
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
    Ok(value.nfc().collect())
}

fn evidence_for(path: &Path) -> Result<(Vec<u8>, u64, u128), M5BackupError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|cause| changed().with_source(cause))?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) {
        return Err(unsafe_source());
    }
    verify_single_link(path, &metadata)?;
    let modified = metadata
        .modified()
        .map_err(|cause| changed().with_source(cause))?
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|cause| changed().with_source(cause))?
        .as_nanos();
    Ok((
        alcomd_platform::file_identity_key(path).map_err(|cause| changed().with_source(cause))?,
        metadata.len(),
        modified,
    ))
}

fn hash_file(path: &Path) -> Result<[u8; 32], M5BackupError> {
    let (mut file, _) = alcomd_platform::open_plain_file(path).map_err(|cause| {
        internal()
            .with_source(cause)
            .with_path(path.display().to_string())
    })?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|cause| {
            internal()
                .with_source(cause)
                .with_path(path.display().to_string())
        })?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(hash.finalize().into())
}

fn hash_restore_file(
    file: &mut File,
    cancellation: Option<&BackupCancellation>,
) -> Result<[u8; 32], M5BackupError> {
    file.rewind()
        .map_err(|cause| integrity().with_source(cause))?;
    let mut hash = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        if cancellation.is_some_and(BackupCancellation::cancelled) {
            return Err(integrity());
        }
        let read = file
            .read(&mut buffer)
            .map_err(|cause| integrity().with_source(cause))?;
        if read == 0 {
            file.rewind()
                .map_err(|cause| integrity().with_source(cause))?;
            return Ok(hash.finalize().into());
        }
        hash.update(&buffer[..read]);
    }
}

fn hash_field(hash: &mut Sha256, bytes: &[u8]) -> Result<(), M5BackupError> {
    hash.update(
        u32::try_from(bytes.len())
            .map_err(|cause| limit().with_source(cause))?
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

fn error(code: M5BackupErrorCode) -> M5BackupError {
    M5BackupError::new(code)
}
fn internal() -> M5BackupError {
    error(M5BackupErrorCode::Internal)
}
fn unsafe_source() -> M5BackupError {
    error(M5BackupErrorCode::BackupSourceUnsafe)
}
fn limit() -> M5BackupError {
    error(M5BackupErrorCode::BackupLimitExceeded)
}
fn changed() -> M5BackupError {
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
            "Library/Artifacts",
            "LibraryCache",
            "Temp",
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
            b"{\"dependencies\":{\"com.example.locked\":{\"version\":\"1.0.0\"}},\"locked\":{\"com.example.locked\":{\"version\":\"1.0.0\"}}}",
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
        std::fs::write(root.join("Library/Artifacts/cache.bin"), b"unity cache").expect("cache");
        std::fs::write(root.join("LibraryCache/cache.bin"), b"nonstandard cache")
            .expect("prefix cache");
        std::fs::write(root.join("Temp/temporary.bin"), b"temporary").expect("temporary");
        #[cfg(unix)]
        let root = std::fs::canonicalize(root).expect("resolve fixture directory");
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
            },
            revision: Revision::INITIAL,
            registered_at_ms: 1,
            last_modified_at_ms: None,
            favorite: false,
        };
        (root, record)
    }

    #[test]
    fn inventory_profile_is_precise_deterministic_and_rejects_hard_links() {
        let (root, project) = fixture();
        let request = BackupCreateRequest {
            project_id: project.project_id,
            expected_revision: Revision::INITIAL,
            compression_mode: BackupCompression::Fast,
            exclude_vpm_packages: true,
            include_library: false,
            created_at_ms: 1,
            destination: BackupDestination {
                directory: String::new(),
                identity: Vec::new(),
                file_name: None,
            },
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
        assert!(!names.contains("Library/Artifacts/cache.bin"));
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

    #[test]
    fn injected_small_quota_and_locked_manifest_mismatch_fail_closed() {
        let (root, mut project) = fixture();
        let request = BackupCreateRequest {
            project_id: project.project_id,
            expected_revision: Revision::INITIAL,
            compression_mode: BackupCompression::Fast,
            exclude_vpm_packages: true,
            include_library: false,
            created_at_ms: 1,
            destination: BackupDestination {
                directory: String::new(),
                identity: Vec::new(),
                file_name: None,
            },
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

    fn create_request(
        project: &ProjectRecord,
        destination: BackupDestination,
    ) -> BackupCreateRequest {
        BackupCreateRequest {
            project_id: project.project_id,
            expected_revision: project.revision,
            compression_mode: BackupCompression::Fast,
            exclude_vpm_packages: true,
            include_library: false,
            created_at_ms: 1,
            destination,
        }
    }

    fn scratch() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "alcomd-zip-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("data")).expect("create case directories");
        std::fs::canonicalize(root).expect("canonical test directory")
    }

    #[tokio::test]
    async fn archive_round_trip_preserves_content_options_and_compression() {
        let (root, project) = fixture();
        let case = scratch();
        let engine = BackupEngine::new(case.join("data/backups")).expect("engine");
        let destination = case.join("Owned ZIP files");
        std::fs::create_dir(&destination).expect("backup directory");
        for (index, mode) in [
            BackupCompression::Store,
            BackupCompression::Fast,
            BackupCompression::Level3,
            BackupCompression::Level5,
            BackupCompression::Level7,
            BackupCompression::Maximum,
        ]
        .into_iter()
        .enumerate()
        {
            for (exclude_vpm_packages, include_library) in [(true, false), (false, true)] {
                let archive = destination.join(format!(
                    "{} the 归档 {index}.ZIP",
                    u8::from(include_library)
                ));
                let destination = engine
                    .prepare_destination(None, Some(path_text(&archive).expect("path")), false)
                    .await
                    .expect("prepare destination");
                let mut request = create_request(&project, destination);
                request.compression_mode = mode;
                request.exclude_vpm_packages = exclude_vpm_packages;
                request.include_library = include_library;
                let operation = OperationId::new();
                let inventory = engine
                    .inventory(project.clone(), request.clone())
                    .await
                    .expect("inventory");
                let evidence = engine
                    .archive(
                        operation,
                        request.clone(),
                        inventory,
                        BackupCancellation::default(),
                    )
                    .await
                    .expect("ZIP output");
                assert!(!archive.exists(), "staged archive isn't publicly visible");
                assert_eq!(
                    engine
                        .publish_or_recover(operation, request.clone(), evidence.clone())
                        .await
                        .expect("publish"),
                    archive
                        .file_name()
                        .and_then(|name| name.to_str())
                        .expect("archive name"),
                );
                engine
                    .publish_or_recover(operation, request, evidence)
                    .await
                    .expect("restart can verify publication");
                let mut zip = ZipArchive::new(File::open(&archive).expect("open")).expect("ZIP");
                assert!(
                    zip.by_name("Assets/Empty/").is_ok(),
                    "keep empty directories"
                );
                assert!(zip.by_name("Assets/source.txt").is_ok());
                assert!(zip.by_name("Packages/vpm-manifest.json").is_ok());
                assert!(
                    zip.by_name("LibraryCache/LastSceneManagerSetup.txt")
                        .is_ok()
                );
                assert!(zip.by_name("LibraryCache/cache.bin").is_err());
                assert!(zip.by_name("Temp/temporary.bin").is_err());
                assert_eq!(
                    zip.by_name("Library/Artifacts/cache.bin").is_ok(),
                    include_library
                );
                assert_eq!(
                    zip.by_name("Packages/com.example.locked/package.json")
                        .is_err(),
                    exclude_vpm_packages
                );
                assert_eq!(
                    zip.by_name("Assets/source.txt")
                        .expect("project asset")
                        .compression(),
                    if mode == BackupCompression::Store {
                        CompressionMethod::Stored
                    } else {
                        CompressionMethod::Deflated
                    },
                );
                drop(zip);

                let target = case.join(format!("还原 here {index} {}", u8::from(include_library)));
                let restore = engine
                    .prepare_restore(path_text(&archive).expect("archive path"), target.clone())
                    .await
                    .expect("restore request");
                let operation = OperationId::new();
                let stage = engine
                    .stage_restore(operation, restore.clone(), BackupCancellation::default())
                    .await
                    .expect("stage");
                assert!(!target.exists());
                engine
                    .publish_restore(stage)
                    .await
                    .expect("publish restore");
                std::fs::remove_file(&archive)
                    .expect("selected ZIP may be moved after publication");
                let recovered = engine
                    .stage_restore(operation, restore.clone(), BackupCancellation::default())
                    .await
                    .expect("recover published target without source");
                assert!(recovered.already_published);
                assert_eq!(
                    std::fs::read(target.join("Assets/source.txt")).expect("restored files"),
                    b"source",
                );
                engine
                    .finalize_restore(operation, restore.clone())
                    .await
                    .expect("finalize");
                std::fs::write(
                    target.join("Assets/source.txt"),
                    b"Unity modified this project",
                )
                .expect("edit registered project");
                engine
                    .finalize_restore(operation, restore)
                    .await
                    .expect("cleanup replay after logical commit");
            }
        }
        std::fs::remove_dir_all(root).expect("cleanup source fixture");
        std::fs::remove_dir_all(case).expect("cleanup case");
    }

    #[tokio::test]
    async fn archive_rejects_existing_or_replaced_target_and_cancellation_leaves_no_partial() {
        let (root, project) = fixture();
        let case = scratch();
        let engine = BackupEngine::new(case.join("data/backups")).expect("engine");
        let destination = case.join("copy.zip");
        let mut request = create_request(
            &project,
            engine
                .prepare_destination(
                    None,
                    Some(path_text(&destination).expect("destination path")),
                    false,
                )
                .await
                .expect("destination"),
        );
        let operation = OperationId::new();
        let inventory = engine
            .inventory(project.clone(), request.clone())
            .await
            .expect("inventory");
        let cancellation = BackupCancellation::default();
        cancellation.cancel();
        assert!(
            engine
                .archive(operation, request.clone(), inventory, cancellation)
                .await
                .is_err()
        );
        assert!(!partial_path(&case, operation).exists());

        let inventory = engine
            .inventory(project.clone(), request.clone())
            .await
            .expect("inventory");
        let evidence = engine
            .archive(
                operation,
                request.clone(),
                inventory,
                BackupCancellation::default(),
            )
            .await
            .expect("stage archive");
        std::fs::copy(partial_path(&case, operation), &destination)
            .expect("simulated competing exact copy");
        assert_eq!(
            engine
                .publish_or_recover(operation, request.clone(), evidence.clone())
                .await
                .expect_err("never replace")
                .code(),
            M5BackupErrorCode::BackupRestoreTargetExists,
        );
        assert!(destination.is_file());
        assert_eq!(
            engine
                .publish_or_recover(operation, request.clone(), evidence)
                .await
                .expect_err("matching bytes are not publication proof")
                .code(),
            M5BackupErrorCode::BackupIntegrityMismatch,
        );

        let nested = root.join("Assets/unsafe.zip");
        request.destination = engine
            .prepare_destination(None, Some(path_text(&nested).expect("nested path")), false)
            .await
            .expect("path is superficially valid");
        assert_eq!(
            engine
                .inventory(project, request)
                .await
                .expect_err("reject archive inside source")
                .code(),
            M5BackupErrorCode::BackupSourceUnsafe
        );
        std::fs::remove_dir_all(root).expect("cleanup source fixture");
        std::fs::remove_dir_all(case).expect("cleanup case");
    }

    #[test]
    fn friendly_names_preserve_unicode_bound_length_and_reject_path_components() {
        let (root, mut project) = fixture();
        project.observation.root_path = format!("root/{}工程", "α".repeat(100));
        let name = archive_file_name(&project, 1).expect("friendly name");
        assert!(name.len() <= 255);
        assert!(name.starts_with('α'));
        assert!(valid_archive_file_name("用户 归档.zip"));
        assert!(!valid_archive_file_name("unsafe/path.zip"));
        assert!(!valid_archive_file_name("NUL.zip"));
        std::fs::remove_dir_all(root).expect("cleanup fixture");
    }
}
