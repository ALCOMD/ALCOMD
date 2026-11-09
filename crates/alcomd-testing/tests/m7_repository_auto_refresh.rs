//! Automatic maintenance reuses the real repository parser/store and preserves read latency.
use alcomd_application::*;
use alcomd_store::{FileConfigStore, StateStoreHandle};
use alcomd_vpm::VpmReader;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::{Notify, Semaphore};

struct NoPlatform;
impl StorageDirectoryPlatform for NoPlatform {
    fn default_directories(&self) -> Result<StorageDirectories, OfficialGuiError> {
        Ok(StorageDirectories {
            project_directory: std::env::temp_dir().to_string_lossy().into_owned(),
            backup_directory: std::env::temp_dir().to_string_lossy().into_owned(),
        })
    }
    fn normalize_directory(&self, _: &str) -> Result<String, OfficialGuiError> {
        panic!("no path mutation")
    }
}
impl UnityHubLauncher for NoPlatform {
    fn launch(&self, _: &str) -> Result<(), UnityHubOpenError> {
        panic!("no launch")
    }
}
impl UnityHubPlatform for NoPlatform {
    fn normalize_path(&self, _: &str) -> Result<String, OfficialGuiError> {
        panic!("no Hub mutation")
    }
    fn inspect(&self, _: Option<&str>) -> UnityHubLocation {
        panic!("no Hub inspection")
    }
}

#[derive(Clone)]
struct CountingReader {
    reader: VpmReader,
    calls: Arc<AtomicUsize>,
    gate: Option<Arc<Semaphore>>,
    entered: Arc<Notify>,
}
impl M3ReadAdapter for CountingReader {
    fn normalize_remote_repository_url(&self, url: &str) -> Result<String, M3Error> {
        self.reader.normalize_remote_repository_url(url)
    }
    async fn inspect_project(
        &self,
        _: String,
        _: ProjectDiscoveryMode,
    ) -> Result<ProjectObservation, M3Error> {
        panic!("no project scan")
    }
    async fn inspect_repository(
        &self,
        source: RepositorySource,
        validators: Option<RepositoryValidators>,
    ) -> Result<RepositoryReadOutcome, M3Error> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        if let Some(gate) = &self.gate {
            gate.acquire().await.expect("gate").forget();
        }
        self.reader.inspect_repository(source, validators).await
    }
}

fn key(value: &str) -> IdempotencyKey {
    IdempotencyKey::parse(value).expect("key")
}
fn repository(path: &Path, version: &str) {
    std::fs::write(path, serde_json::json!({"id":path.file_stem().expect("name").to_string_lossy(),"name":"Fixture","packages":{"com.example.fixture":{"versions":{version:{"name":"com.example.fixture","version":version,"url":"https://example.invalid/package.zip","author":{"name":"Fixture"}}}}}}).to_string()).expect("repository");
}
async fn setup() -> (PathBuf, StateStoreHandle, M7OfficialApplication, VpmReader) {
    let root = std::env::temp_dir().join(format!("alcomd-auto-refresh-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("directory");
    let store = StateStoreHandle::open(root.join("state.db")).expect("store");
    let config = M7OfficialApplication::new(
        Arc::new(FileConfigStore::new(root.join("settings.toml"))),
        Arc::new(NoPlatform),
        Arc::new(NoPlatform),
        Arc::new(NoPlatform),
    );
    config.initialize_settings().await.expect("settings");
    (root, store, config, VpmReader::new().expect("reader"))
}
async fn seed(
    root: &Path,
    store: &StateStoreHandle,
    reader: &VpmReader,
    name: &str,
    stale: bool,
) -> RepositoryRecord {
    let path = root.join(format!("{name}.json"));
    repository(&path, "1.0.0");
    let RepositoryReadOutcome::Fresh(mut observation) = reader
        .inspect_repository(
            RepositorySource::Local {
                path: path.to_string_lossy().into_owned(),
            },
            None,
        )
        .await
        .expect("inspect")
    else {
        panic!("fresh")
    };
    if stale {
        observation.refreshed_at_ms = 1;
    }
    store
        .register_repository(PrincipalId::local_owner(), observation, key(name), 1)
        .await
        .expect("seed")
        .value
}

#[tokio::test]
async fn automatic_refresh_skips_fresh_and_hidden_throttles_failures_and_manual_refresh_bypasses_age()
 {
    let (root, store, config, reader) = setup().await;
    let good = seed(&root, &store, &reader, "good", true).await;
    let fresh = seed(&root, &store, &reader, "fresh", false).await;
    let hidden = seed(&root, &store, &reader, "hidden", true).await;
    let broken = seed(&root, &store, &reader, "broken", true).await;
    config
        .update_settings(
            &AccessContext::local_owner(),
            1,
            ConfigUpdate {
                packages: Some(ConfigPackageSettingsUpdate {
                    hidden_repository_packages: None,
                    hidden_repository_ids: Some(vec![hidden.repository_id.to_string()]),
                    show_prerelease: None,
                    hide_local_user_packages: None,
                }),
                ..Default::default()
            },
        )
        .await
        .expect("hide source");
    for name in ["good", "fresh", "hidden"] {
        repository(&root.join(format!("{name}.json")), "2.0.0");
    }
    std::fs::write(root.join("broken.json"), "invalid JSON").expect("broken source");
    let calls = Arc::new(AtomicUsize::new(0));
    let registry = M3Application::new(
        store.clone(),
        CountingReader {
            reader,
            calls: calls.clone(),
            gate: None,
            entered: Arc::new(Notify::new()),
        },
    );
    let maintenance = RepositoryAutoRefresh::new(registry.clone(), config);
    let outcomes = maintenance.refresh_due().await.expect("maintenance");
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert!(
        outcomes
            .iter()
            .any(|(id, result)| *id == good.repository_id && result == &Ok(true))
    );
    assert!(
        outcomes
            .iter()
            .any(|(id, result)| *id == broken.repository_id && result.is_err())
    );
    let access = AccessContext::local_owner();
    for (record, version, revision) in [
        (&good, "2.0.0", 2),
        (&fresh, "1.0.0", 1),
        (&hidden, "1.0.0", 1),
        (&broken, "1.0.0", 1),
    ] {
        let page = registry
            .list_repository_packages(&access, record.repository_id, None, 100)
            .await
            .expect("packages");
        assert_eq!(page.packages[0].version, version);
        assert_eq!(
            registry
                .get_repository(&access, record.repository_id)
                .await
                .expect("record")
                .revision
                .get(),
            revision
        );
    }
    assert!(
        maintenance
            .refresh_due()
            .await
            .expect("repeat hint")
            .is_empty()
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "failed attempts do not retry on every query"
    );
    registry
        .refresh_repository(&access, fresh.repository_id, fresh.revision, key("manual"))
        .await
        .expect("manual ignores freshness");
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    assert!(
        !registry
            .refresh_repository_if_stale(&access, fresh.repository_id, key("after-manual"))
            .await
            .expect("recheck")
    );
    let denied = AccessContext::new(PrincipalId::local_owner(), [Permission::RepositoriesRead]);
    assert_eq!(
        registry
            .refresh_repository_if_stale(&denied, good.repository_id, key("denied"))
            .await
            .expect_err("manage required")
            .code(),
        M3ErrorCode::PermissionDenied
    );
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn concurrent_maintenance_coalesces_and_queries_do_not_wait_for_its_io() {
    let (root, store, config, reader) = setup().await;
    let record = seed(&root, &store, &reader, "blocked", true).await;
    repository(&root.join("blocked.json"), "2.0.0");
    let calls = Arc::new(AtomicUsize::new(0));
    let gate = Arc::new(Semaphore::new(0));
    let entered = Arc::new(Notify::new());
    let registry = M3Application::new(
        store,
        CountingReader {
            reader,
            calls: calls.clone(),
            gate: Some(gate.clone()),
            entered: entered.clone(),
        },
    );
    let maintenance = RepositoryAutoRefresh::new(registry.clone(), config);
    let worker = tokio::spawn({
        let maintenance = maintenance.clone();
        async move { maintenance.refresh_due().await }
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), entered.notified())
        .await
        .expect("refresh entered");
    assert!(
        maintenance
            .refresh_due()
            .await
            .expect("coalesced")
            .is_empty()
    );
    let page = tokio::time::timeout(
        std::time::Duration::from_millis(100),
        registry.list_repository_packages(
            &AccessContext::local_owner(),
            record.repository_id,
            None,
            100,
        ),
    )
    .await
    .expect("query must not wait for refresh I/O")
    .expect("cached page");
    assert_eq!(page.packages[0].version, "1.0.0");
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    gate.add_permits(1);
    worker.await.expect("worker").expect("refresh");
    let page = registry
        .list_repository_packages(
            &AccessContext::local_owner(),
            record.repository_id,
            None,
            100,
        )
        .await
        .expect("new page");
    assert_eq!(page.packages[0].version, "2.0.0");
}
