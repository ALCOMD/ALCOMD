use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use alcomd_application::{
    BackupArchiveEvidence, BackupCancellation, BackupCompression, BackupCreateRequest,
    BackupRestorePlanDraft, BackupRestorePlanRecord, BackupRestoreTarget, M5BackupAdapter,
    M5BackupError, M5BackupErrorCode, OperationId, PlanId, PreparedBackupRestore, ProjectId,
    ProjectRecord, PublishedBackup, ResourceKey, RestoreExcludedPackage, RestoredProject,
    StagedBackupRestore, StoredBackupRecord,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive};

use crate::{ArchiveLimits, preflight_archive_with_limits};

#[derive(Clone, Debug)]
pub struct BackupEngine {
    root: PathBuf,
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
            root,
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

    async fn plan_restore(
        &self,
        backup: StoredBackupRecord,
        target_parent: PathBuf,
        target_leaf: String,
    ) -> Result<BackupRestorePlanDraft, M5BackupError> {
        let engine = self.clone();
        tokio::task::spawn_blocking(move || {
            plan_restore(&engine, backup, target_parent, target_leaf)
        })
        .await
        .map_err(|_| internal())?
    }

    fn restore_resource(
        &self,
        plan: &BackupRestorePlanRecord,
    ) -> Result<ResourceKey, M5BackupError> {
        Ok(ResourceKey::ProjectCreate {
            parent_identity_sha256: digest(&plan.draft.target_parent_identity),
            target_leaf: plan.draft.target.leaf.clone(),
        })
    }

    async fn prepare_restore(
        &self,
        plan: BackupRestorePlanRecord,
    ) -> Result<PreparedBackupRestore, M5BackupError> {
        let engine = self.clone();
        tokio::task::spawn_blocking(move || prepare_restore(&engine, plan))
            .await
            .map_err(|_| internal())?
    }

    async fn stage_restore(
        &self,
        operation_id: OperationId,
        prepared: PreparedBackupRestore,
    ) -> Result<StagedBackupRestore, M5BackupError> {
        let limits = self.limits;
        tokio::task::spawn_blocking(move || stage_restore(operation_id, prepared, limits))
            .await
            .map_err(|_| internal())?
    }

    async fn discard_staged_restore(
        &self,
        staged: StagedBackupRestore,
    ) -> Result<(), M5BackupError> {
        tokio::task::spawn_blocking(move || discard_staged_restore(staged))
            .await
            .map_err(|_| internal())?
    }

    async fn publish_restore(
        &self,
        staged: StagedBackupRestore,
    ) -> Result<RestoredProject, M5BackupError> {
        tokio::task::spawn_blocking(move || publish_restore(staged))
            .await
            .map_err(|_| internal())?
    }

    async fn validate_published_restore(
        &self,
        operation_id: OperationId,
        plan: BackupRestorePlanRecord,
    ) -> Result<RestoredProject, M5BackupError> {
        tokio::task::spawn_blocking(move || validate_published_restore(operation_id, &plan))
            .await
            .map_err(|_| internal())?
    }

    async fn finalize_restore(
        &self,
        operation_id: OperationId,
        plan: BackupRestorePlanRecord,
    ) -> Result<(), M5BackupError> {
        tokio::task::spawn_blocking(move || finalize_restore(operation_id, &plan))
            .await
            .map_err(|_| internal())?
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RestoreAuthority {
    version: u32,
    backup_id: String,
    preallocated_project_id: String,
    archive_sha256: String,
    archive_file_identity: String,
    archive_bytes: u64,
    backup_manifest_fingerprint: String,
    exclude_vpm_packages: bool,
    excluded_packages: Vec<RestoreExcludedPackage>,
    target_parent_path: String,
    target_parent_identity: String,
    target_leaf: String,
    target_must_be_absent: bool,
    expected_project_fingerprint: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RestoreManifest {
    format_version: u32,
    created_at_ms: u64,
    compression_mode: BackupCompression,
    exclude_vpm_packages: bool,
    source_project_revision: u64,
    source_project_fingerprint: String,
    unity_version: String,
    excluded_packages: Vec<RestoreExcludedPackage>,
    packages_require_resolve: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RestoreOwnerMarker {
    version: u32,
    operation_id: String,
    project_id: String,
    plan_fingerprint: String,
}

fn plan_restore(
    engine: &BackupEngine,
    backup: StoredBackupRecord,
    target_parent: PathBuf,
    target_leaf: String,
) -> Result<BackupRestorePlanDraft, M5BackupError> {
    validate_restore_leaf(&target_leaf)?;
    let (parent, parent_identity) = alcomd_platform::resolve_directory_identity(&target_parent)
        .map_err(|_| restore_unsafe())?;
    let data_root = engine.root.parent().ok_or_else(restore_unsafe)?;
    if parent.starts_with(data_root) || has_unity_project_ancestor(&parent) {
        return Err(restore_unsafe());
    }
    ensure_restore_absent(&parent.join(&target_leaf))?;
    let archive_path = engine
        .objects
        .join(format!("{}.zip", backup.record.backup_id));
    if backup.archive_locator != format!("backup-v1:{}", backup.record.backup_id) {
        return Err(error(M5BackupErrorCode::BackupIntegrityMismatch));
    }
    let inspection = inspect_restore_archive(&archive_path, engine.limits)?;
    let identity = alcomd_platform::file_identity_key(&archive_path)
        .map_err(|_| error(M5BackupErrorCode::BackupIntegrityMismatch))?;
    let bytes = std::fs::metadata(&archive_path)
        .map_err(|_| error(M5BackupErrorCode::BackupIntegrityMismatch))?
        .len();
    if identity != backup.file_identity_key
        || bytes != backup.record.archive_bytes
        || hash_file(&archive_path)? != backup.record.archive_sha256
        || inspection.manifest.format_version != backup.record.format_version
        || inspection.manifest.exclude_vpm_packages != backup.record.exclude_vpm_packages
    {
        return Err(error(M5BackupErrorCode::BackupIntegrityMismatch));
    }
    let plan_id = PlanId::new();
    let project_id = ProjectId::new();
    let authority = RestoreAuthority {
        version: 1,
        backup_id: backup.record.backup_id.to_string(),
        preallocated_project_id: project_id.to_string(),
        archive_sha256: hex(&backup.record.archive_sha256),
        archive_file_identity: hex_slice(&identity),
        archive_bytes: bytes,
        backup_manifest_fingerprint: hex(&inspection.manifest_fingerprint),
        exclude_vpm_packages: inspection.manifest.exclude_vpm_packages,
        excluded_packages: inspection.manifest.excluded_packages.clone(),
        target_parent_path: path_text(&parent)?,
        target_parent_identity: hex_slice(&parent_identity),
        target_leaf: target_leaf.clone(),
        target_must_be_absent: true,
        expected_project_fingerprint: hex(&inspection.project_fingerprint),
    };
    let plan_json = serde_json::to_string(&authority).map_err(|_| internal())?;
    Ok(BackupRestorePlanDraft {
        plan_id,
        project_id,
        backup_id: backup.record.backup_id,
        archive_sha256: backup.record.archive_sha256,
        archive_file_identity: identity,
        archive_bytes: bytes,
        manifest_fingerprint: inspection.manifest_fingerprint,
        exclude_vpm_packages: inspection.manifest.exclude_vpm_packages,
        excluded_packages: inspection.manifest.excluded_packages,
        target: BackupRestoreTarget {
            parent: path_text(&parent)?,
            leaf: target_leaf,
            must_be_absent: true,
        },
        target_parent_identity: parent_identity,
        expected_unity_project_json: serde_json::json!({
            "projectFingerprint": hex(&inspection.project_fingerprint),
            "unityVersion": inspection.manifest.unity_version,
            "packagesRequireResolve": inspection.manifest.packages_require_resolve,
        })
        .to_string(),
        plan_fingerprint: digest(plan_json.as_bytes()),
        plan_json,
    })
}

struct RestoreInspection {
    manifest: RestoreManifest,
    manifest_fingerprint: [u8; 32],
    project_fingerprint: [u8; 32],
}

fn inspect_restore_archive(
    path: &Path,
    limits: ArchiveLimits,
) -> Result<RestoreInspection, M5BackupError> {
    validate_v1(path, limits)?;
    let file = File::open(path).map_err(|_| integrity())?;
    let mut archive = ZipArchive::new(file).map_err(|_| integrity())?;
    let mut manifest_entry = archive.by_name("backup.json").map_err(|_| integrity())?;
    let mut manifest_bytes = Vec::new();
    manifest_entry
        .by_ref()
        .take(65_537)
        .read_to_end(&mut manifest_bytes)
        .map_err(|_| integrity())?;
    if manifest_bytes.len() > 65_536 {
        return Err(error(M5BackupErrorCode::BackupLimitExceeded));
    }
    drop(manifest_entry);
    let manifest: RestoreManifest =
        serde_json::from_slice(&manifest_bytes).map_err(|_| integrity())?;
    if manifest.format_version != 1
        || manifest.excluded_packages.len() > 4_096
        || manifest.source_project_revision == 0
        || manifest.source_project_fingerprint.len() != 64
        || manifest.created_at_ms == 0
    {
        return Err(integrity());
    }
    let project_fingerprint = archive_project_fingerprint(&mut archive)?;
    Ok(RestoreInspection {
        manifest,
        manifest_fingerprint: digest(&manifest_bytes),
        project_fingerprint,
    })
}

fn archive_project_fingerprint(archive: &mut ZipArchive<File>) -> Result<[u8; 32], M5BackupError> {
    let mut entries = Vec::new();
    let mut buffer = [0_u8; 64 * 1024];
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|_| integrity())?;
        let name = entry.name().to_owned();
        if name == "project/" || !name.starts_with("project/") {
            continue;
        }
        let relative = name.trim_start_matches("project/").trim_end_matches('/');
        if relative.is_empty() {
            continue;
        }
        let mut hasher = Sha256::new();
        while !entry.is_dir() {
            let read = entry.read(&mut buffer).map_err(|_| integrity())?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        entries.push((
            relative.to_owned(),
            entry.is_dir(),
            <[u8; 32]>::from(hasher.finalize()),
        ));
    }
    let existing = entries
        .iter()
        .map(|entry| entry.0.clone())
        .collect::<BTreeSet<_>>();
    let implicit = entries
        .iter()
        .flat_map(|entry| Path::new(&entry.0).ancestors().skip(1))
        .filter(|path| !path.as_os_str().is_empty())
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .filter(|path| !existing.contains(path))
        .collect::<BTreeSet<_>>();
    entries.extend(implicit.into_iter().map(|path| (path, true, digest(&[]))));
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = Sha256::new();
    for (relative, directory, content) in entries {
        hash_field(&mut hasher, relative.as_bytes())?;
        hasher.update([u8::from(directory)]);
        hasher.update(content);
    }
    Ok(hasher.finalize().into())
}

fn prepare_restore(
    engine: &BackupEngine,
    plan: BackupRestorePlanRecord,
) -> Result<PreparedBackupRestore, M5BackupError> {
    let authority = restore_authority(&plan)?;
    verify_restore_parent(&plan)?;
    let archive_path = engine.objects.join(format!("{}.zip", plan.draft.backup_id));
    let inspection = inspect_restore_archive(&archive_path, engine.limits)?;
    let identity = alcomd_platform::file_identity_key(&archive_path).map_err(|_| stale())?;
    let bytes = std::fs::metadata(&archive_path).map_err(|_| stale())?.len();
    if identity != plan.draft.archive_file_identity
        || bytes != plan.draft.archive_bytes
        || hash_file(&archive_path)? != plan.draft.archive_sha256
        || inspection.manifest_fingerprint != plan.draft.manifest_fingerprint
        || hex(&inspection.project_fingerprint) != authority.expected_project_fingerprint
    {
        return Err(stale());
    }
    Ok(PreparedBackupRestore { plan, archive_path })
}

fn stage_restore(
    operation_id: OperationId,
    prepared: PreparedBackupRestore,
    limits: ArchiveLimits,
) -> Result<StagedBackupRestore, M5BackupError> {
    let authority = restore_authority(&prepared.plan)?;
    verify_restore_parent(&prepared.plan)?;
    let parent = PathBuf::from(&authority.target_parent_path);
    let target_root = parent.join(&authority.target_leaf);
    let staging_root = parent.join(format!(".alcomd-restore-{operation_id}"));
    let project_root = staging_root.join("project");
    let owner_sidecar = staging_root.with_extension("owner.json");
    if target_root.exists() {
        if !owner_sidecar.exists() {
            return Err(error(M5BackupErrorCode::BackupRestoreTargetExists));
        }
        validate_restore_owner(&owner_sidecar, operation_id, &prepared.plan)?;
        return Ok(StagedBackupRestore {
            plan: prepared.plan,
            staging_root,
            project_root,
            target_root,
            owner_sidecar,
            already_published: true,
        });
    }
    remove_restore_staging(&staging_root, &owner_sidecar, operation_id, &prepared.plan)?;
    std::fs::create_dir(&staging_root).map_err(|_| internal())?;
    write_restore_owner(&owner_sidecar, operation_id, &prepared.plan)?;
    crate::extract_archive_with_limits(&prepared.archive_path, &staging_root, limits)
        .map_err(|_| integrity())?;
    let after_identity =
        alcomd_platform::file_identity_key(&prepared.archive_path).map_err(|_| stale())?;
    if after_identity != prepared.plan.draft.archive_file_identity
        || hash_file(&prepared.archive_path)? != prepared.plan.draft.archive_sha256
    {
        return Err(stale());
    }
    let actual_fingerprint = tree_fingerprint(&project_root)?;
    let expected_fingerprint = parse_digest(&authority.expected_project_fingerprint)?;
    if actual_fingerprint != expected_fingerprint {
        return Err(integrity());
    }
    validate_restore_project(&project_root, prepared.plan.draft.project_id)?;
    Ok(StagedBackupRestore {
        plan: prepared.plan,
        staging_root,
        project_root,
        target_root,
        owner_sidecar,
        already_published: false,
    })
}

fn discard_staged_restore(staged: StagedBackupRestore) -> Result<(), M5BackupError> {
    if staged.already_published {
        return Err(error(M5BackupErrorCode::BackupRestoreRecoveryRequired));
    }
    remove_restore_staging(
        &staged.staging_root,
        &staged.owner_sidecar,
        OperationId::parse(&read_restore_owner(&staged.owner_sidecar)?.operation_id)
            .map_err(|_| recovery())?,
        &staged.plan,
    )
}

fn publish_restore(staged: StagedBackupRestore) -> Result<RestoredProject, M5BackupError> {
    let operation_id = OperationId::parse(&read_restore_owner(&staged.owner_sidecar)?.operation_id)
        .map_err(|_| recovery())?;
    if !staged.already_published {
        verify_restore_parent(&staged.plan)?;
        ensure_restore_absent(&staged.target_root)?;
        std::fs::rename(&staged.project_root, &staged.target_root)
            .map_err(|_| error(M5BackupErrorCode::BackupRestoreTargetExists))?;
        alcomd_platform::sync_directory(staged.target_root.parent().ok_or_else(recovery)?)
            .map_err(|_| recovery())?;
    }
    validate_published_restore(operation_id, &staged.plan)
}

fn validate_published_restore(
    operation_id: OperationId,
    plan: &BackupRestorePlanRecord,
) -> Result<RestoredProject, M5BackupError> {
    let authority = restore_authority(plan)?;
    verify_restore_parent(plan)?;
    let parent = PathBuf::from(&authority.target_parent_path);
    let target = parent.join(&authority.target_leaf);
    let staging = parent.join(format!(".alcomd-restore-{operation_id}"));
    validate_restore_owner(&staging.with_extension("owner.json"), operation_id, plan)?;
    let fingerprint = tree_fingerprint(&target).map_err(|_| recovery())?;
    if fingerprint != parse_digest(&authority.expected_project_fingerprint)? {
        return Err(recovery());
    }
    let observation = validate_restore_project(&target, plan.draft.project_id)?;
    let target_identity = alcomd_platform::file_identity_key(&target).map_err(|_| recovery())?;
    Ok(RestoredProject {
        project_id: plan.draft.project_id,
        observation,
        target_identity,
        project_fingerprint: fingerprint,
    })
}

fn finalize_restore(
    operation_id: OperationId,
    plan: &BackupRestorePlanRecord,
) -> Result<(), M5BackupError> {
    let authority = restore_authority(plan)?;
    let parent = PathBuf::from(authority.target_parent_path);
    let staging = parent.join(format!(".alcomd-restore-{operation_id}"));
    let sidecar = staging.with_extension("owner.json");
    let target = parent.join(authority.target_leaf);
    if tree_fingerprint(&target)? != parse_digest(&authority.expected_project_fingerprint)? {
        return Err(recovery());
    }
    if !staging.exists() && !sidecar.exists() {
        return Ok(());
    }
    validate_restore_owner(&sidecar, operation_id, plan)?;
    if staging.exists() {
        let metadata = std::fs::symlink_metadata(&staging).map_err(|_| recovery())?;
        if !metadata.is_dir() || is_link_or_reparse(&metadata) {
            return Err(recovery());
        }
        std::fs::remove_dir_all(&staging).map_err(|_| internal())?;
    }
    std::fs::remove_file(&sidecar).map_err(|_| internal())?;
    alcomd_platform::sync_directory(&parent).map_err(|_| internal())
}

fn validate_restore_project(
    root: &Path,
    _project_id: ProjectId,
) -> Result<alcomd_application::ProjectObservation, M5BackupError> {
    let (root, identity) =
        alcomd_platform::resolve_directory_identity(root).map_err(|_| recovery())?;
    let reader = crate::VpmReader::new().map_err(|_| internal())?;
    tokio::runtime::Handle::current()
        .block_on(reader.inspect_project_root(root, identity, current_time_ms()?))
        .map_err(|_| error(M5BackupErrorCode::BackupIntegrityMismatch))
}

fn restore_authority(plan: &BackupRestorePlanRecord) -> Result<RestoreAuthority, M5BackupError> {
    if digest(plan.draft.plan_json.as_bytes()) != plan.draft.plan_fingerprint {
        return Err(stale());
    }
    let authority: RestoreAuthority =
        serde_json::from_str(&plan.draft.plan_json).map_err(|_| stale())?;
    if authority.version != 1
        || authority.backup_id != plan.draft.backup_id.to_string()
        || authority.preallocated_project_id != plan.draft.project_id.to_string()
        || !authority.target_must_be_absent
        || authority.archive_sha256 != hex(&plan.draft.archive_sha256)
        || authority.archive_file_identity != hex_slice(&plan.draft.archive_file_identity)
    {
        return Err(stale());
    }
    Ok(authority)
}

fn verify_restore_parent(plan: &BackupRestorePlanRecord) -> Result<(), M5BackupError> {
    let authority = restore_authority(plan)?;
    let (parent, identity) =
        alcomd_platform::resolve_directory_identity(Path::new(&authority.target_parent_path))
            .map_err(|_| stale())?;
    if path_text(&parent)? != authority.target_parent_path
        || identity != plan.draft.target_parent_identity
        || authority.target_parent_identity != hex_slice(&identity)
    {
        return Err(stale());
    }
    Ok(())
}

fn tree_fingerprint(root: &Path) -> Result<[u8; 32], M5BackupError> {
    let metadata = std::fs::symlink_metadata(root).map_err(|_| recovery())?;
    if !metadata.is_dir() || is_link_or_reparse(&metadata) {
        return Err(recovery());
    }
    let mut entries = Vec::new();
    collect_tree_entries(root, root, &mut entries)?;
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
) -> Result<(), M5BackupError> {
    let mut children = std::fs::read_dir(directory)
        .map_err(|_| recovery())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| recovery())?;
    children.sort_by_key(std::fs::DirEntry::file_name);
    for child in children {
        let path = child.path();
        let metadata = std::fs::symlink_metadata(&path).map_err(|_| recovery())?;
        if is_link_or_reparse(&metadata) || (!metadata.is_dir() && !metadata.is_file()) {
            return Err(recovery());
        }
        let relative = normalized_relative(root, &path)?;
        if metadata.is_dir() {
            entries.push((relative, true, digest(&[])));
            collect_tree_entries(root, &path, entries)?;
        } else {
            entries.push((relative, false, hash_file(&path)?));
        }
    }
    Ok(())
}

fn validate_restore_leaf(value: &str) -> Result<(), M5BackupError> {
    let normalized = value.nfc().collect::<String>();
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
        || normalized != value
        || matches!(value, "." | "..")
        || value.ends_with([' ', '.'])
        || value
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\' | ':'))
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
        Err(_) => Err(restore_unsafe()),
    }
}

fn write_restore_owner(
    path: &Path,
    operation_id: OperationId,
    plan: &BackupRestorePlanRecord,
) -> Result<(), M5BackupError> {
    let bytes = serde_json::to_vec(&RestoreOwnerMarker {
        version: 1,
        operation_id: operation_id.to_string(),
        project_id: plan.draft.project_id.to_string(),
        plan_fingerprint: hex(&plan.draft.plan_fingerprint),
    })
    .map_err(|_| internal())?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| internal())?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| internal())?;
    alcomd_platform::sync_directory(path.parent().ok_or_else(internal)?).map_err(|_| internal())
}

fn read_restore_owner(path: &Path) -> Result<RestoreOwnerMarker, M5BackupError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| recovery())?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) || metadata.len() > 4_096 {
        return Err(recovery());
    }
    serde_json::from_slice(&std::fs::read(path).map_err(|_| recovery())?).map_err(|_| recovery())
}

fn validate_restore_owner(
    path: &Path,
    operation_id: OperationId,
    plan: &BackupRestorePlanRecord,
) -> Result<(), M5BackupError> {
    let marker = read_restore_owner(path)?;
    if marker.version != 1
        || marker.operation_id != operation_id.to_string()
        || marker.project_id != plan.draft.project_id.to_string()
        || marker.plan_fingerprint != hex(&plan.draft.plan_fingerprint)
    {
        return Err(recovery());
    }
    Ok(())
}

fn remove_restore_staging(
    staging: &Path,
    sidecar: &Path,
    operation_id: OperationId,
    plan: &BackupRestorePlanRecord,
) -> Result<(), M5BackupError> {
    match std::fs::symlink_metadata(staging) {
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            if sidecar.exists() {
                validate_restore_owner(sidecar, operation_id, plan)?;
                std::fs::remove_file(sidecar).map_err(|_| internal())?;
            }
            return Ok(());
        }
        Ok(metadata) if metadata.is_dir() && !is_link_or_reparse(&metadata) => {}
        _ => return Err(recovery()),
    }
    validate_restore_owner(sidecar, operation_id, plan)?;
    std::fs::remove_dir_all(staging).map_err(|_| internal())?;
    std::fs::remove_file(sidecar).map_err(|_| internal())
}

fn path_text(path: &Path) -> Result<String, M5BackupError> {
    path.to_str().map(str::to_owned).ok_or_else(restore_unsafe)
}

fn hex_slice(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn parse_digest(value: &str) -> Result<[u8; 32], M5BackupError> {
    if value.len() != 64 {
        return Err(stale());
    }
    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).map_err(|_| stale())?;
    }
    Ok(bytes)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn current_time_ms() -> Result<u64, M5BackupError> {
    u64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| internal())?
            .as_millis(),
    )
    .map_err(|_| internal())
}

const fn integrity() -> M5BackupError {
    error(M5BackupErrorCode::BackupIntegrityMismatch)
}

const fn stale() -> M5BackupError {
    error(M5BackupErrorCode::BackupRestorePlanStale)
}

const fn restore_unsafe() -> M5BackupError {
    error(M5BackupErrorCode::BackupRestoreTargetUnsafe)
}

const fn recovery() -> M5BackupError {
    error(M5BackupErrorCode::BackupRestoreRecoveryRequired)
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
