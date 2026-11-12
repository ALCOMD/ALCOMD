use super::*;
use alcomd_application::{Application, IdempotencyKey, OperationState};

#[tokio::test]
async fn idle_shutdown_sees_every_principal_and_only_three_terminal_states() {
    let directory = crate::events_tests::Directory::new();
    let store = StateStoreHandle::open(directory.database()).expect("isolated store");
    let application = Application::new(store.clone());
    assert!(
        !application
            .has_unfinished_operations()
            .await
            .expect("empty store")
    );
    let owner = PrincipalId::parse("extension:idle-shutdown-test").expect("other principal");
    let created = store
        .create_state_check(
            owner.clone(),
            IdempotencyKey::parse("foreign-task").expect("fixture idempotency key"),
            1,
        )
        .await
        .expect("durable operation");
    assert!(
        store
            .list_operations(PrincipalId::local_owner(), None, 1, true)
            .await
            .expect("local owner task list")
            .operations
            .is_empty()
    );
    for (name, state) in [
        ("queued", OperationState::Queued),
        ("planning", OperationState::Planning),
        ("waiting_for_input", OperationState::WaitingForInput),
        ("running", OperationState::Running),
        ("cancelling", OperationState::Cancelling),
        ("interrupted", OperationState::Interrupted),
        ("recovering", OperationState::Recovering),
        ("succeeded", OperationState::Succeeded),
        ("failed", OperationState::Failed),
        ("cancelled", OperationState::Cancelled),
    ] {
        store
            .request_worker(
                move |connection| {
                    connection
                        .execute(
                            "UPDATE operations SET state=?1 WHERE operation_id=?2",
                            rusqlite::params![name, created.operation_id.to_string()],
                        )
                        .expect("set fixture operation state");
                    Ok::<_, StoreError>(())
                },
                sqlite::unavailable,
            )
            .await
            .expect("update fixture on store worker");
        assert_eq!(
            application
                .has_unfinished_operations()
                .await
                .expect("global lifecycle query"),
            !state.is_terminal(),
            "{name}"
        );
    }
}

#[tokio::test]
async fn idle_shutdown_query_does_not_treat_an_unavailable_store_as_empty() {
    let directory = crate::events_tests::Directory::new();
    let store = StateStoreHandle::open(directory.database()).expect("isolated store");
    store
        .inner
        .commands
        .lock()
        .expect("store command mutex")
        .take();
    assert!(
        Application::new(store)
            .has_unfinished_operations()
            .await
            .is_err()
    );
}
