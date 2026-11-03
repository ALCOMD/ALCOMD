//! Real IPC subscription coverage. Startup must own the endpoint before any client connects.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use alcomd_client::{AlcomdClient, ClientConfig, EventSubscription};
use alcomd_platform::{DataConfig, IpcConfig, IpcStream};
use alcomd_protocol::{
    CAPABILITY_EVENTS_LIVE_V1, ClientInfo, Event, EventNotification, EventsCursor,
    EventsSubscribeParams, HelloParams, METHOD_EVENTS_SUBSCRIBE, METHOD_SYSTEM_HELLO,
    OperationState, RPC_VERSION, RequestEnvelope, decode_frame_length, encode_frame, error_code,
};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn committed_events_cross_clients_replay_unaccepted_batches_and_resync_after_restart() {
    let fixture = Fixture::new();
    // Windows intentionally uses the unchanged unique per-user pipe. The owned
    // startup fails before connecting if a real daemon already owns it.
    let (daemon, shutdown) = fixture.start().await;
    verify_correlated_rejections(&fixture.ipc).await;

    let mut subscriber = fixture.subscribe(None, 1).await;
    let ready = subscriber.ready().clone();
    assert!(ready.resync_required);
    let mut commands = fixture.commands().await;
    let operation = commands
        .state_check("m7-live-independent-client".into())
        .await
        .expect("accept on a distinct command connection");

    let first = next_batch(&mut subscriber).await;
    assert_eq!(first.events.len(), 1);
    assert_eq!(first.events[0].aggregate_id, operation.operation_id);
    assert_eq!(first.events[0].kind, "operation.created");
    let accepted_cursor = first.next_cursor.clone();
    let unaccepted = next_batch(&mut subscriber).await;
    let unaccepted_event_id = unaccepted.events[0].event_id.clone();
    assert!(unaccepted.next_cursor.sequence > accepted_cursor.sequence);
    // Receiving the second batch did not ACK it. Reconnect from the deliberately
    // older accepted cursor, not the SDK's received cursor or server sent cursor.
    drop(subscriber);
    let mut resumed = fixture.subscribe(Some(accepted_cursor.clone()), 1).await;
    assert!(!resumed.ready().resync_required);
    assert_eq!(resumed.ready().instance_id, ready.instance_id);
    assert_eq!(resumed.ready().baseline_cursor, accepted_cursor);
    let replayed = next_batch(&mut resumed).await;
    assert_eq!(replayed.events[0].event_id, unaccepted_event_id);

    let mut observed = first.events;
    observed.extend(replayed.events);
    let mut final_cursor = replayed.next_cursor;
    if !observed.iter().any(terminal_event) {
        let (tail, cursor) = through_terminal(&mut resumed, &operation.operation_id).await;
        observed.extend(tail);
        final_cursor = cursor;
    }
    assert!(
        observed
            .windows(2)
            .all(|pair| pair[0].sequence < pair[1].sequence)
    );
    assert!(
        observed
            .iter()
            .any(|event| event.kind == "operation.state_changed")
    );
    assert_eq!(
        observed.last().expect("terminal event").kind,
        "operation.completed"
    );
    let completed = commands
        .operation_get(operation.operation_id.clone())
        .await
        .expect("one authoritative query after terminal notification");
    assert_eq!(completed.state, OperationState::Succeeded);
    assert_eq!(
        observed
            .last()
            .expect("terminal revision")
            .aggregate_revision,
        completed.revision
    );
    let active = commands
        .operations_list(None, Some(1), true)
        .await
        .expect("active count snapshot");
    assert_eq!(active.active_count, 0);
    assert!(active.operations.is_empty());
    drop(resumed);

    // One subscriber deliberately consumes nothing. A second observer and the
    // command connection continue independently; no client ACK gates Store work.
    let unread = fixture.subscribe(None, 1).await;
    let mut observer = fixture.subscribe(None, 100).await;
    let mut pending = HashSet::new();
    for index in 0..16 {
        let accepted = tokio::time::timeout(
            Duration::from_secs(3),
            commands.state_check(format!("m7-unread-subscriber-{index}")),
        )
        .await
        .expect("subscriber must not gate accepting a command")
        .expect("state check");
        pending.insert(accepted.operation_id);
    }
    let status = tokio::time::timeout(Duration::from_secs(3), commands.system_status())
        .await
        .expect("unread subscriber must not block ordinary status")
        .expect("status");
    assert!(!status.daemon_version.is_empty());
    drop(unread);
    let after_disconnect = commands
        .state_check("m7-after-subscriber-disconnect".into())
        .await
        .expect("disconnection does not cancel later commands");
    pending.insert(after_disconnect.operation_id);
    let last_cursor = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let batch = next_batch(&mut observer).await;
            for event in &batch.events {
                if terminal_event(event) {
                    pending.remove(&event.aggregate_id);
                }
            }
            if pending.is_empty() {
                break batch.next_cursor;
            }
        }
    })
    .await
    .expect("all independently accepted tasks reach terminal notifications");
    assert!(last_cursor.sequence > final_cursor.sequence);
    final_cursor = last_cursor;
    drop(observer);
    drop(commands);
    stop(daemon, shutdown).await;

    let (daemon, shutdown) = fixture.start().await;
    let mut restarted = fixture.subscribe(Some(final_cursor), 100).await;
    let restarted_ready = restarted.ready().clone();
    assert!(restarted_ready.resync_required);
    assert_ne!(restarted_ready.instance_id, ready.instance_id);
    let mut commands = fixture.commands().await;
    let fresh = commands
        .state_check("m7-new-core-instance".into())
        .await
        .expect("new instance command");
    let (fresh_events, _) = through_terminal(&mut restarted, &fresh.operation_id).await;
    assert!(
        fresh_events
            .iter()
            .all(|event| event.sequence > restarted_ready.baseline_cursor.sequence)
    );
    assert!(
        fresh_events
            .iter()
            .all(|event| event.aggregate_id == fresh.operation_id)
    );
    assert!(
        fresh_events
            .iter()
            .any(|event| event.kind == "operation.created")
    );
    drop(restarted);
    drop(commands);
    stop(daemon, shutdown).await;
}

async fn verify_correlated_rejections(ipc: &IpcConfig) {
    let mut raw = alcomd_platform::connect(ipc)
        .await
        .expect("owned ready daemon endpoint");
    request(&mut raw, "before-auth", METHOD_EVENTS_SUBSCRIBE, json!({})).await;
    assert_error(
        read_json(&mut raw).await,
        "before-auth",
        error_code::HANDSHAKE_REQUIRED,
    );
    hello(&mut raw, vec![]).await;
    request(
        &mut raw,
        "missing-capability",
        METHOD_EVENTS_SUBSCRIBE,
        json!({}),
    )
    .await;
    assert_error(
        read_json(&mut raw).await,
        "missing-capability",
        error_code::CAPABILITY_REQUIRED,
    );
    drop(raw);

    let mut raw = alcomd_platform::connect(ipc)
        .await
        .expect("second isolated connection");
    hello(&mut raw, vec![CAPABILITY_EVENTS_LIVE_V1.into()]).await;
    request(
        &mut raw,
        "forged-owner",
        METHOD_EVENTS_SUBSCRIBE,
        json!({"owner":"other"}),
    )
    .await;
    assert_error(
        read_json(&mut raw).await,
        "forged-owner",
        error_code::INVALID_REQUEST,
    );
    request(
        &mut raw,
        "invalid-limit",
        METHOD_EVENTS_SUBSCRIBE,
        json!({"limit":1001}),
    )
    .await;
    assert_error(
        read_json(&mut raw).await,
        "invalid-limit",
        error_code::INVALID_REQUEST,
    );
}

fn assert_error(wire: Value, id: &str, code: &str) {
    assert_eq!(wire["id"], id);
    assert_eq!(wire["error"]["code"], code);
    assert!(
        wire.get("method").is_none(),
        "rejections are ordinary correlated responses"
    );
}

async fn hello(stream: &mut IpcStream, capabilities: Vec<String>) {
    request(
        stream,
        "hello",
        METHOD_SYSTEM_HELLO,
        serde_json::to_value(HelloParams {
            rpc_version: RPC_VERSION,
            client: ClientInfo {
                name: "m7-live-isolated-test".into(),
                version: "1".into(),
                instance_id: Uuid::new_v4().to_string(),
            },
            capabilities,
        })
        .expect("hello DTO"),
    )
    .await;
    let response = read_json(stream).await;
    assert_eq!(response["id"], "hello");
    assert!(response.get("result").is_some());
}

async fn request(stream: &mut IpcStream, id: &str, method: &str, params: Value) {
    let payload = serde_json::to_vec(&RequestEnvelope {
        id: id.into(),
        method: method.into(),
        params,
    })
    .expect("request");
    stream
        .write_all(&encode_frame(&payload).expect("frame"))
        .await
        .expect("write request");
    stream.flush().await.expect("flush request");
}

async fn read_json(stream: &mut IpcStream) -> Value {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut prefix = [0; 4];
        stream.read_exact(&mut prefix).await.expect("frame prefix");
        let mut payload = vec![0; decode_frame_length(prefix).expect("bounded frame")];
        stream.read_exact(&mut payload).await.expect("frame body");
        serde_json::from_slice(&payload).expect("JSON response")
    })
    .await
    .expect("bounded response wait")
}

async fn next_batch(subscription: &mut EventSubscription) -> alcomd_protocol::EventsBatch {
    match tokio::time::timeout(Duration::from_secs(5), subscription.next_notification())
        .await
        .expect("event delivery deadline")
        .expect("valid event notification")
    {
        EventNotification::Batch(batch) => batch,
        EventNotification::Closed(closed) => {
            panic!("unexpected subscription closure: {}", closed.error.code)
        }
    }
}

fn terminal_event(event: &Event) -> bool {
    event.aggregate_kind == "operation"
        && matches!(
            event.payload.get("state").and_then(Value::as_str),
            Some("succeeded" | "failed" | "cancelled")
        )
}

async fn through_terminal(
    subscription: &mut EventSubscription,
    operation_id: &str,
) -> (Vec<Event>, EventsCursor) {
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut events = vec![];
        loop {
            let batch = next_batch(subscription).await;
            events.extend(
                batch
                    .events
                    .into_iter()
                    .filter(|event| event.aggregate_id == operation_id),
            );
            if events.iter().any(terminal_event) {
                break (events, batch.next_cursor);
            }
        }
    })
    .await
    .expect("task lifecycle delivery")
}

async fn stop(daemon: alcomd_daemon::test_support::TestDaemon, shutdown: oneshot::Sender<()>) {
    shutdown.send(()).expect("signal owned daemon");
    tokio::time::timeout(Duration::from_secs(5), daemon)
        .await
        .expect("daemon shutdown deadline")
        .expect("join daemon")
        .expect("daemon stopped");
}

struct Fixture {
    directory: PathBuf,
    ipc: IpcConfig,
    client: ClientConfig,
}

impl Fixture {
    fn new() -> Self {
        #[cfg(target_os = "macos")]
        let base = PathBuf::from("/private/tmp");
        #[cfg(not(target_os = "macos"))]
        let base = std::env::temp_dir();
        let directory = base.join(format!("alcomd-m7-events-live-{}", Uuid::new_v4()));
        std::fs::create_dir(&directory).expect("new isolated test root");
        let runtime = directory.join("runtime");
        std::fs::create_dir(&runtime).expect("runtime");
        std::fs::create_dir(directory.join("data")).expect("isolated state data");
        #[cfg(unix)]
        let (ipc, client) = (
            IpcConfig::isolated(runtime.clone()),
            ClientConfig::default()
                .without_daemon_start()
                .with_runtime_directory(runtime),
        );
        #[cfg(windows)]
        let (ipc, client) = (
            IpcConfig::default(),
            ClientConfig::default().without_daemon_start(),
        );
        Self {
            directory,
            ipc,
            client,
        }
    }
    async fn start(&self) -> (alcomd_daemon::test_support::TestDaemon, oneshot::Sender<()>) {
        let (sender, receiver) = oneshot::channel();
        let daemon = alcomd_daemon::test_support::start_with_data_until(
            self.ipc.clone(),
            DataConfig::isolated(self.directory.join("data")),
            async {
                let _ = receiver.await;
            },
        )
        .await
        .expect("must own endpoint and isolated data before making any client connection");
        (daemon, sender)
    }
    async fn commands(&self) -> AlcomdClient {
        AlcomdClient::connect(self.client.clone())
            .await
            .expect("connect only after fixture owns endpoint")
    }
    async fn subscribe(&self, cursor: Option<EventsCursor>, limit: u32) -> EventSubscription {
        EventSubscription::connect(
            self.client.clone(),
            EventsSubscribeParams {
                cursor,
                limit: Some(limit),
            },
        )
        .await
        .expect("subscribe only to owned test daemon")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if !self.directory.exists() {
            return;
        }
        let resolved = self
            .directory
            .canonicalize()
            .expect("resolve explicit fixture root");
        assert!(
            resolved
                .file_name()
                .expect("fixture name")
                .to_string_lossy()
                .starts_with("alcomd-m7-events-live-")
        );
        assert_eq!(
            resolved.parent(),
            self.directory
                .parent()
                .and_then(|parent| parent.canonicalize().ok())
                .as_deref(),
            "cleanup remains one direct child of the chosen temporary directory"
        );
        for _ in 0..50 {
            if std::fs::remove_dir_all(&resolved).is_ok() || !resolved.exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("isolated test root was not released");
    }
}
