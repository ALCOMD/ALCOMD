//! Test-only startup and ownership of the real per-user daemon endpoint.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};

use alcomd_platform::{BindError, DataConfig, IpcConfig, IpcListener};
use tokio::sync::Mutex;
use tokio::task::{JoinError, JoinHandle};

static TEST_DATA_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// A ready test daemon that retains endpoint ownership until the handle drops.
///
/// Keeping the listener separate from the serving task prevents a failed task
/// from releasing the endpoint while a test is still connecting clients.
#[must_use = "keep the test daemon alive until its clients have finished"]
pub struct TestDaemon {
    task: JoinHandle<Result<(), BindError>>,
    _listener: Arc<Mutex<IpcListener>>,
    #[cfg(test)]
    applications: Arc<super::Applications>,
}

impl TestDaemon {
    /// Requests cancellation of the serving task without releasing ownership.
    pub fn abort(&self) {
        self.task.abort();
    }

    /// Reports whether the serving task has finished.
    pub fn is_finished(&self) -> bool {
        self.task.is_finished()
    }
}

impl Future for TestDaemon {
    type Output = Result<Result<(), BindError>, JoinError>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.get_mut().task).poll(context)
    }
}

impl Drop for TestDaemon {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Initializes and binds the daemon before allowing the test to connect.
///
/// Initialization errors, including an existing daemon, return before a client
/// can be created. Windows uses its unchanged, unique current-user endpoint.
pub async fn start_with_data_until<F>(
    ipc: IpcConfig,
    data: DataConfig,
    shutdown: F,
) -> Result<TestDaemon, BindError>
where
    F: Future<Output = ()> + Send + 'static,
{
    start(ipc, data, shutdown, None).await
}

/// Starts the M1/M2 fixtures with an automatically cleaned temporary data root.
pub async fn start_until<F>(ipc: IpcConfig, shutdown: F) -> Result<TestDaemon, BindError>
where
    F: Future<Output = ()> + Send + 'static,
{
    let directory = TemporaryDataDirectory::new();
    let data = DataConfig::isolated(directory.0.clone());
    start(ipc, data, shutdown, Some(directory)).await
}

async fn start<F>(
    ipc: IpcConfig,
    data: DataConfig,
    shutdown: F,
    temporary_data: Option<TemporaryDataDirectory>,
) -> Result<TestDaemon, BindError>
where
    F: Future<Output = ()> + Send + 'static,
{
    let storage_root = data
        .data_directory()
        .ok_or_else(|| BindError::Io(std::io::Error::other("test daemon requires isolated data")))?
        .join("test-user-directories");
    let (listener, applications) = super::initialize_daemon_with_directories(
        ipc,
        data,
        Arc::new(TestStorageDirectories(storage_root)),
    )
    .await?;
    let listener = Arc::new(Mutex::new(listener));
    let task_listener = Arc::clone(&listener);
    #[cfg(test)]
    let test_applications = Arc::clone(&applications);
    let task = tokio::spawn(async move {
        let mut listener = task_listener.lock().await;
        let result = super::run_listener(
            &mut listener,
            applications,
            shutdown,
            &mut tokio::task::JoinSet::new(),
        )
        .await
        .map_err(BindError::Io);
        drop(listener);
        drop(temporary_data);
        result
    });
    Ok(TestDaemon {
        task,
        _listener: listener,
        #[cfg(test)]
        applications: test_applications,
    })
}

struct TemporaryDataDirectory(PathBuf);

struct TestStorageDirectories(PathBuf);

impl alcomd_application::StorageDirectoryPlatform for TestStorageDirectories {
    fn default_directories(
        &self,
    ) -> Result<alcomd_application::StorageDirectories, alcomd_application::OfficialGuiError> {
        Ok(alcomd_application::StorageDirectories {
            project_directory: self.0.join("Projects").to_string_lossy().into_owned(),
            backup_directory: self.0.join("Backups").to_string_lossy().into_owned(),
        })
    }

    fn normalize_directory(
        &self,
        path: &str,
    ) -> Result<String, alcomd_application::OfficialGuiError> {
        alcomd_platform::normalize_storage_directory(path)
            .map_err(|_| alcomd_application::OfficialGuiError::InvalidInput)
    }
}

impl TemporaryDataDirectory {
    fn new() -> Self {
        let sequence = TEST_DATA_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        #[cfg(target_os = "macos")]
        let base = PathBuf::from("/private/tmp");
        #[cfg(not(target_os = "macos"))]
        let base = std::env::temp_dir();
        Self(base.join(format!(
            "alcomd-daemon-test-{}-{sequence}",
            std::process::id()
        )))
    }
}

impl Drop for TemporaryDataDirectory {
    fn drop(&mut self) {
        for _ in 0..20 {
            if std::fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use alcomd_platform::DaemonInstance;
    use alcomd_protocol::{
        HelloResult, METHOD_SYSTEM_HELLO, METHOD_SYSTEM_STATUS, RPC_VERSION, SuccessResponse,
        SystemStatusResult,
    };
    use serde_json::json;
    use tokio::sync::oneshot;

    use super::*;

    static IPC_TEST_LOCK: Mutex<()> = Mutex::const_new(());

    async fn connect_lifecycle_client(ipc: &IpcConfig) -> alcomd_platform::IpcStream {
        let mut stream = alcomd_platform::connect(ipc)
            .await
            .expect("connect real IPC");
        crate::write_json_frame(
            &mut stream,
            &json!({
                "id": "lifecycle-hello", "method": METHOD_SYSTEM_HELLO,
                "params": { "rpcVersion": RPC_VERSION, "client": {
                    "name": "lifecycle-test", "version": "1.0.0", "instanceId": "lifecycle-test"
                }, "capabilities": [alcomd_protocol::CAPABILITY_EVENTS_LIVE_V1] }
            }),
        )
        .await
        .expect("hello");
        let _: SuccessResponse<HelloResult> = read_response(&mut stream).await;
        stream
    }

    async fn start_lifecycle_daemon(fixture: &Fixture) -> TestDaemon {
        start_with_data_until(
            fixture.ipc.clone(),
            DataConfig::isolated(fixture.directory.0.join("data")),
            std::future::pending(),
        )
        .await
        .expect("start isolated lifecycle daemon")
    }

    async fn assert_idle_stopped(daemon: &mut TestDaemon) {
        tokio::time::timeout(Duration::from_secs(8), daemon)
            .await
            .expect("automatic exit deadline")
            .expect("join daemon")
            .expect("graceful automatic exit");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn idle_shutdown_waits_for_first_client_and_every_connection_to_close() {
        let _serial = IPC_TEST_LOCK.lock().await;
        let fixture = Fixture::new();
        let mut daemon = start_lifecycle_daemon(&fixture).await;
        tokio::time::sleep(Duration::from_millis(1300)).await;
        assert!(
            !daemon.is_finished(),
            "startup must allow the spawning client to connect"
        );
        let first = connect_lifecycle_client(&fixture.ipc).await;
        let mut second = connect_lifecycle_client(&fixture.ipc).await;
        drop(first);
        tokio::time::sleep(Duration::from_millis(1300)).await;
        assert!(!daemon.is_finished(), "one open client keeps Core running");
        crate::write_json_frame(
            &mut second,
            &json!({"id":"still-ready", "method":METHOD_SYSTEM_STATUS,"params":{}}),
        )
        .await
        .expect("write status after another client closes");
        let status: SuccessResponse<SystemStatusResult> = read_response(&mut second).await;
        assert_eq!(status.result.state, "ready");
        drop(second);
        assert_idle_stopped(&mut daemon).await;
        drop(daemon);
        // Ownership is released and the same persisted profile can start again.
        let mut restarted = start_lifecycle_daemon(&fixture).await;
        let client = connect_lifecycle_client(&fixture.ipc).await;
        drop(client);
        assert_idle_stopped(&mut restarted).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn idle_shutdown_is_cancelled_when_a_client_reconnects_during_handoff() {
        let _serial = IPC_TEST_LOCK.lock().await;
        let fixture = Fixture::new();
        let mut daemon = start_lifecycle_daemon(&fixture).await;
        drop(connect_lifecycle_client(&fixture.ipc).await);
        tokio::time::sleep(Duration::from_millis(400)).await;
        let replacement = connect_lifecycle_client(&fixture.ipc).await;
        tokio::time::sleep(Duration::from_millis(1300)).await;
        assert!(
            !daemon.is_finished(),
            "the old idle deadline cannot close a new connection"
        );
        drop(replacement);
        assert_idle_stopped(&mut daemon).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn idle_shutdown_counts_an_event_subscription_without_ordinary_rpc_clients() {
        let _serial = IPC_TEST_LOCK.lock().await;
        let fixture = Fixture::new();
        let mut daemon = start_lifecycle_daemon(&fixture).await;
        let ordinary = connect_lifecycle_client(&fixture.ipc).await;
        let mut events = connect_lifecycle_client(&fixture.ipc).await;
        crate::write_json_frame(&mut events, &json!({"id":"subscribe","method":alcomd_protocol::METHOD_EVENTS_SUBSCRIBE,"params":{}})).await.expect("subscribe on the dedicated event stream");
        let _: SuccessResponse<alcomd_protocol::EventsSubscribeResult> =
            read_response(&mut events).await;
        drop(ordinary);
        tokio::time::sleep(Duration::from_millis(1300)).await;
        assert!(
            !daemon.is_finished(),
            "the event stream is a live client connection"
        );
        drop(events);
        assert_idle_stopped(&mut daemon).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn idle_shutdown_waits_for_another_principals_durable_task_to_finish() {
        use alcomd_application::{IdempotencyKey, PrincipalId, StateStore};
        let _serial = IPC_TEST_LOCK.lock().await;
        let fixture = Fixture::new();
        let mut daemon = start_lifecycle_daemon(&fixture).await;
        let client = connect_lifecycle_client(&fixture.ipc).await;
        let owner = PrincipalId::parse("extension:idle-task").expect("other task owner");
        let task = daemon
            .applications
            .store
            .create_state_check(
                owner.clone(),
                IdempotencyKey::parse("idle-task").expect("durable task key"),
                1,
            )
            .await
            .expect("accepted unscheduled durable work");
        drop(client);
        tokio::time::sleep(Duration::from_millis(2300)).await;
        assert!(
            !daemon.is_finished(),
            "durable nonterminal work survives all client exits"
        );
        assert!(
            daemon
                .applications
                .m2
                .has_unfinished_operations()
                .await
                .expect("global task query with no clients")
        );
        daemon
            .applications
            .m2
            .recover()
            .await
            .expect("schedule durable work with no clients");
        assert_idle_stopped(&mut daemon).await;
        assert_eq!(
            daemon
                .applications
                .store
                .get_operation(owner, task.operation_id)
                .await
                .expect("load task after automatic exit")
                .state,
            alcomd_application::OperationState::Succeeded
        );
        assert!(
            !daemon
                .applications
                .m2
                .has_unfinished_operations()
                .await
                .expect("no unfinished task after completion")
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn idle_shutdown_also_stops_an_unclaimed_startup() {
        let _serial = IPC_TEST_LOCK.lock().await;
        let fixture = Fixture::new();
        let mut daemon = start_lifecycle_daemon(&fixture).await;
        assert_idle_stopped(&mut daemon).await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn idle_shutdown_finishes_startup_sync_and_releases_the_real_endpoint() {
        let _serial = IPC_TEST_LOCK.lock().await;
        let fixture = Fixture::new();
        let (mut listener, applications) = crate::initialize_daemon_with_directories(
            fixture.ipc.clone(),
            DataConfig::isolated(fixture.directory.0.join("data")),
            Arc::new(TestStorageDirectories(fixture.directory.0.join("user"))),
        )
        .await
        .expect("initialize isolated Core");
        let (finish, completion) = oneshot::channel();
        let daemon = tokio::spawn(async move {
            let mut sync = tokio::task::JoinSet::new();
            sync.spawn(async move {
                completion.await.expect("complete startup work");
            });
            crate::run_listener(
                &mut listener,
                applications,
                std::future::pending(),
                &mut sync,
            )
            .await
        });
        drop(connect_lifecycle_client(&fixture.ipc).await);
        tokio::time::sleep(Duration::from_millis(1300)).await;
        assert!(
            !daemon.is_finished(),
            "in-flight startup work must not be cut off"
        );
        finish.send(()).expect("release startup synchronization");
        tokio::time::timeout(Duration::from_secs(8), daemon)
            .await
            .expect("automatic exit deadline")
            .expect("join production listener loop")
            .expect("graceful exit");
        drop(DaemonInstance::acquire(&fixture.ipc).expect("automatic exit released the endpoint"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn occupied_endpoint_fails_before_initializing_test_data() {
        let _serial = IPC_TEST_LOCK.lock().await;
        let fixture = Fixture::new();
        let listener = IpcListener::bind(&fixture.ipc).expect("own the fixture endpoint");
        let data = fixture.directory.0.join("data");
        let result = start_with_data_until(
            fixture.ipc.clone(),
            DataConfig::isolated(data.clone()),
            std::future::pending(),
        )
        .await;
        assert!(matches!(result, Err(BindError::AlreadyRunning)));
        assert!(
            !data.exists(),
            "the rejected startup must not open its store"
        );
        drop(listener);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn initialization_failure_returns_without_publishing_an_endpoint() {
        let _serial = IPC_TEST_LOCK.lock().await;
        let fixture = Fixture::new();
        let invalid_data = fixture.directory.0.join("data-file");
        fs::write(&invalid_data, b"not a directory").expect("invalid data fixture");
        let result = start_with_data_until(
            fixture.ipc.clone(),
            DataConfig::isolated(invalid_data.clone()),
            std::future::pending(),
        )
        .await;
        assert!(matches!(result, Err(BindError::Io(_))));
        assert_eq!(
            fs::read(invalid_data).expect("preserved file"),
            b"not a directory"
        );
        let listener = IpcListener::bind(&fixture.ipc).expect("failed startup released ownership");
        drop(listener);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn successful_start_is_ready_for_real_rpc_and_retains_ownership_after_shutdown() {
        let _serial = IPC_TEST_LOCK.lock().await;
        let fixture = Fixture::new();
        let (stop, shutdown) = oneshot::channel();
        let mut daemon = start_with_data_until(
            fixture.ipc.clone(),
            DataConfig::isolated(fixture.directory.0.join("data")),
            async move {
                let _ = shutdown.await;
            },
        )
        .await
        .expect("start fixture daemon");

        let mut stream = alcomd_platform::connect(&fixture.ipc)
            .await
            .expect("ready endpoint accepts the first connection");
        crate::write_json_frame(
            &mut stream,
            &json!({
                "id": "ownership-hello",
                "method": METHOD_SYSTEM_HELLO,
                "params": {
                    "rpcVersion": RPC_VERSION,
                    "client": {
                        "name": "daemon-ownership-test",
                        "version": "1",
                        "instanceId": "daemon-ownership-test"
                    },
                    "capabilities": []
                }
            }),
        )
        .await
        .expect("write hello");
        let hello: SuccessResponse<HelloResult> = read_response(&mut stream).await;
        assert_eq!(hello.result.rpc_version, RPC_VERSION);
        crate::write_json_frame(
            &mut stream,
            &json!({"id": "ownership-status", "method": METHOD_SYSTEM_STATUS, "params": {}}),
        )
        .await
        .expect("write status");
        let status: SuccessResponse<SystemStatusResult> = read_response(&mut stream).await;
        assert_eq!(status.result.state, "ready");
        drop(stream);

        stop.send(()).expect("request fixture shutdown");
        tokio::time::timeout(Duration::from_secs(30), &mut daemon)
            .await
            .expect("fixture shutdown timeout")
            .expect("join fixture daemon")
            .expect("fixture daemon shutdown");
        assert!(matches!(
            DaemonInstance::acquire(&fixture.ipc),
            Err(BindError::AlreadyRunning)
        ));
        drop(daemon);
        drop(DaemonInstance::acquire(&fixture.ipc).expect("dropped handle released ownership"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn failed_serving_task_cannot_release_the_test_owned_endpoint() {
        let _serial = IPC_TEST_LOCK.lock().await;
        let fixture = Fixture::new();
        let (fail, failure) = oneshot::channel();
        let mut daemon = start_with_data_until(
            fixture.ipc.clone(),
            DataConfig::isolated(fixture.directory.0.join("data")),
            async move {
                let _ = failure.await;
                panic!("fixture serving task failure");
            },
        )
        .await
        .expect("start fixture daemon");
        fail.send(()).expect("trigger fixture task failure");
        let result = tokio::time::timeout(Duration::from_secs(30), &mut daemon)
            .await
            .expect("fixture failure timeout");
        assert!(result.is_err_and(|error| error.is_panic()));
        assert!(matches!(
            DaemonInstance::acquire(&fixture.ipc),
            Err(BindError::AlreadyRunning)
        ));
        drop(daemon);
        drop(DaemonInstance::acquire(&fixture.ipc).expect("dropped handle released ownership"));
    }

    async fn read_response<T: serde::de::DeserializeOwned>(
        stream: &mut alcomd_platform::IpcStream,
    ) -> T {
        let payload = tokio::time::timeout(Duration::from_secs(30), crate::read_frame(stream))
            .await
            .expect("fixture RPC response timeout")
            .expect("read fixture response")
            .expect("fixture response frame");
        serde_json::from_slice(&payload).expect("typed fixture response")
    }

    struct Fixture {
        directory: TemporaryDataDirectory,
        ipc: IpcConfig,
    }

    impl Fixture {
        fn new() -> Self {
            let directory = TemporaryDataDirectory::new();
            fs::create_dir_all(&directory.0).expect("fixture root");
            #[cfg(unix)]
            let ipc = IpcConfig::isolated(directory.0.join("runtime"));
            #[cfg(windows)]
            let ipc = IpcConfig::default();
            Self { directory, ipc }
        }
    }
}
