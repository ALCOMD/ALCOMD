//! Deterministic Config change during an in-flight candidate snapshot read.
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use alcomd_application::*;
use alcomd_vpm::ProjectCandidateEngine;
use tokio::sync::Notify;

struct NoHubInspection;
impl StorageDirectoryPlatform for NoHubInspection {
    fn default_directories(&self) -> Result<StorageDirectories, OfficialGuiError> {
        Ok(StorageDirectories {
            project_directory: std::env::temp_dir()
                .join("alcomd-project-default")
                .to_string_lossy()
                .into_owned(),
            backup_directory: std::env::temp_dir()
                .join("alcomd-backup-default")
                .to_string_lossy()
                .into_owned(),
        })
    }

    fn normalize_directory(&self, _: &str) -> Result<String, OfficialGuiError> {
        panic!("candidate query must not validate storage directories")
    }
}
impl UnityHubLauncher for NoHubInspection {
    fn launch(&self, _: &str) -> Result<(), UnityHubOpenError> {
        panic!("candidate query must not launch Hub")
    }
}
impl UnityHubPlatform for NoHubInspection {
    fn normalize_path(&self, _: &str) -> Result<String, OfficialGuiError> {
        panic!("candidate query must not validate Hub paths")
    }
    fn inspect(&self, _: Option<&str>) -> UnityHubLocation {
        panic!("candidate query must not inspect Hub paths")
    }
}

#[derive(Clone)]
struct PausedReadStore {
    entered: Arc<Notify>,
    release: Arc<Notify>,
    reads: Arc<AtomicUsize>,
    project: ProjectRecord,
}

impl CandidateStore for PausedReadStore {
    async fn candidate_authority(
        &self,
        _: PrincipalId,
        _: ProjectId,
    ) -> Result<CandidateStoreAuthority, CandidateQueryError> {
        panic!("this test reads an uncached snapshot")
    }

    async fn candidate_snapshot(
        &self,
        owner: PrincipalId,
        project: ProjectId,
        _: Option<Vec<String>>,
    ) -> Result<CandidateStoreSnapshot, CandidateQueryError> {
        assert_eq!(owner, PrincipalId::local_owner());
        assert_eq!(project, self.project.project_id);
        self.reads.fetch_add(1, Ordering::SeqCst);
        // query() has already copied its initial ConfigSnapshot before this port call.
        self.entered.notify_one();
        self.release.notified().await;
        Ok(CandidateStoreSnapshot {
            project: self.project.clone(),
            entries: vec![],
            fingerprint_records: vec!["unchanged-project-and-source-snapshot".into()],
            catalog_complete: true,
            source_metadata: Default::default(),
        })
    }
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "alcomd-candidate-config-race-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&path).expect("create isolated test directory");
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[derive(Clone)]
struct TrackingStore {
    project: ProjectRecord,
    full_reads: Arc<AtomicUsize>,
    authority_reads: Arc<AtomicUsize>,
    inventory_revision: Arc<AtomicUsize>,
}

impl TrackingStore {
    fn authority(&self, owner: PrincipalId, project: ProjectId) -> CandidateStoreAuthority {
        assert_eq!(owner, PrincipalId::local_owner());
        assert_eq!(project, self.project.project_id);
        CandidateStoreAuthority {
            project_revision: self.project.revision,
            fingerprint_records: vec![self.inventory_revision.load(Ordering::SeqCst).to_string()],
            catalog_complete: true,
        }
    }
}

impl CandidateStore for TrackingStore {
    async fn candidate_authority(
        &self,
        owner: PrincipalId,
        project: ProjectId,
    ) -> Result<CandidateStoreAuthority, CandidateQueryError> {
        self.authority_reads.fetch_add(1, Ordering::SeqCst);
        Ok(self.authority(owner, project))
    }

    async fn candidate_snapshot(
        &self,
        owner: PrincipalId,
        project: ProjectId,
        _: Option<Vec<String>>,
    ) -> Result<CandidateStoreSnapshot, CandidateQueryError> {
        self.full_reads.fetch_add(1, Ordering::SeqCst);
        let authority = self.authority(owner, project);
        let remote = "11111111-1111-4111-8111-111111111111";
        let local = "22222222-2222-4222-8222-222222222222";
        let entries = [
            ("com.example.a", remote, 1),
            ("com.example.b", remote, 1),
            ("com.example.b", local, 2),
        ]
        .into_iter()
        .map(
            |(package_id, repository_id, priority)| CandidateCatalogEntry {
                package_id: package_id.into(),
                version: "1.0.0".into(),
                display_name: Some("Package name".into()),
                links: CandidateLinks::default(),
                source: PackageSourceSelector::Repository {
                    repository_id: repository_id.into(),
                },
                source_revision: 1,
                priority,
                yanked: false,
                unity: None,
                unity_release: None,
                metadata_ready: true,
                legacy_metadata_present: false,
            },
        )
        .collect();
        Ok(CandidateStoreSnapshot {
            project: self.project.clone(),
            entries,
            fingerprint_records: authority.fingerprint_records,
            catalog_complete: authority.catalog_complete,
            source_metadata: [
                (
                    format!("repository:{remote}"),
                    CandidateSourceMetadata {
                        kind: CandidateFilterSource::Remote,
                        search_name: "remote registry".into(),
                    },
                ),
                (
                    format!("repository:{local}"),
                    CandidateSourceMetadata {
                        kind: CandidateFilterSource::Local,
                        search_name: "local registry".into(),
                    },
                ),
            ]
            .into_iter()
            .collect(),
        })
    }
}

#[tokio::test]
async fn subsequent_summary_pages_reuse_one_full_read_but_reject_stale_authority() {
    let fixture = Fixture::new();
    let project_id = ProjectId::new();
    let store = TrackingStore {
        project: ProjectRecord {
            project_id,
            observation: ProjectObservation {
                root_path: fixture
                    .0
                    .join("unopened-project")
                    .to_string_lossy()
                    .into_owned(),
                path_identity_key: vec![1],
                project_type: ProjectType::Unknown,
                unity_version: "2022.3.22f1".into(),
                unity_revision: None,
                vpm_manifest: ManifestState::Valid,
                upm_manifest: ManifestState::Missing,
                direct_dependencies: ["com.example.a", "com.example.b"]
                    .map(|id| DependencyIdentity {
                        package_id: id.to_owned(),
                        value: "1.0.0".into(),
                    })
                    .to_vec(),
                locked_dependencies: vec![],
                issues: vec![],
            },
            revision: Revision::INITIAL,
            registered_at_ms: 1,
            last_modified_at_ms: None,
            favorite: false,
        },
        full_reads: Arc::new(AtomicUsize::new(0)),
        authority_reads: Arc::new(AtomicUsize::new(0)),
        inventory_revision: Arc::new(AtomicUsize::new(1)),
    };
    let config = M7OfficialApplication::new(
        Arc::new(alcomd_store::FileConfigStore::new(
            fixture.0.join("settings.toml"),
        )),
        Arc::new(NoHubInspection),
        Arc::new(NoHubInspection),
        Arc::new(NoHubInspection),
    );
    config
        .initialize_settings()
        .await
        .expect("initialize settings");
    let application =
        ProjectCandidateApplication::new(store.clone(), ProjectCandidateEngine, config);
    let owner = AccessContext::local_owner();
    let request = ProjectCandidateRequest {
        project_id: project_id.to_string(),
        expected_revision: 1,
        view: CandidateView::Summary {
            package_ids: None,
            sources: None,
            filter: None,
        },
        limit: Some(1),
        expected_snapshot: None,
        cursor: None,
    };
    let first = application
        .query(&owner, request.clone())
        .await
        .expect("first page");
    let continuation = ProjectCandidateRequest {
        cursor: first.next_cursor.clone(),
        ..request.clone()
    };
    assert!(continuation.cursor.is_some());
    let second = application
        .query(&owner, continuation.clone())
        .await
        .expect("cached continuation");
    assert_eq!(second.snapshot, first.snapshot);
    assert!(second.next_cursor.is_none());
    assert_ne!(first.page, second.page);
    assert_eq!(store.full_reads.load(Ordering::SeqCst), 1);
    assert_eq!(store.authority_reads.load(Ordering::SeqCst), 1);

    let filtered_request = |source| ProjectCandidateRequest {
        view: CandidateView::Summary {
            package_ids: None,
            sources: None,
            filter: Some(CandidateFilter {
                search: Some("REMOTE Registry".into()),
                status: Some(CandidateFilterStatus::Available),
                source,
            }),
        },
        expected_snapshot: Some(first.snapshot.fingerprint.clone()),
        ..request.clone()
    };
    let filtered = application
        .query(
            &owner,
            filtered_request(Some(CandidateFilterSource::Remote)),
        )
        .await
        .expect("matching first filtered page");
    assert_eq!(filtered.snapshot, first.snapshot);
    assert_eq!(filtered.page, first.page);
    let filtered_cursor = filtered.next_cursor.expect("filtered continuation");
    assert_ne!(
        Some(&filtered_cursor.query_fingerprint),
        continuation
            .cursor
            .as_ref()
            .map(|cursor| &cursor.query_fingerprint)
    );
    let local_request = filtered_request(Some(CandidateFilterSource::Local));
    assert_eq!(
        application
            .query(
                &owner,
                ProjectCandidateRequest {
                    cursor: Some(filtered_cursor),
                    ..local_request.clone()
                }
            )
            .await
            .expect_err("cursor belongs to different filter"),
        CandidateQueryError::InvalidInput,
    );
    let local_page = application
        .query(&owner, local_request)
        .await
        .expect("local provider filter");
    assert_eq!(local_page.page, second.page);
    assert!(local_page.next_cursor.is_none());
    assert_eq!(store.full_reads.load(Ordering::SeqCst), 1);
    assert_eq!(store.authority_reads.load(Ordering::SeqCst), 3);

    store.inventory_revision.fetch_add(1, Ordering::SeqCst);
    assert_eq!(
        application
            .query(&owner, continuation)
            .await
            .expect_err("old cursor rejected"),
        CandidateQueryError::EvidenceStale
    );
    assert_eq!(store.full_reads.load(Ordering::SeqCst), 1);
    assert_eq!(store.authority_reads.load(Ordering::SeqCst), 4);
    let changed = application
        .query(&owner, request)
        .await
        .expect("new snapshot");
    assert_ne!(changed.snapshot, first.snapshot);
    assert_eq!(store.full_reads.load(Ordering::SeqCst), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn config_change_during_candidate_store_read_fails_stale_without_query_writes() {
    let fixture = Fixture::new();
    let settings_path = fixture.0.join("settings.toml");
    let project_id = ProjectId::new();
    let store = PausedReadStore {
        entered: Arc::new(Notify::new()),
        release: Arc::new(Notify::new()),
        reads: Arc::new(AtomicUsize::new(0)),
        project: ProjectRecord {
            project_id,
            observation: ProjectObservation {
                root_path: fixture
                    .0
                    .join("project-not-on-disk")
                    .to_string_lossy()
                    .into(),
                path_identity_key: vec![1],
                project_type: ProjectType::Unknown,
                unity_version: "2022.3.22f1".into(),
                unity_revision: None,
                vpm_manifest: ManifestState::Valid,
                upm_manifest: ManifestState::Missing,
                direct_dependencies: vec![],
                locked_dependencies: vec![],
                issues: vec![],
            },
            revision: Revision::INITIAL,
            registered_at_ms: 1,
            last_modified_at_ms: None,
            favorite: false,
        },
    };
    let config = M7OfficialApplication::new(
        Arc::new(alcomd_store::FileConfigStore::new(settings_path.clone())),
        Arc::new(NoHubInspection),
        Arc::new(NoHubInspection),
        Arc::new(NoHubInspection),
    );
    config
        .initialize_settings()
        .await
        .expect("initialize Config");
    let initial = config.candidate_settings().await.expect("initial snapshot");
    let application =
        ProjectCandidateApplication::new(store.clone(), ProjectCandidateEngine, config.clone());
    let request = ProjectCandidateRequest {
        project_id: project_id.to_string(),
        expected_revision: 1,
        view: CandidateView::Summary {
            package_ids: Some(vec!["com.example.absent".into()]),
            sources: None,
            filter: None,
        },
        limit: None,
        expected_snapshot: None,
        cursor: None,
    };
    let query = tokio::spawn(async move {
        application
            .query(&AccessContext::local_owner(), request)
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), store.entered.notified())
        .await
        .expect("query reaches paused snapshot port");

    let updated = config
        .update_settings(
            &AccessContext::local_owner(),
            initial.revision,
            ConfigUpdate {
                projects: None,
                backups: None,
                integrations: None,
                network_proxy: None,
                window: None,
                paths: None,
                unity: None,
                appearance: None,
                locale: None,
                packages: Some(ConfigPackageSettingsUpdate {
                    show_prerelease: Some(!initial.settings.packages.show_prerelease),
                    hidden_repository_ids: None,
                    hide_local_user_packages: None,
                }),
            },
        )
        .await
        .expect("explicit existing Config update while query is paused");
    assert_eq!(updated.revision, initial.revision + 1);
    let config_after_explicit_update = std::fs::read(&settings_path).expect("updated Config");
    store.release.notify_one();
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(5), query)
            .await
            .expect("query completes after release")
            .expect("query task did not panic")
            .expect_err("mixed Config snapshot must fail"),
        CandidateQueryError::EvidenceStale
    );
    assert_eq!(store.reads.load(Ordering::SeqCst), 1);
    assert_eq!(
        std::fs::read(&settings_path).expect("Config after failed query"),
        config_after_explicit_update
    );
    let remaining: Vec<_> = std::fs::read_dir(&fixture.0)
        .expect("isolated test directory")
        .map(|entry| entry.expect("directory entry").file_name())
        .collect();
    assert_eq!(remaining, vec![std::ffi::OsString::from("settings.toml")]);
    assert_eq!(
        config.candidate_settings().await.expect("confirmed Config"),
        updated
    );
}

#[tokio::test]
async fn wider_legacy_project_observations_fail_query_dto_bounds_without_changing_project() {
    let fixture = Fixture::new();
    for case in 0..3 {
        let project_id = ProjectId::new();
        let mut project = ProjectRecord {
            project_id,
            observation: ProjectObservation {
                root_path: "unopened-project".into(),
                path_identity_key: vec![1],
                project_type: ProjectType::Unknown,
                unity_version: "2022.3.22f1".into(),
                unity_revision: None,
                vpm_manifest: ManifestState::Valid,
                upm_manifest: ManifestState::Missing,
                direct_dependencies: vec![],
                locked_dependencies: vec![],
                issues: vec![],
            },
            revision: Revision::INITIAL,
            registered_at_ms: 1,
            last_modified_at_ms: None,
            favorite: false,
        };
        let dependency = DependencyIdentity {
            package_id: if case < 2 {
                "a".repeat(129)
            } else {
                "com.example.package".into()
            },
            value: if case == 2 {
                format!("1.0.0+{}", "a".repeat(251))
            } else {
                "1.0.0".into()
            },
        };
        if case == 0 {
            project.observation.direct_dependencies.push(dependency);
        } else {
            project.observation.locked_dependencies.push(dependency);
        }
        let original_project = project.clone();
        let store = PausedReadStore {
            entered: Arc::new(Notify::new()),
            release: Arc::new(Notify::new()),
            reads: Arc::new(AtomicUsize::new(0)),
            project,
        };
        let config = M7OfficialApplication::new(
            Arc::new(alcomd_store::FileConfigStore::new(
                fixture.0.join("settings.toml"),
            )),
            Arc::new(NoHubInspection),
            Arc::new(NoHubInspection),
            Arc::new(NoHubInspection),
        );
        config
            .initialize_settings()
            .await
            .expect("initialize Config");
        let application =
            ProjectCandidateApplication::new(store.clone(), ProjectCandidateEngine, config);
        store.release.notify_one();
        let result = application
            .query(
                &AccessContext::local_owner(),
                ProjectCandidateRequest {
                    project_id: project_id.to_string(),
                    expected_revision: 1,
                    view: CandidateView::Summary {
                        package_ids: None,
                        sources: None,
                        filter: None,
                    },
                    limit: None,
                    expected_snapshot: None,
                    cursor: None,
                },
            )
            .await;
        assert_eq!(
            result.expect_err("output cannot fit approved DTO"),
            CandidateQueryError::LimitExceeded
        );
        assert_eq!(
            store.project, original_project,
            "registered observation remains unchanged"
        );
        assert_eq!(store.reads.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_candidate_read_creates_no_files_or_business_writes() {
    let fixture = Fixture::new();
    let path = fixture.0.join("settings.toml");
    let project_id = ProjectId::new();
    let store = PausedReadStore {
        entered: Arc::new(Notify::new()),
        release: Arc::new(Notify::new()),
        reads: Arc::new(AtomicUsize::new(0)),
        project: ProjectRecord {
            project_id,
            observation: ProjectObservation {
                root_path: "unopened-project".into(),
                path_identity_key: vec![1],
                project_type: ProjectType::Unknown,
                unity_version: "2022.3.22f1".into(),
                unity_revision: None,
                vpm_manifest: ManifestState::Valid,
                upm_manifest: ManifestState::Missing,
                direct_dependencies: vec![],
                locked_dependencies: vec![],
                issues: vec![],
            },
            revision: Revision::INITIAL,
            registered_at_ms: 1,
            last_modified_at_ms: None,
            favorite: false,
        },
    };
    let config = M7OfficialApplication::new(
        Arc::new(alcomd_store::FileConfigStore::new(path.clone())),
        Arc::new(NoHubInspection),
        Arc::new(NoHubInspection),
        Arc::new(NoHubInspection),
    );
    config
        .initialize_settings()
        .await
        .expect("initialize Config");
    let before_bytes = std::fs::read(&path).expect("Config bytes");
    let before_config = config.candidate_settings().await.expect("confirmed Config");
    let application =
        ProjectCandidateApplication::new(store.clone(), ProjectCandidateEngine, config.clone());
    let task = tokio::spawn(async move {
        application
            .query(
                &AccessContext::local_owner(),
                ProjectCandidateRequest {
                    project_id: project_id.to_string(),
                    expected_revision: 1,
                    view: CandidateView::Summary {
                        package_ids: None,
                        sources: None,
                        filter: None,
                    },
                    limit: None,
                    expected_snapshot: None,
                    cursor: None,
                },
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), store.entered.notified())
        .await
        .expect("candidate read enters paused port");
    task.abort();
    assert!(
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("cancellation completes")
            .expect_err("task was cancelled")
            .is_cancelled()
    );
    assert_eq!(store.reads.load(Ordering::SeqCst), 1);
    assert_eq!(
        std::fs::read(&path).expect("Config after cancellation"),
        before_bytes
    );
    assert_eq!(
        config
            .candidate_settings()
            .await
            .expect("cache remains available"),
        before_config
    );
    assert_eq!(
        std::fs::read_dir(&fixture.0)
            .expect("fixture entries")
            .count(),
        1
    );
    // PausedReadStore has no business write port, and the sole file is existing Config.
    assert!(path.is_file());
}
