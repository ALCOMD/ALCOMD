use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn registered_ai_lists_native_instances_and_disconnects_one_real_socket() {
    let _serial = REAL_CORE_LOCK.lock().await;
    let core = CoreConfiguration::new();
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .join("target/m8-business-tests")
        .join(uuid::Uuid::new_v4().to_string());
    let data = root.join("data");
    std::fs::create_dir_all(&data).expect("create isolated Core data directory");
    let (shutdown, stopped) = tokio::sync::oneshot::channel();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        core.ipc.clone(),
        DataConfig::isolated(data),
        async {
            let _ = stopped.await;
        },
    )
    .await
    .expect("owned Core");
    let mut owner = AlcomdClient::connect(core.client.clone())
        .await
        .expect("owner");
    let actor = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Registry reader".into(),
            idempotency_key: "client-registration".into(),
        })
        .await
        .expect("register");
    let credential = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: actor.client_id,
            expected_revision: actor.revision,
            idempotency_key: "issue-registry-test".into(),
        })
        .await
        .expect("issue");
    let mut target =
        AlcomdClient::connect(core.client.clone().with_client_name("registry-test-target"))
            .await
            .expect("native target");
    let ai = sdk(&credential.credential, None, &core).await;
    let mut trace = vec![];
    let identity = data(
        call(&ai, &mut trace, "list_ai_clients", json!({}))
            .await
            .expect("list own identity"),
    );
    assert_eq!(
        identity["items"].as_array().expect("identity page").len(),
        1
    );
    assert_eq!(identity["items"][0]["display_name"], "Registry reader");
    let request = json!({"action":{"kind":"createClient","display_name":"Approved assistant"},"idempotency_key":"new-configured-client","confirm":true});
    let pending = data(
        call(&ai, &mut trace, "request_client_change", request.clone())
            .await
            .expect("submit"),
    );
    assert_eq!(
        data(
            call(&ai, &mut trace, "request_client_change", request)
                .await
                .expect("repeat same request")
        ),
        pending
    );
    assert_eq!(pending["status"], "pending");
    let id = pending["request_id"].as_str().expect("review ID");
    let original = data(
        call(
            &ai,
            &mut trace,
            "get_client_change_request",
            json!({"request_id":id}),
        )
        .await
        .expect("original"),
    );
    assert_eq!(original, pending);
    let list = data(
        call(&ai, &mut trace, "list_client_change_requests", json!({}))
            .await
            .expect("my requests"),
    );
    assert_eq!(list["items"], json!([pending]));
    let decision = owner
        .mcp_client_request_decide(alcomd_protocol::McpClientRequestDecideParams {
            request_id: id.to_owned(),
            expected_fingerprint: pending["fingerprint"]
                .as_str()
                .expect("review fingerprint")
                .to_owned(),
            approve: true,
            idempotency_key: "approve-configured-client".into(),
        })
        .await
        .expect("owner decision");
    assert_eq!(
        serde_json::to_value(decision).expect("decision response")["request"]["status"],
        "approved"
    );
    let records = owner
        .mcp_clients_list(alcomd_protocol::McpClientsListParams::default())
        .await
        .expect("owner sees both clients");
    assert_eq!(records.items.len(), 2);
    let scoped = data(
        call(&ai, &mut trace, "list_ai_clients", json!({}))
            .await
            .expect("AI still sees only its own record"),
    );
    assert_eq!(identity, scoped);
    let desktop = daemon.observe_desktop();
    for _ in 0..2 {
        let response = call(
            &ai,
            &mut trace,
            "open_unity_hub_download",
            json!({"idempotency_key":"mcp-download"}),
        )
        .await
        .expect("MCP desktop receipt");
        assert_eq!(data(response)["outcome"], "requested");
    }
    assert_eq!(
        desktop
            .calls
            .lock()
            .expect("MCP desktop receipt")
            .as_slice(),
        [("download".to_owned(), None)]
    );
    let programs = data(
        call(&ai, &mut trace, "list_clients", json!({}))
            .await
            .expect("list"),
    );
    let mut selected = None;
    for program in programs["items"].as_array().expect("native programs") {
        if program["active_connection_count"] == 0 {
            continue;
        }
        let instances = data(
            call(
                &ai,
                &mut trace,
                "list_client_instances",
                json!({"program_key":program["program_key"]}),
            )
            .await
            .expect("list instances"),
        );
        for instance in instances["items"].as_array().expect("instances") {
            if let Some(found) = instance["connections"]
                .as_array()
                .expect("connections")
                .iter()
                .find(|row| row["reported"]["name"] == "registry-test-target")
            {
                selected = Some(found["connection_id"].clone());
            }
        }
        if selected.is_some() {
            break;
        }
    }
    let id = selected.expect("target is connected");
    let arguments = json!({"expected_core_instance_id":programs["core_instance_id"],"target":{"kind":"connection","id":id},"idempotency_key":"disconnect-selected","confirm":true});
    let receipt = data(
        call(&ai, &mut trace, "disconnect_client", arguments.clone())
            .await
            .expect("disconnect"),
    );
    assert_eq!(receipt["disconnected_connection_ids"], json!([id]));
    assert_eq!(
        receipt,
        data(
            call(&ai, &mut trace, "disconnect_client", arguments)
                .await
                .expect("replay")
        )
    );
    assert!(target.system_status().await.is_err());
    assert!(owner.system_status().await.is_ok());
    stop(ai).await;
    drop((target, owner));
    let _ = shutdown.send(());
    daemon.await.expect("join").expect("shutdown");
}
