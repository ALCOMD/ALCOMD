use super::*;
use alcomd_protocol::{
    McpClientMutationParams, McpClientsCreateParams, McpClientsListParams,
    McpTransportConnectionInput, McpTransportConnectionStage, McpTransportKind, McpTransportMethod,
    McpTransportReport, McpTransportRequestInput, McpTransportRequestStage,
    McpTransportStreamInput, UiFieldValue, UiSnapshot, UiSubmittedField,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn transport_host_reads_only_current_granted_groups_and_revokes_old_ui_lease() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    fs::create_dir(&runtime).expect("runtime");
    fs::create_dir(&data).expect("fresh data");
    let package = fixture.path().join("transport.alcomdextension");
    create_signed_package_with_interfaces(
        &package,
        include_bytes!("../../../../crates/alcomd-testing/fixtures/m8/mcp-transport-v1.wasm"),
        "Transport read fixture",
        "[permissions]\nrequired = []\noptional = [\"mcp.connections.read\",\"mcp.requests.read\",\"mcp.subscription-streams.read\"]\n[ui]\nprotocol = \"portable-v1\"\n",
        &["alcomd:mcp-transport/observations@1.0.0"],
        "AGPL-3.0-only",
    );
    let (ipc, config) = isolated_ipc(runtime);
    let shutdown = Arc::new(AtomicBool::new(false));
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc,
        DataConfig::isolated(data),
        wait_for_shutdown(Arc::clone(&shutdown)),
    )
    .await
    .expect("isolated Core");
    let mut owner = connect_with_retry(config).await;
    let plan = owner
        .extension_plan_install(ExtensionPlanInstallParams {
            source_kind: ExtensionSourceKind::LocalOwnerSelected,
            package_path: package.to_string_lossy().into_owned(),
            expected_revision: 0,
        })
        .await
        .expect("real signed plan")
        .plan;
    let installed = owner
        .extension_apply_install(alcomd_protocol::ExtensionInstallApplyParams {
            consent: alcomd_protocol::ExtensionInstallConsent {
                plan_fingerprint: plan.plan_fingerprint.clone(),
                installation_terms_accepted: false,
                required_permissions_accepted: false,
            },
            plan_id: plan.plan_id,
            idempotency_key: "transport-install".into(),
        })
        .await
        .expect("explicit install");
    assert_eq!(
        wait_for_terminal(&mut owner, &installed.operation_id).await,
        OperationState::Succeeded
    );
    let extension = owner
        .extension_get("dev.alcomd.fixture".into())
        .await
        .expect("installed")
        .extension;
    let permissions = owner
        .extension_permissions_list(alcomd_protocol::ExtensionPermissionsListParams {
            extension_id: extension.extension_id.clone(),
            cursor: None,
            limit: Some(64),
        })
        .await
        .expect("current catalog");
    assert_eq!(permissions.available_permissions.len(), 13);
    assert!(
        permissions
            .available_permissions
            .iter()
            .any(|p| p == "mcp.subscription-streams.read")
    );
    assert!(permissions.grants.is_empty());
    owner
        .extension_enable(ExtensionLifecycleParams {
            extension_id: extension.extension_id.clone(),
            expected_revision: extension.revision,
            idempotency_key: "transport-enable".into(),
        })
        .await
        .expect("optional permissions do not block startup");
    let opened = owner
        .extension_ui_open(ExtensionUiOpenParams {
            extension_id: extension.extension_id.clone(),
            locale: "en-US".into(),
        })
        .await
        .expect("real new-WIT Host render");
    assert!(
        serde_json::to_string(&opened.snapshot.document)
            .expect("document")
            .contains("transport_error=permission_denied")
    );
    let grant = owner
        .extension_set_grant(ExtensionGrantParams {
            extension_id: extension.extension_id.clone(),
            permission: "mcp.subscription-streams.read".into(),
            resource_kind: "Global".into(),
            resource_id: "*".into(),
            expected_grant_revision: permissions.grant_revision,
            idempotency_key: "transport-stream-read".into(),
        })
        .await
        .expect("exact independent stream permission");
    assert!(
        owner
            .extension_ui_refresh(ExtensionUiRefreshParams {
                session_id: opened.session.session_id,
                expected_snapshot_revision: opened.snapshot.snapshot_revision
            })
            .await
            .is_err(),
        "grant revision invalidates old UI authority"
    );
    let page = owner
        .extension_ui_open(ExtensionUiOpenParams {
            extension_id: extension.extension_id.clone(),
            locale: "en-US".into(),
        })
        .await
        .expect("new lease");
    assert!(
        serde_json::to_string(&page.snapshot.document)
            .expect("document")
            .contains("connections=false requests=false streams=true coverageIncomplete=true")
    );
    owner
        .extension_revoke_grant(ExtensionGrantParams {
            extension_id: extension.extension_id.clone(),
            permission: "mcp.subscription-streams.read".into(),
            resource_kind: "Global".into(),
            resource_id: "*".into(),
            expected_grant_revision: grant.grant_revision,
            idempotency_key: "transport-revoke-read".into(),
        })
        .await
        .expect("revoke");
    assert!(
        owner
            .extension_ui_refresh(ExtensionUiRefreshParams {
                session_id: page.session.session_id,
                expected_snapshot_revision: page.snapshot.snapshot_revision
            })
            .await
            .is_err(),
        "revoked lease cannot read"
    );
    let page = owner
        .extension_ui_open(ExtensionUiOpenParams {
            extension_id: extension.extension_id,
            locale: "en-US".into(),
        })
        .await
        .expect("revoked optional page stays usable");
    assert!(
        serde_json::to_string(&page.snapshot.document)
            .expect("document")
            .contains("transport_error=permission_denied")
    );
    drop(owner);
    shutdown.store(true, Ordering::Release);
    tokio::time::timeout(Duration::from_secs(5), daemon)
        .await
        .expect("stop deadline")
        .expect("Core join")
        .expect("Core stop");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn runtime_snapshot_rpc_tracks_authoritative_ids_disconnect_revocation_and_restart() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    fs::create_dir(&runtime).expect("runtime directory");
    fs::create_dir(&data).expect("isolated data directory");
    let (ipc, config) = isolated_ipc(runtime);
    let shutdown = Arc::new(AtomicBool::new(false));
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc.clone(),
        DataConfig::isolated(data.clone()),
        wait_for_shutdown(Arc::clone(&shutdown)),
    )
    .await
    .expect("isolated Core");
    let mut owner = connect_with_retry(config.clone()).await;
    let initial = owner
        .mcp_runtime_snapshot(Default::default())
        .await
        .expect("negotiated real capability");
    assert_eq!(
        initial
            .connections
            .expect("owner connections")
            .total_tracked,
        0
    );
    let mut connections = Vec::new();
    let mut ids = Vec::new();
    let mut revisions = Vec::new();
    for index in 0..2 {
        let record = owner
            .mcp_client_create(McpClientsCreateParams {
                display_name: format!("Observed client {index}"),
                idempotency_key: format!("observe-create-{index}"),
            })
            .await
            .expect("create client");
        let issued = owner
            .mcp_client_issue_credential(McpClientMutationParams {
                client_id: record.client_id.clone(),
                expected_revision: record.revision,
                idempotency_key: format!("observe-issue-{index}"),
            })
            .await
            .expect("one-time credential");
        ids.push(record.client_id);
        revisions.push(issued.revision);
        connections.push(
            AlcomdClient::connect(
                config
                    .clone()
                    .with_client_name(&ids[0])
                    .with_mcp_credential(issued.credential),
            )
            .await
            .expect("real credential identity"),
        );
    }
    let current = wait_runtime_connections(&mut owner, 2).await;
    let mut actual: Vec<_> = current
        .connections
        .expect("connections")
        .items
        .into_iter()
        .map(|row| row.client_id)
        .collect();
    actual.sort();
    let mut expected = ids.clone();
    expected.sort();
    assert_eq!(
        actual, expected,
        "clientInfo cannot supply connection identity"
    );
    assert_eq!(current.requests.expect("requests").total_tracked, 0);
    let filtered = owner
        .mcp_runtime_snapshot(alcomd_protocol::McpRuntimeSnapshotParams {
            client_id: Some(ids[0].clone()),
            limit: Some(1),
        })
        .await
        .expect("scoped filter");
    let rows = filtered.connections.expect("filtered connections");
    assert_eq!(rows.total_tracked, 1);
    assert_eq!(rows.items[0].client_id, ids[0]);
    assert!(!rows.truncated);
    let limited = owner
        .mcp_runtime_snapshot(alcomd_protocol::McpRuntimeSnapshotParams {
            client_id: None,
            limit: Some(1),
        })
        .await
        .expect("bounded group");
    let rows = limited.connections.expect("limited connections");
    assert_eq!(rows.total_tracked, 2);
    assert_eq!(rows.items.len(), 1);
    assert!(rows.truncated);
    assert!(
        matches!(owner.mcp_runtime_snapshot(alcomd_protocol::McpRuntimeSnapshotParams {client_id:None,limit:Some(0)}).await,Err(ClientError::Remote(error)) if error.code=="invalid_request")
    );
    assert!(
        matches!(connections[0].mcp_runtime_snapshot(Default::default()).await,Err(ClientError::Remote(error)) if error.code=="permission_denied"),
        "business identity cannot read administrative observations"
    );
    drop(connections.pop());
    wait_runtime_connections(&mut owner, 1).await;
    owner
        .mcp_client_revoke(McpClientMutationParams {
            client_id: ids[0].clone(),
            expected_revision: revisions[0],
            idempotency_key: "observe-revoke".into(),
        })
        .await
        .expect("authoritative revocation");
    assert_eq!(
        owner
            .mcp_runtime_snapshot(Default::default())
            .await
            .expect("physical presence is not authorization")
            .connections
            .expect("connection group")
            .total_tracked,
        1
    );
    assert!(
        connections[0].system_status().await.is_err(),
        "revoked connected identity cannot fall back to owner"
    );
    wait_runtime_connections(&mut owner, 0).await;
    drop(connections);
    drop(owner);
    shutdown.store(true, Ordering::Release);
    daemon.await.expect("daemon task").expect("shutdown");
    let restart = Arc::new(AtomicBool::new(false));
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc,
        DataConfig::isolated(data),
        wait_for_shutdown(Arc::clone(&restart)),
    )
    .await
    .expect("same isolated data restart");
    let mut owner = connect_with_retry(config).await;
    let fresh = owner
        .mcp_runtime_snapshot(Default::default())
        .await
        .expect("fresh observations");
    assert_ne!(initial.instance_id, fresh.instance_id);
    assert_eq!(
        fresh.connections.expect("fresh connections").total_tracked,
        0
    );
    assert_eq!(fresh.requests.expect("fresh calls").total_tracked, 0);
    let persisted = owner
        .mcp_clients_list(Default::default())
        .await
        .expect("durable client records");
    assert_eq!(persisted.items.len(), 2);
    assert!(
        persisted
            .items
            .iter()
            .find(|row| row.client_id == ids[0])
            .expect("revoked client persisted")
            .revoked_at_ms
            .is_some()
    );
    restart.store(true, Ordering::Release);
    daemon
        .await
        .expect("restarted task")
        .expect("restart shutdown");
}

async fn wait_runtime_connections(
    owner: &mut AlcomdClient,
    expected: u32,
) -> alcomd_protocol::McpRuntimeSnapshotResult {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        let snapshot = owner
            .mcp_runtime_snapshot(Default::default())
            .await
            .expect("real snapshot");
        if snapshot
            .connections
            .as_ref()
            .expect("owner connection group")
            .total_tracked
            == expected
        {
            return snapshot;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "physical connection cleanup did not converge"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn runtime_observations_use_real_authenticated_connections_and_revocable_host_grants() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    fs::create_dir(&runtime).expect("real runtime observation fixture");
    fs::create_dir(&data).expect("real runtime observation fixture");
    let package = fixture.path().join("runtime.alcomdextension");
    let declarations = include_str!(
        "../../../../extensions/first-party/alcomd-extension-mcp/alcomd-extension.toml"
    )
    .split_once("[permissions]")
    .expect("current management permission declarations")
    .1;
    let profile = format!("[permissions]{declarations}\n[ui]\nprotocol = \"portable-v1\"\n");
    create_signed_package_with_interfaces(
        &package,
        include_bytes!("../../../../crates/alcomd-testing/fixtures/m8/mcp-runtime-v1.wasm"),
        "Core activity fixture",
        &profile,
        &[
            "alcomd:mcp-management/clients@2.0.0",
            "alcomd:mcp-runtime/observations@1.0.0",
            "alcomd:mcp-transport/observations@1.0.0",
        ],
        "AGPL-3.0-only",
    );
    let (ipc, config) = isolated_ipc(runtime);
    let shutdown = Arc::new(AtomicBool::new(false));
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc,
        DataConfig::isolated(data),
        wait_for_shutdown(Arc::clone(&shutdown)),
    )
    .await
    .expect("real runtime observation fixture");
    let mut owner = connect_with_retry(config.clone()).await;
    let mut connections = Vec::new();
    let mut ids = Vec::new();
    for index in 0..2 {
        let record = owner
            .mcp_client_create(McpClientsCreateParams {
                display_name: format!("Isolated runtime {index}"),
                idempotency_key: format!("runtime-create-{index}"),
            })
            .await
            .expect("real runtime observation fixture");
        let issued = owner
            .mcp_client_issue_credential(McpClientMutationParams {
                client_id: record.client_id.clone(),
                expected_revision: record.revision,
                idempotency_key: format!("runtime-issue-{index}"),
            })
            .await
            .expect("real runtime observation fixture");
        ids.push(record.client_id);
        if index == 0 {
            let mut wrong = issued.credential.clone();
            let last = wrong.pop().expect("real runtime observation fixture");
            wrong.push(if last == '0' { '1' } else { '0' });
            assert!(
                AlcomdClient::connect(config.clone().with_mcp_credential(wrong))
                    .await
                    .is_err()
            );
        }
        connections.push(
            AlcomdClient::connect(
                config
                    .clone()
                    .with_client_name(&ids[0])
                    .with_mcp_credential(issued.credential),
            )
            .await
            .expect("real runtime observation fixture"),
        );
    }
    let plan = owner
        .extension_plan_install(ExtensionPlanInstallParams {
            source_kind: ExtensionSourceKind::LocalOwnerSelected,
            package_path: package.to_string_lossy().into_owned(),
            expected_revision: 0,
        })
        .await
        .expect("real runtime observation fixture")
        .plan;
    assert!(plan.required_permissions.is_empty());
    assert_eq!(
        plan.optional_permissions,
        [
            "mcp.clients.manage",
            "mcp.clients.read",
            "mcp.connections.read",
            "mcp.requests.read",
            "mcp.subscription-streams.read",
        ]
    );
    let install = owner
        .extension_apply_install(alcomd_protocol::ExtensionInstallApplyParams {
            consent: alcomd_protocol::ExtensionInstallConsent {
                plan_fingerprint: plan.plan_fingerprint.clone(),
                installation_terms_accepted: plan.installation_terms.is_some(),
                required_permissions_accepted: !plan.required_permissions.is_empty(),
            },
            plan_id: plan.plan_id,
            idempotency_key: "runtime-install".into(),
        })
        .await
        .expect("real runtime observation fixture");
    assert_eq!(
        wait_for_terminal(&mut owner, &install.operation_id).await,
        OperationState::Succeeded
    );
    let extension = owner
        .extension_get("dev.alcomd.fixture".into())
        .await
        .expect("real runtime observation fixture")
        .extension;
    owner
        .extension_enable(ExtensionLifecycleParams {
            extension_id: extension.extension_id.clone(),
            expected_revision: extension.revision,
            idempotency_key: "runtime-enable".into(),
        })
        .await
        .expect("real runtime observation fixture");
    let page = owner
        .extension_ui_open(ExtensionUiOpenParams {
            extension_id: extension.extension_id.clone(),
            locale: "zh-CN".into(),
        })
        .await
        .expect("real runtime observation fixture");
    let document =
        serde_json::to_string(&page.snapshot.document).expect("real runtime observation fixture");
    assert!(!document.contains("\"actionId\":\"activity\""));
    assert!(!document.contains(&ids[0]));
    assert!(!document.contains("Core 连接 ·"));
    let mut revision = owner
        .extension_permissions_list(alcomd_protocol::ExtensionPermissionsListParams {
            extension_id: extension.extension_id.clone(),
            cursor: None,
            limit: Some(64),
        })
        .await
        .expect("real runtime observation fixture")
        .grant_revision;
    for permission in ["mcp.connections.read", "mcp.requests.read"] {
        if permission == "mcp.requests.read" {
            revision = owner
                .extension_set_grant(ExtensionGrantParams {
                    extension_id: extension.extension_id.clone(),
                    permission: permission.into(),
                    resource_kind: "Global".into(),
                    resource_id: "*".into(),
                    expected_grant_revision: revision,
                    idempotency_key: "runtime-grant-requests".into(),
                })
                .await
                .expect("real runtime observation fixture")
                .grant_revision;
            let mut page = owner
                .extension_ui_open(ExtensionUiOpenParams {
                    extension_id: extension.extension_id.clone(),
                    locale: "zh-CN".into(),
                })
                .await
                .expect("real runtime observation fixture")
                .snapshot;
            // A real authenticated call stays in flight through Core. No mock RPC response or public testing bypass.
            let gate = daemon.hold_next_runtime_project_list().await;
            let mut mcp = connections.pop().expect("real runtime observation fixture");
            let call = tokio::spawn(async move {
                let result = mcp.projects_list(None, Some(10)).await;
                (mcp, result)
            });
            tokio::time::timeout(Duration::from_secs(10), gate.entered())
                .await
                .expect("real runtime observation fixture");
            let mut sequence = 0;
            dispatch(&mut owner, &mut page, &mut sequence, activate("activity")).await;
            let document =
                serde_json::to_string(&page.document).expect("real runtime observation fixture");
            assert!(document.contains("Core 连接 · 2"), "{document}");
            assert!(document.contains("Core 调用 · 1"), "{document}");
            assert!(document.contains("读取项目列表"));
            assert!(document.contains(&ids[1][..8]));
            assert!(!document.contains("credential"));
            assert!(!document.contains("idempotency"));
            gate.release();
            let (mcp, result) = call.await.expect("real runtime observation fixture");
            assert!(result.is_ok(), "connected identity can read the projects");
            connections.push(mcp);
            let narrowed = |permissions: Vec<alcomd_application::Permission>| {
                alcomd_application::AccessContext::new(
                    alcomd_application::PrincipalId::local_owner(),
                    permissions,
                )
                .with_extension_ui_scopes([extension.extension_id.clone()])
            };
            let denied = daemon
                .extension_activity_with_access(
                    narrowed(vec![
                        alcomd_application::Permission::ExtensionsRead,
                        alcomd_application::Permission::ExtensionsUiUse,
                    ]),
                    extension.extension_id.clone(),
                )
                .await
                .expect("real runtime observation fixture");
            let denied = String::from_utf8(denied).expect("real runtime observation fixture");
            assert!(!denied.contains("Core 连接 ·"));
            assert!(!denied.contains("Core 调用 ·"));
            let partial = daemon
                .extension_activity_with_access(
                    narrowed(vec![
                        alcomd_application::Permission::ExtensionsRead,
                        alcomd_application::Permission::ExtensionsUiUse,
                        alcomd_application::Permission::McpConnectionsRead,
                    ]),
                    extension.extension_id.clone(),
                )
                .await
                .expect("real runtime observation fixture");
            let partial = String::from_utf8(partial).expect("real runtime observation fixture");
            assert!(partial.contains("Core 连接 · 2"), "{partial}");
            assert!(!partial.contains("Core 调用 ·"));
            dispatch(
                &mut owner,
                &mut page,
                &mut sequence,
                activate("runtime-refresh"),
            )
            .await;
            assert!(
                serde_json::to_string(&page.document)
                    .expect("real runtime observation fixture")
                    .contains("Core 调用 · 0")
            );
            revision = owner
                .extension_revoke_grant(ExtensionGrantParams {
                    extension_id: extension.extension_id.clone(),
                    permission: "mcp.connections.read".into(),
                    resource_kind: "Global".into(),
                    resource_id: "*".into(),
                    expected_grant_revision: revision,
                    idempotency_key: "runtime-revoke-connections".into(),
                })
                .await
                .expect("real runtime observation fixture")
                .grant_revision;
            assert!(
                owner
                    .extension_ui_refresh(ExtensionUiRefreshParams {
                        session_id: page.session_id,
                        expected_snapshot_revision: page.snapshot_revision
                    })
                    .await
                    .is_err(),
                "old lease must not survive revocation"
            );
            let mut page = owner
                .extension_ui_open(ExtensionUiOpenParams {
                    extension_id: extension.extension_id.clone(),
                    locale: "zh-CN".into(),
                })
                .await
                .expect("real runtime observation fixture")
                .snapshot;
            let mut sequence = 0;
            dispatch(&mut owner, &mut page, &mut sequence, activate("activity")).await;
            let document =
                serde_json::to_string(&page.document).expect("real runtime observation fixture");
            assert!(!document.contains("Core 连接 ·"));
            assert!(document.contains("Core 调用 · 0"));
        } else {
            revision = owner
                .extension_set_grant(ExtensionGrantParams {
                    extension_id: extension.extension_id.clone(),
                    permission: permission.into(),
                    resource_kind: "Global".into(),
                    resource_id: "*".into(),
                    expected_grant_revision: revision,
                    idempotency_key: "runtime-grant-connections".into(),
                })
                .await
                .expect("real runtime observation fixture")
                .grant_revision;
            let mut page = owner
                .extension_ui_open(ExtensionUiOpenParams {
                    extension_id: extension.extension_id.clone(),
                    locale: "zh-CN".into(),
                })
                .await
                .expect("real runtime observation fixture")
                .snapshot;
            let mut sequence = 0;
            dispatch(&mut owner, &mut page, &mut sequence, activate("activity")).await;
            let document =
                serde_json::to_string(&page.document).expect("real runtime observation fixture");
            assert!(document.contains("Core 连接 · 2"), "{document}");
            assert!(!document.contains("Core 调用 ·"));
        }
    }
    let revision = owner
        .extension_revoke_grant(ExtensionGrantParams {
            extension_id: extension.extension_id.clone(),
            permission: "mcp.requests.read".into(),
            resource_kind: "Global".into(),
            resource_id: "*".into(),
            expected_grant_revision: revision,
            idempotency_key: "runtime-revoke-requests".into(),
        })
        .await
        .expect("real runtime observation fixture")
        .grant_revision;
    let page = owner
        .extension_ui_open(ExtensionUiOpenParams {
            extension_id: extension.extension_id.clone(),
            locale: "zh-CN".into(),
        })
        .await
        .expect("real runtime observation fixture");
    assert!(
        !serde_json::to_string(&page.snapshot.document)
            .expect("real runtime observation fixture")
            .contains("Core 调用 ·")
    );
    assert!(
        !serde_json::to_string(&page.snapshot.document)
            .expect("document")
            .contains("\"actionId\":\"activity\"")
    );
    owner
        .extension_ui_close(ExtensionUiCloseParams {
            session_id: page.session.session_id,
        })
        .await
        .expect("close the zero-authorization page");
    let granted = owner
        .extension_set_grant(ExtensionGrantParams {
            extension_id: extension.extension_id.clone(),
            permission: "mcp.subscription-streams.read".into(),
            resource_kind: "Global".into(),
            resource_id: "*".into(),
            expected_grant_revision: revision,
            idempotency_key: "runtime-grant-streams".into(),
        })
        .await
        .expect("only subscription read granted");
    let mut page = owner
        .extension_ui_open(ExtensionUiOpenParams {
            extension_id: extension.extension_id.clone(),
            locale: "zh-CN".into(),
        })
        .await
        .expect("new lease for the first-party guest")
        .snapshot;
    let mut sequence = 0;
    dispatch(&mut owner, &mut page, &mut sequence, activate("activity")).await;
    let unavailable = serde_json::to_string(&page.document).expect("document");
    assert!(unavailable.contains("当前没有可用的 MCP 订阅流报告。"));
    assert!(!unavailable.contains("MCP 订阅流（适配器上报） · 0"));

    let mut reporter = AlcomdClient::connect(config.with_transport_observer())
        .await
        .expect("restricted reporter");
    let connection = uuid::Uuid::new_v4().to_string();
    let request = uuid::Uuid::new_v4().to_string();
    reporter
        .mcp_transport_report(McpTransportReport {
            sequence: 1,
            transport: McpTransportKind::Http,
            connections: vec![McpTransportConnectionInput {
                local_ref: connection.clone(),
                stage: McpTransportConnectionStage::Active,
            }],
            requests: vec![McpTransportRequestInput {
                local_ref: request.clone(),
                connection_ref: connection,
                method: McpTransportMethod::Listen,
                stage: McpTransportRequestStage::Streaming,
            }],
            subscription_streams: vec![McpTransportStreamInput {
                local_ref: uuid::Uuid::new_v4().to_string(),
                request_ref: request,
                tools_list_changed: true,
            }],
            coverage_incomplete: false,
        })
        .await
        .expect("real Core accepts the fixture observer report");
    dispatch(
        &mut owner,
        &mut page,
        &mut sequence,
        activate("runtime-refresh"),
    )
    .await;
    let observed = serde_json::to_string(&page.document).expect("document");
    assert!(
        observed.contains("MCP 订阅流（适配器上报） · 1"),
        "{observed}"
    );
    assert!(observed.contains("未关联客户端 · 工具列表变更"));
    assert!(!observed.contains("Core 调用 ·"));
    assert!(!observed.contains("Core 连接 ·"));
    owner
        .extension_revoke_grant(ExtensionGrantParams {
            extension_id: extension.extension_id.clone(),
            permission: "mcp.subscription-streams.read".into(),
            resource_kind: "Global".into(),
            resource_id: "*".into(),
            expected_grant_revision: granted.grant_revision,
            idempotency_key: "runtime-revoke-streams".into(),
        })
        .await
        .expect("revoke stream read");
    assert!(
        owner
            .extension_ui_refresh(ExtensionUiRefreshParams {
                session_id: page.session_id,
                expected_snapshot_revision: page.snapshot_revision,
            })
            .await
            .is_err(),
        "revoked lease cannot refresh subscription streams"
    );
    drop(reporter);
    drop(connections);
    export_package(&package, "ALCOMD_TEST_MCP_RUNTIME_PACKAGE_OUTPUT");
    shutdown.store(true, Ordering::Release);
    daemon
        .await
        .expect("real runtime observation fixture")
        .expect("real runtime observation fixture");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn optional_management_permissions_allow_zero_grant_start_and_restrict_data() {
    let _serial = RPC_TEST_LOCK.lock().await;
    let fixture = TestDirectory::new();
    let runtime = fixture.path().join("runtime");
    let data = fixture.path().join("data");
    fs::create_dir(&runtime).expect("runtime");
    fs::create_dir(&data).expect("data");
    let package = fixture.path().join("optional-management.alcomdextension");
    let permissions = include_str!(
        "../../../../extensions/first-party/alcomd-extension-mcp/alcomd-extension.toml"
    )
    .split_once("[permissions]")
    .expect("actual management permission declarations")
    .1;
    let profile = format!("[permissions]{permissions}\n[ui]\nprotocol = \"portable-v1\"\n");
    create_signed_package_with_interfaces(
        &package,
        include_bytes!("../../../../crates/alcomd-testing/fixtures/m8/mcp-runtime-v1.wasm"),
        "MCP management fixture",
        &profile,
        &[
            "alcomd:mcp-management/clients@2.0.0",
            "alcomd:mcp-runtime/observations@1.0.0",
            "alcomd:mcp-transport/observations@1.0.0",
        ],
        "AGPL-3.0-only",
    );
    let (ipc, config) = isolated_ipc(runtime);
    let shutdown = Arc::new(AtomicBool::new(false));
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc,
        DataConfig::isolated(data),
        wait_for_shutdown(Arc::clone(&shutdown)),
    )
    .await
    .expect("isolated daemon");
    let mut client = connect_with_retry(config).await;
    client
        .mcp_client_create(McpClientsCreateParams {
            display_name: "授权后才可见的客户端".into(),
            idempotency_key: "optional-management-seed".into(),
        })
        .await
        .expect("owner seed");
    let plan = client
        .extension_plan_install(ExtensionPlanInstallParams {
            source_kind: ExtensionSourceKind::LocalOwnerSelected,
            package_path: package.to_string_lossy().into_owned(),
            expected_revision: 0,
        })
        .await
        .expect("explicit fixture publisher approval")
        .plan;
    assert!(plan.required_permissions.is_empty());
    assert!(
        plan.optional_permissions
            .iter()
            .any(|p| p == "mcp.clients.read")
    );
    let install = client
        .extension_apply_install(alcomd_protocol::ExtensionInstallApplyParams {
            consent: alcomd_protocol::ExtensionInstallConsent {
                plan_fingerprint: plan.plan_fingerprint.clone(),
                installation_terms_accepted: plan.installation_terms.is_some(),
                required_permissions_accepted: !plan.required_permissions.is_empty(),
            },
            plan_id: plan.plan_id,
            idempotency_key: "optional-management-install".into(),
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
        .expect("installed")
        .extension;
    let enabled = client
        .extension_enable(ExtensionLifecycleParams {
            extension_id: extension.extension_id.clone(),
            expected_revision: extension.revision,
            idempotency_key: "optional-management-zero-grants".into(),
        })
        .await
        .expect("zero grants may enable an optional-permission extension")
        .extension;
    assert_eq!(enabled.desired_state, ExtensionDesiredState::Enabled);
    let no_grants = client
        .extension_permissions_list(alcomd_protocol::ExtensionPermissionsListParams {
            extension_id: extension.extension_id.clone(),
            cursor: None,
            limit: Some(64),
        })
        .await
        .expect("actual grants");
    assert!(no_grants.grants.is_empty(), "enable never grants authority");
    let opened = client
        .extension_ui_open(ExtensionUiOpenParams {
            extension_id: extension.extension_id.clone(),
            locale: "zh-CN".into(),
        })
        .await
        .expect("zero-authority real Host page");
    let restricted = serde_json::to_string(&opened.snapshot.document).expect("document");
    assert!(restricted.contains("尚未授权读取 MCP 客户端"));
    assert!(restricted.contains("neutral"));
    assert!(!restricted.contains("授权后才可见的客户端"));
    assert!(!restricted.contains("创建客户端"));
    let forged = client
        .extension_ui_dispatch(ExtensionUiDispatchParams {
            session_id: opened.session.session_id,
            expected_snapshot_revision: opened.snapshot.snapshot_revision,
            sequence: 1,
            request_id: "optional-management-forged-create".into(),
            action: activate("create"),
        })
        .await;
    assert!(
        matches!(forged, Err(ClientError::Remote(error)) if error.code == "extension_ui_action_invalid")
    );
    assert_eq!(
        client
            .mcp_clients_list(McpClientsListParams::default())
            .await
            .expect("authority after refused action")
            .items
            .len(),
        1
    );
    let read = client
        .extension_set_grant(ExtensionGrantParams {
            extension_id: extension.extension_id.clone(),
            permission: "mcp.clients.read".into(),
            resource_kind: "Global".into(),
            resource_id: "*".into(),
            expected_grant_revision: no_grants.grant_revision,
            idempotency_key: "optional-management-read".into(),
        })
        .await
        .expect("explicit read grant");
    let readable = client
        .extension_ui_open(ExtensionUiOpenParams {
            extension_id: extension.extension_id.clone(),
            locale: "zh-CN".into(),
        })
        .await
        .expect("page after explicit read");
    let document = serde_json::to_string(&readable.snapshot.document).expect("document");
    assert!(document.contains("授权后才可见的客户端"));
    assert!(!document.contains("创建客户端"));
    client
        .extension_revoke_grant(ExtensionGrantParams {
            extension_id: extension.extension_id.clone(),
            permission: "mcp.clients.read".into(),
            resource_kind: "Global".into(),
            resource_id: "*".into(),
            expected_grant_revision: read.grant_revision,
            idempotency_key: "optional-management-revoke".into(),
        })
        .await
        .expect("revoke read");
    assert!(
        client
            .extension_ui_refresh(ExtensionUiRefreshParams {
                session_id: readable.session.session_id,
                expected_snapshot_revision: readable.snapshot.snapshot_revision,
            })
            .await
            .is_err()
    );
    let restricted_again = client
        .extension_ui_open(ExtensionUiOpenParams {
            extension_id: extension.extension_id.clone(),
            locale: "zh-CN".into(),
        })
        .await
        .expect("optional extension still opens after revocation");
    let document = serde_json::to_string(&restricted_again.snapshot.document).expect("document");
    assert!(document.contains("尚未授权读取 MCP 客户端"));
    assert!(!document.contains("授权后才可见的客户端"));
    assert!(!document.contains("创建客户端"));
    assert_eq!(
        client
            .extension_get(extension.extension_id.clone())
            .await
            .expect("current authority")
            .extension
            .desired_state,
        ExtensionDesiredState::Enabled
    );
    client
        .extension_ui_close(ExtensionUiCloseParams {
            session_id: restricted_again.session.session_id,
        })
        .await
        .expect("close the zero-authority page");
    wait_for_runtime_state(
        &mut client,
        &extension.extension_id,
        ExtensionRuntimeState::Stopped,
    )
    .await;
    export_development_package(&package);
    shutdown.store(true, Ordering::Release);
    daemon.await.expect("daemon task").expect("clean shutdown");
}

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
        include_bytes!("../../../../crates/alcomd-testing/fixtures/m8/mcp-runtime-v1.wasm"),
        "MCP management fixture",
        "[permissions]\nrequired = [\"mcp.clients.read\"]\noptional = [\"mcp.clients.manage\",\"projects.read\"]\n\n[ui]\nprotocol = \"portable-v1\"\n",
        &[
            "alcomd:mcp-management/clients@2.0.0",
            "alcomd:mcp-runtime/observations@1.0.0",
            "alcomd:mcp-transport/observations@1.0.0",
        ],
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
        })
        .await
        .expect("real record");
    let params = ExtensionPlanInstallParams {
        source_kind: ExtensionSourceKind::LocalOwnerSelected,
        package_path: package.to_string_lossy().into_owned(),
        expected_revision: 0,
    };
    let plan = client
        .extension_plan_install(params)
        .await
        .expect("owner-selected package needs no publisher confirmation")
        .plan;
    assert_eq!(plan.required_permissions, vec!["mcp.clients.read"]);
    let install = client
        .extension_apply_install(alcomd_protocol::ExtensionInstallApplyParams {
            consent: alcomd_protocol::ExtensionInstallConsent {
                plan_fingerprint: plan.plan_fingerprint.clone(),
                installation_terms_accepted: plan.installation_terms.is_some(),
                required_permissions_accepted: !plan.required_permissions.is_empty(),
            },
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
    let permissions = client
        .extension_permissions_list(alcomd_protocol::ExtensionPermissionsListParams {
            extension_id: extension.extension_id.clone(),
            cursor: None,
            limit: Some(64),
        })
        .await
        .expect("installed required consent is authoritative");
    assert_eq!(permissions.grant_revision, extension.grant_revision);
    assert!(
        permissions
            .grants
            .iter()
            .any(|grant| grant.permission == "mcp.clients.read"
                && grant.resource_kind == "Global"
                && grant.resource_id == "*")
    );
    client
        .extension_revoke_grant(ExtensionGrantParams {
            extension_id: extension.extension_id.clone(),
            permission: "mcp.clients.read".into(),
            resource_kind: "Global".into(),
            resource_id: "*".into(),
            expected_grant_revision: permissions.grant_revision,
            idempotency_key: "management-remove-installed-read".into(),
        })
        .await
        .expect("explicitly remove the required permission before testing refusal");
    let extension = client
        .extension_get(extension.extension_id)
        .await
        .expect("authority after required permission revocation")
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
    assert!(document.contains("查看客户端"));
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
            .expect("explicit extension management grants");
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
    assert!(mcp.project_get(project_id.clone()).await.is_ok());
    refresh(&mut client, &mut snapshot).await;
    let document = serde_json::to_string(&snapshot.document).expect("scope document");
    assert!(!document.contains("AllowedProject"));
    assert!(!document.contains(&project_id));
    assert!(!document.contains(&outside_id));
    assert!(!document.contains("\"kind\":\"select\""));
    let bad = client
        .extension_ui_dispatch(ExtensionUiDispatchParams {
            session_id: snapshot.session_id.clone(),
            expected_snapshot_revision: snapshot.snapshot_revision,
            sequence: sequence + 1,
            request_id: uuid::Uuid::new_v4().to_string(),
            action: UiAction::SubmitForm {
                action_id: "permissions".into(),
                values: vec![text("project", &outside_id)],
            },
        })
        .await;
    assert!(
        matches!(bad,Err(ClientError::Remote(error)) if error.code=="extension_ui_action_invalid")
    );

    assert!(mcp.project_get(outside_id).await.is_ok());
    dispatch(
        &mut client,
        &mut snapshot,
        &mut sequence,
        activate("revoke"),
    )
    .await;
    dispatch(
        &mut client,
        &mut snapshot,
        &mut sequence,
        activate("cancel"),
    )
    .await;
    assert_eq!(
        current_client(&mut client, &record.client_id)
            .await
            .revision,
        issued.revision
    );
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
    export_development_package(&package);
    shutdown.store(true, Ordering::Release);
    assert!(daemon.await.expect("join isolated daemon").is_ok());
}

fn export_development_package(package: &Path) {
    export_package(package, "ALCOMD_TEST_MCP_MANAGEMENT_PACKAGE_OUTPUT");
}
fn export_package(package: &Path, output_environment: &str) {
    if let Some(destination) = std::env::var_os(output_environment) {
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
            .write_all(&fs::read(package).expect("verified test package"))
            .expect("persist development package");
    }
}

fn text(id: &str, value: &str) -> UiSubmittedField {
    UiSubmittedField {
        field_id: id.into(),
        value: UiFieldValue::Text {
            value: value.into(),
        },
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
            alcomd_protocol::CAPABILITY_MCP_CLIENTS_V2.into(),
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
