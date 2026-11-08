//! Approved manual root compatibility overrides remain explicit and frozen through Apply.
use alcomd_application::*;
use alcomd_store::StateStoreHandle;
use alcomd_vpm::{PackageCache, PackageEngine, VpmReader};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};

const ROOT: &str = "com.example.manual";
const CHILD: &str = "com.example.dependency";
fn key(value: &str) -> IdempotencyKey {
    IdempotencyKey::parse(value).expect("key")
}

struct Fixture {
    directory: PathBuf,
    project_root: PathBuf,
    repository_path: PathBuf,
    store: StateStoreHandle,
    reader: VpmReader,
    project: ProjectRecord,
    repository: RepositoryRecord,
    packages: serde_json::Value,
}

impl Fixture {
    async fn new(with_dependency: bool) -> Self {
        let directory =
            std::env::temp_dir().join(format!("alcomd-manual-unity-{}", uuid::Uuid::new_v4()));
        let project_root = directory.join("Project");
        std::fs::create_dir_all(project_root.join("ProjectSettings")).expect("settings directory");
        std::fs::create_dir_all(project_root.join("Packages")).expect("packages directory");
        std::fs::write(
            project_root.join("ProjectSettings/ProjectVersion.txt"),
            "m_EditorVersion: 2022.3.22f1\n",
        )
        .expect("Unity");
        std::fs::write(
            project_root.join("Packages/manifest.json"),
            r#"{"dependencies":{}}"#,
        )
        .expect("UPM");
        std::fs::write(
            project_root.join("Packages/vpm-manifest.json"),
            r#"{"dependencies":{},"locked":{},"preserved":true}"#,
        )
        .expect("VPM");
        let reader = VpmReader::new().expect("reader");
        let store = StateStoreHandle::open(directory.join("state.db")).expect("store");
        let observation = reader
            .inspect_project(
                project_root.to_string_lossy().into_owned(),
                ProjectDiscoveryMode::ExactRoot,
            )
            .await
            .expect("project");
        let project = store
            .register_project(PrincipalId::local_owner(), observation, key("project"), 1)
            .await
            .expect("register project")
            .value;
        let cache = PackageCache::new(directory.join("cache")).expect("cache");
        let mut packages = serde_json::Map::new();
        for name in if with_dependency {
            vec![ROOT, CHILD]
        } else {
            vec![ROOT]
        } {
            let mut manifest = serde_json::json!({"name":name,"displayName":name,"version":"1.0.0","unity":"2023.1","author":{"name":"Fixture","email":"fixture@example.invalid"}});
            if name == ROOT && with_dependency {
                manifest["vpmDependencies"] = serde_json::json!({CHILD:"1.0.0"});
            }
            let digest = cache_archive(&directory, &cache, name, &manifest);
            manifest["url"] =
                serde_json::json!("https://network-must-not-be-used.invalid/package.zip");
            manifest["zipSHA256"] = serde_json::json!(
                digest
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            );
            packages.insert(
                name.into(),
                serde_json::json!({"versions":{"1.0.0":manifest}}),
            );
        }
        let packages = serde_json::Value::Object(packages);
        let repository_path = directory.join("repository.json");
        write_repository(&repository_path, &packages);
        let RepositoryReadOutcome::Fresh(observation) = reader
            .inspect_repository(
                RepositorySource::Local {
                    path: repository_path.to_string_lossy().into_owned(),
                },
                None,
            )
            .await
            .expect("repository")
        else {
            panic!("fresh")
        };
        assert!(observation.issues.is_empty(), "{observation:?}");
        let repository = store
            .register_repository(
                PrincipalId::local_owner(),
                observation,
                key("repository"),
                1,
            )
            .await
            .expect("register repository")
            .value;
        let repository = M3Application::new(store.clone(), reader.clone())
            .refresh_repository(
                &AccessContext::local_owner(),
                repository.repository_id,
                repository.revision,
                key("index-repository"),
            )
            .await
            .expect("persist resolver catalog")
            .value;
        Self {
            directory,
            project_root,
            repository_path,
            store,
            reader,
            project,
            repository,
            packages,
        }
    }

    fn application(&self) -> M4Application<StateStoreHandle, PackageEngine<StateStoreHandle>> {
        M4Application::new(
            self.store.clone(),
            PackageEngine::new(
                self.store.clone(),
                self.reader.clone(),
                self.directory.join("cache"),
            )
            .expect("engine"),
        )
    }

    fn request(&self, allow: bool) -> PackagePlanRequest {
        PackagePlanRequest {
            allow_unity_incompatible: allow,
            action: PlanAction::Install,
            project_id: self.project.project_id,
            expected_revision: self.project.revision,
            package_id: Some(ROOT.into()),
            version_range: Some("=1.0.0".into()),
            repository_id: None,
            include_prerelease: false,
            plan_version: PackagePlanVersion::V2,
            source_selector: Some(PackageSourceSelector::Repository {
                repository_id: self.repository.repository_id.to_string(),
            }),
            reinstall_selection: None,
            reinstall_sources: vec![],
            bulk_intents: vec![],
        }
    }
}

fn write_repository(path: &Path, packages: &serde_json::Value) {
    std::fs::write(
        path,
        serde_json::json!({"id":"manual-fixture","name":"Manual fixture","packages":packages})
            .to_string(),
    )
    .expect("repository bytes");
}

fn cache_archive(
    directory: &Path,
    cache: &PackageCache,
    name: &str,
    manifest: &serde_json::Value,
) -> [u8; 32] {
    let path = directory.join(format!("{name}.zip"));
    let mut writer = zip::ZipWriter::new(std::fs::File::create(&path).expect("archive"));
    writer
        .start_file("package.json", zip::write::SimpleFileOptions::default())
        .expect("manifest entry");
    writer
        .write_all(manifest.to_string().as_bytes())
        .expect("manifest content");
    writer
        .start_file(
            "Runtime/payload.txt",
            zip::write::SimpleFileOptions::default(),
        )
        .expect("payload entry");
    writer.write_all(name.as_bytes()).expect("payload");
    writer.finish().expect("finish ZIP");
    let digest: [u8; 32] = Sha256::digest(std::fs::read(&path).expect("ZIP bytes")).into();
    let object = cache.object_path(&digest);
    std::fs::create_dir_all(object.parent().expect("parent")).expect("cache directory");
    std::fs::copy(path, object).expect("cache object");
    digest
}

#[tokio::test]
async fn explicit_manual_root_override_is_default_denied_persisted_and_applied_without_resolving_again()
 {
    let fixture = Fixture::new(false).await;
    let app = fixture.application();
    let owner = AccessContext::local_owner();
    let before =
        std::fs::read(fixture.project_root.join("Packages/vpm-manifest.json")).expect("before");
    assert_eq!(
        app.plan(&owner, fixture.request(false))
            .await
            .expect_err("default must reject")
            .code(),
        M4ErrorCode::PackageUnityIncompatible
    );
    let mut invalid = fixture.request(true);
    invalid.version_range = Some("^1.0.0".into());
    assert_eq!(
        app.plan(&owner, invalid)
            .await
            .expect_err("override requires exact version")
            .code(),
        M4ErrorCode::InvalidInput
    );
    let mut invalid = fixture.request(true);
    invalid.action = PlanAction::Resolve;
    assert_eq!(
        app.plan(&owner, invalid)
            .await
            .expect_err("resolve cannot opt in")
            .code(),
        M4ErrorCode::InvalidInput
    );
    let mut invalid = fixture.request(true);
    invalid.plan_version = PackagePlanVersion::V1;
    assert_eq!(
        app.plan(&owner, invalid)
            .await
            .expect_err("old capability cannot opt in")
            .code(),
        M4ErrorCode::InvalidInput
    );
    let plan = app
        .plan(&owner, fixture.request(true))
        .await
        .expect("explicit root override");
    assert_eq!(
        plan.change_set.unity_compatibility_overrides,
        vec![PackageUnityCompatibilityOverride {
            package_id: ROOT.into(),
            version: "1.0.0".into()
        }]
    );
    assert_eq!(
        fixture
            .store
            .get_package_plan(PrincipalId::local_owner(), plan.plan_id)
            .await
            .expect("persisted plan"),
        plan
    );
    let mut changed = plan.change_set.clone();
    changed.unity_compatibility_overrides.clear();
    assert_ne!(
        <[u8; 32]>::from(Sha256::digest(serde_json::to_vec(&changed).expect("JSON"))),
        plan.change_set_fingerprint,
        "override intent is part of the frozen fingerprint"
    );
    assert_eq!(
        std::fs::read(fixture.project_root.join("Packages/vpm-manifest.json")).expect("after Plan"),
        before
    );
    let readonly = AccessContext::new(
        PrincipalId::local_owner(),
        [
            Permission::ProjectsRead,
            Permission::RepositoriesRead,
            Permission::PackagesRead,
        ],
    );
    assert_eq!(
        app.apply_plan(
            &readonly,
            plan.plan_id,
            plan.project_revision,
            key("denied-apply")
        )
        .await
        .expect_err("override grants no Apply permission")
        .code(),
        M4ErrorCode::PermissionDenied
    );
    let accepted = app
        .apply_plan(&owner, plan.plan_id, plan.project_revision, key("apply"))
        .await
        .expect("Apply frozen plan");
    let operation = tokio::time::timeout(std::time::Duration::from_secs(20), async {
        loop {
            let operation = fixture
                .store
                .get_operation(PrincipalId::local_owner(), accepted.operation_id)
                .await
                .expect("operation");
            if operation.state.is_terminal() {
                break operation;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("terminal operation");
    assert_eq!(operation.state, OperationState::Succeeded, "{operation:?}");
    assert_eq!(
        std::fs::read_to_string(
            fixture
                .project_root
                .join(format!("Packages/{ROOT}/Runtime/payload.txt"))
        )
        .expect("installed payload"),
        ROOT
    );
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(fixture.project_root.join("Packages/vpm-manifest.json")).expect("manifest"),
    )
    .expect("JSON");
    assert_eq!(manifest["locked"][ROOT]["version"], "1.0.0");
    assert_eq!(manifest["preserved"], true);
}

#[tokio::test]
async fn a_manual_root_override_does_not_override_an_incompatible_dependency() {
    let fixture = Fixture::new(true).await;
    assert_eq!(
        fixture
            .application()
            .plan(&AccessContext::local_owner(), fixture.request(true))
            .await
            .expect_err("dependency remains incompatible")
            .code(),
        M4ErrorCode::PackageUnityIncompatible
    );
    let connection = rusqlite::Connection::open_with_flags(
        fixture.directory.join("state.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("inspect");
    assert_eq!(
        connection
            .query_row::<i64, _, _>("SELECT COUNT(*) FROM package_plans", [], |row| row.get(0))
            .expect("plans"),
        0
    );
    assert!(
        !fixture
            .project_root
            .join(format!("Packages/{ROOT}"))
            .exists()
    );
    let mut packages = fixture.packages.clone();
    packages[CHILD]["versions"]["1.0.0"]["unity"] = serde_json::json!("2022.3");
    write_repository(&fixture.repository_path, &packages);
    M3Application::new(fixture.store.clone(), fixture.reader.clone())
        .refresh_repository(
            &AccessContext::local_owner(),
            fixture.repository.repository_id,
            fixture.repository.revision,
            key("compatible-dependency"),
        )
        .await
        .expect("refresh dependency metadata");
    let plan = fixture
        .application()
        .plan(&AccessContext::local_owner(), fixture.request(true))
        .await
        .expect("compatible dependency");
    assert_eq!(plan.change_set.unity_compatibility_overrides.len(), 1);
    assert_eq!(
        plan.change_set.unity_compatibility_overrides[0].package_id,
        ROOT
    );
    assert!(
        plan.change_set
            .mutations
            .iter()
            .any(|mutation| mutation.package_id == CHILD)
    );
}
