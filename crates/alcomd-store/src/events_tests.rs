use super::*;
use alcomd_application::{
    AccessContext, Application, EventInvalidation, EventsCursor, ExtensionInstallPlanDraft,
    ExtensionPackageEvidence, ExtensionSourceKind, ExtensionTrustDecision, FilesystemPhase,
    JournalState, Permission,
};
use rusqlite::params;
use std::time::Duration;

pub(super) struct Directory(PathBuf);

impl Directory {
    pub(super) fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("alcomd-live-events-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).expect("isolated event test directory");
        Self(root)
    }
    pub(super) fn database(&self) -> PathBuf {
        self.0.join("state.db")
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        assert!(self.0.is_absolute() && self.0.starts_with(std::env::temp_dir()));
        assert!(
            self.0
                .file_name()
                .expect("test directory")
                .to_string_lossy()
                .starts_with("alcomd-live-events-")
        );
        for _ in 0..20 {
            if std::fs::remove_dir_all(&self.0).is_ok() {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

fn key(value: &str) -> IdempotencyKey {
    IdempotencyKey::parse(value).expect("test key")
}

async fn installed_extension(store: &StateStoreHandle) -> ExtensionRecord {
    installed_extension_for(store, PrincipalId::local_owner()).await
}

pub(super) async fn installed_extension_for(
    store: &StateStoreHandle,
    owner: PrincipalId,
) -> ExtensionRecord {
    let plan = M6Store::create_install_plan(
        store,
        owner.clone(),
        ExtensionInstallPlanDraft {
            evidence: ExtensionPackageEvidence {
                source_kind: ExtensionSourceKind::LocalOwnerSelected,
                source_locator: "<FIXTURE>/extension.alcomdextension".to_owned(),
                source_identity: vec![7; 24],
                extension_id: "dev.example.live".to_owned(),
                version: "1.0.0".to_owned(),
                api_major: 1,
                profile_version: 1,
                package_digest: [1; 32],
                manifest_digest: [2; 32],
                component_digest: [3; 32],
                publisher_fingerprint: format!("ed25519-sha256:{}", "a".repeat(64)),
                required_permissions: vec!["background.run".to_owned()],
                optional_permissions: vec![],
                required_interfaces: vec!["alcomd:extension/host-data@1.0.0".to_owned()],
                optional_interfaces: vec![],
                ui_protocol: None,
            },
            trust_decision: ExtensionTrustDecision::UserApprovedForExtension,
            expected_revision: None,
            plan_fingerprint: [8; 32],
        },
        1,
    )
    .await
    .expect("frozen extension fixture");
    let accepted = M6Store::accept_plan(store, owner.clone(), plan.plan_id, key("install"), 2)
        .await
        .expect("accept extension install");
    M6Store::begin_apply(store, accepted.operation_id, 3)
        .await
        .expect("begin");
    M6Store::finish_install(
        store,
        accepted.operation_id,
        "<FIXTURE>/installed".to_owned(),
        4,
    )
    .await
    .expect("finish without filesystem IO");
    M6Store::complete_operation(store, accepted.operation_id, 5)
        .await
        .expect("complete");
    M6Store::get_extension(store, owner, "dev.example.live".to_owned())
        .await
        .expect("installed")
}

#[tokio::test]
async fn extension_changes_of_another_owner_do_not_invalidate_local_resources() {
    let directory = Directory::new();
    let store = StateStoreHandle::open(directory.database()).expect("isolated store");
    let owner = PrincipalId::parse("test:other-extension-owner").expect("foreign owner");
    let installed = installed_extension_for(&store, owner.clone()).await;
    let mut commits = store.event_commits();
    commits.borrow_and_update();
    M6Store::disable(
        &store,
        owner,
        installed.extension_id,
        installed.revision,
        key("foreign-disable"),
        6,
    )
    .await
    .expect("foreign owner commit");
    StateStore::list_operations(&store, PrincipalId::local_owner(), None, 100, false)
        .await
        .expect("fence");
    assert!(
        !commits
            .has_changed()
            .expect("foreign resource changes do not wake local owner")
    );
    assert!(
        store
            .list_events(PrincipalId::local_owner(), 0, 100)
            .await
            .expect("owner filtered")
            .events
            .is_empty()
    );
    assert_eq!(commits.borrow().resources_revision, 0);
}

#[tokio::test]
async fn extension_commits_invalidate_without_fabricating_persistent_events() {
    let directory = Directory::new();
    let store = StateStoreHandle::open(directory.database()).expect("isolated store");
    let installed = installed_extension(&store).await;
    let app = Application::new(store.clone());
    let access = AccessContext::local_owner();
    let mut subscription = app
        .subscribe_events(&access, None, 100, false)
        .expect("subscribe");
    let old_cursor = subscription.baseline_cursor.clone();
    let mut commits = store.event_commits();
    commits.borrow_and_update();
    let grant = ExtensionGrantMutation {
        extension_id: installed.extension_id.clone(),
        permission: "background.run".to_owned(),
        resource_kind: "Extension".to_owned(),
        resource_id: installed.extension_id.clone(),
        expected_revision: installed.grant_revision,
        grant: true,
    };
    M6Store::set_grant(
        &store,
        PrincipalId::local_owner(),
        grant.clone(),
        key("grant"),
        6,
    )
    .await
    .expect("grant commit");
    let page = subscription.next_page().await.expect("resource-only page");
    assert!(page.events.is_empty());
    assert_eq!(page.next_sequence, old_cursor.sequence);
    assert_eq!(page.invalidated_scopes, vec![EventInvalidation::Extensions]);
    assert_eq!(page.resources_revision, old_cursor.resources_revision + 1);
    assert!(
        subscription
            .advance(page.next_sequence, page.resources_revision + 1)
            .is_err()
    );
    subscription
        .advance(page.next_sequence, page.resources_revision)
        .expect("resource-only ACK");
    assert!(
        subscription
            .advance(page.next_sequence, page.resources_revision)
            .is_err()
    );
    commits.borrow_and_update();
    let replay = M6Store::set_grant(&store, PrincipalId::local_owner(), grant, key("grant"), 7)
        .await
        .expect("replay");
    assert!(replay.replayed);
    M6Store::mark_instance_stopped(&store, installed.extension_id.clone(), 8)
        .await
        .expect("no instance");
    StateStore::list_operations(&store, PrincipalId::local_owner(), None, 100, false)
        .await
        .expect("worker fence");
    assert!(!commits.has_changed().expect("worker live"));
    let reset = app
        .subscribe_events(&access, Some(old_cursor), 100, false)
        .expect("resource mismatch reset");
    assert!(reset.resync_required);
    assert_eq!(
        reset.baseline_cursor.resources_revision,
        page.resources_revision
    );
    let resume = app
        .subscribe_events(&access, Some(reset.baseline_cursor.clone()), 100, false)
        .expect("valid resource cursor");
    assert!(!resume.resync_required);
    let current = M6Store::get_extension(
        &store,
        PrincipalId::local_owner(),
        installed.extension_id.clone(),
    )
    .await
    .expect("current");
    let enabled = M6Store::enable(
        &store,
        PrincipalId::local_owner(),
        installed.extension_id.clone(),
        current.revision,
        key("enable"),
        9,
    )
    .await
    .expect("enable");
    let page = subscription.next_page().await.expect("enable invalidation");
    assert!(page.events.is_empty());
    assert_eq!(page.next_sequence, reset.baseline_cursor.sequence);
    subscription
        .advance(page.next_sequence, page.resources_revision)
        .expect("enable ACK");
    let replay = M6Store::enable(
        &store,
        PrincipalId::local_owner(),
        installed.extension_id.clone(),
        current.revision,
        key("enable"),
        10,
    )
    .await
    .expect("enable replay");
    assert_eq!(replay, enabled);
    commits.borrow_and_update();
    StateStore::list_operations(&store, PrincipalId::local_owner(), None, 100, false)
        .await
        .expect("fence");
    assert!(!commits.has_changed().expect("replay is not a change"));
    // A real runtime transition invalidates; lease renewal remains quiet.
    let start = M6Store::prepare_instance(
        &store,
        PrincipalId::local_owner(),
        installed.extension_id.clone(),
        OperationId::new().to_string(),
        11,
    )
    .await
    .expect("prepare runtime");
    M6Store::mark_instance_running(&store, start.lease.clone(), 12)
        .await
        .expect("running");
    let page = subscription
        .next_page()
        .await
        .expect("runtime invalidation");
    subscription
        .advance(page.next_sequence, page.resources_revision)
        .expect("runtime ACK");
    commits.borrow_and_update();
    M6Store::renew_instance(&store, start.lease, 13)
        .await
        .expect("lease heartbeat");
    StateStore::list_operations(&store, PrincipalId::local_owner(), None, 100, false)
        .await
        .expect("fence");
    assert!(
        !commits
            .has_changed()
            .expect("heartbeat is not visible state")
    );
    // The real worker reports a commit even after its reply receiver is dropped.
    let (reply, response) = oneshot::channel();
    drop(response);
    let commands = store
        .inner
        .commands
        .lock()
        .expect("sender")
        .as_ref()
        .expect("live")
        .clone();
    commands
        .send(Command::M3(Box::new(move |connection| {
            let result = m6::set_desired(
                connection,
                &PrincipalId::local_owner(),
                &enabled.extension_id,
                enabled.revision,
                &key("disable-dropped"),
                false,
                14,
            );
            let changed = result.as_ref().is_ok_and(|(_, changed)| *changed);
            let _ = reply.send(result.map(|(record, _)| record));
            changed
        })))
        .await
        .expect("queued disable");
    tokio::time::timeout(Duration::from_secs(2), commits.changed())
        .await
        .expect("wakeup")
        .expect("worker live");
    let page = subscription
        .next_page()
        .await
        .expect("dropped-caller invalidation");
    assert_eq!(page.next_sequence, reset.baseline_cursor.sequence);
    assert!(page.events.is_empty());
    assert_eq!(page.invalidated_scopes, vec![EventInvalidation::Extensions]);
}

#[tokio::test]
async fn rollback_noop_and_reads_do_not_publish_commit_watermarks() {
    let directory = Directory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open isolated store");
    let mut commits = store.event_commits();
    commits.borrow_and_update();
    store.request_worker(|connection| {
        let tx = connection.transaction().map_err(|_| sqlite::unavailable())?;
        tx.execute("INSERT INTO events(event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,principal_id,occurred_at_ms,payload_json) VALUES(?1,'operation.created','operation',?2,1,'builtin:local-owner',1,'{}')",
            params![uuid::Uuid::new_v4().to_string(), OperationId::new().to_string()]).map_err(|_| sqlite::unavailable())?;
        tx.rollback().map_err(|_| sqlite::unavailable())
    }, sqlite::unavailable).await.expect("rolled back write");
    store
        .list_operations(PrincipalId::local_owner(), None, 100, false)
        .await
        .expect("read fence");
    assert!(!commits.has_changed().expect("live worker"));
    let created = store
        .create_state_check(PrincipalId::local_owner(), key("once"), 1)
        .await
        .expect("create");
    commits.changed().await.expect("commit signal");
    let sequence = commits.borrow_and_update().sequence;
    assert_eq!(sequence, 1);
    store
        .create_state_check(PrincipalId::local_owner(), key("once"), 2)
        .await
        .expect("idempotent replay");
    store
        .get_operation(PrincipalId::local_owner(), created.operation_id)
        .await
        .expect("read fence");
    assert!(!commits.has_changed().expect("live worker"));
    assert_eq!(commits.borrow().sequence, sequence);
}

#[tokio::test]
async fn dropped_reply_receiver_does_not_drop_a_committed_notification() {
    let directory = Directory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open isolated store");
    let mut commits = store.event_commits();
    commits.borrow_and_update();
    let (reply, response) = oneshot::channel();
    drop(response);
    // Enqueue the real command with its caller already gone. The worker must
    // commit and publish even though replying is impossible.
    let commands = store
        .inner
        .commands
        .lock()
        .expect("worker sender")
        .as_ref()
        .expect("live sender")
        .clone();
    commands
        .send(Command::CreateStateCheck {
            owner: PrincipalId::local_owner(),
            idempotency_key: key("dropped"),
            created_at_ms: 1,
            reply,
        })
        .await
        .expect("queued command");
    tokio::time::timeout(Duration::from_secs(2), commits.changed())
        .await
        .expect("prompt notification")
        .expect("worker alive");
    assert_eq!(commits.borrow_and_update().sequence, 1);
    assert_eq!(
        store
            .list_events(PrincipalId::local_owner(), 0, 100)
            .await
            .expect("durable event")
            .events
            .len(),
        1
    );
}

#[tokio::test]
async fn live_subscription_resumes_prefixes_and_resets_a_new_instance() {
    let directory = Directory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open isolated store");
    let app = Application::new(store.clone());
    let access = AccessContext::local_owner();
    let mut subscription = app
        .subscribe_events(&access, None, 100, false)
        .expect("subscribe");
    assert!(subscription.resync_required);
    let baseline = subscription.baseline_cursor.clone();
    for i in 0..3 {
        store
            .create_state_check(PrincipalId::local_owner(), key(&format!("live-{i}")), i)
            .await
            .expect("task commit");
    }
    let page = subscription.next_page().await.expect("live page");
    assert_eq!(page.events.len(), 3);
    subscription
        .advance(page.events[0].sequence, page.resources_revision)
        .expect("sent prefix only");
    let suffix = subscription
        .next_page()
        .await
        .expect("remainder is not skipped");
    assert_eq!(suffix.events.len(), 2);
    assert!(
        subscription
            .advance(suffix.next_sequence + 1, suffix.resources_revision)
            .is_err()
    );
    let mut resumed = app
        .subscribe_events(
            &access,
            Some(EventsCursor {
                instance_id: baseline.instance_id.clone(),
                sequence: page.events[0].sequence,
                resources_revision: baseline.resources_revision,
            }),
            100,
            false,
        )
        .expect("resume");
    assert!(!resumed.resync_required);
    assert_eq!(
        resumed
            .next_page()
            .await
            .expect("replayed suffix")
            .events
            .len(),
        2
    );
    let reset = app
        .subscribe_events(
            &access,
            Some(EventsCursor {
                instance_id: OperationId::new().to_string(),
                sequence: 0,
                resources_revision: 0,
            }),
            100,
            false,
        )
        .expect("new instance reset");
    assert!(reset.resync_required);
    assert_eq!(reset.baseline_cursor.sequence, 3);
    let denied = AccessContext::new(
        PrincipalId::parse("test:other").expect("other owner"),
        [Permission::EventsRead],
    );
    assert!(matches!(
        app.subscribe_events(&denied, None, 100, false),
        Err(alcomd_application::ApplicationError::PermissionDenied)
    ));
    let denied = AccessContext::new(PrincipalId::local_owner(), []);
    assert!(matches!(
        app.subscribe_events(&denied, None, 100, false),
        Err(alcomd_application::ApplicationError::PermissionDenied)
    ));
}

#[tokio::test]
async fn active_only_pages_find_old_tasks_and_counts_ignore_page_and_owner() {
    let directory = Directory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open isolated store");
    let owner = PrincipalId::local_owner();
    let mut active = vec![];
    for i in 0..125 {
        let operation = store
            .create_state_check(owner.clone(), key(&format!("history-{i}")), i)
            .await
            .expect("task");
        if i < 2 {
            active.push(operation.operation_id);
        } else {
            store
                .finish_cancelled(operation.operation_id, i + 1)
                .await
                .expect("terminal task");
        }
    }
    store
        .create_state_check(
            PrincipalId::parse("test:other").expect("owner"),
            key("other-active"),
            1,
        )
        .await
        .expect("other owned task");
    let recent = store
        .list_operations(owner.clone(), None, 100, false)
        .await
        .expect("history first page");
    assert_eq!(recent.operations.len(), 100);
    assert_eq!(recent.active_count, 2);
    assert!(
        recent
            .operations
            .iter()
            .all(|item| !active.contains(&item.operation_id))
    );
    let first = store
        .list_operations(owner.clone(), None, 1, true)
        .await
        .expect("active first");
    assert_eq!(first.operations[0].operation_id, active[1]);
    assert_eq!(first.active_count, 2);
    let second = store
        .list_operations(owner, first.next_cursor, 1, true)
        .await
        .expect("active next");
    assert_eq!(second.operations[0].operation_id, active[0]);
    assert_eq!(second.active_count, 2);
}

#[tokio::test]
async fn event_page_byte_budget_keeps_a_replayable_prefix() {
    let directory = Directory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open isolated store");
    store.request_worker(|connection| {
        let transaction = connection.transaction().map_err(|_| sqlite::unavailable())?;
        let payload = format!("{{\"text\":\"{}\"}}", "a".repeat(60_000));
        for i in 0..80 {
            transaction.execute("INSERT INTO events(event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,principal_id,occurred_at_ms,payload_json) VALUES(?1,'operation.created','operation',?2,1,'builtin:local-owner',?3,?4)",
                params![uuid::Uuid::new_v4().to_string(), OperationId::new().to_string(), i, payload]).map_err(|_| sqlite::unavailable())?;
        }
        transaction.commit().map_err(|_| sqlite::unavailable())
    }, sqlite::unavailable).await.expect("bounded large fixture");
    let first = store
        .list_events(PrincipalId::local_owner(), 0, 1000)
        .await
        .expect("bounded prefix");
    assert!(!first.events.is_empty() && first.events.len() < 80);
    assert!(
        first
            .events
            .iter()
            .map(|item| item.payload_json.len())
            .sum::<usize>()
            < 4 * 1024 * 1024
    );
    let second = store
        .list_events(PrincipalId::local_owner(), first.next_sequence, 1000)
        .await
        .expect("remaining suffix");
    assert_eq!(first.events.len() + second.events.len(), 80);
}

#[tokio::test]
async fn public_package_phase_is_atomic_and_duplicate_phase_is_not_a_new_change() {
    let directory = Directory::new();
    let mut connection =
        sqlite::initialize_connection(&directory.database()).expect("isolated database");
    let operation = OperationId::new();
    let plan = PlanId::new();
    let project = ProjectId::new();
    connection.execute("INSERT INTO projects(project_id,owner_principal_id,root_path,path_identity_key,project_type,unity_version,snapshot_json,revision,registered_at_ms,updated_at_ms) VALUES(?1,'builtin:local-owner',?2,x'01','unknown','2022.3.22f1','{}',1,1,1)", params![project.to_string(), directory.0.to_string_lossy()]).expect("project fixture");
    connection.execute("INSERT INTO operations(operation_id,kind,state,revision,owner_principal_id,request_json,created_at_ms,updated_at_ms) VALUES(?1,'packages.apply','running',1,'builtin:local-owner','{}',1,1)", [operation.to_string()]).expect("task fixture");
    connection.execute("INSERT INTO package_plans(plan_id,owner_principal_id,project_id,action,state,project_revision,project_snapshot_fingerprint,change_set_fingerprint,change_set_json,source_set_json,apply_operation_id,created_at_ms) VALUES(?1,'builtin:local-owner',?2,'install','applied',1,zeroblob(32),zeroblob(32),'{\"formatVersion\":1,\"mutations\":[],\"dependencyEdges\":[]}','[]',?3,1)", params![plan.to_string(), project.to_string(), operation.to_string()]).expect("bound plan fixture");
    let entry = |step, phase| FilesystemJournalEntry {
        operation_id: operation,
        step,
        plan_id: plan,
        project_id: project,
        phase,
        state: JournalState::Completed,
        project_identity_key: vec![1],
        change_set_fingerprint: [0; 32],
        evidence_json: "{}".to_owned(),
        updated_at_ms: step + 1,
    };
    m4::append_filesystem_journal(&mut connection, entry(1, FilesystemPhase::ArchiveReady))
        .expect("phase change");
    let get = |connection: &rusqlite::Connection| {
        sqlite::load_owned_operation(connection, &PrincipalId::local_owner(), operation)
            .expect("public task")
    };
    assert_eq!(get(&connection).revision.get(), 2);
    assert_eq!(
        get(&connection).progress_phase,
        Some(FilesystemPhase::ArchiveReady)
    );
    m4::append_filesystem_journal(&mut connection, entry(1, FilesystemPhase::ArchiveReady))
        .expect("same immutable row replay");
    m4::append_filesystem_journal(&mut connection, entry(2, FilesystemPhase::ArchiveReady))
        .expect("same phase internal step");
    assert_eq!(get(&connection).revision.get(), 2);
    m4::append_filesystem_journal(&mut connection, entry(3, FilesystemPhase::Extracted))
        .expect("next public phase");
    assert_eq!(get(&connection).revision.get(), 3);
    let events =
        sqlite::list_events(&connection, &PrincipalId::local_owner(), 0, 100).expect("events");
    assert_eq!(events.events.len(), 2);
    assert!(
        events
            .events
            .iter()
            .filter(|item| item.aggregate_kind == "operation")
            .all(|item| item.kind == "operation.progress")
    );
    connection
        .execute(
            "UPDATE operations SET revision=9223372036854775807 WHERE operation_id=?1",
            [operation.to_string()],
        )
        .expect("inject revision exhaustion");
    assert!(
        m4::append_filesystem_journal(&mut connection, entry(4, FilesystemPhase::Prepared))
            .is_err()
    );
    let journals: i64 = connection
        .query_row(
            "SELECT count(*) FROM package_filesystem_journal",
            [],
            |row| row.get(0),
        )
        .expect("atomic rollback");
    assert_eq!(journals, 3);
    assert_eq!(
        get(&connection).progress_phase,
        Some(FilesystemPhase::Extracted)
    );
    assert_eq!(
        sqlite::list_events(&connection, &PrincipalId::local_owner(), 0, 100)
            .expect("unchanged events")
            .events
            .len(),
        2
    );
    drop(connection);
    let store = StateStoreHandle::open(directory.database()).expect("reopen fixture");
    let visible = store
        .get_operation(PrincipalId::local_owner(), operation)
        .await
        .expect("task remains readable");
    assert_eq!(visible.progress_phase, Some(FilesystemPhase::Extracted));
}

#[test]
fn restore_stages_publish_revisions_once_without_an_ipc_endpoint_or_project_io() {
    use alcomd_application::{
        BackupRestorePhase, BackupRestorePlanDraft, BackupRestoreTarget, ManifestState, ProjectType,
    };
    let directory = Directory::new();
    let mut connection =
        sqlite::initialize_connection(&directory.database()).expect("isolated database");
    let owner = PrincipalId::local_owner();
    let backup = BackupId::new();
    connection.execute("INSERT INTO backups(backup_id,owner_principal_id,source_project_id,archive_locator,file_identity_key,archive_sha256,byte_size,format_version,created_at_ms,compression_mode,exclude_vpm_packages) VALUES(?1,'builtin:local-owner',NULL,'backup:isolated-stages',x'01',zeroblob(32),42,1,1,'fast',1)", [backup.to_string()]).expect("backup metadata fixture");
    let project = ProjectId::new();
    let plan = m5_backup_restore::create_plan(&mut connection, &owner, BackupRestorePlanDraft {
        plan_id: PlanId::new(), project_id: project, backup_id: backup,
        archive_sha256: [0;32], archive_file_identity: vec![1], archive_bytes: 42,
        manifest_fingerprint: [0;32], exclude_vpm_packages: true, excluded_packages: vec![],
        target: BackupRestoreTarget { parent: directory.0.to_string_lossy().into_owned(), leaf: "Restored".to_owned(), must_be_absent: true },
        target_parent_identity: vec![2], expected_unity_project_json: "{}".to_owned(),
        plan_fingerprint: [0;32], plan_json: serde_json::json!({"version":1,"backupId":backup,"preallocatedProjectId":project,"targetMustBeAbsent":true}).to_string(),
    }, 1).expect("immutable plan fixture");
    let accepted = m5_backup_restore::accept(
        &mut connection,
        &owner,
        plan.draft.plan_id,
        &key("restore-stages"),
        2,
    )
    .expect("accept restore");
    let operation = accepted.operation_id;
    m5_backup_restore::begin(&mut connection, operation, 3).expect("start restore");
    let restored = RestoredProject {
        project_id: project,
        observation: ProjectObservation {
            root_path: directory.0.join("Restored").to_string_lossy().into_owned(),
            path_identity_key: vec![3],
            project_type: ProjectType::Unknown,
            unity_version: "2022.3.22f1".to_owned(),
            unity_revision: None,
            vpm_manifest: ManifestState::Missing,
            upm_manifest: ManifestState::Missing,
            direct_dependencies: vec![],
            locked_dependencies: vec![],
            issues: vec![],
        },
        target_identity: vec![3],
        project_fingerprint: [0; 32],
    };
    let phases = [
        BackupRestorePhase::ArchiveVerified,
        BackupRestorePhase::Extracting,
        BackupRestorePhase::StagingComplete,
        BackupRestorePhase::PublishIntent,
        BackupRestorePhase::TargetPublished,
        BackupRestorePhase::ProjectRegistryCommitIntent,
    ];
    for (index, phase) in phases.into_iter().enumerate() {
        m5_backup_restore::checkpoint(
            &mut connection,
            operation,
            phase,
            Some(restored.clone()),
            index as u64 + 4,
        )
        .expect("public checkpoint");
        let revision = sqlite::load_owned_operation(&connection, &owner, operation)
            .expect("task")
            .revision;
        assert_eq!(revision.get(), index as u64 + 3);
        m5_backup_restore::checkpoint(
            &mut connection,
            operation,
            phase,
            Some(restored.clone()),
            index as u64 + 4,
        )
        .expect("replayed checkpoint");
        assert_eq!(
            sqlite::load_owned_operation(&connection, &owner, operation)
                .expect("task")
                .revision,
            revision
        );
    }
    m5_backup_restore::complete(&mut connection, operation, restored.clone(), 11)
        .expect("registry commit");
    assert_eq!(
        sqlite::load_owned_operation(&connection, &owner, operation)
            .expect("task")
            .revision
            .get(),
        9
    );
    m5_backup_restore::complete(&mut connection, operation, restored, 12)
        .expect("repeat registry commit");
    assert_eq!(
        sqlite::load_owned_operation(&connection, &owner, operation)
            .expect("task")
            .revision
            .get(),
        9
    );
    m5_backup_restore::finish_success(&mut connection, operation, 13).expect("finish");
    m5_backup_restore::finish_success(&mut connection, operation, 14).expect("repeat finish");
    let complete =
        sqlite::load_owned_operation(&connection, &owner, operation).expect("terminal task");
    assert_eq!(complete.revision.get(), 10);
    assert_eq!(
        complete.state,
        alcomd_application::OperationState::Succeeded
    );
    let page = sqlite::list_events(&connection, &owner, 0, 100).expect("persisted changes");
    let revisions = page
        .events
        .iter()
        .filter(|item| item.aggregate_kind == "operation")
        .map(|item| item.aggregate_revision.get())
        .collect::<Vec<_>>();
    assert_eq!(revisions, (1..=10).collect::<Vec<_>>());
    assert_eq!(
        page.events
            .iter()
            .filter(|item| item.kind == "operation.progress")
            .count(),
        7
    );
    assert!(
        !directory.0.join("Restored").exists(),
        "store test must not restore actual files"
    );
}
