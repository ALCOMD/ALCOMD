use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use alcomd_application::{
    AccessContext, BackupCompression, BackupCreateOptions, BackupDefaults, BackupDirectoryProvider,
    ConfigBackupUpdate, ConfigNullableUpdate, ConfigPathUpdate, ConfigStore, ConfigUpdate,
    IdempotencyKey, M3RegistryStore, M5BackupApplication, M5BackupError, M5BackupErrorCode,
    M5BackupStore, M5BackupWriterGate, M7OfficialApplication, ManifestState, OfficialGuiError,
    OperationId, OperationState, Permission, PrincipalId, ProjectId, ProjectObservation,
    ProjectType, ResourceKey, ResourceLockCoordinator, StateStore, StorageDirectories,
    StorageDirectoryPlatform, UnityHubLauncher, UnityHubLocation, UnityHubOpenError,
    UnityHubPlatform, UnityWriterState, UnityWriterStateKind,
};
use alcomd_store::{FileConfigStore, StateStoreHandle};
use alcomd_vpm::BackupEngine;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("alcomd-storage-{}", uuid::Uuid::new_v4()));
        for relative in [
            "data/config",
            "Source/Assets",
            "Source/ProjectSettings",
            "Source/Packages",
            "Source/Library/Cache",
            "Projects",
            "Backups",
            "OtherBackups",
        ] {
            fs::create_dir_all(root.join(relative)).expect("isolated directory");
        }
        fs::write(
            root.join("Source/Assets/content.txt"),
            b"preserved project content",
        )
        .expect("content");
        fs::write(
            root.join("Source/ProjectSettings/ProjectVersion.txt"),
            "m_EditorVersion: 2022.3.22f1\n",
        )
        .expect("version");
        fs::write(
            root.join("Source/Packages/manifest.json"),
            r#"{"dependencies":{}}"#,
        )
        .expect("UPM manifest");
        fs::write(
            root.join("Source/Packages/vpm-manifest.json"),
            r#"{"dependencies":{},"locked":{}}"#,
        )
        .expect("VPM manifest");
        Self(root)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for _ in 0..20 {
            if fs::remove_dir_all(&self.0).is_ok() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("isolated storage fixture could not be removed");
    }
}

#[derive(Clone)]
struct TestDirectories(StorageDirectories);

impl StorageDirectoryPlatform for TestDirectories {
    fn default_directories(&self) -> Result<StorageDirectories, OfficialGuiError> {
        Ok(self.0.clone())
    }

    fn normalize_directory(&self, path: &str) -> Result<String, OfficialGuiError> {
        alcomd_platform::normalize_storage_directory(path)
            .map_err(|_| OfficialGuiError::InvalidInput)
    }
}

struct UnusedHub;

impl UnityHubPlatform for UnusedHub {
    fn normalize_path(&self, _: &str) -> Result<String, OfficialGuiError> {
        Err(OfficialGuiError::InvalidInput)
    }

    fn inspect(&self, _: Option<&str>) -> UnityHubLocation {
        UnityHubLocation::unavailable(false)
    }
}

impl UnityHubLauncher for UnusedHub {
    fn launch(&self, _: &str) -> Result<(), UnityHubOpenError> {
        Err(UnityHubOpenError::Unavailable)
    }
}

struct ConfiguredDirectory {
    settings: M7OfficialApplication,
    unavailable: AtomicBool,
    reads: AtomicUsize,
}

impl BackupDirectoryProvider for ConfiguredDirectory {
    fn defaults(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<BackupDefaults, M5BackupError>> + Send + '_>> {
        Box::pin(async move {
            self.reads.fetch_add(1, Ordering::SeqCst);
            if self.unavailable.load(Ordering::SeqCst) {
                return Err(M5BackupError::new(M5BackupErrorCode::StoreUnavailable));
            }
            let snapshot = self
                .settings
                .candidate_settings()
                .await
                .map_err(|_| M5BackupError::new(M5BackupErrorCode::StoreUnavailable))?;
            let directories = self
                .settings
                .observe_storage_directories(&snapshot)
                .await
                .map_err(|_| M5BackupError::new(M5BackupErrorCode::StoreUnavailable))?;
            Ok(BackupDefaults {
                directory: Some(directories.backup_directory),
                compression_mode: snapshot.settings.backups.compression_mode,
                include_vpm_packages: snapshot.settings.backups.include_vpm_packages,
                include_library: snapshot.settings.backups.include_library,
            })
        })
    }
}

#[derive(Clone, Copy)]
struct NoWriter;

impl M5BackupWriterGate for NoWriter {
    async fn observe_backup_source(
        &self,
        _: &AccessContext,
        project_id: ProjectId,
    ) -> Result<UnityWriterState, M5BackupError> {
        Ok(UnityWriterState {
            project_id,
            state: UnityWriterStateKind::NotObserved,
            evidence: Vec::new(),
            checked_at_ms: 1,
        })
    }
}

fn text(path: &Path) -> String {
    path.to_str().expect("fixture Unicode path").to_owned()
}

fn key(value: &str) -> IdempotencyKey {
    IdempotencyKey::parse(value).expect("key")
}

async fn succeeded(store: &StateStoreHandle, operation_id: OperationId) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let operation = store
                .get_operation(PrincipalId::local_owner(), operation_id)
                .await
                .expect("operation");
            if operation.state.is_terminal() {
                assert_eq!(
                    operation.state,
                    OperationState::Succeeded,
                    "operation error: {:?}",
                    operation.error_code
                );
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("backup or restore completed");
}

#[tokio::test]
async fn actual_configured_directory_controls_new_backups_without_redirecting_accepted_or_legacy_data()
 {
    let fixture = Fixture::new();
    let store = StateStoreHandle::open(fixture.0.join("data/state.db")).expect("store");
    let config = Arc::new(FileConfigStore::new(
        fixture.0.join("data/config/settings.toml"),
    ));
    let settings = M7OfficialApplication::new(
        config.clone(),
        Arc::new(UnusedHub),
        Arc::new(UnusedHub),
        Arc::new(TestDirectories(StorageDirectories {
            project_directory: text(&fixture.0.join("Projects")),
            backup_directory: text(&fixture.0.join("Backups")),
        })),
    );
    settings
        .initialize_settings()
        .await
        .expect("Config5 initialized");
    let access = AccessContext::new(
        PrincipalId::local_owner(),
        [
            Permission::SettingsRead,
            Permission::SettingsManage,
            Permission::ProjectsRead,
            Permission::ProjectsCreate,
            Permission::BackupsRead,
            Permission::BackupsManage,
        ],
    );
    let source = fs::canonicalize(fixture.0.join("Source")).expect("source");
    fs::write(source.join("Library/Cache/artifact.bin"), b"retained cache").expect("cache fixture");
    let project = store
        .register_project(
            PrincipalId::local_owner(),
            ProjectObservation {
                root_path: text(&source),
                path_identity_key: alcomd_platform::file_identity_key(&source).expect("identity"),
                project_type: ProjectType::Unknown,
                unity_version: "2022.3.22f1".to_owned(),
                unity_revision: None,
                vpm_manifest: ManifestState::Valid,
                upm_manifest: ManifestState::Valid,
                direct_dependencies: Vec::new(),
                locked_dependencies: Vec::new(),
                issues: Vec::new(),
            },
            key("register"),
            1,
        )
        .await
        .expect("project")
        .value;
    let engine = BackupEngine::new(fixture.0.join("data/backups")).expect("engine");
    let locks = Arc::new(ResourceLockCoordinator::default());
    let legacy_service =
        M5BackupApplication::with_locks(store.clone(), engine.clone(), NoWriter, locks.clone());
    let legacy = legacy_service
        .create(
            &access,
            project.project_id,
            project.revision,
            BackupCreateOptions {
                compression_mode: Some(BackupCompression::Fast),
                exclude_vpm_packages: Some(true),
                include_library: Some(false),
            },
            key("legacy"),
        )
        .await
        .expect("legacy create");
    succeeded(&store, legacy.operation_id).await;
    let provider = Arc::new(ConfiguredDirectory {
        settings: settings.clone(),
        unavailable: AtomicBool::new(false),
        reads: AtomicUsize::new(0),
    });
    let service = legacy_service
        .clone()
        .with_directory_provider(provider.clone());

    // Hold the source lock so this accepted task cannot archive before the preference changes.
    let source_lock = locks
        .acquire(vec![ResourceKey::Project(project.project_id)])
        .await;
    let accepted = service
        .create(
            &access,
            project.project_id,
            project.revision,
            BackupCreateOptions::default(),
            key("first"),
        )
        .await
        .expect("first create");
    let baseline = settings.get_settings(&access).await.expect("settings");
    let changed = settings
        .update_settings(
            &access,
            baseline.revision,
            ConfigUpdate {
                integrations: None,
                network_proxy: None,
                window: None,
                appearance: None,
                locale: None,
                packages: None,
                unity: None,
                projects: None,
                backups: Some(ConfigBackupUpdate {
                    compression_mode: Some(BackupCompression::Maximum),
                    include_vpm_packages: Some(false),
                    include_library: Some(true),
                }),
                paths: Some(ConfigPathUpdate {
                    project_directory: ConfigNullableUpdate::Unchanged,
                    backup_directory: ConfigNullableUpdate::Set(text(
                        &fixture.0.join("OtherBackups"),
                    )),
                }),
            },
        )
        .await
        .expect("save new backup directory");
    assert_eq!(config.load().expect("persisted settings"), changed);
    drop(source_lock);
    succeeded(&store, accepted.operation_id).await;
    assert!(
        fixture
            .0
            .join("Backups")
            .join(format!("{}.zip", accepted.backup_id))
            .is_file()
    );
    assert!(
        !fixture
            .0
            .join("OtherBackups")
            .join(format!("{}.zip", accepted.backup_id))
            .exists()
    );

    let later = service
        .create(
            &access,
            project.project_id,
            project.revision,
            BackupCreateOptions::default(),
            key("second"),
        )
        .await
        .expect("second create");
    succeeded(&store, later.operation_id).await;
    assert!(
        fixture
            .0
            .join("OtherBackups")
            .join(format!("{}.zip", later.backup_id))
            .is_file()
    );
    provider.unavailable.store(true, Ordering::SeqCst);
    let reads = provider.reads.load(Ordering::SeqCst);
    let replayed = service
        .create(
            &access,
            project.project_id,
            project.revision,
            BackupCreateOptions::default(),
            key("first"),
        )
        .await
        .expect("replay despite unavailable provider");
    assert!(replayed.replayed);
    assert_eq!(replayed.operation_id, accepted.operation_id);
    assert_eq!(provider.reads.load(Ordering::SeqCst), reads);

    for (index, backup_id) in [legacy.backup_id, accepted.backup_id, later.backup_id]
        .into_iter()
        .enumerate()
    {
        let record = store
            .get_backup(PrincipalId::local_owner(), backup_id)
            .await
            .expect("backup metadata");
        assert_eq!(record.archive_locator.starts_with("backup-v1:"), index == 0);
        assert_eq!(
            record.record.compression_mode,
            if index == 2 {
                BackupCompression::Maximum
            } else if index == 1 {
                BackupCompression::Level5
            } else {
                BackupCompression::Fast
            }
        );
        assert_eq!(record.record.exclude_vpm_packages, index != 1);
        let leaf = format!("Restored-{index}");
        let plan = service
            .plan_restore(&access, backup_id, fixture.0.join("Projects"), leaf.clone())
            .await
            .expect("restore frozen backup");
        let restore = service
            .apply_restore(&access, plan.plan_id, key(&format!("restore-{index}")))
            .await
            .expect("apply restore");
        succeeded(&store, restore.operation_id).await;
        assert_eq!(
            fs::read(
                fixture
                    .0
                    .join("Projects")
                    .join(leaf)
                    .join("Assets/content.txt")
            )
            .expect("restored contents"),
            b"preserved project content"
        );
        let restored_cache = fixture
            .0
            .join("Projects")
            .join(format!("Restored-{index}"))
            .join("Library/Cache/artifact.bin");
        assert_eq!(restored_cache.exists(), index == 2);
        if index == 2 {
            assert_eq!(
                fs::read(restored_cache).expect("restored cache"),
                b"retained cache"
            );
        }
    }
    drop(service);
    drop(legacy_service);
    drop(provider);
    drop(settings);
    drop(store);
}
