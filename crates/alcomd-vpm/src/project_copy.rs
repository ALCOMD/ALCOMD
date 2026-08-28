use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};

use alcomd_application::{
    M7CopyAdapter, M7CopyError, M7CopyErrorCode, OperationId, PlanId, ProjectCopyInventoryEvidence,
    ProjectCopyPlanDraft, ProjectCopyPlanRecord, ProjectId, ProjectRecord, PublishedProjectCopy,
    ResourceKey, UnityWriterState,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

const PROFILE_VERSION: u32 = 1;
const MAX_ENTRIES: u64 = 500_000;
const MAX_SINGLE_FILE_BYTES: u64 = 32 * 1024 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 128 * 1024 * 1024 * 1024;
const MAX_DEPTH: usize = 128;
const MAX_RELATIVE_PATH_BYTES: usize = 1_024;

#[derive(Clone, Copy, Default)]
pub struct ProjectCopyEngine;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InventoryHeader {
    format_version: u32,
    operation_id: OperationId,
    plan_id: PlanId,
    source_project_id: ProjectId,
    source_root_identity: Vec<u8>,
    copy_profile_version: u32,
    entry_count: u64,
    total_regular_file_bytes: u64,
    created_at: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum EntryKind {
    Directory,
    RegularFile,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InventoryEntry {
    normalized_relative_path: String,
    entry_kind: EntryKind,
    size: u64,
    source_filesystem_identity_evidence: Vec<u8>,
    executable_bit: bool,
    content_sha256: Option<[u8; 32]>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "record", content = "value", rename_all = "snake_case")]
enum InventoryLine {
    Header(InventoryHeader),
    Entry(InventoryEntry),
}

impl M7CopyAdapter for ProjectCopyEngine {
    async fn plan(
        &self,
        source: ProjectRecord,
        target_parent: PathBuf,
        target_leaf: String,
        writer_evidence: UnityWriterState,
        plan_key: alcomd_application::IdempotencyKey,
        now_ms: u64,
    ) -> Result<ProjectCopyPlanDraft, M7CopyError> {
        validate_leaf(&target_leaf)?;
        let source_root =
            canonical_directory(Path::new(&source.observation.root_path), source_error())?;
        let source_identity =
            alcomd_platform::file_identity_key(&source_root).map_err(|_| source_error())?;
        if source_identity != source.observation.path_identity_key {
            return Err(source_error());
        }
        let target_parent = canonical_directory(&target_parent, target_error())?;
        let target_parent_identity =
            alcomd_platform::file_identity_key(&target_parent).map_err(|_| target_error())?;
        let target = target_parent.join(&target_leaf);
        if target.exists() {
            return Err(error(M7CopyErrorCode::ProjectCopyTargetExists));
        }
        reject_containment(&source_root, &target)?;
        let target_parent_identity_sha256 = digest(&target_parent_identity);
        let plan_id = PlanId::new();
        let target_project_id = ProjectId::new();
        let expires_at_ms = now_ms
            .checked_add(alcomd_application::PROJECT_COPY_PLAN_EXPIRY_MS)
            .ok_or_else(internal)?;
        let authority = serde_json::json!({
            "version": 1,
            "planId": plan_id,
            "sourceProjectId": source.project_id,
            "sourceRevision": source.revision,
            "sourceRootIdentity": hex(&source_identity),
            "targetParent": target_parent.to_string_lossy(),
            "targetParentIdentity": hex(&target_parent_identity),
            "targetLeaf": target_leaf,
            "targetProjectId": target_project_id,
            "profileVersion": PROFILE_VERSION,
            "writerEvidence": writer_evidence,
            "createdAtMs": now_ms,
            "expiresAtMs": expires_at_ms
        });
        let plan_json = serde_json::to_string(&authority).map_err(|_| internal())?;
        let plan_fingerprint = digest(plan_json.as_bytes());
        Ok(ProjectCopyPlanDraft {
            plan_id,
            source_project: source,
            source_root_identity: source_identity,
            target_parent_path: target_parent.to_string_lossy().into_owned(),
            target_parent_identity,
            target_parent_identity_sha256,
            target_leaf,
            target_project_id,
            writer_evidence,
            profile_version: PROFILE_VERSION,
            plan_fingerprint,
            plan_json,
            plan_idempotency_key: plan_key,
            created_at_ms: now_ms,
            expires_at_ms,
        })
    }

    fn resources(&self, plan: &ProjectCopyPlanRecord) -> Vec<ResourceKey> {
        vec![
            ResourceKey::Project(plan.draft.source_project.project_id),
            ResourceKey::ProjectCreate {
                parent_identity_sha256: plan.draft.target_parent_identity_sha256,
                target_leaf: plan.draft.target_leaf.clone(),
            },
        ]
    }

    async fn revalidate_plan(&self, plan: &ProjectCopyPlanRecord) -> Result<(), M7CopyError> {
        if now_ms()? >= plan.draft.expires_at_ms {
            return Err(error(M7CopyErrorCode::ProjectCopyPlanStale));
        }
        let source = canonical_directory(
            Path::new(&plan.draft.source_project.observation.root_path),
            source_error(),
        )?;
        let parent =
            canonical_directory(Path::new(&plan.draft.target_parent_path), target_error())?;
        if alcomd_platform::file_identity_key(&source).map_err(|_| source_error())?
            != plan.draft.source_root_identity
            || alcomd_platform::file_identity_key(&parent).map_err(|_| target_error())?
                != plan.draft.target_parent_identity
        {
            return Err(error(M7CopyErrorCode::ProjectCopyPlanStale));
        }
        let target = parent.join(&plan.draft.target_leaf);
        if target.exists() {
            return Err(error(M7CopyErrorCode::ProjectCopyTargetExists));
        }
        reject_containment(&source, &target)
    }

    async fn inventory(
        &self,
        operation_id: OperationId,
        plan: ProjectCopyPlanRecord,
    ) -> Result<ProjectCopyInventoryEvidence, M7CopyError> {
        let source = PathBuf::from(&plan.draft.source_project.observation.root_path);
        let (shell, payload, owner, inventory_path) = workspace(operation_id, &plan);
        if shell.exists() {
            verify_owner(&owner, operation_id, plan.draft.plan_id)?;
        } else {
            fs::create_dir(&shell).map_err(|_| recovery_error())?;
            write_owner(&owner, operation_id, plan.draft.plan_id)?;
        }
        if !payload.exists() {
            fs::create_dir(&payload).map_err(|_| recovery_error())?;
        }
        let entries = scan(&source, false)?;
        let total = regular_bytes(&entries);
        let header = InventoryHeader {
            format_version: 1,
            operation_id,
            plan_id: plan.draft.plan_id,
            source_project_id: plan.draft.source_project.project_id,
            source_root_identity: plan.draft.source_root_identity.clone(),
            copy_profile_version: PROFILE_VERSION,
            entry_count: entries.len() as u64,
            total_regular_file_bytes: total,
            created_at: now_ms()?,
        };
        write_inventory(&inventory_path, &header, &entries)?;
        evidence(&inventory_path, &owner, header.entry_count, total)
    }

    async fn stage(
        &self,
        operation_id: OperationId,
        plan: ProjectCopyPlanRecord,
        inventory: ProjectCopyInventoryEvidence,
    ) -> Result<ProjectCopyInventoryEvidence, M7CopyError> {
        let source = PathBuf::from(&plan.draft.source_project.observation.root_path);
        let (_, payload, owner, inventory_path) = workspace(operation_id, &plan);
        verify_owner(&owner, operation_id, plan.draft.plan_id)?;
        if payload.exists() {
            fs::remove_dir_all(&payload).map_err(|_| recovery_error())?;
        }
        fs::create_dir(&payload).map_err(|_| recovery_error())?;
        let (header, mut entries) = read_inventory(Path::new(&inventory.private_locator))?;
        for entry in &mut entries {
            let relative = path_from_normalized(&entry.normalized_relative_path)?;
            let source_path = source.join(&relative);
            let target_path = payload.join(relative);
            match entry.entry_kind {
                EntryKind::Directory => {
                    fs::create_dir_all(&target_path).map_err(|_| recovery_error())?;
                }
                EntryKind::RegularFile => {
                    if let Some(parent) = target_path.parent() {
                        fs::create_dir_all(parent).map_err(|_| recovery_error())?;
                    }
                    verify_entry_metadata(&source_path, entry)?;
                    entry.content_sha256 = Some(copy_and_hash(&source_path, &target_path)?);
                    preserve_executable(&source_path, &target_path)?;
                }
            }
        }
        write_inventory(&inventory_path, &header, &entries)?;
        alcomd_platform::sync_directory(&payload).map_err(|_| recovery_error())?;
        evidence(
            &inventory_path,
            &owner,
            header.entry_count,
            header.total_regular_file_bytes,
        )
    }

    async fn verify_source(
        &self,
        plan: ProjectCopyPlanRecord,
        inventory: ProjectCopyInventoryEvidence,
    ) -> Result<(), M7CopyError> {
        let source = PathBuf::from(&plan.draft.source_project.observation.root_path);
        let (_, expected) = read_inventory(Path::new(&inventory.private_locator))?;
        let actual = scan(&source, true)?;
        if actual != expected {
            return Err(error(M7CopyErrorCode::ProjectCopySourceChanged));
        }
        Ok(())
    }

    async fn publish(
        &self,
        operation_id: OperationId,
        plan: ProjectCopyPlanRecord,
        _inventory: ProjectCopyInventoryEvidence,
    ) -> Result<PublishedProjectCopy, M7CopyError> {
        let (_, payload, owner, _) = workspace(operation_id, &plan);
        verify_owner(&owner, operation_id, plan.draft.plan_id)?;
        let target = PathBuf::from(&plan.draft.target_parent_path).join(&plan.draft.target_leaf);
        if target.exists() {
            return Err(error(M7CopyErrorCode::ProjectCopyTargetExists));
        }
        fs::rename(&payload, &target).map_err(|_| recovery_error())?;
        alcomd_platform::sync_directory(Path::new(&plan.draft.target_parent_path))
            .map_err(|_| recovery_error())?;
        published(&plan, &target)
    }

    async fn validate_published(
        &self,
        operation_id: OperationId,
        plan: ProjectCopyPlanRecord,
        inventory: ProjectCopyInventoryEvidence,
        expected: Option<PublishedProjectCopy>,
    ) -> Result<PublishedProjectCopy, M7CopyError> {
        let (_, payload, owner, _) = workspace(operation_id, &plan);
        verify_owner(&owner, operation_id, plan.draft.plan_id)?;
        let target = PathBuf::from(&plan.draft.target_parent_path).join(&plan.draft.target_leaf);
        let was_unpublished = payload.is_dir() && !target.exists();
        if was_unpublished {
            fs::rename(&payload, &target).map_err(|_| recovery_error())?;
            alcomd_platform::sync_directory(Path::new(&plan.draft.target_parent_path))
                .map_err(|_| recovery_error())?;
        } else if payload.exists() || !target.is_dir() {
            return Err(recovery_error());
        }
        let current = published(&plan, &target)?;
        if let Some(expected) = expected
            && expected.target_identity != current.target_identity
        {
            return Err(recovery_error());
        }
        let (_, expected_entries) = read_inventory(Path::new(&inventory.private_locator))?;
        let actual_entries = scan(&target, true).map_err(|_| recovery_error())?;
        if !published_matches_inventory(&actual_entries, &expected_entries) {
            return Err(recovery_error());
        }
        Ok(current)
    }

    async fn cleanup(
        &self,
        operation_id: OperationId,
        plan: ProjectCopyPlanRecord,
    ) -> Result<(), M7CopyError> {
        let (shell, payload, owner, _) = workspace(operation_id, &plan);
        verify_owner(&owner, operation_id, plan.draft.plan_id)?;
        if payload.exists() {
            return Err(recovery_error());
        }
        fs::remove_dir_all(&shell).map_err(|_| recovery_error())?;
        alcomd_platform::sync_directory(Path::new(&plan.draft.target_parent_path))
            .map_err(|_| recovery_error())
    }

    async fn discard(
        &self,
        operation_id: OperationId,
        plan: ProjectCopyPlanRecord,
    ) -> Result<(), M7CopyError> {
        let (shell, _, owner, _) = workspace(operation_id, &plan);
        verify_owner(&owner, operation_id, plan.draft.plan_id)?;
        let target = PathBuf::from(&plan.draft.target_parent_path).join(&plan.draft.target_leaf);
        if target.exists() {
            return Err(recovery_error());
        }
        fs::remove_dir_all(&shell).map_err(|_| recovery_error())?;
        alcomd_platform::sync_directory(Path::new(&plan.draft.target_parent_path))
            .map_err(|_| recovery_error())
    }
}

fn scan(root: &Path, hash_content: bool) -> Result<Vec<InventoryEntry>, M7CopyError> {
    let mut result = Vec::new();
    let mut collisions = BTreeMap::<String, String>::new();
    walk(root, root, 0, hash_content, &mut collisions, &mut result)?;
    result.sort_by(|a, b| a.normalized_relative_path.cmp(&b.normalized_relative_path));
    Ok(result)
}

fn walk(
    root: &Path,
    directory: &Path,
    depth: usize,
    hash_content: bool,
    collisions: &mut BTreeMap<String, String>,
    result: &mut Vec<InventoryEntry>,
) -> Result<(), M7CopyError> {
    if depth > MAX_DEPTH {
        return Err(limit_error());
    }
    let mut children = fs::read_dir(directory)
        .map_err(|_| source_error())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| source_error())?;
    children.sort_by_key(std::fs::DirEntry::file_name);
    for child in children {
        let relative = child
            .path()
            .strip_prefix(root)
            .map_err(|_| source_error())?
            .to_path_buf();
        let normalized = normalize_relative(&relative)?;
        if excluded(&normalized, depth) {
            continue;
        }
        register_collision(collisions, &normalized)?;
        if result.len() as u64 >= MAX_ENTRIES {
            return Err(limit_error());
        }
        let metadata = fs::symlink_metadata(child.path()).map_err(|_| source_error())?;
        if is_link_or_reparse(&metadata) {
            return Err(source_error());
        }
        let kind = if metadata.is_dir() {
            EntryKind::Directory
        } else if metadata.is_file() {
            EntryKind::RegularFile
        } else {
            return Err(source_error());
        };
        if kind == EntryKind::RegularFile {
            if metadata.len() > MAX_SINGLE_FILE_BYTES
                || regular_bytes(result)
                    .checked_add(metadata.len())
                    .filter(|value| *value <= MAX_TOTAL_BYTES)
                    .is_none()
            {
                return Err(limit_error());
            }
            reject_hard_link(&child.path(), &metadata)?;
        }
        let content_sha256 = if kind == EntryKind::RegularFile && hash_content {
            Some(hash_file(&child.path())?)
        } else {
            None
        };
        result.push(InventoryEntry {
            normalized_relative_path: normalized,
            entry_kind: kind.clone(),
            size: if kind == EntryKind::RegularFile {
                metadata.len()
            } else {
                0
            },
            source_filesystem_identity_evidence: alcomd_platform::file_identity_key(&child.path())
                .map_err(|_| source_error())?,
            executable_bit: executable(&metadata),
            content_sha256,
        });
        if kind == EntryKind::Directory {
            walk(
                root,
                &child.path(),
                depth + 1,
                hash_content,
                collisions,
                result,
            )?;
        }
    }
    Ok(())
}

fn write_inventory(
    path: &Path,
    header: &InventoryHeader,
    entries: &[InventoryEntry],
) -> Result<(), M7CopyError> {
    let temporary = path.with_extension("jsonl.tmp");
    let file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)
        .map_err(|_| recovery_error())?;
    let mut writer = BufWriter::new(file);
    write_line(&mut writer, &InventoryLine::Header(header.clone()))?;
    for entry in entries {
        write_line(&mut writer, &InventoryLine::Entry(entry.clone()))?;
    }
    writer.flush().map_err(|_| recovery_error())?;
    writer.get_ref().sync_all().map_err(|_| recovery_error())?;
    drop(writer);
    fs::rename(&temporary, path).map_err(|_| recovery_error())?;
    if let Some(parent) = path.parent() {
        alcomd_platform::sync_directory(parent).map_err(|_| recovery_error())?;
    }
    Ok(())
}

fn write_line(writer: &mut BufWriter<File>, line: &InventoryLine) -> Result<(), M7CopyError> {
    serde_json::to_writer(&mut *writer, line).map_err(|_| internal())?;
    writer.write_all(b"\n").map_err(|_| recovery_error())
}

fn read_inventory(path: &Path) -> Result<(InventoryHeader, Vec<InventoryEntry>), M7CopyError> {
    let file = File::open(path).map_err(|_| recovery_error())?;
    let mut lines = BufReader::new(file).lines();
    let first = lines
        .next()
        .ok_or_else(recovery_error)?
        .map_err(|_| recovery_error())?;
    let InventoryLine::Header(header) =
        serde_json::from_str(&first).map_err(|_| recovery_error())?
    else {
        return Err(recovery_error());
    };
    let mut entries = Vec::new();
    for line in lines {
        let InventoryLine::Entry(entry) =
            serde_json::from_str(&line.map_err(|_| recovery_error())?)
                .map_err(|_| recovery_error())?
        else {
            return Err(recovery_error());
        };
        if entries.len() as u64 >= MAX_ENTRIES {
            return Err(limit_error());
        }
        entries.push(entry);
    }
    if entries.len() as u64 != header.entry_count {
        return Err(recovery_error());
    }
    Ok((header, entries))
}

fn evidence(
    path: &Path,
    owner: &Path,
    entry_count: u64,
    total: u64,
) -> Result<ProjectCopyInventoryEvidence, M7CopyError> {
    let bytes = fs::read(path).map_err(|_| recovery_error())?;
    Ok(ProjectCopyInventoryEvidence {
        private_locator: path.to_string_lossy().into_owned(),
        sha256: digest(&bytes),
        byte_length: bytes.len() as u64,
        owner_marker: owner.to_string_lossy().into_owned(),
        entry_count,
        total_regular_file_bytes: total,
    })
}

fn workspace(
    operation: OperationId,
    plan: &ProjectCopyPlanRecord,
) -> (PathBuf, PathBuf, PathBuf, PathBuf) {
    let shell = PathBuf::from(&plan.draft.target_parent_path)
        .join(format!(".alcomd-copy-{operation}.staging"));
    (
        shell.clone(),
        shell.join("payload"),
        shell.join("owner.json"),
        shell.join("inventory.jsonl"),
    )
}

fn write_owner(path: &Path, operation: OperationId, plan: PlanId) -> Result<(), M7CopyError> {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "formatVersion": 1,
        "operationId": operation,
        "planId": plan
    }))
    .map_err(|_| internal())?;
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|_| recovery_error())?;
    file.write_all(&bytes).map_err(|_| recovery_error())?;
    file.sync_all().map_err(|_| recovery_error())
}

fn verify_owner(path: &Path, operation: OperationId, plan: PlanId) -> Result<(), M7CopyError> {
    let value: serde_json::Value =
        serde_json::from_slice(&fs::read(path).map_err(|_| recovery_error())?)
            .map_err(|_| recovery_error())?;
    if value["operationId"] != operation.to_string() || value["planId"] != plan.to_string() {
        return Err(recovery_error());
    }
    Ok(())
}

fn published(
    plan: &ProjectCopyPlanRecord,
    target: &Path,
) -> Result<PublishedProjectCopy, M7CopyError> {
    let identity = alcomd_platform::file_identity_key(target).map_err(|_| recovery_error())?;
    let mut observation = plan.draft.source_project.observation.clone();
    observation.root_path = target.to_string_lossy().into_owned();
    observation.path_identity_key = identity.clone();
    observation.observed_at_ms = now_ms()?;
    Ok(PublishedProjectCopy {
        observation,
        target_identity: identity,
    })
}

fn copy_and_hash(source: &Path, target: &Path) -> Result<[u8; 32], M7CopyError> {
    let mut input = BufReader::new(File::open(source).map_err(|_| source_error())?);
    let output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(target)
        .map_err(|_| recovery_error())?;
    let mut output = BufWriter::new(output);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input.read(&mut buffer).map_err(|_| source_error())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        output
            .write_all(&buffer[..read])
            .map_err(|_| recovery_error())?;
    }
    output.flush().map_err(|_| recovery_error())?;
    output.get_ref().sync_all().map_err(|_| recovery_error())?;
    Ok(hasher.finalize().into())
}

fn hash_file(path: &Path) -> Result<[u8; 32], M7CopyError> {
    let mut input = BufReader::new(File::open(path).map_err(|_| source_error())?);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = input.read(&mut buffer).map_err(|_| source_error())?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().into())
}

fn verify_entry_metadata(path: &Path, expected: &InventoryEntry) -> Result<(), M7CopyError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| changed_error())?;
    if !metadata.is_file()
        || is_link_or_reparse(&metadata)
        || metadata.len() != expected.size
        || alcomd_platform::file_identity_key(path).map_err(|_| changed_error())?
            != expected.source_filesystem_identity_evidence
        || executable(&metadata) != expected.executable_bit
    {
        return Err(changed_error());
    }
    reject_hard_link(path, &metadata).map_err(|_| changed_error())
}

#[cfg(unix)]
fn executable(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    metadata.mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable(_: &fs::Metadata) -> bool {
    false
}

#[cfg(unix)]
fn preserve_executable(source: &Path, target: &Path) -> Result<(), M7CopyError> {
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(source)
        .map_err(|_| source_error())?
        .permissions()
        .mode();
    fs::set_permissions(
        target,
        fs::Permissions::from_mode(if mode & 0o111 != 0 { 0o755 } else { 0o644 }),
    )
    .map_err(|_| recovery_error())
}

#[cfg(not(unix))]
fn preserve_executable(_: &Path, _: &Path) -> Result<(), M7CopyError> {
    Ok(())
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
}

#[cfg(unix)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(unix)]
fn reject_hard_link(_: &Path, metadata: &fs::Metadata) -> Result<(), M7CopyError> {
    use std::os::unix::fs::MetadataExt;
    if metadata.nlink() != 1 {
        Err(source_error())
    } else {
        Ok(())
    }
}

#[cfg(windows)]
fn reject_hard_link(path: &Path, _: &fs::Metadata) -> Result<(), M7CopyError> {
    if alcomd_platform::file_link_count(path).map_err(|_| source_error())? != 1 {
        Err(source_error())
    } else {
        Ok(())
    }
}

fn normalize_relative(path: &Path) -> Result<String, M7CopyError> {
    if path.is_absolute() {
        return Err(source_error());
    }
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => parts.push(value.to_str().ok_or_else(source_error)?),
            _ => return Err(source_error()),
        }
    }
    if parts.is_empty() || parts.len() > MAX_DEPTH {
        return Err(limit_error());
    }
    let normalized = parts.join("/");
    if normalized.len() > MAX_RELATIVE_PATH_BYTES {
        return Err(limit_error());
    }
    Ok(normalized)
}

fn register_collision(
    collisions: &mut BTreeMap<String, String>,
    normalized: &str,
) -> Result<(), M7CopyError> {
    let collision_key = normalized
        .nfkc()
        .flat_map(char::to_lowercase)
        .collect::<String>();
    if collisions
        .insert(collision_key, normalized.to_owned())
        .is_some()
    {
        Err(source_error())
    } else {
        Ok(())
    }
}

fn path_from_normalized(value: &str) -> Result<PathBuf, M7CopyError> {
    let path = PathBuf::from(value.replace('/', std::path::MAIN_SEPARATOR_STR));
    if path
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        Ok(path)
    } else {
        Err(source_error())
    }
}

fn excluded(path: &str, depth: usize) -> bool {
    let leaf = path.rsplit('/').next().unwrap_or(path);
    leaf.eq_ignore_ascii_case(".git")
        || (depth == 0
            && ["Logs", "Obj", "Temp"]
                .iter()
                .any(|value| leaf.eq_ignore_ascii_case(value)))
}

fn regular_bytes(entries: &[InventoryEntry]) -> u64 {
    entries
        .iter()
        .filter(|entry| entry.entry_kind == EntryKind::RegularFile)
        .map(|entry| entry.size)
        .sum()
}

fn published_matches_inventory(actual: &[InventoryEntry], expected: &[InventoryEntry]) -> bool {
    actual.len() == expected.len()
        && actual.iter().zip(expected).all(|(actual, expected)| {
            actual.normalized_relative_path == expected.normalized_relative_path
                && actual.entry_kind == expected.entry_kind
                && actual.size == expected.size
                && actual.executable_bit == expected.executable_bit
                && actual.content_sha256 == expected.content_sha256
        })
}

fn canonical_directory(path: &Path, kind: M7CopyError) -> Result<PathBuf, M7CopyError> {
    if !path.is_absolute() {
        return Err(kind);
    }
    let value = fs::canonicalize(path).map_err(|_| kind)?;
    if value.to_str().is_none() || !value.is_dir() {
        return Err(kind);
    }
    Ok(value)
}

fn validate_leaf(value: &str) -> Result<(), M7CopyError> {
    if value.is_empty()
        || value.len() > 255
        || value == "."
        || value == ".."
        || value.contains(['/', '\\', '\0'])
    {
        Err(target_error())
    } else {
        Ok(())
    }
}

fn reject_containment(source: &Path, target: &Path) -> Result<(), M7CopyError> {
    let source_key = lexical_key(source);
    let target_key = lexical_key(target);
    if target_key.starts_with(&(source_key.clone() + "\\"))
        || source_key.starts_with(&(target_key.clone() + "\\"))
        || source_key == target_key
    {
        Err(target_error())
    } else {
        Ok(())
    }
}

fn lexical_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}

fn digest(value: &[u8]) -> [u8; 32] {
    Sha256::digest(value).into()
}

fn hex(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn now_ms() -> Result<u64, M7CopyError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| u64::try_from(value.as_millis()).unwrap_or(u64::MAX))
        .map_err(|_| internal())
}

fn error(code: M7CopyErrorCode) -> M7CopyError {
    M7CopyError::new(code)
}
fn internal() -> M7CopyError {
    error(M7CopyErrorCode::Internal)
}
fn source_error() -> M7CopyError {
    error(M7CopyErrorCode::ProjectCopySourceUnsafe)
}
fn target_error() -> M7CopyError {
    error(M7CopyErrorCode::ProjectCopyTargetUnsafe)
}
fn changed_error() -> M7CopyError {
    error(M7CopyErrorCode::ProjectCopySourceChanged)
}
fn limit_error() -> M7CopyError {
    error(M7CopyErrorCode::ProjectCopyLimitExceeded)
}
fn recovery_error() -> M7CopyError {
    error(M7CopyErrorCode::ProjectCopyRecoveryRequired)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alcomd_application::{
        ManifestState, Revision, UnityWriterEvidenceKind, UnityWriterStateKind,
    };

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "alcomd-project-copy-unit-{}-{unique}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create fixture root");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn copy_profile_excludes_only_frozen_entries() {
        assert!(excluded(".git", 0));
        assert!(excluded("Assets/.GIT", 1));
        assert!(excluded("Logs", 0));
        assert!(!excluded("Assets/Logs", 1));
        assert!(!excluded("Library", 0));
    }

    #[test]
    fn leaf_and_containment_fail_closed() {
        assert!(validate_leaf("copy").is_ok());
        assert!(validate_leaf("../copy").is_err());
        assert!(reject_containment(Path::new("C:\\root"), Path::new("C:\\root\\copy")).is_err());
    }

    #[tokio::test]
    async fn plan_rejects_existing_and_nested_targets_without_scanning_source() {
        let root = TestDirectory::new();
        let source = root.0.join("Source");
        let destination = root.0.join("Destination");
        fs::create_dir(&source).expect("source");
        fs::create_dir(&destination).expect("destination");
        let project_id = ProjectId::new();
        let project = ProjectRecord {
            project_id,
            observation: alcomd_application::ProjectObservation {
                root_path: fs::canonicalize(&source)
                    .expect("canonical source")
                    .to_string_lossy()
                    .into_owned(),
                path_identity_key: alcomd_platform::file_identity_key(&source)
                    .expect("source identity"),
                project_type: alcomd_application::ProjectType::Unknown,
                unity_version: "2022.3.22f1".to_owned(),
                unity_revision: None,
                vpm_manifest: ManifestState::Missing,
                upm_manifest: ManifestState::Missing,
                direct_dependencies: Vec::new(),
                locked_dependencies: Vec::new(),
                issues: Vec::new(),
                observed_at_ms: 1,
            },
            revision: Revision::INITIAL,
            favorite: false,
            registered_at_ms: 1,
        };
        let writer = UnityWriterState {
            project_id,
            state: UnityWriterStateKind::NotObserved,
            evidence: Vec::<UnityWriterEvidenceKind>::new(),
            checked_at_ms: 1,
        };
        fs::create_dir(destination.join("Existing")).expect("existing target");
        let existing = ProjectCopyEngine
            .plan(
                project.clone(),
                destination.clone(),
                "Existing".to_owned(),
                writer.clone(),
                alcomd_application::IdempotencyKey::parse("existing-target".to_owned())
                    .expect("key"),
                1,
            )
            .await
            .expect_err("existing target");
        assert_eq!(existing.code(), M7CopyErrorCode::ProjectCopyTargetExists);

        let nested = ProjectCopyEngine
            .plan(
                project,
                source.clone(),
                "Nested".to_owned(),
                writer,
                alcomd_application::IdempotencyKey::parse("nested-target".to_owned()).expect("key"),
                1,
            )
            .await
            .expect_err("nested target");
        assert_eq!(nested.code(), M7CopyErrorCode::ProjectCopyTargetUnsafe);
    }

    #[test]
    fn inventory_includes_project_content_and_excludes_only_frozen_paths() {
        let root = TestDirectory::new();
        for directory in [
            "Library",
            "LibraryCache",
            "Packages",
            "Assets/Logs",
            "Assets/.git",
            "Loose",
            "Logs",
            "Obj",
            "Temp",
        ] {
            fs::create_dir_all(root.0.join(directory)).expect("create fixture directory");
        }
        for (path, bytes) in [
            ("Library/state.bin", b"library".as_slice()),
            ("LibraryCache/state.bin", b"library-cache".as_slice()),
            ("Packages/manifest.json", b"{}".as_slice()),
            (".hidden", b"hidden".as_slice()),
            ("Assets/Logs/kept.log", b"kept".as_slice()),
            ("Assets/.git/config", b"excluded".as_slice()),
            ("Loose/.GIT", b"excluded".as_slice()),
            ("Logs/editor.log", b"excluded".as_slice()),
            ("Obj/object.bin", b"excluded".as_slice()),
            ("Temp/file.tmp", b"excluded".as_slice()),
        ] {
            fs::write(root.0.join(path), bytes).expect("write fixture file");
        }
        let paths = scan(&root.0, true)
            .expect("scan")
            .into_iter()
            .map(|entry| entry.normalized_relative_path)
            .collect::<Vec<_>>();
        for included in [
            ".hidden",
            "Assets",
            "Assets/Logs",
            "Assets/Logs/kept.log",
            "Library",
            "Library/state.bin",
            "LibraryCache",
            "LibraryCache/state.bin",
            "Packages",
            "Packages/manifest.json",
        ] {
            assert!(paths.iter().any(|path| path == included), "{included}");
        }
        assert!(paths.iter().all(|path| !path.starts_with("Logs")));
        assert!(paths.iter().all(|path| !path.starts_with("Obj")));
        assert!(paths.iter().all(|path| !path.starts_with("Temp")));
        assert!(paths.iter().all(|path| !path.contains(".git")));
        assert!(paths.iter().all(|path| !path.contains(".GIT")));
    }

    #[test]
    fn hard_link_is_rejected_fail_closed() {
        let root = TestDirectory::new();
        fs::write(root.0.join("source.txt"), b"same object").expect("source");
        fs::hard_link(root.0.join("source.txt"), root.0.join("alias.txt")).expect("hard link");
        let failure = scan(&root.0, false).expect_err("hard link must fail closed");
        assert_eq!(failure.code(), M7CopyErrorCode::ProjectCopySourceUnsafe);
    }

    #[test]
    fn content_level_second_pass_detects_source_change() {
        let root = TestDirectory::new();
        let source = root.0.join("content.txt");
        fs::write(&source, b"before").expect("source");
        let mut staged = scan(&root.0, false).expect("initial inventory");
        let file = staged
            .iter_mut()
            .find(|entry| entry.entry_kind == EntryKind::RegularFile)
            .expect("file entry");
        file.content_sha256 = Some(hash_file(&source).expect("staging digest"));
        assert_eq!(scan(&root.0, true).expect("unchanged second pass"), staged);
        fs::write(&source, b"after!").expect("mutate source");
        assert_ne!(scan(&root.0, true).expect("changed second pass"), staged);
    }

    #[test]
    fn normalized_path_quota_is_exact() {
        assert!(normalize_relative(Path::new(&"a".repeat(MAX_RELATIVE_PATH_BYTES))).is_ok());
        let failure = normalize_relative(Path::new(&"a".repeat(MAX_RELATIVE_PATH_BYTES + 1)))
            .expect_err("oversized normalized path");
        assert_eq!(failure.code(), M7CopyErrorCode::ProjectCopyLimitExceeded);
    }

    #[cfg(unix)]
    #[test]
    fn executable_bit_is_preserved_without_copying_full_mode() {
        use std::os::unix::fs::PermissionsExt;

        let root = TestDirectory::new();
        let source = root.0.join("source.sh");
        let target = root.0.join("target.sh");
        fs::write(&source, b"#!/bin/sh\n").expect("source");
        fs::set_permissions(&source, fs::Permissions::from_mode(0o711)).expect("source mode");
        fs::write(&target, b"#!/bin/sh\n").expect("target");
        preserve_executable(&source, &target).expect("preserve executable");
        assert_eq!(
            fs::metadata(&target)
                .expect("target metadata")
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_git_symlink_is_rejected_but_git_symlink_is_excluded() {
        use std::os::unix::fs::symlink;

        let root = TestDirectory::new();
        fs::write(root.0.join("target.txt"), b"target").expect("target");
        symlink(root.0.join("target.txt"), root.0.join("linked.txt")).expect("symlink");
        assert_eq!(
            scan(&root.0, false).expect_err("symlink must fail").code(),
            M7CopyErrorCode::ProjectCopySourceUnsafe
        );
        fs::remove_file(root.0.join("linked.txt")).expect("remove symlink");
        symlink(root.0.join("target.txt"), root.0.join(".git")).expect("git symlink");
        assert!(scan(&root.0, false).is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn windows_reparse_directory_is_rejected_but_git_reparse_is_excluded() {
        let root = TestDirectory::new();
        let target = root.0.join("target");
        fs::create_dir(&target).expect("target");
        std::os::windows::fs::symlink_dir(&target, root.0.join("linked"))
            .expect("directory symlink");
        assert_eq!(
            scan(&root.0, false)
                .expect_err("reparse directory must fail")
                .code(),
            M7CopyErrorCode::ProjectCopySourceUnsafe
        );
        fs::remove_dir(root.0.join("linked")).expect("remove directory symlink");
        std::os::windows::fs::symlink_dir(&target, root.0.join(".GIT"))
            .expect("git directory symlink");
        assert!(scan(&root.0, false).is_ok());
    }

    #[cfg(any(unix, windows))]
    #[test]
    fn non_utf8_paths_are_rejected() {
        use std::ffi::OsString;

        #[cfg(unix)]
        use std::os::unix::ffi::OsStringExt;

        #[cfg(windows)]
        use std::os::windows::ffi::OsStringExt;

        #[cfg(unix)]
        let name = OsString::from_vec(vec![0xff]);
        #[cfg(windows)]
        let name = OsString::from_wide(&[0xd800]);
        assert_eq!(
            normalize_relative(Path::new(&name))
                .expect_err("non-UTF-8 must fail")
                .code(),
            M7CopyErrorCode::ProjectCopySourceUnsafe
        );
    }

    #[test]
    fn normalization_collisions_are_rejected() {
        let composed = normalize_relative(Path::new("é.txt")).expect("composed path");
        let decomposed = normalize_relative(Path::new("e\u{301}.txt")).expect("decomposed path");
        assert_ne!(composed, decomposed);

        let mut collisions = BTreeMap::new();
        register_collision(&mut collisions, &composed).expect("first path");
        assert_eq!(
            register_collision(&mut collisions, &decomposed)
                .expect_err("Unicode collision must fail")
                .code(),
            M7CopyErrorCode::ProjectCopySourceUnsafe
        );
    }
}
