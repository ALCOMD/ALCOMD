use super::*;
use rmcp::model::SubscriptionFilter;

async fn start_http_subscription_check(
    core: &CoreConfiguration,
) -> (
    alcomd_daemon::test_support::TestDaemon,
    AlcomdClient,
    HttpFixture,
) {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-subscription-limits")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(root.join("data")).expect("owned disposable data");
    let root = root.canonicalize().expect("normalized isolation");
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        core.ipc.clone(),
        DataConfig::isolated(root.join("data")),
        std::future::pending(),
    )
    .await
    .expect("exclusive isolated Core");
    let owner = AlcomdClient::connect(core.client.clone())
        .await
        .expect("test owner");
    (daemon, owner, HttpFixture::start(core).await)
}

async fn subscription_reader(owner: &mut AlcomdClient, name: &str) -> (McpClientRecord, String) {
    let client = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: name.into(),
            idempotency_key: format!("create-{name}"),
        })
        .await
        .expect("isolated read client");
    let credential = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: client.client_id.clone(),
            expected_revision: client.revision,
            idempotency_key: format!("issue-{name}"),
        })
        .await
        .expect("real read credential");
    let client = McpClientRecord {
        revision: credential.revision,
        ..client
    };
    (client, credential.credential)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "uses the real ten-minute subscription lifetime and an isolated Core endpoint"]
async fn http_subscription_reaches_stream_limit_with_valid_bearer() {
    use rmcp::service::SubscriptionEnd;
    use tokio::time::Instant;

    let _serial = REAL_CORE_LOCK.lock().await;
    let core = CoreConfiguration::new();
    let (daemon, mut owner, http) = start_http_subscription_check(&core).await;
    let (_, credential) = subscription_reader(&mut owner, "stream-lifetime").await;
    let client = sdk(&credential, Some(&http.url), &core).await;
    let filter = SubscriptionFilter::builder().tools_list_changed().build();
    let started = Instant::now();
    let mut stream = client.listen(filter.clone()).await.expect("real SDK ack");
    let acknowledged = Instant::now();
    assert_eq!(stream.acknowledged().tools_list_changed, Some(true));
    assert!(
        tokio::time::timeout_at(started + Duration::from_secs(595), stream.next())
            .await
            .is_err(),
        "unchanged authorized catalogue stays open near the ten-minute limit"
    );
    let mut trace = vec![];
    assert!(
        data(
            call(&client, &mut trace, "list_projects", json!({}))
                .await
                .expect("ordinary call near stream deadline")
        )
        .is_object()
    );
    let end = tokio::time::timeout_at(started + Duration::from_secs(610), stream.next())
        .await
        .expect("bounded absolute stream lifetime")
        .expect("no protocol error at the age limit");
    assert!(end.is_none(), "no false catalogue notification");
    assert!(
        matches!(stream.end(), Some(SubscriptionEnd::Graceful(_))),
        "age limit must emit the terminal result, got {:?}",
        stream.end()
    );
    let elapsed = started.elapsed();
    assert!(elapsed >= Duration::from_secs(598));
    assert!(
        acknowledged.elapsed() <= Duration::from_secs(601),
        "the terminal result must arrive within the hard age limit and timer precision"
    );
    assert!(
        data(
            call(&client, &mut trace, "list_projects", json!({}))
                .await
                .expect("same credential still authorizes a real call")
        )
        .is_object()
    );
    let mut reopened = client
        .listen(filter)
        .await
        .expect("same credential opens a new stream");
    reopened.cancel().await.expect("explicit cancellation");
    eprintln!(
        "Subscription ended gracefully after {} ms with the same still-valid Bearer",
        elapsed.as_millis()
    );
    stop(client).await;
    http.finish().await;
    drop(owner);
    daemon.abort();
    let _ = daemon.await;
}

async fn raw_subscription_check_request(
    client: &reqwest::Client,
    url: &str,
    credential: &str,
    method: &str,
    mut params: Value,
) -> Result<reqwest::Response, reqwest::Error> {
    params["_meta"] = json!({
        "io.modelcontextprotocol/protocolVersion":"2026-07-28",
        "io.modelcontextprotocol/clientCapabilities":{}
    });
    let mut request = client
        .post(url)
        .bearer_auth(credential)
        .header("Accept", "application/json,text/event-stream")
        .header("MCP-Protocol-Version", "2026-07-28")
        .header("Mcp-Method", method)
        .json(&json!({"jsonrpc":"2.0","id":7,"method":method,"params":params}));
    if method == "tools/call" {
        request = request.header("Mcp-Name", "alcomd_list_projects");
    }
    request.send().await
}

async fn wait_subscription_counts(
    owner: &mut AlcomdClient,
    streams: Option<u32>,
    sockets: Option<u32>,
) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let snapshot = owner
                .mcp_transport_snapshot(Default::default())
                .await
                .expect("real transport observation");
            if snapshot.status == McpTransportAvailability::Available
                && streams.is_none_or(|count| {
                    snapshot
                        .subscription_streams
                        .as_ref()
                        .is_some_and(|page| page.total_tracked == count)
                })
                && sockets.is_none_or(|count| {
                    snapshot
                        .connections
                        .as_ref()
                        .is_some_and(|page| page.total_tracked == count)
                })
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("reported stream and connection counts converge");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "holds real local HTTP resources and uses an isolated Core endpoint"]
async fn http_subscription_capacity_and_paused_reader_isolate_business() {
    let _serial = REAL_CORE_LOCK.lock().await;
    let core = CoreConfiguration::new();
    let (daemon, mut owner, http) = start_http_subscription_check(&core).await;
    let mut readers = Vec::new();
    for i in 0..4 {
        readers.push(subscription_reader(&mut owner, &format!("capacity-{i}")).await);
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .pool_max_idle_per_host(0)
        .connect_timeout(Duration::from_secs(3))
        .build()
        .expect("local unpooled HTTP client");
    let listen = |credential| {
        raw_subscription_check_request(
            &client,
            &http.url,
            credential,
            "subscriptions/listen",
            json!({"notifications":{"toolsListChanged":true}}),
        )
    };
    let tool = |credential| {
        raw_subscription_check_request(
            &client,
            &http.url,
            credential,
            "tools/call",
            json!({"name":"alcomd_list_projects","arguments":{}}),
        )
    };
    let mut paused = Vec::new();
    for _ in 0..4 {
        let response = listen(&readers[0].1).await.expect("accepted stream");
        assert_eq!(response.status(), 200);
        paused.push(response);
    }
    wait_subscription_counts(&mut owner, Some(4), None).await;
    assert_eq!(
        listen(&readers[0].1)
            .await
            .expect("same credential limit")
            .status(),
        429
    );
    // The application has not polled any body on these four connections.
    // Cross one real SSE heartbeat interval while retaining all four slots.
    tokio::time::sleep(Duration::from_secs(16)).await;
    wait_subscription_counts(&mut owner, Some(4), None).await;
    assert_read_tool_success(
        tool(&readers[1].1)
            .await
            .expect("another client can still call"),
    )
    .await;
    let mut streams = paused;
    for (_, credential) in &readers[1..] {
        for _ in 0..4 {
            let response = listen(credential)
                .await
                .expect("independent credential quota");
            assert_eq!(response.status(), 200);
            streams.push(response);
        }
    }
    wait_subscription_counts(&mut owner, Some(16), None).await;
    assert_eq!(
        tool(&readers[1].1)
            .await
            .expect("shared request capacity")
            .status(),
        429
    );
    drop(streams.pop());
    wait_subscription_counts(&mut owner, Some(15), None).await;
    assert_read_tool_success(
        tool(&readers[1].1)
            .await
            .expect("one available request slot"),
    )
    .await;
    let restored = listen(&readers[3].1).await.expect("slot reusable");
    assert_eq!(restored.status(), 200);
    drop(restored);
    drop(streams);
    wait_subscription_counts(&mut owner, Some(0), Some(0)).await;

    let authority = http
        .url
        .trim_start_matches("http://")
        .trim_end_matches("/mcp");
    let mut sockets = Vec::new();
    for _ in 0..32 {
        sockets.push(
            tokio::net::TcpStream::connect(authority)
                .await
                .expect("owned TCP connection"),
        );
    }
    wait_subscription_counts(&mut owner, Some(0), Some(32)).await;
    assert!(
        tool(&readers[1].1).await.is_err(),
        "33rd TCP connection must not receive a response"
    );
    drop(sockets.pop());
    wait_subscription_counts(&mut owner, Some(0), Some(31)).await;
    assert_read_tool_success(tool(&readers[1].1).await.expect("one available TCP slot")).await;
    drop(sockets);
    eprintln!("Verified four unread streams, 4/16/32 admission and ordinary tool recovery");
    http.finish().await;
    drop(owner);
    daemon.abort();
    let _ = daemon.await;
}

async fn assert_read_tool_success(response: reqwest::Response) {
    assert_eq!(response.status(), 200);
    let body = response.text().await.expect("ordinary finite tool result");
    let result: Value = body
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|message| message["id"] == 7)
        .expect("real JSON-RPC result");
    assert_ne!(result["result"]["isError"], true);
    assert!(result["result"]["structuredContent"]["data"].is_object());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn lost_observer_ticket_reply_does_not_reconnect_reissue_or_interrupt_business() {
    let _serial = REAL_CORE_LOCK.lock().await;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-observer-loss-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(root.join("data")).expect("disposable data");
    let root = root.canonicalize().expect("normalized isolation");
    let core = CoreConfiguration::new();
    let config = core.client.clone();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        core.ipc.clone(),
        DataConfig::isolated(root.join("data")),
        std::future::pending(),
    )
    .await
    .expect("exclusive isolated Core");
    let mut owner = AlcomdClient::connect(config).await.expect("test owner");
    let client = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Observer loss".into(),
            idempotency_key: "loss-client".into(),
        })
        .await
        .expect("read client");
    let credential = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: client.client_id,
            expected_revision: client.revision,
            idempotency_key: "loss-credential".into(),
        })
        .await
        .expect("one-time credential");
    let gate = daemon.hold_next_transport_binding_response().await;
    let http = HttpFixture::start(&core).await;
    let sdk = sdk(&credential.credential, Some(&http.url), &core).await;
    let mut subscription = sdk
        .listen(SubscriptionFilter::builder().tools_list_changed().build())
        .await
        .expect("real accepted subscription");
    tokio::time::timeout(Duration::from_secs(5), gate.accepted())
        .await
        .expect("Core accepted a ticket-bearing report before its reply is lost");
    let accepted = owner
        .mcp_transport_snapshot(Default::default())
        .await
        .expect("accepted real metadata");
    assert!(
        accepted
            .requests
            .expect("readable requests")
            .items
            .iter()
            .any(|r| r.client_id.is_none()),
        "unreturned tickets must not invent an authenticated client"
    );
    gate.release();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if owner
                .mcp_transport_snapshot(Default::default())
                .await
                .expect("observer close")
                .observers
                .is_empty()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("lost observer reply closes only that channel");
    // More than two reporting periods: the old retry loop would create another
    // observer and ask Core for replacement tickets for the still-active stream.
    tokio::time::sleep(Duration::from_millis(2300)).await;
    let unavailable = owner
        .mcp_transport_snapshot(Default::default())
        .await
        .expect("explicit incomplete coverage");
    assert!(
        unavailable.observers.is_empty(),
        "no reconnect or ticket reissue after unknown outcome"
    );
    assert_eq!(unavailable.status, McpTransportAvailability::Unavailable);
    assert!(unavailable.coverage_incomplete);
    assert!(
        sdk.list_all_tools()
            .await
            .expect("authorized business connection still works")
            .iter()
            .any(|tool| tool.name == "alcomd_apply_package_changes")
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(1200), subscription.next())
            .await
            .is_err(),
        "metadata channel failure cannot cancel the independent subscription"
    );
    subscription
        .cancel()
        .await
        .expect("explicit protocol cancellation");
    stop(sdk).await;
    http.finish().await;
    drop(owner);
    daemon.abort();
    let _ = daemon.await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn frozen_sdk_stdio_subscriptions_and_real_transport_binding() {
    flow(false).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn frozen_sdk_http_subscriptions_and_real_transport_binding() {
    flow(true).await;
}

async fn flow(http: bool) {
    let _serial = REAL_CORE_LOCK.lock().await;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-subscription-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).expect("owned fixture");
    let root = root.canonicalize().expect("normalized isolated root");
    let data_root = root.join("data");
    std::fs::create_dir(&data_root).expect("fresh data");
    let core = CoreConfiguration::new();
    let config = core.client.clone();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        core.ipc.clone(),
        DataConfig::isolated(data_root.clone()),
        std::future::pending(),
    )
    .await
    .expect("exclusive disposable Core");
    let mut owner = AlcomdClient::connect(config.clone())
        .await
        .expect("test owner");
    let initial = owner
        .mcp_transport_snapshot(Default::default())
        .await
        .expect("unavailable projection");
    assert_eq!(initial.status, McpTransportAvailability::Unavailable);
    assert!(initial.coverage_incomplete);
    let mut a = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Subscriptions A".into(),
            idempotency_key: "subscription-a".into(),
        })
        .await
        .expect("A");
    let mut b = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Subscriptions B".into(),
            idempotency_key: "subscription-b".into(),
        })
        .await
        .expect("B");
    let ta = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: a.client_id.clone(),
            expected_revision: a.revision,
            idempotency_key: "issue-sub-a".into(),
        })
        .await
        .expect("A credential");
    a.revision = ta.revision;
    let tb = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: b.client_id.clone(),
            expected_revision: b.revision,
            idempotency_key: "issue-sub-b".into(),
        })
        .await
        .expect("B credential");
    b.revision = tb.revision;
    let mut ca = AlcomdClient::connect(config.clone().with_mcp_credential(ta.credential.clone()))
        .await
        .expect("actual A auth");
    let mut cb = AlcomdClient::connect(config.clone().with_mcp_credential(tb.credential.clone()))
        .await
        .expect("actual B auth");
    assert!(
        matches!(ca.mcp_transport_snapshot(Default::default()).await,Err(alcomd_client::ClientError::Remote(e)) if e.code=="permission_denied")
    );
    let mut observer = AlcomdClient::connect(config.clone().with_transport_observer())
        .await
        .expect("restricted reporter hello");
    assert!(!observer.supports_capability(CAPABILITY_PROJECTS_READ_V1));
    assert!(observer.supports_capability(CAPABILITY_MCP_TRANSPORT_REPORT_V1));
    assert!(
        matches!(observer.system_status().await,Err(alcomd_client::ClientError::Remote(e)) if e.code=="permission_denied")
    );
    assert!(
        observer
            .mcp_transport_snapshot(Default::default())
            .await
            .is_err()
    );
    let c = uuid::Uuid::new_v4().to_string();
    let r1 = uuid::Uuid::new_v4().to_string();
    let r2 = uuid::Uuid::new_v4().to_string();
    let report = McpTransportReport {
        sequence: 1,
        transport: McpTransportKind::Http,
        coverage_incomplete: false,
        connections: vec![McpTransportConnectionInput {
            local_ref: c.clone(),
            stage: McpTransportConnectionStage::Active,
        }],
        requests: [r1, r2]
            .into_iter()
            .map(|local_ref| McpTransportRequestInput {
                local_ref,
                connection_ref: c.clone(),
                method: McpTransportMethod::CallTool,
                stage: McpTransportRequestStage::Executing,
            })
            .collect(),
        subscription_streams: vec![],
    };
    assert!(owner.mcp_transport_report(report.clone()).await.is_err());
    let tickets = observer
        .mcp_transport_report(report.clone())
        .await
        .expect("full report");
    assert_eq!(tickets.request_bindings.len(), 2);
    assert!(
        matches!(observer.mcp_transport_report(report).await,Err(alcomd_client::ClientError::Remote(e)) if e.code=="revision_conflict")
    );
    assert!(
        owner
            .mcp_transport_bind_request(McpTransportBindParams {
                ticket: tickets.request_bindings[0].ticket.clone()
            })
            .await
            .is_err()
    );
    for (client, binding) in [
        (&mut ca, &tickets.request_bindings[0]),
        (&mut cb, &tickets.request_bindings[1]),
    ] {
        assert!(
            client
                .mcp_transport_bind_request(McpTransportBindParams {
                    ticket: binding.ticket.clone()
                })
                .await
                .expect("actual independent auth binds")
                .bound
        );
    }
    assert!(
        cb.mcp_transport_bind_request(McpTransportBindParams {
            ticket: tickets.request_bindings[0].ticket.clone()
        })
        .await
        .is_err()
    );
    let snapshot = owner
        .mcp_transport_snapshot(Default::default())
        .await
        .expect("bound projection");
    let requests = &snapshot.requests.as_ref().expect("request group").items;
    assert_eq!(requests.len(), 2);
    assert!(
        requests
            .iter()
            .any(|r| r.client_id.as_deref() == Some(&a.client_id))
    );
    assert!(
        requests
            .iter()
            .any(|r| r.client_id.as_deref() == Some(&b.client_id))
    );
    let filtered = owner
        .mcp_transport_snapshot(McpTransportQuery {
            client_id: Some(a.client_id.clone()),
            limit: Some(1),
        })
        .await
        .expect("actual client filter");
    assert_eq!(filtered.requests.expect("filtered").total_tracked, 1);
    assert_eq!(
        filtered
            .connections
            .expect("shared socket not owner")
            .total_tracked,
        1
    );
    let raw = serde_json::to_string(&snapshot).expect("safe projection");
    for binding in tickets.request_bindings {
        assert!(!raw.contains(&binding.ticket));
    }
    drop(observer);
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if owner
                .mcp_transport_snapshot(Default::default())
                .await
                .expect("disconnect")
                .observers
                .is_empty()
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("forget on exact reporter close");

    let http_fixture = if http {
        Some(HttpFixture::start(&core).await)
    } else {
        None
    };
    let url = http_fixture.as_ref().map(|h| h.url.as_str());
    let sa = sdk(&ta.credential, url, &core).await;
    let sb = sdk(&tb.credential, url, &core).await;
    let initial = ca
        .mcp_tools_catalog(Default::default())
        .await
        .expect("published real catalog");
    assert!(
        initial
            .tools
            .iter()
            .any(|tool| tool.name == "alcomd_get_project")
    );
    assert_eq!(initial.catalog_fingerprint.len(), 64);
    assert!(
        sa.listen(SubscriptionFilter::builder().prompts_list_changed().build())
            .await
            .is_err()
    );
    let requested = SubscriptionFilter::builder()
        .tools_list_changed()
        .prompts_list_changed()
        .build();
    let mut sub = sa
        .listen(requested)
        .await
        .expect("SDK validates ack first and correct subscriptionId");
    assert_eq!(sub.acknowledged().tools_list_changed, Some(true));
    assert_ne!(sub.acknowledged().prompts_list_changed, Some(true));
    let mut sub_b = sb
        .listen(SubscriptionFilter::builder().tools_list_changed().build())
        .await
        .expect("independent B stream");
    tokio::time::timeout(Duration::from_secs(7), async {
        loop {
            let snapshot = owner
                .mcp_transport_snapshot(Default::default())
                .await
                .expect("real adapter observation");
            let streams = snapshot.subscription_streams.expect("stream read group");
            if streams
                .items
                .iter()
                .any(|s| s.client_id.as_deref() == Some(&a.client_id))
                && streams
                    .items
                    .iter()
                    .any(|s| s.client_id.as_deref() == Some(&b.client_id))
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("real STDIO/HTTP reporter and authenticated stream binding");
    // A long HTTP idle stream must survive the former ten-second total-body limit.
    let idle = if http {
        Duration::from_secs(11)
    } else {
        Duration::from_millis(1200)
    };
    assert!(
        tokio::time::timeout(idle, sub.next()).await.is_err(),
        "unchanged catalogue must stay open without spurious invalidation"
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(1200), sub_b.next())
            .await
            .is_err(),
        "the other client's catalogue is also unchanged"
    );
    let current = ca
        .mcp_tools_catalog(Default::default())
        .await
        .expect("current catalogue");
    assert_eq!(current.catalog_fingerprint, initial.catalog_fingerprint);
    sub_b
        .cancel()
        .await
        .expect("only B protocol subscription cancelled");
    let rejected = sb
        .listen(SubscriptionFilter::builder().tools_list_changed().build())
        .await
        .expect("new B stream after cancel");
    drop(rejected);
    owner
        .mcp_client_revoke(McpClientMutationParams {
            client_id: a.client_id.clone(),
            expected_revision: a.revision,
            idempotency_key: "revoke-sub-a".into(),
        })
        .await
        .expect("revoke current A");
    let revoked = tokio::time::timeout(Duration::from_secs(4), sub.next())
        .await
        .expect("revoked stream stops");
    assert!(
        !matches!(revoked, Ok(Some(_))),
        "revocation is not a false catalogue notification"
    );
    stop(sa).await;
    stop(sb).await;
    if let Some(http) = http_fixture {
        http.finish().await;
    }
    drop(ca);
    drop(cb);
    drop(owner);
    daemon.abort();
    let _ = daemon.await;
    let db = rusqlite::Connection::open_with_flags(
        data_root.join("state.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .expect("read-only evidence");
    assert_eq!(
        db.pragma_query_value(None, "user_version", |r| r.get::<_, u32>(0))
            .expect("current database schema"),
        alcomd_store::CURRENT_DATA_SCHEMA
    );
    let persisted: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name LIKE 'mcp_transport%'",
            [],
            |r| r.get(0),
        )
        .expect("no persistent observation tables");
    assert_eq!(persisted, 0);
}
