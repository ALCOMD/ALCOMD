#[path = "support/mcp_actor.rs"]
mod mcp_actor;

use alcomd_client::{AlcomdClient, ClientConfig, ClientError};
use alcomd_platform::{DataConfig, IpcConfig};
use alcomd_protocol::{
    RepositoryListEntryStatus, RepositoryListSource, RepositoryMoveParams, RepositorySource,
};
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
        ipc.clone(),
        DataConfig::isolated(data.clone()),
        async {
            let _ = shutdown.await;
        },
    )
    .await
    .expect("owned test daemon");
    let mut client = AlcomdClient::connect(config.clone()).await.expect("client");
    let issued = mcp_actor::issue_actor(&mut client, "native-repository-manager").await;
    let mut actor = AlcomdClient::connect(config.clone().with_mcp_credential(issued.credential))
        .await
        .expect("connect authenticated MCP client");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback");
    let address = listener.local_addr().expect("address");
    let url = format!("http://{address}/repository.json?z=2&a=1&a=3");
    let server = tokio::spawn(async move {
        for _ in 0..2 {
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
        }
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
    let alcom_list = fixture.0.join("input.alcomrepositories");
    fs::write(&alcom_list, &input_bytes).expect("ALCOM exchange file");
    let alcom_inspection = client
        .repository_list_inspect_file(alcom_list.to_str().expect("ALCOM path").into())
        .await
        .expect("inspect ALCOM repository list");
    assert_eq!(alcom_inspection.fingerprint, inspected.fingerprint);
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
    let remote = client
        .repository_inspect(source)
        .await
        .expect("read source");
    let remote_id = remote.preview_id.expect("original ID");
    client
        .repository_register(remote_id.clone(), "list-add-remote".into())
        .await
        .expect("registration revalidates the remote observation");
    server.await.expect("server");
    assert!(
        client
            .repository_register(remote_id.clone(), "list-add-remote".into())
            .await
            .expect("registration receipt remains available after the HTTP server stops")
            .replayed
    );
    let old_preview = client
        .repository_configuration_preview_export(alcomd_protocol::RepositoryListFormat::Legacy)
        .await
        .expect("preview");
    assert_eq!(old_preview.exportable_count, 1);
    let local = fixture.0.join("repository.json");
    fs::write(&local, r#"{"id":"list-rpc-local","packages":{}}"#).expect("local repository");
    let local_source = RepositorySource::Local {
        path: local.to_str().expect("path").into(),
    };
    let local_preview = actor
        .repository_inspect(local_source.clone())
        .await
        .expect("local preview");
    let local_id = local_preview
        .preview_id
        .expect("local registration preview");
    let params = alcomd_protocol::PlansGetParams {
        kind: alcomd_protocol::PlanKind::RepositoryRegistration,
        plan_id: local_id.clone(),
    };
    assert_code(
        client
            .get_plan(params.clone())
            .await
            .expect_err("foreign preview"),
        "revision_conflict",
    );
    let alcomd_protocol::PlansGetResult::RepositoryRegistration { plan } = actor
        .get_plan(params.clone())
        .await
        .expect("read original preview")
    else {
        panic!("wrong kind")
    };
    assert_eq!(*plan, local_preview.repository);
    fs::write(&local, r#"{"id":"changed","packages":{}}"#)
        .expect("change contents after inspection");
    assert_code(
        actor
            .repository_register(local_id.clone(), "local-stale".into())
            .await
            .expect_err("changed preview"),
        "revision_conflict",
    );
    fs::write(&local, r#"{"id":"list-rpc-local","packages":{}}"#)
        .expect("restore reviewed content");
    let registered = actor
        .repository_register(local_id.clone(), "list-add-local".into())
        .await
        .expect("MCP registers a shared local repository");
    assert!(!registered.replayed);
    let offline = fixture.0.join("offline-repository.json");
    fs::rename(&local, &offline).expect("take owned source offline");
    assert_code(
        actor
            .repository_register(remote_id, "list-add-local".into())
            .await
            .expect_err("another source cannot replay the accepted key"),
        "idempotency_conflict",
    );
    assert!(
        actor
            .repository_register(local_id.clone(), "list-add-local".into())
            .await
            .expect("MCP replays its registration while the source file is offline")
            .replayed
    );
    fs::rename(offline, &local).expect("restore owned source");
    let owner_preview = client
        .repository_inspect(local_source)
        .await
        .expect("owner inspection")
        .preview_id
        .expect("owner preview ID");
    assert_code(
        client
            .repository_register(owner_preview, "list-add-local".into())
            .await
            .expect_err("local owner has an independent receipt"),
        "repository_already_registered",
    );
    let output = fixture.0.join("export.txt");
    assert_code(
        client
            .repository_configuration_export_file(
                output.to_str().expect("path").into(),
                old_preview.fingerprint,
                alcomd_protocol::RepositoryListFormat::Legacy,
                "export-fixture-1".into(),
            )
            .await
            .expect_err("stale preview"),
        "repository_list_changed",
    );
    assert!(!output.exists());
    let preview = client
        .repository_configuration_preview_export(alcomd_protocol::RepositoryListFormat::Legacy)
        .await
        .expect("fresh preview");
    assert_eq!(preview.exportable_count, 1);
    assert_eq!(preview.excluded.len(), 1);
    let exported = client
        .repository_configuration_export_file(
            output.to_str().expect("path").into(),
            preview.fingerprint.clone(),
            alcomd_protocol::RepositoryListFormat::Legacy,
            "export-fixture-2".into(),
        )
        .await
        .expect("export");
    let replay = client
        .repository_configuration_export_file(
            output.to_str().expect("path").into(),
            preview.fingerprint.clone(),
            alcomd_protocol::RepositoryListFormat::Legacy,
            "export-fixture-2".into(),
        )
        .await
        .expect("replay successful publication even though the file now exists");
    assert_eq!(replay.exported_count, 1);
    assert_eq!(exported.exported_count, 1);
    assert_eq!(
        fs::read_to_string(&output).expect("complete output"),
        format!("{url}\n")
    );
    assert_code(
        client
            .repository_configuration_export_file(
                output.to_str().expect("path").into(),
                preview.fingerprint.clone(),
                alcomd_protocol::RepositoryListFormat::Legacy,
                "export-fixture-3".into(),
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
            .repository_configuration_export_file(
                data.join("blocked.txt").to_str().expect("path").into(),
                preview.fingerprint,
                alcomd_protocol::RepositoryListFormat::Legacy,
                "export-fixture-4".into(),
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
    let page = client
        .repositories_list(None, Some(1))
        .await
        .expect("first page");
    let remote_id = page.repositories[0]
        .repository_id
        .clone()
        .expect("remote ID");
    let last_page = client
        .repositories_list(page.next_cursor.clone(), Some(1))
        .await
        .expect("second page");
    let local_id = last_page.repositories[0]
        .repository_id
        .clone()
        .expect("local ID");
    assert_eq!(page.order_fingerprint, last_page.order_fingerprint);
    let movement = RepositoryMoveParams {
        repository_id: local_id.clone(),
        before_repository_id: Some(remote_id.clone()),
        expected_order_fingerprint: page.order_fingerprint.clone(),
        idempotency_key: "move-local-first".into(),
    };
    let applied = client
        .repository_move(movement.clone())
        .await
        .expect("move");
    assert!(applied.changed && !applied.replayed);
    let replayed = client
        .repository_move(movement.clone())
        .await
        .expect("idempotent replay");
    assert!(replayed.changed && replayed.replayed);
    assert_code(
        client
            .repository_move(RepositoryMoveParams {
                idempotency_key: "competing-move".into(),
                ..movement
            })
            .await
            .expect_err("stale order"),
        "revision_conflict",
    );
    assert_code(
        client
            .repositories_list(page.next_cursor, Some(1))
            .await
            .expect_err("stale cursor"),
        "revision_conflict",
    );
    let moved = client
        .repositories_list(None, None)
        .await
        .expect("new order");
    assert_ne!(moved.order_fingerprint, page.order_fingerprint);
    assert_eq!(
        moved.repositories[0].repository_id.as_deref(),
        Some(local_id.as_str())
    );
    assert_eq!(
        moved.repositories[1].repository_id.as_deref(),
        Some(remote_id.as_str())
    );
    let no_op = client
        .repository_move(RepositoryMoveParams {
            repository_id: local_id.clone(),
            before_repository_id: Some(remote_id.clone()),
            expected_order_fingerprint: moved.order_fingerprint.clone(),
            idempotency_key: "confirm-same-position".into(),
        })
        .await
        .expect("no change");
    assert!(!no_op.changed && !no_op.replayed);
    assert_code(
        client
            .repository_move(RepositoryMoveParams {
                repository_id: alcomd_application::VRCHAT_OFFICIAL_REPOSITORY_ID.into(),
                before_repository_id: None,
                expected_order_fingerprint: moved.order_fingerprint.clone(),
                idempotency_key: "reject-builtin-move".into(),
            })
            .await
            .expect_err("built-in cannot be moved"),
        "invalid_request",
    );

    let native = alcomd_protocol::RepositoryListFormat::Native;
    let preview = actor
        .repository_configuration_preview_export(native)
        .await
        .expect("native RPC preview");
    assert_eq!(preview.exportable_count, 2, "native includes local sources");
    assert!(preview.excluded.is_empty());
    let native_path = fixture.0.join("repositories.alcomdrepositories");
    let path = native_path.to_string_lossy().into_owned();
    client
        .repository_configuration_export_file(
            path.clone(),
            preview.fingerprint,
            native,
            "native-export".into(),
        )
        .await
        .expect("native RPC export");
    let mut document: serde_json::Value =
        serde_json::from_slice(&fs::read(&native_path).expect("native file")).expect("native JSON");
    document["repositories"][0]["configuration"]["displayName"] =
        serde_json::json!("Restored by native RPC");
    document["repositories"][0]["configuration"]["enabled"] = serde_json::json!(false);
    document["repositories"][0]["configuration"]["disabledPackageIds"] =
        serde_json::json!(["com.example.future"]);
    fs::write(
        &native_path,
        serde_json::to_vec(&document).expect("document"),
    )
    .expect("owned edited file");
    let inspection = client
        .repository_list_inspect_file(path.clone())
        .await
        .expect("native inspection");
    assert_eq!(inspection.format, native);
    assert_eq!(
        inspection.entries[0]
            .configuration
            .as_ref()
            .expect("configuration")
            .display_name
            .as_deref(),
        Some("Restored by native RPC")
    );
    let import_key = "native-rpc-apply";
    let cancel = alcomd_protocol::RepositoryListCancelParams {
        idempotency_key: import_key.into(),
    };
    assert!(
        client
            .repository_list_cancel_registration(cancel.clone())
            .await
            .expect("the local owner can request an early stop for its own import")
            .stopped
    );
    let imported = actor
        .repository_list_import_file(alcomd_protocol::RepositoryListImportParams {
            path,
            expected_fingerprint: inspection.fingerprint,
            idempotency_key: import_key.into(),
            overwrite_existing: None,
        })
        .await
        .expect("default native overwrite");
    assert!(
        imported
            .items
            .contains(&alcomd_protocol::RepositoryListImportItem::Updated { line: 1 })
    );
    assert!(
        !actor
            .repository_list_cancel_registration(cancel.clone())
            .await
            .expect("completed MCP import")
            .stopped
    );
    assert!(
        client
            .repository_list_cancel_registration(cancel)
            .await
            .expect("the owner's early cancellation remains independent")
            .stopped
    );
    let settings = client
        .settings_get()
        .await
        .expect("restored settings over RPC");
    assert_eq!(settings.settings.packages.hidden_repository_ids.len(), 1);
    assert_eq!(
        settings.settings.packages.hidden_repository_packages[0].package_ids,
        ["com.example.future"]
    );
    let remote = actor
        .repository_get(remote_id.clone())
        .await
        .expect("MCP reads local registration")
        .repository;
    let name = alcomd_protocol::RepositorySetDisplayNameParams {
        repository_id: remote_id.clone(),
        expected_revision: remote.revision.expect("revision"),
        display_name: Some("MCP-managed repository".into()),
        idempotency_key: "cross-client-rename".into(),
    };
    let renamed = actor
        .repository_set_display_name(name.clone())
        .await
        .expect("rename shared repository");
    assert!(
        actor
            .repository_set_display_name(name.clone())
            .await
            .expect("MCP replays rename")
            .replayed
    );
    assert_code(
        client
            .repository_set_display_name(name)
            .await
            .expect_err("separate owner receipt"),
        "revision_conflict",
    );
    assert_eq!(
        client
            .repository_get(remote_id.clone())
            .await
            .expect("owner reads updated name")
            .repository
            .display_name,
        renamed.repository.display_name
    );
    let clear = alcomd_protocol::RepositorySetCredentialsParams {
        repository_id: remote_id.clone(),
        expected_revision: renamed.repository.revision.expect("renamed revision"),
        idempotency_key: "clear-shared-credentials".into(),
        credentials: None,
    };
    actor
        .repository_set_credentials(clear.clone())
        .await
        .expect("clear shared credential reference");
    assert!(
        actor
            .repository_set_credentials(clear)
            .await
            .expect("credential replay")
            .replayed
    );
    let local_revision = client
        .repository_get(local_id.clone())
        .await
        .expect("local repository")
        .repository
        .revision
        .expect("local revision");
    let refreshed = actor
        .repository_refresh(local_id.clone(), local_revision, "mcp-refresh".into())
        .await
        .expect("refresh local repository");
    assert!(
        actor
            .repository_refresh(local_id.clone(), local_revision, "mcp-refresh".into())
            .await
            .expect("MCP replays refresh")
            .replayed
    );
    assert_eq!(
        client
            .repository_get(local_id.clone())
            .await
            .expect("read refreshed shared record")
            .repository
            .revision,
        refreshed.repository.revision
    );
    let all = actor
        .repositories_refresh_all(alcomd_protocol::RepositoryRefreshAllParams {
            idempotency_key: "mcp-refresh-all".into(),
        })
        .await
        .expect("MCP refreshes shared repositories");
    assert!(
        all.items
            .contains(&alcomd_protocol::RepositoryRefreshAllItem::Refreshed {
                repository_id: local_id.clone()
            })
    );
    drop(actor);
    drop(client);
    stop.send(()).expect("stop");
    daemon.await.expect("join").expect("daemon result");
    let (stop, shutdown) = tokio::sync::oneshot::channel();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc,
        DataConfig::isolated(data),
        async {
            let _ = shutdown.await;
        },
    )
    .await
    .expect("reopen fixture");
    let mut client = AlcomdClient::connect(config.clone())
        .await
        .expect("reconnect");
    let reopened = client
        .repositories_list(None, None)
        .await
        .expect("persisted order");
    assert_eq!(
        reopened.repositories[0].repository_id.as_deref(),
        Some(local_id.as_str())
    );
    assert_eq!(
        reopened.repositories[1].repository_id.as_deref(),
        Some(remote_id.as_str())
    );
    assert_eq!(reopened.order_fingerprint, moved.order_fingerprint);
    let issued = mcp_actor::issue_actor(&mut client, "native-repository-remover").await;
    let mut actor = AlcomdClient::connect(config.with_mcp_credential(issued.credential))
        .await
        .expect("connect remover");
    let revision = reopened.repositories[0].revision.expect("revision");
    let removed = actor
        .repository_unregister(local_id.clone(), revision, "mcp-unregister".into())
        .await
        .expect("remove shared repository");
    assert!(!removed.replayed);
    assert!(
        actor
            .repository_unregister(local_id.clone(), revision, "mcp-unregister".into())
            .await
            .expect("replay removal")
            .replayed
    );
    assert_code(
        client
            .repository_get(local_id)
            .await
            .expect_err("removed from shared catalog"),
        "repository_not_registered",
    );
    drop(actor);
    drop(client);
    stop.send(()).expect("stop reopened fixture");
    daemon.await.expect("join").expect("daemon result");
}

#[tokio::test]
async fn accepted_authenticated_repository_writes_replay_without_network_or_staged_credentials() {
    use alcomd_application::{
        AccessContext, IdempotencyKey, M3Application, M3RegistryStore, RepositoryCredentials,
        RepositoryObservation, RepositorySource as Source,
    };

    let fixture = Fixture::new();
    let store = alcomd_store::StateStoreHandle::open(fixture.0.join("receipts.db")).expect("store");
    let access = AccessContext::local_owner();
    let owner = access.principal();
    let key = |text: &str| IdempotencyKey::parse(text.to_owned()).expect("key");
    let endpoint = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve fixture endpoint");
    let source = Source::Remote {
        url: format!(
            "https://{}/repository.json",
            endpoint.local_addr().expect("address")
        ),
    };
    drop(endpoint);
    let preview_id = alcomd_application::OperationId::new();
    let record = store
        .register_repository_authenticated(
            owner.clone(),
            alcomd_application::RepositoryRegistration {
                preview_id: Some(preview_id),
                requested_source: source.clone(),
                observation: RepositoryObservation {
                    source: source.clone(),
                    source_identity_key: vec![7],
                    declared_id: Some("fixture".into()),
                    name: None,
                    declared_url: None,
                    issues: vec![],
                    packages: vec![],
                    validators: Default::default(),
                    refreshed_at_ms: 1,
                },
            },
            uuid::Uuid::new_v4().to_string(),
            key("register"),
            1,
        )
        .await
        .expect("accepted registration")
        .value;
    let changed = store
        .set_repository_credential_reference(
            owner.clone(),
            record.repository_id,
            record.revision,
            Some(uuid::Uuid::new_v4().to_string()),
            key("replace"),
            2,
        )
        .await
        .expect("accepted credential replacement");
    let app = M3Application::new(store, alcomd_vpm::VpmReader::new().expect("reader"));
    let credentials = RepositoryCredentials {
        headers: [("X-Fixture".to_owned(), "temporary-value".to_owned())].into(),
    };
    let registered = app
        .register_repository_preview(&access, preview_id, key("register"))
        .await
        .expect("registration receipt with neither a live source nor a keyring record");
    let replaced = app
        .set_repository_credentials(
            &access,
            record.repository_id,
            record.revision,
            Some(credentials),
            key("replace"),
        )
        .await
        .expect("credential receipt without opening the source");
    assert!(registered.replayed && replaced.replayed);
    assert_eq!(registered.value, record);
    assert_eq!(replaced.value, changed.value);
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
