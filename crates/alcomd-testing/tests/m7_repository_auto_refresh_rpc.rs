//! Real IPC: reads return the cached index before a separately owned refresh completes.
use alcomd_application::{
    IdempotencyKey, M3RegistryStore, PrincipalId, RepositoryReadOutcome, RepositorySource,
};
use alcomd_client::{AlcomdClient, ClientConfig};
use alcomd_platform::{DataConfig, IpcConfig};
use alcomd_store::StateStoreHandle;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{Notify, Semaphore};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reads_hint_background_refresh_without_waiting_and_existing_events_publish_new_index() {
    let root = std::env::temp_dir().join(format!("alcomd-auto-rpc-{}", uuid::Uuid::new_v4()));
    let data = root.join("data");
    std::fs::create_dir_all(&data).expect("data");
    let manifest = |version: &str| {
        serde_json::json!({"id":"rpc-fixture","name":"Fixture","packages":{"com.example.fixture":{"versions":{version:{"name":"com.example.fixture","version":version,"url":"https://example.invalid/package.zip","author":{"name":"Fixture"}}}}}}).to_string()
    };
    let local = root.join("seed.json");
    std::fs::write(&local, manifest("1.0.0")).expect("seed file");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("HTTP fixture");
    let url = format!(
        "http://{}/index.json",
        listener.local_addr().expect("address")
    );
    let reader = alcomd_vpm::VpmReader::new().expect("reader");
    let RepositoryReadOutcome::Fresh(mut observation) = reader
        .inspect_local_repository(local, vec![1], 1)
        .await
        .expect("seed parse")
    else {
        panic!("fresh")
    };
    observation.source = RepositorySource::Remote { url: url.clone() };
    observation.source_identity_key = url.as_bytes().to_vec();
    observation.validators.etag = Some("\"old\"".to_owned());
    let store = StateStoreHandle::open(data.join("state.db")).expect("store");
    let record = store
        .register_repository(
            PrincipalId::local_owner(),
            observation,
            IdempotencyKey::parse("seed").expect("key"),
            1,
        )
        .await
        .expect("seed")
        .value;
    drop(store);
    let requests = Arc::new(AtomicUsize::new(0));
    let entered = Arc::new(Notify::new());
    let gate = Arc::new(Semaphore::new(0));
    let mut tasks = tokio::task::JoinSet::new();
    tasks.spawn({let requests=requests.clone();let entered=entered.clone();let gate=gate.clone();let body=manifest("2.0.0");async move {
        let mut connections = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                connection = listener.accept() => {
                    let (mut socket,_) = connection.expect("HTTP connection");
                    let requests=requests.clone();let entered=entered.clone();let gate=gate.clone();let body=body.clone();
                    connections.spawn(async move {
                        let mut header=Vec::new();
                        while !header.ends_with(b"\r\n\r\n") {
                            assert!(header.len()<8192);
                            header.push(socket.read_u8().await.expect("request byte"));
                        }
                        assert!(String::from_utf8_lossy(&header).to_ascii_lowercase().contains("if-none-match:"));
                        requests.fetch_add(1,Ordering::SeqCst);entered.notify_one();
                        gate.acquire().await.expect("release HTTP").forget();
                        let response=format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nETag: \"new\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",body.len(),body);
                        socket.write_all(response.as_bytes()).await.expect("HTTP response");
                    });
                }
                result = connections.join_next(), if !connections.is_empty() => {result.expect("connection result").expect("connection task");}
            }
        }
    }});
    #[cfg(unix)]
    let (ipc, client_config) = (
        IpcConfig::isolated(root.join("runtime")),
        ClientConfig::default()
            .without_daemon_start()
            .with_runtime_directory(root.join("runtime")),
    );
    #[cfg(windows)]
    let (ipc, client_config) = (
        IpcConfig::default(),
        ClientConfig::default().without_daemon_start(),
    );
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc,
        DataConfig::isolated(data),
        async {
            let _ = stopped.await;
        },
    )
    .await
    .expect("owned daemon");
    let mut client = AlcomdClient::connect(client_config).await.expect("client");
    client
        .repository_get("not-an-id".to_owned())
        .await
        .expect_err("invalid read");
    tokio::time::sleep(std::time::Duration::from_millis(650)).await;
    assert_eq!(
        requests.load(Ordering::SeqCst),
        0,
        "invalid queries cannot trigger network"
    );
    let cached = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        client.repository_packages(record.repository_id.to_string(), None, Some(100)),
    )
    .await
    .expect("cached response before HTTP release")
    .expect("cached packages");
    assert_eq!(
        serde_json::to_value(cached).expect("JSON")["packages"][0]["version"],
        "1.0.0"
    );
    tokio::time::timeout(std::time::Duration::from_secs(5), entered.notified())
        .await
        .expect("background request");
    for _ in 0..3 {
        client
            .repositories_list(None, Some(100))
            .await
            .expect("concurrent read hints");
    }
    assert_eq!(
        requests.load(Ordering::SeqCst),
        1,
        "one in-flight request per source"
    );
    gate.add_permits(1);
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let page = client
                .repository_packages(record.repository_id.to_string(), None, Some(100))
                .await
                .expect("packages");
            if serde_json::to_value(page).expect("JSON")["packages"][0]["version"] == "2.0.0" {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("new index published");
    let events = serde_json::to_value(client.events_list(0, Some(100)).await.expect("events"))
        .expect("JSON");
    assert!(
        events["events"]
            .as_array()
            .expect("events")
            .iter()
            .any(|event| event["kind"] == "repository.refreshed")
    );
    tokio::time::sleep(std::time::Duration::from_millis(650)).await;
    assert_eq!(
        requests.load(Ordering::SeqCst),
        1,
        "fresh index prevents refresh loops after events"
    );
    stop.send(()).expect("shutdown");
    drop(client);
    daemon.await.expect("daemon task").expect("shutdown");
    tasks.shutdown().await;
}
