use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use alcomd_application::{OfficialGuiStore, StateCheckResult, StateStore, StoreErrorKind};
use alcomd_domain::{IdempotencyKey, OperationId, OperationState, PrincipalId, Revision};
use alcomd_store::{StateStoreHandle, StoreOpenError};
use rusqlite::Connection;
use uuid::Uuid;

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("alcomd-store-test-{}", Uuid::new_v4()));
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }

    fn database(&self) -> PathBuf {
        self.0.join("state.db")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        for _ in 0..20 {
            match fs::remove_dir_all(&self.0) {
                Ok(()) => return,
                Err(_) => thread::sleep(Duration::from_millis(10)),
            }
        }
        panic!("failed to remove isolated store directory");
    }
}

fn key(value: &str) -> IdempotencyKey {
    IdempotencyKey::parse(value).expect("valid idempotency key")
}

#[tokio::test]
async fn state_check_is_persistent_idempotent_and_evented() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open store");
    let owner = PrincipalId::local_owner();
    let accepted = store
        .create_state_check(owner.clone(), key("check-1"), 10)
        .await
        .expect("create state check");
    assert!(!accepted.replayed);
    assert!(accepted.schedule);
    let replay = store
        .create_state_check(owner.clone(), key("check-1"), 20)
        .await
        .expect("replay state check");
    assert_eq!(replay.operation_id, accepted.operation_id);
    assert!(replay.replayed);
    assert!(!replay.schedule);

    let running = store
        .begin_state_check(accepted.operation_id, 30)
        .await
        .expect("begin state check");
    assert_eq!(running.state, OperationState::Running);
    let result = StateCheckResult {
        integrity: store.check_integrity().await.expect("integrity check"),
        foreign_keys: store.check_foreign_keys().await.expect("foreign key check"),
    };
    let completed = store
        .finish_state_check(accepted.operation_id, result, 40)
        .await
        .expect("finish state check");
    assert_eq!(completed.state, OperationState::Succeeded);
    assert_eq!(completed.revision.get(), 3);

    let events = store
        .list_events(owner.clone(), 0, 100)
        .await
        .expect("list events");
    assert_eq!(events.events.len(), 3);
    assert_eq!(events.next_sequence, events.events[2].sequence);
    let empty = store
        .list_events(owner.clone(), events.next_sequence, 100)
        .await
        .expect("empty event page");
    assert!(empty.events.is_empty());
    assert_eq!(empty.next_sequence, events.next_sequence);

    let operations = store
        .list_operations(owner, None, 100)
        .await
        .expect("list operations");
    assert_eq!(operations.operations, [completed]);
}

#[tokio::test]
async fn official_activity_and_diagnostics_are_owner_scoped_redacted_and_pageable() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open store");
    let owner = PrincipalId::local_owner();
    let accepted = store
        .create_state_check(owner.clone(), key("official-gui-failure"), 10)
        .await
        .expect("create operation");
    store
        .begin_state_check(accepted.operation_id, 20)
        .await
        .expect("begin operation");
    store
        .finish_failed(
            accepted.operation_id,
            "internal_error".to_owned(),
            "00000000-0000-4000-8000-000000000777".to_owned(),
            30,
        )
        .await
        .expect("fail operation");

    let first = store
        .list_official_activity(owner.clone(), None, 2)
        .await
        .expect("first activity page");
    assert_eq!(first.items.len(), 2);
    assert!(first.next_cursor.is_some());
    assert!(first.items.iter().all(|item| {
        !item.summary_code.contains("Authorization")
            && !item.summary_code.contains('\\')
            && !item.summary_code.contains('/')
    }));
    let second = store
        .list_official_activity(owner.clone(), first.next_cursor, 2)
        .await
        .expect("second activity page");
    assert_eq!(second.items.len(), 2);
    assert!(second.next_cursor.is_none());

    let diagnostics = store
        .list_official_diagnostics(owner.clone(), None, 100)
        .await
        .expect("diagnostics");
    assert_eq!(diagnostics.items.len(), 1);
    assert_eq!(diagnostics.items[0].code, "internal_error");
    assert_eq!(
        diagnostics.items[0].diagnostic_id.as_deref(),
        Some("00000000-0000-4000-8000-000000000777")
    );

    let other = PrincipalId::parse("test:other-owner").expect("other principal");
    assert!(
        store
            .list_official_activity(other.clone(), None, 100)
            .await
            .expect("isolated activity")
            .items
            .is_empty()
    );
    assert!(
        store
            .list_official_diagnostics(other, None, 100)
            .await
            .expect("isolated diagnostics")
            .items
            .is_empty()
    );
}

#[tokio::test]
async fn queued_cancel_requires_revision_and_replays_saved_result() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open store");
    let owner = PrincipalId::local_owner();
    let accepted = store
        .create_state_check(owner.clone(), key("check-2"), 10)
        .await
        .expect("create state check");
    let (cancelled, replayed) = store
        .cancel_operation(
            owner.clone(),
            accepted.operation_id,
            Revision::INITIAL,
            key("cancel-1"),
            20,
        )
        .await
        .expect("cancel operation");
    assert_eq!(cancelled.state, OperationState::Cancelled);
    assert!(!replayed);
    let (saved, replayed) = store
        .cancel_operation(
            owner,
            accepted.operation_id,
            Revision::INITIAL,
            key("cancel-1"),
            30,
        )
        .await
        .expect("replay cancellation");
    assert_eq!(saved, cancelled);
    assert!(replayed);
    let conflict = store
        .cancel_operation(
            PrincipalId::local_owner(),
            accepted.operation_id,
            Revision::new(2).expect("revision two"),
            key("cancel-1"),
            40,
        )
        .await
        .expect_err("different fingerprint must conflict");
    assert_eq!(conflict.kind(), StoreErrorKind::IdempotencyConflict);
}

#[tokio::test]
async fn stale_revision_and_cancel_completion_races_have_stable_outcomes() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open store");
    let owner = PrincipalId::local_owner();
    let accepted = store
        .create_state_check(owner.clone(), key("race-check-1"), 10)
        .await
        .expect("create first operation");
    let stale = store
        .cancel_operation(
            owner.clone(),
            accepted.operation_id,
            Revision::new(2).expect("revision two"),
            key("stale-cancel"),
            20,
        )
        .await
        .expect_err("stale revision");
    assert_eq!(stale.kind(), StoreErrorKind::RevisionConflict);

    let _running = store
        .begin_state_check(accepted.operation_id, 30)
        .await
        .expect("begin first operation");
    let (cancelling, _) = store
        .cancel_operation(
            owner.clone(),
            accepted.operation_id,
            Revision::new(2).expect("running revision"),
            key("race-cancel-1"),
            40,
        )
        .await
        .expect("request first cancellation");
    assert_eq!(cancelling.state, OperationState::Cancelling);
    let succeeded = store
        .finish_state_check(
            accepted.operation_id,
            StateCheckResult {
                integrity: alcomd_application::CheckClassification::Ok,
                foreign_keys: alcomd_application::CheckClassification::Ok,
            },
            50,
        )
        .await
        .expect("completion wins race");
    assert_eq!(succeeded.state, OperationState::Succeeded);

    let second = store
        .create_state_check(owner.clone(), key("race-check-2"), 60)
        .await
        .expect("create second operation");
    let _ = store
        .begin_state_check(second.operation_id, 70)
        .await
        .expect("begin second operation");
    let _ = store
        .cancel_operation(
            owner,
            second.operation_id,
            Revision::new(2).expect("running revision"),
            key("race-cancel-2"),
            80,
        )
        .await
        .expect("request second cancellation");
    let cancelled = store
        .finish_cancelled(second.operation_id, 90)
        .await
        .expect("cancellation wins race");
    assert_eq!(cancelled.state, OperationState::Cancelled);
}

#[tokio::test]
async fn running_operation_recovers_through_interrupted_and_recovering() {
    let directory = TestDirectory::new();
    let database = directory.database();
    let owner = PrincipalId::local_owner();
    let operation_id = {
        let store = StateStoreHandle::open(database.clone()).expect("open store");
        let accepted = store
            .create_state_check(owner.clone(), key("check-recover"), 10)
            .await
            .expect("create state check");
        let running = store
            .begin_state_check(accepted.operation_id, 20)
            .await
            .expect("begin state check");
        assert_eq!(running.revision.get(), 2);
        accepted.operation_id
    };
    wait_until_unlocked(&database);
    let store = StateStoreHandle::open(database.clone()).expect("reopen store");
    let scheduled = store.recover(30).await.expect("recover store");
    assert_eq!(scheduled, [operation_id]);
    let recovered = store
        .get_operation(owner, operation_id)
        .await
        .expect("recovered operation");
    assert_eq!(recovered.state, OperationState::Recovering);
    assert_eq!(recovered.revision.get(), 4);
    drop(store);
    wait_until_unlocked(&database);
    let store = StateStoreHandle::open(database).expect("reopen recovering store");
    assert_eq!(
        store.recover(40).await.expect("recover again"),
        [operation_id]
    );
    let recovered_again = store
        .get_operation(PrincipalId::local_owner(), operation_id)
        .await
        .expect("twice recovered operation");
    assert_eq!(recovered_again.state, OperationState::Recovering);
    assert_eq!(recovered_again.revision.get(), 6);
}

#[tokio::test]
async fn queued_and_cancelling_recovery_follow_the_frozen_table() {
    let directory = TestDirectory::new();
    let database = directory.database();
    let owner = PrincipalId::local_owner();
    let (queued_id, cancelling_id) = {
        let store = StateStoreHandle::open(database.clone()).expect("open store");
        let queued = store
            .create_state_check(owner.clone(), key("queued-recovery"), 10)
            .await
            .expect("create queued operation");
        let cancelling = store
            .create_state_check(owner.clone(), key("cancelling-recovery"), 20)
            .await
            .expect("create cancelling operation");
        let _ = store
            .begin_state_check(cancelling.operation_id, 30)
            .await
            .expect("begin cancelling operation");
        let (cancelling_record, _) = store
            .cancel_operation(
                owner.clone(),
                cancelling.operation_id,
                Revision::new(2).expect("running revision"),
                key("recovery-cancel"),
                40,
            )
            .await
            .expect("request cancellation");
        assert_eq!(cancelling_record.state, OperationState::Cancelling);
        (queued.operation_id, cancelling.operation_id)
    };
    wait_until_unlocked(&database);

    let store = StateStoreHandle::open(database).expect("reopen store");
    let scheduled = store.recover(50).await.expect("recover operations");
    assert_eq!(scheduled, [queued_id, cancelling_id]);
    let queued = store
        .get_operation(owner.clone(), queued_id)
        .await
        .expect("queued operation");
    assert_eq!(queued.state, OperationState::Queued);
    assert_eq!(queued.revision, Revision::INITIAL);
    let cancelling = store
        .get_operation(owner, cancelling_id)
        .await
        .expect("cancelling operation");
    assert_eq!(cancelling.state, OperationState::Recovering);
    assert_eq!(cancelling.revision.get(), 5);
    assert!(cancelling.cancel_requested);
}

#[tokio::test]
async fn missing_recovery_journal_fails_operation_safely() {
    let directory = TestDirectory::new();
    let database = directory.database();
    let owner = PrincipalId::local_owner();
    let operation_id = {
        let store = StateStoreHandle::open(database.clone()).expect("open store");
        store
            .create_state_check(owner.clone(), key("check-corrupt-journal"), 10)
            .await
            .expect("create state check")
            .operation_id
    };
    wait_until_unlocked(&database);
    let connection = Connection::open(&database).expect("open database for fault injection");
    connection
        .execute(
            "DELETE FROM operation_journal WHERE operation_id=?1",
            [operation_id.to_string()],
        )
        .expect("remove recovery journal");
    drop(connection);

    let store = StateStoreHandle::open(database.clone()).expect("reopen store");
    let scheduled = store
        .recover(20)
        .await
        .expect("scan recoverable operations");
    assert!(scheduled.is_empty());
    let failed = store
        .get_operation(owner, operation_id)
        .await
        .expect("load safely failed operation");
    assert_eq!(failed.state, OperationState::Failed);
    assert_eq!(failed.revision.get(), 2);
    assert_eq!(failed.error_code.as_deref(), Some("internal_error"));
    assert!(failed.diagnostic_id.is_some());
}

#[tokio::test]
async fn generic_state_check_recovery_does_not_claim_backup_create() {
    let directory = TestDirectory::new();
    let database = directory.database();
    {
        let store = StateStoreHandle::open(database.clone()).expect("initialize v6 store");
        drop(store);
    }
    wait_until_unlocked(&database);
    let operation_id = "00000000-0000-4000-8000-000000000731";
    let connection = Connection::open(&database).expect("open database for Backup fixture");
    connection
        .execute(
            "INSERT INTO operations (
                operation_id, kind, state, revision, owner_principal_id, request_json,
                cancel_requested, created_at_ms, updated_at_ms
             ) VALUES (?1, 'backups.create', 'queued', 1, 'builtin:local-owner',
                       '{}', 0, 1, 1)",
            [operation_id],
        )
        .expect("insert Backup Create operation");
    drop(connection);

    let store = StateStoreHandle::open(database).expect("reopen store");
    assert!(
        store
            .recover(20)
            .await
            .expect("run generic recovery")
            .is_empty()
    );
    let operation = store
        .get_operation(
            PrincipalId::local_owner(),
            OperationId::parse(operation_id).expect("operation ID"),
        )
        .await
        .expect("load untouched Backup operation");
    assert_eq!(operation.state, OperationState::Queued);
    assert_eq!(operation.revision, Revision::INITIAL);
}

#[tokio::test]
async fn principal_scopes_operations_events_and_idempotency() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open store");
    let owner_a = PrincipalId::parse("synthetic:a").expect("Principal A");
    let owner_b = PrincipalId::parse("synthetic:b").expect("Principal B");
    let first = store
        .create_state_check(owner_a.clone(), key("shared-key"), 10)
        .await
        .expect("create for A");
    let second = store
        .create_state_check(owner_b.clone(), key("shared-key"), 20)
        .await
        .expect("create for B");
    assert_ne!(first.operation_id, second.operation_id);
    assert!(
        store
            .get_operation(owner_b.clone(), first.operation_id)
            .await
            .is_err()
    );
    let events = store
        .list_events(owner_b, 0, 100)
        .await
        .expect("events for B");
    assert_eq!(events.events.len(), 1);
    assert_eq!(
        events.events[0].aggregate_id,
        second.operation_id.to_string()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn one_hundred_concurrent_commands_commit_without_duplicates() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("open store");
    let mut tasks = tokio::task::JoinSet::new();
    for index in 0..100_u64 {
        let store = store.clone();
        tasks.spawn(async move {
            store
                .create_state_check(
                    PrincipalId::local_owner(),
                    IdempotencyKey::parse(format!("concurrent-{index}")).expect("key"),
                    1_000 + index,
                )
                .await
                .expect("create concurrent operation")
                .operation_id
        });
    }
    let mut identifiers = std::collections::HashSet::new();
    while let Some(result) = tasks.join_next().await {
        assert!(identifiers.insert(result.expect("join command")));
    }
    assert_eq!(identifiers.len(), 100);
    let page = store
        .list_operations(PrincipalId::local_owner(), None, 100)
        .await
        .expect("list concurrent operations");
    assert_eq!(page.operations.len(), 100);
}

#[test]
fn newer_schema_is_rejected_before_migration_or_pragma_changes() {
    let directory = TestDirectory::new();
    let database = directory.database();
    let connection = Connection::open(&database).expect("create newer database");
    connection
        .execute_batch("PRAGMA user_version=14; CREATE TABLE sentinel(value TEXT);")
        .expect("create future schema");
    drop(connection);
    let error = StateStoreHandle::open(database.clone()).expect_err("reject future schema");
    assert!(matches!(
        error,
        StoreOpenError::UnsupportedDataSchema {
            found: 14,
            supported: 13
        }
    ));
    let connection = Connection::open(database).expect("reopen future database");
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("read version");
    let sentinel: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_schema WHERE type='table' AND name='sentinel'",
            [],
            |row| row.get(0),
        )
        .expect("read sentinel");
    assert_eq!(version, 14);
    assert_eq!(sentinel, 1);
}

#[test]
fn malformed_database_fails_closed_without_exposing_sqlite_details() {
    let directory = TestDirectory::new();
    fs::write(directory.database(), b"not a sqlite database").expect("write malformed database");
    let error = StateStoreHandle::open(directory.database()).expect_err("reject malformed store");
    assert!(matches!(error, StoreOpenError::Unavailable));
    assert_eq!(error.to_string(), "state store initialization failed");
}

fn wait_until_unlocked(database: &Path) {
    for _ in 0..50 {
        if Connection::open(database)
            .and_then(|connection| connection.execute_batch("PRAGMA journal_mode=WAL;"))
            .is_ok()
        {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("store worker did not release database");
}
