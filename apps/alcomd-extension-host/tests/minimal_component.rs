use std::io::{BufReader, BufWriter, Write};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use alcomd_extensions::{
    HOST_PROTOCOL_VERSION, HostMessage, HostMessageBody, RuntimeLimits, invocation_context_id,
    read_host_message, write_host_message,
};
use serde_json::json;

#[test]
fn minimal_component_activates_revokes_and_deactivates() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "alcomd-m6-host-fixture-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).expect("create fixture directory");
    let component = directory.join("extension.wasm");
    std::fs::write(
        &component,
        include_bytes!("../../../crates/alcomd-testing/fixtures/m6/minimal-extension-v1.wasm"),
    )
    .expect("write fixture");

    let extension_id = "dev.alcomd.fixture";
    let epoch = "10000000-0000-4000-8000-000000000001";
    let instance = "20000000-0000-4000-8000-000000000002";
    let lease = "30000000-0000-4000-8000-000000000003";
    let nonce = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let mut child = Command::new(env!("CARGO_BIN_EXE_alcomd-extension-host"))
        .arg("--extension")
        .arg(extension_id)
        .arg("--component")
        .arg(&component)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn host");
    let mut input = BufWriter::new(child.stdin.take().expect("stdin"));
    let mut output = BufReader::new(child.stdout.take().expect("stdout"));

    send(
        &mut input,
        epoch,
        instance,
        1,
        HostMessageBody::Bootstrap {
            nonce: nonce.to_owned(),
            lease_id: lease.to_owned(),
            extension_id: extension_id.to_owned(),
            api_world: "alcomd:extension/extension-v1@1.0.0".to_owned(),
            limits: RuntimeLimits::default(),
        },
    );
    let ready = read_host_message(&mut output).expect("ready");
    assert_eq!(ready.sequence, 1);
    assert!(matches!(
        ready.body,
        HostMessageBody::Ready { nonce: value } if value == nonce
    ));

    send(
        &mut input,
        epoch,
        instance,
        2,
        HostMessageBody::InvokeExport {
            request_id: "activate-1".to_owned(),
            invocation_context_id: invocation_context_id(),
            export: "activate".to_owned(),
            input: json!({
                "extensionId": extension_id,
                "instanceId": instance,
                "apiMajor": 1,
                "lifecycleGeneration": 1,
                "kind": "background"
            }),
        },
    );
    assert!(matches!(
        read_host_message(&mut output).expect("activation result").body,
        HostMessageBody::ExportResult { request_id, result: Some(_), error: None }
            if request_id == "activate-1"
    ));

    send(
        &mut input,
        epoch,
        instance,
        3,
        HostMessageBody::InvokeExport {
            request_id: "ui-open-1".to_owned(),
            invocation_context_id: invocation_context_id(),
            export: "ui.open".to_owned(),
            input: json!({"sessionId": "00000000-0000-4000-8000-000000000005", "locale": "en-US"}),
        },
    );
    let ui_result = read_host_message(&mut output).expect("UI open result");
    match ui_result.body {
        HostMessageBody::ExportResult {
            request_id,
            result: Some(value),
            error: None,
        } => {
            assert_eq!(request_id, "ui-open-1");
            assert_eq!(value["protocol"], json!("portable-v1"));
            assert_eq!(value["nodes"], json!([]));
        }
        other => panic!("unexpected UI result: {other:?}"),
    }

    send(
        &mut input,
        epoch,
        instance,
        4,
        HostMessageBody::RevokeLease {
            lease_id: lease.to_owned(),
        },
    );
    send(
        &mut input,
        epoch,
        instance,
        5,
        HostMessageBody::InvokeExport {
            request_id: "deactivate-1".to_owned(),
            invocation_context_id: invocation_context_id(),
            export: "deactivate".to_owned(),
            input: json!({"reason": "permission_revoked"}),
        },
    );
    assert!(matches!(
        read_host_message(&mut output).expect("deactivation result").body,
        HostMessageBody::ExportResult { request_id, result: Some(_), error: None }
            if request_id == "deactivate-1"
    ));
    send(&mut input, epoch, instance, 6, HostMessageBody::Shutdown);
    drop(input);
    assert!(child.wait().expect("wait").success());
    std::fs::remove_dir_all(directory).expect("cleanup fixture");
}

#[test]
fn portable_ui_component_lifts_a_valid_document() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "alcomd-m7-host-fixture-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).expect("create fixture directory");
    let component = directory.join("extension.wasm");
    std::fs::write(
        &component,
        include_bytes!("../../../crates/alcomd-testing/fixtures/m7/portable-ui-extension-v1.wasm"),
    )
    .expect("write fixture");

    let extension_id = "dev.alcomd.portable-ui";
    let epoch = "70000000-0000-4000-8000-000000000007";
    let instance = "80000000-0000-4000-8000-000000000008";
    let lease = "90000000-0000-4000-8000-000000000009";
    let nonce = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    let mut child = Command::new(env!("CARGO_BIN_EXE_alcomd-extension-host"))
        .args(["--extension", extension_id, "--component"])
        .arg(&component)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn host");
    let mut input = BufWriter::new(child.stdin.take().expect("stdin"));
    let mut output = BufReader::new(child.stdout.take().expect("stdout"));
    send(
        &mut input,
        epoch,
        instance,
        1,
        HostMessageBody::Bootstrap {
            nonce: nonce.to_owned(),
            lease_id: lease.to_owned(),
            extension_id: extension_id.to_owned(),
            api_world: "alcomd:extension/extension-v1@1.0.0".to_owned(),
            limits: RuntimeLimits::default(),
        },
    );
    assert!(matches!(
        read_host_message(&mut output).expect("ready").body,
        HostMessageBody::Ready { nonce: value } if value == nonce
    ));
    send(
        &mut input,
        epoch,
        instance,
        2,
        HostMessageBody::InvokeExport {
            request_id: "activate-ui".to_owned(),
            invocation_context_id: invocation_context_id(),
            export: "activate".to_owned(),
            input: json!({
                "extensionId": extension_id,
                "instanceId": instance,
                "apiMajor": 1,
                "lifecycleGeneration": 1,
                "kind": "interactive_ui"
            }),
        },
    );
    assert!(matches!(
        read_host_message(&mut output).expect("activation").body,
        HostMessageBody::ExportResult {
            result: Some(_),
            error: None,
            ..
        }
    ));
    send(
        &mut input,
        epoch,
        instance,
        3,
        HostMessageBody::InvokeExport {
            request_id: "open-ui".to_owned(),
            invocation_context_id: invocation_context_id(),
            export: "ui.open".to_owned(),
            input: json!({
                "sessionId": "00000000-0000-4000-8000-000000000010",
                "locale": "en-US"
            }),
        },
    );
    let capability = read_host_message(&mut output).expect("render capability call");
    let HostMessageBody::CapabilityCall {
        call_id,
        capability,
        invocation_context_id,
        ..
    } = capability.body
    else {
        panic!("expected render capability call");
    };
    assert_eq!(capability, "host-data.set");
    assert!(invocation_context_id.starts_with("ictx_"));
    send(
        &mut input,
        epoch,
        instance,
        4,
        HostMessageBody::CapabilityResult {
            call_id,
            result: None,
            error: Some(alcomd_extensions::HostStableError {
                code: "extension_permission_denied".to_owned(),
                diagnostic_id: None,
            }),
        },
    );
    let result = read_host_message(&mut output).expect("UI result");
    let HostMessageBody::ExportResult {
        result: Some(document),
        error: None,
        ..
    } = result.body
    else {
        panic!("unexpected UI result");
    };
    assert_eq!(document["protocol"], "portable-v1");
    assert_eq!(document["title"], "Fixture UI");
    assert_eq!(document["nodes"].as_array().expect("nodes").len(), 2);
    assert_eq!(document["nodes"][0]["kind"], "page");
    assert_eq!(document["nodes"][0]["nodeId"], "root");
    assert_eq!(document["nodes"][0]["payload"]["title"], "Fixture");
    assert_eq!(document["nodes"][1]["kind"], "button");
    assert_eq!(document["nodes"][1]["parentId"], "root");
    assert_eq!(document["nodes"][1]["payload"]["actionId"], "refresh");

    send(&mut input, epoch, instance, 5, HostMessageBody::Shutdown);
    drop(input);
    assert!(child.wait().expect("wait").success());
    std::fs::remove_dir_all(directory).expect("cleanup fixture");
}

#[test]
fn trap_and_infinite_loop_are_isolated_with_stable_errors() {
    for (name, component, expected) in [
        (
            "trap",
            include_bytes!("../../../crates/alcomd-testing/fixtures/m6/trap-extension-v1.wasm")
                .as_slice(),
            "extension_crashed",
        ),
        (
            "loop",
            include_bytes!("../../../crates/alcomd-testing/fixtures/m6/loop-extension-v1.wasm")
                .as_slice(),
            "extension_resource_limit",
        ),
    ] {
        assert_failed_activation(name, component, expected);
    }
}

fn assert_failed_activation(name: &str, component_bytes: &[u8], expected: &str) {
    let directory = std::env::temp_dir().join(format!(
        "alcomd-m6-host-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).expect("fixture directory");
    let component = directory.join("extension.wasm");
    std::fs::write(&component, component_bytes).expect("fixture component");
    let epoch = "40000000-0000-4000-8000-000000000004";
    let instance = "50000000-0000-4000-8000-000000000005";
    let lease = "60000000-0000-4000-8000-000000000006";
    let nonce = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let mut child = Command::new(env!("CARGO_BIN_EXE_alcomd-extension-host"))
        .args(["--extension", "dev.alcomd.fixture", "--component"])
        .arg(&component)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn host");
    let mut input = BufWriter::new(child.stdin.take().expect("stdin"));
    let mut output = BufReader::new(child.stdout.take().expect("stdout"));
    send(
        &mut input,
        epoch,
        instance,
        1,
        HostMessageBody::Bootstrap {
            nonce: nonce.to_owned(),
            lease_id: lease.to_owned(),
            extension_id: "dev.alcomd.fixture".to_owned(),
            api_world: "alcomd:extension/extension-v1@1.0.0".to_owned(),
            limits: RuntimeLimits::default(),
        },
    );
    assert!(matches!(
        read_host_message(&mut output).expect("ready").body,
        HostMessageBody::Ready { nonce: value } if value == nonce
    ));
    send(
        &mut input,
        epoch,
        instance,
        2,
        HostMessageBody::InvokeExport {
            request_id: format!("{name}-activate"),
            invocation_context_id: invocation_context_id(),
            export: "activate".to_owned(),
            input: json!({
                "extensionId": "dev.alcomd.fixture",
                "instanceId": instance,
                "apiMajor": 1,
                "lifecycleGeneration": 1,
                "kind": "background"
            }),
        },
    );
    let result = read_host_message(&mut output)
        .unwrap_or_else(|error| panic!("{name} failed activation response: {error:?}"));
    assert!(matches!(
        result.body,
        HostMessageBody::ExportResult { result: None, error: Some(error), .. }
            if error.code == expected
    ));
    send(&mut input, epoch, instance, 3, HostMessageBody::Shutdown);
    drop(input);
    assert!(child.wait().expect("wait").success());
    std::fs::remove_dir_all(directory).expect("cleanup");
}

fn send(
    input: &mut BufWriter<std::process::ChildStdin>,
    epoch: &str,
    instance: &str,
    sequence: u64,
    body: HostMessageBody,
) {
    let encoded = serde_json::to_string(&HostMessage {
        protocol_version: HOST_PROTOCOL_VERSION,
        daemon_epoch: epoch.to_owned(),
        instance_id: instance.to_owned(),
        lifecycle_generation: 1,
        sequence,
        body: body.clone(),
    })
    .expect("json");
    let _: HostMessage = serde_json::from_str(&encoded).expect("round trip");
    write_host_message(
        input,
        &HostMessage {
            protocol_version: HOST_PROTOCOL_VERSION,
            daemon_epoch: epoch.to_owned(),
            instance_id: instance.to_owned(),
            lifecycle_generation: 1,
            sequence,
            body,
        },
    )
    .expect("send");
    input.flush().expect("flush");
}
