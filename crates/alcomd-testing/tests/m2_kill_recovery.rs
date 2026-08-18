use std::fs;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use alcomd_application::StateStore;
use alcomd_domain::{IdempotencyKey, OperationId, OperationState, PrincipalId};
use alcomd_store::StateStoreHandle;

const DATABASE_ENV: &str = "ALCOMD_M2_TEST_DATABASE";
const SIGNAL_ENV: &str = "ALCOMD_M2_TEST_SIGNAL";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn killed_process_recovers_running_operation_without_duplicate() {
    let directory = isolated_directory();
    fs::create_dir(&directory).expect("create isolated directory");
    let database = directory.join("state.db");
    let signal = directory.join("running.signal");
    let mut child = Command::new(std::env::current_exe().expect("current test executable"))
        .args([
            "--exact",
            "subprocess_holds_running_operation",
            "--ignored",
            "--nocapture",
        ])
        .env(DATABASE_ENV, &database)
        .env(SIGNAL_ENV, &signal)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn store subprocess");

    let operation_id = wait_for_signal(&signal);
    child.kill().expect("force-kill store subprocess");
    let _ = child.wait().expect("wait for killed subprocess");

    let store = open_with_retry(&database);
    let scheduled = store.recover(100).await.expect("recover killed operation");
    assert_eq!(scheduled, [operation_id]);
    let operation = store
        .get_operation(PrincipalId::local_owner(), operation_id)
        .await
        .expect("load recovered operation");
    assert_eq!(operation.state, OperationState::Recovering);
    assert_eq!(operation.revision.get(), 4);
    let page = store
        .list_operations(PrincipalId::local_owner(), None, 100)
        .await
        .expect("list recovered operations");
    assert_eq!(page.operations.len(), 1);
    drop(store);
    remove_with_retry(&directory);
}

#[test]
#[ignore = "subprocess fixture invoked by the kill/restart test"]
fn subprocess_holds_running_operation() {
    let database = std::env::var_os(DATABASE_ENV).expect("database environment");
    let signal = std::env::var_os(SIGNAL_ENV).expect("signal environment");
    let runtime = tokio::runtime::Runtime::new().expect("test runtime");
    runtime.block_on(async move {
        let store = StateStoreHandle::open(database.into()).expect("open child store");
        let accepted = store
            .create_state_check(
                PrincipalId::local_owner(),
                IdempotencyKey::parse("kill-recovery").expect("idempotency key"),
                10,
            )
            .await
            .expect("create child operation");
        let running = store
            .begin_state_check(accepted.operation_id, 20)
            .await
            .expect("begin child operation");
        assert_eq!(running.state, OperationState::Running);
        fs::write(signal, accepted.operation_id.to_string()).expect("write running signal");
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    });
}

fn wait_for_signal(signal: &std::path::Path) -> OperationId {
    for _ in 0..200 {
        if let Ok(value) = fs::read_to_string(signal) {
            return OperationId::parse(value.trim()).expect("signal Operation ID");
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("subprocess did not reach running state");
}

fn open_with_retry(database: &std::path::Path) -> StateStoreHandle {
    for _ in 0..100 {
        if let Ok(store) = StateStoreHandle::open(database.to_path_buf()) {
            return store;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("killed subprocess did not release the database");
}

fn remove_with_retry(directory: &std::path::Path) {
    for _ in 0..100 {
        if fs::remove_dir_all(directory).is_ok() || !directory.exists() {
            return;
        }
        thread::sleep(Duration::from_millis(10));
    }
    panic!("failed to remove kill/recovery fixture");
}

fn isolated_directory() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("alcomd-kill-test-{}-{nonce}", std::process::id()))
}
