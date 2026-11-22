//! Real signed Provider -> frozen SDK -> STDIO/HTTP -> real Core transactions.
use super::*;
#[path = "../../../../crates/alcomd-testing/fixtures/m8/signed_entry_package.rs"]
mod signed_entries;
pub(super) use signed_entries::package_with_install_requirements;
use signed_entries::{ID, declarations};
fn public(local: &str) -> String {
    alcomd_application::tool_public_name(ID, local)
}
pub(super) async fn ext(client: &Sdk, name: &str, args: Value) -> CallToolResult {
    match tokio::time::timeout(
        Duration::from_secs(30),
        client.call_tool_once(
            CallToolRequestParams::new(public(name)).with_arguments(
                args.as_object()
                    .expect("valid isolated Provider fixture")
                    .clone(),
            ),
        ),
    )
    .await
    .expect("provider deadline")
    .unwrap_or_else(|e| panic!("Provider {name} protocol failure: {e}"))
    {
        CallToolResponse::Complete(result) => result,
        _ => panic!("no session/tasks/MRTR"),
    }
}
async fn wait(owner: &mut AlcomdClient, id: &str) {
    for _ in 0..500 {
        let op = owner
            .operation_get(id.into())
            .await
            .expect("valid isolated Provider fixture");
        match op.state {
            OperationState::Succeeded => return,
            OperationState::Failed | OperationState::Cancelled => {
                panic!("real Operation failed: {:?}", op.error_code)
            }
            _ => tokio::time::sleep(Duration::from_millis(20)).await,
        }
    }
    panic!("operation deadline")
}
async fn client(owner: &mut AlcomdClient, name: &str) -> (McpClientRecord, String) {
    let mut record = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: name.into(),
            idempotency_key: format!("create-{name}"),
        })
        .await
        .expect("valid isolated Provider fixture");
    let token = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: record.client_id.clone(),
            expected_revision: record.revision,
            idempotency_key: format!("issue-{name}"),
        })
        .await
        .expect("valid isolated Provider fixture");
    record.revision = token.revision;
    (record, token.credential)
}
async fn permission(owner: &mut AlcomdClient, name: &str, revoke: bool, key: &str) {
    let revision = owner
        .extension_get(ID.into())
        .await
        .expect("valid isolated Provider fixture")
        .extension
        .grant_revision;
    let params = ExtensionGrantParams {
        extension_id: ID.into(),
        permission: name.into(),
        resource_kind: "Global".into(),
        resource_id: "*".into(),
        expected_grant_revision: revision,
        idempotency_key: key.into(),
    };
    if revoke {
        owner
            .extension_revoke_grant(params)
            .await
            .expect("valid isolated Provider fixture");
    } else {
        owner
            .extension_set_grant(params)
            .await
            .expect("valid isolated Provider fixture");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn frozen_sdk_extension_tools_real_signed_provider_and_core_effects() {
    let _serial = REAL_CORE_LOCK.lock().await;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-extension-provider-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).expect("valid isolated Provider fixture");
    let root = root
        .canonicalize()
        .expect("valid isolated Provider fixture");
    let data_root = root.join("data");
    std::fs::create_dir(&data_root).expect("valid isolated Provider fixture");
    let core = CoreConfiguration::new();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        core.ipc.clone(),
        DataConfig::isolated(data_root.clone()),
        std::future::pending(),
    )
    .await
    .expect("exclusive isolated Core");
    let mut owner = AlcomdClient::connect(core.client.clone())
        .await
        .expect("valid isolated Provider fixture");
    let path = root.join("provider.alcomdextension");
    package_with_install_requirements(&path, false);
    let request = ExtensionPlanInstallParams {
        source_kind: ExtensionSourceKind::LocalOwnerSelected,
        package_path: path.to_string_lossy().into_owned(),
        expected_revision: 0,
    };
    let plan = owner
        .extension_plan_install(request)
        .await
        .expect("normal signed plan")
        .plan;
    assert_eq!(plan.mcp_tools.len(), 13);
    assert_eq!(plan.cli_commands.len(), 13);
    assert!(plan.required_permissions.is_empty());
    let install = owner
        .extension_apply_install(alcomd_protocol::ExtensionInstallApplyParams {
            consent: alcomd_protocol::ExtensionInstallConsent {
                plan_fingerprint: plan.plan_fingerprint.clone(),
                installation_terms_accepted: plan.installation_terms.is_some(),
                required_permissions_accepted: !plan.required_permissions.is_empty(),
            },
            plan_id: plan.plan_id,
            idempotency_key: "install-provider".into(),
        })
        .await
        .expect("valid isolated Provider fixture");
    wait(&mut owner, &install.operation_id).await;
    let record = owner
        .extension_get(ID.into())
        .await
        .expect("valid isolated Provider fixture")
        .extension;
    let (a, secret_a) = client(&mut owner, "Provider A").await;
    let (b, secret_b) = client(&mut owner, "Provider B").await;
    let sa = sdk(&secret_a, None, &core).await;
    let sb = sdk(&secret_b, None, &core).await;
    let mut management_trace = Vec::new();
    let shared = call(
        &sa,
        &mut management_trace,
        "get_extension",
        json!({"extension_id": ID}),
    )
    .await
    .expect("authenticated operator can inspect the shared installation")
    .structured_content
    .expect("complete reply");
    assert_eq!(shared["data"]["extension"]["extension_id"], ID);
    assert_eq!(shared["data"]["extension"]["revision"], record.revision);
    let account_b = core.client.clone().with_mcp_credential(secret_b.clone());
    let mut operator_b = AlcomdClient::connect(account_b)
        .await
        .expect("authenticated second operator");
    let removal = operator_b
        .extension_plan_uninstall(alcomd_protocol::ExtensionPlanUninstallParams {
            extension_id: ID.into(),
            expected_revision: record.revision,
            data_disposition: alcomd_protocol::ExtensionDataDisposition::RetainData,
        })
        .await
        .expect("shared installation removable by second operator")
        .plan;
    let id = &removal.plan_id;
    let lookup = alcomd_protocol::PlansGetParams {
        kind: alcomd_protocol::PlanKind::ExtensionUninstall,
        plan_id: id.clone(),
    };
    assert!(
        owner.get_plan(lookup.clone()).await.is_err(),
        "plan belongs to its actual caller"
    );
    assert!(operator_b.get_plan(lookup).await.is_ok());
    let initial = sa.list_all_tools().await.expect("built-in directory");
    assert!(initial.iter().any(|tool| tool.name == "alcomd_get_project"));
    assert!(initial.iter().all(|tool| tool.name != public("echo")));
    let mut subscription = sa
        .listen(
            rmcp::model::SubscriptionFilter::builder()
                .tools_list_changed()
                .build(),
        )
        .await
        .expect("catalogue subscription before publication");
    let enabled = operator_b
        .extension_enable(ExtensionLifecycleParams {
            extension_id: ID.into(),
            expected_revision: record.revision,
            idempotency_key: "enable-no-background".into(),
        })
        .await
        .expect("valid isolated Provider fixture")
        .extension;
    let stale = operator_b
        .extension_apply_uninstall(alcomd_protocol::ExtensionApplyParams {
            plan_id: removal.plan_id,
            idempotency_key: "stale-shared-removal".into(),
        })
        .await;
    assert!(
        matches!(stale, Err(alcomd_client::ClientError::Remote(error)) if error.code=="extension_plan_stale" || error.code=="revision_conflict")
    );
    assert_eq!(enabled.desired_state, ExtensionDesiredState::Enabled);
    assert_eq!(enabled.runtime_state, ExtensionRuntimeState::Stopped);

    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(4), subscription.next())
            .await
            .expect("provider publication notice")
            .expect("SSE event"),
        Some(rmcp::model::ServerNotification::ToolListChangedNotification(_))
    ));
    subscription
        .cancel()
        .await
        .expect("close only the subscription");
    let visible = sa
        .list_all_tools()
        .await
        .expect("valid isolated Provider fixture");
    assert!(visible.iter().any(|t| t.name == public("echo")));
    assert!(visible.iter().any(|t| t.name == public("projects")));
    let unauthorized = ext(&sa, "projects", json!({})).await;
    assert_eq!(unauthorized.is_error, Some(true));
    let cli_commands = owner
        .extension_cli_commands_list(alcomd_protocol::ExtensionCliCommandsListParams {
            extension_id: Some(ID.into()),
            ..Default::default()
        })
        .await
        .expect("discover CLI commands without business grants");
    assert!(
        cli_commands
            .commands
            .iter()
            .any(|entry| entry.name == "projects")
    );
    let call_cli =
        |name: &str, arguments: Value| alcomd_protocol::ExtensionCliCommandsInvokeParams {
            extension_id: ID.into(),
            name: name.into(),
            arguments,
        };
    let command = owner
        .extension_cli_commands_invoke(call_cli("echo", json!({"text":"actual Wasm"})))
        .await
        .expect("CLI-only guest export");
    assert_eq!(command.data, json!({"cli":"actual Wasm"}));
    let rejected = owner
        .extension_cli_commands_invoke(call_cli("projects", json!({})))
        .await;
    assert!(
        matches!(&rejected, Err(alcomd_client::ClientError::Remote(e)) if e.code=="permission_denied")
            || matches!(rejected,Ok(value) if value.error.as_deref()==Some("permission_denied"))
    );
    let echo = ext(&sa, "echo", json!({"text":"actual Wasm"})).await;
    assert_ne!(echo.is_error, Some(true));
    let echo = echo
        .structured_content
        .expect("valid isolated Provider fixture");
    assert_eq!(echo["data"]["text"], "actual Wasm");
    assert!(echo["coreEffect"].is_null());
    let invalid = ext(&sa, "echo", json!({"text":42})).await;
    assert_eq!(invalid.is_error, Some(true));
    for name in [
        "projects.read",
        "repositories.read",
        "packages.read",
        "packages.manage",
        "operations.read",
        "operations.cancel",
    ] {
        permission(&mut owner, name, false, &format!("grant-{name}")).await;
    }
    assert!(
        owner
            .extension_cli_commands_invoke(call_cli("projects", json!({})))
            .await
            .expect("authorized CLI command")
            .error
            .is_none()
    );
    let all = sa
        .list_tools(None)
        .await
        .expect("valid isolated Provider fixture");
    assert_eq!(all.tools.len(), 16);
    let next = all
        .next_cursor
        .clone()
        .expect("combined directory paginated");
    let remainder = sa
        .list_tools(Some(
            rmcp::model::PaginatedRequestParams::default().with_cursor(Some(next.clone())),
        ))
        .await
        .expect("valid isolated Provider fixture");
    let published = sa.list_all_tools().await.expect("complete catalogue");
    assert_eq!(
        published.len(),
        initial.len()
            + declarations()["tools"]
                .as_array()
                .expect("declared tools")
                .len()
    );
    assert!(!remainder.tools.is_empty());
    for name in [
        "alcomd_list_package_sources",
        "alcomd_search_packages",
        "alcomd_get_package",
        "alcomd_plan_package_install",
        "alcomd_plan_package_reinstall",
        "alcomd_plan_package_changes",
        "alcomd_list_operations",
        "alcomd_list_repositories",
        "alcomd_inspect_repository",
    ] {
        assert!(
            published.iter().any(|tool| tool.name == name),
            "missing {name}"
        );
    }
    assert_eq!(
        remainder.next_cursor.is_some(),
        all.tools.len() + remainder.tools.len() < published.len()
    );
    assert!(
        sb.list_tools(Some(
            rmcp::model::PaginatedRequestParams::default().with_cursor(Some(next.clone()))
        ))
        .await
        .is_err(),
        "credential-bound cursor"
    );
    permission(&mut owner, "packages.manage", true, "revoke-mid-page").await;
    assert!(
        sa.list_tools(Some(
            rmcp::model::PaginatedRequestParams::default().with_cursor(Some(next))
        ))
        .await
        .is_ok(),
        "grant changes leave the signed catalogue unchanged"
    );
    permission(&mut owner, "packages.manage", false, "restore-manage").await;
    let denied = ext(
        &sa,
        "unauthorized_read",
        json!({"project_id":uuid::Uuid::new_v4().to_string()}),
    )
    .await;
    assert_eq!(denied.is_error, Some(true));
    let http = HttpFixture::start(&core).await;
    let http_sdk = sdk(&secret_a, Some(&http.url), &core).await;
    let http_echo = ext(&http_sdk, "echo", json!({"text":"HTTP provider"})).await;
    assert_ne!(http_echo.is_error, Some(true));
    let mut operations = Vec::new();
    let mut recovery_args = None;
    operations
        .push(super::oauth_flow::extension_provider_oauth(&mut owner, &root, &a, &core).await);
    for fault in [false, true] {
        use rmcp::model::{ClientRequest, Request, ServerResult};
        use rmcp::service::PeerRequestOptions;
        let p = root.join(format!("Fault-{fault}"));
        project(&p, false);
        let registered = owner
            .project_register(alcomd_protocol::ProjectRegisterParams {
                source: alcomd_protocol::ProjectSource::Absolute {
                    path: p.to_string_lossy().into_owned(),
                },
                idempotency_key: format!("fault-project-{fault}"),
            })
            .await
            .expect("valid isolated Provider fixture")
            .project;
        let planned=ext(&sa,"plan_remove",json!({"project_id":registered.project_id,"expected_revision":registered.revision,"package_id":"com.example.fixture","idempotency_key":format!("fault-plan-{fault}")})).await.structured_content.expect("valid isolated Provider fixture");
        let plan = &planned["coreEffect"];
        let args = json!({"plan_id":plan["planId"],"expected_revision":plan["projectRevision"],"idempotency_key":format!("fault-apply-{fault}"),"confirm":true});
        let risk_sdk = sdk(&secret_a, None, &core).await;
        let gate = daemon.hold_next_mcp_apply_response(fault).await;
        let mut pending = risk_sdk
            .send_cancellable_request(
                ClientRequest::CallToolRequest(Request::new(
                    CallToolRequestParams::new(public("apply")).with_arguments(
                        args.as_object()
                            .expect("valid isolated Provider fixture")
                            .clone(),
                    ),
                )),
                PeerRequestOptions::no_options(),
            )
            .await
            .expect("valid isolated Provider fixture");
        let accepted = tokio::time::timeout(Duration::from_secs(15), gate.accepted())
            .await
            .expect("real provider Apply acceptance before fault");
        assert!(!accepted.replayed);
        if !fault {
            let (_, replacement) = tokio::sync::oneshot::channel();
            let response = std::mem::replace(&mut pending.rx, replacement);
            pending
                .cancel(Some("isolated provider request cancellation".into()))
                .await
                .expect("valid isolated Provider fixture");
            assert!(matches!(
                response.await.expect("valid isolated Provider fixture"),
                Err(ServiceError::Cancelled { .. })
            ));
            gate.release();
        } else {
            gate.release();
            let result = tokio::time::timeout(Duration::from_secs(10), pending.await_response())
                .await
                .expect("valid isolated Provider fixture")
                .expect("valid isolated Provider fixture");
            let ServerResult::CallToolResult(result) = result else {
                panic!("unknown write tool response")
            };
            error(result, "core_write_outcome_unknown");
        }
        let recovery = sdk(&secret_a, None, &core).await;
        let restored = ext(&recovery, "apply", args).await;
        assert_ne!(restored.is_error, Some(true));
        let restored = restored
            .structured_content
            .expect("valid isolated Provider fixture");
        assert_eq!(restored["coreEffect"]["operationId"], accepted.operation_id);
        assert_eq!(restored["replayed"], true);
        let mut trace = vec![];
        assert_eq!(
            terminal(&recovery, &mut trace, &accepted.operation_id).await["state"],
            "succeeded"
        );
        assert!(!p.join("Packages/com.example.fixture").exists());
        operations.push(accepted.operation_id);
        stop(recovery).await;
        stop(risk_sdk).await;
    }

    for (index, apply) in [
        "apply",
        "apply_twice",
        "bad_after_apply",
        "trap_after_apply",
        "loop_after_apply",
    ]
    .into_iter()
    .enumerate()
    {
        let p = root.join(format!("Project-{index}"));
        project(&p, false);
        let record = owner
            .project_register(alcomd_protocol::ProjectRegisterParams {
                source: alcomd_protocol::ProjectSource::Absolute {
                    path: p.to_string_lossy().into_owned(),
                },
                idempotency_key: format!("register-{index}"),
            })
            .await
            .expect("valid isolated Provider fixture")
            .project;
        let args = json!({"project_id":record.project_id,"expected_revision":record.revision,"package_id":"com.example.fixture","idempotency_key":format!("plan-{index}")});
        let planned = ext(
            &sa,
            if index == 1 {
                "plan_twice"
            } else {
                "plan_remove"
            },
            args.clone(),
        )
        .await;
        assert_ne!(
            planned.is_error,
            Some(true),
            "Provider plan fixture {index} failed: {}",
            planned
                .structured_content
                .as_ref()
                .and_then(|result| result.pointer("/error/code"))
                .and_then(Value::as_str)
                .unwrap_or("missing stable error code")
        );
        let planned = planned
            .structured_content
            .expect("valid isolated Provider fixture");
        let effect = &planned["coreEffect"];
        assert_eq!(effect["kind"], "plan");
        assert_eq!(effect["source"]["extensionId"], ID);
        assert_eq!(effect["plan"]["action"], "remove");
        if index == 0 {
            let mut same_caller =
                AlcomdClient::connect(core.client.clone().with_mcp_credential(secret_a.clone()))
                    .await
                    .expect("same authenticated caller through native command RPC");
            let input = call_cli("plan_remove", args.clone());
            let original = same_caller
                .extension_cli_commands_invoke(input.clone())
                .await
                .expect("independent CLI plan");
            let independent = original.core_effect.expect("accepted CLI effect");
            assert_ne!(independent["planId"], effect["planId"]);
            let restored = same_caller
                .extension_cli_commands_invoke(input)
                .await
                .expect("CLI replay");
            assert!(restored.replayed);
            assert_eq!(restored.core_effect.as_ref(), Some(&independent));
        }
        let plan_replay = ext(
            &sa,
            if index == 1 {
                "plan_twice"
            } else {
                "plan_remove"
            },
            args,
        )
        .await
        .structured_content
        .expect("valid isolated Provider fixture");
        assert_eq!(plan_replay["coreEffect"]["planId"], effect["planId"]);
        assert_eq!(plan_replay["replayed"], true);
        let args = json!({"plan_id":effect["planId"],"expected_revision":effect["projectRevision"],"idempotency_key":format!("apply-{index}"),"confirm":true});
        let wrong = ext(&sb, apply, args.clone()).await;
        assert_eq!(wrong.is_error, Some(true));
        assert!(
            wrong
                .structured_content
                .expect("valid isolated Provider fixture")["coreEffect"]
                .is_null(),
            "B cannot apply A's plan"
        );
        recovery_args = Some(args.clone());
        let transport = if index == 0 { &http_sdk } else { &sa };
        let applied = ext(transport, apply, args.clone()).await;
        assert_eq!(applied.is_error == Some(true), index >= 2);
        let applied = applied
            .structured_content
            .expect("valid isolated Provider fixture");
        let operation = applied["coreEffect"]["operationId"]
            .as_str()
            .expect("authoritative accepted operation")
            .to_owned();
        if index != 4 {
            let replay = ext(transport, apply, args.clone())
                .await
                .structured_content
                .expect("valid isolated Provider fixture");
            assert_eq!(replay["coreEffect"]["operationId"], operation);
            assert_eq!(replay["replayed"], true);
        }
        let mut trace = vec![];
        let result = terminal(&sa, &mut trace, &operation).await;
        assert_eq!(result["state"], "succeeded");
        assert!(!p.join("Packages/com.example.fixture").exists());
        if index == 4 {
            assert_eq!(
                owner
                    .extension_get(ID.into())
                    .await
                    .expect("valid isolated Provider fixture")
                    .extension
                    .quarantine_state,
                ExtensionQuarantineState::Quarantined,
                "three genuine provider faults use existing isolation"
            );
            assert!(
                sb.call_tool_once(
                    CallToolRequestParams::new(public("operation")).with_arguments(
                        json!({"operation_id":operation})
                            .as_object()
                            .expect("valid isolated Provider fixture")
                            .clone()
                    )
                )
                .await
                .is_err(),
                "quarantined provider accepts no new call"
            );
        } else {
            let foreign = ext(&sb, "operation", json!({"operation_id":operation})).await;
            assert_eq!(foreign.is_error, Some(true));
        }
        operations.push(operation);
    }
    let removed = sa
        .list_all_tools()
        .await
        .expect("valid isolated Provider fixture");
    assert!(removed.iter().all(|t| !t.name.starts_with("alcomd_ext_")));
    assert_eq!(removed.len(), initial.len());
    let db = rusqlite::Connection::open_with_flags(
        data_root.join("state.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("valid isolated Provider fixture");
    let count: i64 = db
        .query_row(
            "SELECT count(*) FROM operations WHERE owner_principal_id=?1 AND kind='packages.apply'",
            [format!("mcp:{}", a.client_id)],
            |r| r.get(0),
        )
        .expect("valid isolated Provider fixture");
    assert_eq!(count, 8, "no repeated Core Apply");
    let owners: i64 = db
        .query_row(
            "SELECT count(*) FROM projects WHERE owner_principal_id<>'builtin:local-owner'",
            [],
            |r| r.get(0),
        )
        .expect("valid isolated Provider fixture");
    assert_eq!(owners, 0, "resource ownership was not rewritten");
    drop(db);
    let original = owner
        .extension_get(ID.into())
        .await
        .expect("valid isolated Provider fixture")
        .extension;
    owner
        .extension_disable(ExtensionLifecycleParams {
            extension_id: ID.into(),
            expected_revision: original.revision,
            idempotency_key: "disable-provider".into(),
        })
        .await
        .expect("valid isolated Provider fixture");
    let stale_args = recovery_args.expect("previously accepted call");
    let mut authenticated =
        AlcomdClient::connect(core.client.clone().with_mcp_credential(secret_a.clone()))
            .await
            .expect("connect as the original caller");
    let replay = authenticated
        .extension_mcp_tools_call(alcomd_protocol::ExtensionMcpToolsCallParams {
            name: public("loop_after_apply"),
            arguments: stale_args.clone(),
        })
        .await;
    assert!(
        matches!(replay, Err(alcomd_client::ClientError::Remote(error)) if error.code == "permission_denied"),
        "Core must withhold a disabled provider's receipt from the original caller"
    );
    drop(authenticated);
    assert!(
        sa.call_tool_once(
            CallToolRequestParams::new(public("loop_after_apply"))
                .with_arguments(stale_args.as_object().expect("arguments").clone())
        )
        .await
        .is_err(),
        "disabled extension cannot replay a previous write"
    );
    std::fs::write(root.join("summary.json"),serde_json::to_vec_pretty(&json!({"operations":operations,"signedProvider":ID,"clientA":a.client_id,"clientB":b.client_id,"stdio":true,"http":true})).expect("valid isolated Provider fixture")).expect("valid isolated Provider fixture");
    stop(http_sdk).await;
    http.finish().await;
    stop(sa).await;
    stop(sb).await;
    drop(owner);
    daemon.abort();
    let _ = daemon.await;
}
