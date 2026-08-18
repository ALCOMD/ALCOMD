use serde_json::{Value, json};

use alcomd_protocol::{
    ClientInfo, HelloParams, HelloResult, METHOD_SYSTEM_HELLO, METHOD_SYSTEM_STATUS, RPC_VERSION,
    RequestEnvelope, SuccessResponse, SystemStatusResult,
};

const SCHEMAS: [(&str, &str); 7] = [
    (
        "request-envelope",
        include_str!("../../../specs/rpc/request-envelope.schema.json"),
    ),
    (
        "response-envelope",
        include_str!("../../../specs/rpc/response-envelope.schema.json"),
    ),
    (
        "rpc-error",
        include_str!("../../../specs/rpc/rpc-error.schema.json"),
    ),
    (
        "system-hello.request",
        include_str!("../../../specs/rpc/system-hello.request.schema.json"),
    ),
    (
        "system-hello.response",
        include_str!("../../../specs/rpc/system-hello.response.schema.json"),
    ),
    (
        "system-status.request",
        include_str!("../../../specs/rpc/system-status.request.schema.json"),
    ),
    (
        "system-status.response",
        include_str!("../../../specs/rpc/system-status.response.schema.json"),
    ),
];

#[test]
fn all_m1_schemas_are_valid_json_schema_documents() {
    for (name, source) in SCHEMAS {
        let schema: Value = serde_json::from_str(source).unwrap_or_else(|error| {
            panic!("{name} must be valid JSON: {error}");
        });
        assert_eq!(
            schema["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
        assert!(
            schema["$id"]
                .as_str()
                .is_some_and(|id| id.contains("/rpc/v1/"))
        );
    }
}

#[test]
fn request_schema_freezes_m1_limits() {
    let schema = schema("request-envelope");
    assert_eq!(schema["additionalProperties"], false);
    assert_eq!(schema["properties"]["id"]["maxLength"], 64);
    assert_eq!(schema["properties"]["method"]["maxLength"], 128);
    assert_eq!(schema["properties"]["params"]["type"], "object");
}

#[test]
fn hello_schema_has_no_future_subsystem_versions() {
    let response = schema("system-hello.response");
    let properties = response["properties"]["result"]["properties"]
        .as_object()
        .expect("hello result properties");
    assert_eq!(
        properties.keys().cloned().collect::<Vec<_>>(),
        ["capabilities", "daemonVersion", "rpcVersion"]
    );
    assert_eq!(
        response["properties"]["result"]["additionalProperties"],
        true
    );
}

#[test]
fn m1_examples_match_the_frozen_json_shape() {
    let hello_request = RequestEnvelope {
        id: "hello-1".to_owned(),
        method: METHOD_SYSTEM_HELLO.to_owned(),
        params: serde_json::to_value(HelloParams {
            rpc_version: RPC_VERSION,
            client: ClientInfo {
                name: "alcomd-cli".to_owned(),
                version: "4.0.0-alpha.0".to_owned(),
                instance_id: "test-instance".to_owned(),
            },
            capabilities: Vec::new(),
        })
        .expect("serialize hello params"),
    };
    assert_eq!(
        serde_json::to_value(hello_request).expect("serialize hello request"),
        json!({
            "id": "hello-1",
            "method": "system.hello",
            "params": {
                "rpcVersion": 1,
                "client": {
                    "name": "alcomd-cli",
                    "version": "4.0.0-alpha.0",
                    "instanceId": "test-instance"
                },
                "capabilities": []
            }
        })
    );

    let hello_response = SuccessResponse {
        id: "hello-1".to_owned(),
        result: HelloResult::m1(),
    };
    assert_eq!(
        serde_json::to_value(hello_response).expect("serialize hello response"),
        json!({
            "id": "hello-1",
            "result": {
                "rpcVersion": 1,
                "daemonVersion": env!("CARGO_PKG_VERSION"),
                "capabilities": []
            }
        })
    );

    let status_request = RequestEnvelope {
        id: "status-1".to_owned(),
        method: METHOD_SYSTEM_STATUS.to_owned(),
        params: json!({}),
    };
    assert_eq!(status_request.method, "system.status");

    let status_response = SuccessResponse {
        id: "status-1".to_owned(),
        result: SystemStatusResult::ready(),
    };
    let value = serde_json::to_value(status_response).expect("serialize status response");
    assert_eq!(value["result"]["product"], "ALCOMD");
    assert_eq!(value["result"]["state"], "ready");
    assert!(value["result"].get("pid").is_none());
}

fn schema(name: &str) -> Value {
    let source = SCHEMAS
        .iter()
        .find_map(|(candidate, source)| (*candidate == name).then_some(*source))
        .expect("known schema");
    serde_json::from_str(source).expect("schema JSON")
}
