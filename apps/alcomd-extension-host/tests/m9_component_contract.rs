wasmtime::component::bindgen!({
    path: "../../specs/extensions/wit/extension-v1",
    world: "background-integration-v1",
});

#[test]
fn background_world_compiles_against_the_public_types() {
    use alcomd::extension::host_local_ipc::{Connection, IpcError};
    use alcomd::extension::host_unity_sessions::{Observation, SessionError, UnitySession};

    let session = UnitySession {
        project_id: "73d8e373-5668-4f02-ad9b-94b19a1843be".to_owned(),
        started_at_ms: 86_400_000,
    };
    let observation = Observation {
        current_session: Some(session),
        editor_count: 2,
    };
    let connection = Connection {
        channel: "opaque-channel".to_owned(),
        client_process_id: 23,
    };

    assert_eq!(
        observation.current_session.expect("session").started_at_ms,
        86_400_000
    );
    assert_eq!(observation.editor_count, 2);
    assert_eq!(connection.client_process_id, 23);
    assert_ne!(SessionError::Unavailable, SessionError::PermissionDenied);
    assert_ne!(IpcError::TimedOut, IpcError::PeerClosed);
}

#[test]
fn local_ipc_wire_carries_the_full_binary_budget_and_rejects_invalid_encoding() {
    use alcomd_extensions::{
        MAX_LOCAL_IPC_BYTES, MAX_WIT_VALUE_BYTES, decode_local_ipc_bytes, encode_local_ipc_bytes,
    };

    let payload = vec![255; MAX_LOCAL_IPC_BYTES];
    let encoded = encode_local_ipc_bytes(&payload).expect("64 KiB fits");
    assert!(
        serde_json::to_vec(&serde_json::json!({"bytes": encoded}))
            .expect("wire")
            .len()
            < MAX_WIT_VALUE_BYTES
    );
    assert_eq!(decode_local_ipc_bytes(&encoded), Some(payload));
    assert_eq!(decode_local_ipc_bytes("2X"), None);
    assert_eq!(decode_local_ipc_bytes("a"), None);
    assert_eq!(decode_local_ipc_bytes(&(encoded + "00")), None);
}

#[test]
fn background_guest_crosses_the_private_host_channel_with_opaque_handles() {
    use alcomd_extensions::{
        BACKGROUND_INTEGRATION_WORLD, HOST_PROTOCOL_VERSION, HostMessage, HostMessageBody,
        RuntimeLimits, invocation_context_id, read_host_message, write_host_message,
    };
    use serde_json::json;
    use std::process::{Command, Stdio};

    let root =
        std::env::temp_dir().join(format!("alcomd-background-host-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).expect("owned fixture directory");
    let component = root.join("extension.wasm");
    std::fs::copy(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/alcomd-testing/fixtures/m9/background-integration-v1.wasm"),
        &component,
    )
    .expect("compiled background integration fixture");
    let mut child = Command::new(env!("CARGO_BIN_EXE_alcomd-extension-host"))
        .args(["--extension", "dev.example.background", "--component"])
        .arg(&component)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Host");
    let mut input = child.stdin.take().expect("Host stdin");
    let mut output = child.stdout.take().expect("Host stdout");
    let epoch = "10000000-0000-4000-8000-000000000001";
    let instance = "20000000-0000-4000-8000-000000000002";
    let lease = "30000000-0000-4000-8000-000000000003";
    let mut sequence = 0;
    let mut send = |body| {
        sequence += 1;
        write_host_message(
            &mut input,
            &HostMessage {
                protocol_version: HOST_PROTOCOL_VERSION,
                daemon_epoch: epoch.into(),
                instance_id: instance.into(),
                lifecycle_generation: 1,
                sequence,
                body,
            },
        )
        .expect("send Host message");
    };
    send(HostMessageBody::Bootstrap {
        nonce: "a".repeat(64),
        lease_id: lease.into(),
        extension_id: "dev.example.background".into(),
        api_world: BACKGROUND_INTEGRATION_WORLD.into(),
        limits: RuntimeLimits::default(),
    });
    assert!(matches!(
        read_host_message(&mut output).expect("ready").body,
        HostMessageBody::Ready { .. }
    ));
    let context = invocation_context_id();
    send(HostMessageBody::InvokeExport {
        request_id: "tick".into(),
        invocation_context_id: context.clone(),
        export: "background.tick".into(),
        input: json!({}),
    });

    for (expected_capability, expected_input, response) in [
        (
            "host-unity-sessions.current",
            json!({}),
            json!({"editorCount": 1, "currentSession": {"projectId": "73d8e373-5668-4f02-ad9b-94b19a1843be", "startedAtMs": 250}}),
        ),
        (
            "host-local-ipc.connect",
            json!({"targetId": "fixture"}),
            json!({"channel": "opaque", "clientProcessId": 23}),
        ),
        (
            "host-local-ipc.write",
            json!({"channel": "opaque", "bytes": "00ff"}),
            json!({}),
        ),
        (
            "host-local-ipc.read",
            json!({"channel": "opaque", "maxBytes": 2}),
            json!({"bytes": "2a07"}),
        ),
        (
            "host-local-ipc.close",
            json!({"channel": "opaque"}),
            json!({}),
        ),
    ] {
        let call = read_host_message(&mut output).expect("read capability call");
        let HostMessageBody::CapabilityCall {
            call_id,
            invocation_context_id,
            lease_id,
            capability,
            input,
        } = call.body
        else {
            panic!("expected capability {expected_capability}");
        };
        assert_eq!(invocation_context_id, context);
        assert_eq!(lease_id, lease);
        assert_eq!(capability, expected_capability);
        assert_eq!(input, expected_input);
        send(HostMessageBody::CapabilityResult {
            call_id,
            result: Some(response),
            error: None,
        });
    }
    let result = read_host_message(&mut output).expect("background result");
    assert!(matches!(
        result.body,
        HostMessageBody::ExportResult {
            result: Some(_),
            error: None,
            ..
        }
    ));
    send(HostMessageBody::Shutdown);
    drop(input);
    assert!(child.wait().expect("Host exits").success());
    std::fs::remove_file(component).expect("remove fixture file");
    std::fs::remove_dir(root).expect("remove fixture directory");
}
