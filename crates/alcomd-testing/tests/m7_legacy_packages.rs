//! Real reader/store/Plan/Apply tests using owned projects and cached synthetic
//! packages. Legacy fields follow the official VPM path-to-GUID object shape.
//! No IPC endpoint, Editor process, user profile, or real project is used.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use alcomd_application::{
    AccessContext, IdempotencyKey, M3Application, M4Application, M4ErrorCode, M4Store, OperationId,
    OperationLogQuery, OperationRecord, OperationState, PackagePlanRequest, PackagePlanVersion,
    PlanAction, PrincipalId, ProjectRecord, RepositorySource, ResolverCatalogSource, StateStore,
    read_operation_logs, with_log_caller,
};
use alcomd_store::StateStoreHandle;
use alcomd_vpm::{PackageCache, PackageEngine, VpmReader};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _};
use uuid::Uuid;

const KEEP: &str = "com.example.keep";
const NEW: &str = "com.example.new";
const CHILD: &str = "com.example.child";
const FILE_GUID: &str = "11111111111111111111111111111111";
const FOLDER_GUID: &str = "22222222222222222222222222222222";

type Packages = M4Application<StateStoreHandle, PackageEngine<StateStoreHandle>>;

struct Fixture {
    project_root: PathBuf,
    store: StateStoreHandle,
    packages: Packages,
    project: ProjectRecord,
    _directory: TestDirectory,
}

#[derive(Clone, Copy)]
enum ExistingLegacy {
    None,
    File,
    Folder,
    MovedGuid,
    Package,
    MissingPackageDirectory,
}

impl Fixture {
    async fn new(archive_has_legacy: bool, existing: ExistingLegacy) -> Self {
        let directory = TestDirectory::new();
        let project_root = directory.0.join("Project");
        for child in ["Assets", "ProjectSettings", "Packages/com.example.keep"] {
            fs::create_dir_all(project_root.join(child)).expect("owned project directory");
        }
        fs::write(
            project_root.join("ProjectSettings/ProjectVersion.txt"),
            "m_EditorVersion: 2022.3.22f1\n",
        )
        .expect("full project version");
        fs::write(
            project_root.join("Packages/manifest.json"),
            b"{\"dependencies\":{}}",
        )
        .expect("UPM project manifest");
        fs::write(project_root.join("Packages/vpm-manifest.json"), serde_json::to_vec_pretty(&json!({
            "dependencies": {KEEP: {"version": "1.0.0", "note": "retain direct metadata"}},
            "locked": {KEEP: {"version": "1.0.0", "dependencies": {"com.example.existing-child": "^1.0.0"}, "url": "https://example.invalid/original-index.json", "extension": {"preserved": true}},
                "com.example.existing-child": {"version": "1.0.0", "dependencies": {}}}
        })).expect("VPM fixture")).expect("VPM project manifest");
        fs::write(
            project_root.join("Packages/com.example.keep/package.json"),
            br#"{"name":"com.example.keep","version":"1.0.0"}"#,
        )
        .expect("existing package");
        fs::write(
            project_root.join("Packages/com.example.keep/keep.txt"),
            b"existing package bytes",
        )
        .expect("existing package sentinel");
        seed_legacy(&project_root, existing);
        if matches!(
            existing,
            ExistingLegacy::Package | ExistingLegacy::MissingPackageDirectory
        ) {
            let path = project_root.join("Packages/vpm-manifest.json");
            let bytes = fs::read(&path).expect("read initial manifest");
            let mut manifest: Value =
                serde_json::from_slice(&bytes).expect("parse initial manifest");
            manifest["dependencies"]["com.example.legacy"] = json!({"version": "1.0.0"});
            manifest["locked"]["com.example.legacy"] =
                json!({"version": "1.0.0", "dependencies": {}});
            let bytes = serde_json::to_vec(&manifest).expect("serialize legacy package lock");
            fs::write(path, bytes).expect("seed old package in the manifest");
        }

        let cache_root = directory.0.join("package-cache");
        let cache = PackageCache::new(cache_root.clone()).expect("owned package cache");
        let mut new_manifest = json!({"name": NEW, "version": "2.0.0", "vpmDependencies": {CHILD: "^1.0.0"}, "unity": "2022.3", "unityRelease": "22f1"});
        if archive_has_legacy {
            add_legacy(&mut new_manifest);
        }
        let new_digest = cache_package(&cache, &new_manifest);
        let child_manifest = json!({"name": CHILD, "version": "1.0.0"});
        let child_digest = cache_package(&cache, &child_manifest);
        let mut indexed_new = new_manifest;
        add_legacy(&mut indexed_new);
        add_repository_fields(&mut indexed_new, &new_digest);
        let mut indexed_child = child_manifest;
        add_repository_fields(&mut indexed_child, &child_digest);
        let index = directory.0.join("index.json");
        fs::write(&index, serde_json::to_vec(&json!({"id": "isolated-legacy-fixture", "name": "Isolated VPM shape fixture", "packages": {
            NEW: {"versions": {"2.0.0": indexed_new}}, CHILD: {"versions": {"1.0.0": indexed_child}}
        }})).expect("index")).expect("owned local index");
        let store = StateStoreHandle::open(directory.0.join("state.db")).expect("isolated store");
        let reader = VpmReader::new().expect("reader");
        let registry = M3Application::new(store.clone(), reader.clone());
        let access = AccessContext::local_owner();
        let project = registry
            .register_project(
                &access,
                project_root.to_string_lossy().into_owned(),
                key("register-project"),
            )
            .await
            .expect("register owned project")
            .value;
        let repository = registry
            .register_repository(
                &access,
                RepositorySource::Local {
                    path: index.to_string_lossy().into_owned(),
                },
                key("register-repository"),
            )
            .await
            .expect("read and register actual local index")
            .value;
        assert!(
            repository.observation.issues.is_empty(),
            "official-shape legacy metadata is readable"
        );
        registry
            .refresh_repository(
                &access,
                repository.repository_id,
                repository.revision,
                key("refresh-repository"),
            )
            .await
            .expect("persist resolver-ready local index");
        let engine = PackageEngine::new(store.clone(), reader, cache_root).expect("engine");
        let packages = M4Application::new(store.clone(), engine);
        Self {
            _directory: directory,
            project_root,
            store,
            packages,
            project,
        }
    }

    fn request(&self) -> PackagePlanRequest {
        PackagePlanRequest {
            action: PlanAction::Install,
            project_id: self.project.project_id,
            expected_revision: self.project.revision,
            package_id: Some(NEW.to_owned()),
            version_range: Some("2.0.0".to_owned()),
            repository_id: None,
            include_prerelease: false,
            plan_version: PackagePlanVersion::V2,
            source_selector: None,
            reinstall_selection: None,
            reinstall_sources: Vec::new(),
            bulk_intents: Vec::new(),
        }
    }

    async fn replace_indexed_legacy_packages(&self, packages: &[&str]) {
        let path = self._directory.0.join("index.json");
        let mut index: Value =
            serde_json::from_slice(&fs::read(&path).expect("owned index")).expect("index JSON");
        index["packages"][NEW]["versions"]["2.0.0"]["legacyPackages"] = json!(packages);
        fs::write(path, serde_json::to_vec(&index).expect("index JSON"))
            .expect("change actual indexed package metadata");
        let registry = M3Application::new(self.store.clone(), VpmReader::new().expect("reader"));
        let access = AccessContext::local_owner();
        let repositories = registry
            .list_repositories(&access, None, 10)
            .await
            .expect("owned repositories");
        assert_eq!(repositories.repositories.len(), 1);
        let repository = &repositories.repositories[0];
        registry
            .refresh_repository(
                &access,
                repository.repository_id,
                repository.revision,
                key("refresh-conflicting-legacy-packages"),
            )
            .await
            .expect("refresh actual metadata through the Core reader");
    }

    async fn terminal(&self, id: OperationId) -> OperationRecord {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let operation = self
                    .store
                    .get_operation(PrincipalId::local_owner(), id)
                    .await
                    .expect("owned operation");
                if operation.state.is_terminal() {
                    return operation;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("package worker terminates")
    }

    async fn replace_project_manifest(&mut self, manifest: &Value) {
        fs::write(
            self.project_root.join("Packages/vpm-manifest.json"),
            serde_json::to_vec(manifest).expect("serialize isolated manifest"),
        )
        .expect("write isolated manifest");
        let registry = M3Application::new(self.store.clone(), VpmReader::new().expect("reader"));
        self.project = registry
            .refresh_project(
                &AccessContext::local_owner(),
                self.project.project_id,
                self.project.revision,
                key("refresh-isolated-graph"),
            )
            .await
            .expect("refresh isolated project")
            .value;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn install_cannot_downgrade_a_shared_dependency_past_an_existing_constraint() {
    let mut fixture = Fixture::new(false, ExistingLegacy::None).await;
    let manifest = json!({
        "dependencies": {KEEP: {"version": "1.0.0"}},
        "locked": {
            KEEP: {"version": "1.0.0", "dependencies": {CHILD: "^2.0.0"}},
            CHILD: {"version": "2.0.0", "dependencies": {}}
        }
    });
    fixture.replace_project_manifest(&manifest).await;
    let old = fixture.project_root.join("Packages").join(CHILD);
    fs::create_dir(&old).expect("existing dependency");
    fs::write(
        old.join("package.json"),
        serde_json::to_vec(&json!({
            "name": CHILD, "version": "2.0.0"
        }))
        .expect("encode manifest"),
    )
    .expect("seed existing dependency");
    let error = fixture
        .packages
        .plan(&AccessContext::local_owner(), fixture.request())
        .await
        .expect_err("the selected package cannot invalidate another installed package");
    assert_eq!(error.code(), M4ErrorCode::PackageDependencyConflict);
    assert!(!fixture.project_root.join("Packages").join(NEW).exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn install_preserves_a_satisfying_locked_dependency_missing_from_the_catalog() {
    let mut fixture = Fixture::new(false, ExistingLegacy::None).await;
    let manifest = json!({
        "dependencies": {KEEP: {"version": "1.0.0"}},
        "locked": {
            KEEP: {"version": "1.0.0", "dependencies": {CHILD: "^1.0.0"}},
            CHILD: {"version": "1.5.0", "dependencies": {}, "note": "preserve"}
        }
    });
    fixture.replace_project_manifest(&manifest).await;
    let old = fixture.project_root.join("Packages").join(CHILD);
    fs::create_dir(&old).expect("existing dependency");
    fs::write(
        old.join("package.json"),
        serde_json::to_vec(&json!({
            "name": CHILD, "version": "1.5.0"
        }))
        .expect("encode manifest"),
    )
    .expect("seed unlisted dependency");
    let access = AccessContext::local_owner();
    let plan = fixture
        .packages
        .plan(&access, fixture.request())
        .await
        .expect("keep unlisted installed dependency");
    assert_eq!(plan.change_set.mutations.len(), 1);
    assert_eq!(plan.change_set.mutations[0].package_id, NEW);
    let accepted = fixture
        .packages
        .apply_plan(
            &access,
            plan.plan_id,
            fixture.project.revision,
            key("apply-with-retained-dependency"),
        )
        .await
        .expect("accept install");
    assert_eq!(
        fixture.terminal(accepted.operation_id).await.state,
        OperationState::Succeeded
    );
    let bytes =
        fs::read(fixture.project_root.join("Packages/vpm-manifest.json")).expect("result manifest");
    let result: Value = serde_json::from_slice(&bytes).expect("parse result manifest");
    assert_eq!(result["locked"][CHILD], manifest["locked"][CHILD]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn resolve_repairs_missing_and_wrong_versions_without_rewriting_the_lock() {
    let mut fixture = Fixture::new(false, ExistingLegacy::None).await;
    let manifest = json!({
        "dependencies": {NEW: {"version": "^2.0.0"}},
        "locked": {
            NEW: {"version": "2.0.0", "dependencies": {CHILD: "^1.0.0"}, "note": "preserve"},
            CHILD: {"version": "1.0.0", "dependencies": {}}
        }
    });
    fixture.replace_project_manifest(&manifest).await;
    let wrong = fixture.project_root.join("Packages").join(CHILD);
    fs::create_dir(&wrong).expect("wrong version fixture");
    fs::write(
        wrong.join("package.json"),
        serde_json::to_vec(&json!({
            "name": CHILD, "version": "2.0.0"
        }))
        .expect("encode manifest"),
    )
    .expect("seed wrong version");
    let access = AccessContext::local_owner();
    let mut request = fixture.request();
    request.action = PlanAction::Resolve;
    request.package_id = None;
    request.version_range = None;
    let plan = fixture
        .packages
        .plan(&access, request.clone())
        .await
        .expect("plan exact repairs");
    assert_eq!(plan.change_set.mutations.len(), 2);
    let child = plan
        .change_set
        .mutations
        .iter()
        .find(|mutation| mutation.package_id == CHILD)
        .expect("wrong version repair");
    assert_eq!(child.from_version.as_deref(), Some("2.0.0"));
    assert_eq!(child.to_version.as_deref(), Some("1.0.0"));
    let accepted = fixture
        .packages
        .apply_plan(
            &access,
            plan.plan_id,
            fixture.project.revision,
            key("apply-exact-repair"),
        )
        .await
        .expect("accept repair");
    let operation = fixture.terminal(accepted.operation_id).await;
    assert_eq!(
        operation.state,
        OperationState::Succeeded,
        "{:?}",
        operation.error_code
    );
    let bytes =
        fs::read(fixture.project_root.join("Packages/vpm-manifest.json")).expect("result manifest");
    let result: Value = serde_json::from_slice(&bytes).expect("parse result manifest");
    assert_eq!(result, manifest);
    for (id, version) in [(NEW, "2.0.0"), (CHILD, "1.0.0")] {
        let bytes = fs::read(
            fixture
                .project_root
                .join("Packages")
                .join(id)
                .join("package.json"),
        )
        .expect("restored package");
        assert_eq!(
            serde_json::from_slice::<Value>(&bytes).expect("parse package")["version"],
            version
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn absent_legacy_obligations_allow_real_install_and_preserve_frozen_lock_information() {
    let fixture = Fixture::new(true, ExistingLegacy::None).await;
    let access = AccessContext::local_owner();
    let manifest_path = fixture.project_root.join("Packages/vpm-manifest.json");
    let before: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("original manifest"))
            .expect("JSON");
    let plan = fixture
        .packages
        .plan(&access, fixture.request())
        .await
        .expect("metadata alone does not forbid installation");
    assert_eq!(plan.change_set.format_version, 4);
    assert!(plan.change_set.legacy_removals.is_empty());
    let dependency = plan
        .change_set
        .dependency_edges
        .iter()
        .find(|edge| !edge.direct && edge.from_package_id == NEW && edge.to_package_id == CHILD)
        .expect("frozen dependency edge")
        .range
        .clone();
    let accepted = fixture
        .packages
        .apply_plan(
            &access,
            plan.plan_id,
            fixture.project.revision,
            key("install"),
        )
        .await
        .expect("accept actual Plan");
    let operation = fixture.terminal(accepted.operation_id).await;
    assert_eq!(
        operation.state,
        OperationState::Succeeded,
        "{:?}",
        operation.error_code
    );
    let after_bytes = fs::read(&manifest_path).expect("published manifest");
    let after: Value = serde_json::from_slice(&after_bytes).expect("published JSON");
    assert_eq!(after["locked"][KEEP], before["locked"][KEEP]);
    assert_eq!(after["dependencies"][KEEP], before["dependencies"][KEEP]);
    assert_eq!(after["locked"][NEW]["dependencies"][CHILD], dependency);
    assert_eq!(after["locked"][CHILD]["version"], "1.0.0");
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(&after_bytes)),
        plan.change_set.vpm_manifest_sha256
    );
    assert_eq!(
        fs::read(
            fixture
                .project_root
                .join("Packages/com.example.keep/keep.txt")
        )
        .expect("old package retained"),
        b"existing package bytes"
    );
    assert!(
        fixture
            .project_root
            .join("Packages/com.example.new/package.json")
            .is_file()
    );
    assert!(
        fixture
            .project_root
            .join("Packages/com.example.child/package.json")
            .is_file()
    );
    let replay = fixture
        .packages
        .apply_plan(
            &access,
            plan.plan_id,
            fixture.project.revision,
            key("install"),
        )
        .await
        .expect("idempotent replay");
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, accepted.operation_id);
    assert_eq!(
        fs::read(manifest_path).expect("replayed bytes"),
        after_bytes
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ordinary_apply_removes_reviewed_paths_moved_assets_and_legacy_packages() {
    for (existing, expected) in [
        (ExistingLegacy::File, &["Assets/Old.txt"][..]),
        (
            ExistingLegacy::Folder,
            &["Assets/Legacy", "Assets/Legacy.meta"][..],
        ),
        (
            ExistingLegacy::MovedGuid,
            &["Assets/Moved.txt", "Assets/Moved.txt.meta"][..],
        ),
        (
            ExistingLegacy::Package,
            &["Packages/com.example.legacy"][..],
        ),
        (ExistingLegacy::MissingPackageDirectory, &[][..]),
    ] {
        let fixture = Fixture::new(true, existing).await;
        let before = fs::read(fixture.project_root.join("Packages/vpm-manifest.json"))
            .expect("original manifest");
        let access = AccessContext::local_owner();
        let plan = fixture
            .packages
            .plan(&access, fixture.request())
            .await
            .expect("review normal installation");
        assert_eq!(
            plan.change_set
                .legacy_removals
                .iter()
                .map(|entry| entry.path.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        for path in expected {
            assert!(fixture.project_root.join(path).exists());
        }
        assert_eq!(
            fs::read(fixture.project_root.join("Packages/vpm-manifest.json"))
                .expect("untouched manifest"),
            before
        );
        let accepted = fixture
            .packages
            .apply_plan(
                &access,
                plan.plan_id,
                fixture.project.revision,
                key("apply-reviewed-legacy"),
            )
            .await
            .expect("apply reviewed legacy cleanup");
        let operation = fixture.terminal(accepted.operation_id).await;
        assert_eq!(
            operation.state,
            OperationState::Succeeded,
            "{:?}",
            operation.error_code
        );
        for path in expected {
            assert!(!fixture.project_root.join(path).exists());
        }
        let manifest: Value = serde_json::from_slice(
            &fs::read(fixture.project_root.join("Packages/vpm-manifest.json"))
                .expect("read the committed manifest"),
        )
        .expect("parse the committed manifest");
        assert!(manifest["dependencies"].get("com.example.legacy").is_none());
        assert!(manifest["locked"].get("com.example.legacy").is_none());
        assert_eq!(manifest["locked"][NEW]["version"], "2.0.0");
        let operation_dir = fixture
            .project_root
            .join("Library/ALCOMD/transactions")
            .join(accepted.operation_id.to_string());
        assert!(
            fs::read_dir(operation_dir)
                .expect("read the transaction directory")
                .all(|entry| !entry
                    .expect("read the transaction attempt")
                    .path()
                    .join("legacy-backup")
                    .exists())
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn indexed_legacy_evidence_remains_authoritative_when_the_archive_omits_it() {
    let fixture = Fixture::new(false, ExistingLegacy::File).await;
    let plan = fixture
        .packages
        .plan(&AccessContext::local_owner(), fixture.request())
        .await
        .expect("archive omission cannot erase indexed cleanup obligations");
    assert_eq!(plan.change_set.legacy_removals[0].path, "Assets/Old.txt");
    assert_eq!(
        fs::read(fixture.project_root.join("Assets/Old.txt")).expect("legacy file retained"),
        b"legacy content"
    );
    assert!(
        !fixture
            .project_root
            .join("Packages/com.example.new")
            .exists()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cleanup_of_a_package_selected_by_the_same_plan_is_not_proven_absent() {
    let fixture = Fixture::new(true, ExistingLegacy::None).await;
    let path = fixture.project_root.join("Packages/vpm-manifest.json");
    let before = fs::read(&path).expect("original manifest");
    let original: Value = serde_json::from_slice(&before).expect("manifest JSON");
    assert!(original["locked"].get(CHILD).is_none());
    assert!(!fixture.project_root.join("Packages").join(CHILD).exists());
    fixture.replace_indexed_legacy_packages(&[CHILD]).await;
    let error = fixture.packages.plan(&AccessContext::local_owner(), fixture.request()).await
        .expect_err("a dependency selected for installation cannot simultaneously be discharged as absent legacy content");
    assert_eq!(error.code(), M4ErrorCode::PackageDependencyConflict);
    assert_eq!(fs::read(path).expect("original manifest retained"), before);
    for package in [NEW, CHILD] {
        assert!(!fixture.project_root.join("Packages").join(package).exists());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_legacy_target_created_after_planning_fails_apply_before_changing_packages() {
    let fixture = Fixture::new(true, ExistingLegacy::None).await;
    let access = AccessContext::local_owner();
    let plan = fixture
        .packages
        .plan(&access, fixture.request())
        .await
        .expect("absence proven while planning");
    let original_vpm =
        fs::read(fixture.project_root.join("Packages/vpm-manifest.json")).expect("VPM manifest");
    let original_upm =
        fs::read(fixture.project_root.join("Packages/manifest.json")).expect("UPM manifest");
    seed_legacy(&fixture.project_root, ExistingLegacy::File);
    let accepted = fixture
        .packages
        .apply_plan(
            &access,
            plan.plan_id,
            fixture.project.revision,
            key("changed-before-apply"),
        )
        .await
        .expect("accepted before execution rechecks target evidence");
    let operation = fixture.terminal(accepted.operation_id).await;
    assert_eq!(operation.state, OperationState::Failed);
    assert_eq!(operation.error_code.as_deref(), Some("plan_stale"));
    assert_eq!(
        fs::read(fixture.project_root.join("Packages/vpm-manifest.json"))
            .expect("original VPM preserved"),
        original_vpm
    );
    assert_eq!(
        fs::read(fixture.project_root.join("Packages/manifest.json"))
            .expect("original UPM preserved"),
        original_upm
    );
    assert_eq!(
        fs::read(
            fixture
                .project_root
                .join("Packages/com.example.keep/keep.txt")
        )
        .expect("original package preserved"),
        b"existing package bytes"
    );
    assert!(
        !fixture
            .project_root
            .join("Packages/com.example.new")
            .exists()
    );
    assert!(
        !fixture
            .project_root
            .join("Packages/com.example.child")
            .exists()
    );
    assert_eq!(
        fs::read(fixture.project_root.join("Assets/Old.txt"))
            .expect("newly observed legacy file retained"),
        b"legacy content"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn failed_package_publish_restores_quarantined_assets_without_touching_conflict() {
    let fixture = Fixture::new(true, ExistingLegacy::MovedGuid).await;
    let access = AccessContext::local_owner();
    let plan = fixture
        .packages
        .plan(&access, fixture.request())
        .await
        .expect("plan");
    let conflict = fixture.project_root.join("Packages").join(NEW);
    fs::create_dir(&conflict).expect("create a conflicting package directory");
    fs::write(conflict.join("sentinel"), b"external directory").expect("seed conflict sentinel");
    let accepted = fixture
        .packages
        .apply_plan(
            &access,
            plan.plan_id,
            fixture.project.revision,
            key("publish-conflict"),
        )
        .await
        .expect("accepted apply");
    let operation = fixture.terminal(accepted.operation_id).await;
    assert_eq!(operation.state, OperationState::Failed);
    assert_eq!(
        fs::read(conflict.join("sentinel")).expect("read untouched conflict"),
        b"external directory"
    );
    assert!(!fixture.project_root.join("Packages").join(CHILD).exists());
    assert_eq!(
        fs::read(fixture.project_root.join("Assets/Moved.txt")).expect("read restored asset"),
        b"moved legacy content"
    );
    assert!(fixture.project_root.join("Assets/Moved.txt.meta").is_file());
}

#[tokio::test]
async fn malformed_publisher_digest_is_logged_and_pinned_but_a_valid_mismatch_is_rejected() {
    let fixture = Fixture::new(false, ExistingLegacy::None).await;
    let access = AccessContext::local_owner();
    let registry = M3Application::new(fixture.store.clone(), VpmReader::new().expect("reader"));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("fixture listener");
    let url = format!(
        "http://{}/archive.zip",
        listener.local_addr().expect("fixture address")
    );
    let package = "com.example.digest";
    let mut accepted = json!({"name": package, "version": "1.0.0"});
    let bytes = package_archive(&accepted);
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    let invalid_value = "fixture-invalid-digest-not-for-logs";
    add_repository_fields(&mut accepted, &digest);
    accepted["url"] = json!(url);
    accepted["zipSHA256"] = json!(invalid_value);
    let mut mismatched = accepted.clone();
    mismatched["version"] = json!("2.0.0");
    mismatched["zipSHA256"] = json!("FF".repeat(32));
    let index = fixture._directory.0.join("digest-index.json");
    fs::write(&index, serde_json::to_vec(&json!({
        "id": "digest-fixture", "packages": {package: {"versions": {"1.0.0": accepted, "2.0.0": mismatched}}}
    })).expect("index JSON")).expect("write index");
    let source = RepositorySource::Local {
        path: index.to_string_lossy().into_owned(),
    };
    with_log_caller(
        fixture.store.clone(),
        &access,
        registry.inspect_repository(&access, "digest-fixture-preview", source.clone(), None),
    )
    .await
    .expect("repository preview");
    let log = read_operation_logs(
        &fixture.store,
        &access,
        OperationLogQuery {
            limit: 20,
            ..Default::default()
        },
    )
    .await
    .expect("technical log");
    let note = log
        .entries
        .iter()
        .find(|entry| entry.step == "ignore_invalid_zip_sha256")
        .expect("recorded ignored digest");
    assert_eq!(
        note.target.as_ref().and_then(|target| target.id.as_deref()),
        Some("digest-fixture")
    );
    let details = note.details.as_deref().expect("log detail");
    assert!(details.contains("com.example.digest@1.0.0"));
    assert!(!details.contains(invalid_value));
    let repository = registry
        .register_repository(&access, source, key("register-digest-fixture"))
        .await
        .expect("register repository")
        .value;

    let server = tokio::spawn(async move {
        for _ in 0..2 {
            let (stream, _) = listener.accept().await.expect("archive request");
            let mut reader = tokio::io::BufReader::new(stream);
            let mut line = String::new();
            while reader.read_line(&mut line).await.expect("request header") > 0 && line != "\r\n" {
                line.clear();
            }
            let mut stream = reader.into_inner();
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        bytes.len()
                    )
                    .as_bytes(),
                )
                .await
                .expect("response header");
            stream.write_all(&bytes).await.expect("archive body");
        }
    });
    let mut request = fixture.request();
    request.package_id = Some(package.to_owned());
    request.repository_id = Some(repository.repository_id.to_string());
    request.version_range = Some("2.0.0".to_owned());
    let error = fixture
        .packages
        .plan(&access, request.clone())
        .await
        .expect_err("reject mismatched valid digest");
    assert_eq!(error.code(), M4ErrorCode::PackageIntegrityMismatch);

    request.version_range = Some("1.0.0".to_owned());
    let plan = fixture
        .packages
        .plan(&access, request)
        .await
        .expect("plan from verified first download");
    assert_eq!(plan.source_set[0].archive_sha256(), digest);
    let catalog = fixture
        .store
        .resolver_catalog(access.principal().clone(), false)
        .await
        .expect("indexed catalog");
    assert!(catalog.entries.iter().any(|entry| entry.package_id == package && entry.version == "1.0.0"
        && matches!(&entry.source, ResolverCatalogSource::Repository { zip_sha256: None, local_zip_sha256: Some(pin), .. } if *pin == digest)));
    let install = fixture
        .packages
        .apply_plan(
            &access,
            plan.plan_id,
            fixture.project.revision,
            key("apply-digest-fixture"),
        )
        .await
        .expect("accept install");
    assert_eq!(
        fixture.terminal(install.operation_id).await.state,
        OperationState::Succeeded
    );
    assert!(
        fixture
            .project_root
            .join("Packages")
            .join(package)
            .join("package.json")
            .is_file()
    );
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .expect("both downloads completed")
        .expect("fixture server");
}

fn add_legacy(manifest: &mut Value) {
    manifest["legacyFiles"] = json!({"Assets/Old.txt": FILE_GUID});
    manifest["legacyFolders"] = json!({"Assets/Legacy": FOLDER_GUID});
    manifest["legacyPackages"] = json!(["com.example.legacy"]);
}

fn add_repository_fields(manifest: &mut Value, digest: &[u8; 32]) {
    manifest["displayName"] = json!(manifest["name"]);
    manifest["author"] = json!({"name": "Isolated fixture", "email": "fixture@example.invalid"});
    manifest["url"] = json!("https://network-must-not-be-used.invalid/synthetic.zip");
    manifest["zipSHA256"] = json!(
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
}

fn package_archive(manifest: &Value) -> Vec<u8> {
    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    archive
        .start_file(
            "package.json",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .expect("manifest entry");
    archive
        .write_all(
            serde_json::to_string(manifest)
                .expect("manifest JSON")
                .as_bytes(),
        )
        .expect("manifest bytes");
    archive
        .start_file(
            "Runtime/fixture.txt",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .expect("payload entry");
    archive
        .write_all(b"independently authored synthetic package content")
        .expect("payload");
    archive.finish().expect("archive").into_inner()
}

fn cache_package(cache: &PackageCache, manifest: &Value) -> [u8; 32] {
    let bytes = package_archive(manifest);
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    let path = cache.object_path(&digest);
    fs::create_dir_all(path.parent().expect("cache parent")).expect("owned cache parent");
    fs::write(path, bytes).expect("owned cached package");
    digest
}

fn seed_legacy(root: &Path, existing: ExistingLegacy) {
    match existing {
        ExistingLegacy::None => {}
        ExistingLegacy::File => {
            fs::write(root.join("Assets/Old.txt"), b"legacy content").expect("owned legacy file")
        }
        ExistingLegacy::Folder => {
            fs::create_dir(root.join("Assets/Legacy")).expect("owned legacy directory");
            fs::write(root.join("Assets/Legacy/data.txt"), b"nested legacy data")
                .expect("seed nested legacy asset");
            fs::write(
                root.join("Assets/Legacy.meta"),
                format!("guid: {FOLDER_GUID}\n"),
            )
            .expect("seed the old folder metadata");
        }
        ExistingLegacy::Package => {
            let package = root.join("Packages/com.example.legacy");
            fs::create_dir(&package).expect("seed the old package directory");
            fs::write(
                package.join("package.json"),
                br#"{"name":"com.example.legacy","version":"1.0.0"}"#,
            )
            .expect("seed the old package identity");
        }
        ExistingLegacy::MissingPackageDirectory => {}
        ExistingLegacy::MovedGuid => {
            fs::write(root.join("Assets/Moved.txt"), b"moved legacy content").expect("moved file");
            fs::write(
                root.join("Assets/Moved.txt.meta"),
                format!("fileFormatVersion: 2\nguid: {FILE_GUID}\n"),
            )
            .expect("moved GUID evidence");
        }
    }
}

fn key(value: &str) -> IdempotencyKey {
    IdempotencyKey::parse(value.to_owned()).expect("key")
}

struct TestDirectory(PathBuf);
impl TestDirectory {
    fn new() -> Self {
        #[cfg(target_os = "macos")]
        let base = PathBuf::from("/private/tmp");
        #[cfg(not(target_os = "macos"))]
        let base = std::env::temp_dir();
        let path = base.join(format!("alcomd-m7-legacy-{}", Uuid::new_v4()));
        fs::create_dir(&path).expect("new owned fixture root");
        Self(path)
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
