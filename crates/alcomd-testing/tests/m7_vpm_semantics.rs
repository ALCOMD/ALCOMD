//! Real policy, reader, store, Plan/Apply regressions for the seven audited VPM gaps.
//! All writes and cached synthetic packages are owned by each isolated fixture.
use alcomd_application::*;
use alcomd_store::StateStoreHandle;
use alcomd_vpm::{PackageCache, PackageEngine, ProjectCandidateEngine, VpmReader};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

const A: &str = "com.example.parent";
const B: &str = "com.example.child";
const C: &str = "com.example.orphan";
const D: &str = "com.example.new";
const L: &str = "com.example.local";
type Packages = M4Application<StateStoreHandle, PackageEngine<StateStoreHandle>>;
struct Fixture {
    directory: PathBuf,
    root: PathBuf,
    project: ProjectRecord,
    store: StateStoreHandle,
    packages: Packages,
    config: M7OfficialApplication,
    reader: VpmReader,
    repository: RepositoryId,
}
fn key(s: &str) -> IdempotencyKey {
    IdempotencyKey::parse(s).expect("owned VPM regression fixture succeeds")
}
fn package(id: &str, version: &str, deps: Value) -> Value {
    json!({"name":id,"version":version,"vpmDependencies":deps})
}
fn seed(root: &Path, manifest: &Value) {
    let dir = root.join("Packages").join(
        manifest["name"]
            .as_str()
            .expect("owned VPM regression fixture succeeds"),
    );
    fs::create_dir_all(&dir).expect("owned VPM regression fixture succeeds");
    fs::write(
        dir.join("package.json"),
        serde_json::to_vec(manifest).expect("owned VPM regression fixture succeeds"),
    )
    .expect("owned VPM regression fixture succeeds");
    fs::write(dir.join("sentinel.txt"), b"owned audit fixture")
        .expect("owned VPM regression fixture succeeds");
}
async fn fixture(
    label: &str,
    graph: Value,
    manifests: Vec<Value>,
    missing: Option<&str>,
) -> Fixture {
    let dir = std::env::temp_dir().join(format!("alcomd-seven-{label}-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&dir).expect("owned VPM regression fixture succeeds");
    let root = dir.join("Project");
    for part in ["Assets", "Packages", "ProjectSettings"] {
        fs::create_dir_all(root.join(part)).expect("owned VPM regression fixture succeeds");
    }
    fs::write(
        root.join("ProjectSettings/ProjectVersion.txt"),
        "m_EditorVersion: 2022.3.22f1\n",
    )
    .expect("owned VPM regression fixture succeeds");
    fs::write(
        root.join("Packages/manifest.json"),
        b"{\"dependencies\":{}}",
    )
    .expect("owned VPM regression fixture succeeds");
    fs::write(
        root.join("Packages/vpm-manifest.json"),
        serde_json::to_vec(&graph).expect("owned VPM regression fixture succeeds"),
    )
    .expect("owned VPM regression fixture succeeds");
    for (id, entry) in graph["locked"]
        .as_object()
        .expect("owned VPM regression fixture succeeds")
    {
        if missing != Some(id.as_str()) {
            seed(
                &root,
                &package(
                    id,
                    entry["version"]
                        .as_str()
                        .expect("owned VPM regression fixture succeeds"),
                    entry.get("dependencies").cloned().unwrap_or(json!({})),
                ),
            );
        }
    }
    let cache_root = dir.join("cache");
    let cache =
        PackageCache::new(cache_root.clone()).expect("owned VPM regression fixture succeeds");
    let mut index = json!({"name":"Owned deep audit", "id":"owned.deep.audit", "packages":{}});
    for mut manifest in manifests {
        let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        archive
            .start_file(
                "package.json",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .expect("owned VPM regression fixture succeeds");
        archive
            .write_all(
                &serde_json::to_vec(&manifest).expect("owned VPM regression fixture succeeds"),
            )
            .expect("owned VPM regression fixture succeeds");
        archive
            .start_file(
                "sentinel.txt",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .expect("owned VPM regression fixture succeeds");
        archive
            .write_all(b"owned cached archive")
            .expect("owned VPM regression fixture succeeds");
        let bytes = archive
            .finish()
            .expect("owned VPM regression fixture succeeds")
            .into_inner();
        let digest: [u8; 32] = Sha256::digest(&bytes).into();
        let object = cache.object_path(&digest);
        fs::create_dir_all(
            object
                .parent()
                .expect("owned VPM regression fixture succeeds"),
        )
        .expect("owned VPM regression fixture succeeds");
        fs::write(object, bytes).expect("owned VPM regression fixture succeeds");
        manifest["displayName"] = manifest["name"].clone();
        manifest["author"] = json!({"name":"Owned fixture"});
        manifest["url"] = json!("https://network-must-not-be-used.invalid/package.zip");
        manifest["zipSHA256"] = json!(
            digest
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        let id = manifest["name"]
            .as_str()
            .expect("owned VPM regression fixture succeeds")
            .to_owned();
        let version = manifest["version"]
            .as_str()
            .expect("owned VPM regression fixture succeeds")
            .to_owned();
        if index["packages"].get(&id).is_none() {
            index["packages"][&id] = json!({"versions":{}});
        }
        index["packages"][&id]["versions"][&version] = manifest;
    }
    let index_path = dir.join("index.json");
    fs::write(
        &index_path,
        serde_json::to_vec(&index).expect("owned VPM regression fixture succeeds"),
    )
    .expect("owned VPM regression fixture succeeds");
    let store = StateStoreHandle::open(dir.join("state.db"))
        .expect("owned VPM regression fixture succeeds");
    let ports = Arc::new(NoDesktop(dir.clone()));
    let config = M7OfficialApplication::new(
        Arc::new(alcomd_store::FileConfigStore::new(
            dir.join("settings.toml"),
        )),
        ports.clone(),
        ports.clone(),
        ports,
    );
    config
        .initialize_settings()
        .await
        .expect("owned VPM regression fixture succeeds");
    let reader =
        VpmReader::with_settings(config.clone()).expect("owned VPM regression fixture succeeds");
    let registry = M3Application::new(store.clone(), reader.clone());
    let access = AccessContext::local_owner();
    let project = registry
        .register_project(
            &access,
            root.to_string_lossy().into_owned(),
            key("register-project"),
        )
        .await
        .expect("owned VPM regression fixture succeeds")
        .value;
    let repo = registry
        .register_repository(
            &access,
            RepositorySource::Local {
                path: index_path.to_string_lossy().into_owned(),
            },
            key("register-repo"),
        )
        .await
        .expect("owned VPM regression fixture succeeds")
        .value;
    assert!(
        repo.observation.issues.is_empty(),
        "{:?}",
        repo.observation.issues
    );
    registry
        .refresh_repository(
            &access,
            repo.repository_id,
            repo.revision,
            key("refresh-repo"),
        )
        .await
        .expect("owned VPM regression fixture succeeds");
    let packages = M4Application::new(
        store.clone(),
        PackageEngine::new(store.clone(), reader.clone(), cache_root)
            .expect("owned VPM regression fixture succeeds"),
    );
    Fixture {
        directory: dir,
        root,
        project,
        store,
        packages,
        config,
        reader,
        repository: repo.repository_id,
    }
}
fn request(f: &Fixture, action: PlanAction, target: Option<&str>) -> PackagePlanRequest {
    PackagePlanRequest {
        allow_unity_incompatible: false,
        action,
        project_id: f.project.project_id,
        expected_revision: f.project.revision,
        package_id: target.map(str::to_owned),
        version_range: None,
        repository_id: None,
        include_prerelease: false,
        plan_version: PackagePlanVersion::V2,
        source_selector: None,
        reinstall_selection: None,
        reinstall_sources: vec![],
        bulk_intents: vec![],
    }
}
async fn apply(f: &Fixture, plan: PackagePlanRecord) -> OperationRecord {
    let operation = f
        .packages
        .apply_plan(
            &AccessContext::local_owner(),
            plan.plan_id,
            f.project.revision,
            key("apply-owned-plan"),
        )
        .await
        .expect("owned VPM regression fixture succeeds");
    for _ in 0..1000 {
        let result = f
            .store
            .get_operation(PrincipalId::local_owner(), operation.operation_id)
            .await
            .expect("owned VPM regression fixture succeeds");
        if result.state.is_terminal() {
            return result;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("owned operation did not finish")
}

struct NoDesktop(PathBuf);
impl StorageDirectoryPlatform for NoDesktop {
    fn default_directories(&self) -> Result<StorageDirectories, OfficialGuiError> {
        Ok(StorageDirectories {
            project_directory: self.0.join("Projects").to_string_lossy().into_owned(),
            backup_directory: self.0.join("Backups").to_string_lossy().into_owned(),
        })
    }
    fn normalize_directory(&self, _: &str) -> Result<String, OfficialGuiError> {
        panic!("no desktop directory mutation")
    }
}
impl UnityHubPlatform for NoDesktop {
    fn normalize_path(&self, _: &str) -> Result<String, OfficialGuiError> {
        panic!("no Hub inspection")
    }
    fn inspect(&self, _: Option<&str>) -> UnityHubLocation {
        panic!("no Hub inspection")
    }
}
impl UnityHubLauncher for NoDesktop {
    fn launch(&self, _: &str) -> Result<(), UnityHubOpenError> {
        panic!("no Hub launch")
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}
impl Fixture {
    async fn settings(&self, change: impl FnOnce(&mut ConfigPackageSettings)) {
        let current = self
            .config
            .candidate_settings()
            .await
            .expect("owned VPM regression fixture succeeds");
        let mut value = current.settings.packages;
        change(&mut value);
        self.config
            .update_settings(
                &AccessContext::local_owner(),
                current.revision,
                ConfigUpdate {
                    packages: Some(ConfigPackageSettingsUpdate {
                        show_prerelease: Some(value.show_prerelease),
                        hidden_repository_ids: Some(value.hidden_repository_ids),
                        hide_local_user_packages: Some(value.hide_local_user_packages),
                    }),
                    ..Default::default()
                },
            )
            .await
            .expect("owned VPM regression fixture succeeds");
    }
    async fn summary(&self) -> ProjectCandidatePage {
        ProjectCandidateApplication::new(
            self.store.clone(),
            ProjectCandidateEngine,
            self.reader.clone(),
            self.config.clone(),
        )
        .query(
            &AccessContext::local_owner(),
            ProjectCandidateRequest {
                project_id: self.project.project_id.to_string(),
                expected_revision: self.project.revision.get(),
                view: CandidateView::Summary {
                    package_ids: None,
                    sources: None,
                    filter: None,
                    bulk_package_ids: None,
                },
                limit: None,
                expected_snapshot: None,
                cursor: None,
            },
        )
        .await
        .expect("owned VPM regression fixture succeeds")
    }
    fn manifest(&self) -> Value {
        serde_json::from_slice(
            &fs::read(self.root.join("Packages/vpm-manifest.json"))
                .expect("owned VPM regression fixture succeeds"),
        )
        .expect("owned VPM regression fixture succeeds")
    }
}
fn empty() -> Value {
    json!({"dependencies":{},"locked":{}})
}
fn install(f: &Fixture, id: &str, version: &str) -> PackagePlanRequest {
    let mut r = request(f, PlanAction::Install, Some(id));
    r.version_range = Some(format!("={version}"));
    r
}
fn bulk(f: &Fixture, intents: Vec<PackageBulkIntent>) -> PackagePlanRequest {
    let mut r = request(f, PlanAction::Bulk, None);
    r.bulk_intents = intents;
    r
}
fn install_intent(id: &str, version: &str) -> PackageBulkIntent {
    PackageBulkIntent::Install {
        package_id: id.into(),
        version_range: Some(format!("={version}")),
        source: None,
        include_prerelease: false,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn closing_a_source_during_archive_preparation_does_not_publish_a_plan() {
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    let f = fixture(
        "close-during-plan",
        empty(),
        vec![package(A, "1.0.0", json!({}))],
        None,
    )
    .await;
    let index_path = f.directory.join("index.json");
    let mut index: Value =
        serde_json::from_slice(&fs::read(&index_path).expect("index")).expect("index JSON");
    let digest = index["packages"][A]["versions"]["1.0.0"]["zipSHA256"]
        .as_str()
        .expect("digest");
    let mut hash = [0; 32];
    for (i, byte) in hash.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&digest[i * 2..i * 2 + 2], 16).expect("digest byte");
    }
    let object = PackageCache::new(f.directory.join("cache"))
        .expect("cache")
        .object_path(&hash);
    let archive = fs::read(&object).expect("owned cached archive");
    fs::remove_file(object).expect("remove owned object to pause the next preparation");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("owned loopback server");
    index["packages"][A]["versions"]["1.0.0"]["url"] = json!(format!(
        "http://{}/archive.zip",
        listener.local_addr().expect("server address")
    ));
    fs::write(index_path, serde_json::to_vec(&index).expect("index JSON"))
        .expect("update owned source");
    let record = f
        .store
        .get_repository(PrincipalId::local_owner(), f.repository)
        .await
        .expect("source");
    M3Application::new(f.store.clone(), f.reader.clone())
        .refresh_repository(
            &AccessContext::local_owner(),
            f.repository,
            record.revision,
            key("prepare-race-source"),
        )
        .await
        .expect("refresh owned source");
    let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("archive request");
        let mut request = Vec::new();
        let mut buffer = [0; 1024];
        while !request.ends_with(b"\r\n\r\n") {
            let count = stream.read(&mut buffer).await.expect("request headers");
            assert!(count > 0 && request.len() < 16_384);
            request.extend_from_slice(&buffer[..count]);
        }
        entered_tx.send(()).expect("signal preparation");
        release_rx.await.expect("release archive");
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    archive.len()
                )
                .as_bytes(),
            )
            .await
            .expect("response header");
        stream.write_all(&archive).await.expect("archive body");
    });
    let application = f.packages.clone();
    let request = install(&f, A, "1.0.0");
    let pending = tokio::spawn(async move {
        application
            .plan(&AccessContext::local_owner(), request)
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), entered_rx)
        .await
        .expect("plan reaches archive preparation")
        .expect("preparation signal");
    f.settings(|settings| settings.hidden_repository_ids = vec![f.repository.to_string()])
        .await;
    release_tx.send(()).expect("release preparation");
    assert_eq!(
        pending
            .await
            .expect("plan task")
            .expect_err("closed source cannot publish a plan")
            .code(),
        M4ErrorCode::PlanStale
    );
    server.await.expect("owned server");
    assert_eq!(f.manifest(), empty());
    assert!(!f.root.join("Packages").join(A).exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn closed_repository_is_absent_from_queries_plans_and_frozen_revalidation() {
    let f = fixture(
        "closed-repository",
        empty(),
        vec![package(A, "1.0.0", json!({}))],
        None,
    )
    .await;
    let access = AccessContext::local_owner();
    let plan = f
        .packages
        .plan(&access, install(&f, A, "1.0.0"))
        .await
        .expect("owned VPM regression fixture succeeds");
    f.settings(|settings| settings.hidden_repository_ids = vec![f.repository.to_string()])
        .await;
    let page = f.summary().await;
    assert!(page.catalog_complete);
    assert!(matches!(page.page,CandidatePageItems::Summary{items} if items.is_empty()));
    let mut selected = install(&f, A, "1.0.0");
    selected.source_selector = Some(PackageSourceSelector::Repository {
        repository_id: f.repository.to_string(),
    });
    assert_eq!(
        f.packages
            .plan(&access, selected)
            .await
            .expect_err("the audited invalid request must be rejected")
            .code(),
        M4ErrorCode::PackageNotFound
    );
    assert_eq!(
        f.packages
            .apply_plan(
                &access,
                plan.plan_id,
                f.project.revision,
                key("hidden-apply")
            )
            .await
            .expect_err("the audited invalid request must be rejected")
            .code(),
        M4ErrorCode::PlanStale
    );
    assert!(!f.root.join("Packages").join(A).exists());
    f.settings(|settings| settings.hidden_repository_ids.clear())
        .await;
    assert_eq!(apply(&f, plan).await.state, OperationState::Succeeded);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn hidden_unindexed_repository_does_not_block_visible_queries_or_installation() {
    let f = fixture(
        "hidden-unindexed",
        empty(),
        vec![package(A, "1.0.0", json!({}))],
        None,
    )
    .await;
    let index = f.directory.join("unindexed.json");
    let mut manifest = package(B, "1.0.0", json!({}));
    manifest["displayName"] = json!("Unindexed");
    manifest["author"] = json!({"name":"Fixture"});
    manifest["url"] = json!("https://network-must-not-be-used.invalid/not-used.zip");
    fs::write(
        &index,
        serde_json::to_vec(
            &json!({"id":"unindexed.audit","packages":{B:{"versions":{"1.0.0":manifest}}}}),
        )
        .expect("owned VPM regression fixture succeeds"),
    )
    .expect("owned VPM regression fixture succeeds");
    let RepositoryReadOutcome::Fresh(raw) = f
        .reader
        .inspect_repository(
            RepositorySource::Local {
                path: index.to_string_lossy().into_owned(),
            },
            None,
        )
        .await
        .expect("owned VPM regression fixture succeeds")
    else {
        panic!("fresh fixture index")
    };
    let other = RepositoryId::new();
    f.store
        .ensure_builtin_repositories(PrincipalId::local_owner(), vec![(other, raw)], 1)
        .await
        .expect("owned VPM regression fixture succeeds");
    assert!(!f.summary().await.catalog_complete);
    f.settings(|settings| settings.hidden_repository_ids = vec![other.to_string()])
        .await;
    assert!(f.summary().await.catalog_complete);
    let plan = f
        .packages
        .plan(&AccessContext::local_owner(), install(&f, A, "1.0.0"))
        .await
        .expect("owned VPM regression fixture succeeds");
    assert_eq!(apply(&f, plan).await.state, OperationState::Succeeded);
}

async fn enroll_local(f: &Fixture, manifest: Value) {
    let source = f.directory.join("LocalSource");
    fs::create_dir(&source).expect("owned VPM regression fixture succeeds");
    fs::write(
        source.join("package.json"),
        serde_json::to_vec(&manifest).expect("owned VPM regression fixture succeeds"),
    )
    .expect("owned VPM regression fixture succeeds");
    fs::write(source.join("content.txt"), b"owned local source")
        .expect("owned VPM regression fixture succeeds");
    let engine = alcomd_vpm::UserPackageEngine::new(
        f.directory.join("cache"),
        f.directory.join("user-staging"),
    )
    .expect("owned VPM regression fixture succeeds");
    let snapshot = engine
        .snapshot(source.to_string_lossy().into_owned())
        .await
        .expect("owned VPM regression fixture succeeds");
    f.store
        .enroll_user_package(PrincipalId::local_owner(), snapshot, key("enroll-local"), 1)
        .await
        .expect("owned VPM regression fixture succeeds");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn closed_local_source_is_excluded_and_invalidates_a_prepared_plan() {
    let f = fixture("closed-local", empty(), vec![], None).await;
    enroll_local(&f, package(A, "1.0.0", json!({}))).await;
    let plan = f
        .packages
        .plan(&AccessContext::local_owner(), install(&f, A, "1.0.0"))
        .await
        .expect("owned VPM regression fixture succeeds");
    f.settings(|settings| settings.hide_local_user_packages = true)
        .await;
    assert!(
        matches!(f.summary().await.page,CandidatePageItems::Summary{items} if items.is_empty())
    );
    assert_eq!(
        f.packages
            .plan(&AccessContext::local_owner(), install(&f, A, "1.0.0"))
            .await
            .expect_err("the audited invalid request must be rejected")
            .code(),
        M4ErrorCode::PackageNotFound
    );
    assert_eq!(
        f.packages
            .apply_plan(
                &AccessContext::local_owner(),
                plan.plan_id,
                f.project.revision,
                key("closed-local-apply")
            )
            .await
            .expect_err("the audited invalid request must be rejected")
            .code(),
        M4ErrorCode::PlanStale
    );
    f.settings(|settings| settings.hide_local_user_packages = false)
        .await;
    assert_eq!(apply(&f, plan).await.state, OperationState::Succeeded);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn closing_prereleases_overrides_request_opt_in_and_invalidates_a_frozen_plan() {
    let f = fixture(
        "closed-prerelease",
        empty(),
        vec![package(A, "1.0.0-beta.1", json!({}))],
        None,
    )
    .await;
    f.settings(|settings| settings.show_prerelease = true).await;
    let mut request = install(&f, A, "1.0.0-beta.1");
    request.include_prerelease = true;
    let plan = f
        .packages
        .plan(&AccessContext::local_owner(), request.clone())
        .await
        .expect("owned VPM regression fixture succeeds");
    f.settings(|settings| settings.show_prerelease = false)
        .await;
    assert_eq!(
        f.packages
            .plan(&AccessContext::local_owner(), request)
            .await
            .expect_err("the audited invalid request must be rejected")
            .code(),
        M4ErrorCode::PackageNotFound
    );
    assert_eq!(
        f.packages
            .apply_plan(
                &AccessContext::local_owner(),
                plan.plan_id,
                f.project.revision,
                key("closed-prerelease-apply")
            )
            .await
            .expect_err("the audited invalid request must be rejected")
            .code(),
        M4ErrorCode::PlanStale
    );
    assert!(
        matches!(f.summary().await.page,CandidatePageItems::Summary{items} if items.is_empty())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn withdrawn_repository_and_user_dependencies_are_never_automatically_selected() {
    for user in [false, true] {
        for withdrawn in [json!(true), json!("withdrawn for regression")] {
            let mut dependency = package(A, "1.0.0", json!({}));
            dependency["vrc-get"] = json!({"yanked":withdrawn});
            let root = package(D, "1.0.0", json!({A:"^1.0.0"}));
            let f = fixture(
                "withdrawn-dependency",
                empty(),
                if user {
                    vec![root]
                } else {
                    vec![root, dependency.clone()]
                },
                None,
            )
            .await;
            if user {
                enroll_local(&f, dependency).await;
            }
            assert_eq!(
                f.packages
                    .plan(&AccessContext::local_owner(), install(&f, D, "1.0.0"))
                    .await
                    .expect_err("the audited invalid request must be rejected")
                    .code(),
                M4ErrorCode::PackageVersionYanked
            );
            assert!(!f.root.join("Packages").join(A).exists());
            assert_eq!(f.manifest(), empty());
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn batch_install_and_reinstall_keep_unrelated_packages_and_need_only_selected_sources() {
    for reinstall in [false, true] {
        let mut graph = json!({"dependencies":{A:{"version":"1.0.0"}},"locked":{A:{"version":"1.0.0","dependencies":{},"custom":"unknown-source"},C:{"version":"1.0.0","dependencies":{},"custom":"orphan"}}});
        if reinstall {
            graph["dependencies"][D] = json!({"version":"1.0.0"});
            graph["locked"][D] = json!({"version":"1.0.0","dependencies":{},"custom":"restore"});
        }
        let f = fixture(
            "batch-selected",
            graph.clone(),
            vec![package(D, "1.0.0", json!({}))],
            None,
        )
        .await;
        let before = fs::read(f.root.join("Packages/vpm-manifest.json"))
            .expect("owned VPM regression fixture succeeds");
        let intent = if reinstall {
            PackageBulkIntent::Reinstall {
                package_id: D.into(),
                source: None,
            }
        } else {
            install_intent(D, "1.0.0")
        };
        let plan = f
            .packages
            .plan(&AccessContext::local_owner(), bulk(&f, vec![intent]))
            .await
            .expect("owned VPM regression fixture succeeds");
        assert_eq!(plan.change_set.mutations.len(), 1);
        assert_eq!(plan.change_set.mutations[0].package_id, D);
        assert_eq!(plan.source_set.len(), 1);
        assert_eq!(apply(&f, plan).await.state, OperationState::Succeeded);
        assert_eq!(f.manifest()["locked"][A], graph["locked"][A]);
        assert_eq!(f.manifest()["locked"][C], graph["locked"][C]);
        assert!(
            f.root
                .join("Packages")
                .join(C)
                .join("sentinel.txt")
                .is_file()
        );
        if reinstall {
            assert_eq!(
                fs::read(f.root.join("Packages/vpm-manifest.json"))
                    .expect("owned VPM regression fixture succeeds"),
                before
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn single_and_bulk_upgrade_clean_only_unused_old_dependencies() {
    for bulk_action in [false, true] {
        for protected in [false, true] {
            let graph = json!({"dependencies":{A:{"version":"1.0.0"}},"locked":{A:{"version":"1.0.0","dependencies":{B:"1.0.0"}},B:{"version":"1.0.0","dependencies":{},"custom":"shared"},C:{"version":"1.0.0","dependencies":{}}}});
            let f = fixture(
                "upgrade-cleanup",
                graph.clone(),
                vec![package(A, "2.0.0", json!({}))],
                None,
            )
            .await;
            if protected {
                seed(&f.root, &package(L, "1.0.0", json!({B:"1.0.0"})));
            }
            let request = if bulk_action {
                bulk(
                    &f,
                    vec![PackageBulkIntent::Upgrade {
                        package_id: A.into(),
                        version_range: Some("=2.0.0".into()),
                        source: None,
                        include_prerelease: false,
                    }],
                )
            } else {
                let mut r = request(&f, PlanAction::Upgrade, Some(A));
                r.version_range = Some("=2.0.0".into());
                r
            };
            let plan = f
                .packages
                .plan(&AccessContext::local_owner(), request)
                .await
                .expect("owned VPM regression fixture succeeds");
            assert_eq!(
                plan.change_set
                    .mutations
                    .iter()
                    .any(|m| m.package_id == B && m.kind == PackageMutationKind::Remove),
                !protected
            );
            assert!(!plan.change_set.mutations.iter().any(|m| m.package_id == C));
            assert_eq!(apply(&f, plan).await.state, OperationState::Succeeded);
            assert_eq!(f.manifest()["locked"][C], graph["locked"][C]);
            assert_eq!(f.root.join("Packages").join(B).exists(), protected);
            if protected {
                assert_eq!(f.manifest()["locked"][B], graph["locked"][B]);
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reinstall_freezes_actual_directory_presence_and_restores_without_rewriting_the_manifest() {
    for batch in [false, true] {
        for missing in [false, true] {
            let graph = json!({"dependencies":{A:{"version":"1.0.0"}},"locked":{A:{"version":"1.0.0","dependencies":{},"custom":"preserved"}}});
            let f = fixture(
                "reinstall-presence",
                graph,
                vec![package(A, "1.0.0", json!({}))],
                missing.then_some(A),
            )
            .await;
            let before = fs::read(f.root.join("Packages/vpm-manifest.json"))
                .expect("owned VPM regression fixture succeeds");
            let r = if batch {
                bulk(
                    &f,
                    vec![PackageBulkIntent::Reinstall {
                        package_id: A.into(),
                        source: None,
                    }],
                )
            } else {
                let mut r = request(&f, PlanAction::Reinstall, None);
                r.reinstall_selection = Some(ReinstallSelection::Packages(vec![A.into()]));
                r
            };
            let plan = f
                .packages
                .plan(&AccessContext::local_owner(), r)
                .await
                .expect("owned VPM regression fixture succeeds");
            assert_eq!(
                plan.change_set.mutations[0].kind,
                if missing {
                    PackageMutationKind::Install
                } else {
                    PackageMutationKind::Replace
                }
            );
            let path = f.root.join("Packages").join(A);
            let held = f.root.join("held-directory");
            if missing {
                seed(&f.root, &package(A, "9.0.0", json!({})));
            } else {
                fs::rename(&path, &held).expect("owned VPM regression fixture succeeds");
            }
            assert_eq!(
                f.packages
                    .apply_plan(
                        &AccessContext::local_owner(),
                        plan.plan_id,
                        f.project.revision,
                        key("presence-changed")
                    )
                    .await
                    .expect_err("the audited invalid request must be rejected")
                    .code(),
                M4ErrorCode::PlanStale
            );
            if missing {
                fs::rename(&path, &held).expect("owned VPM regression fixture succeeds");
            } else {
                fs::rename(&held, &path).expect("owned VPM regression fixture succeeds");
            }
            assert_eq!(apply(&f, plan).await.state, OperationState::Succeeded);
            assert_eq!(
                fs::read(f.root.join("Packages/vpm-manifest.json"))
                    .expect("owned VPM regression fixture succeeds"),
                before
            );
            assert_eq!(
                serde_json::from_slice::<Value>(
                    &fs::read(path.join("package.json"))
                        .expect("owned VPM regression fixture succeeds")
                )
                .expect("owned VPM regression fixture succeeds")["version"],
                "1.0.0"
            );
            if missing {
                assert_eq!(
                    serde_json::from_slice::<Value>(
                        &fs::read(held.join("package.json"))
                            .expect("owned VPM regression fixture succeeds")
                    )
                    .expect("owned VPM regression fixture succeeds")["version"],
                    "9.0.0"
                );
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn resolve_reuses_unlocked_providers_and_restores_only_missing_dependencies() {
    for missing_root in [false, true] {
        let f = fixture(
            "local-provider",
            empty(),
            if missing_root {
                vec![package(D, "1.0.0", json!({B:"1.0.0"}))]
            } else {
                vec![]
            },
            None,
        )
        .await;
        seed(&f.root, &package(B, "1.0.0", json!({})));
        seed(
            &f.root,
            &package(
                L,
                "1.0.0",
                if missing_root {
                    json!({D:"1.0.0"})
                } else {
                    json!({B:"1.0.0"})
                },
            ),
        );
        let original = fs::read(f.root.join("Packages").join(B).join("package.json"))
            .expect("owned VPM regression fixture succeeds");
        let plan = f
            .packages
            .plan(
                &AccessContext::local_owner(),
                request(&f, PlanAction::Resolve, None),
            )
            .await
            .expect("owned VPM regression fixture succeeds");
        assert_eq!(plan.change_set.mutations.len(), usize::from(missing_root));
        assert!(!plan.change_set.mutations.iter().any(|m| m.package_id == B));
        if missing_root {
            assert_eq!(apply(&f, plan).await.state, OperationState::Succeeded);
        }
        assert!(f.manifest()["locked"].get(B).is_none());
        assert_eq!(
            fs::read(f.root.join("Packages").join(B).join("package.json"))
                .expect("owned VPM regression fixture succeeds"),
            original
        );
    }
    let f = fixture("missing-dependency-error", empty(), vec![], None).await;
    seed(&f.root, &package(L, "1.0.0", json!({B:"1.0.0"})));
    assert_eq!(
        f.packages
            .plan(
                &AccessContext::local_owner(),
                request(&f, PlanAction::Resolve, None)
            )
            .await
            .expect_err("the audited invalid request must be rejected")
            .code(),
        M4ErrorCode::PackageNotFound
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn fixed_restore_accepts_locked_yanked_unity_versions_but_not_new_install_dependencies() {
    for action in [PlanAction::Resolve, PlanAction::Reinstall, PlanAction::Bulk] {
        let graph = json!({"dependencies":{A:{"version":"1.0.0"}},"locked":{A:{"version":"1.0.0","dependencies":{},"note":"frozen"}}});
        let mut restored = package(A, "1.0.0", json!({B:"1.0.0"}));
        restored["vrc-get"] = json!({"yanked":"old release"});
        restored["unity"] = json!("2023.1");
        let f = fixture(
            "fixed-restore",
            graph.clone(),
            vec![restored.clone(), package(D, "1.0.0", json!({A:"1.0.0"}))],
            Some(A),
        )
        .await;
        assert_eq!(
            f.packages
                .plan(&AccessContext::local_owner(), install(&f, D, "1.0.0"))
                .await
                .expect_err("the audited invalid request must be rejected")
                .code(),
            M4ErrorCode::PackageVersionYanked
        );
        let mut r = request(&f, action, None);
        if action == PlanAction::Reinstall {
            r.reinstall_selection = Some(ReinstallSelection::Packages(vec![A.into()]));
        }
        if action == PlanAction::Bulk {
            r.bulk_intents = vec![PackageBulkIntent::Reinstall {
                package_id: A.into(),
                source: None,
            }];
        }
        let plan = f
            .packages
            .plan(&AccessContext::local_owner(), r)
            .await
            .expect("owned VPM regression fixture succeeds");
        assert_eq!(plan.change_set.mutations.len(), 1);
        assert_eq!(plan.change_set.mutations[0].package_id, A);
        assert!(plan.change_set.unity_compatibility_overrides.is_empty());
        assert_eq!(apply(&f, plan).await.state, OperationState::Succeeded);
        assert_eq!(f.manifest(), graph);
        assert!(!f.root.join("Packages").join(B).exists());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn restoring_a_locked_version_keeps_its_exact_build_identity() {
    const VERSION: &str = "1.0.0+locked-build";
    for action in [PlanAction::Resolve, PlanAction::Reinstall, PlanAction::Bulk] {
        let graph = json!({"dependencies":{A:{"version":VERSION}},"locked":{A:{"version":VERSION,"dependencies":{}}}});
        let f = fixture(
            "exact-build-restore",
            graph.clone(),
            vec![
                package(A, VERSION, json!({})),
                package(A, "1.0.0+other-build", json!({})),
            ],
            Some(A),
        )
        .await;
        let mut r = request(&f, action, None);
        if action == PlanAction::Reinstall {
            r.reinstall_selection = Some(ReinstallSelection::Packages(vec![A.into()]));
        }
        if action == PlanAction::Bulk {
            r.bulk_intents = vec![PackageBulkIntent::Reinstall {
                package_id: A.into(),
                source: None,
            }];
        }
        let plan = f
            .packages
            .plan(&AccessContext::local_owner(), r)
            .await
            .expect("only the exact locked artifact is selected");
        assert_eq!(plan.change_set.mutations.len(), 1);
        assert_eq!(
            plan.change_set.mutations[0].to_version.as_deref(),
            Some(VERSION)
        );
        assert!(plan.source_set.iter().all(|pin| pin.version() == VERSION));
        assert_eq!(apply(&f, plan).await.state, OperationState::Succeeded);
        assert_eq!(f.manifest(), graph);
        let installed: Value = serde_json::from_slice(
            &fs::read(f.root.join("Packages").join(A).join("package.json"))
                .expect("installed metadata"),
        )
        .expect("JSON");
        assert_eq!(installed["version"], VERSION);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn resolve_does_not_add_a_package_against_an_incompatible_local_provider() {
    let f = fixture(
        "incompatible-local-provider",
        empty(),
        vec![package(D, "1.0.0", json!({B:"^2.0.0"}))],
        None,
    )
    .await;
    seed(&f.root, &package(B, "1.0.0", json!({})));
    seed(&f.root, &package(L, "1.0.0", json!({D:"1.0.0"})));
    assert_eq!(
        f.packages
            .plan(
                &AccessContext::local_owner(),
                request(&f, PlanAction::Resolve, None)
            )
            .await
            .expect_err("a newly selected package must satisfy its actual local provider")
            .code(),
        M4ErrorCode::PackageDependencyConflict
    );
    assert_eq!(f.manifest(), empty());
    assert!(!f.root.join("Packages").join(D).exists());
}
