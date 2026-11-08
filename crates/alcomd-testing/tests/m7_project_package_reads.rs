//! Normal project-package requests read files without filling a database copy.
use alcomd_application::*;
use alcomd_store::StateStoreHandle;
use alcomd_vpm::{ProjectCandidateEngine, VpmReader};
use rusqlite::{Connection, OpenFlags};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

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
struct CountingReader {
    reader: VpmReader,
    reads: Arc<AtomicUsize>,
}
impl CandidateProjectReader for CountingReader {
    async fn read_candidate_project(
        &self,
        registered: &ProjectObservation,
    ) -> Result<CandidateProjectRead, CandidateQueryError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.reader.read_candidate_project(registered).await
    }
}

fn stored_state(path: &std::path::Path) -> (String, i64, i64, i64, i64) {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("read stored state");
    connection.query_row("SELECT snapshot_json,revision,(SELECT COUNT(*) FROM events),(SELECT COUNT(*) FROM idempotency_records),(SELECT COUNT(*) FROM operation_log_entries) FROM projects",[],|row|Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?))).expect("state projection")
}

fn write_project(root: &std::path::Path, version: &str, title: &str) {
    std::fs::create_dir_all(root.join("Packages/com.example.installed"))
        .expect("package directory");
    std::fs::create_dir_all(root.join("Packages/com.example.z")).expect("second package directory");
    std::fs::create_dir_all(root.join("ProjectSettings")).expect("project settings");
    std::fs::write(
        root.join("ProjectSettings/ProjectVersion.txt"),
        "m_EditorVersion: 2022.3.22f1\nm_EditorVersionWithRevision: 2022.3.22f1 (abcdef)\n",
    )
    .expect("version");
    std::fs::write(root.join("Packages/manifest.json"), "{\"dependencies\":{}}")
        .expect("UPM manifest");
    std::fs::write(root.join("Packages/vpm-manifest.json"),serde_json::to_vec(&serde_json::json!({"dependencies":{"com.example.installed":{"version":format!("^{version}")}},"locked":{"com.example.installed":{"version":version,"dependencies":{}},"com.example.z":{"version":"1.0.0","dependencies":{}}}})).expect("manifest JSON")).expect("VPM manifest");
    std::fs::write(root.join("Packages/com.example.installed/package.json"),serde_json::to_vec(&serde_json::json!({"name":"com.example.installed","version":version,"displayName":title,"description":"Directly read description","vrc-get":{"aliases":["search-needle"]}})).expect("package JSON")).expect("package manifest");
    std::fs::write(
        root.join("Packages/com.example.z/package.json"),
        r#"{"name":"com.example.z","version":"1.0.0"}"#,
    )
    .expect("second package manifest");
}

#[tokio::test]
async fn normal_reads_observe_project_changes_without_database_writes_and_pages_share_memory() {
    let directory =
        std::env::temp_dir().join(format!("alcomd-project-read-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&directory).expect("fixture directory");
    let root = directory.join("Project");
    write_project(&root, "1.0.0", "First title");
    let reader = VpmReader::new().expect("reader");
    let observation = reader
        .inspect_project(
            root.to_string_lossy().into_owned(),
            ProjectDiscoveryMode::ExactRoot,
        )
        .await
        .expect("initial project inspection");
    let database = directory.join("state.db");
    let store = StateStoreHandle::open(database.clone()).expect("store");
    let record = store
        .register_project(
            PrincipalId::local_owner(),
            observation,
            IdempotencyKey::parse("register").expect("key"),
            1,
        )
        .await
        .expect("register")
        .value;
    let config = M7OfficialApplication::new(
        Arc::new(alcomd_store::FileConfigStore::new(
            directory.join("settings.toml"),
        )),
        Arc::new(NoHubInspection),
        Arc::new(NoHubInspection),
        Arc::new(NoHubInspection),
    );
    config.initialize_settings().await.expect("settings");
    let reads = Arc::new(AtomicUsize::new(0));
    let locks = Arc::new(ResourceLockCoordinator::default());
    let app = ProjectCandidateApplication::new(
        store.clone(),
        ProjectCandidateEngine,
        CountingReader {
            reader,
            reads: reads.clone(),
        },
        config,
    )
    .with_locks(locks.clone());
    let request = ProjectCandidateRequest {
        project_id: record.project_id.to_string(),
        expected_revision: 1,
        view: CandidateView::Summary {
            package_ids: None,
            sources: None,
            filter: None,
            bulk_package_ids: None,
        },
        limit: Some(1),
        expected_snapshot: None,
        cursor: None,
    };
    let before = stored_state(&database);
    let first = {
        let writer = locks
            .acquire(vec![ResourceKey::Project(record.project_id)])
            .await;
        let access = AccessContext::local_owner();
        let pending = app.query(&access, request.clone());
        tokio::pin!(pending);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), &mut pending)
                .await
                .is_err(),
            "a normal read must not scan a project being written by Core"
        );
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        drop(writer);
        pending
            .await
            .expect("normal read after project writer releases its fence")
    };
    let CandidatePageItems::Summary { items } = &first.page else {
        panic!("summary")
    };
    assert_eq!(items[0].display_name.as_deref(), Some("First title"));
    assert!(items[0].providers.is_empty());
    let continuation = ProjectCandidateRequest {
        expected_snapshot: Some(first.snapshot.fingerprint.clone()),
        cursor: first.next_cursor.clone(),
        ..request.clone()
    };
    app.query(&AccessContext::local_owner(), continuation.clone())
        .await
        .expect("next page");
    assert_eq!(
        reads.load(Ordering::SeqCst),
        1,
        "pagination reuses only an in-memory read"
    );
    write_project(&root, "1.1.0", "Changed title");
    let changed = app
        .query(&AccessContext::local_owner(), request.clone())
        .await
        .expect("normal read after file change");
    let CandidatePageItems::Summary { items } = &changed.page else {
        panic!("summary")
    };
    assert_eq!(items[0].display_name.as_deref(), Some("Changed title"));
    assert_eq!(items[0].requested_range.as_deref(), Some("^1.1.0"));
    assert!(matches!(&items[0].installed,CandidateInstalled::Locked {version} if version=="1.1.0"));
    assert_ne!(changed.snapshot.fingerprint, first.snapshot.fingerprint);
    assert_eq!(
        reads.load(Ordering::SeqCst),
        2,
        "one file read for each normal request, without a refresh/fill RPC"
    );
    assert_eq!(
        app.query(&AccessContext::local_owner(), continuation)
            .await
            .expect_err("old cursor is stale after a new observation"),
        CandidateQueryError::EvidenceStale
    );
    assert_eq!(
        stored_state(&database),
        before,
        "queries do not fill tables, update registry facts or create events/logs"
    );
    let connection = Connection::open_with_flags(&database, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("inspect schema");
    assert_eq!(connection.query_row::<i64,_,_>("SELECT COUNT(*) FROM sqlite_schema WHERE name IN ('project_package_presentations','project_presentation_observations')",[],|row|row.get(0)).expect("retired tables"),0);
    drop(connection);
    drop(app);
    drop(store);
    // Actor disposal completes asynchronously; the isolated fixture is retained if Windows still holds its files.
    let _ = std::fs::remove_dir_all(directory);
}
