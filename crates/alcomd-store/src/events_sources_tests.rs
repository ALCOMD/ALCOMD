use super::events_tests::{Directory, installed_extension_for};
use super::*;
use alcomd_application::{
    AccessContext, Application, EventInvalidation, ExtensionDataDisposition, ExtensionDesiredState,
    ManifestState, OfficialGuiStore, ProjectType, RepositorySource, UnityArchitecture,
    UnityMigrationClassificationKind, UnitySourceKind, UnityWriterState, UnityWriterStateKind,
};
use rusqlite::{Connection, params};

fn key(value: &str) -> IdempotencyKey {
    IdempotencyKey::parse(value).expect("fixture key")
}

#[tokio::test]
async fn beginning_a_real_uninstall_publishes_extensions_invalidation_only_for_its_owner() {
    for owner in [
        PrincipalId::local_owner(),
        PrincipalId::parse("test:other").expect("owner"),
    ] {
        let directory = Directory::new();
        let store = StateStoreHandle::open(directory.database()).expect("isolated store");
        let extension = installed_extension_for(&store, owner.clone()).await;
        let plan = M6Store::create_uninstall_plan(
            &store,
            owner.clone(),
            ExtensionUninstallPlanDraft {
                extension: extension.clone(),
                data_disposition: ExtensionDataDisposition::RetainData,
                plan_fingerprint: [9; 32],
            },
            10,
        )
        .await
        .expect("uninstall plan");
        let accepted =
            M6Store::accept_plan(&store, owner.clone(), plan.plan_id, key("uninstall"), 11)
                .await
                .expect("accept uninstall");
        StateStore::list_events(&store, owner.clone(), 0, 100)
            .await
            .expect("commit fence");
        let mut commits = store.event_commits();
        assert_eq!(
            commits.borrow_and_update().resources_revision,
            0,
            "install begin has no installed resource transition"
        );
        let app = Application::new(store.clone());
        let mut subscription = app
            .subscribe_events(&AccessContext::local_owner(), None, 100)
            .expect("subscribe");
        M6Store::begin_apply(&store, accepted.operation_id, 12)
            .await
            .expect("uninstall transition");
        let current = M6Store::get_extension(&store, owner.clone(), extension.extension_id)
            .await
            .expect("committed extension");
        assert_eq!(current.desired_state, ExtensionDesiredState::Uninstalling);
        if owner == PrincipalId::local_owner() {
            let page = subscription.next_page().await.expect("uninstall signal");
            assert_eq!(page.invalidated_scopes, [EventInvalidation::Extensions]);
            assert_eq!(page.resources_revision, 1);
            assert_eq!(page.events.len(), 2);
            assert_eq!(page.events[0].kind, "activity.changed");
            assert_eq!(page.events[1].kind, "operation.state_changed");
        } else {
            assert_eq!(
                commits.borrow().resources_revision,
                0,
                "foreign changes do not invalidate local extensions"
            );
        }
        commits.borrow_and_update();
        M6Store::begin_apply(&store, accepted.operation_id, 13)
            .await
            .expect("already running replay");
        StateStore::list_events(&store, owner, 0, 100)
            .await
            .expect("commit fence");
        assert!(!commits.has_changed().expect("repeated begin is quiet"));
    }
}

pub(super) fn project_observation() -> ProjectObservation {
    ProjectObservation {
        root_path: "<FIXTURE>/Project".to_owned(),
        path_identity_key: vec![1],
        project_type: ProjectType::Avatars,
        unity_version: "2022.3.22f1".to_owned(),
        unity_revision: None,
        vpm_manifest: ManifestState::Valid,
        upm_manifest: ManifestState::Valid,
        direct_dependencies: vec![],
        locked_dependencies: vec![],
        issues: vec![],
    }
}

fn repository_observation() -> RepositoryObservation {
    RepositoryObservation {
        source: RepositorySource::Remote {
            url: "https://example.invalid/vpm.json".to_owned(),
        },
        source_identity_key: vec![2],
        declared_id: None,
        name: Some("Fixture".to_owned()),
        declared_url: None,
        issues: vec![],
        packages: vec![],
        validators: RepositoryValidators::default(),
        refreshed_at_ms: 1,
    }
}

fn staged_operation(connection: &mut Connection, kind: &str) -> OperationId {
    let owner = PrincipalId::local_owner();
    let project = m3::register_project(
        connection,
        &owner,
        project_observation(),
        &key("project"),
        1,
    )
    .expect("project fixture")
    .value;
    let writer = UnityWriterState {
        project_id: project.project_id,
        state: UnityWriterStateKind::NotObserved,
        evidence: vec![],
        checked_at_ms: 1,
    };
    match kind {
        "copy" => {
            let plan = m7_copy::create_plan(
                connection,
                &owner,
                ProjectCopyPlanDraft {
                    plan_id: PlanId::new(),
                    source_project: project.clone(),
                    source_root_identity: vec![1],
                    target_parent_path: "<FIXTURE>".to_owned(),
                    target_parent_identity: vec![2],
                    target_parent_identity_sha256: [2; 32],
                    target_leaf: "Copy".to_owned(),
                    target_project_id: ProjectId::new(),
                    writer_evidence: writer,
                    profile_version: 1,
                    plan_fingerprint: [3; 32],
                    plan_json: "{}".to_owned(),
                    plan_idempotency_key: key("plan"),
                    created_at_ms: 1,
                    expires_at_ms: 900_001,
                },
            )
            .expect("copy plan")
            .plan;
            m7_copy::accept(
                connection,
                &owner,
                plan.draft.plan_id,
                project.revision,
                &key("apply"),
                2,
            )
            .expect("accept copy")
            .operation_id
        }
        "delete" => {
            let plan = m7_delete::create_plan(
                connection,
                &owner,
                ProjectDeletePlanDraft {
                    plan_id: PlanId::new(),
                    project: project.clone(),
                    root_identity: vec![1],
                    canonical_parent_path: "<FIXTURE>".to_owned(),
                    parent_identity: vec![2],
                    parent_identity_sha256: [2; 32],
                    normalized_leaf: "Project".to_owned(),
                    project_marker_sha256: [3; 32],
                    writer_evidence: writer,
                    profile_version: 1,
                    plan_fingerprint: [3; 32],
                    plan_json: "{}".to_owned(),
                    plan_idempotency_key: key("plan"),
                    created_at_ms: 1,
                    expires_at_ms: 900_001,
                },
            )
            .expect("delete plan")
            .plan;
            m7_delete::accept(
                connection,
                &owner,
                plan.draft.plan_id,
                project.revision,
                &key("apply"),
                2,
            )
            .expect("accept delete")
            .operation_id
        }
        "migration" => {
            let installation = m5::register_installation(
                connection,
                &owner,
                UnityInstallationObservation {
                    executable_path: "<FIXTURE>/Unity".to_owned(),
                    filesystem_identity: vec![3],
                    unity_version: "2022.3.23f1".to_owned(),
                    architecture: UnityArchitecture::Unknown,
                    source_kind: UnitySourceKind::Manual,
                    observed_at_ms: 1,
                },
                &key("installation"),
                1,
            )
            .expect("installation metadata")
            .0;
            let plan = PlanId::new();
            m7_unity_migration::create_plan(
                connection,
                &owner,
                UnityMigrationPlanDraft {
                    plan_id: plan,
                    project,
                    source_unity_version: "2022.3.22f1".to_owned(),
                    source_revision_metadata: None,
                    project_root_identity: vec![1],
                    project_version_marker_sha256: [1; 32],
                    target_unity_version: "2022.3.23f1".to_owned(),
                    target_revision_metadata: None,
                    target_installation: installation,
                    writer_evidence: writer,
                    classification: UnityMigrationClassificationKind::PatchOrMinorUpgrade,
                    preparation_profile: None,
                    plan_fingerprint: "a".repeat(64),
                    request_fingerprint: "{\"version\":1}".to_owned(),
                    plan_idempotency_key: key("plan"),
                    created_at_ms: 1,
                    expires_at_ms: 1000,
                },
            )
            .expect("migration plan");
            m7_unity_migration::accept(connection, &owner, plan, &key("apply"), 2)
                .expect("accept migration")
                .operation_id
        }
        _ => unreachable!("known staged kind"),
    }
}

fn begin_staged(connection: &mut Connection, kind: &str, operation: OperationId, time: u64) {
    match kind {
        "copy" => {
            m7_copy::begin_operation(connection, operation, time).expect("copy begin");
        }
        "delete" => {
            m7_delete::begin_operation(connection, operation, time).expect("delete begin");
        }
        "migration" => {
            m7_unity_migration::begin_operation(connection, operation, time)
                .expect("migration begin");
        }
        _ => unreachable!("known staged kind"),
    }
}

fn checkpoint_staged(
    connection: &mut Connection,
    kind: &str,
    operation: OperationId,
    second: bool,
    time: u64,
) -> bool {
    match kind {
        "copy" => m7_copy::checkpoint(
            connection,
            operation,
            if second {
                ProjectCopyPhase::StagingComplete
            } else {
                ProjectCopyPhase::Staging
            },
            None,
            None,
            time,
        )
        .is_ok(),
        "delete" => m7_delete::checkpoint(
            connection,
            operation,
            if second {
                ProjectDeletePhase::QuarantineIntent
            } else {
                ProjectDeletePhase::PreflightComplete
            },
            ProjectDeleteFilesystemEvidence {
                quarantine_locator: "quarantine:fixture".to_owned(),
                quarantine_identity: Some(vec![3]),
                entry_count: Some(time),
                safe_evidence: vec![],
            },
            time,
        )
        .is_ok(),
        "migration" => m7_unity_migration::checkpoint(
            connection,
            operation,
            if second {
                UnityMigrationPhase::PreparationIntent
            } else {
                UnityMigrationPhase::PreflightComplete
            },
            UnityMigrationEvidence {
                safe_evidence: vec![time.to_string()],
                ..Default::default()
            },
            time,
            None,
        )
        .is_ok(),
        _ => unreachable!("known staged kind"),
    }
}

#[test]
fn migration_checkpoint_diagnostic_updates_task_and_activity_atomically_without_duplicate_replay() {
    let directory = Directory::new();
    let mut connection =
        sqlite::initialize_connection(&directory.database()).expect("isolated database");
    let owner = PrincipalId::local_owner();
    let operation = staged_operation(&mut connection, "migration");
    begin_staged(&mut connection, "migration", operation, 3);
    m7_unity_migration::checkpoint(
        &mut connection,
        operation,
        UnityMigrationPhase::LaunchIntent,
        UnityMigrationEvidence::default(),
        4,
        None,
    )
    .expect("persist public launch intent");
    let before = sqlite::load_owned_operation(&connection, &owner, operation)
        .expect("task before real spawn error");
    let activity_before =
        activity::get(&connection, &owner, &operation.to_string()).expect("matching task activity");
    let diagnostic = uuid::Uuid::new_v4().to_string();
    connection
        .execute_batch("CREATE TEMP TRIGGER reject_activity_diagnostic BEFORE INSERT ON events WHEN NEW.kind='activity.changed' BEGIN SELECT RAISE(ABORT,'controlled activity event failure'); END;")
        .expect("inject activity event failure");
    assert!(
        m7_unity_migration::checkpoint(
            &mut connection,
            operation,
            UnityMigrationPhase::LaunchIntent,
            UnityMigrationEvidence::default(),
            5,
            Some(diagnostic.clone()),
        )
        .is_err()
    );
    assert_eq!(
        sqlite::load_owned_operation(&connection, &owner, operation)
            .expect("task rollback")
            .revision,
        before.revision
    );
    assert_eq!(
        activity::get(&connection, &owner, &operation.to_string()).expect("activity rollback"),
        activity_before
    );
    connection
        .execute_batch("DROP TRIGGER reject_activity_diagnostic")
        .expect("release injected failure");
    m7_unity_migration::checkpoint(
        &mut connection,
        operation,
        UnityMigrationPhase::LaunchIntent,
        UnityMigrationEvidence::default(),
        6,
        Some(diagnostic.clone()),
    )
    .expect("commit real error association without changing launch phase");
    let task = sqlite::load_owned_operation(&connection, &owner, operation)
        .expect("committed diagnostic task");
    let activity = activity::get(&connection, &owner, &operation.to_string())
        .expect("committed diagnostic activity");
    assert_eq!(task.revision.get(), before.revision.get() + 1);
    assert_eq!(task.diagnostic_id.as_deref(), Some(diagnostic.as_str()));
    assert_eq!(activity.diagnostic_id, task.diagnostic_id);
    assert_eq!(activity.revision, activity_before.revision + 1);
    let events_before_replay =
        sqlite::list_events(&connection, &owner, 0, 100).expect("diagnostic commit events");
    for diagnostic_id in [None, Some(diagnostic)] {
        m7_unity_migration::checkpoint(
            &mut connection,
            operation,
            UnityMigrationPhase::LaunchIntent,
            UnityMigrationEvidence::default(),
            7,
            diagnostic_id,
        )
        .expect("same phase and same diagnostic are publicly quiet");
    }
    assert_eq!(
        sqlite::load_owned_operation(&connection, &owner, operation)
            .expect("replayed task")
            .revision,
        task.revision
    );
    assert_eq!(
        activity::get(&connection, &owner, &operation.to_string()).expect("replayed activity"),
        activity
    );
    assert_eq!(
        sqlite::list_events(&connection, &owner, 0, 100)
            .expect("replay events")
            .events,
        events_before_replay.events
    );
}

#[test]
fn successful_task_logs_only_after_commit_and_not_on_replay() {
    struct CountSuccess(std::sync::Arc<std::sync::atomic::AtomicUsize>);
    impl tracing::Subscriber for CountSuccess {
        fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
            *metadata.level() == tracing::Level::INFO
        }
        fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
            tracing::span::Id::from_u64(1)
        }
        fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
        fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
        fn event(&self, event: &tracing::Event<'_>) {
            struct SuccessMessage(bool);
            impl tracing::field::Visit for SuccessMessage {
                fn record_debug(
                    &mut self,
                    field: &tracing::field::Field,
                    value: &dyn std::fmt::Debug,
                ) {
                    if field.name() == "message" && format!("{value:?}") == "operation completed" {
                        self.0 = true;
                    }
                }
            }
            let mut message = SuccessMessage(false);
            event.record(&mut message);
            if message.0 {
                self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }
        fn enter(&self, _: &tracing::span::Id) {}
        fn exit(&self, _: &tracing::span::Id) {}
    }
    let directory = Directory::new();
    let mut connection =
        sqlite::initialize_connection(&directory.database()).expect("isolated database");
    let operation = staged_operation(&mut connection, "migration");
    begin_staged(&mut connection, "migration", operation, 3);
    m7_unity_migration::checkpoint(
        &mut connection,
        operation,
        UnityMigrationPhase::CleanupComplete,
        UnityMigrationEvidence::default(),
        4,
        None,
    )
    .expect("cleanup ready");
    let count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    tracing::subscriber::with_default(CountSuccess(std::sync::Arc::clone(&count)), || {
        connection.execute_batch("CREATE TEMP TRIGGER reject_success BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT,'controlled commit failure'); END;").expect("inject failed transaction");
        assert!(m7_unity_migration::finish_success(&mut connection, operation, 5).is_err());
        assert_eq!(
            count.load(std::sync::atomic::Ordering::Relaxed),
            0,
            "rollback is not successful execution"
        );
        connection
            .execute_batch("DROP TRIGGER reject_success")
            .expect("release failure");
        m7_unity_migration::finish_success(&mut connection, operation, 6)
            .expect("real success commit");
        assert_eq!(count.load(std::sync::atomic::Ordering::Relaxed), 1);
        m7_unity_migration::finish_success(&mut connection, operation, 7).expect("terminal replay");
        assert_eq!(
            count.load(std::sync::atomic::Ordering::Relaxed),
            1,
            "terminal replay does not make another technical record"
        );
    });
}

#[test]
fn public_stage_replay_is_quiet_while_internal_journals_and_rollback_remain_correct() {
    for (kind, journal) in [
        ("copy", "project_copy_filesystem_journal"),
        ("delete", "project_delete_filesystem_journal"),
        ("migration", "project_unity_migration_journal"),
    ] {
        let directory = Directory::new();
        let mut connection =
            sqlite::initialize_connection(&directory.database()).expect("isolated database");
        let owner = PrincipalId::local_owner();
        let operation = staged_operation(&mut connection, kind);
        begin_staged(&mut connection, kind, operation, 3);
        let begun = sqlite::load_owned_operation(&connection, &owner, operation).expect("begun");
        begin_staged(&mut connection, kind, operation, 4);
        assert_eq!(
            sqlite::load_owned_operation(&connection, &owner, operation)
                .expect("begin replay")
                .revision,
            begun.revision,
            "{kind}: repeated begin"
        );
        assert!(
            checkpoint_staged(&mut connection, kind, operation, false, 5),
            "{kind}: phase change"
        );
        let public =
            sqlite::load_owned_operation(&connection, &owner, operation).expect("public task");
        let event_count = sqlite::list_events(&connection, &owner, 0, 100)
            .expect("events")
            .events
            .len();
        let journal_count = |connection: &Connection| -> i64 {
            connection
                .query_row(&format!("SELECT count(*) FROM {journal}"), [], |row| {
                    row.get(0)
                })
                .expect("fixed fixture table")
        };
        let previous_journals = journal_count(&connection);
        assert!(
            checkpoint_staged(&mut connection, kind, operation, false, 6),
            "{kind}: repeated public phase"
        );
        assert_eq!(
            journal_count(&connection),
            previous_journals + 1,
            "{kind}: internal evidence retained"
        );
        let replay =
            sqlite::load_owned_operation(&connection, &owner, operation).expect("replayed stage");
        assert_eq!(replay.revision, public.revision, "{kind}");
        assert_eq!(replay.updated_at_ms, public.updated_at_ms, "{kind}");
        assert_eq!(
            sqlite::list_events(&connection, &owner, 0, 100)
                .expect("events")
                .events
                .len(),
            event_count,
            "{kind}"
        );
        connection.execute_batch("CREATE TEMP TRIGGER reject_stage BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT,'controlled event failure'); END;").expect("controlled failure");
        assert!(
            !checkpoint_staged(&mut connection, kind, operation, true, 7),
            "{kind}: event failure"
        );
        assert_eq!(
            journal_count(&connection),
            previous_journals + 1,
            "{kind}: journal rolled back"
        );
        assert_eq!(
            sqlite::load_owned_operation(&connection, &owner, operation)
                .expect("rollback")
                .revision,
            public.revision,
            "{kind}"
        );
        connection
            .execute_batch("DROP TRIGGER reject_stage")
            .expect("release");
        assert!(
            checkpoint_staged(&mut connection, kind, operation, true, 8),
            "{kind}: next phase"
        );
        if kind == "delete" {
            m7_delete::commit_registry(&mut connection, operation, 9).expect("registry commit");
            let committed = sqlite::load_owned_operation(&connection, &owner, operation)
                .expect("state committed");
            let events = sqlite::list_events(&connection, &owner, 0, 100)
                .expect("commit events")
                .events
                .len();
            m7_delete::commit_registry(&mut connection, operation, 10)
                .expect("registry commit replay");
            let repeated =
                sqlite::load_owned_operation(&connection, &owner, operation).expect("same commit");
            assert_eq!(repeated.revision, committed.revision);
            assert_eq!(repeated.updated_at_ms, committed.updated_at_ms);
            assert_eq!(
                sqlite::list_events(&connection, &owner, 0, 100)
                    .expect("no invented events")
                    .events
                    .len(),
                events
            );
        }
    }
}

#[tokio::test]
async fn accepted_launch_activity_is_owned_and_replay_safe_while_observation_noise_is_excluded() {
    // Isolated SQLite and Store worker requests: no network, Unity or project IO.
    let directory = Directory::new();
    let store = StateStoreHandle::open(directory.database()).expect("isolated store");
    let owner = PrincipalId::local_owner();
    let project = store
        .register_project(owner.clone(), project_observation(), key("project"), 1)
        .await
        .expect("registered fixture")
        .value;
    let repository = store
        .register_repository(
            owner.clone(),
            repository_observation(),
            key("repository"),
            1,
        )
        .await
        .expect("registered fixture")
        .value;
    store
        .list_events(owner.clone(), 0, 100)
        .await
        .expect("initial commit fence");
    let app = Application::new(store.clone());
    let mut subscription = app
        .subscribe_events(&AccessContext::local_owner(), None, 100)
        .expect("subscribe");
    let baseline = subscription.baseline_cursor.sequence;
    let config = ProjectUnityLaunchConfig {
        project_id: project.project_id,
        arguments: vec![],
        revision: None,
        updated_at_ms: 0,
    };
    let installation = UnityInstallationId::new();
    for (name, time) in [
        ("launch", 10),
        ("launch", 20),
        ("same-time", 10),
        ("old-time", 9),
    ] {
        store
            .accept_launch(
                owner.clone(),
                project.clone(),
                config.clone(),
                installation,
                key(name),
                time,
            )
            .await
            .expect("accepted launch metadata only");
    }
    let mut fresh = repository.observation.clone();
    fresh.refreshed_at_ms = 2;
    store
        .refresh_repository(
            owner.clone(),
            repository.repository_id,
            repository.revision,
            fresh.clone(),
            key("fresh"),
            2,
        )
        .await
        .expect("unchanged repository content, new public observation time");
    store
        .refresh_repository(
            owner.clone(),
            repository.repository_id,
            repository.revision,
            fresh,
            key("fresh"),
            3,
        )
        .await
        .expect("fresh replay");
    for (name, time) in [("304", 3), ("304", 4), ("private-validator", 3)] {
        store
            .update_repository_validators(
                owner.clone(),
                repository.repository_id,
                repository.revision,
                RepositoryValidators {
                    etag: Some(name.to_owned()),
                    last_modified: None,
                },
                key(name),
                time,
            )
            .await
            .expect("304 or private validator change");
    }
    let current = store
        .get_project(owner.clone(), project.project_id)
        .await
        .expect("current project");
    assert_eq!(current.last_modified_at_ms, Some(10));
    assert_eq!(current.revision, project.revision);
    let page = subscription
        .next_page()
        .await
        .expect("committed notifications");
    assert_eq!(
        page.events
            .iter()
            .map(|event| event.kind.as_str())
            .collect::<Vec<_>>(),
        [
            "project.activity-recorded",
            "activity.changed",
            "activity.changed",
            "activity.changed",
            "repository.observed",
            "repository.observed"
        ]
    );
    assert!(
        page.events
            .iter()
            .all(|event| event.aggregate_revision == Revision::INITIAL)
    );
    subscription
        .advance(page.next_sequence, page.resources_revision)
        .expect("ACK");
    assert!(
        store
            .list_events(
                PrincipalId::parse("test:other").expect("owner"),
                baseline,
                100
            )
            .await
            .expect("owner-filtered events")
            .events
            .is_empty()
    );
    let before_activity = store
        .list_official_activity(owner.clone(), None, 100)
        .await
        .expect("activity");
    assert_eq!(
        before_activity.items.len(),
        5,
        "accepted launch and original registrations remain visible"
    );
    let launch_activity = before_activity
        .items
        .iter()
        .filter(|item| item.action == "unity.launch")
        .collect::<Vec<_>>();
    assert_eq!(
        launch_activity.len(),
        3,
        "three distinct accepted requests, with same-key replay excluded"
    );
    assert_eq!(
        launch_activity[0].subject.id.as_deref(),
        Some(project.project_id.to_string().as_str())
    );
    assert_eq!(launch_activity[0].started_at_ms, 10);
    assert_eq!(launch_activity[0].status, "running");
    assert!(
        launch_activity[0].result.is_none(),
        "accepting a request is not spawning Unity"
    );
    assert!(
        store
            .list_official_activity(PrincipalId::parse("test:other").expect("owner"), None, 100)
            .await
            .expect("owner-filtered activity")
            .items
            .is_empty()
    );
    let mut content = repository.observation.clone();
    content.name = Some("Changed".to_owned());
    content.refreshed_at_ms = 4;
    store
        .refresh_repository(
            owner.clone(),
            repository.repository_id,
            repository.revision,
            content,
            key("content"),
            4,
        )
        .await
        .expect("semantic repository change");
    let page = subscription
        .next_page()
        .await
        .expect("content invalidation");
    assert_eq!(page.events.len(), 2);
    assert_eq!(page.events[0].kind, "repository.refreshed");
    assert_eq!(page.events[0].aggregate_revision.get(), 2);
    assert_eq!(page.events[1].kind, "activity.changed");
    // Raw task/progress events are no longer manufactured into user history.
    store.request_worker(|connection| {
        for kind in ["operation.progress", "operation.succeeded"] {
            connection.execute("INSERT INTO events(event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,principal_id,occurred_at_ms,payload_json) VALUES(?1,?2,'operation',?3,1,?4,5,'{}')",
                params![OperationId::new().to_string(),kind,OperationId::new().to_string(),PrincipalId::LOCAL_OWNER]).map_err(|_|sqlite::unavailable())?;
        }
        Ok(())
    },sqlite::unavailable).await.expect("internal event fixture");
    let activity = store
        .list_official_activity(owner, None, 100)
        .await
        .expect("durable user activity");
    assert_eq!(
        activity.items.len(),
        6,
        "registrations, accepted launch and real repository change only"
    );
    assert!(
        activity
            .items
            .iter()
            .all(|item| !item.action.starts_with("operation."))
    );
}

#[test]
fn metadata_event_failure_rolls_back_the_public_value_and_does_not_consume_the_key() {
    let directory = Directory::new();
    let mut connection =
        sqlite::initialize_connection(&directory.database()).expect("isolated database");
    let owner = PrincipalId::local_owner();
    let project = m3::register_project(
        &mut connection,
        &owner,
        project_observation(),
        &key("project"),
        1,
    )
    .expect("project")
    .value;
    let repository = m3::register_repository(
        &mut connection,
        &owner,
        repository_observation(),
        &key("repository"),
        1,
    )
    .expect("repository")
    .value;
    connection.execute_batch("CREATE TEMP TRIGGER reject_observation BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT,'controlled event failure'); END;")
        .expect("controlled failure");
    let config = ProjectUnityLaunchConfig {
        project_id: project.project_id,
        arguments: vec![],
        revision: None,
        updated_at_ms: 0,
    };
    assert!(
        m5::accept_launch(
            &mut connection,
            &owner,
            project.clone(),
            config.clone(),
            UnityInstallationId::new(),
            &key("launch"),
            10
        )
        .is_err()
    );
    assert!(
        m3::update_repository_validators(
            &mut connection,
            &owner,
            repository.repository_id,
            repository.revision,
            RepositoryValidators::default(),
            &key("304"),
            2
        )
        .is_err()
    );
    assert_eq!(
        m3::get_project(&connection, &owner, project.project_id)
            .expect("project rollback")
            .last_modified_at_ms,
        Some(1)
    );
    assert_eq!(
        m3::get_repository(&connection, &owner, repository.repository_id)
            .expect("repository rollback")
            .observation
            .refreshed_at_ms,
        1
    );
    connection
        .execute_batch("DROP TRIGGER reject_observation")
        .expect("release controlled failure");
    m5::accept_launch(
        &mut connection,
        &owner,
        project,
        config,
        UnityInstallationId::new(),
        &key("launch"),
        10,
    )
    .expect("failed key was not consumed");
    m3::update_repository_validators(
        &mut connection,
        &owner,
        repository.repository_id,
        repository.revision,
        RepositoryValidators::default(),
        &key("304"),
        2,
    )
    .expect("failed key was not consumed");
}

fn recover(connection: &mut Connection, kind: &str, time: u64) -> Result<Vec<OperationId>, String> {
    match kind {
        "state.check" => sqlite::recover(connection, time).map_err(|error| format!("{error:?}")),
        "packages.apply" => {
            m4::recover_package_operations(connection, time).map_err(|error| format!("{error:?}"))
        }
        "backups.create" => m5_backup::recover_backup_operations(connection, time)
            .map_err(|error| format!("{error:?}")),
        "backups.restore" => {
            m5_backup_restore::recover(connection, time).map_err(|error| format!("{error:?}"))
        }
        "templates.import" => m5_template::recover_template_operations(connection, time)
            .map_err(|error| format!("{error:?}")),
        "extensions.install" => {
            m6::recover_operations(connection, time).map_err(|error| format!("{error:?}"))
        }
        "projects.copy" => m7_copy::recover(connection, time).map_err(|error| format!("{error:?}")),
        "projects.delete-directory" => {
            m7_delete::recover(connection, time).map_err(|error| format!("{error:?}"))
        }
        "projects.unity-migration" => {
            m7_unity_migration::recover(connection, time).map_err(|error| format!("{error:?}"))
        }
        _ => unreachable!("known recovery kind"),
    }
}

#[test]
fn each_existing_recovery_commits_state_revision_and_event_atomically_and_repeats_quietly() {
    for kind in [
        "state.check",
        "packages.apply",
        "backups.create",
        "backups.restore",
        "templates.import",
        "extensions.install",
        "projects.copy",
        "projects.delete-directory",
        "projects.unity-migration",
    ] {
        let directory = Directory::new();
        let mut connection =
            sqlite::initialize_connection(&directory.database()).expect("isolated database");
        let owner = PrincipalId::parse("test:recovery-owner").expect("fixture owner");
        let operation = OperationId::new();
        connection.execute("INSERT INTO operations(operation_id,kind,state,revision,owner_principal_id,request_json,created_at_ms,updated_at_ms)
            VALUES(?1,?2,'running',1,?3,'{}',1,1)", params![operation.to_string(), kind, owner.as_str()]).expect("public task fixture");
        if kind == "state.check" {
            connection.execute("INSERT INTO operation_journal(operation_id,step,kind,state,payload_json,updated_at_ms) VALUES(?1,1,'state.check','prepared','{}',1)", [operation.to_string()]).expect("recovery journal");
        }
        if kind == "packages.apply" {
            let project = ProjectId::new();
            connection.execute("INSERT INTO projects(project_id,owner_principal_id,root_path,path_identity_key,project_type,unity_version,snapshot_json,revision,registered_at_ms,updated_at_ms) VALUES(?1,?2,'<FIXTURE>',x'01','unknown','2022.3.22f1','{}',1,1,1)", params![project.to_string(),owner.as_str()]).expect("project fixture");
            connection.execute("INSERT INTO package_plans(plan_id,owner_principal_id,project_id,action,state,project_revision,project_snapshot_fingerprint,change_set_fingerprint,change_set_json,source_set_json,apply_operation_id,created_at_ms) VALUES(?1,?2,?3,'install','applied',1,zeroblob(32),zeroblob(32),'{\"formatVersion\":1,\"mutations\":[],\"dependencyEdges\":[]}','[]',?4,1)", params![PlanId::new().to_string(), owner.as_str(), project.to_string(), operation.to_string()]).expect("bound plan");
        }
        // Generic recovery must leave every kind owned by a dedicated handler intact.
        if kind != "state.check" {
            assert!(
                sqlite::recover(&mut connection, 2)
                    .expect("generic recovery")
                    .is_empty(),
                "{kind}"
            );
            assert_eq!(
                sqlite::load_owned_operation(&connection, &owner, operation)
                    .expect("not stolen")
                    .revision
                    .get(),
                1,
                "{kind}"
            );
        }
        connection.execute_batch("CREATE TEMP TRIGGER reject_recovery BEFORE INSERT ON events BEGIN SELECT RAISE(ABORT,'controlled event failure'); END;").expect("controlled failure");
        assert!(
            recover(&mut connection, kind, 10).is_err(),
            "{kind}: event failure"
        );
        let unchanged =
            sqlite::load_owned_operation(&connection, &owner, operation).expect("rollback");
        assert_eq!(unchanged.revision.get(), 1, "{kind}: revision rolled back");
        assert_eq!(
            unchanged.state,
            alcomd_domain::OperationState::Running,
            "{kind}: state rolled back"
        );
        connection
            .execute_batch("DROP TRIGGER reject_recovery")
            .expect("release");
        assert_eq!(
            recover(&mut connection, kind, 10).expect(kind),
            [operation],
            "{kind}: recovery scheduling"
        );
        let first =
            sqlite::load_owned_operation(&connection, &owner, operation).expect("recovered task");
        assert_eq!(
            first.state,
            alcomd_domain::OperationState::Recovering,
            "{kind}"
        );
        let events =
            sqlite::list_events(&connection, &owner, 0, 100).expect("committed owner events");
        assert_eq!(
            events
                .events
                .last()
                .expect("recovery event")
                .aggregate_revision,
            first.revision,
            "{kind}"
        );
        assert!(
            sqlite::list_events(&connection, &PrincipalId::local_owner(), 0, 100)
                .expect("isolated owner")
                .events
                .is_empty()
        );
        assert_eq!(
            recover(&mut connection, kind, 20).expect(kind),
            [operation],
            "{kind}: recovering is still scheduled"
        );
        let repeated =
            sqlite::load_owned_operation(&connection, &owner, operation).expect("same task");
        assert_eq!(
            repeated.revision, first.revision,
            "{kind}: no duplicate revision"
        );
        assert_eq!(
            repeated.updated_at_ms, first.updated_at_ms,
            "{kind}: no duplicate public time"
        );
        assert_eq!(
            sqlite::list_events(&connection, &owner, 0, 100)
                .expect("no extra events")
                .events
                .len(),
            events.events.len(),
            "{kind}"
        );
    }
}

#[tokio::test]
async fn extension_worker_failure_and_activity_share_the_reported_diagnostic() {
    use alcomd_application::{M6ErrorCode, OfficialGuiStore};
    let directory = Directory::new();
    let store =
        StateStoreHandle::open(directory.database()).expect("valid isolated activity fixture");
    let owner = PrincipalId::local_owner();
    let extension = installed_extension_for(&store, owner.clone()).await;
    let plan = M6Store::create_uninstall_plan(
        &store,
        owner.clone(),
        ExtensionUninstallPlanDraft {
            extension,
            data_disposition: ExtensionDataDisposition::RetainData,
            plan_fingerprint: [8; 32],
        },
        10,
    )
    .await
    .expect("valid isolated activity fixture");
    let accepted = M6Store::accept_plan(
        &store,
        owner.clone(),
        plan.plan_id,
        key("failed-uninstall"),
        11,
    )
    .await
    .expect("valid isolated activity fixture");
    M6Store::begin_apply(&store, accepted.operation_id, 12)
        .await
        .expect("valid isolated activity fixture");
    let diagnostic = OperationId::new().to_string();
    M6Store::fail_operation(
        &store,
        accepted.operation_id,
        M6ErrorCode::PackageInvalid,
        diagnostic.clone(),
        13,
    )
    .await
    .expect("valid isolated activity fixture");
    let task = StateStore::get_operation(&store, owner.clone(), accepted.operation_id)
        .await
        .expect("valid isolated activity fixture");
    let activity = store
        .get_official_activity(owner.clone(), accepted.operation_id.to_string())
        .await
        .expect("valid isolated activity fixture");
    assert_eq!(task.diagnostic_id.as_deref(), Some(diagnostic.as_str()));
    assert_eq!(activity.diagnostic_id, task.diagnostic_id);
    assert_eq!(activity.status, "failed");
    let revision = activity.revision;
    M6Store::fail_operation(
        &store,
        accepted.operation_id,
        M6ErrorCode::PackageInvalid,
        diagnostic,
        14,
    )
    .await
    .expect("valid isolated activity fixture");
    assert_eq!(
        store
            .get_official_activity(owner, accepted.operation_id.to_string())
            .await
            .expect("valid isolated activity fixture")
            .revision,
        revision
    );
}
