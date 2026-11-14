use super::*;
use alcomd_protocol::{
    McpClientMutationParams, McpClientsCreateParams, McpClientsListParams,
    McpClientsSetGrantsParams, UiFieldValue, UiSnapshot, UiSubmittedField,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn management_component_uses_real_core_grants_and_loses_access_after_revocation() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    fs::create_dir(&runtime).expect("runtime");
    fs::create_dir(&data).expect("data");
    let package = fixture.path().join("management.alcomdextension");
    create_signed_package_with_interfaces(
        &package,
        include_bytes!("../../../../crates/alcomd-testing/fixtures/m8/mcp-management-v1.wasm"),
        "MCP management fixture",
        "[permissions]\nrequired = [\"mcp.clients.read\"]\noptional = [\"mcp.clients.manage\",\"projects.read\"]\n\n[ui]\nprotocol = \"portable-v1\"\n",
        &["alcomd:mcp-management/clients@1.0.0"],
        "AGPL-3.0-only",
    );
    let (ipc, config) = isolated_ipc(runtime);
    let raw_ipc = ipc.clone();
    let shutdown = Arc::new(AtomicBool::new(false));
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc,
        DataConfig::isolated(data.clone()),
        wait_for_shutdown(Arc::clone(&shutdown)),
    )
    .await
    .expect("isolated daemon");
    let mut client = connect_with_retry(config.clone()).await;
    let created = client
        .mcp_client_create(McpClientsCreateParams {
            display_name: "真实 Core 客户端".into(),
            idempotency_key: "management-seed".into(),
            grants: vec![],
        })
        .await
        .expect("real record");
    assert!(created.grants.is_empty());
    let params = ExtensionPlanInstallParams {
        source_kind: ExtensionSourceKind::LocalOwnerSelected,
        package_path: package.to_string_lossy().into_owned(),
        expected_revision: 0,
        publisher_approval: ExtensionPublisherApproval::None,
    };
    assert!(
        matches!(client.extension_plan_install(params.clone()).await,Err(ClientError::Remote(error)) if error.code=="extension_publisher_confirmation_required")
    );
    let plan = client
        .extension_plan_install(ExtensionPlanInstallParams {
            publisher_approval: ExtensionPublisherApproval::ApproveForExtension,
            ..params
        })
        .await
        .expect("explicit trust")
        .plan;
    assert_eq!(plan.required_permissions, vec!["mcp.clients.read"]);
    let install = client
        .extension_apply_install(ExtensionApplyParams {
            plan_id: plan.plan_id,
            idempotency_key: "management-install".into(),
        })
        .await
        .expect("install");
    assert_eq!(
        wait_for_terminal(&mut client, &install.operation_id).await,
        OperationState::Succeeded
    );
    let extension = client
        .extension_get("dev.alcomd.fixture".into())
        .await
        .expect("extension")
        .extension;
    assert!(
        matches!(client.extension_enable(ExtensionLifecycleParams{extension_id:extension.extension_id.clone(),expected_revision:extension.revision,idempotency_key:"management-no-read".into()}).await,Err(ClientError::Remote(error)) if error.code=="permission_denied" || error.code=="extension_permission_denied")
    );
    let unchanged = client
        .extension_get(extension.extension_id.clone())
        .await
        .expect("authority after denied enable")
        .extension;
    assert_eq!(
        unchanged.desired_state,
        ExtensionDesiredState::InstalledDisabled
    );
    assert_eq!(unchanged.revision, extension.revision);
    let read = client
        .extension_set_grant(ExtensionGrantParams {
            extension_id: extension.extension_id.clone(),
            permission: "mcp.clients.read".into(),
            resource_kind: "Global".into(),
            resource_id: "*".into(),
            expected_grant_revision: extension.grant_revision,
            idempotency_key: "management-read".into(),
        })
        .await
        .expect("explicit global read grant");
    let extension = client
        .extension_get(extension.extension_id)
        .await
        .expect("granted")
        .extension;
    client
        .extension_enable(ExtensionLifecycleParams {
            extension_id: extension.extension_id.clone(),
            expected_revision: extension.revision,
            idempotency_key: "management-enable".into(),
        })
        .await
        .expect("enable");
    let opened = client
        .extension_ui_open(ExtensionUiOpenParams {
            extension_id: extension.extension_id.clone(),
            locale: "zh-CN".into(),
        })
        .await
        .expect("real new-world guest UI");
    let document = serde_json::to_string(&opened.snapshot.document).expect("safe document");
    assert!(document.contains("真实 Core 客户端"));
    assert!(document.contains("查看权限"));
    assert!(!document.contains("创建客户端"));
    assert!(!document.contains("credential"));
    // Closing/reopening exercises the real Host lifetime, not a renderer fixture.
    client
        .extension_ui_close(ExtensionUiCloseParams {
            session_id: opened.session.session_id,
        })
        .await
        .expect("close");
    let reopened = client
        .extension_ui_open(ExtensionUiOpenParams {
            extension_id: extension.extension_id.clone(),
            locale: "zh-CN".into(),
        })
        .await
        .expect("reopen");
    client
        .extension_revoke_grant(ExtensionGrantParams {
            extension_id: extension.extension_id.clone(),
            permission: "mcp.clients.read".into(),
            resource_kind: "Global".into(),
            resource_id: "*".into(),
            expected_grant_revision: read.grant_revision,
            idempotency_key: "management-revoke-read".into(),
        })
        .await
        .expect("revoke while connected");
    assert!(
        client
            .extension_ui_refresh(ExtensionUiRefreshParams {
                session_id: reopened.session.session_id,
                expected_snapshot_revision: reopened.snapshot.snapshot_revision
            })
            .await
            .is_err()
    );
    let clients = client
        .mcp_clients_list(McpClientsListParams::default())
        .await
        .expect("owner still reads authoritative records");
    assert_eq!(clients.items.len(), 1);
    assert_eq!(clients.items[0], created);
    let first = fixture.path().join("AllowedProject");
    let second = fixture.path().join("OutsideProject");
    for path in [&first, &second] {
        fs::create_dir_all(path.join("ProjectSettings")).expect("isolated project settings");
        fs::create_dir_all(path.join("Packages")).expect("isolated packages");
        fs::write(
            path.join("ProjectSettings/ProjectVersion.txt"),
            "m_EditorVersion: 2022.3.22f1\n",
        )
        .expect("version");
        fs::write(path.join("Packages/manifest.json"), "{\"dependencies\":{}}").expect("UPM");
        fs::write(
            path.join("Packages/vpm-manifest.json"),
            "{\"dependencies\":{},\"locked\":{}}",
        )
        .expect("VPM");
    }
    let project = client
        .project_register(
            first.to_string_lossy().into_owned(),
            "management-project".into(),
        )
        .await
        .expect("register disposable project")
        .project;
    let outside = client
        .project_register(
            second.to_string_lossy().into_owned(),
            "management-outside".into(),
        )
        .await
        .expect("register outside fixture")
        .project;
    let project_id = project.project_id.expect("ID");
    let outside_id = outside.project_id.expect("ID");
    for (index, (permission, kind, id)) in [
        ("mcp.clients.read", "Global", "*"),
        ("mcp.clients.manage", "Global", "*"),
        ("projects.read", "Project", project_id.as_str()),
    ]
    .into_iter()
    .enumerate()
    {
        let current = client
            .extension_get(extension.extension_id.clone())
            .await
            .expect("current extension")
            .extension;
        client
            .extension_set_grant(ExtensionGrantParams {
                extension_id: extension.extension_id.clone(),
                permission: permission.into(),
                resource_kind: kind.into(),
                resource_id: id.into(),
                expected_grant_revision: current.grant_revision,
                idempotency_key: format!("management-authorize-{index}"),
            })
            .await
            .expect("explicit management/project grants");
    }
    let opened = client
        .extension_ui_open(ExtensionUiOpenParams {
            extension_id: extension.extension_id.clone(),
            locale: "zh-CN".into(),
        })
        .await
        .expect("management UI");
    let mut snapshot = opened.snapshot;
    let mut sequence = 0;
    assert!(
        serde_json::to_string(&snapshot.document)
            .expect("document")
            .contains("创建客户端")
    );
    dispatch(
        &mut client,
        &mut snapshot,
        &mut sequence,
        UiAction::SubmitForm {
            action_id: "create".into(),
            values: vec![text("name", "Created through real guest")],
        },
    )
    .await;
    let record = client
        .mcp_clients_list(McpClientsListParams::default())
        .await
        .expect("real created record")
        .items
        .into_iter()
        .find(|item| item.display_name == "Created through real guest")
        .expect("new client");
    assert!(record.grants.is_empty());
    assert_eq!(record.principal_id, format!("mcp:{}", record.client_id));
    let issued = client
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: record.client_id.clone(),
            expected_revision: record.revision,
            idempotency_key: "management-client-credential".into(),
        })
        .await
        .expect("owner-only one-time issue");
    let mut mcp = AlcomdClient::connect(
        config
            .clone()
            .with_mcp_credential(issued.credential.clone()),
    )
    .await
    .expect("independent authenticated client");
    assert!(mcp.project_get(project_id.clone()).await.is_err());
    refresh(&mut client, &mut snapshot).await;
    let document = serde_json::to_string(&snapshot.document).expect("scope document");
    assert!(document.contains("AllowedProject"));
    assert!(!document.contains(&outside_id));
    let bad = client
        .extension_ui_dispatch(ExtensionUiDispatchParams {
            session_id: snapshot.session_id.clone(),
            expected_snapshot_revision: snapshot.snapshot_revision,
            sequence: sequence + 1,
            request_id: uuid::Uuid::new_v4().to_string(),
            action: grant_form("projects.read", &format!("project-{outside_id}")),
        })
        .await;
    assert!(
        matches!(bad,Err(ClientError::Remote(error)) if error.code=="extension_ui_action_invalid")
    );
    dispatch(
        &mut client,
        &mut snapshot,
        &mut sequence,
        grant_form("state.check", "global"),
    )
    .await;
    dispatch(
        &mut client,
        &mut snapshot,
        &mut sequence,
        activate("cancel"),
    )
    .await;
    assert!(
        current_client(&mut client, &record.client_id)
            .await
            .grants
            .is_empty()
    );
    add(
        &mut client,
        &mut snapshot,
        &mut sequence,
        "state.check",
        "global",
    )
    .await;
    add(
        &mut client,
        &mut snapshot,
        &mut sequence,
        "projects.read",
        &format!("project-{project_id}"),
    )
    .await;
    assert!(mcp.project_get(project_id.clone()).await.is_ok());
    assert!(mcp.project_get(outside_id.clone()).await.is_err());
    let current = current_client(&mut client, &record.client_id).await;
    let retained = current
        .grants
        .iter()
        .filter(|grant| grant.permission != "state.check")
        .cloned()
        .collect();
    client
        .mcp_client_set_grants(McpClientsSetGrantsParams {
            client_id: record.client_id.clone(),
            expected_revision: current.revision,
            grants: retained,
            idempotency_key: "management-concurrent-change".into(),
        })
        .await
        .expect("real concurrent change");
    add(
        &mut client,
        &mut snapshot,
        &mut sequence,
        "events.read",
        "global",
    )
    .await;
    assert!(
        serde_json::to_string(&snapshot.document)
            .expect("readable conflict")
            .contains("记录已变化")
    );
    assert!(
        !current_client(&mut client, &record.client_id)
            .await
            .grants
            .iter()
            .any(|g| g.permission == "events.read")
    );
    refresh(&mut client, &mut snapshot).await;
    add(
        &mut client,
        &mut snapshot,
        &mut sequence,
        "events.read",
        "global",
    )
    .await;
    let document = serde_json::to_value(&snapshot.document).expect("public UI tree");
    let nodes = document["nodes"].as_array().expect("nodes");
    let row = nodes
        .iter()
        .find(|node| {
            node["kind"] == "list-item"
                && node["payload"]["label"]
                    .as_str()
                    .is_some_and(|label| label.contains(&project_id))
        })
        .expect("project scope row");
    let remove = nodes
        .iter()
        .find(|node| node["kind"] == "button" && node["parentId"] == row["nodeId"])
        .expect("published removal action")["payload"]["actionId"]
        .as_str()
        .expect("action ID")
        .to_owned();
    dispatch(&mut client, &mut snapshot, &mut sequence, activate(&remove)).await;
    dispatch(
        &mut client,
        &mut snapshot,
        &mut sequence,
        activate("confirm"),
    )
    .await;
    assert!(mcp.project_get(project_id.clone()).await.is_err());
    dispatch(
        &mut client,
        &mut snapshot,
        &mut sequence,
        activate("revoke"),
    )
    .await;
    assert!(
        current_client(&mut client, &record.client_id)
            .await
            .revoked_at_ms
            .is_none()
    );
    dispatch(
        &mut client,
        &mut snapshot,
        &mut sequence,
        activate("confirm"),
    )
    .await;
    assert!(
        current_client(&mut client, &record.client_id)
            .await
            .revoked_at_ms
            .is_some()
    );
    assert!(mcp.system_status().await.is_err());
    assert!(
        AlcomdClient::connect(config.with_mcp_credential(issued.credential))
            .await
            .is_err()
    );
    assert_eq!(
        current_client(&mut client, &created.client_id).await,
        created
    );
    assert_eq!(
        fs::read_to_string(first.join("Packages/manifest.json")).expect("unchanged real files"),
        "{\"dependencies\":{}}"
    );
    accepted_ui_write_survives_lost_reply_and_connection_close(
        &raw_ipc,
        &mut client,
        &extension.extension_id,
    )
    .await;
    client
        .extension_ui_close(ExtensionUiCloseParams {
            session_id: snapshot.session_id,
        })
        .await
        .expect("close final UI");
    let current = client
        .extension_get(extension.extension_id.clone())
        .await
        .expect("current extension")
        .extension;
    let disabled = client
        .extension_disable(ExtensionLifecycleParams {
            extension_id: extension.extension_id,
            expected_revision: current.revision,
            idempotency_key: "management-disable".into(),
        })
        .await
        .expect("disable")
        .extension;
    let uninstall = client
        .extension_plan_uninstall(ExtensionPlanUninstallParams {
            extension_id: disabled.extension_id,
            expected_revision: disabled.revision,
            data_disposition: ExtensionDataDisposition::RetainData,
        })
        .await
        .expect("uninstall plan")
        .plan;
    let removed = client
        .extension_apply_uninstall(ExtensionApplyParams {
            plan_id: uninstall.plan_id,
            idempotency_key: "management-uninstall".into(),
        })
        .await
        .expect("uninstall");
    assert_eq!(
        wait_for_terminal(&mut client, &removed.operation_id).await,
        OperationState::Succeeded
    );
    let kept = client
        .mcp_clients_list(McpClientsListParams::default())
        .await
        .expect("extension removal keeps global clients");
    assert_eq!(kept.items.len(), 3);
    if let Some(destination) = std::env::var_os("ALCOMD_TEST_MCP_MANAGEMENT_PACKAGE_OUTPUT") {
        let destination = PathBuf::from(destination);
        let target = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target")
            .canonicalize()
            .expect("workspace target");
        assert!(
            destination.is_absolute()
                && destination
                    .parent()
                    .expect("parent")
                    .canonicalize()
                    .expect("owned output parent")
                    .starts_with(&target)
        );
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .expect("new development package output");
        output
            .write_all(&fs::read(&package).expect("verified test package"))
            .expect("persist development package");
    }
    shutdown.store(true, Ordering::Release);
    assert!(daemon.await.expect("join isolated daemon").is_ok());
}

fn text(id: &str, value: &str) -> UiSubmittedField {
    UiSubmittedField {
        field_id: id.into(),
        value: UiFieldValue::Text {
            value: value.into(),
        },
    }
}
fn grant_form(permission: &str, scope: &str) -> UiAction {
    UiAction::SubmitForm {
        action_id: "add".into(),
        values: vec![
            UiSubmittedField {
                field_id: "permission".into(),
                value: UiFieldValue::Selection {
                    value: permission.into(),
                },
            },
            UiSubmittedField {
                field_id: "project".into(),
                value: UiFieldValue::Selection {
                    value: scope.into(),
                },
            },
        ],
    }
}
fn activate(id: &str) -> UiAction {
    UiAction::Activate {
        action_id: id.into(),
    }
}
async fn dispatch(
    client: &mut AlcomdClient,
    snapshot: &mut UiSnapshot,
    sequence: &mut u64,
    action: UiAction,
) {
    // Respect the unchanged session bucket (burst 10, refill one action/sec).
    // This is deliberate input pacing, never retrying a refused write.
    tokio::time::sleep(Duration::from_millis(1050)).await;
    let result = client
        .extension_ui_dispatch(ExtensionUiDispatchParams {
            session_id: snapshot.session_id.clone(),
            expected_snapshot_revision: snapshot.snapshot_revision,
            sequence: *sequence + 1,
            request_id: uuid::Uuid::new_v4().to_string(),
            action,
        })
        .await
        .expect("real guest action");
    *snapshot = result.snapshot;
    *sequence += 1;
    snapshot
        .document
        .validate()
        .expect("unchanged public UI contract");
}
async fn add(
    client: &mut AlcomdClient,
    snapshot: &mut UiSnapshot,
    sequence: &mut u64,
    permission: &str,
    scope: &str,
) {
    dispatch(client, snapshot, sequence, grant_form(permission, scope)).await;
    dispatch(client, snapshot, sequence, activate("confirm")).await;
}
async fn refresh(client: &mut AlcomdClient, snapshot: &mut UiSnapshot) {
    tokio::time::sleep(Duration::from_millis(1050)).await;
    *snapshot = client
        .extension_ui_refresh(ExtensionUiRefreshParams {
            session_id: snapshot.session_id.clone(),
            expected_snapshot_revision: snapshot.snapshot_revision,
        })
        .await
        .expect("explicit refresh")
        .snapshot;
}
async fn current_client(client: &mut AlcomdClient, id: &str) -> alcomd_protocol::McpClientRecord {
    client
        .mcp_clients_list(McpClientsListParams::default())
        .await
        .expect("authoritative list")
        .items
        .into_iter()
        .find(|item| item.client_id == id)
        .expect("client")
}

/// Fault injection at the real UI RPC response boundary. No production transport/authentication
/// is added: these are the frozen native protocol codec and protected local-owner OS connection.
async fn accepted_ui_write_survives_lost_reply_and_connection_close(
    ipc: &IpcConfig,
    owner: &mut AlcomdClient,
    extension: &str,
) {
    let mut stream = native_ui_connection(ipc).await;
    send_ui_request(
        &mut stream,
        alcomd_protocol::METHOD_EXTENSIONS_UI_OPEN,
        serde_json::to_value(ExtensionUiOpenParams {
            extension_id: extension.into(),
            locale: "zh-CN".into(),
        })
        .expect("open params"),
    )
    .await;
    let opened = read_ui_reply(&mut stream).await;
    let opened: alcomd_protocol::ExtensionUiOpenResult =
        serde_json::from_value(opened["result"].clone()).expect("real open result");
    let params = ExtensionUiDispatchParams {
        session_id: opened.session.session_id,
        expected_snapshot_revision: opened.snapshot.snapshot_revision,
        sequence: 1,
        request_id: uuid::Uuid::new_v4().to_string(),
        action: UiAction::SubmitForm {
            action_id: "create".into(),
            values: vec![text("name", "Accepted before UI response loss")],
        },
    };
    let params = serde_json::to_value(params).expect("dispatch params");
    send_ui_request(
        &mut stream,
        alcomd_protocol::METHOD_EXTENSIONS_UI_DISPATCH,
        params.clone(),
    )
    .await;
    // Do not consume the actual success reply. Verify acceptance independently, then lose
    // that connection/reply; an accepted Core command must not be undone or reissued.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
    loop {
        let items = owner
            .mcp_clients_list(McpClientsListParams::default())
            .await
            .expect("Core registry")
            .items;
        if items
            .iter()
            .any(|item| item.display_name == "Accepted before UI response loss")
        {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "write was not accepted"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    drop(stream);
    let mut other = native_ui_connection(ipc).await;
    send_ui_request(
        &mut other,
        alcomd_protocol::METHOD_EXTENSIONS_UI_DISPATCH,
        params,
    )
    .await;
    let replay = read_ui_reply(&mut other).await;
    assert_eq!(replay["error"]["code"], "extension_ui_session_not_found");
    drop(other);
    let items = owner
        .mcp_clients_list(McpClientsListParams::default())
        .await
        .expect("durable accepted record")
        .items;
    let accepted: Vec<_> = items
        .iter()
        .filter(|item| item.display_name == "Accepted before UI response loss")
        .collect();
    assert_eq!(accepted.len(), 1);
    assert_eq!(accepted[0].revision, 1);
    assert!(accepted[0].grants.is_empty());
}
async fn native_ui_connection(ipc: &IpcConfig) -> alcomd_platform::IpcStream {
    let mut stream = alcomd_platform::connect(ipc)
        .await
        .expect("protected native test connection");
    let hello = alcomd_protocol::HelloParams {
        authentication: None,
        rpc_version: alcomd_protocol::RPC_VERSION,
        client: alcomd_protocol::ClientInfo {
            name: "management-response-loss-fixture".into(),
            version: "1.0.0".into(),
            instance_id: uuid::Uuid::new_v4().to_string(),
        },
        capabilities: vec![
            alcomd_protocol::CAPABILITY_EXTENSIONS_UI_PORTABLE_V1.into(),
            alcomd_protocol::CAPABILITY_MCP_CLIENTS_V1.into(),
        ],
    };
    send_ui_request(
        &mut stream,
        alcomd_protocol::METHOD_SYSTEM_HELLO,
        serde_json::to_value(hello).expect("hello params"),
    )
    .await;
    assert!(read_ui_reply(&mut stream).await.get("result").is_some());
    stream
}
async fn send_ui_request(
    stream: &mut alcomd_platform::IpcStream,
    method: &str,
    params: serde_json::Value,
) {
    use tokio::io::AsyncWriteExt;
    assert!(matches!(
        method,
        alcomd_protocol::METHOD_SYSTEM_HELLO
            | alcomd_protocol::METHOD_EXTENSIONS_UI_OPEN
            | alcomd_protocol::METHOD_EXTENSIONS_UI_DISPATCH
    ));
    let request = alcomd_protocol::RequestEnvelope {
        id: uuid::Uuid::new_v4().to_string(),
        method: method.into(),
        params,
    };
    request.validate().expect("frozen envelope");
    let payload = serde_json::to_vec(&request).expect("request");
    let frame = alcomd_protocol::encode_frame(&payload).expect("bounded frame");
    stream.write_all(&frame).await.expect("send real request");
}
async fn read_ui_reply(stream: &mut alcomd_platform::IpcStream) -> serde_json::Value {
    use tokio::io::AsyncReadExt;
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut prefix = [0; 4];
        stream.read_exact(&mut prefix).await.expect("frame prefix");
        let length = alcomd_protocol::decode_frame_length(prefix).expect("bounded frame");
        let mut bytes = vec![0; length];
        stream.read_exact(&mut bytes).await.expect("frame body");
        serde_json::from_slice(&bytes).expect("real reply")
    })
    .await
    .expect("bounded reply wait")
}
