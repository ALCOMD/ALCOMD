use alcomd_client::{AlcomdClient, ClientConfig, ClientError};
use alcomd_platform::{DataConfig, IpcConfig};
use alcomd_protocol::{RepositoryListEntryStatus, RepositoryListSource, RepositorySource};
use std::fs;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn repository_list_rpc_reads_previews_and_creates_without_overwrite_or_hidden_registration() {
    let fixture = Fixture::new();
    let data = fixture.0.join("data");
    let runtime = fixture.0.join("runtime");
    fs::create_dir(&data).expect("data");
    fs::create_dir(&runtime).expect("runtime");
    #[cfg(unix)]
    let (ipc, config) = (
        IpcConfig::isolated(runtime.clone()),
        ClientConfig::default()
            .without_daemon_start()
            .with_runtime_directory(runtime),
    );
    #[cfg(windows)]
    let (ipc, config) = (
        IpcConfig::default(),
        ClientConfig::default().without_daemon_start(),
    );
    let (stop, shutdown) = tokio::sync::oneshot::channel();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc,
        DataConfig::isolated(data.clone()),
        async {
            let _ = shutdown.await;
        },
    )
    .await
    .expect("owned test daemon");
    let mut client = AlcomdClient::connect(config).await.expect("client");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback");
    let address = listener.local_addr().expect("address");
    let url = format!("http://{address}/repository.json?z=2&a=1&a=3");
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("request");
        let mut bytes = [0_u8; 4096];
        let read = stream.read(&mut bytes).await.expect("read");
        assert!(
            String::from_utf8_lossy(&bytes[..read])
                .starts_with("GET /repository.json?z=2&a=1&a=3 HTTP/1.1")
        );
        let body = r#"{"id":"list-rpc-remote","name":"List RPC","packages":{}}"#;
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            )
            .await
            .expect("response");
    });
    let input = fixture.0.join("input.txt");
    let input_bytes = format!(
        "# v3 export\r\n{url}\r\n{url}\r\nfile:///local\r\nvcc://vpm/addRepo?url=https%3A%2F%2Fexample.test&headers%5B%5D=X-Token%3Aprivate-preview-token\r\ninvalid\r\n"
    );
    fs::write(&input, &input_bytes).expect("input");
    let inspected = client
        .repository_list_inspect_file(input.to_str().expect("path").into())
        .await
        .expect("inspect list");
    assert_eq!(
        inspected
            .entries
            .iter()
            .map(|entry| entry.status)
            .collect::<Vec<_>>(),
        [
            RepositoryListEntryStatus::Candidate,
            RepositoryListEntryStatus::Duplicate,
            RepositoryListEntryStatus::Unsupported,
            RepositoryListEntryStatus::Candidate,
            RepositoryListEntryStatus::Invalid
        ]
    );
    let authenticated = &inspected.entries[3];
    assert!(authenticated.credentials_configured);
    assert!(matches!(
        &authenticated.source,
        Some(RepositoryListSource::Remote { url }) if url == "https://example.test/"
    ));
    let preview_json = serde_json::to_string(&inspected).expect("public preview");
    assert!(!preview_json.contains("X-Token"));
    assert!(!preview_json.contains("private-preview-token"));
    assert!(
        client
            .repositories_list(None, None)
            .await
            .expect("no registration")
            .repositories
            .is_empty()
    );
    assert_eq!(
        fs::read(&input).expect("unchanged input"),
        input_bytes.as_bytes()
    );
    let source = RepositorySource::Remote { url: url.clone() };
    client
        .repository_inspect(source.clone())
        .await
        .expect("read source");
    client
        .repository_register(source, "list-add-remote".into())
        .await
        .expect("explicit append reuses the single remote response");
    server.await.expect("server");
    let old_preview = client
        .repository_list_preview_export()
        .await
        .expect("preview");
    assert_eq!(old_preview.exportable_count, 1);
    let local = fixture.0.join("repository.json");
    fs::write(&local, r#"{"id":"list-rpc-local","packages":{}}"#).expect("local repository");
    client
        .repository_register(
            RepositorySource::Local {
                path: local.to_str().expect("path").into(),
            },
            "list-add-local".into(),
        )
        .await
        .expect("explicit local append");
    let output = fixture.0.join("export.txt");
    assert_code(
        client
            .repository_list_export_file(
                output.to_str().expect("path").into(),
                old_preview.fingerprint,
            )
            .await
            .expect_err("stale preview"),
        "repository_list_changed",
    );
    assert!(!output.exists());
    let preview = client
        .repository_list_preview_export()
        .await
        .expect("fresh preview");
    assert_eq!(preview.exportable_count, 1);
    assert_eq!(preview.excluded.len(), 1);
    let exported = client
        .repository_list_export_file(
            output.to_str().expect("path").into(),
            preview.fingerprint.clone(),
        )
        .await
        .expect("export");
    assert_eq!(exported.exported_count, 1);
    assert_eq!(
        fs::read_to_string(&output).expect("complete output"),
        format!("{url}\n")
    );
    assert_code(
        client
            .repository_list_export_file(
                output.to_str().expect("path").into(),
                preview.fingerprint.clone(),
            )
            .await
            .expect_err("no overwrite"),
        "repository_list_target_exists",
    );
    assert_eq!(
        fs::read_to_string(&output).expect("preserved"),
        format!("{url}\n")
    );
    assert_code(
        client
            .repository_list_export_file(
                data.join("blocked.txt").to_str().expect("path").into(),
                preview.fingerprint,
            )
            .await
            .expect_err("protected data"),
        "repository_list_target_invalid",
    );
    let after = client
        .repository_list_inspect_file(output.to_str().expect("path").into())
        .await
        .expect("round trip");
    assert_eq!(
        after.entries[0].status,
        RepositoryListEntryStatus::Duplicate
    );
    assert_eq!(
        client
            .repositories_list(None, None)
            .await
            .expect("preserved registry")
            .repositories
            .len(),
        2
    );
    assert!(!fs::read_dir(&fixture.0).expect("files").any(|entry| {
        entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .ends_with(".partial")
    }));
    drop(client);
    stop.send(()).expect("stop");
    daemon.await.expect("join").expect("daemon result");
}

fn assert_code(error: ClientError, expected: &str) {
    match error {
        ClientError::Remote(error) => assert_eq!(error.code, expected),
        other => panic!("unexpected client error: {other:?}"),
    }
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        #[cfg(target_os = "macos")]
        let base = PathBuf::from("/private/tmp");
        #[cfg(not(target_os = "macos"))]
        let base = std::env::temp_dir();
        let root = base.join(format!("alcomd-list-rpc-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).expect("fixture");
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
