use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use alcomd_application::{
    OperationId, PlanAction, ProjectCreateAdapter, ProjectCreateError, ProjectCreateErrorCode,
    ProjectCreateInput, ProjectCreatePackages, ProjectCreateWork, ProjectObservation,
    ProjectRecord, ProjectTemplateDeclaration, RepositoryArchivePinStore, ResolverCatalog,
    ResourceKey, ResourceLockCoordinator, Revision, canonical_unity_version,
    valid_project_template_id,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;

use crate::range::VpmRange;
use crate::{
    FrozenPackageMaterializer, ResolveRequest, StagingProjectEvidence, UnityVersion, VpmReader,
    build_resolution_plan, candidates_from_catalog, inspect_package_project,
    project_creation_resource,
};

const UPM_MANIFEST: &[u8] = b"{\"dependencies\":{}}\n";
const VPM_MANIFEST: &[u8] = b"{\"dependencies\":{},\"locked\":{}}\n";
const MAX_DECLARATION_BYTES: usize = 24 * 1024;
const OWNER_MARKER: &str = "project-create.json";

#[derive(Clone)]
pub struct ProjectCreateEngine<S> {
    store: S,
    reader: VpmReader,
    materializer: FrozenPackageMaterializer,
}

impl<S> ProjectCreateEngine<S> {
    pub fn new(
        store: S,
        reader: VpmReader,
        cache_root: PathBuf,
    ) -> Result<Self, ProjectCreateError> {
        let cache = reader
            .package_cache(cache_root)
            .map_err(|cause| internal().with_source(cause))?;
        Ok(Self {
            store,
            reader,
            materializer: FrozenPackageMaterializer::new(cache),
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
        let expected_vpm = work.packages.as_ref().map_or_else(
            || Sha256::digest(VPM_MANIFEST).into(),
            |packages| packages.change_set.vpm_manifest_sha256,
        );
        let files = [
            (
                &["Packages", "manifest.json"][..],
                Sha256::digest(UPM_MANIFEST).into(),
            ),
            (&["Packages", "vpm-manifest.json"][..], expected_vpm),
            (
                &["ProjectSettings", "ProjectVersion.txt"][..],
                Sha256::digest(project_version(work)).into(),
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
            if actual != expected {
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

    fn validate_declaration(
        &self,
        mut declaration: ProjectTemplateDeclaration,
    ) -> Result<ProjectTemplateDeclaration, ProjectCreateError> {
        declaration.name = declaration.name.trim().to_owned();
        if !valid_project_template_id(&declaration.id)
            || declaration.name.is_empty()
            || declaration.name.len() > 128
            || declaration.name.chars().any(char::is_control)
            || declaration.unity.series.len() > 64
            || declaration.unity.presets.len() > 64
            || declaration.dependencies.len() > 128
            || declaration.constraints.len() > 256
        {
            return Err(invalid());
        }
        let mut series = BTreeSet::new();
        for value in &declaration.unity.series {
            let valid = value.len() <= 64
                && UnityVersion::minimum(value, None).is_some()
                && series.insert(value);
            if !valid {
                return Err(invalid());
            }
        }
        declaration.unity.series.sort();
        let mut presets = BTreeSet::new();
        for value in &declaration.unity.presets {
            if canonical_unity_version(value).is_err() || !presets.insert(value) {
                return Err(invalid());
            }
        }
        declaration
            .dependencies
            .sort_by(|a, b| a.package_id.cmp(&b.package_id));
        let mut packages = BTreeSet::new();
        for entry in &mut declaration.dependencies {
            validate_package_id(&entry.package_id)?;
            if !packages.insert(&entry.package_id) {
                return Err(invalid());
            }
            entry.version_range = canonical_range(&entry.version_range)?;
        }
        declaration.constraints.sort_by(|a, b| {
            (&a.unity_version, &a.package_id).cmp(&(&b.unity_version, &b.package_id))
        });
        let mut constraints = BTreeSet::new();
        for entry in &mut declaration.constraints {
            validate_package_id(&entry.package_id)?;
            if canonical_unity_version(&entry.unity_version).is_err()
                || !declaration.unity.allows(&entry.unity_version)
                || !constraints.insert((&entry.unity_version, &entry.package_id))
            {
                return Err(invalid());
            }
            entry.version_range = canonical_range(&entry.version_range)?;
        }
        let bytes =
            serde_json::to_vec(&declaration).map_err(|cause| internal().with_source(cause))?;
        if bytes.len() > MAX_DECLARATION_BYTES {
            return Err(invalid());
        }
        Ok(declaration)
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
        let bytes = serde_json::to_vec(input).map_err(|cause| internal().with_source(cause))?;
        let digest = Sha256::digest(bytes);
        let hash = digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        Ok(format!(r#"{{"sha256":"{hash}"}}"#))
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
        for relative in ["Library", "Library/ALCOMD"] {
            let path = root.join(relative);
            fs::create_dir(&path).map_err(|cause| filesystem_error(&path, cause))?;
        }
        let marker = serde_json::to_vec(&owner_marker(id, work, &identity))
            .map_err(|cause| internal().with_source(cause))?;
        write_new(&root.join("Library/ALCOMD").join(OWNER_MARKER), &marker)?;
        for relative in ["Assets", "Packages", "ProjectSettings"] {
            let path = root.join(relative);
            fs::create_dir(&path).map_err(|cause| filesystem_error(&path, cause))?;
        }
        write_new(&root.join("Packages/manifest.json"), UPM_MANIFEST)?;
        write_new(&root.join("Packages/vpm-manifest.json"), VPM_MANIFEST)?;
        write_new(
            &root.join("ProjectSettings/ProjectVersion.txt"),
            project_version(work).as_bytes(),
        )?;
        sync_stage(&root)?;
        alcomd_platform::sync_directory(&container)
            .map_err(|cause| filesystem_error(&container, cause))?;
        Ok(ProjectCreateStage { root, identity })
    }

    async fn resolve(
        &self,
        stage: &Self::Stage,
        work: &ProjectCreateWork,
        catalog: ResolverCatalog,
    ) -> Result<ProjectCreatePackages, ProjectCreateError> {
        let declaration = work.declaration.as_ref().ok_or_else(internal)?;
        if !catalog.complete || !declaration.unity.allows(&work.input.unity_version) {
            return Err(dependency_error());
        }
        let constraints = declaration
            .constraints
            .iter()
            .filter(|entry| entry.unity_version == work.input.unity_version)
            .map(|entry| {
                let range = VpmRange::parse(&entry.version_range)
                    .map_err(|cause| dependency_error().with_source(cause))?;
                Ok((entry.package_id.as_str(), range))
            })
            .collect::<Result<BTreeMap<_, _>, ProjectCreateError>>()?;
        let mut candidates = candidates_from_catalog(&catalog)
            .map_err(|cause| dependency_error().with_source(cause))?;
        candidates.retain(|candidate| {
            constraints
                .get(candidate.package_id.as_str())
                .is_none_or(|range| range.matches(&candidate.version, false))
        });
        let unity_version = UnityVersion::parse_editor(&work.input.unity_version);
        let requests = declaration
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
        let resolution = crate::engine::resolve_project_packages(
            self.materializer.cache(),
            &self.store,
            &work.owner,
            &stage.root,
            &candidates,
            &requests,
        )
        .await
        .map_err(|cause| dependency_error().with_source(cause))?;
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
        let plan = build_resolution_plan(&snapshot, PlanAction::Install, &resolution)
            .map_err(|cause| dependency_error().with_source(cause))?;
        Ok(ProjectCreatePackages {
            change_set: plan.change_set,
            source_set: plan.source_set,
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
            .materialize(
                prepared,
                StagingProjectEvidence {
                    root: stage.root.clone(),
                    root_identity: stage.identity.clone(),
                    initial_vpm_manifest_sha256: Sha256::digest(VPM_MANIFEST).into(),
                    initial_upm_manifest_sha256: Sha256::digest(UPM_MANIFEST).into(),
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

fn validate_package_id(value: &str) -> Result<(), ProjectCreateError> {
    if value.len() > 128
        || !value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(invalid());
    }
    Ok(())
}

fn canonical_range(value: &str) -> Result<String, ProjectCreateError> {
    VpmRange::parse(value)
        .map(|range| range.canonical())
        .map_err(|cause| invalid().with_source(cause))
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use alcomd_application::{
        M4Error, PrincipalId, ProjectId, ProjectTemplateRef, RepositoryPackageSourcePin,
        ResolverCatalogEntry, ResolverCatalogSource,
    };

    use super::*;

    const BUILTINS: [&str; 2] = [
        include_str!("../../../specs/templates/vrchat-avatars.json"),
        include_str!("../../../specs/templates/vrchat-worlds.json"),
    ];

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
    fn both_builtin_files_use_the_normal_declaration_serializer() {
        let fixture = Fixture::new();
        for bytes in BUILTINS {
            let input = serde_json::from_str(bytes).expect("ordinary authoring input");
            let declaration = fixture
                .engine
                .validate_declaration(input)
                .expect("validate");
            assert_eq!(
                serde_json::to_string(&declaration).expect("canonical"),
                bytes
            );
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
    async fn both_templates_materialize_three_unity_versions_using_declaration_constraints() {
        let fixture = Fixture::new();
        let engine = &fixture.engine;
        let catalog = sdk_fixture(engine);
        for (index, bytes) in BUILTINS.into_iter().enumerate() {
            let declaration = engine
                .validate_declaration(serde_json::from_str(bytes).expect("declaration"))
                .expect("canonical declaration");
            let direct = &declaration.dependencies[0].package_id;
            for (version, expected) in [
                ("2022.3.22f1", "3.10.0"),
                ("2022.3.6f1", "3.9.0"),
                ("2019.4.31f1", "3.4.2"),
            ] {
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
