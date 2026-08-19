use serde_json::{Value, json};

use alcomd_protocol::{
    CAPABILITY_EVENTS_REPLAY_V1, CAPABILITY_OPERATIONS_V1, CAPABILITY_STATE_CHECK_V1, ClientInfo,
    EventsListParams, EventsListResult, HelloParams, HelloResult, MAX_PAGE_LIMIT,
    METHOD_EVENTS_LIST, METHOD_OPERATIONS_CANCEL, METHOD_OPERATIONS_GET, METHOD_OPERATIONS_LIST,
    METHOD_STATE_CHECK, METHOD_SYSTEM_HELLO, METHOD_SYSTEM_STATUS, OperationAccepted,
    OperationsCancelParams, OperationsListCursor, OperationsListParams, RPC_VERSION,
    RequestEnvelope, StateCheckParams, SuccessResponse, SystemStatusResult,
};

const SCHEMAS: [(&str, &str); 21] = [
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
    (
        "operation",
        include_str!("../../../specs/rpc/operation.schema.json"),
    ),
    (
        "event",
        include_str!("../../../specs/rpc/event.schema.json"),
    ),
    (
        "state-check.request",
        include_str!("../../../specs/rpc/state-check.request.schema.json"),
    ),
    (
        "state-check.response",
        include_str!("../../../specs/rpc/state-check.response.schema.json"),
    ),
    (
        "operations-get.request",
        include_str!("../../../specs/rpc/operations-get.request.schema.json"),
    ),
    (
        "operations-get.response",
        include_str!("../../../specs/rpc/operations-get.response.schema.json"),
    ),
    (
        "operations-list.request",
        include_str!("../../../specs/rpc/operations-list.request.schema.json"),
    ),
    (
        "operations-list.response",
        include_str!("../../../specs/rpc/operations-list.response.schema.json"),
    ),
    (
        "operations-cancel.request",
        include_str!("../../../specs/rpc/operations-cancel.request.schema.json"),
    ),
    (
        "operations-cancel.response",
        include_str!("../../../specs/rpc/operations-cancel.response.schema.json"),
    ),
    (
        "events-list.request",
        include_str!("../../../specs/rpc/events-list.request.schema.json"),
    ),
    (
        "events-list.response",
        include_str!("../../../specs/rpc/events-list.response.schema.json"),
    ),
    (
        "m3-project-repository",
        include_str!("../../../specs/rpc/m3-project-repository.schema.json"),
    ),
    (
        "m4-package-transaction",
        include_str!("../../../specs/rpc/m4-package-transaction.schema.json"),
    ),
];

#[test]
fn all_rpc_v1_schemas_are_valid_json_schema_documents() {
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
fn m3_schema_freezes_project_repository_contract() {
    let contract = schema("m3-project-repository");
    let definitions = contract["$defs"].as_object().expect("M3 definitions");
    let project_types = definitions["projectSnapshot"]["properties"]["projectType"]["enum"]
        .as_array()
        .expect("project type enum")
        .iter()
        .map(|value| value.as_str().expect("project type"))
        .collect::<Vec<_>>();
    assert_eq!(
        project_types,
        [
            "avatars",
            "worlds",
            "vpm-starter",
            "upm-avatars",
            "upm-worlds",
            "upm-starter",
            "legacy-sdk2",
            "legacy-worlds",
            "legacy-avatars",
            "unknown",
        ]
    );
    assert_eq!(
        definitions["projectSnapshot"]["properties"]["issues"]["maxItems"],
        1_024
    );
    assert_eq!(
        definitions["projectSnapshot"]["properties"]["directDependencies"]["maxItems"],
        4_096
    );
    assert_eq!(definitions["pageLimit"]["maximum"], 1_000);
    assert_eq!(definitions["absolutePath"]["maxLength"], 32_768);
    assert_eq!(definitions["idempotencyKey"]["maxLength"], 128);
    assert!(definitions.contains_key("repositorySource"));
    assert!(definitions.contains_key("packageVersion"));
    assert!(definitions.contains_key("registryCursor"));
    assert!(definitions.contains_key("packageCursor"));
}

#[test]
fn m3_error_codes_are_machine_readable_and_frozen() {
    let errors = schema("rpc-error");
    let codes = errors["properties"]["code"]["enum"]
        .as_array()
        .expect("error code enum");
    for code in [
        "path_encoding_unsupported",
        "project_not_registered",
        "project_manifest_invalid",
        "repository_not_registered",
        "repository_document_too_large",
        "repository_credentials_unsupported",
    ] {
        assert!(codes.iter().any(|candidate| candidate == code), "{code}");
    }
}

#[test]
fn m4_schema_freezes_methods_capabilities_and_bounded_changeset() {
    let contract = schema("m4-package-transaction");
    let definitions = contract["$defs"].as_object().expect("M4 definitions");
    assert_eq!(
        definitions["methodName"]["enum"],
        json!([
            "packages.planInstall",
            "packages.planRemove",
            "packages.planUpgrade",
            "packages.planDowngrade",
            "packages.planResolve",
            "packages.applyPlan"
        ])
    );
    assert_eq!(
        definitions["capability"]["enum"],
        json!(["packages.plan.v1", "packages.apply.v1"])
    );
    assert_eq!(
        definitions["changeSet"]["properties"]["mutations"]["maxItems"],
        1_024
    );
    assert_eq!(
        definitions["changeSet"]["properties"]["dependencyEdges"]["maxItems"],
        4_096
    );
    assert_eq!(
        definitions["applyPlanParams"]["required"],
        json!(["planId", "expectedRevision", "idempotencyKey"])
    );
    assert_eq!(
        definitions["plan"]["properties"]["state"]["enum"],
        json!(["unapplied", "applied"])
    );
    assert!(definitions.get("planExpiry").is_none());
}

#[test]
fn m4_schema_pins_source_and_stale_subreasons() {
    let contract = schema("m4-package-transaction");
    let definitions = contract["$defs"].as_object().expect("M4 definitions");
    assert_eq!(
        definitions["sourcePin"]["required"],
        json!([
            "repositoryId",
            "repositoryRevision",
            "sourceIdentity",
            "manifestFingerprint",
            "packageId",
            "version",
            "artifactUrl",
            "archiveSha256"
        ])
    );
    let stale = definitions["planStaleReason"]["enum"]
        .as_array()
        .expect("stale reasons");
    for reason in [
        "project_revision_changed",
        "project_identity_changed",
        "repository_revision_changed",
        "source_identity_changed",
        "manifest_fingerprint_changed",
        "artifact_url_changed",
        "archive_digest_changed",
        "plan_already_applied",
    ] {
        assert!(
            stale.iter().any(|candidate| candidate == reason),
            "{reason}"
        );
    }
}

#[test]
fn m4_package_manifest_freezes_required_identity_and_unity_semantics() {
    let contract = schema("m4-package-transaction");
    let definitions = contract["$defs"].as_object().expect("M4 definitions");
    assert_eq!(
        definitions["packageManifest"]["required"],
        json!([
            "name",
            "displayName",
            "version",
            "url",
            "zipSHA256",
            "author"
        ])
    );
    assert_eq!(
        definitions["packageAuthor"]["required"],
        json!(["name", "email"])
    );
    assert_eq!(
        definitions["packageManifest"]["properties"]["unity"]["pattern"],
        "^[0-9]+\\.[0-9]+$"
    );
    for legacy in ["legacyFolders", "legacyFiles", "legacyPackages"] {
        assert_eq!(
            definitions["packageManifest"]["properties"][legacy]["type"],
            "array"
        );
    }
}

#[test]
fn m4_error_codes_are_machine_readable_and_have_no_expiry_error() {
    let errors = schema("rpc-error");
    let codes = errors["properties"]["code"]["enum"]
        .as_array()
        .expect("error code enum");
    for code in [
        "package_manifest_invalid",
        "package_version_yanked",
        "package_legacy_cleanup_required",
        "package_archive_unsupported_compression",
        "package_cache_quota_exceeded",
        "plan_too_large",
        "repository_refresh_required",
        "project_changed_during_apply",
    ] {
        assert!(codes.iter().any(|candidate| candidate == code), "{code}");
    }
    assert!(!codes.iter().any(|candidate| candidate == "plan_expired"));
}

#[test]
fn m4_operation_and_data_schema_are_compatible_additions() {
    let operation = schema("operation");
    assert_eq!(
        operation["properties"]["kind"]["enum"],
        json!(["state.check", "packages.apply"])
    );
    assert!(operation["properties"].get("progress").is_some());

    let hello = schema("system-hello.response");
    assert_eq!(
        hello["properties"]["result"]["properties"]["dataSchema"]["enum"],
        json!([1, 2, 3])
    );
}

#[test]
fn hello_schema_only_adds_the_ready_m2_data_schema() {
    let response = schema("system-hello.response");
    let properties = response["properties"]["result"]["properties"]
        .as_object()
        .expect("hello result properties");
    assert_eq!(
        properties.keys().cloned().collect::<Vec<_>>(),
        ["capabilities", "daemonVersion", "dataSchema", "rpcVersion"]
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

#[test]
fn m2_hello_advertises_only_negotiated_capabilities_and_ready_schema() {
    let result = HelloResult::m2(vec![
        CAPABILITY_STATE_CHECK_V1.to_owned(),
        CAPABILITY_OPERATIONS_V1.to_owned(),
        CAPABILITY_EVENTS_REPLAY_V1.to_owned(),
    ]);
    assert_eq!(
        serde_json::to_value(result).expect("serialize M2 hello"),
        json!({
            "rpcVersion": 1,
            "daemonVersion": env!("CARGO_PKG_VERSION"),
            "capabilities": ["state.check.v1", "operations.v1", "events.replay.v1"],
            "dataSchema": 1
        })
    );
}

#[test]
fn m2_request_golden_shapes_freeze_methods_and_write_preconditions() {
    let state_check = RequestEnvelope {
        id: "check-1".to_owned(),
        method: METHOD_STATE_CHECK.to_owned(),
        params: serde_json::to_value(StateCheckParams {
            idempotency_key: "check-once".to_owned(),
        })
        .expect("state.check params"),
    };
    assert_eq!(state_check.method, "state.check");
    assert_eq!(state_check.params["idempotencyKey"], "check-once");

    let cancel = RequestEnvelope {
        id: "cancel-1".to_owned(),
        method: METHOD_OPERATIONS_CANCEL.to_owned(),
        params: serde_json::to_value(OperationsCancelParams {
            operation_id: "00000000-0000-4000-8000-000000000001".to_owned(),
            expected_revision: 2,
            idempotency_key: "cancel-once".to_owned(),
        })
        .expect("operations.cancel params"),
    };
    assert_eq!(cancel.method, "operations.cancel");
    assert_eq!(cancel.params["expectedRevision"], 2);
    assert_eq!(cancel.params["idempotencyKey"], "cancel-once");

    assert_eq!(METHOD_OPERATIONS_GET, "operations.get");
    assert_eq!(METHOD_OPERATIONS_LIST, "operations.list");
    assert_eq!(METHOD_EVENTS_LIST, "events.list");
}

#[test]
fn pagination_contracts_are_exclusive_and_bounded() {
    let operations = OperationsListParams {
        cursor: Some(OperationsListCursor {
            created_at_ms: 123,
            operation_id: "00000000-0000-4000-8000-000000000001".to_owned(),
        }),
        limit: Some(MAX_PAGE_LIMIT),
    };
    let operation_value = serde_json::to_value(operations).expect("operations.list params");
    assert_eq!(operation_value["cursor"]["createdAtMs"], 123);
    assert_eq!(operation_value["limit"], 1_000);

    let events = EventsListParams {
        after_sequence: 40,
        limit: None,
    };
    assert_eq!(
        serde_json::to_value(events).expect("events.list params"),
        json!({"afterSequence": 40})
    );
    let empty_page = EventsListResult {
        events: Vec::new(),
        next_sequence: 40,
    };
    assert_eq!(empty_page.next_sequence, 40);
}

#[test]
fn state_check_acceptance_records_idempotency_replay() {
    let response = OperationAccepted {
        operation_id: "00000000-0000-4000-8000-000000000001".to_owned(),
        replayed: true,
    };
    assert_eq!(
        serde_json::to_value(response).expect("serialize acceptance"),
        json!({
            "operationId": "00000000-0000-4000-8000-000000000001",
            "replayed": true
        })
    );
}

fn schema(name: &str) -> Value {
    let source = SCHEMAS
        .iter()
        .find_map(|(candidate, source)| (*candidate == name).then_some(*source))
        .expect("known schema");
    serde_json::from_str(source).expect("schema JSON")
}
