//! Real frozen SDK client -> STDIO executable -> authenticated Core -> project transaction.
use alcomd_client::{AlcomdClient, ClientConfig};
use alcomd_platform::{DataConfig, IpcConfig};
use alcomd_protocol::*;
use rmcp::{
    ClientLifecycleMode, ClientServiceExt, RoleClient, ServiceError,
    model::{CallToolRequestParams, CallToolResponse, CallToolResult, ProtocolVersion},
    service::RunningService,
    transport::TokioChildProcess,
};
use serde_json::{Value, json};
use std::{path::Path, time::Duration};
type Sdk = RunningService<RoleClient, ()>;

async fn sdk(secret: &str) -> Sdk {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_alcomd-mcp"));
    command
        .arg("stdio")
        .env("ALCOMD_MCP_CLIENT_CREDENTIAL", secret)
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let transport = TokioChildProcess::new(command).expect("owned MCP STDIO process");
    ().serve_with_lifecycle(
        transport,
        ClientLifecycleMode::Discover {
            preferred_versions: vec![ProtocolVersion::V_2026_07_28],
        },
    )
    .await
    .expect("real SDK Discover, no legacy fallback")
}
async fn stop(client: Sdk) {
    tokio::time::timeout(Duration::from_secs(10), client.cancel())
        .await
        .expect("SDK close deadline")
        .expect("SDK close");
}
async fn call(
    client: &Sdk,
    trace: &mut Vec<Value>,
    name: &str,
    args: Value,
) -> Result<CallToolResult, ServiceError> {
    let params = CallToolRequestParams::new(format!("alcomd_{name}"))
        .with_arguments(args.as_object().expect("tool args").clone());
    let response = tokio::time::timeout(Duration::from_secs(30), client.call_tool_once(params))
        .await
        .expect("tool response deadline");
    match response {
        Ok(CallToolResponse::Complete(result)) => {
            trace.push(json!({"name":format!("alcomd_{name}"),"arguments":args,"result":result}));
            Ok(result)
        }
        Ok(_) => panic!("no Tasks or MRTR in this toolset"),
        Err(error) => {
            trace.push(
                json!({"name":format!("alcomd_{name}"),"arguments":args,"protocol_error":true}),
            );
            Err(error)
        }
    }
}
fn data(result: CallToolResult) -> Value {
    assert_ne!(result.is_error, Some(true), "business success required");
    result.structured_content.expect("structured data")["data"].clone()
}
fn error(result: CallToolResult, code: &str) {
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.expect("structured error")["error"]["code"],
        code
    );
}
fn grant(permission: &str, kind: &str, id: &str) -> McpClientGrant {
    McpClientGrant {
        permission: permission.into(),
        resource_kind: kind.into(),
        resource_id: id.into(),
    }
}
fn grants(project: &str, write: bool) -> Vec<McpClientGrant> {
    let mut result = vec![grant("projects.read", "project", project)];
    if write {
        result.push(grant("packages.manage", "project", project));
    }
    for p in [
        "packages.read",
        "repositories.read",
        "operations.read",
        "operations.cancel",
    ] {
        result.push(grant(p, "global", "*"));
    }
    result
}
async fn update(
    owner: &mut AlcomdClient,
    client: &McpClientRecord,
    grants: Vec<McpClientGrant>,
    key: &str,
) -> McpClientRecord {
    owner
        .mcp_client_set_grants(McpClientsSetGrantsParams {
            client_id: client.client_id.clone(),
            expected_revision: client.revision,
            idempotency_key: key.into(),
            grants,
        })
        .await
        .expect("explicit owner grants")
}
fn project(root: &Path, risk: bool) {
    std::fs::create_dir_all(root.join("ProjectSettings")).expect("isolated project");
    std::fs::create_dir(root.join("Packages")).expect("isolated packages");
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
    let package = |name: &str, dependencies: Value| {
        let path = root.join("Packages").join(name);
        std::fs::create_dir(&path).expect("package folder");
        std::fs::write(
            path.join("package.json"),
            json!({"name":name,"version":"1.0.0","dependencies":dependencies}).to_string(),
        )
        .expect("package manifest");
    };
    let manifest = if risk {
        package("com.example.a", json!({"com.example.b":"1.0.0"}));
        package("com.example.b", json!({}));
        json!({"dependencies":{"com.example.a":{"version":"1.0.0"}},"locked":{"com.example.a":{"version":"1.0.0","dependencies":{"com.example.b":"1.0.0"}},"com.example.b":{"version":"1.0.0"}}})
    } else {
        package("com.example.fixture", json!({}));
        json!({"dependencies":{"com.example.fixture":{"version":"1.0.0"}},"locked":{"com.example.fixture":{"version":"1.0.0"}}})
    };
    std::fs::write(
        root.join("Packages/vpm-manifest.json"),
        manifest.to_string(),
    )
    .expect("VPM");
}
async fn terminal(client: &Sdk, trace: &mut Vec<Value>, id: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let op = data(
                call(client, trace, "get_operation", json!({"operation_id":id}))
                    .await
                    .expect("operation read"),
            );
            if ["succeeded", "failed", "cancelled"].contains(&op["state"].as_str().expect("state"))
            {
                return op;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("business terminal deadline")
}

/// Bind the real per-user endpoint before any client is created. On Windows the
/// authorized preservation runner must release the original Core first.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn frozen_sdk_package_removal_real_core_flow() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-business-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).expect("owned evidence root");
    let root = root.canonicalize().expect("fixture root");
    let data_root = root.join("data");
    std::fs::create_dir(&data_root).expect("isolated Core data");
    let ipc = IpcConfig::default();
    let config = ClientConfig::default().without_daemon_start();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc.clone(),
        DataConfig::isolated(data_root.clone()),
        std::future::pending(),
    )
    .await
    .expect("exclusive real Core bind");
    let mut owner = AlcomdClient::connect(config.clone())
        .await
        .expect("native test owner");
    let p = root.join("Project");
    let risk = root.join("Risk");
    let cancel = root.join("Cancellation");
    let detached = root.join("Detached");
    project(&p, false);
    project(&risk, true);
    project(&cancel, false);
    project(&detached, false);
    let first = owner
        .project_register(p.to_string_lossy().into_owned(), "register-one".into())
        .await
        .expect("register")
        .project;
    let risk_record = owner
        .project_register(risk.to_string_lossy().into_owned(), "register-risk".into())
        .await
        .expect("register risk")
        .project;
    let cancel_record = owner
        .project_register(
            cancel.to_string_lossy().into_owned(),
            "register-cancel".into(),
        )
        .await
        .expect("register cancellation")
        .project;
    let detached_record = owner
        .project_register(
            detached.to_string_lossy().into_owned(),
            "register-detached".into(),
        )
        .await
        .expect("register detached")
        .project;
    let id = first.project_id.clone().expect("ID");
    let rid = risk_record.project_id.clone().expect("risk ID");
    let cid = cancel_record.project_id.clone().expect("cancel ID");
    let before = std::fs::read(p.join("Packages/vpm-manifest.json")).expect("before manifest");
    let risk_before = std::fs::read(risk.join("Packages/vpm-manifest.json")).expect("risk before");
    let upm_before = std::fs::read(p.join("Packages/manifest.json")).expect("UPM before");
    let mut a = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Tools A".into(),
            grants: vec![],
            idempotency_key: "client-a".into(),
        })
        .await
        .expect("create A");
    let mut b = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Tools B".into(),
            grants: vec![],
            idempotency_key: "client-b".into(),
        })
        .await
        .expect("create B");
    let token_a = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: a.client_id.clone(),
            expected_revision: a.revision,
            idempotency_key: "issue-a".into(),
        })
        .await
        .expect("issue A");
    a.revision = token_a.revision;
    let token_b = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: b.client_id.clone(),
            expected_revision: b.revision,
            idempotency_key: "issue-b".into(),
        })
        .await
        .expect("issue B");
    b.revision = token_b.revision;
    let sa = sdk(&token_a.credential).await;
    let sb = sdk(&token_b.credential).await;
    let mut trace = vec![];
    let definitions = sa.list_tools(None).await.expect("SDK tools/list");
    assert_eq!(definitions.tools.len(), 6);
    assert_eq!(definitions.ttl_ms, Some(0));
    std::fs::write(
        root.join("tools-list.json"),
        serde_json::to_vec_pretty(&definitions).expect("list JSON"),
    )
    .expect("definitions evidence");
    error(
        call(&sa, &mut trace, "list_projects", json!({}))
            .await
            .expect("default reject"),
        "permission_denied",
    );
    a = update(&mut owner, &a, grants(&id, false), "read-a").await;
    let listed = data(
        call(&sa, &mut trace, "list_projects", json!({"limit":1}))
            .await
            .expect("filtered read"),
    );
    assert_eq!(listed["projects"].as_array().expect("projects").len(), 1);
    assert_eq!(listed["projects"][0]["project_id"], id);
    error(
        call(&sa, &mut trace, "get_project", json!({"project_id":rid}))
            .await
            .expect("outside scope"),
        "permission_denied",
    );
    let read = data(
        call(&sa, &mut trace, "get_project", json!({"project_id":id}))
            .await
            .expect("snapshot"),
    );
    assert_eq!(
        read["locked_dependencies"][0]["package_id"],
        "com.example.fixture"
    );
    assert!(read.get("root_path").is_none());
    error(call(&sa,&mut trace,"plan_package_uninstall",json!({"project_id":id,"expected_revision":first.revision.expect("revision"),"package_id":"com.example.absent"})).await.expect("real business failure"),"package_not_found");
    let old_plan=data(call(&sa,&mut trace,"plan_package_uninstall",json!({"project_id":id,"expected_revision":first.revision.expect("revision"),"package_id":"com.example.fixture"})).await.expect("real plan"));
    let args = json!({"plan_id":old_plan["plan_id"],"expected_revision":old_plan["project_revision"],"idempotency_key":"apply","confirm":true});
    error(
        call(&sa, &mut trace, "apply_package_changes", args.clone())
            .await
            .expect("read cannot write"),
        "permission_denied",
    );
    let mut unconfirmed = args.clone();
    unconfirmed["confirm"] = json!(false);
    assert!(
        call(&sa, &mut trace, "apply_package_changes", unconfirmed)
            .await
            .is_err()
    );
    assert_eq!(
        std::fs::read(p.join("Packages/vpm-manifest.json")).expect("no pre-confirm write"),
        before
    );
    a = update(&mut owner, &a, grants(&id, true), "write-a").await;
    update(&mut owner, &b, grants(&id, true), "same-resource-b").await;
    error(
        call(&sb, &mut trace, "apply_package_changes", args.clone())
            .await
            .expect("foreign plan"),
        "plan_not_found",
    );
    let mut changed: Value = serde_json::from_slice(&before).expect("manifest");
    changed["dependencies"]["com.example.fixture"]["version"] = json!("^1.0.0");
    std::fs::write(p.join("Packages/vpm-manifest.json"), changed.to_string())
        .expect("authorized disposable external edit");
    let refreshed = owner
        .project_refresh(
            id.clone(),
            first.revision.expect("revision"),
            "refresh-fixture".into(),
        )
        .await
        .expect("current registry refresh")
        .project;
    assert!(refreshed.revision > first.revision);
    error(
        call(&sa, &mut trace, "apply_package_changes", args)
            .await
            .expect("old plan must fail"),
        "plan_stale",
    );
    let mut scoped = grants(&id, true);
    scoped.push(grant("projects.read", "project", &rid));
    scoped.push(grant("projects.read", "project", &cid));
    scoped.push(grant("packages.manage", "project", &cid));
    a = update(&mut owner, &a, scoped, "explicit-second-projects").await;
    let risks=data(call(&sa,&mut trace,"plan_package_uninstall",json!({"project_id":rid,"expected_revision":risk_record.revision,"package_id":"com.example.b"})).await.expect("dependency risk plan"));
    assert_eq!(
        risks["change_set"]["removal_conflicts"][0]["package_id"],
        "com.example.b"
    );
    error(call(&sa,&mut trace,"apply_package_changes",json!({"plan_id":risks["plan_id"],"expected_revision":risks["project_revision"],"idempotency_key":"no-write-scope","confirm":true})).await.expect("read scope is not write scope"),"permission_denied");
    let plan=data(call(&sa,&mut trace,"plan_package_uninstall",json!({"project_id":id,"expected_revision":refreshed.revision,"package_id":"com.example.fixture"})).await.expect("fresh plan"));
    let apply = json!({"plan_id":plan["plan_id"],"expected_revision":plan["project_revision"],"idempotency_key":"accepted-removal","confirm":true});
    let accepted = data(
        call(&sa, &mut trace, "apply_package_changes", apply.clone())
            .await
            .expect("actual Core Apply"),
    );
    let operation = accepted["operation_id"]
        .as_str()
        .expect("operation ID")
        .to_owned();
    error(
        call(
            &sb,
            &mut trace,
            "get_operation",
            json!({"operation_id":operation}),
        )
        .await
        .expect("foreign task"),
        "operation_not_found",
    );
    let completed = terminal(&sa, &mut trace, &operation).await;
    assert_eq!(completed["state"], "succeeded");
    assert_eq!(completed["result"]["plan_id"], plan["plan_id"]);
    assert_eq!(
        completed["result"]["change_set_fingerprint"],
        plan["change_set_fingerprint"]
    );
    let replay = data(
        call(&sa, &mut trace, "apply_package_changes", apply.clone())
            .await
            .expect("same key replay"),
    );
    assert_eq!(replay["operation_id"], operation);
    assert_eq!(replay["replayed"], true);
    assert!(!p.join("Packages/com.example.fixture").exists());
    let after: Value = serde_json::from_slice(
        &std::fs::read(p.join("Packages/vpm-manifest.json")).expect("actual manifest"),
    )
    .expect("actual JSON");
    assert!(after["dependencies"].get("com.example.fixture").is_none());
    assert!(after["locked"].get("com.example.fixture").is_none());
    assert_eq!(
        std::fs::read(p.join("Packages/manifest.json")).expect("UPM after"),
        upm_before
    );
    a = update(&mut owner, &a, grants(&id, false), "withdraw-write").await;
    error(
        call(&sa, &mut trace, "apply_package_changes", apply.clone())
            .await
            .expect("replay cannot bypass withdrawal"),
        "permission_denied",
    );
    a = update(&mut owner, &a, grants(&cid, true), "cancel-project").await;
    let cancel_plan=data(call(&sa,&mut trace,"plan_package_uninstall",json!({"project_id":cid,"expected_revision":cancel_record.revision,"package_id":"com.example.fixture"})).await.expect("cancel plan"));
    let cancel_apply=data(call(&sa,&mut trace,"apply_package_changes",json!({"plan_id":cancel_plan["plan_id"],"expected_revision":cancel_plan["project_revision"],"idempotency_key":"cancel-task","confirm":true})).await.expect("cancel candidate"));
    let cancel_id = cancel_apply["operation_id"].as_str().expect("cancel ID");
    let state = data(
        call(
            &sa,
            &mut trace,
            "get_operation",
            json!({"operation_id":cancel_id}),
        )
        .await
        .expect("current task revision"),
    );
    let cancellation=call(&sa,&mut trace,"cancel_operation",json!({"operation_id":cancel_id,"expected_revision":state["revision"],"idempotency_key":"cancel"})).await.expect("real cancellation response");
    if cancellation.is_error == Some(true) {
        let code = &cancellation.structured_content.expect("cancel error")["error"]["code"];
        assert!(code == "operation_not_cancellable" || code == "revision_conflict");
    } else {
        let returned = data(cancellation);
        assert!(
            returned["operation"]["state"] == "cancelling"
                || returned["operation"]["state"] == "cancelled"
                || returned["operation"]["state"] == "succeeded"
        );
    }
    let cancel_terminal = terminal(&sa, &mut trace, cancel_id).await;
    assert!(cancel_terminal["state"] == "succeeded" || cancel_terminal["state"] == "cancelled");
    let detached_id = detached_record.project_id.expect("detached project ID");
    a = update(
        &mut owner,
        &a,
        grants(&detached_id, true),
        "detached-grants",
    )
    .await;
    let detached_plan=data(call(&sa,&mut trace,"plan_package_uninstall",json!({"project_id":detached_id,"expected_revision":detached_record.revision,"package_id":"com.example.fixture"})).await.expect("detached plan"));
    let detached_op=data(call(&sa,&mut trace,"apply_package_changes",json!({"plan_id":detached_plan["plan_id"],"expected_revision":detached_plan["project_revision"],"idempotency_key":"disconnect-after-accept","confirm":true})).await.expect("accepted before EOF"));
    // Close the actual SDK/STDIO process immediately after acceptance. Query the
    // accepted task using a new independently authenticated SDK connection.
    stop(sa).await;
    let reconnected = sdk(&token_a.credential).await;
    let detached_terminal = terminal(
        &reconnected,
        &mut trace,
        detached_op["operation_id"]
            .as_str()
            .expect("detached operation"),
    )
    .await;
    assert_eq!(detached_terminal["state"], "succeeded");
    assert!(!detached.join("Packages/com.example.fixture").exists());
    stop(reconnected).await;
    stop(sb).await;
    drop(owner);
    daemon.abort();
    let _ = daemon.await;
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc,
        DataConfig::isolated(data_root.clone()),
        std::future::pending(),
    )
    .await
    .expect("restart authoritative data");
    let mut owner = AlcomdClient::connect(config.clone())
        .await
        .expect("native owner after restart");
    a = update(&mut owner, &a, grants(&id, true), "restore-original-scope").await;
    let sa = sdk(&token_a.credential).await;
    let sb = sdk(&token_b.credential).await;
    let persisted = data(
        call(
            &sa,
            &mut trace,
            "get_operation",
            json!({"operation_id":operation}),
        )
        .await
        .expect("persisted result"),
    );
    assert_eq!(persisted["state"], "succeeded");
    let persisted_replay = data(
        call(&sa, &mut trace, "apply_package_changes", apply)
            .await
            .expect("persistent idempotency"),
    );
    assert_eq!(persisted_replay["operation_id"], operation);
    owner
        .mcp_client_revoke(McpClientMutationParams {
            client_id: a.client_id.clone(),
            expected_revision: a.revision,
            idempotency_key: "revoke-a".into(),
        })
        .await
        .expect("revoke connected client");
    error(
        call(&sa, &mut trace, "get_project", json!({"project_id":id}))
            .await
            .expect("revoked tool returns error"),
        "permission_denied",
    );
    assert!(sa.list_tools(None).await.is_err());
    assert_eq!(
        data(
            call(&sb, &mut trace, "get_project", json!({"project_id":id}))
                .await
                .expect("independent B")
        )["project_id"],
        id
    );
    stop(sa).await;
    stop(sb).await;
    drop(owner);
    daemon.abort();
    let _ = daemon.await;
    assert_eq!(
        std::fs::read(risk.join("Packages/vpm-manifest.json")).expect("risk unchanged"),
        risk_before
    );
    let db = rusqlite::Connection::open_with_flags(
        data_root.join("state.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("isolated read-only evidence");
    let owner_id: String = db
        .query_row(
            "SELECT owner_principal_id FROM projects WHERE project_id=?1",
            [&id],
            |r| r.get(0),
        )
        .expect("resource owner");
    assert_eq!(owner_id, "builtin:local-owner");
    let task_owner: String = db
        .query_row(
            "SELECT owner_principal_id FROM operations WHERE operation_id=?1",
            [&operation],
            |r| r.get(0),
        )
        .expect("caller owner");
    assert_eq!(task_owner, a.principal_id);
    let stored_plan: String = db
        .query_row(
            "SELECT plan_id FROM package_plans WHERE apply_operation_id=?1",
            [&operation],
            |r| r.get(0),
        )
        .expect("original plan");
    assert_eq!(stored_plan, plan["plan_id"]);
    let trace_json = serde_json::to_vec_pretty(&trace).expect("trace");
    let visible = String::from_utf8_lossy(&trace_json);
    assert!(!visible.contains(&token_a.credential));
    assert!(!visible.contains(&token_b.credential));
    assert!(!visible.contains("root_path"));
    std::fs::write(root.join("tool-trace.json"), trace_json).expect("public contract evidence");
    std::fs::write(root.join("coverage.json"),json!({"cancelTerminal":cancel_terminal["state"],"realSdk":"rmcp 3.10.0 Discover","business":"removed actual VPM package","revoked":true,"resourceOwnerPreserved":true}).to_string()).expect("coverage");
    println!(
        "Frozen SDK business closure verified; isolated evidence: {}",
        root.display()
    );
}
