use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use alcomd_application::{
    OperationId, ProjectCreateAdapter, ProjectCreateError, ProjectCreateErrorCode,
    ProjectCreateInput, ProjectCreatePackages, ProjectCreateWork, ProjectObservation,
    ProjectRecord, ProjectTemplateDeclaration, ProjectTemplateJobRequest, ProjectTemplateSource,
    RepositoryArchivePinStore, ResolverCatalog, ResourceKey, ResourceLockCoordinator, Revision,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use crate::range::VpmRange;
use crate::template_archive::TemplateArchiveStore;
use crate::{
    FrozenPackageMaterializer, ResolveRequest, StagingProjectEvidence, UnityVersion, VpmReader,
    candidates_from_catalog, inspect_package_project, project_creation_resource,
};

const UPM_MANIFEST: &[u8] = b"{\"dependencies\":{}}\n";
const VPM_MANIFEST: &[u8] = b"{\"dependencies\":{},\"locked\":{}}\n";
const PROJECT_VERSION_PATH: &[&str] = &["ProjectSettings", "ProjectVersion.txt"];
const OWNER_MARKER: &str = "project-create.json";

#[derive(Clone)]
pub struct ProjectCreateEngine<S> {
    store: S,
    reader: VpmReader,
    materializer: FrozenPackageMaterializer,
    templates: TemplateArchiveStore,
}

impl<S> ProjectCreateEngine<S> {
    pub fn new(
        store: S,
        reader: VpmReader,
        cache_root: PathBuf,
    ) -> Result<Self, ProjectCreateError> {
        let templates =
            TemplateArchiveStore::new(cache_root.parent().ok_or_else(invalid)?.join("templates"))?;
        let cache = reader
            .package_cache(cache_root)
            .map_err(|cause| internal().with_source(cause))?;
        Ok(Self {
            store,
            reader,
            materializer: FrozenPackageMaterializer::new(cache),
            templates,
        })
    }

    async fn observe(
        &self,
        path: &Path,
        identity: Vec<u8>,
        work: &ProjectCreateWork,
    ) -> Result<ProjectObservation, ProjectCreateError> {
        if directory_identity(path)? != identity {
            return Err(destination_changed(path));
        }
        let expected_manifest = |name, default: &[u8]| {
            work.declaration.as_ref().map_or_else(
                || Ok(Sha256::digest(default).into()),
                |declaration| self.templates.manifest_digest(declaration, name, default),
            )
        };
        let expected_vpm = match &work.packages {
            Some(packages) => packages.change_set.vpm_manifest_sha256,
            None => expected_manifest("Packages/vpm-manifest.json", VPM_MANIFEST)?,
        };
        let version = project_version(work);
        let generated_version_digest: [u8; 32] = Sha256::digest(&version).into();
        let files = [
            (
                &["Packages", "manifest.json"][..],
                expected_manifest("Packages/manifest.json", UPM_MANIFEST)?,
            ),
            (&["Packages", "vpm-manifest.json"][..], expected_vpm),
            (
                PROJECT_VERSION_PATH,
                expected_manifest("ProjectSettings/ProjectVersion.txt", version.as_bytes())?,
            ),
        ];
        for (components, expected) in files {
            let bytes = crate::plan::read_regular_component(
                path,
                components,
                crate::PROJECT_MANIFEST_LIMIT,
            )
            .map_err(|cause| destination_changed(path).with_source(cause))?;
            let actual: [u8; 32] = Sha256::digest(bytes).into();
            // Older workers could publish a version file without the template's revision line.
            if actual != expected
                && !(components == PROJECT_VERSION_PATH && actual == generated_version_digest)
            {
                return Err(destination_changed(path));
            }
        }
        let observation = self
            .reader
            .inspect_project_root(path.to_owned(), identity.clone())
            .await
            .map_err(|cause| destination_changed(path).with_source(cause))?;
        if directory_identity(path)? != identity {
            return Err(destination_changed(path));
        }
        Ok(observation)
    }
}

#[doc(hidden)]
pub struct ProjectCreateStage {
    root: PathBuf,
    identity: Vec<u8>,
    vpm_digest: [u8; 32],
    upm_digest: [u8; 32],
}

#[derive(Serialize, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OwnerMarker {
    operation_id: String,
    project_id: String,
    root_identity: Vec<u8>,
}

impl<S: RepositoryArchivePinStore> ProjectCreateAdapter for ProjectCreateEngine<S> {
    type Stage = ProjectCreateStage;

    fn prepare_template(
        &self,
        source: ProjectTemplateSource,
        project: Option<ProjectRecord>,
    ) -> Result<ProjectTemplateDeclaration, ProjectCreateError> {
        self.templates.prepare(source, project)
    }

    fn builtin_templates(&self) -> Result<Vec<ProjectTemplateDeclaration>, ProjectCreateError> {
        self.templates.builtins()
    }

    fn prune_template_archives(
        &self,
        referenced: &BTreeSet<String>,
    ) -> Result<(), ProjectCreateError> {
        self.templates.prune_unreferenced(referenced)
    }

    fn template_job_fingerprint(
        &self,
        request: &ProjectTemplateJobRequest,
    ) -> Result<String, ProjectCreateError> {
        fingerprint(request)
    }

    fn export_template(
        &self,
        declaration: &ProjectTemplateDeclaration,
        destination_file: &str,
        id: OperationId,
    ) -> Result<(), ProjectCreateError> {
        self.templates.export(declaration, destination_file, id)
    }

    fn finish_template_export(
        &self,
        destination_file: &str,
        id: OperationId,
    ) -> Result<(), ProjectCreateError> {
        self.templates.finish_export(destination_file, id)
    }

    fn prepare_destination(
        &self,
        input: &ProjectCreateInput,
    ) -> Result<(String, Vec<u8>), ProjectCreateError> {
        validate_leaf(&input.name)?;
        let requested = Path::new(&input.target_parent);
        let (parent, identity) = alcomd_platform::resolve_directory_identity(requested)
            .map_err(|cause| invalid().with_source(cause).with_path(&input.target_parent))?;
        ensure_absent(&parent.join(&input.name))?;
        let parent = parent.to_str().ok_or_else(invalid)?.to_owned();
        Ok((parent, identity))
    }

    fn fingerprint(&self, input: &ProjectCreateInput) -> Result<String, ProjectCreateError> {
        fingerprint(input)
    }

    fn target_resource(&self, work: &ProjectCreateWork) -> ResourceKey {
        project_creation_resource(&work.parent_identity, work.input.name.clone())
    }

    fn stage(
        &self,
        id: OperationId,
        work: &ProjectCreateWork,
    ) -> Result<Self::Stage, ProjectCreateError> {
        verify_parent(work)?;
        self.discard(id, work)?;
        let container = stage_container(id, work);
        fs::create_dir(&container).map_err(|cause| filesystem_error(&container, cause))?;
        let container_identity = directory_identity(&container)?;
        let marker = serde_json::to_vec(&owner_marker(id, work, &container_identity))
            .map_err(|cause| internal().with_source(cause))?;
        write_new(&container.join(OWNER_MARKER), &marker)?;
        // Discovery inspects direct children of the projects directory.
        let root = container.join("project");
        fs::create_dir(&root).map_err(|cause| filesystem_error(&root, cause))?;
        let identity = directory_identity(&root)?;
        if let Some(declaration) = &work.declaration {
            self.templates.expand(declaration, &root)?;
        }
        for relative in ["Library", "Library/ALCOMD"] {
            let path = root.join(relative);
            fs::create_dir(&path).map_err(|cause| filesystem_error(&path, cause))?;
        }
        let marker = serde_json::to_vec(&owner_marker(id, work, &identity))
            .map_err(|cause| internal().with_source(cause))?;
        write_new(&root.join("Library/ALCOMD").join(OWNER_MARKER), &marker)?;
        for relative in ["Assets", "Packages", "ProjectSettings"] {
            let path = root.join(relative);
            if !exists(&path)? {
                fs::create_dir(&path).map_err(|cause| filesystem_error(&path, cause))?;
            }
        }
        if work.declaration.is_some() {
            let product_guid = work.project_id.to_string().replace('-', "");
            apply_product_name(&root, &work.input.name, &product_guid)?;
        }
        let upm_digest = scaffold_manifest(&root, "manifest.json", UPM_MANIFEST)?;
        let vpm_digest = scaffold_manifest(&root, "vpm-manifest.json", VPM_MANIFEST)?;
        let version = root.join("ProjectSettings/ProjectVersion.txt");
        if !exists(&version)? {
            write_new(&version, project_version(work).as_bytes())?;
        }
        sync_stage(&root)?;
        alcomd_platform::sync_directory(&container)
            .map_err(|cause| filesystem_error(&container, cause))?;
        Ok(ProjectCreateStage {
            root,
            identity,
            vpm_digest,
            upm_digest,
        })
    }

    async fn resolve(
        &self,
        stage: &Self::Stage,
        work: &ProjectCreateWork,
        catalog: ResolverCatalog,
    ) -> Result<ProjectCreatePackages, ProjectCreateError> {
        let declaration = work.declaration.as_ref().ok_or_else(internal)?;
        if !catalog.complete
            || declaration
                .editor_version
                .as_deref()
                .is_some_and(|version| version != work.input.unity_version)
        {
            return Err(dependency_error());
        }
        let mut candidates = candidates_from_catalog(&catalog)
            .map_err(|cause| dependency_error().with_source(cause))?;
        let unity_version = UnityVersion::parse_editor(&work.input.unity_version);
        let direct = declaration
            .dependencies
            .iter()
            .map(|dependency| dependency.package_id.as_str())
            .collect::<BTreeSet<_>>();
        for (id, version) in &declaration.locked_versions {
            let version = version.parse::<semver::Version>().map_err(|cause| {
                dependency_error()
                    .with_source(cause)
                    .with_package_id(id.clone())
            })?;
            if !candidates.iter().any(|candidate| {
                &candidate.package_id == id
                    && candidate.version == version
                    && crate::unity_version::meets_minimum(unity_version, candidate.unity_minimum)
            }) {
                return Err(dependency_error().with_package_id(id.clone()));
            }
            candidates
                .retain(|candidate| &candidate.package_id != id || candidate.version == version);
        }
        let mut requests = declaration
            .dependencies
            .iter()
            .map(|dependency| ResolveRequest {
                package_id: dependency.package_id.clone(),
                range: dependency.version_range.clone(),
                source: None,
                include_prerelease: false,
                unity_version,
            })
            .collect::<Vec<_>>();
        for (id, version) in &declaration.locked_versions {
            if let Some(request) = requests
                .iter_mut()
                .find(|request| request.package_id == *id)
            {
                let actual = version.parse::<semver::Version>().map_err(|cause| {
                    dependency_error()
                        .with_source(cause)
                        .with_package_id(id.clone())
                })?;
                if !VpmRange::parse(&request.range).is_ok_and(|range| range.matches(&actual, true))
                {
                    return Err(dependency_error().with_package_id(id.clone()));
                }
                request.range = format!("={version}");
                request.include_prerelease = true;
            } else {
                requests.push(ResolveRequest {
                    package_id: id.clone(),
                    range: format!("={version}"),
                    source: None,
                    include_prerelease: true,
                    unity_version,
                });
            }
        }
        let mut resolution = crate::engine::resolve_project_packages(
            self.materializer.cache(),
            &self.store,
            &work.owner,
            &stage.root,
            &candidates,
            &requests,
        )
        .await
        .map_err(map_template_resolve_error)?;
        for package in &mut resolution.packages {
            package.direct = direct.contains(package.package_id.as_str());
        }
        resolution.dependency_edges.retain(|edge| !edge.direct);
        let observation = self
            .reader
            .inspect_project_root(stage.root.clone(), stage.identity.clone())
            .await
            .map_err(|cause| internal().with_source(cause))?;
        let snapshot = inspect_package_project(&ProjectRecord {
            project_id: work.project_id,
            observation,
            revision: Revision::INITIAL,
            registered_at_ms: 0,
            last_modified_at_ms: None,
            favorite: false,
        })
        .map_err(|cause| internal().with_source(cause))?;
        let plan = crate::plan::build_template_resolution_plan(&snapshot, &resolution)
            .map_err(|cause| dependency_error().with_source(cause))?;
        Ok(ProjectCreatePackages {
            source_set: plan
                .source_set
                .into_iter()
                .filter(|source| {
                    plan.change_set
                        .mutations
                        .iter()
                        .any(|mutation| mutation.source.as_ref() == Some(source))
                })
                .collect(),
            change_set: plan.change_set,
        })
    }

    async fn materialize(
        &self,
        stage: &Self::Stage,
        packages: ProjectCreatePackages,
        locks: Arc<ResourceLockCoordinator>,
    ) -> Result<(), ProjectCreateError> {
        let final_vpm_manifest_sha256 = packages.change_set.vpm_manifest_sha256;
        let prepared = self
            .materializer
            .prefetch(packages.change_set, packages.source_set, locks)
            .await
            .map_err(|cause| dependency_error().with_source(cause))?;
        self.materializer
            .materialize_template(
                prepared,
                StagingProjectEvidence {
                    root: stage.root.clone(),
                    root_identity: stage.identity.clone(),
                    initial_vpm_manifest_sha256: stage.vpm_digest,
                    initial_upm_manifest_sha256: stage.upm_digest,
                    final_vpm_manifest_sha256,
                },
            )
            .await
            .map_err(|cause| dependency_error().with_source(cause))?;
        Ok(())
    }

    async fn publish(
        &self,
        stage: Self::Stage,
        id: OperationId,
        work: &ProjectCreateWork,
    ) -> Result<ProjectObservation, ProjectCreateError> {
        verify_parent(work)?;
        let target = target_path(work);
        ensure_absent(&target)?;
        verify_owner(&stage.root, id, work, &stage.identity)?;
        self.observe(&stage.root, stage.identity.clone(), work)
            .await?;
        sync_stage(&stage.root)?;
        verify_parent(work)?;
        alcomd_platform::publish_path_no_replace(&stage.root, &target).map_err(|cause| {
            if fs::symlink_metadata(&target).is_ok() {
                target_exists(&target).with_source(cause)
            } else {
                filesystem_error(&target, cause)
            }
        })?;
        alcomd_platform::sync_directory(Path::new(&work.canonical_parent))
            .map_err(|cause| filesystem_error(&target, cause).recover_on_restart())?;
        self.discard(id, work)
            .map_err(ProjectCreateError::recover_on_restart)?;
        self.observe(&target, stage.identity, work)
            .await
            .map_err(ProjectCreateError::recover_on_restart)
    }

    async fn recover_published(
        &self,
        id: OperationId,
        work: &ProjectCreateWork,
    ) -> Result<Option<ProjectObservation>, ProjectCreateError> {
        verify_parent(work)?;
        let target = target_path(work);
        if !exists(&target)? {
            return Ok(None);
        }
        let identity = directory_identity(&target)?;
        verify_owner(&target, id, work, &identity)?;
        alcomd_platform::sync_directory(Path::new(&work.canonical_parent))
            .map_err(|cause| filesystem_error(&target, cause).recover_on_restart())?;
        self.discard(id, work)
            .map_err(ProjectCreateError::recover_on_restart)?;
        self.observe(&target, identity, work).await.map(Some)
    }

    fn discard(&self, id: OperationId, work: &ProjectCreateWork) -> Result<(), ProjectCreateError> {
        verify_parent(work)?;
        let container = stage_container(id, work);
        if exists(&container)? {
            let identity = directory_identity(&container)?;
            verify_marker(&container, &[OWNER_MARKER], id, work, &identity)?;
            fs::remove_dir_all(&container).map_err(|cause| filesystem_error(&container, cause))?;
        }
        Ok(())
    }
}

fn project_version(work: &ProjectCreateWork) -> String {
    format!("m_EditorVersion: {}\n", work.input.unity_version)
}

fn fingerprint(value: &impl Serialize) -> Result<String, ProjectCreateError> {
    let bytes = serde_json::to_vec(value).map_err(|cause| internal().with_source(cause))?;
    let digest = Sha256::digest(bytes);
    let hash = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(format!(r#"{{"sha256":"{hash}"}}"#))
}

fn scaffold_manifest(
    root: &Path,
    name: &str,
    default: &[u8],
) -> Result<[u8; 32], ProjectCreateError> {
    let path = root.join("Packages").join(name);
    if !exists(&path)? {
        write_new(&path, default)?;
        return Ok(Sha256::digest(default).into());
    }
    let bytes = crate::plan::read_regular_component(
        root,
        &["Packages", name],
        crate::PROJECT_MANIFEST_LIMIT,
    )
    .map_err(|cause| destination_changed(&path).with_source(cause))?;
    Ok(Sha256::digest(bytes).into())
}

fn apply_product_name(
    root: &Path,
    name: &str,
    product_guid: &str,
) -> Result<(), ProjectCreateError> {
    let path = root.join("ProjectSettings/ProjectSettings.asset");
    let original = if exists(&path)? {
        Some(
            crate::plan::read_regular_component(
                root,
                &["ProjectSettings", "ProjectSettings.asset"],
                crate::PROJECT_MANIFEST_LIMIT,
            )
            .map_err(|cause| {
                invalid()
                    .with_source(cause)
                    .with_path(path.display().to_string())
            })?,
        )
    } else {
        None
    };
    let updated = crate::template_archive::player_settings(original.as_deref(), name, product_guid)
        .map_err(|cause| cause.with_path(path.display().to_string()))?;
    if original.is_some() {
        fs::remove_file(&path).map_err(|cause| filesystem_error(&path, cause))?;
    }
    write_new(&path, &updated)
}

fn target_path(work: &ProjectCreateWork) -> PathBuf {
    Path::new(&work.canonical_parent).join(&work.input.name)
}

fn stage_container(id: OperationId, work: &ProjectCreateWork) -> PathBuf {
    Path::new(&work.canonical_parent).join(format!(".alcomd-project-create-{id}"))
}

fn verify_parent(work: &ProjectCreateWork) -> Result<(), ProjectCreateError> {
    validate_leaf(&work.input.name)?;
    let path = Path::new(&work.canonical_parent);
    let (resolved, identity) = alcomd_platform::resolve_directory_identity(path)
        .map_err(|cause| destination_changed(path).with_source(cause))?;
    if resolved != path || identity != work.parent_identity {
        return Err(destination_changed(path));
    }
    Ok(())
}

fn directory_identity(path: &Path) -> Result<Vec<u8>, ProjectCreateError> {
    let (canonical, identity) = alcomd_platform::resolve_directory_identity(path)
        .map_err(|cause| destination_changed(path).with_source(cause))?;
    if canonical != path {
        return Err(destination_changed(path));
    }
    Ok(identity)
}

fn owner_marker(id: OperationId, work: &ProjectCreateWork, identity: &[u8]) -> OwnerMarker {
    OwnerMarker {
        operation_id: id.to_string(),
        project_id: work.project_id.to_string(),
        root_identity: identity.to_vec(),
    }
}

fn verify_owner(
    path: &Path,
    id: OperationId,
    work: &ProjectCreateWork,
    identity: &[u8],
) -> Result<(), ProjectCreateError> {
    verify_marker(
        path,
        &["Library", "ALCOMD", OWNER_MARKER],
        id,
        work,
        identity,
    )
}

fn verify_marker(
    path: &Path,
    components: &[&str],
    id: OperationId,
    work: &ProjectCreateWork,
    identity: &[u8],
) -> Result<(), ProjectCreateError> {
    let bytes = crate::plan::read_regular_component(path, components, 4096)
        .map_err(|cause| destination_changed(path).with_source(cause))?;
    let actual: OwnerMarker = serde_json::from_slice(&bytes)
        .map_err(|cause| destination_changed(path).with_source(cause))?;
    if actual != owner_marker(id, work, identity) {
        return Err(destination_changed(path));
    }
    Ok(())
}

fn sync_stage(root: &Path) -> Result<(), ProjectCreateError> {
    for directory in [
        "Packages",
        "ProjectSettings",
        "Library/ALCOMD",
        "Library",
        "Assets",
        "",
    ] {
        let path = root.join(directory);
        alcomd_platform::sync_directory(&path).map_err(|cause| filesystem_error(&path, cause))?;
    }
    Ok(())
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), ProjectCreateError> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|cause| filesystem_error(path, cause))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|cause| filesystem_error(path, cause))
}

fn validate_leaf(value: &str) -> Result<(), ProjectCreateError> {
    let stem = value
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end()
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|suffix| {
                matches!(
                    suffix,
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                )
            });
    if value.is_empty()
        || value.len() > 255
        || value.nfc().ne(value.chars())
        || value.ends_with([' ', '.'])
        || value.starts_with(".alcomd-project-create-")
        || value.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*'
                )
        })
        || reserved
    {
        return Err(invalid());
    }
    Ok(())
}

fn exists(path: &Path) -> Result<bool, ProjectCreateError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(cause) => Err(filesystem_error(path, cause)),
    }
}

fn ensure_absent(path: &Path) -> Result<(), ProjectCreateError> {
    if exists(path)? {
        return Err(target_exists(path));
    }
    Ok(())
}

fn target_exists(path: &Path) -> ProjectCreateError {
    ProjectCreateError::new(ProjectCreateErrorCode::TargetExists)
        .with_path(path.display().to_string())
}

fn destination_changed(path: &Path) -> ProjectCreateError {
    ProjectCreateError::new(ProjectCreateErrorCode::DestinationChanged)
        .with_path(path.display().to_string())
}

fn filesystem_error(path: &Path, cause: io::Error) -> ProjectCreateError {
    internal()
        .with_source(cause)
        .with_path(path.display().to_string())
}

fn invalid() -> ProjectCreateError {
    ProjectCreateError::new(ProjectCreateErrorCode::InvalidInput)
}

fn internal() -> ProjectCreateError {
    ProjectCreateError::new(ProjectCreateErrorCode::Internal)
}

fn dependency_error() -> ProjectCreateError {
    ProjectCreateError::new(ProjectCreateErrorCode::DependencyUnsatisfied)
}

fn map_template_resolve_error(cause: alcomd_application::M4Error) -> ProjectCreateError {
    use crate::ResolveError::*;
    let package = std::error::Error::source(&cause).and_then(|error| {
        match error.downcast_ref::<crate::ResolveError>()? {
            PackageNotFound { package_id }
            | DependencyMissing { package_id }
            | DependencyConflict { package_id }
            | UnityIncompatible { package_id }
            | VersionYanked { package_id }
            | SourceAmbiguous { package_id, .. }
            | LegacyCleanupRequired { package_id } => Some(package_id.clone()),
            _ => None,
        }
    });
    let error = dependency_error().with_source(cause);
    if let Some(package) = package {
        error.with_package_id(package)
    } else {
        error
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use alcomd_application::{
        DependencyIdentity, M4Error, ManifestState, PrincipalId, ProjectId, ProjectTemplateRef,
        ProjectType, RepositoryPackageSourcePin, ResolverCatalogEntry, ResolverCatalogSource,
    };

    use super::*;

    #[derive(Clone)]
    struct FixturePinStore;

    impl RepositoryArchivePinStore for FixturePinStore {
        async fn pin_repository_archive(
            &self,
            _owner: PrincipalId,
            _pin: RepositoryPackageSourcePin,
        ) -> Result<(), M4Error> {
            panic!("fixture archives already have a declared digest");
        }
    }

    struct Fixture {
        root: PathBuf,
        engine: ProjectCreateEngine<FixturePinStore>,
    }

    impl Fixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("alcomd-create-test-{}", OperationId::new()));
            fs::create_dir(&root).expect("isolated fixture");
            let root = fs::canonicalize(root).expect("canonical fixture");
            let engine = ProjectCreateEngine::new(
                FixturePinStore,
                VpmReader::new().expect("reader"),
                root.join("cache"),
            )
            .expect("create engine");
            Self { root, engine }
        }

        fn work(
            &self,
            name: &str,
            unity_version: &str,
            declaration: Option<ProjectTemplateDeclaration>,
        ) -> ProjectCreateWork {
            let owner = PrincipalId::local_owner();
            let input = ProjectCreateInput {
                name: name.to_owned(),
                target_parent: self.root.to_str().expect("fixture path").to_owned(),
                unity_version: unity_version.to_owned(),
                template: declaration.as_ref().map(|value| ProjectTemplateRef {
                    id: value.id.clone(),
                    revision: 1,
                }),
            };
            let (canonical_parent, parent_identity) = self
                .engine
                .prepare_destination(&input)
                .expect("accept destination");
            ProjectCreateWork {
                project_id: ProjectId::new(),
                owner,
                input,
                parent_identity,
                canonical_parent,
                declaration,
                packages: None,
                publishing: false,
                cancel_requested: false,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn builtins_export_and_import_with_the_same_archive_format() {
        let fixture = Fixture::new();
        for declaration in fixture.engine.builtin_templates().expect("builtins") {
            let path = fixture
                .root
                .join(format!("{}.alcomdtemplate", declaration.id));
            let path = path.to_str().expect("fixture path");
            let operation = OperationId::new();
            fixture
                .engine
                .export_template(&declaration, path, operation)
                .expect("export");
            fixture
                .engine
                .export_template(&declaration, path, operation)
                .expect("recover published export");
            let collision = fixture
                .engine
                .export_template(&declaration, path, OperationId::new())
                .expect_err("unique path");
            assert_eq!(collision.code(), ProjectCreateErrorCode::TargetExists);
            fixture
                .engine
                .finish_template_export(path, operation)
                .expect("finish export");
            let imported = fixture
                .engine
                .prepare_template(
                    ProjectTemplateSource::File {
                        source_file: path.to_owned(),
                    },
                    None,
                )
                .expect("import");
            assert_eq!(imported, declaration);
        }
    }

    #[test]
    fn imported_template_requires_an_editor_version() {
        let fixture = Fixture::new();
        let path = fixture.root.join("no-version.alcomdtemplate");
        let mut archive = zip::ZipWriter::new(fs::File::create(&path).expect("fixture file"));
        archive
            .start_file("package.json", zip::write::SimpleFileOptions::default())
            .expect("metadata entry");
        archive
            .write_all(br#"{"name":"com.example.no-version","displayName":"No version"}"#)
            .expect("metadata");
        archive.finish().expect("fixture archive");

        let error = fixture
            .engine
            .prepare_template(
                ProjectTemplateSource::File {
                    source_file: path.to_str().expect("fixture path").to_owned(),
                },
                None,
            )
            .expect_err("a template without an editor version cannot be imported");
        assert_eq!(error.code(), ProjectCreateErrorCode::InvalidInput);
        assert_eq!(
            std::error::Error::source(&error)
                .expect("specific reason")
                .to_string(),
            "template requires ProjectSettings/ProjectVersion.txt"
        );
        assert_eq!(
            fs::read_dir(fixture.root.join("templates"))
                .unwrap()
                .count(),
            0
        );
    }

    #[test]
    fn capture_uses_scoped_roots_and_keeps_only_selected_package_policy() {
        let fixture = Fixture::new();
        let source = fixture.root.join("source");
        fs::create_dir(&source).expect("project root");
        for relative in [
            "ProjectSettings",
            "Packages",
            "Packages/com.example.local",
            "Packages/com.example.direct",
            "Packages/com.example.transitive",
            "Library",
        ] {
            fs::create_dir(source.join(relative)).expect("project directories");
        }
        for (relative, data) in [
            ("Assets", "ordinary file that would fail any traversal"),
            ("Library/cache", "must remain outside archive"),
            (
                "ProjectSettings/ProjectVersion.txt",
                "m_EditorVersion: 2022.3.22f1\n",
            ),
            ("ProjectSettings/TagManager.asset", "custom tags"),
            (
                "Packages/manifest.json",
                r#"{"dependencies":{"com.example.local":"file:com.example.local"}}"#,
            ),
            (
                "Packages/vpm-manifest.json",
                r#"{"dependencies":{"com.example.direct":{"version":"^1.0.0"}},"locked":{"com.example.direct":{"version":"1.2.0","dependencies":{"com.example.transitive":"^2.0.0"}},"com.example.transitive":{"version":"2.1.0"}}}"#,
            ),
            ("Packages/com.example.direct/cache", "managed package"),
            (
                "Packages/com.example.transitive/cache",
                "managed transitive package",
            ),
            (
                "Packages/com.example.local/package.json",
                r#"{"name":"com.example.local","version":"1.0.0"}"#,
            ),
        ] {
            fs::write(source.join(relative), data).expect("fixture file");
        }
        let (source, path_identity_key) =
            alcomd_platform::resolve_directory_identity(&source).expect("canonical source");
        let record = ProjectRecord {
            project_id: ProjectId::new(),
            observation: ProjectObservation {
                root_path: source.to_str().expect("source path").to_owned(),
                path_identity_key,
                project_type: ProjectType::VpmStarter,
                unity_version: "2022.3.22f1".to_owned(),
                unity_revision: None,
                vpm_manifest: ManifestState::Valid,
                upm_manifest: ManifestState::Valid,
                direct_dependencies: vec![DependencyIdentity {
                    package_id: "com.example.direct".to_owned(),
                    value: "^1.0.0".to_owned(),
                }],
                locked_dependencies: vec![
                    DependencyIdentity {
                        package_id: "com.example.direct".to_owned(),
                        value: "1.2.0".to_owned(),
                    },
                    DependencyIdentity {
                        package_id: "com.example.transitive".to_owned(),
                        value: "2.1.0".to_owned(),
                    },
                ],
                issues: vec![],
            },
            revision: Revision::INITIAL,
            registered_at_ms: 0,
            last_modified_at_ms: None,
            favorite: false,
        };
        let prepare = |include_assets, lock_package_versions| {
            fixture.engine.prepare_template(
                ProjectTemplateSource::Project {
                    project_id: record.project_id,
                    expected_project_revision: record.revision.get(),
                    id: "com.example.captured".to_owned(),
                    name: "Captured project".to_owned(),
                    include_assets,
                    lock_package_versions,
                },
                Some(record.clone()),
            )
        };
        let flexible = prepare(false, false).expect("unread Assets is harmless");
        assert_eq!(flexible.dependencies[0].version_range, "*");
        assert!(flexible.locked_versions.is_empty());
        let destination = fixture.root.join("flexible");
        fs::create_dir(&destination).expect("destination");
        fixture
            .engine
            .templates
            .expand(&flexible, &destination)
            .expect("expand");
        assert!(
            fs::read_dir(destination.join("Assets"))
                .expect("empty Assets directory")
                .next()
                .is_none()
        );
        assert!(!destination.join("Library").exists());
        assert!(
            destination
                .join("ProjectSettings/TagManager.asset")
                .is_file()
        );
        assert!(
            destination
                .join("Packages/com.example.local/package.json")
                .is_file()
        );
        assert!(!destination.join("Packages/com.example.direct").exists());
        assert!(!destination.join("Packages/com.example.transitive").exists());

        assert!(prepare(true, true).is_err());
        fs::remove_file(source.join("Assets")).expect("replace invalid fixture");
        fs::create_dir(source.join("Assets")).expect("asset root");
        fs::write(source.join("Assets/Scene.unity"), "scene").expect("asset");
        fs::write(source.join("Assets/Scene.unity.meta"), "guid: stable").expect("metadata");
        let locked = prepare(true, true).expect("select assets and exact packages");
        assert_eq!(
            locked
                .locked_versions
                .get("com.example.direct")
                .map(String::as_str),
            Some("1.2.0")
        );
        assert_eq!(
            locked
                .locked_versions
                .get("com.example.transitive")
                .map(String::as_str),
            Some("2.1.0")
        );
        let destination = fixture.root.join("locked");
        fs::create_dir(&destination).expect("destination");
        fixture
            .engine
            .templates
            .expand(&locked, &destination)
            .expect("expand");
        assert_eq!(
            fs::read(destination.join("Assets/Scene.unity.meta")).expect("asset metadata"),
            b"guid: stable"
        );
    }

    #[test]
    fn import_rejects_implicit_case_collisions_and_outside_roots() {
        let fixture = Fixture::new();
        for paths in [
            ["Assets/Case/first.asset", "Assets/case/second.asset"],
            ["Assets/valid.asset", "Library/secret"],
        ] {
            let input = fixture.root.join("invalid.alcomdtemplate");
            let mut archive = zip::ZipWriter::new(fs::File::create(&input).expect("input"));
            archive
                .start_file("package.json", zip::write::SimpleFileOptions::default())
                .expect("manifest");
            archive
                .write_all(br#"{"name":"com.example.test","displayName":"test"}"#)
                .expect("manifest data");
            for path in paths {
                archive
                    .start_file(path, zip::write::SimpleFileOptions::default())
                    .expect("fixture entry");
                archive.write_all(b"fixture").expect("entry data");
            }
            archive.finish().expect("close ZIP");
            let result = fixture.engine.prepare_template(
                ProjectTemplateSource::File {
                    source_file: input.to_str().expect("input path").to_owned(),
                },
                None,
            );
            assert!(result.is_err());
        }
    }

    #[tokio::test]
    async fn project_create_keeps_unpublished_scaffold_out_of_discovery_and_recovers_own_target() {
        let fixture = Fixture::new();
        let engine = &fixture.engine;
        let id = OperationId::new();
        let work = fixture.work("Blank Project", "2022.3.22f1", None);
        let target = target_path(&work);
        let container = stage_container(id, &work);
        let stage = engine.stage(id, &work).expect("stage blank");
        assert!(
            !container
                .join("ProjectSettings/ProjectVersion.txt")
                .exists()
        );
        assert!(
            stage
                .root
                .join("ProjectSettings/ProjectVersion.txt")
                .is_file()
        );
        assert!(!target.exists());
        fs::create_dir(&target).expect("external target");
        fs::write(target.join("owner.txt"), b"external").expect("external sentinel");
        let error = engine
            .publish(stage, id, &work)
            .await
            .expect_err("no replacement");
        assert_eq!(error.code(), ProjectCreateErrorCode::TargetExists);
        assert_eq!(
            fs::read(target.join("owner.txt")).expect("external sentinel"),
            b"external"
        );
        assert!(engine.recover_published(id, &work).await.is_err());
        fs::remove_dir_all(&target).expect("remove isolated collision");
        let stage = engine.stage(id, &work).expect("retry stage");
        let published = engine.publish(stage, id, &work).await.expect("publish");
        assert!(!container.exists());
        assert_eq!(published.unity_version, "2022.3.22f1");
        let recovered = engine
            .recover_published(id, &work)
            .await
            .expect("read receipt")
            .expect("recover published");
        assert_eq!(recovered.path_identity_key, published.path_identity_key);
        assert_eq!(recovered.root_path, published.root_path);
        assert_eq!(
            fs::read(target.join("ProjectSettings/ProjectVersion.txt")).expect("version"),
            b"m_EditorVersion: 2022.3.22f1\n"
        );
    }

    #[tokio::test]
    async fn each_builtin_year_materializes_its_fixed_unity_version() {
        let fixture = Fixture::new();
        let engine = &fixture.engine;
        let catalog = sdk_fixture(engine);
        let builtins = engine.builtin_templates().expect("builtins");
        assert_eq!(builtins.len(), 4);
        let mut ids = BTreeSet::new();
        let mut v3 = zip::ZipArchive::new(std::io::Cursor::new(include_bytes!(
            "../../../specs/templates/builtin-scaffolds/v3-projects.zip"
        )))
        .expect("frozen v3 project data");
        for (index, declaration) in builtins.into_iter().enumerate() {
            let direct = &declaration.dependencies[0].package_id;
            assert_eq!(declaration.dependencies[0].version_range, "3.x");
            let version = declaration
                .editor_version
                .as_deref()
                .expect("versioned builtin");
            let (year, expected) = match version {
                "2022.3.22f1" => ("2022", "3.10.0"),
                "2019.4.31f1" => ("2019", "3.4.2"),
                other => panic!("unexpected Unity version {other}"),
            };
            let family = direct.strip_prefix("com.vrchat.").expect("VRChat package");
            assert_eq!(
                declaration.id,
                format!("com.vrclearn.alcomd.templates.vrchat.{family}.{year}")
            );
            assert!(declaration.name.ends_with(&format!(" {year}")));
            assert!(ids.insert(declaration.id.clone()));
            let id = OperationId::new();
            let mut work = fixture.work(
                &format!("Project {index} {version}"),
                version,
                Some(declaration.clone()),
            );
            let stage = engine.stage(id, &work).expect("stage");
            let packages = engine
                .resolve(&stage, &work, catalog.clone())
                .await
                .expect("resolve");
            work.packages = Some(packages.clone());
            engine
                .materialize(
                    &stage,
                    packages,
                    Arc::new(ResourceLockCoordinator::default()),
                )
                .await
                .expect("materialize frozen packages");
            let observation = engine.publish(stage, id, &work).await.expect("publish");
            assert_eq!(observation.unity_version, version);
            assert_eq!(observation.direct_dependencies.len(), 1);
            assert_eq!(&observation.direct_dependencies[0].package_id, direct);
            assert_eq!(observation.locked_dependencies.len(), 2);
            assert!(
                observation
                    .locked_dependencies
                    .iter()
                    .all(|value| value.value == expected)
            );
            let project = Path::new(&observation.root_path);
            let prefix = format!("{family}-{version}/");
            let mut compared = 0;
            for index in 0..v3.len() {
                let mut entry = v3.by_index(index).expect("v3 file");
                let entry_name = entry.name().expect("v3 file name");
                let Some(relative) = entry_name.strip_prefix(&prefix).map(str::to_owned) else {
                    continue;
                };
                let actual =
                    fs::read(project.join(&relative)).expect("v3 file in generated project");
                if relative == "Packages/vpm-manifest.json" {
                    continue;
                }
                let mut expected = Vec::new();
                std::io::Read::read_to_end(&mut entry, &mut expected).expect("v3 file bytes");
                if relative == "ProjectSettings/ProjectSettings.asset" {
                    expected = String::from_utf8(expected)
                        .expect("v3 settings UTF-8")
                        .replace(
                            &format!("  productName: vrc-get-template-{family}-{year}"),
                            &format!("  productName: \"{}\"", work.input.name),
                        )
                        .replace(
                            "  productGUID: 00000000000000000000000000000000",
                            &format!(
                                "  productGUID: {}",
                                work.project_id.to_string().replace('-', "")
                            ),
                        )
                        .into_bytes();
                }
                assert_eq!(actual, expected, "{relative}");
                compared += 1;
            }
            assert_eq!(compared, if year == "2019" { 22 } else { 26 });
            assert_eq!(
                fs::read(project.join("LICENSE")).expect("template license"),
                include_bytes!("../../../specs/templates/builtin-scaffolds/LICENSE")
            );
            assert!(
                project
                    .join("Packages/com.vrchat.base/package.json")
                    .is_file()
            );
            assert!(
                project
                    .join("Packages")
                    .join(direct)
                    .join("package.json")
                    .is_file()
            );
        }
    }

    fn sdk_fixture(engine: &ProjectCreateEngine<FixturePinStore>) -> ResolverCatalog {
        let mut entries = Vec::new();
        for package_id in ["com.vrchat.base", "com.vrchat.avatars", "com.vrchat.worlds"] {
            let dependencies = if package_id == "com.vrchat.base" {
                serde_json::json!({})
            } else {
                serde_json::json!({ "com.vrchat.base": "*" })
            };
            for (version, unity, release) in [
                ("3.4.2", None, None),
                ("3.5.0", None, None),
                ("3.9.0", Some("2022.3"), Some("6f1")),
                ("3.10.0", Some("2022.3"), Some("22f1")),
            ] {
                let mut manifest = serde_json::json!({
                    "name": package_id, "version": version, "vpmDependencies": dependencies,
                });
                if let Some(unity) = unity {
                    manifest["unity"] = unity.into();
                }
                if let Some(release) = release {
                    manifest["unityRelease"] = release.into();
                }
                let manifest = serde_json::to_vec(&manifest).expect("fixture manifest");
                let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
                archive
                    .start_file("package.json", zip::write::SimpleFileOptions::default())
                    .expect("archive manifest entry");
                archive.write_all(&manifest).expect("write manifest");
                let bytes = archive.finish().expect("finish archive").into_inner();
                let digest: [u8; 32] = Sha256::digest(&bytes).into();
                let path = engine.materializer.cache().object_path(&digest);
                fs::create_dir_all(path.parent().expect("object parent")).expect("cache directory");
                fs::write(path, bytes).expect("cache object");
                entries.push(ResolverCatalogEntry {
                    source: ResolverCatalogSource::Repository {
                        repository_locator: "https://example.invalid/index.json".to_owned(),
                        repository_id: "fixture-sdk".to_owned(),
                        repository_revision: 1,
                        repository_priority: 1,
                        source_identity: "fixture-sdk".to_owned(),
                        credential_reference: None,
                        download_headers_present: false,
                        artifact_url: "https://example.invalid/package.zip".to_owned(),
                        zip_sha256: Some(digest.iter().map(|byte| format!("{byte:02x}")).collect()),
                        local_zip_sha256: None,
                        manifest_fingerprint: Sha256::digest(&manifest).into(),
                    },
                    package_id: package_id.to_owned(),
                    version: version.to_owned(),
                    yanked: false,
                    unity: unity.map(str::to_owned),
                    unity_release: release.map(str::to_owned),
                    author_name: String::new(),
                    author_email: None,
                    dependencies_json: dependencies.to_string(),
                    legacy_metadata_present: false,
                });
            }
        }
        ResolverCatalog {
            entries,
            complete: true,
        }
    }
}
