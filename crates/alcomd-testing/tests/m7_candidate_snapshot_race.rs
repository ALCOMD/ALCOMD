//! Deterministic Config change during an in-flight candidate snapshot read.
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use alcomd_application::*;
use alcomd_vpm::ProjectCandidateEngine;
use tokio::sync::Notify;

#[derive(Clone)]
struct PausedReadStore {
    entered: Arc<Notify>,
    release: Arc<Notify>,
    reads: Arc<AtomicUsize>,
    project: ProjectRecord,
}

impl CandidateStore for PausedReadStore {
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
        })
    }
}

// This fixture deliberately implements no Plan, Operation, Event or write store port.
impl OfficialGuiStore for PausedReadStore {
    async fn list_official_activity(
        &self,
        _: PrincipalId,
        _: Option<OfficialActivityCursor>,
        _: u32,
    ) -> Result<OfficialActivityPage, StoreError> {
        panic!("candidate query must not read Activity")
    }
    async fn list_official_diagnostics(
        &self,
        _: PrincipalId,
        _: Option<OfficialDiagnosticCursor>,
        _: u32,
    ) -> Result<OfficialDiagnosticPage, StoreError> {
        panic!("candidate query must not read Diagnostics")
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
                observed_at_ms: 1,
            },
            revision: Revision::INITIAL,
            registered_at_ms: 1,
            favorite: false,
        },
    };
    let config = M7OfficialApplication::new(store.clone(), settings_path.clone());
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
                observed_at_ms: 1,
            },
            revision: Revision::INITIAL,
            registered_at_ms: 1,
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
        let config = M7OfficialApplication::new(store.clone(), fixture.0.join("settings.toml"));
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
                observed_at_ms: 1,
            },
            revision: Revision::INITIAL,
            registered_at_ms: 1,
            favorite: false,
        },
    };
    let config = M7OfficialApplication::new(store.clone(), path.clone());
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
