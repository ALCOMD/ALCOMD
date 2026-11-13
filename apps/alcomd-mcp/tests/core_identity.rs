use alcomd_client::{AlcomdClient, ClientConfig, ClientError};
use alcomd_platform::{DataConfig, IpcConfig};
use alcomd_protocol::*;
use std::{path::Path, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

fn denied<T>(result: Result<T, ClientError>) {
    assert!(
        matches!(result,Err(ClientError::Remote(ref error)) if error.code=="permission_denied")
    );
}
fn invisible_operation<T>(result: Result<T, ClientError>) {
    assert!(
        matches!(result,Err(ClientError::Remote(ref error)) if error.code=="operation_not_found")
    );
}
fn grant(permission: &str, kind: &str, id: &str) -> McpClientGrant {
    McpClientGrant {
        permission: permission.into(),
        resource_kind: kind.into(),
        resource_id: id.into(),
    }
}
fn permissions(project: &str) -> Vec<McpClientGrant> {
    let mut grants = vec![grant("projects.read", "project", project)];
    for permission in [
        "state.check",
        "operations.read",
        "operations.cancel",
        "events.read",
        "repositories.read",
        "packages.read",
    ] {
        grants.push(grant(permission, "global", "*"));
    }
    grants
}
fn mutation(id: &str, revision: u64, key: &str) -> McpClientMutationParams {
    McpClientMutationParams {
        client_id: id.into(),
        expected_revision: revision,
        idempotency_key: key.into(),
    }
}
fn project(root: &Path) {
    std::fs::create_dir_all(root.join("ProjectSettings")).expect("isolated settings");
    std::fs::create_dir_all(root.join("Packages/com.example.fixture")).expect("isolated package");
    std::fs::write(
        root.join("ProjectSettings/ProjectVersion.txt"),
        "m_EditorVersion: 2022.3.22f1\n",
    )
    .expect("version");
    std::fs::write(
        root.join("Packages/manifest.json"),
        r#"{"dependencies":{}}"#,
    )
    .expect("UPM");
    std::fs::write(root.join("Packages/vpm-manifest.json"),r#"{"dependencies":{"com.example.fixture":{"version":"1.0.0"}},"locked":{"com.example.fixture":{"version":"1.0.0"}}}"#).expect("VPM");
    std::fs::write(
        root.join("Packages/com.example.fixture/package.json"),
        r#"{"name":"com.example.fixture","version":"1.0.0"}"#,
    )
    .expect("package");
}
async fn terminal(client: &mut AlcomdClient, id: &str) {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let op = client
                .operation_get(id.into())
                .await
                .expect("owned operation");
            if matches!(
                op.state,
                OperationState::Succeeded | OperationState::Failed | OperationState::Cancelled
            ) {
                assert_eq!(op.state, OperationState::Succeeded);
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("operation terminal deadline");
}
async fn bridge_discover<W: tokio::io::AsyncWrite + Unpin, R: tokio::io::AsyncRead + Unpin>(
    input: &mut W,
    output: &mut BufReader<R>,
    id: u32,
) -> serde_json::Value {
    let request = serde_json::json!({"jsonrpc":"2.0","id":id,"method":"server/discover","params":{"_meta":{"io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}});
    input
        .write_all(format!("{request}\n").as_bytes())
        .await
        .expect("protocol request");
    let mut response = String::new();
    tokio::time::timeout(Duration::from_secs(10), output.read_line(&mut response))
        .await
        .expect("protocol deadline")
        .expect("protocol response");
    serde_json::from_str(&response).expect("protocol JSON")
}

/// Requires exclusive per-user IPC ownership on Windows. Never run against an
/// existing daemon: test_support binds before any test client is constructed.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn m8_client_identity_real_core_isolation_rotation_revocation_and_restart() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-identity-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).expect("owned fixture");
    let root = root.canonicalize().expect("fixture path");
    let data = root.join("data");
    std::fs::create_dir(&data).expect("isolated data");
    #[cfg(windows)]
    let (ipc, config) = (
        IpcConfig::default(),
        ClientConfig::default().without_daemon_start(),
    );
    #[cfg(unix)]
    let (ipc, config) = {
        #[cfg(target_os = "macos")]
        let base = std::path::PathBuf::from("/private/tmp");
        #[cfg(not(target_os = "macos"))]
        let base = std::env::temp_dir();
        let runtime = base.join(format!("am8-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir(&runtime).expect("isolated runtime");
        (
            IpcConfig::isolated(runtime.clone()),
            ClientConfig::default()
                .without_daemon_start()
                .with_runtime_directory(runtime),
        )
    };
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc.clone(),
        DataConfig::isolated(data.clone()),
        std::future::pending(),
    )
    .await
    .expect("bind isolated Core before connecting");
    let mut owner = AlcomdClient::connect(config.clone())
        .await
        .expect("native owner");
    let p1 = root.join("ProjectOne");
    let p2 = root.join("ProjectTwo");
    project(&p1);
    project(&p2);
    let original_manifest =
        std::fs::read(p1.join("Packages/vpm-manifest.json")).expect("manifest before");
    let first = owner
        .project_register(p1.to_string_lossy().into_owned(), "project-one".into())
        .await
        .expect("project one")
        .project;
    let second = owner
        .project_register(p2.to_string_lossy().into_owned(), "project-two".into())
        .await
        .expect("project two")
        .project;
    let p1id = first.project_id.clone().expect("ID");
    let p2id = second.project_id.clone().expect("ID");
    let mut a = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "A".into(),
            grants: vec![],
            idempotency_key: "create-a".into(),
        })
        .await
        .expect("create A");
    assert!(a.grants.is_empty());
    let replay = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "A".into(),
            grants: vec![],
            idempotency_key: "create-a".into(),
        })
        .await
        .expect("create replay");
    assert_eq!(replay.client_id, a.client_id);
    let b = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "B".into(),
            grants: vec![],
            idempotency_key: "create-b".into(),
        })
        .await
        .expect("create B");
    let issued_a = owner
        .mcp_client_issue_credential(mutation(&a.client_id, a.revision, "issue-a"))
        .await
        .expect("issue A");
    a.revision = issued_a.revision;
    let issued_b = owner
        .mcp_client_issue_credential(mutation(&b.client_id, b.revision, "issue-b"))
        .await
        .expect("issue B");
    assert!(issued_a.credential != issued_b.credential);
    assert!(
        matches!(owner.mcp_client_issue_credential(mutation(&a.client_id,1,"issue-a")).await,Err(ClientError::Remote(ref e)) if e.code=="revision_conflict")
    );
    assert!(
        AlcomdClient::connect(config.clone().with_mcp_credential("invalid".repeat(12)))
            .await
            .is_err()
    );
    let mut ca = AlcomdClient::connect(
        config
            .clone()
            .with_client_name("alcomd-gui")
            .with_mcp_credential(issued_a.credential.clone()),
    )
    .await
    .expect("A bound, not clientInfo identity");
    let mut cb = AlcomdClient::connect(
        config
            .clone()
            .with_mcp_credential(issued_b.credential.clone()),
    )
    .await
    .expect("B bound");
    denied(ca.project_get(p1id.clone()).await);
    denied(ca.state_check("must-not-run".into()).await);
    denied(ca.mcp_clients_list(McpClientsListParams::default()).await);
    let page = owner
        .mcp_clients_list(McpClientsListParams {
            cursor: None,
            limit: Some(1),
        })
        .await
        .expect("preview page");
    a = owner
        .mcp_client_set_grants(McpClientsSetGrantsParams {
            client_id: a.client_id.clone(),
            expected_revision: a.revision,
            idempotency_key: "grant-a".into(),
            grants: permissions(&p1id),
        })
        .await
        .expect("grant A");
    assert!(
        matches!(owner.mcp_clients_list(McpClientsListParams{cursor:page.next_cursor,limit:Some(1)}).await,Err(ClientError::Remote(ref e)) if e.code=="revision_conflict")
    );
    owner
        .mcp_client_set_grants(McpClientsSetGrantsParams {
            client_id: b.client_id.clone(),
            expected_revision: issued_b.revision,
            idempotency_key: "grant-b".into(),
            grants: permissions(&p2id),
        })
        .await
        .expect("grant B");
    assert!(ca.project_get(p1id.clone()).await.is_ok());
    denied(ca.project_get(p2id.clone()).await);
    denied(cb.project_get(p1id.clone()).await);
    let visible = ca
        .projects_list(None, Some(100))
        .await
        .expect("filtered collection");
    assert_eq!(visible.projects.len(), 1);
    assert_eq!(
        visible.projects[0].project_id.as_deref(),
        Some(p1id.as_str())
    );
    let oa = ca.state_check("same-key".into()).await.expect("A write");
    let ob = cb.state_check("same-key".into()).await.expect("B write");
    assert_ne!(oa.operation_id, ob.operation_id);
    assert_eq!(
        ca.state_check("same-key".into())
            .await
            .expect("A idempotency")
            .operation_id,
        oa.operation_id
    );
    invisible_operation(cb.operation_get(oa.operation_id.clone()).await);
    invisible_operation(ca.operation_get(ob.operation_id.clone()).await);
    terminal(&mut ca, &oa.operation_id).await;
    terminal(&mut cb, &ob.operation_id).await;
    let a_operations = ca
        .operations_list(None, Some(100), false)
        .await
        .expect("A tasks");
    assert_eq!(a_operations.operations.len(), 1);
    assert_eq!(a_operations.operations[0].operation_id, oa.operation_id);
    let a_events = ca.events_list(0, Some(1000)).await.expect("A events");
    assert!(!a_events.events.is_empty());
    assert!(
        a_events
            .events
            .iter()
            .all(|event| event.aggregate_id == oa.operation_id)
    );
    invisible_operation(
        cb.operation_cancel(oa.operation_id.clone(), 1, "foreign-cancel".into())
            .await,
    );
    let detached = ca
        .state_check("accepted-before-disconnect".into())
        .await
        .expect("accepted before disconnect");
    drop(ca);
    let mut ca = AlcomdClient::connect(
        config
            .clone()
            .with_mcp_credential(issued_a.credential.clone()),
    )
    .await
    .expect("reconnect same identity");
    terminal(&mut ca, &detached.operation_id).await;
    let plan = ca
        .package_plan_remove(PackagePlanRemoveParams {
            project_id: p1id.clone(),
            expected_revision: first.revision.expect("revision"),
            package_id: "com.example.fixture".into(),
        })
        .await
        .expect("A read-only plan");
    denied(
        cb.package_apply_plan(PackageApplyPlanParams {
            plan_id: plan.plan_id.clone(),
            expected_revision: plan.project_revision,
            idempotency_key: "foreign-plan".into(),
        })
        .await,
    );
    a = owner
        .mcp_client_set_grants(McpClientsSetGrantsParams {
            client_id: a.client_id.clone(),
            expected_revision: a.revision,
            idempotency_key: "change-scope".into(),
            grants: permissions(&p2id),
        })
        .await
        .expect("scope change while connected");
    denied(ca.project_get(p1id.clone()).await);
    assert!(ca.project_get(p2id.clone()).await.is_ok());
    a = owner
        .mcp_client_set_grants(McpClientsSetGrantsParams {
            client_id: a.client_id.clone(),
            expected_revision: a.revision,
            idempotency_key: "remove-operation-permissions".into(),
            grants: vec![grant("projects.read", "project", &p2id)],
        })
        .await
        .expect("remove permissions while connected");
    denied(ca.state_check("removed-permission".into()).await);
    denied(ca.operations_list(None, Some(100), false).await);
    denied(ca.events_list(0, Some(100)).await);
    assert!(ca.project_get(p2id.clone()).await.is_ok());
    a = owner
        .mcp_client_set_grants(McpClientsSetGrantsParams {
            client_id: a.client_id.clone(),
            expected_revision: a.revision,
            idempotency_key: "restore-explicit-permissions".into(),
            grants: permissions(&p2id),
        })
        .await
        .expect("explicitly regrant, no implicit restoration");
    let bridge_client = alcomd_mcp::connect_core(config.clone(), issued_a.credential.clone())
        .await
        .expect("STDIO existing typed client");
    let (peer, server) = tokio::io::duplex(65536);
    let (server_input, server_output) = tokio::io::split(server);
    let (peer_input, mut peer_output) = tokio::io::split(peer);
    let mut peer_input = BufReader::new(peer_input);
    let bridge = tokio::spawn(alcomd_mcp::serve_stdio_with_client(
        server_input,
        server_output,
        Some(bridge_client),
    ));
    // Exercise the actual Windows executable and controlled startup environment,
    // using the same isolated per-user Core endpoint already bound above.
    #[cfg(windows)]
    let (mut child, mut child_input, mut child_output) = {
        let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_alcomd-mcp"))
            .arg("stdio")
            .env("ALCOMD_MCP_CLIENT_CREDENTIAL", &issued_a.credential)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .kill_on_drop(true)
            .spawn()
            .expect("owned STDIO process");
        let input = child.stdin.take().expect("STDIO input");
        let output = BufReader::new(child.stdout.take().expect("STDIO output"));
        (child, input, output)
    };
    #[cfg(windows)]
    assert!(
        bridge_discover(&mut child_input, &mut child_output, 1)
            .await
            .get("result")
            .is_some()
    );
    assert!(
        bridge_discover(&mut peer_output, &mut peer_input, 1)
            .await
            .get("result")
            .is_some()
    );
    let rotated = owner
        .mcp_client_issue_credential(mutation(&a.client_id, a.revision, "rotate-a"))
        .await
        .expect("rotation");
    denied(ca.system_status().await);
    #[cfg(windows)]
    {
        assert!(
            bridge_discover(&mut child_input, &mut child_output, 2)
                .await
                .get("error")
                .is_some()
        );
        drop(child_input);
        let status = tokio::time::timeout(Duration::from_secs(10), child.wait())
            .await
            .expect("STDIO EOF deadline")
            .expect("STDIO exit");
        assert!(status.success());
        use tokio::io::AsyncReadExt;
        let mut stderr = Vec::new();
        child
            .stderr
            .take()
            .expect("stderr")
            .read_to_end(&mut stderr)
            .await
            .expect("stderr evidence");
        assert!(
            !stderr
                .windows(issued_a.credential.len())
                .any(|w| w == issued_a.credential.as_bytes()),
            "credential exposed on stderr"
        );
    }
    assert!(
        bridge_discover(&mut peer_output, &mut peer_input, 2)
            .await
            .get("error")
            .is_some()
    );
    drop(peer_output);
    drop(peer_input);
    tokio::time::timeout(Duration::from_secs(10), bridge)
        .await
        .expect("bridge EOF")
        .expect("join")
        .expect("bridge closes without cancelling operations");
    assert!(
        AlcomdClient::connect(
            config
                .clone()
                .with_mcp_credential(issued_a.credential.clone())
        )
        .await
        .is_err()
    );
    drop(ca);
    drop(cb);
    drop(owner);
    daemon.abort();
    let _ = daemon.await;
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc.clone(),
        DataConfig::isolated(data.clone()),
        std::future::pending(),
    )
    .await
    .expect("restart same isolated database");
    let mut owner = AlcomdClient::connect(config.clone())
        .await
        .expect("owner after restart");
    let mut ca = AlcomdClient::connect(
        config
            .clone()
            .with_mcp_credential(rotated.credential.clone()),
    )
    .await
    .expect("persisted credential");
    assert!(ca.project_get(p2id.clone()).await.is_ok());
    denied(ca.project_get(p1id.clone()).await);
    assert_eq!(
        ca.state_check("same-key".into())
            .await
            .expect("persisted idempotency")
            .operation_id,
        oa.operation_id
    );
    owner
        .mcp_client_revoke(mutation(&a.client_id, rotated.revision, "revoke-a"))
        .await
        .expect("revoke");
    denied(ca.system_status().await);
    assert!(
        AlcomdClient::connect(
            config
                .clone()
                .with_mcp_credential(rotated.credential.clone())
        )
        .await
        .is_err()
    );
    let mut cb = AlcomdClient::connect(
        config
            .clone()
            .with_mcp_credential(issued_b.credential.clone()),
    )
    .await
    .expect("other client unaffected");
    assert!(cb.project_get(p2id.clone()).await.is_ok());
    terminal(&mut cb, &ob.operation_id).await;
    drop(ca);
    drop(cb);
    drop(owner);
    daemon.abort();
    let _ = daemon.await;
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc,
        DataConfig::isolated(data.clone()),
        std::future::pending(),
    )
    .await
    .expect("restart after revocation");
    assert!(
        AlcomdClient::connect(
            config
                .clone()
                .with_mcp_credential(rotated.credential.clone())
        )
        .await
        .is_err()
    );
    let mut owner = AlcomdClient::connect(config.clone())
        .await
        .expect("owner after revoked restart");
    let clients = owner
        .mcp_clients_list(McpClientsListParams::default())
        .await
        .expect("persisted client states");
    assert!(
        clients
            .items
            .iter()
            .find(|client| client.client_id == a.client_id)
            .expect("revoked record retained")
            .revoked_at_ms
            .is_some()
    );
    let mut cb = AlcomdClient::connect(config.with_mcp_credential(issued_b.credential.clone()))
        .await
        .expect("B persists after second restart");
    assert!(cb.project_get(p2id.clone()).await.is_ok());
    drop(cb);
    drop(owner);
    daemon.abort();
    let _ = daemon.await;
    assert_eq!(
        std::fs::read(p1.join("Packages/vpm-manifest.json")).expect("manifest after"),
        original_manifest
    );
    let store = alcomd_store::StateStoreHandle::open(data.join("state.db"))
        .expect("isolated durable evidence");
    use alcomd_application::M4Store;
    let a_principal =
        alcomd_domain::PrincipalId::parse(a.principal_id.clone()).expect("A Principal");
    let b_principal =
        alcomd_domain::PrincipalId::parse(b.principal_id.clone()).expect("B Principal");
    let plan_id = alcomd_domain::PlanId::parse(&plan.plan_id).expect("plan ID");
    assert!(store.get_package_plan(a_principal, plan_id).await.is_ok());
    assert!(
        matches!(store.get_package_plan(b_principal,plan_id).await,Err(ref error) if error.code()==alcomd_application::M4ErrorCode::PlanNotFound)
    );
    drop(store);
    let db = rusqlite::Connection::open_with_flags(
        data.join("state.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("read-only evidence");
    let resource_owner: String = db
        .query_row(
            "SELECT owner_principal_id FROM projects WHERE project_id=?1",
            [&p1id],
            |r| r.get(0),
        )
        .expect("resource owner");
    assert_eq!(resource_owner, "builtin:local-owner");
    let credentials_in_response:u32=db.query_row("SELECT count(*) FROM idempotency_records WHERE method='mcp.clients.issueCredential' AND (instr(response_json,?1)>0 OR instr(response_json,?2)>0 OR instr(response_json,?3)>0)",rusqlite::params![issued_a.credential,issued_b.credential,rotated.credential],|r|r.get(0)).expect("no durable plaintext");
    assert_eq!(credentials_in_response, 0);
    let isolated_keys:u32=db.query_row("SELECT count(DISTINCT principal_id) FROM idempotency_records WHERE method='state.check' AND idempotency_key='same-key'",[],|r|r.get(0)).expect("principal-isolated keys");
    assert_eq!(isolated_keys, 2);
    fn scan(directory: &Path, secrets: &[&[u8]]) {
        for entry in std::fs::read_dir(directory).expect("owned data directory") {
            let path = entry.expect("entry").path();
            if path.is_dir() {
                scan(&path, secrets);
            } else {
                let bytes = std::fs::read(path).expect("read owned evidence");
                for secret in secrets {
                    assert!(
                        !bytes.windows(secret.len()).any(|window| window == *secret),
                        "plaintext credential in persisted Core data"
                    );
                }
            }
        }
    }
    scan(
        &data,
        &[
            issued_a.credential.as_bytes(),
            issued_b.credential.as_bytes(),
            rotated.credential.as_bytes(),
        ],
    );
    println!(
        "Identity closure verified; isolated evidence: {}",
        root.display()
    );
}
