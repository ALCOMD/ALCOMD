use std::path::{Path, PathBuf};
use std::sync::Arc;

use alcomd_application::{
    CreatedTemplateProject, M5TemplateError, M5TemplateErrorCode, OperationId, PackageChangeSet,
    PackageSourcePin, PlanAction, ProjectId, ProjectRecord, ResolverCatalog, ResourceKey,
    ResourceLockCoordinator, Revision, StoredTemplateRecord, TemplatePlanDraft, TemplatePlanKind,
    TemplatePlanRecord,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use crate::{
    PreparedFrozenPackages, Resolution, ResolveRequest, StagingProjectEvidence, TemplateEngine,
    VpmReader, build_resolution_plan, candidates_from_catalog, inspect_package_project,
    inspect_template_bundle, materialize_vpm_manifest, resolve_packages,
};

#[doc(hidden)]
pub struct PreparedTemplateProject {
    authority: CreateProjectAuthority,
    packages: PreparedFrozenPackages,
}

#[doc(hidden)]
pub struct StagedTemplateProject {
    authority: CreateProjectAuthority,
    staging_root: PathBuf,
    target_root: PathBuf,
    owner_sidecar: PathBuf,
    already_published: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateProjectAuthority {
    version: u32,
    kind: String,
    template_id: String,
    template_revision: u64,
    template_version: String,
    bundle_sha256: String,
    manifest_fingerprint: String,
    payload_tree_sha256: String,
    manifest_json: String,
    parent_path: String,
    parent_filesystem_identity: String,
    parent_identity_sha256: String,
    target_leaf: String,
    target_path: String,
    target_must_be_absent: bool,
    project_id: String,
    package_change_set_fingerprint: String,
    package_change_set: PackageChangeSet,
    package_source_set: Vec<PackageSourcePin>,
    initial_vpm_manifest_sha256: String,
    initial_upm_manifest_sha256: String,
    final_vpm_manifest_sha256: String,
    resource_digests: Vec<String>,
    project_summary_fingerprint: String,
}

pub(super) async fn plan_create_project(
    engine: &TemplateEngine,
    template: StoredTemplateRecord,
    catalog: ResolverCatalog,
    target_parent: String,
    target_leaf: String,
) -> Result<TemplatePlanDraft, M5TemplateError> {
    validate_target_leaf(&target_leaf)?;
    let requested_parent = PathBuf::from(&target_parent);
    let (parent, parent_identity) = alcomd_platform::resolve_directory_identity(&requested_parent)
        .map_err(|_| error(M5TemplateErrorCode::InvalidInput))?;
    let target = parent.join(&target_leaf);
    ensure_absent(&target)?;
    let object = engine
        .objects
        .open_verified(template.bundle_sha256)
        .map_err(super::template_engine::map_object_error)?;
    let inspection = inspect_template_bundle(object.path())
        .map_err(|_| error(M5TemplateErrorCode::TemplateBundleInvalid))?;
    if inspection.normalized_manifest_json != template.manifest_json
        || inspection.manifest.template_id != template.template_id.to_string()
        || inspection.manifest.template_version != template.template_version
    {
        return Err(error(M5TemplateErrorCode::TemplateBundleChanged));
    }
    let project_id = ProjectId::new();
    let scratch = engine
        .staging
        .join(format!("create-plan-{}", OperationId::new()));
    std::fs::create_dir(&scratch).map_err(|_| error(M5TemplateErrorCode::Internal))?;
    let result = async {
        crate::template::materialize_template_bundle(object.path(), &scratch)
            .map_err(|_| error(M5TemplateErrorCode::TemplateBundleInvalid))?;
        let (_, scratch_identity) = alcomd_platform::resolve_directory_identity(&scratch)
            .map_err(|_| error(M5TemplateErrorCode::Internal))?;
        let reader = VpmReader::new().map_err(|_| error(M5TemplateErrorCode::Internal))?;
        let observation = reader
            .inspect_project_root(scratch.clone(), scratch_identity, 0)
            .await
            .map_err(|_| error(M5TemplateErrorCode::TemplateBundleInvalid))?;
        let project = ProjectRecord {
            project_id,
            observation,
            revision: Revision::INITIAL,
            registered_at_ms: 0,
            favorite: false,
        };
        let snapshot = inspect_package_project(&project)
            .map_err(|_| error(M5TemplateErrorCode::TemplateBundleInvalid))?;
        let candidates = candidates_from_catalog(&catalog)
            .map_err(|_| error(M5TemplateErrorCode::TemplatePlanStale))?;
        let requests = inspection
            .manifest
            .dependencies
            .iter()
            .map(|dependency| ResolveRequest {
                package_id: dependency.package_id.clone(),
                range: dependency.version_range.clone(),
                source: None,
                include_prerelease: dependency.include_prerelease,
                unity_version: Some((
                    u64::from(inspection.manifest.unity.major),
                    u64::from(inspection.manifest.unity.minor),
                )),
            })
            .collect::<Vec<_>>();
        let resolution = if requests.is_empty() {
            Resolution {
                packages: Vec::new(),
                dependency_edges: Vec::new(),
            }
        } else {
            resolve_packages(&candidates, &requests)
                .map_err(|_| error(M5TemplateErrorCode::TemplatePlanStale))?
        };
        let package_plan = build_resolution_plan(&snapshot, PlanAction::Install, &resolution)
            .map_err(map_m4_error)?;
        let initial_vpm = read_bounded(&scratch.join("Packages/vpm-manifest.json"))?;
        let initial_upm = read_bounded(&scratch.join("Packages/manifest.json"))?;
        let expected_vpm = materialize_vpm_manifest(&initial_vpm, &package_plan.change_set)
            .map_err(map_m4_error)?;
        if digest(&expected_vpm) != package_plan.change_set.vpm_manifest_sha256 {
            return Err(error(M5TemplateErrorCode::Internal));
        }
        let change_set_fingerprint = digest(
            &serde_json::to_vec(&package_plan.change_set)
                .map_err(|_| error(M5TemplateErrorCode::Internal))?,
        );
        let mut resource_digests = inspection
            .manifest
            .additional_resources
            .iter()
            .map(|resource| resource.sha256.clone())
            .collect::<Vec<_>>();
        resource_digests.sort();
        let parent_identity_sha256 = digest(&parent_identity);
        let project_summary_fingerprint = digest(
            &[
                inspection.payload_tree_sha256.as_slice(),
                change_set_fingerprint.as_slice(),
                parent_identity_sha256.as_slice(),
                target_leaf.as_bytes(),
            ]
            .concat(),
        );
        let authority = CreateProjectAuthority {
            version: 1,
            kind: "create-project".to_owned(),
            template_id: template.template_id.to_string(),
            template_revision: template.revision.get(),
            template_version: template.template_version,
            bundle_sha256: hex(&template.bundle_sha256),
            manifest_fingerprint: hex(&inspection.manifest_fingerprint),
            payload_tree_sha256: hex(&inspection.payload_tree_sha256),
            manifest_json: template.manifest_json,
            parent_path: path_text(&parent)?,
            parent_filesystem_identity: hex_slice(&parent_identity),
            parent_identity_sha256: hex(&parent_identity_sha256),
            target_leaf,
            target_path: path_text(&target)?,
            target_must_be_absent: true,
            project_id: project_id.to_string(),
            package_change_set_fingerprint: hex(&change_set_fingerprint),
            package_change_set: package_plan.change_set,
            package_source_set: package_plan.source_set,
            initial_vpm_manifest_sha256: hex(&digest(&initial_vpm)),
            initial_upm_manifest_sha256: hex(&digest(&initial_upm)),
            final_vpm_manifest_sha256: hex(&digest(&expected_vpm)),
            resource_digests,
            project_summary_fingerprint: hex(&project_summary_fingerprint),
        };
        let plan_json =
            serde_json::to_string(&authority).map_err(|_| error(M5TemplateErrorCode::Internal))?;
        Ok(TemplatePlanDraft {
            kind: TemplatePlanKind::CreateProject,
            plan_fingerprint: digest(plan_json.as_bytes()),
            plan_json,
        })
    }
    .await;
    let cleanup = std::fs::remove_dir_all(&scratch);
    if cleanup.is_err() && result.is_ok() {
        return Err(error(M5TemplateErrorCode::Internal));
    }
    result
}

pub(super) fn create_project_resource(
    plan: &TemplatePlanRecord,
) -> Result<ResourceKey, M5TemplateError> {
    let authority = authority(plan)?;
    Ok(ResourceKey::ProjectCreate {
        parent_identity_sha256: parse_digest(&authority.parent_identity_sha256)?,
        target_leaf: authority.target_leaf,
    })
}

pub(super) async fn prepare_create_project(
    engine: &TemplateEngine,
    plan: TemplatePlanRecord,
    locks: Arc<ResourceLockCoordinator>,
) -> Result<PreparedTemplateProject, M5TemplateError> {
    let authority = authority(&plan)?;
    verify_parent(&authority)?;
    let object = engine
        .objects
        .open_verified(parse_digest(&authority.bundle_sha256)?)
        .map_err(super::template_engine::map_object_error)?;
    let inspection = inspect_template_bundle(object.path())
        .map_err(|_| error(M5TemplateErrorCode::TemplateBundleInvalid))?;
    if inspection.normalized_manifest_json != authority.manifest_json
        || hex(&inspection.manifest_fingerprint) != authority.manifest_fingerprint
        || hex(&inspection.payload_tree_sha256) != authority.payload_tree_sha256
        || inspection.manifest.template_id != authority.template_id
        || inspection.manifest.template_version != authority.template_version
    {
        return Err(error(M5TemplateErrorCode::TemplateBundleChanged));
    }
    let packages = engine
        .package_materializer
        .prefetch(
            authority.package_change_set.clone(),
            authority.package_source_set.clone(),
            locks,
        )
        .await
        .map_err(map_m4_error)?;
    Ok(PreparedTemplateProject {
        authority,
        packages,
    })
}

pub(super) async fn stage_create_project(
    engine: &TemplateEngine,
    operation_id: OperationId,
    prepared: PreparedTemplateProject,
) -> Result<StagedTemplateProject, M5TemplateError> {
    let authority = prepared.authority;
    verify_parent(&authority)?;
    let target_root = PathBuf::from(&authority.target_path);
    let staging_root =
        PathBuf::from(&authority.parent_path).join(format!(".alcomd-create-{operation_id}"));
    let owner_sidecar = staging_root.with_extension("owner.json");
    if target_root.exists() {
        validate_published_target(&authority, &target_root).await?;
        remove_owned_sidecar(&owner_sidecar, operation_id, &authority)?;
        return Ok(StagedTemplateProject {
            authority,
            staging_root,
            target_root,
            owner_sidecar,
            already_published: true,
        });
    }
    remove_owned_staging(&staging_root, &owner_sidecar, operation_id, &authority)?;
    std::fs::create_dir(&staging_root).map_err(|_| error(M5TemplateErrorCode::Internal))?;
    write_owner_marker(&owner_sidecar, operation_id, &authority)?;
    test_template_kill_gate("prepared")?;
    let object = engine
        .objects
        .open_verified(parse_digest(&authority.bundle_sha256)?)
        .map_err(super::template_engine::map_object_error)?;
    crate::template::materialize_template_bundle(object.path(), &staging_root)
        .map_err(|_| error(M5TemplateErrorCode::TemplateBundleInvalid))?;
    let initial_vpm = read_bounded(&staging_root.join("Packages/vpm-manifest.json"))?;
    let initial_upm = read_bounded(&staging_root.join("Packages/manifest.json"))?;
    if hex(&digest(&initial_vpm)) != authority.initial_vpm_manifest_sha256
        || hex(&digest(&initial_upm)) != authority.initial_upm_manifest_sha256
    {
        return Err(error(M5TemplateErrorCode::TemplatePlanStale));
    }
    let (_, root_identity) = alcomd_platform::resolve_directory_identity(&staging_root)
        .map_err(|_| error(M5TemplateErrorCode::Internal))?;
    let package_evidence = engine
        .package_materializer
        .materialize(
            prepared.packages,
            StagingProjectEvidence {
                root: staging_root.clone(),
                root_identity,
                initial_vpm_manifest_sha256: parse_digest(&authority.initial_vpm_manifest_sha256)?,
                initial_upm_manifest_sha256: parse_digest(&authority.initial_upm_manifest_sha256)?,
                final_vpm_manifest_sha256: parse_digest(&authority.final_vpm_manifest_sha256)?,
            },
        )
        .await
        .map_err(map_m4_error)?;
    if hex(&package_evidence.vpm_manifest_sha256) != authority.final_vpm_manifest_sha256
        || hex(&package_evidence.upm_manifest_sha256) != authority.initial_upm_manifest_sha256
    {
        return Err(error(
            M5TemplateErrorCode::ProjectChangedDuringTemplateCreate,
        ));
    }
    write_marker(&staging_root, operation_id, &authority, "staging_complete")?;
    alcomd_platform::sync_directory(&staging_root)
        .map_err(|_| error(M5TemplateErrorCode::Internal))?;
    test_template_kill_gate("staging_complete")?;
    Ok(StagedTemplateProject {
        authority,
        staging_root,
        target_root,
        owner_sidecar,
        already_published: false,
    })
}

pub(super) fn discard_staged_create(staged: StagedTemplateProject) -> Result<(), M5TemplateError> {
    if staged.already_published {
        return Err(error(M5TemplateErrorCode::Internal));
    }
    std::fs::remove_dir_all(staged.staging_root)
        .map_err(|_| error(M5TemplateErrorCode::Internal))?;
    std::fs::remove_file(staged.owner_sidecar).map_err(|_| error(M5TemplateErrorCode::Internal))
}

pub(super) async fn publish_create_project(
    staged: StagedTemplateProject,
) -> Result<CreatedTemplateProject, M5TemplateError> {
    if !staged.already_published {
        verify_parent(&staged.authority)?;
        ensure_absent(&staged.target_root)?;
        test_template_kill_gate("target_publish_intent")?;
        std::fs::rename(&staged.staging_root, &staged.target_root)
            .map_err(|_| error(M5TemplateErrorCode::TemplateTargetExists))?;
        std::fs::remove_file(&staged.owner_sidecar)
            .map_err(|_| error(M5TemplateErrorCode::Internal))?;
        alcomd_platform::sync_directory(Path::new(&staged.authority.parent_path))
            .map_err(|_| error(M5TemplateErrorCode::Internal))?;
        test_template_kill_gate("target_published")?;
    }
    let project = validate_published_target(&staged.authority, &staged.target_root).await?;
    Ok(project)
}

#[cfg(feature = "test-kill-gates")]
fn test_template_kill_gate(checkpoint: &str) -> Result<(), M5TemplateError> {
    if std::env::var("ALCOMD_TEST_M5_TEMPLATE_KILL_GATE").as_deref() != Ok(checkpoint) {
        return Ok(());
    }
    let signal = std::env::var_os("ALCOMD_TEST_M5_TEMPLATE_KILL_SIGNAL")
        .ok_or_else(|| error(M5TemplateErrorCode::Internal))?;
    let path = PathBuf::from(signal);
    std::fs::write(&path, checkpoint.as_bytes())
        .map_err(|_| error(M5TemplateErrorCode::Internal))?;
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .map_err(|_| error(M5TemplateErrorCode::Internal))?;
    file.sync_all()
        .map_err(|_| error(M5TemplateErrorCode::Internal))?;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}

#[cfg(not(feature = "test-kill-gates"))]
fn test_template_kill_gate(_checkpoint: &str) -> Result<(), M5TemplateError> {
    Ok(())
}

async fn validate_published_target(
    authority: &CreateProjectAuthority,
    target: &Path,
) -> Result<CreatedTemplateProject, M5TemplateError> {
    let marker: CreateMarker = serde_json::from_slice(&read_bounded(&marker_path(target))?)
        .map_err(|_| error(M5TemplateErrorCode::ProjectChangedDuringTemplateCreate))?;
    if marker.version != 1
        || marker.project_id != authority.project_id
        || marker.plan_fingerprint != authority.project_summary_fingerprint
        || marker.phase != "staging_complete"
    {
        return Err(error(
            M5TemplateErrorCode::ProjectChangedDuringTemplateCreate,
        ));
    }
    let (root, identity) = alcomd_platform::resolve_directory_identity(target)
        .map_err(|_| error(M5TemplateErrorCode::ProjectChangedDuringTemplateCreate))?;
    let reader = VpmReader::new().map_err(|_| error(M5TemplateErrorCode::Internal))?;
    let observation = reader
        .inspect_project_root(root, identity, now_ms()?)
        .await
        .map_err(|_| error(M5TemplateErrorCode::ProjectChangedDuringTemplateCreate))?;
    if hex(&digest(&read_bounded(
        &target.join("Packages/vpm-manifest.json"),
    )?)) != authority.final_vpm_manifest_sha256
        || hex(&digest(&read_bounded(
            &target.join("Packages/manifest.json"),
        )?)) != authority.initial_upm_manifest_sha256
    {
        return Err(error(
            M5TemplateErrorCode::ProjectChangedDuringTemplateCreate,
        ));
    }
    Ok(CreatedTemplateProject {
        project_id: ProjectId::parse(&authority.project_id)
            .map_err(|_| error(M5TemplateErrorCode::Internal))?,
        observation,
    })
}

fn authority(plan: &TemplatePlanRecord) -> Result<CreateProjectAuthority, M5TemplateError> {
    if plan.kind != TemplatePlanKind::CreateProject
        || digest(plan.plan_json.as_bytes()) != plan.plan_fingerprint
    {
        return Err(error(M5TemplateErrorCode::TemplatePlanStale));
    }
    let authority: CreateProjectAuthority = serde_json::from_str(&plan.plan_json)
        .map_err(|_| error(M5TemplateErrorCode::TemplatePlanStale))?;
    if authority.version != 1
        || authority.kind != "create-project"
        || !authority.target_must_be_absent
        || ProjectId::parse(&authority.project_id).is_err()
        || hex(&digest(
            &serde_json::to_vec(&authority.package_change_set)
                .map_err(|_| error(M5TemplateErrorCode::Internal))?,
        )) != authority.package_change_set_fingerprint
        || authority.package_change_set.vpm_manifest_sha256
            != parse_digest(&authority.final_vpm_manifest_sha256)?
    {
        return Err(error(M5TemplateErrorCode::TemplatePlanStale));
    }
    validate_target_leaf(&authority.target_leaf)?;
    Ok(authority)
}

fn verify_parent(authority: &CreateProjectAuthority) -> Result<(), M5TemplateError> {
    let parent = PathBuf::from(&authority.parent_path);
    let (canonical, identity) = alcomd_platform::resolve_directory_identity(&parent)
        .map_err(|_| error(M5TemplateErrorCode::TemplatePlanStale))?;
    if path_text(&canonical)? != authority.parent_path
        || hex_slice(&identity) != authority.parent_filesystem_identity
        || hex(&digest(&identity)) != authority.parent_identity_sha256
        || canonical.join(&authority.target_leaf) != Path::new(&authority.target_path)
    {
        return Err(error(M5TemplateErrorCode::TemplatePlanStale));
    }
    Ok(())
}

fn validate_target_leaf(value: &str) -> Result<(), M5TemplateError> {
    let nfc = value.nfc().collect::<String>();
    let stem = value
        .split('.')
        .next()
        .unwrap_or(value)
        .to_ascii_uppercase();
    let reserved = matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );
    if value.is_empty()
        || value.len() > 255
        || nfc != value
        || matches!(value, "." | "..")
        || value.ends_with([' ', '.'])
        || value
            .chars()
            .any(|character| character.is_control() || matches!(character, '/' | '\\' | ':'))
        || reserved
    {
        return Err(error(M5TemplateErrorCode::InvalidInput));
    }
    Ok(())
}

fn ensure_absent(path: &Path) -> Result<(), M5TemplateError> {
    match std::fs::symlink_metadata(path) {
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(error(M5TemplateErrorCode::TemplateTargetExists)),
        Err(_) => Err(error(M5TemplateErrorCode::InvalidInput)),
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateMarker {
    version: u32,
    project_id: String,
    plan_fingerprint: String,
    phase: String,
}

fn marker_path(root: &Path) -> PathBuf {
    root.join("Library/ALCOMD/create-project-evidence.json")
}

fn write_marker(
    root: &Path,
    operation_id: OperationId,
    authority: &CreateProjectAuthority,
    phase: &str,
) -> Result<(), M5TemplateError> {
    let directory = root.join("Library/ALCOMD");
    std::fs::create_dir_all(&directory).map_err(|_| error(M5TemplateErrorCode::Internal))?;
    let marker = CreateMarker {
        version: 1,
        project_id: authority.project_id.clone(),
        plan_fingerprint: authority.project_summary_fingerprint.clone(),
        phase: phase.to_owned(),
    };
    let bytes = serde_json::to_vec(&marker).map_err(|_| error(M5TemplateErrorCode::Internal))?;
    let path = marker_path(root);
    let temporary = directory.join(format!("create-project-{operation_id}.new"));
    use std::io::Write;
    let mut temporary_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|_| error(M5TemplateErrorCode::Internal))?;
    temporary_file
        .write_all(&bytes)
        .and_then(|()| temporary_file.sync_all())
        .map_err(|_| error(M5TemplateErrorCode::Internal))?;
    drop(temporary_file);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|_| error(M5TemplateErrorCode::Internal))?;
    }
    std::fs::rename(temporary, path).map_err(|_| error(M5TemplateErrorCode::Internal))?;
    alcomd_platform::sync_directory(&directory).map_err(|_| error(M5TemplateErrorCode::Internal))
}

fn write_owner_marker(
    path: &Path,
    operation_id: OperationId,
    authority: &CreateProjectAuthority,
) -> Result<(), M5TemplateError> {
    let marker = CreateOwnerMarker {
        version: 1,
        operation_id: operation_id.to_string(),
        project_id: authority.project_id.clone(),
        plan_fingerprint: authority.project_summary_fingerprint.clone(),
    };
    let bytes = serde_json::to_vec(&marker).map_err(|_| error(M5TemplateErrorCode::Internal))?;
    let mut options = std::fs::OpenOptions::new();
    let file = options
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| error(M5TemplateErrorCode::Internal))?;
    use std::io::Write;
    let mut file = file;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| error(M5TemplateErrorCode::Internal))?;
    alcomd_platform::sync_directory(
        path.parent()
            .ok_or_else(|| error(M5TemplateErrorCode::Internal))?,
    )
    .map_err(|_| error(M5TemplateErrorCode::Internal))
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CreateOwnerMarker {
    version: u32,
    operation_id: String,
    project_id: String,
    plan_fingerprint: String,
}

fn remove_owned_staging(
    staging: &Path,
    owner_sidecar: &Path,
    operation_id: OperationId,
    authority: &CreateProjectAuthority,
) -> Result<(), M5TemplateError> {
    match std::fs::symlink_metadata(staging) {
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            return remove_owned_sidecar(owner_sidecar, operation_id, authority);
        }
        Ok(metadata) if metadata.is_dir() && !is_link_or_reparse(&metadata) => {}
        _ => {
            return Err(error(
                M5TemplateErrorCode::ProjectChangedDuringTemplateCreate,
            ));
        }
    }
    let marker: CreateOwnerMarker = serde_json::from_slice(&read_bounded(owner_sidecar)?)
        .map_err(|_| error(M5TemplateErrorCode::ProjectChangedDuringTemplateCreate))?;
    if marker.version != 1
        || marker.operation_id != operation_id.to_string()
        || marker.project_id != authority.project_id
        || marker.plan_fingerprint != authority.project_summary_fingerprint
    {
        return Err(error(
            M5TemplateErrorCode::ProjectChangedDuringTemplateCreate,
        ));
    }
    std::fs::remove_dir_all(staging).map_err(|_| error(M5TemplateErrorCode::Internal))?;
    std::fs::remove_file(owner_sidecar).map_err(|_| error(M5TemplateErrorCode::Internal))
}

fn remove_owned_sidecar(
    path: &Path,
    operation_id: OperationId,
    authority: &CreateProjectAuthority,
) -> Result<(), M5TemplateError> {
    match std::fs::symlink_metadata(path) {
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Ok(metadata) if metadata.is_file() && !is_link_or_reparse(&metadata) => {}
        _ => {
            return Err(error(
                M5TemplateErrorCode::ProjectChangedDuringTemplateCreate,
            ));
        }
    }
    let marker: CreateOwnerMarker = serde_json::from_slice(&read_bounded(path)?)
        .map_err(|_| error(M5TemplateErrorCode::ProjectChangedDuringTemplateCreate))?;
    if marker.version != 1
        || marker.operation_id != operation_id.to_string()
        || marker.project_id != authority.project_id
        || marker.plan_fingerprint != authority.project_summary_fingerprint
    {
        return Err(error(
            M5TemplateErrorCode::ProjectChangedDuringTemplateCreate,
        ));
    }
    std::fs::remove_file(path).map_err(|_| error(M5TemplateErrorCode::Internal))
}

fn read_bounded(path: &Path) -> Result<Vec<u8>, M5TemplateError> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| error(M5TemplateErrorCode::ProjectChangedDuringTemplateCreate))?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) || metadata.len() > 4 * 1024 * 1024 {
        return Err(error(
            M5TemplateErrorCode::ProjectChangedDuringTemplateCreate,
        ));
    }
    std::fs::read(path).map_err(|_| error(M5TemplateErrorCode::ProjectChangedDuringTemplateCreate))
}

fn path_text(path: &Path) -> Result<String, M5TemplateError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| error(M5TemplateErrorCode::InvalidInput))
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn hex(value: &[u8; 32]) -> String {
    hex_slice(value)
}

fn hex_slice(value: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(value.len() * 2);
    for byte in value {
        result.push(char::from(HEX[(byte >> 4) as usize]));
        result.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    result
}

fn parse_digest(value: &str) -> Result<[u8; 32], M5TemplateError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(error(M5TemplateErrorCode::TemplatePlanStale));
    }
    let mut digest = [0_u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(chunk)
            .map_err(|_| error(M5TemplateErrorCode::TemplatePlanStale))?;
        digest[index] = u8::from_str_radix(text, 16)
            .map_err(|_| error(M5TemplateErrorCode::TemplatePlanStale))?;
    }
    Ok(digest)
}

fn now_ms() -> Result<u64, M5TemplateError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| error(M5TemplateErrorCode::Internal))
        .and_then(|duration| {
            u64::try_from(duration.as_millis()).map_err(|_| error(M5TemplateErrorCode::Internal))
        })
}

fn map_m4_error(_: alcomd_application::M4Error) -> M5TemplateError {
    error(M5TemplateErrorCode::TemplatePlanStale)
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

const fn error(code: M5TemplateErrorCode) -> M5TemplateError {
    M5TemplateError::new(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    use alcomd_application::{PlanId, PrincipalId, TemplatePlanState, TemplateSourceKind};

    static NEXT_TEMPORARY_PATH: AtomicU64 = AtomicU64::new(0);

    fn temporary(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "alcomd-m5-template-create-{name}-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos(),
            NEXT_TEMPORARY_PATH.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[tokio::test]
    async fn blank_template_create_is_frozen_staged_published_and_recoverable() {
        let root = temporary("root");
        let parent = temporary("parent");
        let builtin_staging = temporary("builtins");
        std::fs::create_dir_all(&root).expect("root");
        std::fs::create_dir_all(&parent).expect("parent");
        let engine = TemplateEngine::new(root.clone()).expect("engine");
        let template = engine
            .materialize_builtins(&builtin_staging)
            .expect("builtins")
            .into_iter()
            .find(|record| {
                record.source_kind == TemplateSourceKind::Builtin
                    && serde_json::from_str::<serde_json::Value>(&record.manifest_json)
                        .ok()
                        .and_then(|value| value.get("displayName").cloned())
                        .and_then(|value| value.as_str().map(str::to_owned))
                        .as_deref()
                        == Some("Blank")
            })
            .expect("blank");
        let draft = plan_create_project(
            &engine,
            template,
            ResolverCatalog {
                entries: Vec::new(),
                complete: true,
            },
            path_text(&parent).expect("parent path"),
            "Created Project".to_owned(),
        )
        .await
        .expect("plan");
        let plan = TemplatePlanRecord {
            plan_id: PlanId::new(),
            owner: PrincipalId::local_owner(),
            kind: TemplatePlanKind::CreateProject,
            state: TemplatePlanState::Applied,
            plan_json: draft.plan_json,
            plan_fingerprint: draft.plan_fingerprint,
            apply_operation_id: None,
            created_at_ms: 0,
        };
        let resource = create_project_resource(&plan).expect("resource");
        let locks = Arc::new(ResourceLockCoordinator::default());
        let prepared = prepare_create_project(&engine, plan.clone(), Arc::clone(&locks))
            .await
            .expect("prepare");
        let _guard = locks.acquire(vec![resource]).await;
        let operation_id = OperationId::new();
        let staged = stage_create_project(&engine, operation_id, prepared)
            .await
            .expect("stage");
        let external_target = parent.join("Created Project");
        std::fs::create_dir(&external_target).expect("external target wins publish race");
        std::fs::write(external_target.join("owner.txt"), b"external").expect("external sentinel");
        assert_eq!(
            publish_create_project(staged)
                .await
                .expect_err("external target must fail closed")
                .code(),
            M5TemplateErrorCode::TemplateTargetExists
        );
        assert_eq!(
            std::fs::read(external_target.join("owner.txt")).expect("external sentinel preserved"),
            b"external"
        );
        std::fs::remove_dir_all(&external_target).expect("remove external target");
        let prepared = prepare_create_project(&engine, plan.clone(), Arc::clone(&locks))
            .await
            .expect("prepare after publish race");
        let staged = stage_create_project(&engine, operation_id, prepared)
            .await
            .expect("rebuild owned staging after publish race");
        let created = publish_create_project(staged).await.expect("publish");
        assert_eq!(
            created.project_id,
            ProjectId::parse(&authority(&plan).expect("authority").project_id).expect("project id")
        );
        assert_eq!(
            created.observation.root_path,
            path_text(
                &std::fs::canonicalize(&parent)
                    .expect("canonical parent")
                    .join("Created Project")
            )
            .expect("target")
        );
        assert!(
            parent
                .join("Created Project/Packages/manifest.json")
                .is_file()
        );
        assert!(marker_path(&parent.join("Created Project")).is_file());

        let recovery_prepared = prepare_create_project(&engine, plan, Arc::clone(&locks))
            .await
            .expect("recovery prepare");
        let recovered_stage = stage_create_project(&engine, operation_id, recovery_prepared)
            .await
            .expect("recover published target");
        assert!(recovered_stage.already_published);
        let mut recovered = publish_create_project(recovered_stage)
            .await
            .expect("recovered publish");
        recovered.observation.observed_at_ms = created.observation.observed_at_ms;
        assert_eq!(recovered, created);
        std::fs::remove_dir_all(root).expect("remove root");
        std::fs::remove_dir_all(parent).expect("remove parent");
        std::fs::remove_dir_all(builtin_staging).expect("remove builtins");
    }

    #[tokio::test]
    async fn plan_rejects_existing_target_and_non_normalized_leaf() {
        let root = temporary("reject-root");
        let parent = temporary("reject-parent");
        let builtin_staging = temporary("reject-builtins");
        std::fs::create_dir_all(&root).expect("root");
        std::fs::create_dir_all(parent.join("Existing")).expect("target");
        let engine = TemplateEngine::new(root.clone()).expect("engine");
        let template = engine
            .materialize_builtins(&builtin_staging)
            .expect("builtins")
            .into_iter()
            .next()
            .expect("template");
        let catalog = ResolverCatalog {
            entries: Vec::new(),
            complete: true,
        };
        assert_eq!(
            plan_create_project(
                &engine,
                template.clone(),
                catalog.clone(),
                path_text(&parent).expect("parent"),
                "Existing".to_owned(),
            )
            .await
            .expect_err("existing target")
            .code(),
            M5TemplateErrorCode::TemplateTargetExists
        );
        assert_eq!(
            plan_create_project(
                &engine,
                template,
                catalog,
                path_text(&parent).expect("parent"),
                "cafe\u{301}".to_owned(),
            )
            .await
            .expect_err("non-NFC leaf")
            .code(),
            M5TemplateErrorCode::InvalidInput
        );
        std::fs::remove_dir_all(root).expect("remove root");
        std::fs::remove_dir_all(parent).expect("remove parent");
        std::fs::remove_dir_all(builtin_staging).expect("remove builtins");
    }
}
