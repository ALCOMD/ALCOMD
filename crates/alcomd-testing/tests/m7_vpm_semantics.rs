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

#[tokio::test]
async fn repository_configuration_round_trip_restores_names_switches_and_remaps_sources() {
    let source = fixture("repository-configuration-source", empty(), Vec::new(), None).await;
    let access = AccessContext::local_owner();
    let registry = M3Application::new(source.store.clone(), source.reader.clone());
    let record = registry
        .get_repository(&access, source.repository)
        .await
        .expect("source repository");
    registry
        .set_repository_display_name(
            &access,
            source.repository,
            record.revision,
            Some("Renamed configuration source".into()),
            key("rename-export-source"),
        )
        .await
        .expect("rename source");
    source
        .settings(|settings| {
            settings.hidden_repository_ids = vec![source.repository.to_string()];
            settings.hidden_repository_packages = vec![ConfigRepositoryPackages {
                repository_id: source.repository.to_string(),
                package_ids: vec!["com.example.future".into()],
            }];
        })
        .await;
    let data = source.directory.join("configuration-data");
    fs::create_dir(&data).expect("isolated Core data");
    let lists = RepositoryListApplication::new(
        source.store.clone(),
        alcomd_vpm::RepositoryListFileAdapter::new(data),
    )
    .with_settings(source.config.clone());
    let preview = lists
        .preview_format(&access, RepositoryListFormat::Native)
        .await
        .expect("native preview");
    assert_eq!(preview.exportable_count, 1, "local sources are included");
    assert!(preview.excluded.is_empty());
    let output = source.directory.join("repositories.alcomdrepositories");
    lists
        .export_format(
            &access,
            output.to_string_lossy().into_owned(),
            preview.fingerprint,
            RepositoryListFormat::Native,
        )
        .await
        .expect("native export");
    let inspection = lists
        .inspect(&access, output.to_string_lossy().into_owned())
        .await
        .expect("inspect configuration");
    assert_eq!(inspection.format, RepositoryListFormat::Native);
    assert!(
        inspection
            .entries
            .iter()
            .all(|entry| entry.credentials.is_none()),
        "RPC inspection never carries credentials"
    );
    let target = fixture("repository-configuration-target", empty(), Vec::new(), None).await;
    target
        .settings(|settings| {
            settings.hidden_repository_packages = vec![ConfigRepositoryPackages {
                repository_id: target.repository.to_string(),
                package_ids: vec!["com.example.keep".into()],
            }]
        })
        .await;
    let target_data = target.directory.join("configuration-data");
    fs::create_dir(&target_data).expect("target Core data");
    let target_lists = RepositoryListApplication::new(
        target.store.clone(),
        alcomd_vpm::RepositoryListFileAdapter::new(target_data),
    )
    .with_settings(target.config.clone());
    let target_registry = M3Application::new(target.store.clone(), target.reader.clone());
    let path = output.to_string_lossy().into_owned();
    let outcomes = target_lists
        .import(
            &access,
            &target_registry,
            path.clone(),
            inspection.fingerprint,
            key("import-configuration"),
        )
        .await
        .expect("apply configuration");
    assert_eq!(outcomes, vec![(1, RepositoryBatchItem::Added)]);
    let snapshot = target
        .store
        .repository_list_snapshot(PrincipalId::local_owner())
        .await
        .expect("target sources");
    let imported = snapshot
        .repositories
        .iter()
        .find(|record| record.repository_id != target.repository.to_string())
        .expect("new source identity");
    assert_ne!(imported.repository_id, source.repository.to_string());
    assert_eq!(
        imported.configuration.display_name.as_deref(),
        Some("Renamed configuration source")
    );
    assert!(
        imported.configuration.priority
            < snapshot
                .repositories
                .iter()
                .find(|record| record.repository_id == target.repository.to_string())
                .expect("retained source")
                .configuration
                .priority
    );
    let settings = target
        .config
        .candidate_settings()
        .await
        .expect("restored switches");
    assert!(
        settings
            .settings
            .packages
            .hidden_repository_ids
            .contains(&imported.repository_id)
    );
    assert!(
        settings
            .settings
            .packages
            .hidden_repository_packages
            .iter()
            .any(|group| group.repository_id == imported.repository_id
                && group.package_ids == ["com.example.future"])
    );
    assert!(
        settings
            .settings
            .packages
            .hidden_repository_packages
            .iter()
            .any(|group| group.repository_id == target.repository.to_string()
                && group.package_ids == ["com.example.keep"])
    );
    let target_id = RepositoryId::parse(&imported.repository_id).expect("new repository ID");
    let current = target_registry
        .get_repository(&access, target_id)
        .await
        .expect("current source");
    target_registry
        .set_repository_display_name(
            &access,
            target_id,
            current.revision,
            None,
            key("clear-imported-name"),
        )
        .await
        .expect("clear override");
    target
        .settings(|settings| {
            settings.hidden_repository_ids.clear();
            settings
                .hidden_repository_packages
                .retain(|group| group.repository_id != imported.repository_id);
        })
        .await;
    let inspected = target_lists
        .inspect(&access, path.clone())
        .await
        .expect("preview existing configuration");
    let kept_revision = target_registry
        .get_repository(&access, target_id)
        .await
        .expect("before add-only")
        .revision;
    let kept_settings = target
        .config
        .candidate_settings()
        .await
        .expect("before add-only settings");
    let skipped = target_lists
        .import_with_options(
            &access,
            &target_registry,
            path.clone(),
            inspected.fingerprint.clone(),
            key("keep-existing-configuration"),
            false,
        )
        .await
        .expect("add-only skips existing source");
    assert_eq!(skipped, vec![(1, RepositoryBatchItem::Existing)]);
    assert_eq!(
        target_registry
            .get_repository(&access, target_id)
            .await
            .expect("kept source")
            .revision,
        kept_revision
    );
    assert_eq!(
        target
            .config
            .candidate_settings()
            .await
            .expect("kept settings"),
        kept_settings
    );
    let results = target_lists
        .import(
            &access,
            &target_registry,
            path.clone(),
            inspected.fingerprint,
            key("restore-existing-configuration"),
        )
        .await
        .expect("restore existing configuration");
    assert_eq!(results, vec![(1, RepositoryBatchItem::Updated)]);
    let before = target_registry
        .get_repository(&access, target_id)
        .await
        .expect("restored source");
    let settings_before = target.config.candidate_settings().await.expect("settings");
    let inspected = target_lists
        .inspect(&access, path.clone())
        .await
        .expect("repeat preview");
    target_lists
        .import(
            &access,
            &target_registry,
            path,
            inspected.fingerprint,
            key("repeat-configuration"),
        )
        .await
        .expect("same configuration is harmless");
    assert_eq!(
        target_registry
            .get_repository(&access, target_id)
            .await
            .expect("unchanged source")
            .revision,
        before.revision
    );
    assert_eq!(
        target
            .config
            .candidate_settings()
            .await
            .expect("unchanged settings"),
        settings_before
    );
    let add_only = fixture(
        "repository-configuration-add-only",
        empty(),
        Vec::new(),
        None,
    )
    .await;
    let original = add_only
        .store
        .repository_list_snapshot(PrincipalId::local_owner())
        .await
        .expect("existing metadata")
        .repositories
        .remove(0);
    let data = add_only.directory.join("configuration-data");
    fs::create_dir(&data).expect("add-only data");
    let lists = RepositoryListApplication::new(
        add_only.store.clone(),
        alcomd_vpm::RepositoryListFileAdapter::new(data),
    )
    .with_settings(add_only.config.clone());
    let registry = M3Application::new(add_only.store.clone(), add_only.reader.clone());
    let path = output.to_string_lossy().into_owned();
    let inspected = lists
        .inspect(&access, path.clone())
        .await
        .expect("add-only preview");
    assert_eq!(
        lists
            .import_with_options(
                &access,
                &registry,
                path,
                inspected.fingerprint,
                key("add-only-new-source"),
                false
            )
            .await
            .expect("add-only import"),
        vec![(1, RepositoryBatchItem::Added)]
    );
    let after = add_only
        .store
        .repository_list_snapshot(PrincipalId::local_owner())
        .await
        .expect("add-only result");
    let retained = after
        .repositories
        .iter()
        .find(|record| record.repository_id == original.repository_id)
        .expect("retained");
    assert_eq!(retained.revision, original.revision);
    assert_eq!(retained.configuration, original.configuration);
    let added = after
        .repositories
        .iter()
        .find(|record| record.repository_id != original.repository_id)
        .expect("added");
    assert!(
        added.configuration.priority > retained.configuration.priority,
        "new sources append when overwrite is disabled"
    );
    assert_eq!(
        added.configuration.display_name.as_deref(),
        Some("Renamed configuration source")
    );
}

#[tokio::test]
async fn repository_configuration_permissions_early_stop_and_preview_changes_do_not_write() {
    let fixture = fixture("repository-configuration-guards", empty(), Vec::new(), None).await;
    let access = AccessContext::local_owner();
    let registry = M3Application::new(fixture.store.clone(), fixture.reader.clone());
    let data = fixture.directory.join("configuration-data");
    fs::create_dir(&data).expect("isolated Core data");
    let lists = RepositoryListApplication::new(
        fixture.store.clone(),
        alcomd_vpm::RepositoryListFileAdapter::new(data),
    )
    .with_settings(fixture.config.clone());
    let output = fixture.directory.join("repositories.alcomdrepositories");
    let path = output.to_string_lossy().into_owned();
    let preview = lists
        .preview_format(&access, RepositoryListFormat::Native)
        .await
        .expect("preview");
    lists
        .export_format(
            &access,
            path.clone(),
            preview.fingerprint.clone(),
            RepositoryListFormat::Native,
        )
        .await
        .expect("export");
    let bytes = fs::read(&output).expect("exported bytes");
    assert_eq!(
        lists
            .export_format(
                &access,
                path.clone(),
                preview.fingerprint,
                RepositoryListFormat::Native
            )
            .await
            .err(),
        Some(RepositoryListError("repository_list_target_exists"))
    );
    assert_eq!(fs::read(&output).expect("original export"), bytes);
    let inspected = lists.inspect(&access, path.clone()).await.expect("inspect");
    let settings_before = fixture.config.candidate_settings().await.expect("settings");
    let repository_before = registry
        .get_repository(&access, fixture.repository)
        .await
        .expect("source");
    let limited = AccessContext::new(
        PrincipalId::local_owner(),
        [
            Permission::RepositoriesRead,
            Permission::RepositoriesManage,
            Permission::SettingsRead,
        ],
    );
    assert_eq!(
        lists
            .import(
                &limited,
                &registry,
                path.clone(),
                inspected.fingerprint.clone(),
                key("denied-configuration")
            )
            .await
            .err(),
        Some(RepositoryListError("permission_denied"))
    );
    let stopped = key("stopped-configuration");
    lists
        .cancel_registration(&access, &stopped)
        .expect("early stop");
    assert_eq!(
        lists
            .import(
                &access,
                &registry,
                path.clone(),
                inspected.fingerprint.clone(),
                stopped
            )
            .await
            .expect("stopped"),
        vec![(1, RepositoryBatchItem::Unprocessed)]
    );
    let mut changed: Value = serde_json::from_slice(&bytes).expect("export JSON");
    changed["repositories"][0]["configuration"]["displayName"] = json!("Changed after preview");
    fs::write(
        &output,
        serde_json::to_vec(&changed).expect("changed document"),
    )
    .expect("changed fixture");
    assert_eq!(
        lists
            .import(
                &access,
                &registry,
                path,
                inspected.fingerprint,
                key("changed-configuration")
            )
            .await
            .err(),
        Some(RepositoryListError("repository_list_changed"))
    );
    let stale_output = fixture.directory.join("stale.alcomdrepositories");
    let preview = lists
        .preview_format(&access, RepositoryListFormat::Native)
        .await
        .expect("before settings change");
    fixture
        .settings(|settings| settings.hidden_repository_ids = vec![fixture.repository.to_string()])
        .await;
    assert_eq!(
        lists
            .export_format(
                &access,
                stale_output.to_string_lossy().into_owned(),
                preview.fingerprint,
                RepositoryListFormat::Native
            )
            .await
            .err(),
        Some(RepositoryListError("repository_list_changed"))
    );
    assert!(!stale_output.exists());
    assert_eq!(
        registry
            .get_repository(&access, fixture.repository)
            .await
            .expect("unchanged source")
            .revision,
        repository_before.revision
    );
    let current = fixture
        .config
        .candidate_settings()
        .await
        .expect("current settings");
    assert_eq!(
        current.revision,
        settings_before.revision + 1,
        "only the deliberate fixture change was saved"
    );
    let metadata = fixture
        .store
        .repository_list_snapshot(PrincipalId::local_owner())
        .await
        .expect("metadata before refresh")
        .repositories
        .remove(0);
    registry
        .refresh_repository(
            &access,
            fixture.repository,
            repository_before.revision,
            key("refresh-before-config-metadata"),
        )
        .await
        .expect("ordinary index refresh");
    let change = RepositoryListMetadata {
        repository_id: metadata.repository_id,
        expected_display_name: metadata.configuration.display_name,
        expected_priority: metadata.configuration.priority,
        display_name: Some("Configuration after index refresh".into()),
        priority: metadata.configuration.priority,
    };
    fixture
        .store
        .apply_repository_list_metadata(PrincipalId::local_owner(), vec![change.clone()], 1)
        .await
        .expect("index revision is irrelevant to configuration");
    let mut conflict = change;
    conflict.display_name = Some("Conflicting configuration".into());
    assert_eq!(
        fixture
            .store
            .apply_repository_list_metadata(PrincipalId::local_owner(), vec![conflict], 2)
            .await
            .expect_err("actual name change conflicts")
            .code(),
        M3ErrorCode::RevisionConflict
    );
}

#[tokio::test]
async fn automatic_ndmf_downgrade_uses_repository_priority_and_checks_dependents() {
    const MA: &str = "nadena.dev.modular-avatar";
    const NDMF: &str = "nadena.dev.ndmf";
    for conflict in [false, true] {
        let deps = json!({NDMF: ">=1.14.7 <2.0.0-a"});
        let f = fixture("ndmf-automatic-source",
            json!({"dependencies":{MA:{"version":"1.18.7"},NDMF:{"version":"1.14.8"}},
                "locked":{MA:{"version":"1.18.7","dependencies":deps},NDMF:{"version":"1.14.8","dependencies":{}}}}),
            vec![package(MA,"1.18.7",deps),package(NDMF,"1.14.7",json!({})),package(NDMF,"1.14.6",json!({}))],None).await;
        let mut mirror: Value =
            serde_json::from_slice(&fs::read(f.directory.join("index.json")).expect("index"))
                .expect("index JSON");
        mirror["id"] = json!("owned.ndmf.mirror");
        let mirror_path = f.directory.join("mirror.json");
        fs::write(
            &mirror_path,
            serde_json::to_vec(&mirror).expect("mirror JSON"),
        )
        .expect("mirror index");
        let registry = M3Application::new(f.store.clone(), f.reader.clone());
        let repository = registry
            .register_repository(
                &AccessContext::local_owner(),
                RepositorySource::Local {
                    path: mirror_path.to_string_lossy().into_owned(),
                },
                key("register-mirror"),
            )
            .await
            .expect("register mirror")
            .value;
        registry
            .refresh_repository(
                &AccessContext::local_owner(),
                repository.repository_id,
                repository.revision,
                key("refresh-mirror"),
            )
            .await
            .expect("refresh mirror");
        let mut intent = request(&f, PlanAction::Downgrade, Some(NDMF));
        intent.version_range = Some(format!("={}", if conflict { "1.14.6" } else { "1.14.7" }));
        let planned = f.packages.plan(&AccessContext::local_owner(), intent).await;
        if conflict {
            assert_eq!(
                planned
                    .expect_err("MA requires the higher NDMF version")
                    .code(),
                M4ErrorCode::PackageDependencyConflict
            );
            assert_eq!(f.manifest()["locked"][NDMF]["version"], "1.14.8");
        } else {
            let plan = planned.expect("Core chooses a source for the exact version");
            assert_eq!(plan.change_set.mutations.len(), 1);
            assert!(plan.source_set.iter().any(|source| matches!(source,PackageSourcePin::Repository(pin) if pin.package_id==NDMF && pin.repository_id==f.repository.to_string())));
            let operation = apply(&f, plan).await;
            assert_eq!(operation.state, OperationState::Succeeded, "{operation:?}");
            assert_eq!(f.manifest()["locked"][NDMF]["version"], "1.14.7");
            assert_eq!(f.manifest()["locked"][MA]["version"], "1.18.7");
        }
    }
}

#[tokio::test]
async fn downgrade_accepts_normalized_dependency_ranges_from_the_archive() {
    const MA: &str = "nadena.dev.modular-avatar";
    const NDMF: &str = "nadena.dev.ndmf";
    let deps = json!({NDMF: ">=1.14.7 <2.0.0-a"});
    let mut older = package(MA, "1.18.6", deps.clone());
    older["unity"] = json!("2022.3");
    let f = fixture(
        "ma-downgrade-range-order",
        json!({"dependencies":{MA:{"version":"1.18.7"}},
            "locked":{MA:{"version":"1.18.7","dependencies":deps},
                NDMF:{"version":"1.14.8","dependencies":{}}}}),
        vec![older, package(NDMF, "1.14.8", json!({}))],
        None,
    )
    .await;
    let ndmf = f.root.join("Packages").join(NDMF).join("package.json");
    let ndmf_before = fs::read(&ndmf).expect("installed NDMF metadata");
    let mut intent = request(&f, PlanAction::Downgrade, Some(MA));
    intent.version_range = Some("=1.18.6".to_owned());
    let plan = f
        .packages
        .plan(&AccessContext::local_owner(), intent)
        .await
        .expect("MA downgrade plan");
    assert_eq!(plan.change_set.mutations.len(), 1);
    assert_eq!(
        plan.change_set.mutations[0].from_version.as_deref(),
        Some("1.18.7")
    );
    assert_eq!(
        plan.change_set.mutations[0].to_version.as_deref(),
        Some("1.18.6")
    );
    let operation = apply(&f, plan).await;
    assert_eq!(operation.state, OperationState::Succeeded, "{operation:?}");
    assert_eq!(f.manifest()["locked"][MA]["version"], "1.18.6");
    assert_eq!(f.manifest()["locked"][NDMF]["version"], "1.14.8");
    assert_eq!(fs::read(ndmf).expect("retained NDMF metadata"), ndmf_before);
}

#[tokio::test]
async fn package_actions_use_current_files_without_refreshing_registered_observations() {
    for action in [
        PlanAction::Install,
        PlanAction::Upgrade,
        PlanAction::Downgrade,
        PlanAction::Remove,
        PlanAction::Resolve,
        PlanAction::Reinstall,
        PlanAction::Bulk,
    ] {
        let f = fixture("external-package-edit",
            json!({"dependencies":{A:{"version":"1.0.0"}},"locked":{A:{"version":"1.0.0","dependencies":{}}}}),
            vec![package(A,"1.0.0",json!({})),package(A,"2.0.0",json!({})),
                package(A,"3.0.0",json!({})),package(B,"1.0.0",json!({})),package(D,"1.0.0",json!({}))], None).await;
        let graph = json!({"dependencies":{A:{"version":"2.0.0"},B:{"version":"1.0.0"}},
            "locked":{A:{"version":"2.0.0","dependencies":{}},B:{"version":"1.0.0","dependencies":{}}}});
        fs::write(
            f.root.join("Packages/vpm-manifest.json"),
            serde_json::to_vec(&graph).expect("owned current-file regression fixture"),
        )
        .expect("owned current-file regression fixture");
        seed(&f.root, &package(A, "2.0.0", json!({})));
        seed(&f.root, &package(B, "1.0.0", json!({})));
        if action == PlanAction::Resolve {
            fs::remove_dir_all(f.root.join("Packages").join(A))
                .expect("owned missing-package fixture");
        }
        let target = if action == PlanAction::Install { D } else { A };
        let mut intent = request(&f, action, Some(target));
        match action {
            PlanAction::Downgrade => intent.version_range = Some("=1.0.0".to_owned()),
            PlanAction::Upgrade => intent.version_range = Some("=3.0.0".to_owned()),
            PlanAction::Reinstall => {
                intent.reinstall_selection = Some(ReinstallSelection::Packages(vec![A.to_owned()]))
            }
            PlanAction::Resolve => intent.package_id = None,
            PlanAction::Bulk => {
                intent.package_id = None;
                intent.bulk_intents = vec![PackageBulkIntent::Upgrade {
                    package_id: A.to_owned(),
                    version_range: Some("=3.0.0".to_owned()),
                    source: None,
                    include_prerelease: false,
                }];
            }
            _ => {}
        }
        let plan = f
            .packages
            .plan(&AccessContext::local_owner(), intent)
            .await
            .expect("plan current files");
        let registered = f
            .store
            .get_project(PrincipalId::local_owner(), f.project.project_id)
            .await
            .expect("owned current-file regression fixture");
        assert_eq!(
            registered, f.project,
            "Plan must not refresh registry observations"
        );
        if action != PlanAction::Install && action != PlanAction::Resolve {
            assert_eq!(
                plan.change_set
                    .mutations
                    .iter()
                    .find(|m| m.package_id == A)
                    .expect("owned current-file regression fixture")
                    .from_version
                    .as_deref(),
                Some("2.0.0")
            );
        }
        assert!(!plan.change_set.mutations.iter().any(|m| m.package_id == B));
        let operation = apply(&f, plan).await;
        assert_eq!(
            operation.state,
            OperationState::Succeeded,
            "{action:?}: {operation:?}"
        );
        assert!(
            f.root
                .join("Packages")
                .join(B)
                .join("package.json")
                .exists()
        );
    }
}

#[tokio::test]
async fn current_file_plans_still_reject_changes_after_review() {
    for change in ["vpm", "package", "unity"] {
        let f = fixture("after-review-change",
            json!({"dependencies":{A:{"version":"1.0.0"}},"locked":{A:{"version":"1.0.0","dependencies":{}}}}),
            vec![package(A,"1.0.0",json!({}))], None).await;
        let plan = f
            .packages
            .plan(
                &AccessContext::local_owner(),
                request(&f, PlanAction::Remove, Some(A)),
            )
            .await
            .expect("owned current-file regression fixture");
        match change {
            "vpm" => fs::write(
                f.root.join("Packages/vpm-manifest.json"),
                b"{\"dependencies\":{},\"locked\":{}}",
            ),
            "package" => fs::write(
                f.root.join("Packages").join(A).join("package.json"),
                serde_json::to_vec(&package(A, "2.0.0", json!({})))
                    .expect("owned current-file regression fixture"),
            ),
            _ => fs::write(
                f.root.join("ProjectSettings/ProjectVersion.txt"),
                b"m_EditorVersion: 2022.3.23f1\n",
            ),
        }
        .expect("owned current-file regression fixture");
        let error = f
            .packages
            .apply_plan(
                &AccessContext::local_owner(),
                plan.plan_id,
                f.project.revision,
                key("stale-review"),
            )
            .await
            .expect_err("changed review is rejected");
        assert_eq!(error.code(), M4ErrorCode::PlanStale);
        assert_eq!(error.subreason(), Some("project_snapshot_changed"));
        assert!(
            f.root
                .join("Packages")
                .join(A)
                .join("sentinel.txt")
                .exists()
        );
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
                        hidden_repository_packages: Some(value.hidden_repository_packages),
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
async fn closing_a_source_during_confirmed_archive_download_does_not_modify_the_project() {
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
    let (entered_tx, mut entered_rx) = tokio::sync::oneshot::channel();
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
    let plan = f
        .packages
        .plan(&AccessContext::local_owner(), install(&f, A, "1.0.0"))
        .await
        .expect("metadata-only plan");
    assert!(
        matches!(
            entered_rx.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ),
        "planning must not request the archive"
    );
    let close_during_download = async {
        tokio::time::timeout(Duration::from_secs(5), entered_rx)
            .await
            .expect("confirmed Apply reaches download")
            .expect("download signal");
        f.settings(|settings| settings.hidden_repository_ids = vec![f.repository.to_string()])
            .await;
        release_tx.send(()).expect("release preparation");
    };
    let (operation, ()) = tokio::join!(apply(&f, plan), close_during_download);
    assert_eq!(operation.state, OperationState::Failed);
    assert_eq!(operation.error_code.as_deref(), Some("plan_stale"));
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
async fn package_switch_keeps_installed_facts_and_local_removal_but_blocks_dependency_and_bulk_plans()
 {
    let graph = json!({"dependencies":{A:{"version":"1.0.0"}},"locked":{A:{"version":"1.0.0","dependencies":{}}}});
    let f = fixture(
        "hidden-installed-package",
        graph.clone(),
        vec![
            package(A, "1.0.0", json!({})),
            package(A, "2.0.0", json!({})),
            package(D, "1.0.0", json!({A:"^2.0.0"})),
        ],
        None,
    )
    .await;
    f.settings(|settings| {
        settings.hidden_repository_packages = vec![ConfigRepositoryPackages {
            repository_id: f.repository.to_string(),
            package_ids: vec![A.into()],
        }]
    })
    .await;
    let CandidatePageItems::Summary { items } = f.summary().await.page else {
        panic!("summary")
    };
    let row = items
        .iter()
        .find(|item| item.package_id == A)
        .expect("installed fact");
    assert!(matches!(&row.installed, CandidateInstalled::Locked { version } if version == "1.0.0"));
    assert!(row.providers.is_empty());
    assert!(items.iter().any(|item| item.package_id == D));
    let access = AccessContext::local_owner();
    for intent in [
        install(&f, D, "1.0.0"),
        bulk(&f, vec![install_intent(A, "2.0.0")]),
        request(&f, PlanAction::Reinstall, Some(A)),
    ] {
        assert!(f.packages.plan(&access, intent).await.is_err());
        assert_eq!(f.manifest(), graph);
    }
    let plan = f
        .packages
        .plan(&access, request(&f, PlanAction::Remove, Some(A)))
        .await
        .expect("local removal needs no enabled source");
    assert_eq!(apply(&f, plan).await.state, OperationState::Succeeded);
    assert!(!f.root.join("Packages").join(A).exists());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn package_switch_isolates_sources_and_versions_without_hiding_management_or_user_packages() {
    let f = fixture(
        "per-source-package",
        empty(),
        vec![
            package(A, "1.0.0", json!({})),
            package(A, "2.0.0", json!({})),
            package(B, "1.0.0", json!({})),
        ],
        None,
    )
    .await;
    let mut index: Value =
        serde_json::from_slice(&fs::read(f.directory.join("index.json")).expect("index"))
            .expect("JSON");
    index["id"] = json!("independent.audit");
    index["name"] = json!("Independent source");
    index["packages"][A]["versions"]
        .as_object_mut()
        .expect("versions")
        .remove("2.0.0");
    index["packages"]
        .as_object_mut()
        .expect("packages")
        .remove(B);
    let second = f.directory.join("second.json");
    fs::write(&second, serde_json::to_vec(&index).expect("JSON")).expect("file");
    let access = AccessContext::local_owner();
    let registry = M3Application::new(f.store.clone(), f.reader.clone());
    let other = registry
        .register_repository(
            &access,
            RepositorySource::Local {
                path: second.to_string_lossy().into_owned(),
            },
            key("second-source"),
        )
        .await
        .expect("second repository")
        .value;
    f.settings(|settings| {
        settings.hidden_repository_packages = vec![ConfigRepositoryPackages {
            repository_id: f.repository.to_string(),
            package_ids: vec![A.into()],
        }]
    })
    .await;
    let CandidatePageItems::Summary { items } = f.summary().await.page else {
        panic!("summary")
    };
    assert!(items.iter().any(|item| item.package_id == B));
    let row = items
        .iter()
        .find(|item| item.package_id == A)
        .expect("other source retained");
    assert_eq!(row.providers.len(), 1);
    let query = ProjectCandidateApplication::new(
        f.store.clone(),
        ProjectCandidateEngine,
        f.reader.clone(),
        f.config.clone(),
    );
    let versions = query
        .query(
            &access,
            ProjectCandidateRequest {
                project_id: f.project.project_id.to_string(),
                expected_revision: f.project.revision.get(),
                view: CandidateView::Versions {
                    package_id: A.into(),
                    source: None,
                },
                limit: None,
                expected_snapshot: None,
                cursor: None,
            },
        )
        .await
        .expect("versions");
    let CandidatePageItems::Versions { items, .. } = versions.page else {
        panic!("versions")
    };
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].version, "1.0.0");
    let plan = f
        .packages
        .plan(&access, install(&f, A, "1.0.0"))
        .await
        .expect("alternative provider");
    assert!(
        matches!(&plan.source_set[0],PackageSourcePin::Repository(pin) if pin.repository_id==other.repository_id.to_string())
    );
    // Only the explicit management endpoint retains disabled package rows.
    let raw = registry
        .list_repository_packages(&access, f.repository, None, 100)
        .await
        .expect("management catalog");
    assert!(
        raw.packages
            .iter()
            .any(|item| item.package_id == A && item.version == "2.0.0")
    );
    enroll_local(&f, package(A, "3.0.0", json!({}))).await;
    let CandidatePageItems::Summary { items } = f.summary().await.page else {
        panic!("summary")
    };
    assert_eq!(
        items
            .iter()
            .find(|item| item.package_id == A)
            .expect("sources")
            .providers
            .len(),
        2
    );
    let persisted = alcomd_store::FileConfigStore::new(f.directory.join("settings.toml"))
        .initialize()
        .expect("restart preferences");
    assert_eq!(
        persisted.settings.packages.hidden_repository_packages[0].package_ids,
        vec![A]
    );
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
async fn retained_orphan_dependencies_survive_single_and_bulk_changes() {
    for batch in [false, true] {
        for remove in [false, true] {
            let graph = json!({"dependencies":{A:{"version":"1.0.0"}},"locked":{
                A:{"version":"1.0.0","dependencies":{B:"1.0.0"}},
                B:{"version":"1.0.0","dependencies":{D:"1.0.0"},"custom":"shared"},
                C:{"version":"1.0.0","dependencies":{B:"1.0.0"},"custom":"retained orphan"},
                D:{"version":"1.0.0","dependencies":{},"custom":"transitive"}
            }});
            let f = fixture(
                "retained-orphan-dependencies",
                graph.clone(),
                vec![package(A, "2.0.0", json!({}))],
                None,
            )
            .await;
            let action = if remove {
                PlanAction::Remove
            } else {
                PlanAction::Upgrade
            };
            let mut r = request(&f, action, Some(A));
            if !remove {
                r.version_range = Some("=2.0.0".into());
            }
            if batch {
                r = bulk(
                    &f,
                    vec![if remove {
                        PackageBulkIntent::Remove {
                            package_id: A.into(),
                        }
                    } else {
                        PackageBulkIntent::Upgrade {
                            package_id: A.into(),
                            version_range: Some("=2.0.0".into()),
                            source: None,
                            include_prerelease: false,
                        }
                    }],
                );
            }
            let plan = f
                .packages
                .plan(&AccessContext::local_owner(), r)
                .await
                .expect("shared dependencies must not block the selected change");
            assert_eq!(plan.change_set.mutations.len(), 1);
            assert_eq!(plan.change_set.mutations[0].package_id, A);
            assert!(plan.change_set.removal_conflicts.is_empty());
            assert_eq!(apply(&f, plan).await.state, OperationState::Succeeded);
            for id in [B, C, D] {
                assert_eq!(f.manifest()["locked"][id], graph["locked"][id]);
                assert_eq!(
                    fs::read(f.root.join("Packages").join(id).join("sentinel.txt"))
                        .expect("retained dependency content"),
                    b"owned audit fixture"
                );
            }
            assert_eq!(f.root.join("Packages").join(A).exists(), !remove);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn user_withdrawal_is_independent_of_optional_presentation_fields() {
    for (withdrawn, fields) in [
        (true, json!({"vrc-get":{"yanked":true,"aliases":false}})),
        (
            true,
            json!({"vrc-get":{"yanked":"withdrawn","aliases":false}}),
        ),
        (true, json!({"yanked":true,"keywords":false})),
        (false, json!({"vrc-get":{"yanked":false,"aliases":false}})),
        (false, json!({"yanked":false,"keywords":false})),
    ] {
        let f = fixture("local-withdrawal-display", empty(), vec![], None).await;
        let mut manifest = package(A, "1.0.0", json!({}));
        manifest
            .as_object_mut()
            .expect("fixture manifest")
            .extend(fields.as_object().expect("fixture fields").clone());
        enroll_local(&f, manifest).await;
        let CandidatePageItems::Summary { items } = f.summary().await.page else {
            panic!("expected summary");
        };
        assert_eq!(items.len(), usize::from(!withdrawn));
        for item in items {
            assert!(matches!(item.latest, CandidateChoice::Candidate { .. }));
            assert!(matches!(
                item.project_latest,
                CandidateChoice::Candidate { .. }
            ));
        }
        let versions = ProjectCandidateApplication::new(
            f.store.clone(),
            ProjectCandidateEngine,
            f.reader.clone(),
            f.config.clone(),
        )
        .query(
            &AccessContext::local_owner(),
            ProjectCandidateRequest {
                project_id: f.project.project_id.to_string(),
                expected_revision: f.project.revision.get(),
                view: CandidateView::Versions {
                    package_id: A.into(),
                    source: None,
                },
                limit: None,
                expected_snapshot: None,
                cursor: None,
            },
        )
        .await
        .expect("read version candidates");
        let CandidatePageItems::Versions { items, .. } = versions.page else {
            panic!("expected versions");
        };
        assert_eq!(items.len(), usize::from(!withdrawn));
        let result = f
            .packages
            .plan(&AccessContext::local_owner(), install(&f, A, "1.0.0"))
            .await;
        if withdrawn {
            assert_eq!(
                result
                    .expect_err("withdrawn version must be rejected")
                    .code(),
                M4ErrorCode::PackageVersionYanked
            );
        } else {
            assert_eq!(
                result
                    .expect("optional display fields must not block installation")
                    .change_set
                    .mutations
                    .len(),
                1
            );
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn closed_repository_package_is_absent_from_queries_plans_and_frozen_revalidation() {
    let f = fixture(
        "closed-repository-package",
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
    f.settings(|settings| {
        settings.hidden_repository_packages = vec![ConfigRepositoryPackages {
            repository_id: f.repository.to_string(),
            package_ids: vec![A.into()],
        }]
    })
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
    f.settings(|settings| settings.hidden_repository_packages.clear())
        .await;
    assert_eq!(apply(&f, plan).await.state, OperationState::Succeeded);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn hidden_unindexed_repository_package_does_not_block_visible_queries_or_installation() {
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
    f.settings(|settings| {
        settings.hidden_repository_packages = vec![ConfigRepositoryPackages {
            repository_id: other.to_string(),
            package_ids: vec![B.into()],
        }]
    })
    .await;
    assert!(f.summary().await.catalog_complete);
    let plan = f
        .packages
        .plan(&AccessContext::local_owner(), install(&f, A, "1.0.0"))
        .await
        .expect("owned VPM regression fixture succeeds");
    assert_eq!(apply(&f, plan).await.state, OperationState::Succeeded);
}
