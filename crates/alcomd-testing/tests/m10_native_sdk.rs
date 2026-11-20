//! Native SDK consumers sharing one owned, disposable Core installation state.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use alcomd_client::{AlcomdClient, ClientConfig, EventSubscription};
use alcomd_platform::{DataConfig, IpcConfig, IpcStream};
use alcomd_protocol::{
    BackupsListParams, CAPABILITY_PACKAGES_APPLY_V1, EventNotification, EventsSubscribeParams,
    ExtensionsListParams, METHOD_PACKAGES_APPLY_PLAN, METHOD_SYSTEM_HELLO, OperationState,
    PackageApplyPlanParams, PackagePlanRemoveParams, RPC_VERSION, RequestEnvelope, Response,
    decode_frame_length, encode_frame,
};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::oneshot;

const PACKAGE: &str = "com.example.m10-sdk";
const NODE_PACKAGE_CONSUMER_ENV: &str = "ALCOMD_M10_NODE_PACKAGE_CONSUMER";

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn native_sdk_projects_packages_tasks_and_live_events_agree_on_real_core() {
    let root = Fixture::new();
    let (ipc, config) = isolated_ipc(&root.path);
    let data = root.path.join("data");
    fs::create_dir(&data).expect("isolated state directory");
    let (stop, shutdown) = oneshot::channel::<()>();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc.clone(),
        DataConfig::isolated(data),
        async {
            let _ = shutdown.await;
        },
    )
    .await
    .expect("own the test endpoint before any client connects");

    let mut rust = AlcomdClient::connect(config.clone().with_client_name("m10-rust-consumer"))
        .await
        .expect("connect using the public Rust SDK");
    assert_eq!(rust.hello_result().rpc_version, RPC_VERSION);
    assert_eq!(
        rust.hello_result().data_schema,
        Some(alcomd_store::CURRENT_DATA_SCHEMA)
    );
    assert_eq!(
        rust.hello_result().config_schema,
        Some(alcomd_protocol::CONFIG_SCHEMA_VERSION)
    );

    let mut identities = Vec::new();
    for name in ["RustFixture", "NodeFixture"] {
        let project = root.path.join(name);
        create_project(&project);
        let project = rust
            .project_register(
                project.to_string_lossy().into_owned(),
                format!("register-{name}"),
            )
            .await
            .expect("register an owned project")
            .project;
        identities.push((
            project.project_id.expect("registered project ID"),
            project.revision.expect("registered project revision"),
        ));
    }
    let first = rust.projects_list(None, Some(1)).await.expect("first page");
    let second = rust
        .projects_list(first.next_cursor, Some(1))
        .await
        .expect("second page");
    assert_eq!(first.projects.len(), 1);
    assert_eq!(second.projects.len(), 1);
    assert!(second.next_cursor.is_none());
    let mut observed = [
        first.projects[0].project_id.as_deref(),
        second.projects[0].project_id.as_deref(),
    ];
    let mut expected = [
        Some(identities[0].0.as_str()),
        Some(identities[1].0.as_str()),
    ];
    observed.sort_unstable();
    expected.sort_unstable();
    assert_eq!(observed, expected);

    let (rust_id, rust_revision) = &identities[0];
    let plan = rust
        .package_plan_remove(PackagePlanRemoveParams {
            project_id: rust_id.clone(),
            expected_revision: *rust_revision,
            package_id: PACKAGE.into(),
        })
        .await
        .expect("create a reviewed package Plan");
    let params = PackageApplyPlanParams {
        plan_id: plan.plan_id,
        expected_revision: plan.project_revision,
        idempotency_key: "m10-rust-recover-apply".into(),
    };
    let mut events = EventSubscription::connect(config.clone(), EventsSubscribeParams::default())
        .await
        .expect("dedicated Rust event stream");
    let mut lost = send_apply_without_reading(&ipc, &params).await;
    let operation_id = wait_for_completed(&mut events).await;
    // The event proves the write was accepted. Drop the command stream with its response unread.
    lost.shutdown().await.expect("close the write-reply stream");
    drop(lost);
    let replay = rust
        .package_apply_plan(params)
        .await
        .expect("recover original Apply");
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, operation_id);
    let operation = rust
        .operation_get(operation_id)
        .await
        .expect("query authoritative task");
    assert_eq!(operation.state, OperationState::Succeeded);
    assert!(
        !root
            .path
            .join("RustFixture/Packages")
            .join(PACKAGE)
            .exists()
    );
    drop(events);

    let settings = rust.settings_get().await.expect("settings snapshot");
    assert_eq!(
        settings.config_schema,
        alcomd_protocol::CONFIG_SCHEMA_VERSION
    );
    assert!(
        rust.backups_list(BackupsListParams::default())
            .await
            .expect("backup catalog")
            .backups
            .is_empty()
    );
    assert!(
        rust.extensions_list(ExtensionsListParams {
            cursor: None,
            limit: Some(1)
        })
        .await
        .expect("extension management catalog")
        .extensions
        .is_empty()
    );

    if let Some(consumer) = std::env::var_os(NODE_PACKAGE_CONSUMER_ENV) {
        let consumer = PathBuf::from(consumer);
        assert!(
            consumer.is_absolute() && consumer.is_dir(),
            "external npm consumer directory"
        );
        run_node_consumer(&root.path, &consumer, &identities).await;
        assert!(
            !root
                .path
                .join("NodeFixture/Packages")
                .join(PACKAGE)
                .exists()
        );
    }

    drop(rust);
    stop.send(()).expect("request owned server shutdown");
    tokio::time::timeout(Duration::from_secs(5), daemon)
        .await
        .expect("owned daemon shutdown deadline")
        .expect("join owned daemon")
        .expect("owned daemon result");
}

fn create_project(project: &Path) {
    let package = project.join("Packages").join(PACKAGE);
    fs::create_dir_all(project.join("ProjectSettings")).expect("project settings");
    fs::create_dir_all(&package).expect("embedded package");
    fs::write(
        project.join("ProjectSettings/ProjectVersion.txt"),
        "m_EditorVersion: 2022.3.22f1\n",
    )
    .expect("fixture Unity version");
    fs::write(
        project.join("Packages/manifest.json"),
        br#"{"dependencies":{}}"#,
    )
    .expect("UPM manifest");
    fs::write(
        project.join("Packages/vpm-manifest.json"),
        serde_json::to_vec(&json!({
            "dependencies": { PACKAGE: { "version": "1.0.0" } },
            "locked": { PACKAGE: { "version": "1.0.0", "dependencies": {} } }
        }))
        .expect("VPM manifest JSON"),
    )
    .expect("write VPM manifest");
    fs::write(
        package.join("package.json"),
        serde_json::to_vec(&json!({
            "name": PACKAGE, "version": "1.0.0"
        }))
        .expect("package JSON"),
    )
    .expect("write embedded package manifest");
}

async fn write_request(stream: &mut IpcStream, id: &str, method: &str, params: Value) {
    let request = RequestEnvelope {
        id: id.into(),
        method: method.into(),
        params,
    };
    let bytes = serde_json::to_vec(&request).expect("request JSON");
    stream
        .write_all(&encode_frame(&bytes).expect("request frame"))
        .await
        .expect("write request");
    stream.flush().await.expect("flush request");
}

async fn send_apply_without_reading(ipc: &IpcConfig, params: &PackageApplyPlanParams) -> IpcStream {
    let exchange = async {
        let mut stream = alcomd_platform::connect(ipc)
            .await
            .expect("own raw native connection");
        write_request(
            &mut stream,
            "hello",
            METHOD_SYSTEM_HELLO,
            json!({
                "rpcVersion": RPC_VERSION,
                "client": { "name": "m10-lost-reply", "version": "1", "instanceId": "lost" },
                "capabilities": [CAPABILITY_PACKAGES_APPLY_V1]
            }),
        )
        .await;
        let mut prefix = [0; 4];
        stream.read_exact(&mut prefix).await.expect("hello prefix");
        let mut payload = vec![0; decode_frame_length(prefix).expect("hello frame length")];
        stream
            .read_exact(&mut payload)
            .await
            .expect("hello payload");
        let response: Response<Value> = serde_json::from_slice(&payload).expect("hello response");
        assert!(matches!(response, Response::Success(response) if response.id == "hello"));
        write_request(
            &mut stream,
            "discarded-apply",
            METHOD_PACKAGES_APPLY_PLAN,
            serde_json::to_value(params).expect("approved Apply DTO"),
        )
        .await;
        stream
    };
    tokio::time::timeout(Duration::from_secs(5), exchange)
        .await
        .expect("raw Apply deadline")
}

async fn wait_for_completed(subscription: &mut EventSubscription) -> String {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let notification = subscription
                .next_notification()
                .await
                .expect("live task event");
            let EventNotification::Batch(batch) = notification else {
                panic!("stream closed before operation completion");
            };
            if let Some(event) = batch
                .events
                .into_iter()
                .find(|event| event.kind == "operation.completed")
            {
                return event.aggregate_id;
            }
        }
    })
    .await
    .expect("Core emits the accepted operation's completion")
}

fn isolated_ipc(root: &Path) -> (IpcConfig, ClientConfig) {
    let config = ClientConfig::default().without_daemon_start();
    #[cfg(unix)]
    {
        let runtime = root.join("alcomd");
        (
            IpcConfig::isolated(runtime.clone()),
            config.with_runtime_directory(runtime),
        )
    }
    #[cfg(windows)]
    {
        let _ = root;
        (IpcConfig::default(), config)
    }
}

async fn run_node_consumer(root: &Path, consumer: &Path, identities: &[(String, u64)]) {
    const SCRIPT: &str = r#"
        import { strict as assert } from 'node:assert';
        import { connect } from '@alcomd/sdk/node';
        const deadline = setTimeout(() => process.exit(124), 45_000);
        const input = JSON.parse(process.env.ALCOMD_M10_CONSUMER_INPUT);
        const client = await connect();
        let stream;
        try {
            assert.equal(client.hello.rpcVersion, 1);
            const first = await client.request('projects.list', {limit: 1});
            const second = await client.request('projects.list', {limit: 1, cursor: first.nextCursor});
            assert.deepEqual([...first.projects, ...second.projects].map(p => p.projectId).sort(), input.ids.sort());
            assert.equal(second.nextCursor, undefined);
            const [projectId, expectedRevision] = input.node;
            const own = await client.request('projects.get', {projectId});
            assert.equal(own.project.projectId, projectId);
            const rejectedId = 'm10-invalid-project-id';
            await assert.rejects(client.request('projects.get', {projectId: rejectedId}), error =>
                error.kind === 'remote' && error.code === 'invalid_request' && !error.message.includes(rejectedId));
            const plan = await client.request('packages.planRemove', {projectId, expectedRevision, packageId: input.packageId});
            stream = await client.subscribeEvents();
            const params = {planId: plan.planId, expectedRevision: plan.projectRevision, idempotencyKey: 'm10-node-apply'};
            const accepted = await client.request('packages.applyPlan', params);
            let completed = false;
            for await (const event of stream.notifications) {
                assert.equal(event.method, 'events.batch');
                if (event.params.events.some(item => item.kind === 'operation.completed' && item.aggregateId === accepted.operationId)) {
                    completed = true;
                    break;
                }
            }
            assert.equal(completed, true);
            const operation = await client.request('operations.get', {operationId: accepted.operationId});
            assert.equal(operation.state, 'succeeded');
            const replay = await client.request('packages.applyPlan', params);
            assert.deepEqual(replay, {...accepted, replayed: true});
            assert.equal((await client.request('settings.get', {})).configSchema, client.hello.configSchema);
            assert.deepEqual((await client.request('backups.list', {})).backups, []);
            assert.deepEqual((await client.request('extensions.list', {})).extensions, []);
        } finally {
            stream?.close();
            client.close();
            clearTimeout(deadline);
        }
    "#;

    let mut command = Command::new("node");
    command
        .args(["--input-type=module", "--eval", SCRIPT])
        .current_dir(consumer)
        .env(
            "ALCOMD_M10_CONSUMER_INPUT",
            json!({
                "ids": [&identities[0].0, &identities[1].0],
                "node": &identities[1],
                "packageId": PACKAGE,
            })
            .to_string(),
        )
        .stdin(Stdio::null())
        .env_remove("NODE_OPTIONS")
        .env_remove("ALCOMD_MCP_CLIENT_CREDENTIAL");
    #[cfg(target_os = "linux")]
    command
        .env("XDG_RUNTIME_DIR", root)
        .env("XDG_DATA_HOME", root.join("data"));
    #[cfg(target_os = "macos")]
    command.env("TMPDIR", root);
    #[cfg(windows)]
    let _ = root;
    let output = tokio::task::spawn_blocking(move || command.output())
        .await
        .expect("join Node consumer")
        .expect("start independent Node consumer");
    assert!(
        output.status.success(),
        "Node SDK consumer: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

struct Fixture {
    base: PathBuf,
    path: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        #[cfg(target_os = "macos")]
        let base = PathBuf::from("/private/tmp");
        #[cfg(not(target_os = "macos"))]
        let base = std::env::temp_dir()
            .canonicalize()
            .expect("canonical temp root");
        let path = base.join(format!("ac-m10-{}", uuid::Uuid::new_v4().simple()));
        fs::create_dir(&path).expect("unique fixture root");
        Self { base, path }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if self
            .path
            .canonicalize()
            .is_ok_and(|path| path == self.path && path.parent() == Some(&self.base))
        {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
