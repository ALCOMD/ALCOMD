use std::path::{Path, PathBuf};

use alcomd_application::{
    M5TemplateAdapter, M5TemplateError, M5TemplateErrorCode, PublishedTemplate, Revision,
    StoredTemplateRecord, TemplateBundleEvidence, TemplateId, TemplatePlanDraft, TemplatePlanKind,
    TemplatePlanRecord, TemplateRecord, TemplateSourceKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    TemplateManifest, TemplateObjectErrorCode, TemplateObjectStore, TemplateProvenanceKind,
    inspect_template_bundle,
};

#[derive(Clone)]
pub struct TemplateEngine {
    pub(crate) objects: TemplateObjectStore,
    pub(crate) staging: PathBuf,
    pub(crate) package_materializer: crate::FrozenPackageMaterializer,
}

impl TemplateEngine {
    pub fn new(root: PathBuf) -> Result<Self, M5TemplateError> {
        let package_cache = root.join("package-cache");
        Self::with_package_cache(root, package_cache)
    }

    pub fn with_package_cache(
        root: PathBuf,
        package_cache: PathBuf,
    ) -> Result<Self, M5TemplateError> {
        let staging = root.join("staging");
        std::fs::create_dir_all(&staging).map_err(|_| error(M5TemplateErrorCode::Internal))?;
        Ok(Self {
            objects: TemplateObjectStore::new(root.join("objects")).map_err(map_object_error)?,
            staging,
            package_materializer: crate::FrozenPackageMaterializer::new(package_cache)
                .map_err(|_| error(M5TemplateErrorCode::Internal))?,
        })
    }
}

impl M5TemplateAdapter for TemplateEngine {
    type PreparedCreateProject = crate::template_create::PreparedTemplateProject;
    type StagedCreateProject = crate::template_create::StagedTemplateProject;
    async fn inspect_bundle(
        &self,
        source_path: String,
    ) -> Result<TemplateBundleEvidence, M5TemplateError> {
        inspect_evidence(Path::new(&source_path))
    }

    async fn present_record(
        &self,
        record: StoredTemplateRecord,
    ) -> Result<TemplateRecord, M5TemplateError> {
        let manifest = parse_manifest_record(&record)?;
        let manifest_fingerprint = Sha256::digest(record.manifest_json.as_bytes()).into();
        Ok(TemplateRecord {
            template_id: record.template_id,
            source_kind: record.source_kind,
            template_version: record.template_version,
            display_name: manifest.display_name,
            description: manifest.description,
            provenance: provenance_name(manifest.provenance.created_by).to_owned(),
            manifest_json: record.manifest_json,
            bundle_sha256: record.bundle_sha256,
            manifest_fingerprint,
            favorite: record.favorite,
            revision: record.revision,
            created_at_ms: record.created_at_ms,
            updated_at_ms: record.updated_at_ms,
        })
    }

    async fn import_plan(
        &self,
        evidence: TemplateBundleEvidence,
        existing: Option<StoredTemplateRecord>,
        override_existing: bool,
        expected_revision: Option<Revision>,
    ) -> Result<TemplatePlanDraft, M5TemplateError> {
        let (expected_revision_value, old_bundle_sha256) = match existing {
            None if !override_existing && expected_revision.is_none() => (0, None),
            Some(existing)
                if existing.source_kind == TemplateSourceKind::User
                    && existing.bundle_sha256 == evidence.bundle_sha256
                    && expected_revision.is_none_or(|value| value == existing.revision) =>
            {
                (existing.revision.get(), Some(hex(&existing.bundle_sha256)))
            }
            Some(existing)
                if existing.source_kind == TemplateSourceKind::User
                    && expected_revision == Some(existing.revision)
                    && (existing.bundle_sha256 == evidence.bundle_sha256 || override_existing) =>
            {
                (existing.revision.get(), Some(hex(&existing.bundle_sha256)))
            }
            Some(existing) if existing.source_kind == TemplateSourceKind::Builtin => {
                return Err(error(M5TemplateErrorCode::TemplateImmutable));
            }
            _ => return Err(error(M5TemplateErrorCode::TemplateConflict)),
        };
        let authority = ImportPlanAuthority {
            version: 1,
            kind: "import".to_owned(),
            template_id: evidence.template_id.to_string(),
            expected_revision: expected_revision_value,
            source_path: evidence.source_path,
            source_filesystem_identity: hex_slice(&evidence.source_filesystem_identity),
            old_bundle_sha256,
            new_bundle_sha256: hex(&evidence.bundle_sha256),
            manifest_fingerprint: hex(&evidence.manifest_fingerprint),
            template_version: evidence.template_version,
            manifest_json: evidence.manifest_json,
        };
        let plan_json =
            serde_json::to_string(&authority).map_err(|_| error(M5TemplateErrorCode::Internal))?;
        Ok(TemplatePlanDraft {
            kind: TemplatePlanKind::Import,
            plan_fingerprint: Sha256::digest(plan_json.as_bytes()).into(),
            plan_json,
        })
    }

    async fn publish_import(
        &self,
        plan: TemplatePlanRecord,
    ) -> Result<PublishedTemplate, M5TemplateError> {
        if plan.kind != TemplatePlanKind::Import {
            return Err(error(M5TemplateErrorCode::Internal));
        }
        let authority: ImportPlanAuthority = serde_json::from_str(&plan.plan_json)
            .map_err(|_| error(M5TemplateErrorCode::Internal))?;
        let evidence = inspect_evidence(Path::new(&authority.source_path))?;
        if authority.version != 1
            || authority.kind != "import"
            || evidence.template_id.to_string() != authority.template_id
            || evidence.template_version != authority.template_version
            || evidence.manifest_json != authority.manifest_json
            || hex_slice(&evidence.source_filesystem_identity)
                != authority.source_filesystem_identity
            || hex(&evidence.bundle_sha256) != authority.new_bundle_sha256
            || hex(&evidence.manifest_fingerprint) != authority.manifest_fingerprint
        {
            return Err(error(M5TemplateErrorCode::TemplateBundleChanged));
        }
        let object = self
            .objects
            .publish(Path::new(&authority.source_path), evidence.bundle_sha256)
            .map_err(map_object_error)?;
        Ok(PublishedTemplate {
            record: StoredTemplateRecord {
                template_id: evidence.template_id,
                source_kind: TemplateSourceKind::User,
                template_version: evidence.template_version,
                manifest_json: evidence.manifest_json,
                payload_locator: object.locator,
                bundle_sha256: evidence.bundle_sha256,
                favorite: false,
                revision: Revision::INITIAL,
                created_at_ms: 0,
                updated_at_ms: 0,
            },
        })
    }

    async fn export_bundle(
        &self,
        template: StoredTemplateRecord,
        target_path: String,
    ) -> Result<(), M5TemplateError> {
        if !Path::new(&target_path).is_absolute() {
            return Err(error(M5TemplateErrorCode::InvalidInput));
        }
        self.objects
            .export_create_new(template.bundle_sha256, Path::new(&target_path))
            .map_err(map_object_error)
    }

    async fn derive_plan(
        &self,
        project: alcomd_application::ProjectRecord,
        catalog: alcomd_application::ResolverCatalog,
        template_id: TemplateId,
        template_version: String,
        display_name: String,
        description: Option<String>,
        writer_state: alcomd_application::UnityWriterStateKind,
    ) -> Result<TemplatePlanDraft, M5TemplateError> {
        crate::template_derive::derive_plan(
            project,
            catalog,
            template_id,
            template_version,
            display_name,
            description,
            writer_state,
        )
    }

    fn derive_project_id(
        &self,
        plan: &TemplatePlanRecord,
    ) -> Result<alcomd_application::ProjectId, M5TemplateError> {
        crate::template_derive::derive_project_id(plan)
    }

    async fn publish_derive(
        &self,
        plan: TemplatePlanRecord,
        project: alcomd_application::ProjectRecord,
    ) -> Result<PublishedTemplate, M5TemplateError> {
        crate::template_derive::publish_derive(self, plan, project)
    }

    async fn create_project_plan(
        &self,
        template: StoredTemplateRecord,
        catalog: alcomd_application::ResolverCatalog,
        target_parent: String,
        target_leaf: String,
    ) -> Result<TemplatePlanDraft, M5TemplateError> {
        crate::template_create::plan_create_project(
            self,
            template,
            catalog,
            target_parent,
            target_leaf,
        )
        .await
    }

    fn create_project_resource(
        &self,
        plan: &TemplatePlanRecord,
    ) -> Result<alcomd_application::ResourceKey, M5TemplateError> {
        crate::template_create::create_project_resource(plan)
    }

    async fn prepare_create_project(
        &self,
        plan: TemplatePlanRecord,
        locks: std::sync::Arc<alcomd_application::ResourceLockCoordinator>,
    ) -> Result<Self::PreparedCreateProject, M5TemplateError> {
        crate::template_create::prepare_create_project(self, plan, locks).await
    }

    async fn stage_create_project(
        &self,
        operation_id: alcomd_application::OperationId,
        prepared: Self::PreparedCreateProject,
    ) -> Result<Self::StagedCreateProject, M5TemplateError> {
        crate::template_create::stage_create_project(self, operation_id, prepared).await
    }

    async fn discard_staged_create(
        &self,
        staged: Self::StagedCreateProject,
    ) -> Result<(), M5TemplateError> {
        crate::template_create::discard_staged_create(staged)
    }

    async fn publish_create_project(
        &self,
        staged: Self::StagedCreateProject,
    ) -> Result<alcomd_application::CreatedTemplateProject, M5TemplateError> {
        crate::template_create::publish_create_project(staged).await
    }
}

fn inspect_evidence(path: &Path) -> Result<TemplateBundleEvidence, M5TemplateError> {
    if !path.is_absolute() {
        return Err(error(M5TemplateErrorCode::InvalidInput));
    }
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| error(M5TemplateErrorCode::TemplateBundleInvalid))?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) {
        return Err(error(M5TemplateErrorCode::TemplateBundleInvalid));
    }
    let identity = alcomd_platform::file_identity_key(path)
        .map_err(|_| error(M5TemplateErrorCode::TemplateBundleInvalid))?;
    let inspection = inspect_template_bundle(path)
        .map_err(|_| error(M5TemplateErrorCode::TemplateBundleInvalid))?;
    let template_id = TemplateId::parse(&inspection.manifest.template_id)
        .map_err(|_| error(M5TemplateErrorCode::TemplateBundleInvalid))?;
    Ok(TemplateBundleEvidence {
        template_id,
        template_version: inspection.manifest.template_version,
        display_name: inspection.manifest.display_name,
        description: inspection.manifest.description,
        provenance: provenance_name(inspection.manifest.provenance.created_by).to_owned(),
        manifest_json: inspection.normalized_manifest_json,
        bundle_sha256: inspection.bundle_sha256,
        manifest_fingerprint: inspection.manifest_fingerprint,
        payload_tree_sha256: inspection.payload_tree_sha256,
        entry_count: inspection.entry_count,
        total_bytes: inspection.total_uncompressed_bytes,
        source_path: path
            .to_str()
            .ok_or_else(|| error(M5TemplateErrorCode::InvalidInput))?
            .to_owned(),
        source_filesystem_identity: identity,
    })
}

fn parse_manifest_record(
    record: &StoredTemplateRecord,
) -> Result<TemplateManifest, M5TemplateError> {
    let manifest: TemplateManifest = serde_json::from_str(&record.manifest_json)
        .map_err(|_| error(M5TemplateErrorCode::Internal))?;
    if manifest.template_id != record.template_id.to_string()
        || manifest.template_version != record.template_version
    {
        return Err(error(M5TemplateErrorCode::Internal));
    }
    Ok(manifest)
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImportPlanAuthority {
    version: u32,
    kind: String,
    template_id: String,
    expected_revision: u64,
    source_path: String,
    source_filesystem_identity: String,
    old_bundle_sha256: Option<String>,
    new_bundle_sha256: String,
    manifest_fingerprint: String,
    template_version: String,
    manifest_json: String,
}

fn provenance_name(value: TemplateProvenanceKind) -> &'static str {
    match value {
        TemplateProvenanceKind::Authored => "authored",
        TemplateProvenanceKind::Imported => "imported",
        TemplateProvenanceKind::Derived => "derived",
    }
}

pub(crate) fn map_object_error(source: crate::TemplateObjectError) -> M5TemplateError {
    match source.code() {
        TemplateObjectErrorCode::ObjectMissing => error(M5TemplateErrorCode::TemplateObjectMissing),
        TemplateObjectErrorCode::TargetExists => error(M5TemplateErrorCode::TemplateTargetExists),
        TemplateObjectErrorCode::DigestMismatch => {
            error(M5TemplateErrorCode::TemplateBundleChanged)
        }
        TemplateObjectErrorCode::Io => error(M5TemplateErrorCode::Internal),
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

const fn error(code: M5TemplateErrorCode) -> M5TemplateError {
    M5TemplateError::new(code)
}
