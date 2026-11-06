use std::path::Path;
use std::time::{Duration, Instant};

use alcomd_application::{
    M4Store, OperationRecord, PackagePlanRequest, PackagePlanVersion, PlanAction,
    ResolverCatalogSource,
};
use serde_json::{Value, json};

use super::*;

const PACKAGE_ID: &str = "com.vrcfury.vrcfury";

#[tokio::test]
#[ignore = "downloads VRCFury from its public repository into a disposable project"]
async fn live_vrcfury_installs_with_a_locally_pinned_digest() {
    let root = std::env::temp_dir()
        .canonicalize()
        .expect("temporary directory")
        .join(format!("alcomd-vrcfury-test-{}", OperationId::new()));
    let project_root = root.join("Project");
    for directory in ["Assets", "Packages", "ProjectSettings"] {
        std::fs::create_dir_all(project_root.join(directory)).expect("isolated Unity scaffold");
    }
    let legacy = project_root.join("Assets/VRCFury");
    std::fs::create_dir(&legacy).expect("isolated legacy VRCFury directory");
    std::fs::write(legacy.join("sentinel.txt"), b"isolated legacy asset")
        .expect("seed isolated legacy asset");
    std::fs::write(
        project_root.join("Packages/manifest.json"),
        b"{\"dependencies\":{}}",
    )
    .expect("UPM manifest");
    let initial_vpm = b"{\"dependencies\":{},\"locked\":{}}";
    std::fs::write(project_root.join("Packages/vpm-manifest.json"), initial_vpm)
        .expect("VPM manifest");
    std::fs::write(
        project_root.join("ProjectSettings/ProjectVersion.txt"),
        b"m_EditorVersion: 2022.3.22f1\n",
    )
    .expect("Unity version file");

    let access = AccessContext::local_owner();
    let store = StateStoreHandle::open(root.join("state.sqlite3")).expect("isolated store");
    let reader = alcomd_vpm::VpmReader::new().expect("VPM reader");
    let registry = M3ReadApplication::new(store.clone(), reader.clone());
    let cache_root = root.join("cache");
    let package_engine = alcomd_vpm::PackageEngine::new(store.clone(), reader, cache_root.clone())
        .expect("package engine");
    let packages = M4PackageApplication::new(store.clone(), package_engine);
    let operations = M2Application::new(store.clone());
    let project = registry
        .register_project(
            &access,
            project_root.to_string_lossy().into_owned(),
            key("project"),
        )
        .await
        .expect("registered isolated project")
        .value;
    let download_index = Instant::now();
    let repo = registry
        .register_repository(
            &access,
            alcomd_application::RepositorySource::Remote {
                url: "https://vcc.vrcfury.com/".into(),
            },
            key("vrcfury"),
        )
        .await
        .expect("registered public VRCFury repository")
        .value;
    let index_elapsed = download_index.elapsed();
    let request = PackagePlanRequest {
        action: PlanAction::Install,
        project_id: project.project_id,
        expected_revision: project.revision,
        package_id: Some(PACKAGE_ID.into()),
        version_range: None,
        repository_id: Some(repo.repository_id.to_string()),
        include_prerelease: false,
        plan_version: PackagePlanVersion::V2,
        source_selector: None,
        reinstall_selection: None,
        reinstall_sources: Vec::new(),
        bulk_intents: Vec::new(),
    };
    let cold_start = Instant::now();
    let first = packages
        .plan(&access, request.clone())
        .await
        .expect("cold hashless plan");
    let cold_elapsed = cold_start.elapsed();
    assert!(
        first
            .change_set
            .legacy_removals
            .iter()
            .any(|removal| removal.path == "Assets/VRCFury")
    );
    assert!(legacy.join("sentinel.txt").is_file());
    let pin = first
        .source_set
        .iter()
        .find(|pin| pin.package_id() == PACKAGE_ID)
        .expect("selected package pin");
    let digest = pin.archive_sha256();
    let version = pin.version().to_owned();
    let catalog = store
        .resolver_catalog(access.principal().clone(), false)
        .await
        .expect("indexed catalogue");
    let selected = catalog
        .entries
        .iter()
        .find(|entry| entry.package_id == PACKAGE_ID && entry.version == version)
        .expect("selected catalogue entry");
    assert!(selected.author_email.is_none());
    assert!(
        matches!(&selected.source, ResolverCatalogSource::Repository { zip_sha256: None, local_zip_sha256: Some(stored), .. } if *stored == digest)
    );

    let cache = alcomd_vpm::PackageCache::new(cache_root).expect("package cache");
    let cached = cache.object_path(&digest);
    let original = std::fs::read(&cached).expect("first download produced cached archive");
    std::fs::write(&cached, b"modified test archive").expect("tamper with isolated cache");
    let rejected = packages
        .apply_plan(&access, first.plan_id, project.revision, key("tampered"))
        .await
        .expect("accepted apply request");
    let rejected = terminal_operation(&operations, &access, rejected.operation_id).await;
    assert_eq!(rejected.state, DomainState::Failed, "{rejected:?}");
    assert_eq!(
        rejected.error_code.as_deref(),
        Some("package_cache_corrupt")
    );
    assert_eq!(
        std::fs::read(project_root.join("Packages/vpm-manifest.json")).expect("unchanged manifest"),
        initial_vpm
    );
    assert!(!project_root.join("Packages").join(PACKAGE_ID).exists());
    assert!(legacy.join("sentinel.txt").is_file());

    std::fs::write(&cached, &original).expect("restore isolated verified archive");
    let warm_start = Instant::now();
    let second = packages
        .plan(&access, request)
        .await
        .expect("warm hashless plan");
    let warm_elapsed = warm_start.elapsed();
    assert_eq!(second.change_set_fingerprint, first.change_set_fingerprint);
    let apply_start = Instant::now();
    let accepted = packages
        .apply_plan(&access, second.plan_id, project.revision, key("install"))
        .await
        .expect("accepted install");
    let installed = terminal_operation(&operations, &access, accepted.operation_id).await;
    assert_eq!(installed.state, DomainState::Succeeded, "{installed:?}");
    assert!(!legacy.exists());
    let apply_elapsed = apply_start.elapsed();
    let manifest = read_json(
        &project_root
            .join("Packages")
            .join(PACKAGE_ID)
            .join("package.json"),
    );
    assert_eq!(manifest["name"], PACKAGE_ID);
    assert_eq!(manifest["version"], version);
    let locked = read_json(&project_root.join("Packages/vpm-manifest.json"));
    assert_eq!(locked["locked"][PACKAGE_ID]["version"], json!(version));
    eprintln!(
        "VRCFury {version}: {} bytes, index {index_elapsed:?}, cold plan {cold_elapsed:?}, warm plan {warm_elapsed:?}, apply {apply_elapsed:?}, isolated root {}",
        original.len(),
        root.display()
    );
}

fn key(text: &str) -> IdempotencyKey {
    IdempotencyKey::parse(text).expect("test key")
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).expect("JSON file")).expect("JSON contents")
}

async fn terminal_operation(
    operations: &M2Application,
    access: &AccessContext,
    id: OperationId,
) -> OperationRecord {
    tokio::time::timeout(Duration::from_secs(120), async {
        loop {
            let operation = operations
                .get_operation(access, id)
                .await
                .expect("operation");
            if operation.state.is_terminal() {
                return operation;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("operation finished")
}
