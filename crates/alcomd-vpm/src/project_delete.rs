use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use alcomd_application::{
    M7DeleteAdapter, M7DeleteError, M7DeleteErrorCode, OperationId,
    ProjectDeleteFilesystemEvidence, ProjectDeletePlanDraft, ProjectDeletePlanRecord, ResourceKey,
    UnityWriterState,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const PROFILE_VERSION: u32 = 1;
const OWNER_FORMAT_VERSION: u32 = 1;
const MARKER_LIMIT: u64 = 64 * 1024;

#[derive(Clone, Copy, Default)]
pub struct ProjectDeleteEngine;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuarantineOwner {
    format_version: u32,
    operation_id: OperationId,
    plan_id: alcomd_application::PlanId,
    project_id: alcomd_application::ProjectId,
    root_identity: Vec<u8>,
    parent_identity: Vec<u8>,
    profile_version: u32,
}

impl M7DeleteAdapter for ProjectDeleteEngine {
    async fn plan(
        &self,
        project: alcomd_application::ProjectRecord,
        writer_evidence: UnityWriterState,
        plan_key: alcomd_application::IdempotencyKey,
        now_ms: u64,
    ) -> Result<ProjectDeletePlanDraft, M7DeleteError> {
        let root = exact_root(&project)?;
        protected_root_check(&root)?;
        let root_identity = identity(&root, source_changed())?;
        if root_identity != project.observation.path_identity_key {
            return Err(source_changed());
        }
        let parent = root
            .parent()
            .ok_or_else(|| unsafe_reason("missing_parent"))?
            .to_path_buf();
        require_ordinary_directory(&parent)?;
        let parent =
            fs::canonicalize(parent).map_err(|cause| source_changed().with_source(cause))?;
        let parent_identity = identity(&parent, source_changed())?;
        let normalized_leaf = root
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .filter(|value| !value.is_empty() && value.len() <= 255)
            .ok_or_else(|| unsafe_reason("invalid_leaf"))?
            .to_owned();
        let marker = marker_digest(&root)?;
        let plan_id = alcomd_application::PlanId::new();
        let expires_at_ms = now_ms
            .checked_add(alcomd_application::PROJECT_DELETE_PLAN_EXPIRY_MS)
            .ok_or_else(internal)?;
        let parent_identity_sha256 = digest(&parent_identity);
        let authority = serde_json::json!({
            "version": 1,
            "planId": plan_id,
            "projectId": project.project_id,
            "projectRevision": project.revision,
            "rootIdentity": hex(&root_identity),
            "parentIdentity": hex(&parent_identity),
            "normalizedLeaf": normalized_leaf,
            "projectMarkerSha256": hex(&marker),
            "writerEvidence": writer_evidence,
            "profile": {
                "id": "alcomd-project-delete",
                "version": PROFILE_VERSION,
                "mode": "sibling-quarantine-permanent-v1",
                "protectedRootProfileVersion": 1,
                "progress": "phase-only"
            },
            "createdAtMs": now_ms,
            "expiresAtMs": expires_at_ms
        });
        let plan_json =
            serde_json::to_string(&authority).map_err(|cause| internal().with_source(cause))?;
        Ok(ProjectDeletePlanDraft {
            plan_id,
            project,
            root_identity,
            canonical_parent_path: parent.to_string_lossy().into_owned(),
            parent_identity,
            parent_identity_sha256,
            normalized_leaf,
            project_marker_sha256: marker,
            writer_evidence,
            profile_version: PROFILE_VERSION,
            plan_fingerprint: digest(plan_json.as_bytes()),
            plan_json,
            plan_idempotency_key: plan_key,
            created_at_ms: now_ms,
            expires_at_ms,
        })
    }

    fn resources(&self, plan: &ProjectDeletePlanRecord) -> Vec<ResourceKey> {
        vec![
            ResourceKey::Project(plan.draft.project.project_id),
            ResourceKey::ProjectCreate {
                parent_identity_sha256: plan.draft.parent_identity_sha256,
                target_leaf: plan.draft.normalized_leaf.clone(),
            },
        ]
    }

    async fn revalidate_plan(&self, plan: &ProjectDeletePlanRecord) -> Result<(), M7DeleteError> {
        if now_ms()? >= plan.draft.expires_at_ms {
            return Err(error(M7DeleteErrorCode::ProjectDeletePlanStale));
        }
        let root = exact_root(&plan.draft.project)?;
        protected_root_check(&root)?;
        let parent = fs::canonicalize(
            root.parent()
                .ok_or_else(|| unsafe_reason("missing_parent"))?,
        )
        .map_err(|cause| source_changed().with_source(cause))?;
        if identity(&root, source_changed())? != plan.draft.root_identity
            || identity(&parent, source_changed())? != plan.draft.parent_identity
            || root.file_name().and_then(std::ffi::OsStr::to_str)
                != Some(plan.draft.normalized_leaf.as_str())
            || marker_digest(&root)? != plan.draft.project_marker_sha256
        {
            return Err(source_changed());
        }
        Ok(())
    }

    async fn preflight(
        &self,
        operation_id: OperationId,
        plan: ProjectDeletePlanRecord,
    ) -> Result<ProjectDeleteFilesystemEvidence, M7DeleteError> {
        self.revalidate_plan(&plan).await?;
        let wrapper = wrapper(operation_id, &plan);
        let owner_path = wrapper.join("owner.json");
        let payload = wrapper.join("payload");
        let owner = expected_owner(operation_id, &plan);
        if wrapper.exists() {
            verify_wrapper(&wrapper, &owner)?;
        } else {
            fs::create_dir(&wrapper).map_err(|cause| recovery().with_source(cause))?;
            write_owner(&owner_path, &owner)?;
            alcomd_platform::sync_directory(&wrapper)
                .map_err(|cause| recovery().with_source(cause))?;
            alcomd_platform::sync_directory(Path::new(&plan.draft.canonical_parent_path))
                .map_err(|cause| recovery().with_source(cause))?;
        }
        if payload.exists() {
            return Err(recovery());
        }
        let root = PathBuf::from(&plan.draft.project.observation.root_path);
        let count =
            tokio::task::spawn_blocking(move || alcomd_platform::project_delete_preflight(&root))
                .await
                .map_err(|cause| internal().with_source(cause))?
                .map_err(map_filesystem)?;
        Ok(ProjectDeleteFilesystemEvidence {
            quarantine_locator: wrapper.to_string_lossy().into_owned(),
            quarantine_identity: None,
            entry_count: Some(count),
            safe_evidence: vec!["mount_safe_preflight_complete".to_owned()],
        })
    }

    async fn quarantine(
        &self,
        operation_id: OperationId,
        plan: ProjectDeletePlanRecord,
        mut evidence: ProjectDeleteFilesystemEvidence,
    ) -> Result<ProjectDeleteFilesystemEvidence, M7DeleteError> {
        let wrapper = verified_locator(operation_id, &plan, &evidence)?;
        let payload = wrapper.join("payload");
        let root = PathBuf::from(&plan.draft.project.observation.root_path);
        if payload.exists() || !root.exists() {
            return Err(recovery());
        }
        rename_into_quarantine(operation_id, &plan, &evidence)?;
        alcomd_platform::sync_directory(Path::new(&plan.draft.canonical_parent_path))
            .map_err(|cause| recovery().with_source(cause))?;
        alcomd_platform::sync_directory(&wrapper).map_err(|cause| recovery().with_source(cause))?;
        let payload_identity = identity(&payload, recovery())?;
        if payload_identity != plan.draft.root_identity || root.exists() {
            return Err(recovery());
        }
        evidence.quarantine_identity = Some(payload_identity);
        evidence.safe_evidence.push("root_quarantined".to_owned());
        Ok(evidence)
    }

    async fn validate_quarantine(
        &self,
        operation_id: OperationId,
        plan: ProjectDeletePlanRecord,
        mut evidence: ProjectDeleteFilesystemEvidence,
    ) -> Result<ProjectDeleteFilesystemEvidence, M7DeleteError> {
        let wrapper = verified_locator(operation_id, &plan, &evidence)?;
        let payload = wrapper.join("payload");
        let root = PathBuf::from(&plan.draft.project.observation.root_path);
        if !payload.exists() {
            if evidence.quarantine_identity.is_some() {
                return Err(recovery());
            }
            if let Some(failure) = missing_source_failure(operation_id, &plan, &evidence) {
                return Err(failure);
            }
            if !root.exists() {
                return Err(recovery());
            }
            if identity(&root, recovery())? != plan.draft.root_identity {
                return Err(recovery());
            }
            rename_into_quarantine(operation_id, &plan, &evidence)?;
            alcomd_platform::sync_directory(Path::new(&plan.draft.canonical_parent_path))
                .map_err(|cause| recovery().with_source(cause))?;
            alcomd_platform::sync_directory(&wrapper)
                .map_err(|cause| recovery().with_source(cause))?;
        } else if !payload.is_dir() {
            return Err(recovery());
        }
        let payload_identity = identity(&payload, recovery())?;
        if payload_identity != plan.draft.root_identity
            || evidence
                .quarantine_identity
                .as_ref()
                .is_some_and(|value| value != &payload_identity)
        {
            return Err(recovery());
        }
        evidence.quarantine_identity = Some(payload_identity);
        Ok(evidence)
    }

    async fn cleanup(
        &self,
        operation_id: OperationId,
        plan: ProjectDeletePlanRecord,
        mut evidence: ProjectDeleteFilesystemEvidence,
    ) -> Result<ProjectDeleteFilesystemEvidence, M7DeleteError> {
        let wrapper = verified_locator(operation_id, &plan, &evidence)?;
        let payload = wrapper.join("payload");
        if identity(&payload, recovery())? != plan.draft.root_identity {
            return Err(recovery());
        }
        let count =
            tokio::task::spawn_blocking(move || alcomd_platform::project_delete_cleanup(&payload))
                .await
                .map_err(|cause| internal().with_source(cause))?
                .map_err(map_filesystem)?;
        let owner = wrapper.join("owner.json");
        fs::remove_file(owner).map_err(|cause| recovery().with_source(cause))?;
        if fs::read_dir(&wrapper)
            .map_err(|cause| recovery().with_source(cause))?
            .next()
            .is_some()
        {
            return Err(recovery());
        }
        fs::remove_dir(&wrapper).map_err(|cause| recovery().with_source(cause))?;
        alcomd_platform::sync_directory(Path::new(&plan.draft.canonical_parent_path))
            .map_err(|cause| recovery().with_source(cause))?;
        evidence.entry_count = Some(count);
        evidence.safe_evidence.push("cleanup_complete".to_owned());
        Ok(evidence)
    }

    async fn discard(
        &self,
        operation_id: OperationId,
        plan: ProjectDeletePlanRecord,
        evidence: ProjectDeleteFilesystemEvidence,
    ) -> Result<(), M7DeleteError> {
        discard_prepared_quarantine(operation_id, &plan, &evidence)
    }
}

fn rename_into_quarantine(
    operation_id: OperationId,
    plan: &ProjectDeletePlanRecord,
    evidence: &ProjectDeleteFilesystemEvidence,
) -> Result<(), M7DeleteError> {
    let root = Path::new(&plan.draft.project.observation.root_path);
    let payload = Path::new(&evidence.quarantine_locator).join("payload");
    if let Err(cause) = fs::rename(root, &payload) {
        if let Some(failure) = missing_source_failure(operation_id, plan, evidence) {
            return Err(failure.with_source(cause));
        }
        let failure = source_unsafe()
            .with_source(cause)
            .with_path(root.display().to_string());
        let payload_absent = matches!(fs::symlink_metadata(&payload), Err(error) if error.kind() == std::io::ErrorKind::NotFound);
        if evidence.quarantine_identity.is_none()
            && payload_absent
            && identity(root, recovery())? == plan.draft.root_identity
            && identity(Path::new(&plan.draft.canonical_parent_path), recovery())?
                == plan.draft.parent_identity
        {
            let failure = match discard_prepared_quarantine(operation_id, plan, evidence) {
                Ok(()) => failure,
                Err(cleanup) => failure.with_reason(format!(
                    "source was not moved; preparation cleanup failed: {}",
                    alcomd_application::technical_error_chain(&cleanup)
                )),
            };
            return Err(failure.with_unmoved_source());
        }
        return Err(recovery().with_source(failure));
    }
    Ok(())
}

fn missing_source_failure(
    operation_id: OperationId,
    plan: &ProjectDeletePlanRecord,
    evidence: &ProjectDeleteFilesystemEvidence,
) -> Option<M7DeleteError> {
    let root = Path::new(&plan.draft.project.observation.root_path);
    let payload = Path::new(&evidence.quarantine_locator).join("payload");
    let absent = |path: &Path| matches!(fs::symlink_metadata(path), Err(cause) if cause.kind() == std::io::ErrorKind::NotFound);
    if evidence.quarantine_identity.is_some() || !absent(root) || !absent(&payload) {
        return None;
    }
    let failure = error(M7DeleteErrorCode::ProjectDeleteSourceMissing)
        .with_path(root.display().to_string())
        .with_reason("source and owned quarantine payload are both absent");
    let failure = match discard_prepared_quarantine(operation_id, plan, evidence) {
        Ok(()) => failure,
        Err(cleanup) => failure.with_reason(format!(
            "source and owned payload are absent; preparation cleanup failed: {}",
            alcomd_application::technical_error_chain(&cleanup)
        )),
    };
    Some(failure.with_missing_source_and_payload())
}

fn discard_prepared_quarantine(
    operation_id: OperationId,
    plan: &ProjectDeletePlanRecord,
    evidence: &ProjectDeleteFilesystemEvidence,
) -> Result<(), M7DeleteError> {
    let wrapper = verified_locator(operation_id, plan, evidence)?;
    if wrapper.join("payload").exists() {
        return Err(recovery());
    }
    fs::remove_file(wrapper.join("owner.json")).map_err(|cause| recovery().with_source(cause))?;
    if fs::read_dir(&wrapper)
        .map_err(|cause| recovery().with_source(cause))?
        .next()
        .is_some()
    {
        return Err(recovery());
    }
    fs::remove_dir(wrapper).map_err(|cause| recovery().with_source(cause))?;
    Ok(())
}

fn exact_root(project: &alcomd_application::ProjectRecord) -> Result<PathBuf, M7DeleteError> {
    let stored = Path::new(&project.observation.root_path);
    if !stored.exists() {
        return Err(error(M7DeleteErrorCode::ProjectDeleteSourceMissing));
    }
    require_ordinary_directory(stored)?;
    let root = fs::canonicalize(stored).map_err(|cause| source_changed().with_source(cause))?;
    if root.to_str().is_none() {
        return Err(unsafe_reason("non_unicode_root"));
    }
    Ok(root)
}

fn require_ordinary_directory(path: &Path) -> Result<(), M7DeleteError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|cause| source_changed().with_source(cause))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || reparse(&metadata) {
        return Err(unsafe_reason("root_link_or_reparse"));
    }
    Ok(())
}

#[cfg(windows)]
fn reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x0000_0400 != 0
}

#[cfg(not(windows))]
fn reparse(_: &fs::Metadata) -> bool {
    false
}

fn protected_root_check(candidate: &Path) -> Result<(), M7DeleteError> {
    if candidate.parent().is_none() {
        return Err(unsafe_reason("filesystem_root"));
    }
    if let Some(home) = home_directory()?
        && (home == candidate || home.starts_with(candidate))
    {
        return Err(unsafe_reason("home_or_home_ancestor"));
    }
    if let Ok(executable) = std::env::current_exe()
        && let Some(parent) = executable.parent()
    {
        reject_overlap(
            candidate,
            &fs::canonicalize(parent)
                .map_err(|cause| unsafe_reason("executable_unavailable").with_source(cause))?,
        )?;
    }
    if let Ok(database) =
        alcomd_platform::state_database_path(&alcomd_platform::DataConfig::default())
        && let Some(data_root) = database.parent()
        && data_root.exists()
    {
        reject_overlap(
            candidate,
            &fs::canonicalize(data_root)
                .map_err(|cause| unsafe_reason("data_root_unavailable").with_source(cause))?,
        )?;
    }
    Ok(())
}

#[cfg(windows)]
fn home_directory() -> Result<Option<PathBuf>, M7DeleteError> {
    let local = alcomd_platform::local_app_data_directory()
        .map_err(|cause| unsafe_reason("home_unavailable").with_source(cause))?;
    let home = local
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| unsafe_reason("home_unavailable"))?
        .to_path_buf();
    if !home.is_absolute() || fs::symlink_metadata(&home).is_err() {
        return Err(unsafe_reason("home_unavailable"));
    }
    Ok(Some(fs::canonicalize(&home).unwrap_or(home)))
}

#[cfg(not(windows))]
fn home_directory() -> Result<Option<PathBuf>, M7DeleteError> {
    let Some(home) = std::env::var_os("HOME") else {
        return Err(unsafe_reason("home_unavailable"));
    };
    fs::canonicalize(home)
        .map(Some)
        .map_err(|cause| unsafe_reason("home_unavailable").with_source(cause))
}

fn reject_overlap(candidate: &Path, protected: &Path) -> Result<(), M7DeleteError> {
    if candidate == protected
        || candidate.starts_with(protected)
        || protected.starts_with(candidate)
    {
        return Err(unsafe_reason("protected_root_overlap"));
    }
    Ok(())
}

fn marker_digest(root: &Path) -> Result<[u8; 32], M7DeleteError> {
    let path = root.join("ProjectSettings").join("ProjectVersion.txt");
    let metadata =
        fs::symlink_metadata(&path).map_err(|cause| source_changed().with_source(cause))?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || reparse(&metadata)
        || metadata.len() > MARKER_LIMIT
    {
        return Err(source_changed());
    }
    let file = fs::File::open(path).map_err(|cause| source_changed().with_source(cause))?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MARKER_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|cause| source_changed().with_source(cause))?;
    if bytes.len() as u64 > MARKER_LIMIT || bytes.is_empty() {
        return Err(source_changed());
    }
    Ok(digest(&bytes))
}

fn wrapper(operation_id: OperationId, plan: &ProjectDeletePlanRecord) -> PathBuf {
    PathBuf::from(&plan.draft.canonical_parent_path)
        .join(format!(".alcomd-delete-{operation_id}.quarantine"))
}

fn expected_owner(operation_id: OperationId, plan: &ProjectDeletePlanRecord) -> QuarantineOwner {
    QuarantineOwner {
        format_version: OWNER_FORMAT_VERSION,
        operation_id,
        plan_id: plan.draft.plan_id,
        project_id: plan.draft.project.project_id,
        root_identity: plan.draft.root_identity.clone(),
        parent_identity: plan.draft.parent_identity.clone(),
        profile_version: PROFILE_VERSION,
    }
}

fn write_owner(path: &Path, owner: &QuarantineOwner) -> Result<(), M7DeleteError> {
    let bytes = serde_json::to_vec(owner).map_err(|cause| {
        internal()
            .with_source(cause)
            .with_path(path.display().to_string())
    })?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|cause| recovery().with_source(cause))?;
    file.write_all(&bytes)
        .map_err(|cause| recovery().with_source(cause))?;
    file.sync_all()
        .map_err(|cause| recovery().with_source(cause))
}

fn verify_wrapper(path: &Path, expected: &QuarantineOwner) -> Result<(), M7DeleteError> {
    require_ordinary_directory(path).map_err(|cause| recovery().with_source(cause))?;
    let bytes = fs::read(path.join("owner.json")).map_err(|cause| recovery().with_source(cause))?;
    let actual: QuarantineOwner =
        serde_json::from_slice(&bytes).map_err(|cause| recovery().with_source(cause))?;
    if actual != *expected {
        return Err(recovery());
    }
    for entry in fs::read_dir(path).map_err(|cause| recovery().with_source(cause))? {
        let name = entry
            .map_err(|cause| recovery().with_source(cause))?
            .file_name()
            .to_string_lossy()
            .into_owned();
        if name != "owner.json" && name != "payload" {
            return Err(recovery());
        }
    }
    Ok(())
}

fn verified_locator(
    operation_id: OperationId,
    plan: &ProjectDeletePlanRecord,
    evidence: &ProjectDeleteFilesystemEvidence,
) -> Result<PathBuf, M7DeleteError> {
    let expected = wrapper(operation_id, plan);
    if evidence.quarantine_locator != expected.to_string_lossy() {
        return Err(recovery());
    }
    verify_wrapper(&expected, &expected_owner(operation_id, plan))?;
    Ok(expected)
}

fn identity(path: &Path, kind: M7DeleteError) -> Result<Vec<u8>, M7DeleteError> {
    alcomd_platform::file_identity_key(path).map_err(|cause| {
        kind.with_source(cause)
            .with_path(path.display().to_string())
    })
}

fn map_filesystem(source: alcomd_platform::ProjectDeleteFilesystemError) -> M7DeleteError {
    use alcomd_platform::ProjectDeleteFilesystemErrorKind as Kind;
    (match source.kind() {
        Kind::MountBoundary | Kind::MountGuardUnavailable | Kind::UnsafeEntry => source_unsafe(),
        Kind::Io => recovery(),
    })
    .with_source(source)
}

fn digest(value: &[u8]) -> [u8; 32] {
    Sha256::digest(value).into()
}

fn hex(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn now_ms() -> Result<u64, M7DeleteError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| u64::try_from(value.as_millis()).unwrap_or(u64::MAX))
        .map_err(|cause| internal().with_source(cause))
}

fn error(code: M7DeleteErrorCode) -> M7DeleteError {
    M7DeleteError::new(code)
}

fn source_changed() -> M7DeleteError {
    error(M7DeleteErrorCode::ProjectDeleteSourceChanged)
}

fn source_unsafe() -> M7DeleteError {
    error(M7DeleteErrorCode::ProjectDeleteSourceUnsafe)
}

fn unsafe_reason(reason: &str) -> M7DeleteError {
    #[cfg(feature = "test-kill-gates")]
    eprintln!("M7 Project Delete test-only unsafe classification: {reason}");
    #[cfg(not(feature = "test-kill-gates"))]
    let _ = reason;
    source_unsafe().with_reason(reason)
}

fn recovery() -> M7DeleteError {
    error(M7DeleteErrorCode::ProjectDeleteRecoveryRequired)
}

fn internal() -> M7DeleteError {
    error(M7DeleteErrorCode::Internal)
}
