use serde_json::{Value, json};

use alcomd_protocol::{
    CAPABILITY_EVENTS_REPLAY_V1, CAPABILITY_OPERATIONS_V1, CAPABILITY_STATE_CHECK_V1, ClientInfo,
    EventsListParams, EventsListResult, HelloParams, HelloResult, MAX_PAGE_LIMIT,
    METHOD_EVENTS_LIST, METHOD_OPERATIONS_CANCEL, METHOD_OPERATIONS_GET, METHOD_OPERATIONS_LIST,
    METHOD_STATE_CHECK, METHOD_SYSTEM_HELLO, METHOD_SYSTEM_STATUS, OperationAccepted,
    OperationsCancelParams, OperationsListCursor, OperationsListParams, RPC_VERSION,
    RequestEnvelope, StateCheckParams, SuccessResponse, SystemStatusResult,
};

const SCHEMAS: [(&str, &str); 31] = [
    (
        "m7-operation-logs",
        include_str!("../../../specs/rpc/m7-operation-logs.schema.json"),
    ),
    (
        "m7-events-live",
        include_str!("../../../specs/rpc/m7-events-live.schema.json"),
    ),
    (
        "notification-envelope",
        include_str!("../../../specs/rpc/notification-envelope.schema.json"),
    ),
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
    (
        "m5-unity",
        include_str!("../../../specs/rpc/m5-unity.schema.json"),
    ),
    (
        "m7-project-create",
        include_str!("../../../specs/rpc/m7-project-create.schema.json"),
    ),
    (
        "m5-backup-create",
        include_str!("../../../specs/rpc/m5-backup-create.schema.json"),
    ),
    (
        "m5-backup-restore",
        include_str!("../../../specs/rpc/m5-backup-restore.schema.json"),
    ),
    (
        "m7-official-gui",
        include_str!("../../../specs/rpc/m7-official-gui.schema.json"),
    ),
    (
        "m6-extension-runtime",
        include_str!("../../../specs/rpc/m6-extension-runtime.schema.json"),
    ),
    (
        "m7-repository-list",
        include_str!("../../../specs/rpc/m7-repository-list.schema.json"),
    ),
];

#[test]
fn live_events_contract_separates_ready_responses_and_closed_bounded_notifications() {
    use alcomd_protocol::{
        EventNotification, EventsSubscribeParams, EventsSubscribeResult, OperationsListResult,
        Response,
    };
    let contract = schema("m7-events-live");
    assert_eq!(
        contract["x-alcomd-method-permissions"],
        json!({"events.subscribe":["events.read"]})
    );
    assert_eq!(contract["x-alcomd-local-owner-only"], true);
    assert_eq!(contract["x-alcomd-default-batch-limit"], 100);
    assert_eq!(
        contract["x-alcomd-max-encoded-frame-bytes"],
        4 * 1024 * 1024
    );
    assert_eq!(
        contract["$defs"]["batch"]["properties"]["events"]["maxItems"],
        1000
    );
    let envelope = schema("notification-envelope");
    assert_eq!(envelope["additionalProperties"], false);
    assert_eq!(envelope["required"], json!(["method", "params"]));
    assert!(envelope["properties"].get("id").is_none());
    let vectors: Value = serde_json::from_str(include_str!(
        "../../../specs/rpc/m7-events-live.contract-vectors.json"
    ))
    .expect("vectors");
    let _: Response<EventsSubscribeResult> =
        serde_json::from_value(vectors["ready"].clone()).expect("ready response");
    let _: EventsSubscribeParams =
        serde_json::from_value(vectors["request"]["params"].clone()).expect("params");
    for name in ["batch", "resourceBatch", "closed"] {
        let notification: EventNotification =
            serde_json::from_value(vectors[name].clone()).expect("notification");
        assert_eq!(
            serde_json::to_value(notification).expect("roundtrip"),
            vectors[name]
        );
        assert!(serde_json::from_value::<Response<Value>>(vectors[name].clone()).is_err());
        let mut invalid = vectors[name].clone();
        invalid["id"] = Value::Null;
        assert!(serde_json::from_value::<EventNotification>(invalid).is_err());
    }
    assert_eq!(
        contract["$defs"]["cursor"]["required"],
        json!(["instanceId", "sequence", "resourcesRevision"])
    );
    assert_eq!(
        contract["$defs"]["batch"]["properties"]["invalidatedScopes"]["items"]["enum"],
        json!(["extensions", "operation-log"])
    );
    assert_eq!(
        contract["$defs"]["batch"]["properties"]["invalidatedScopes"]["maxItems"],
        json!(2)
    );
    assert!(contract["$defs"]["batch"]["anyOf"].is_array());
    let durable_kinds = schema("event")["properties"]["aggregateKind"]["enum"].clone();
    assert_eq!(
        durable_kinds,
        json!([
            "operation",
            "project",
            "repository",
            "unity-installation",
            "project-editor-preference",
            "project-unity-launch-config",
            "project-template",
            "user-package"
        ]),
        "the log scope does not add a durable business Event aggregate"
    );
    assert!(
        !durable_kinds
            .as_array()
            .expect("durable kinds")
            .contains(&json!("extension"))
    );
    assert!(
        !durable_kinds
            .as_array()
            .expect("durable kinds")
            .contains(&json!("extensions"))
    );
    let resource_only: EventNotification =
        serde_json::from_value(vectors["resourceBatch"].clone()).expect("resource-only batch");
    let EventNotification::Batch(resource_only) = resource_only else {
        panic!("resource batch")
    };
    assert!(resource_only.events.is_empty());
    assert_eq!(
        resource_only.invalidated_scopes,
        vec![alcomd_protocol::EventInvalidation::Extensions]
    );
    let page: OperationsListResult = serde_json::from_value(vectors["activePage"].clone())
        .expect("active total without loaded rows");
    assert_eq!(page.active_count, 5);
    assert!(page.operations.is_empty());
    assert!(serde_json::from_value::<OperationsListResult>(json!({"operations":[]})).is_err());
    let request: OperationsListParams =
        serde_json::from_value(vectors["activeRequest"].clone()).expect("active filter");
    assert!(request.active_only);
    assert!(
        !serde_json::from_value::<OperationsListParams>(json!({}))
            .expect("default filter")
            .active_only
    );
    assert!(serde_json::from_value::<OperationsListParams>(json!({"activeOnly":null})).is_err());
}

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
fn m6_permissions_read_contract_is_bounded_owner_scoped_and_snapshot_bound() {
    let contract = schema("m6-extension-runtime");
    assert_eq!(
        contract["x-alcomd-method-permissions"]["extensions.permissions.list"],
        json!(["extensions.read"])
    );
    assert_eq!(
        contract["x-alcomd-method-results"]["extensions.permissions.list"],
        "#/$defs/permissionsListResult"
    );
    assert_eq!(
        contract["x-alcomd-permissions-pagination"]["order"],
        json!(["permission", "resourceKind", "resourceId"])
    );
    assert_eq!(
        contract["x-alcomd-permissions-pagination"]["changedSnapshotError"],
        "revision_conflict"
    );
    assert_eq!(contract["$defs"]["permissionDeclarations"]["maxItems"], 64);
    assert_eq!(
        contract["$defs"]["permissionDeclarations"]["items"]["maxLength"],
        128
    );
    assert_eq!(
        contract["$defs"]["permissionsListParams"]["additionalProperties"],
        false
    );
    assert_eq!(
        contract["$defs"]["permissionsListParams"]["properties"]["limit"]["maximum"],
        1_000
    );
    let params: alcomd_protocol::ExtensionPermissionsListParams =
        serde_json::from_value(json!({"extensionId":"dev.example.fixture"}))
            .expect("optional paging");
    assert!(params.cursor.is_none());
    assert!(params.limit.is_none());
    assert!(
        serde_json::from_value::<alcomd_protocol::ExtensionPermissionsListParams>(
            json!({"extensionId":"dev.example.fixture", "owner":"other"})
        )
        .is_err()
    );
    let result = alcomd_protocol::ExtensionPermissionsListResult {
        extension_id: "dev.example.fixture".to_owned(),
        grant_revision: 3,
        required_permissions: vec!["background.run".to_owned()],
        optional_permissions: vec!["projects.read".to_owned()],
        grants: vec![alcomd_protocol::ExtensionPermissionGrant {
            permission: "projects.read".to_owned(),
            resource_kind: "Project".to_owned(),
            resource_id: "00000000-0000-4000-8000-000000000001".to_owned(),
        }],
        next_cursor: None,
    };
    let value = serde_json::to_value(result).expect("typed permissions result");
    assert_eq!(
        value,
        json!({"extensionId":"dev.example.fixture","grantRevision":3,"requiredPermissions":["background.run"],"optionalPermissions":["projects.read"],"grants":[{"permission":"projects.read","resourceKind":"Project","resourceId":"00000000-0000-4000-8000-000000000001"}]})
    );
    let required = contract["$defs"]["planRecord"]["required"]
        .as_array()
        .expect("plan required fields");
    assert!(required.contains(&json!("requiredPermissions")));
    assert!(required.contains(&json!("optionalPermissions")));
}

#[test]
fn m7_official_gui_contract_freezes_only_the_approved_additions() {
    let contract = schema("m7-official-gui");
    assert_eq!(
        contract["x-alcomd-publication"],
        "active-unreleased-breaking-replacement-rpc-v1"
    );
    assert_eq!(contract["x-alcomd-capability"], Value::Null);
    assert_eq!(
        contract["$defs"]["methodName"]["enum"],
        json!(["settings.get", "settings.update", "unity.hub.open"])
    );
    assert_eq!(
        contract["x-alcomd-method-permissions"],
        json!({
            "settings.get": ["settings.read"],
            "settings.update": ["settings.manage"],
            "unity.hub.open": ["settings.read", "unity.hub.open"]
        })
    );

    let settings: Value = serde_json::from_str(include_str!(
        "../../../specs/config/settings-v12.schema.json"
    ))
    .expect("Config12 schema must be valid JSON");
    assert_eq!(
        settings["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(settings["$defs"]["settings"]["additionalProperties"], false);
    assert_eq!(
        settings["$defs"]["partialUpdate"]["additionalProperties"],
        false
    );
    assert_eq!(settings["properties"]["schema"]["const"], 12);
    assert_eq!(
        contract["$defs"]["settingsGetResult"]["properties"]["configSchema"]["const"],
        12
    );
    assert_eq!(
        contract["$defs"]["settingsGetResult"]["properties"]["settings"]["$ref"],
        "https://alcomd.vrclearn.com/schema/config/settings-v12.json#/$defs/settings"
    );
    for historical in [
        include_str!("../../../specs/config/settings-v1.schema.json"),
        include_str!("../../../specs/config/settings-v2.schema.json"),
    ] {
        let historical: Value =
            serde_json::from_str(historical).expect("historical migration input");
        assert!(
            historical["$defs"]["appearance"]["properties"]
                .get("scheme")
                .is_none()
        );
    }
}

#[test]
fn unity_hub_open_has_separate_authority_and_closed_guarded_payloads() {
    use alcomd_protocol::{
        RpcError, UnityHubOpenOutcome, UnityHubOpenParams, UnityHubOpenResult, error_code,
    };
    assert_eq!(alcomd_protocol::METHOD_UNITY_HUB_OPEN, "unity.hub.open");
    assert_eq!(
        alcomd_protocol::CAPABILITY_UNITY_HUB_OPEN_V1,
        "unity.hub.open.v1"
    );
    let contract = schema("m7-official-gui");
    assert_eq!(
        contract["x-alcomd-method-capabilities"],
        json!({"unity.hub.open":"unity.hub.open.v1"})
    );
    let proposal: Value = serde_json::from_str(include_str!(
        "../../../specs/config/unity-hub-location.proposal.contract.json"
    ))
    .expect("Hub proposal");
    assert_eq!(
        contract["$defs"]["hubOpenParams"],
        proposal["openActionProposal"]["schemaFragments"]["$defs"]["openParams"]
    );
    assert_eq!(
        contract["$defs"]["hubOpenResult"],
        proposal["openActionProposal"]["schemaFragments"]["$defs"]["openResult"]
    );
    for expected_path in [Value::Null, json!("C:\\Tools\\Unity Hub.exe")] {
        let wire = json!({"expectedRevision": 7, "expectedPath": expected_path});
        let parsed: UnityHubOpenParams =
            serde_json::from_value(wire.clone()).expect("guarded request");
        assert_eq!(
            serde_json::to_value(parsed).expect("request roundtrip"),
            wire
        );
    }
    for invalid in [
        json!({"expectedRevision":1}),
        json!({"expectedPath":null}),
        json!({"expectedRevision":0,"expectedPath":null}),
        json!({"expectedRevision":1,"expectedPath":""}),
        json!({"expectedRevision":1,"expectedPath":"line\nbreak"}),
        json!({"expectedRevision":1,"expectedPath":"界".repeat(1366)}),
        json!({"expectedRevision":1,"expectedPath":null,"arguments":["--projectPath","other"]}),
        json!({"expectedRevision":1,"expectedPath":null,"url":"https://example.invalid"}),
    ] {
        assert!(serde_json::from_value::<UnityHubOpenParams>(invalid).is_err());
    }
    assert_eq!(
        serde_json::to_value(UnityHubOpenResult {
            outcome: UnityHubOpenOutcome::Spawned
        })
        .expect("spawn result"),
        json!({"outcome":"spawned"})
    );
    assert_eq!(
        serde_json::to_value(UnityHubOpenResult {
            outcome: UnityHubOpenOutcome::DownloadRequired
        })
        .expect("download result"),
        json!({"outcome":"download_required"})
    );
    assert!(serde_json::from_value::<UnityHubOpenResult>(json!({"outcome":"ready"})).is_err());
    assert!(
        serde_json::from_value::<UnityHubOpenResult>(json!({"outcome":"spawned","pid":1})).is_err()
    );
    for (code, message) in [
        (
            error_code::HUB_PATH_CHANGED,
            "The Unity Hub location changed. Read the current settings before trying again.",
        ),
        (
            error_code::HUB_PATH_INVALID,
            "The configured Unity Hub executable is not usable.",
        ),
        (
            error_code::HUB_UNAVAILABLE,
            "The Unity Hub location could not be verified.",
        ),
        (
            error_code::HUB_OPEN_BUSY,
            "Unity Hub launch capacity is temporarily unavailable.",
        ),
        (
            error_code::HUB_OPEN_FAILED,
            "Unity Hub could not be started.",
        ),
    ] {
        let error = RpcError::unity_hub(code);
        assert_eq!(error.code, code);
        assert_eq!(error.message, message);
        assert_eq!(error.data, None);
        assert_eq!(error.diagnostic_id, None);
        assert!(
            schema("rpc-error")["properties"]["code"]["enum"]
                .as_array()
                .expect("error codes")
                .contains(&json!(code))
        );
    }
}

#[test]
fn theme_scheme_contract_matches_typed_rpc_and_rejects_unknown_or_null_values() {
    use alcomd_protocol::{AppearanceScheme, AppearanceSettingsUpdate};

    let settings: Value = serde_json::from_str(include_str!(
        "../../../specs/config/settings-v12.schema.json"
    ))
    .expect("current Config schema");
    let schemes = settings["$defs"]["scheme"]["enum"]
        .as_array()
        .expect("nine schemes");
    assert_eq!(schemes.len(), 9);
    for scheme in schemes {
        let typed: AppearanceScheme = serde_json::from_value(scheme.clone()).expect("typed scheme");
        assert_eq!(
            serde_json::to_value(typed).expect("serialize scheme"),
            *scheme
        );
        let update: AppearanceSettingsUpdate =
            serde_json::from_value(json!({"scheme": scheme})).expect("scheme-only update");
        assert_eq!(update.scheme, Some(typed));
        assert_eq!(
            serde_json::to_value(update).expect("serialize update"),
            json!({"scheme": scheme})
        );
    }
    let unchanged: AppearanceSettingsUpdate =
        serde_json::from_value(json!({"mode": "dark"})).expect("scheme is optional");
    assert!(unchanged.scheme.is_none());
    for invalid in [
        json!("unknown"),
        json!("tonal_spot"),
        Value::Null,
        json!(true),
    ] {
        assert!(
            serde_json::from_value::<AppearanceSettingsUpdate>(json!({
                "mode": "dark", "scheme": invalid
            }))
            .is_err()
        );
    }
}

#[test]
fn motion_contract_and_typed_updates_roundtrip_three_values_and_reject_unknown_input() {
    use alcomd_protocol::{AppearanceMotion, AppearanceSettingsUpdate};
    let omitted: AppearanceSettingsUpdate =
        serde_json::from_value(json!({})).expect("omitted motion is unchanged");
    assert_eq!(omitted.motion, None);
    assert!(
        serde_json::to_value(omitted)
            .expect("serialize omitted motion")
            .get("motion")
            .is_none()
    );
    let settings: Value = serde_json::from_str(include_str!(
        "../../../specs/config/settings-v12.schema.json"
    ))
    .expect("current Config schema");
    assert_eq!(
        settings["$defs"]["motion"]["enum"],
        json!(["system", "full", "reduced"])
    );
    for (text, expected) in [
        ("system", AppearanceMotion::System),
        ("full", AppearanceMotion::Full),
        ("reduced", AppearanceMotion::Reduced),
    ] {
        let motion: AppearanceMotion = serde_json::from_value(json!(text)).expect("typed motion");
        assert_eq!(motion, expected);
        assert_eq!(
            serde_json::to_value(motion).expect("serialize motion"),
            json!(text)
        );
        let update: AppearanceSettingsUpdate =
            serde_json::from_value(json!({"motion": text})).expect("motion-only update");
        assert_eq!(update.motion, Some(expected));
        assert_eq!(
            serde_json::to_value(update).expect("serialize motion-only update"),
            json!({"motion": text})
        );
    }
    for invalid in [
        json!("Full"),
        json!("enabled"),
        json!("disabled"),
        json!("auto"),
        json!("unknown"),
        json!(true),
        Value::Null,
    ] {
        assert!(serde_json::from_value::<AppearanceMotion>(invalid.clone()).is_err());
        assert!(
            serde_json::from_value::<AppearanceSettingsUpdate>(json!({"motion": invalid})).is_err()
        );
    }
    for historical in [
        include_str!("../../../specs/config/settings-v1.schema.json"),
        include_str!("../../../specs/config/settings-v2.schema.json"),
    ] {
        let historical: Value = serde_json::from_str(historical).expect("historical Config schema");
        assert_eq!(
            historical["$defs"]["motion"]["enum"],
            json!(["system", "reduced"])
        );
    }
}

#[test]
fn config_v5_public_settings_and_updates_omit_and_reject_removed_density() {
    use alcomd_protocol::{AppearanceSettings, AppearanceSettingsUpdate, SettingsUpdateParams};

    assert_eq!(alcomd_protocol::CONFIG_SCHEMA_VERSION, 12);
    let appearance =
        json!({"mode":"dark", "scheme":"fidelity", "sourceColor":"#AABBCC", "motion":"reduced"});
    let typed: AppearanceSettings =
        serde_json::from_value(appearance.clone()).expect("current appearance");
    assert_eq!(
        serde_json::to_value(typed).expect("current serialized appearance"),
        appearance
    );
    let schema: Value = serde_json::from_str(include_str!(
        "../../../specs/config/settings-v12.schema.json"
    ))
    .expect("Config12 schema");
    for properties in [
        &schema["$defs"]["appearance"]["properties"],
        &schema["$defs"]["partialUpdate"]["properties"]["appearance"]["properties"],
    ] {
        assert!(properties.get("density").is_none());
    }
    assert!(schema["$defs"].get("density").is_none());
    for density in [json!("default"), json!("compact"), Value::Null, json!(true)] {
        let mut with_removed_field = appearance.clone();
        with_removed_field["density"] = density.clone();
        assert!(serde_json::from_value::<AppearanceSettings>(with_removed_field).is_err());
        assert!(
            serde_json::from_value::<AppearanceSettingsUpdate>(json!({"density":density})).is_err()
        );
        assert!(serde_json::from_value::<SettingsUpdateParams>(json!({"expectedRevision":1,"update":{"appearance":{"density":density},"locale":"ja-JP"}})).is_err());
    }
    for historical in [
        include_str!("../../../specs/config/settings-v1.schema.json"),
        include_str!("../../../specs/config/settings-v2.schema.json"),
        include_str!("../../../specs/config/settings-v3.schema.json"),
    ] {
        let historical: Value = serde_json::from_str(historical).expect("frozen migration shape");
        assert_eq!(
            historical["$defs"]["density"]["enum"],
            json!(["default", "compact"])
        );
        assert!(
            historical["$defs"]["appearance"]["required"]
                .as_array()
                .expect("legacy required fields")
                .contains(&json!("density"))
        );
    }
}

#[test]
fn config5_directory_updates_are_closed_nullable_and_snapshot_complete() {
    use alcomd_protocol::{NullableUpdate, PathSettings, SettingsGetResult, SettingsUpdateParams};
    for invalid in [
        json!({"paths": null, "locale": "ja-JP"}),
        json!({"paths": {}, "locale": "ja-JP"}),
        json!({"paths": {"directory": "/arbitrary"}}),
        json!({"paths": {"projectDirectory": 3}}),
        json!({"paths": {"backupDirectory": false}}),
    ] {
        assert!(
            serde_json::from_value::<SettingsUpdateParams>(
                json!({"expectedRevision":1,"update":invalid})
            )
            .is_err()
        );
    }
    let update: SettingsUpdateParams = serde_json::from_value(json!({"expectedRevision":7,"update":{"paths":{"projectDirectory":"/fixture/projects","backupDirectory":null}}})).expect("two path update");
    let paths = update.update.paths.expect("paths present");
    assert_eq!(
        paths.project_directory,
        NullableUpdate::Set("/fixture/projects".into())
    );
    assert_eq!(paths.backup_directory, NullableUpdate::Clear);
    let omitted: SettingsUpdateParams = serde_json::from_value(
        json!({"expectedRevision":7,"update":{"paths":{"backupDirectory":null}}}),
    )
    .expect("one nullable path");
    assert_eq!(
        omitted.update.paths.expect("paths").project_directory,
        NullableUpdate::Unchanged
    );
    assert!(serde_json::from_value::<PathSettings>(json!({"projectDirectory":null})).is_err());
    let vectors: Value = serde_json::from_str(include_str!(
        "../../../specs/config/settings-v12.contract-vectors.json"
    ))
    .expect("vectors");
    let current: SettingsGetResult =
        serde_json::from_value(vectors["current"]["getResult"].clone()).expect("current result");
    assert_eq!(current.config_schema, 12);
    assert_eq!(current.settings.paths.project_directory, None);
    assert_eq!(
        current.directories.backup_directory,
        "/fixture/Documents/ALCOMD/Backups"
    );
}

#[test]
fn settings_partial_updates_reject_explicit_null_except_nullable_values() {
    use alcomd_protocol::{NullableUpdate, SettingsUpdate, SettingsUpdateParams};

    let empty: SettingsUpdate =
        serde_json::from_value(json!({})).expect("omitted fields stay unchanged");
    assert_eq!(empty, SettingsUpdate::default());
    assert_eq!(
        serde_json::to_value(empty).expect("serialize omissions"),
        json!({})
    );
    for invalid_update in [
        json!({"appearance": null, "locale": "ja-JP"}),
        json!({"locale": null, "appearance": {"mode": "light"}}),
        json!({"packages": null, "locale": "ja-JP"}),
        json!({"appearance": {"mode": null, "sourceColor": null}, "locale": "ja-JP"}),
        json!({"appearance": {"scheme": null}, "locale": "ja-JP"}),
        json!({"appearance": {"density": null}, "locale": "ja-JP"}),
        json!({"appearance": {"motion": null}, "locale": "ja-JP"}),
        json!({"packages": {"showPrerelease": null}, "locale": "ja-JP"}),
        json!({"packages": {"hiddenRepositoryIds": null}, "locale": "ja-JP"}),
        json!({"packages": {"hideLocalUserPackages": null}, "locale": "ja-JP"}),
        json!({"unity": null, "locale": "ja-JP"}),
        json!({"unity": {}, "locale": "ja-JP"}),
        json!({"unity": {"hubPath": null, "launch": true}, "locale": "ja-JP"}),
    ] {
        assert!(
            serde_json::from_value::<SettingsUpdateParams>(json!({
                "expectedRevision": 1, "update": invalid_update
            }))
            .is_err(),
            "explicit null must reject the whole mixed update: {invalid_update}"
        );
    }
    let clear: SettingsUpdate =
        serde_json::from_value(json!({"appearance": {"sourceColor": null}}))
            .expect("sourceColor retains its explicit clear state");
    assert_eq!(
        clear.appearance.as_ref().expect("appearance").source_color,
        NullableUpdate::Clear
    );
    assert_eq!(
        serde_json::to_value(clear).expect("serialize clear"),
        json!({"appearance": {"sourceColor": null}})
    );
}

#[test]
fn config6_default_arguments_distinguish_omitted_null_empty_and_literal_argv() {
    use alcomd_protocol::{NullableUpdate, SettingsUpdate, UnitySettings};
    for arguments in [
        Value::Null,
        json!([]),
        json!(["-logFile", "C:\\User Data\\Unity\\a \"quoted\".log"]),
    ] {
        let wire = json!({"unity":{"defaultLaunchArguments":arguments}});
        let update: SettingsUpdate =
            serde_json::from_value(wire.clone()).expect("arguments update");
        let unity = update.unity.as_ref().expect("unity");
        assert_eq!(unity.hub_path, NullableUpdate::Unchanged);
        assert_eq!(
            matches!(unity.default_launch_arguments, NullableUpdate::Clear),
            arguments.is_null()
        );
        assert_eq!(serde_json::to_value(update).expect("roundtrip"), wire);
        let settings = json!({"hubPath":null,"defaultLaunchArguments":arguments});
        let parsed: UnitySettings =
            serde_json::from_value(settings.clone()).expect("full settings");
        assert_eq!(
            serde_json::to_value(parsed).expect("settings roundtrip"),
            settings
        );
    }
    let hub_only: SettingsUpdate =
        serde_json::from_value(json!({"unity":{"hubPath":null}})).expect("Hub-only update");
    assert_eq!(
        hub_only.unity.expect("unity").default_launch_arguments,
        NullableUpdate::Unchanged
    );
    for invalid in [json!("-batchmode"), json!([1]), json!([null]), json!({})] {
        assert!(
            serde_json::from_value::<SettingsUpdate>(
                json!({"unity":{"defaultLaunchArguments":invalid},"locale":"ja-JP"})
            )
            .is_err()
        );
    }
    assert!(serde_json::from_value::<UnitySettings>(json!({"hubPath":null})).is_err());
    assert!(
        serde_json::from_value::<UnitySettings>(json!({"defaultLaunchArguments":null})).is_err()
    );
    let current: Value = serde_json::from_str(include_str!(
        "../../../specs/config/settings-v12.schema.json"
    ))
    .expect("Config6");
    assert_eq!(
        current["$defs"]["unitySettings"]["required"],
        json!(["hubPath", "defaultLaunchArguments"])
    );
    assert_eq!(current["$defs"]["launchArguments"]["maxItems"], 64);
    assert_eq!(
        current["$defs"]["launchArguments"]["items"]["x-alcomd-maxUtf8Bytes"],
        4096
    );
    assert_eq!(current["x-alcomd-storage"]["maxUtf8Bytes"], 16384);
    assert_eq!(
        schema("m7-official-gui")["x-alcomd-field-permissions"]["settings.update.unity.defaultLaunchArguments"],
        json!(["settings.manage", "unity.manage"])
    );
    let historical: Value = serde_json::from_str(include_str!(
        "../../../specs/config/settings-v5.schema.json"
    ))
    .expect("frozen Config5");
    assert_eq!(historical["properties"]["schema"]["const"], 5);
    assert!(
        historical["$defs"]["unitySettings"]["properties"]
            .get("defaultLaunchArguments")
            .is_none()
    );
}

#[test]
fn project_launch_config_get_requires_default_and_effective_projection() {
    use alcomd_protocol::ProjectUnityLaunchConfigResult;
    let config = json!({"projectId":"00000000-0000-4000-8000-000000000001","arguments":[],"revision":0,"updatedAtMs":0});
    let wire = json!({"config":config,"defaultArguments":["-batchmode"],"effectiveArguments":["-batchmode"]});
    let typed: ProjectUnityLaunchConfigResult =
        serde_json::from_value(wire.clone()).expect("read projection");
    assert_eq!(
        serde_json::to_value(typed).expect("roundtrip projection"),
        wire
    );
    assert!(
        serde_json::from_value::<ProjectUnityLaunchConfigResult>(json!({"config":config})).is_err()
    );
    assert_eq!(
        schema("m5-unity")["$defs"]["projectUnityLaunchConfigResult"]["required"],
        json!(["config", "defaultArguments", "effectiveArguments"])
    );
    assert_eq!(
        schema("m5-unity")["$defs"]["projectUnityLaunchConfigMutationResult"]["required"],
        json!(["config", "changed", "replayed"])
    );
}

#[test]
fn config7_window_settings_are_closed_required_bool_and_false_is_an_explicit_update() {
    use alcomd_protocol::{Settings, SettingsUpdate, WindowSettings};
    for remember_state in [false, true] {
        let window = json!({"rememberState":remember_state,"fullscreen":false});
        let typed: WindowSettings = serde_json::from_value(window.clone()).expect("window setting");
        assert_eq!(typed.remember_state, remember_state);
        let wire = json!({"window":window});
        let update: SettingsUpdate = serde_json::from_value(wire.clone()).expect("window update");
        assert_eq!(
            update.window.expect("explicit window").remember_state,
            Some(remember_state)
        );
        assert_eq!(
            serde_json::to_value(update).expect("window roundtrip"),
            wire
        );
    }
    for invalid in [
        Value::Null,
        json!({}),
        json!({"rememberState":null}),
        json!({"rememberState":"true"}),
        json!({"rememberState":1}),
        json!({"rememberState":true,"width":1000}),
        json!({"rememberState":false,"maximized":true}),
    ] {
        assert!(
            serde_json::from_value::<SettingsUpdate>(json!({"window":invalid,"locale":"ja-JP"}))
                .is_err()
        );
        assert!(serde_json::from_value::<WindowSettings>(invalid).is_err());
    }
    let omitted = SettingsUpdate::default();
    assert_eq!(omitted.window, None);
    assert_eq!(
        serde_json::to_value(omitted).expect("omitted update"),
        json!({})
    );
    let vectors: Value = serde_json::from_str(include_str!(
        "../../../specs/config/settings-v12.contract-vectors.json"
    ))
    .expect("Config12 vectors");
    let settings: Settings =
        serde_json::from_value(vectors["current"]["getResult"]["settings"].clone())
            .expect("complete current settings");
    assert!(!settings.window.remember_state);
    let mut missing = vectors["current"]["getResult"]["settings"].clone();
    missing
        .as_object_mut()
        .expect("settings object")
        .remove("window");
    assert!(serde_json::from_value::<Settings>(missing).is_err());
    for legacy in vectors["legacyInputs"]
        .as_array()
        .expect("historical vectors")
    {
        let normalized: Settings = serde_json::from_value(legacy["expectedSettings"].clone())
            .expect("normalized migration output");
        assert_eq!(
            normalized.window.remember_state,
            legacy["expectedSettings"]["window"]["rememberState"]
                .as_bool()
                .expect("window preference")
        );
    }
    let historical: Value = serde_json::from_str(include_str!(
        "../../../specs/config/settings-v6.schema.json"
    ))
    .expect("frozen Config6");
    assert_eq!(historical["properties"]["schema"]["const"], 6);
    assert!(
        historical["$defs"]["settings"]["properties"]
            .get("window")
            .is_none()
    );
}

#[test]
fn config9_projects_and_backups_are_complete_closed_and_partial_updates_are_non_nullable() {
    use alcomd_protocol::{Settings, SettingsUpdate};
    let vectors: Value = serde_json::from_str(include_str!(
        "../../../specs/config/settings-v12.contract-vectors.json"
    ))
    .expect("Config12 vectors");
    let settings: Settings =
        serde_json::from_value(vectors["current"]["getResult"]["settings"].clone())
            .expect("Config12 settings");
    assert!(settings.projects.auto_discover);
    assert_eq!(
        settings.backups.compression_mode,
        alcomd_protocol::BackupCompression::Fast
    );
    assert!(settings.backups.include_vpm_packages);
    assert!(!settings.backups.include_library);
    assert_eq!(
        schema("m7-official-gui")["x-alcomd-field-permissions"]["settings.update.projects.autoDiscover"],
        json!(["settings.manage", "projects.manage"])
    );
    assert_eq!(
        schema("m7-official-gui")["x-alcomd-conditional-field-permissions"]["settings.update.paths.projectDirectory"],
        json!({
            "when": { "field":"projects.autoDiscover", "equals":true, "source":"effective-update" },
            "permissions":["settings.manage","projects.manage"]
        })
    );
    for invalid in vectors["rejectedUpdates"]
        .as_array()
        .expect("rejected updates")
    {
        assert!(
            serde_json::from_value::<SettingsUpdate>(invalid["update"].clone()).is_err(),
            "{}",
            invalid["id"]
        );
    }
    for update in [
        json!({"projects":{"autoDiscover":false}}),
        json!({"backups":{"compressionMode":"store"}}),
        json!({"backups":{"compressionMode":"fast"}}),
        json!({"backups":{"compressionMode":"level3"}}),
        json!({"backups":{"compressionMode":"level5"}}),
        json!({"backups":{"compressionMode":"level7"}}),
        json!({"backups":{"compressionMode":"maximum"}}),
        json!({"backups":{"includeVpmPackages":false}}),
        json!({"backups":{"includeLibrary":true}}),
        json!({"backups":{"compressionMode":"store","includeVpmPackages":false,"includeLibrary":false}}),
    ] {
        let decoded: SettingsUpdate =
            serde_json::from_value(update.clone()).expect("partial update");
        assert_eq!(
            serde_json::to_value(decoded).expect("partial roundtrip"),
            update
        );
    }
    for missing in ["projects", "backups"] {
        let mut incomplete = vectors["current"]["getResult"]["settings"].clone();
        incomplete
            .as_object_mut()
            .expect("settings")
            .remove(missing);
        assert!(serde_json::from_value::<Settings>(incomplete).is_err());
    }
}

#[test]
fn unity_hub_contract_preserves_nullable_preference_and_separate_evidence() {
    use alcomd_protocol::{NullableUpdate, SettingsUpdate, UnityHubLocation, UnitySettings};

    for path in [Value::Null, json!("C:\\Tools\\Unity Hub\\Unity Hub.exe")] {
        let wire = json!({"unity": {"hubPath": path}});
        let update: SettingsUpdate = serde_json::from_value(wire.clone()).expect("Hub update");
        let value = &update.unity.as_ref().expect("unity").hub_path;
        assert_eq!(matches!(value, NullableUpdate::Clear), path.is_null());
        assert_eq!(serde_json::to_value(update).expect("Hub roundtrip"), wire);
    }
    assert!(serde_json::from_value::<UnitySettings>(json!({})).is_err());
    assert!(serde_json::from_value::<UnitySettings>(json!({"hubPath": 1})).is_err());
    let location = json!({
        "status": "available", "source": "discovered",
        "path": "/opt/unityhub/unityhub", "configuredPathStatus": "invalid"
    });
    let parsed: UnityHubLocation =
        serde_json::from_value(location.clone()).expect("derived evidence");
    assert_eq!(
        serde_json::to_value(parsed).expect("evidence roundtrip"),
        location
    );
    for invalid in [
        json!({"status":"unknown", "source":"none", "path":null, "configuredPathStatus":"unset"}),
        json!({"status":"not_found", "source":"none", "configuredPathStatus":"unset"}),
        json!({"status":"not_found", "source":"none", "path":null, "configuredPathStatus":"unset", "launch":true}),
    ] {
        assert!(serde_json::from_value::<UnityHubLocation>(invalid).is_err());
    }

    let contract = schema("m7-official-gui");
    let settings: Value = serde_json::from_str(include_str!(
        "../../../specs/config/settings-v12.schema.json"
    ))
    .expect("Config12 schema");
    assert_eq!(
        contract["$defs"]["settingsGetResult"]["required"],
        json!([
            "configSchema",
            "revision",
            "settings",
            "unityHub",
            "directories"
        ])
    );
    assert_eq!(
        contract["x-alcomd-redaction"]["authorizedSettingsPathException"],
        json!([
            "settings.unity.hubPath",
            "unityHub.path",
            "settings.paths.projectDirectory",
            "settings.paths.backupDirectory",
            "directories.projectDirectory",
            "directories.backupDirectory"
        ])
    );
    assert_eq!(
        settings["$defs"]["settings"]["required"],
        json!([
            "appearance",
            "packages",
            "locale",
            "unity",
            "paths",
            "window",
            "projects",
            "backups",
            "integrations",
            "network"
        ])
    );
    assert_eq!(settings["$defs"]["hubPath"]["x-alcomd-maxUtf8Bytes"], 4096);
    assert_eq!(settings["$defs"]["partialUnityUpdate"]["minProperties"], 1);
    let proposal: Value = serde_json::from_str(include_str!(
        "../../../specs/config/unity-hub-location.proposal.contract.json"
    ))
    .expect("approved proposal");
    let mut expected = proposal["schemaFragments"]["$defs"]["hubLocation"].clone();
    expected["properties"]["path"] = json!({"$ref":"https://alcomd.vrclearn.com/schema/config/settings-v12.json#/$defs/hubPath"});
    assert_eq!(contract["$defs"]["unityHubLocation"], expected);
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
    assert_eq!(
        definitions["projectSnapshot"]["properties"]["registeredAtMs"]["maximum"],
        9_223_372_036_854_775_807_i64
    );
    assert!(
        !definitions["projectSnapshot"]["required"]
            .as_array()
            .expect("project snapshot required fields")
            .iter()
            .any(|field| field.as_str() == Some("registeredAtMs"))
    );
    assert_eq!(
        definitions["projectSnapshot"]["properties"]["lastModifiedAtMs"],
        json!({"type": "integer", "minimum": 0, "maximum": 9_223_372_036_854_775_807_i64})
    );
    assert!(
        !definitions["projectSnapshot"]["required"]
            .as_array()
            .expect("project snapshot required fields")
            .iter()
            .any(|field| field.as_str() == Some("lastModifiedAtMs"))
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
fn repository_refresh_all_uses_one_key_and_reports_each_registered_source() {
    use alcomd_protocol::*;
    let contract = schema("m3-project-repository");
    let definitions = &contract["$defs"];
    assert_eq!(METHOD_REPOSITORIES_REFRESH_ALL, "repositories.refreshAll");
    assert_eq!(
        definitions["repositoryRefreshAllParams"]["required"],
        json!(["idempotencyKey"])
    );
    assert_eq!(
        definitions["repositoryRefreshAllParams"]["properties"]["idempotencyKey"]["maxLength"],
        64
    );
    assert_eq!(
        definitions["repositoryRefreshAllResult"]["properties"]["items"]["maxItems"],
        4096
    );
    assert!(
        serde_json::from_value::<RepositoryRefreshAllParams>(
            json!({"idempotencyKey":"refresh-key","repositoryIds":[]})
        )
        .is_err()
    );
    let repository_id = "00000000-0000-4000-8000-000000000101";
    assert_eq!(
        serde_json::to_value(RepositoryRefreshAllResult {
            items: vec![RepositoryRefreshAllItem::Failed {
                repository_id: repository_id.into(),
                code: "repository_unavailable".into(),
            }],
        })
        .expect("refresh-all result wire"),
        json!({"items":[{"repositoryId":repository_id,"status":"failed","code":"repository_unavailable"}]})
    );
    assert!(
        serde_json::from_value::<RepositoryRefreshAllItem>(
            json!({"repositoryId":repository_id,"status":"refreshed","code":"unexpected"})
        )
        .is_err()
    );
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
        json!(["name", "displayName", "version", "url", "author"])
    );
    assert_eq!(definitions["packageAuthor"]["required"], json!(["name"]));
    let manifest = &definitions["packageManifest"];
    assert!(manifest.get("if").is_none());
    assert_eq!(
        manifest["properties"]["url"]["pattern"],
        "^[Hh][Tt][Tt][Pp][Ss]?://"
    );
    let publisher_digest = manifest["properties"]["zipSHA256"]
        .as_object()
        .expect("digest definition");
    assert!(
        !publisher_digest.contains_key("$ref")
            && !publisher_digest.contains_key("type")
            && !publisher_digest.contains_key("pattern")
    );
    assert_eq!(definitions["sha256"]["pattern"], "^[0-9a-f]{64}$");
    assert_eq!(
        definitions["packageManifest"]["properties"]["unity"]["pattern"],
        "^[0-9]+\\.[0-9]+$"
    );
    for legacy in ["legacyFolders", "legacyFiles"] {
        assert_eq!(
            definitions["packageManifest"]["properties"][legacy]["type"],
            "object"
        );
    }
    assert_eq!(
        definitions["packageManifest"]["properties"]["legacyPackages"]["type"],
        "array"
    );
}

#[test]
fn m4_package_change_set_exposes_only_reviewable_legacy_removal_fields() {
    let contract = schema("m4-package-transaction");
    let properties = &contract["$defs"]["changeSet"]["properties"];
    assert_eq!(properties["formatVersion"]["enum"], json!([1, 2, 3, 4]));
    let removal = &properties["legacyRemovals"]["items"];
    assert_eq!(removal["additionalProperties"], false);
    assert_eq!(removal["required"], json!(["path", "kind", "packageId"]));
    let dto = alcomd_protocol::PackageLegacyRemoval {
        path: "Assets/OldTools.meta".to_owned(),
        kind: alcomd_protocol::PackageLegacyRemovalKind::File,
        package_id: "com.example.tools".to_owned(),
    };
    assert_eq!(
        serde_json::to_value(dto).expect("serialize the public removal DTO"),
        json!({
            "path": "Assets/OldTools.meta", "kind": "file", "packageId": "com.example.tools"
        })
    );
}

#[test]
fn removal_conflicts_are_reviewable_bounded_and_omitted_from_old_wire_plans() {
    let contract = schema("m4-package-transaction");
    let conflict = &contract["$defs"]["changeSet"]["properties"]["removalConflicts"];
    assert_eq!(conflict["maxItems"], 1024);
    assert_eq!(
        conflict["items"]["properties"]["requiredBy"]["maxItems"],
        4096
    );
    assert_eq!(
        conflict["items"]["required"],
        json!(["packageId", "requiredBy"])
    );
    let mut value = json!({
        "formatVersion":4,"mutations":[],"dependencyEdges":[],"vpmManifestSha256":"0".repeat(64),
        "removalConflicts":[{"packageId":"com.example.dependency","requiredBy":[{"packageId":"com.example.root","range":"^1.0.0"}]}]
    });
    let decoded: alcomd_protocol::PackageChangeSet =
        serde_json::from_value(value.clone()).expect("new wire field");
    assert_eq!(serde_json::to_value(&decoded).expect("round trip"), value);
    value["removalConflicts"][0]["requiredBy"][0]["path"] = json!("not a public field");
    assert!(serde_json::from_value::<alcomd_protocol::PackageChangeSet>(value.clone()).is_err());
    value
        .as_object_mut()
        .expect("object")
        .remove("removalConflicts");
    let old: alcomd_protocol::PackageChangeSet =
        serde_json::from_value(value.clone()).expect("no conflict plan");
    assert!(old.removal_conflicts.is_empty());
    assert_eq!(serde_json::to_value(old).expect("omit empty field"), value);
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
        json!([
            "state.check",
            "packages.apply",
            "projects.create",
            "backups.create",
            "backups.restore",
            "extensions.install",
            "extensions.uninstall"
        ])
    );
    assert!(operation["properties"].get("progress").is_some());

    let hello = schema("system-hello.response");
    assert_eq!(
        hello["properties"]["result"]["properties"]["dataSchema"]["enum"],
        json!([
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 21, 22, 23, 27
        ])
    );
}

#[test]
fn m5_unity_schema_freezes_methods_capabilities_and_honest_writer_states() {
    let contract = schema("m5-unity");
    let definitions = contract["$defs"].as_object().expect("M5 Unity definitions");
    assert_eq!(
        definitions["methodName"]["enum"],
        json!([
            "unity.installations.list",
            "unity.installations.get",
            "unity.installations.register",
            "unity.installations.remove",
            "unity.installations.refresh",
            "unity.projectLaunchConfig.get",
            "unity.projectLaunchConfig.set",
            "unity.projectLaunchConfig.clear",
            "unity.writerState",
            "unity.launchOptions",
            "unity.launch"
        ])
    );
    assert!(!definitions.contains_key("launchStatusParams"));
    assert_eq!(
        definitions["capability"]["enum"],
        json!(["unity.read.v1", "unity.manage.v1", "unity.launch.v1"])
    );
    assert_eq!(
        definitions["writerStateKind"]["enum"],
        json!([
            "running_confirmed",
            "running_suspected",
            "not_observed",
            "unknown"
        ])
    );
    assert_eq!(
        definitions["architecture"]["enum"],
        json!(["x86_64", "arm64", "universal", "unknown"])
    );
    assert_eq!(definitions["boundedArguments"]["maxItems"], 64);
    assert_eq!(definitions["boundedArguments"]["items"]["maxLength"], 4_096);
}

#[test]
fn m5_unity_schema_keeps_launch_and_management_separate() {
    let definitions = schema("m5-unity")["$defs"]
        .as_object()
        .expect("M5 Unity definitions")
        .clone();
    assert_eq!(
        definitions["projectUnityLaunchConfigSetParams"]["required"],
        json!([
            "projectId",
            "arguments",
            "expectedRevision",
            "idempotencyKey"
        ])
    );
    assert_eq!(
        definitions["launchParams"]["required"],
        json!([
            "projectId",
            "installationId",
            "expectedProjectRevision",
            "idempotencyKey"
        ])
    );
    assert_eq!(
        definitions["launchState"]["enum"],
        json!(["opening", "open", "failed"])
    );
    assert_eq!(definitions["expectedOptionalRevision"]["minimum"], 0);
}

#[test]
fn hello_schema_accepts_the_implemented_v14_store() {
    let hello = schema("system-hello.response");
    assert!(
        hello["properties"]["result"]["properties"]["dataSchema"]["enum"]
            .as_array()
            .expect("supported historical data schemas")
            .contains(&json!(14))
    );
}

#[test]
fn hello_schema_accepts_the_project_observation_removal_v16_store() {
    let hello = schema("system-hello.response");
    assert_eq!(
        hello["properties"]["result"]["properties"]["dataSchema"]["enum"],
        json!([
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 21, 22, 23, 27
        ])
    );
}

#[test]
fn m3_project_list_view_is_an_explicit_pagination_contract() {
    let contract = schema("m3-project-repository");
    let definitions = &contract["$defs"];
    assert_eq!(
        definitions["projectListView"]["properties"]["sort"]["enum"],
        json!(["name", "type", "unity", "added", "modified"])
    );
    assert_eq!(
        definitions["projectListView"]["properties"]["search"]["maxLength"],
        256
    );
    assert_eq!(
        definitions["projectListResult"]["properties"]["projects"]["maxItems"],
        1000
    );
    let value = json!({
        "view": {"search": "Avatar", "sort": "modified", "descending": true},
        "cursor": {"registeredAtMs": 100, "id": "00000000-0000-4000-8000-000000000101"},
        "expectedViewSequence": 8
    });
    let params: alcomd_protocol::ProjectsListParams =
        serde_json::from_value(value.clone()).expect("project view params");
    assert_eq!(
        serde_json::to_value(params).expect("project view json"),
        value
    );
    for invalid in [
        json!({"view": null}),
        json!({"view": {"sort": "random", "descending": false}}),
        json!({"view": {"search": null, "sort": "name", "descending": false}}),
        json!({"expectedViewSequence": null}),
    ] {
        assert!(serde_json::from_value::<alcomd_protocol::ProjectsListParams>(invalid).is_err());
    }
}

#[test]
fn project_snapshot_has_optional_modified_time_without_observation_time() {
    let project_schema = &schema("m3-project-repository")["$defs"]["projectSnapshot"];
    assert!(project_schema["properties"].get("observedAtMs").is_none());
    assert!(
        !project_schema["required"]
            .as_array()
            .expect("project required fields")
            .contains(&json!("observedAtMs"))
    );
    let original = json!({
        "projectId": "00000000-0000-4000-8000-000000000101",
        "registeredAtMs": 100,
        "rootPath": "<private-project>",
        "projectType": "avatars",
        "unityVersion": "2022.3.22f1",
        "unityRevision": null,
        "vpmManifest": "valid",
        "upmManifest": "valid",
        "directDependencies": [],
        "lockedDependencies": [],
        "issues": [],
        "revision": 2
    });
    let unknown: alcomd_protocol::ProjectSnapshot = serde_json::from_value(original.clone())
        .expect("read a snapshot without modified evidence");
    assert_eq!(unknown.last_modified_at_ms, None);
    let unknown_json = serde_json::to_value(&unknown).expect("serialize unknown modified time");
    assert!(unknown_json.get("lastModifiedAtMs").is_none());
    assert!(unknown_json.get("observedAtMs").is_none());
    let mut updated = original;
    updated["lastModifiedAtMs"] = json!(500);
    let current: alcomd_protocol::ProjectSnapshot =
        serde_json::from_value(updated).expect("read authoritative modified time");
    assert_eq!(current.last_modified_at_ms, Some(500));
    let current_json = serde_json::to_value(&current).expect("serialize known modified time");
    assert_eq!(current_json["lastModifiedAtMs"], 500);
    assert!(current_json.get("observedAtMs").is_none());
}

#[test]
fn m5_cli_and_unity_errors_are_stable_machine_codes() {
    let errors = schema("rpc-error");
    let codes = errors["properties"]["code"]["enum"]
        .as_array()
        .expect("error code enum");
    for code in [
        "confirmation_required",
        "unity_installation_not_found",
        "unity_installation_invalid",
        "unity_installation_in_use",
        "unity_version_unverified",
        "unity_version_mismatch",
        "unity_architecture_unsupported",
        "unity_project_running",
        "unity_launch_state_uncertain",
        "unity_project_selector_forbidden",
        "unity_launch_failed",
        "unity_launch_not_found",
        "unity_editor_selection_required",
    ] {
        assert!(codes.iter().any(|candidate| candidate == code), "{code}");
    }
}

#[test]
fn m7_project_favorite_and_unity_launch_config_are_active_contracts() {
    let projects = schema("m3-project-repository");
    assert_eq!(
        projects["$defs"]["projectSnapshot"]["properties"]["favorite"]["type"],
        "boolean"
    );
    assert_eq!(
        projects["$defs"]["projectSetFavoriteParams"]["required"],
        json!([
            "projectId",
            "favorite",
            "expectedRevision",
            "idempotencyKey"
        ])
    );

    let unity = schema("m5-unity");
    assert_eq!(
        unity["$defs"]["projectUnityLaunchConfig"]["required"],
        json!(["projectId", "arguments", "revision", "updatedAtMs"])
    );
    assert_eq!(
        unity["$defs"]["projectUnityLaunchConfig"]["properties"]["revision"]["minimum"],
        0
    );
    assert_eq!(
        unity["$defs"]["projectUnityLaunchConfigClearParams"]["required"],
        json!(["projectId", "expectedRevision", "idempotencyKey"])
    );
    assert_eq!(
        unity["$defs"]["launchOptionsParams"]["required"],
        json!(["projectId", "expectedProjectRevision"])
    );
}

#[test]
fn project_creation_and_template_archives_round_trip() {
    use alcomd_protocol::{
        ProjectTemplatesExportParams, ProjectTemplatesListResult, ProjectTemplatesRemoveParams,
        ProjectTemplatesRemoveResult, ProjectTemplatesSaveParams, ProjectTemplatesSaveResult,
        ProjectsCreateParams, ProjectsCreateResult,
    };
    let contract = schema("m7-project-create");
    assert_eq!(contract["x-alcomd-local-owner-only"], true);
    assert_eq!(contract["x-alcomd-max-canonical-declaration-bytes"], 24_576);
    assert_eq!(
        contract["x-alcomd-method-capabilities"],
        json!({
            "projects.create": "projects.create.v1",
            "projectTemplates.list": "projectTemplates.v2",
            "projectTemplates.save": "projectTemplates.v2",
            "projectTemplates.export": "projectTemplates.v2",
            "projectTemplates.remove": "projectTemplates.v2"
        })
    );
    assert_eq!(
        contract["$defs"]["createParams"]["required"],
        json!(["name", "targetParent", "unityVersion", "idempotencyKey"])
    );
    let vectors: Value = serde_json::from_str(include_str!(
        "../../../specs/rpc/m7-project-create.contract-vectors.json"
    ))
    .expect("project creation vectors");
    fn round_trip<T: serde::de::DeserializeOwned + serde::Serialize>(sample: &Value) {
        let parsed: T = serde_json::from_value(sample.clone()).expect("valid contract vector");
        assert_eq!(serde_json::to_value(parsed).expect("round trip"), *sample);
    }
    round_trip::<ProjectsCreateParams>(&vectors["create"]);
    round_trip::<ProjectsCreateParams>(&vectors["createFromTemplate"]);
    round_trip::<ProjectsCreateResult>(&vectors["created"]);
    round_trip::<ProjectTemplatesListResult>(&vectors["list"]);
    round_trip::<ProjectTemplatesSaveParams>(&vectors["save"]);
    round_trip::<ProjectTemplatesSaveParams>(&vectors["saveFile"]);
    round_trip::<ProjectTemplatesSaveParams>(&vectors["saveProject"]);
    round_trip::<ProjectTemplatesExportParams>(&vectors["export"]);
    round_trip::<ProjectTemplatesSaveResult>(&vectors["saved"]);
    round_trip::<ProjectTemplatesRemoveParams>(&vectors["remove"]);
    round_trip::<ProjectTemplatesRemoveResult>(&vectors["removed"]);
    let mut request = vectors["create"].clone();
    request["template"] = Value::Null;
    assert!(serde_json::from_value::<ProjectsCreateParams>(request).is_err());
    let mut request = vectors["create"].clone();
    request
        .as_object_mut()
        .expect("request")
        .remove("unityVersion");
    assert!(serde_json::from_value::<ProjectsCreateParams>(request).is_err());
    let mut request = vectors["save"].clone();
    request["source"]["base"] = json!("com.example.base");
    assert!(serde_json::from_value::<ProjectTemplatesSaveParams>(request).is_err());

    let hello = HelloParams {
        rpc_version: RPC_VERSION,
        client: ClientInfo {
            name: "contract-test".to_owned(),
            version: "test".to_owned(),
            instance_id: "contract".to_owned(),
        },
        capabilities: vec![
            "projects.create.v1".to_owned(),
            "projectTemplates.v2".to_owned(),
        ],
    };
    assert!(hello.validate().is_ok());
    let invalid = HelloParams {
        capabilities: vec!["projectTemplates.v1".to_owned()],
        ..hello
    };
    assert!(invalid.validate().is_err());

    let errors = schema("rpc-error");
    let codes = errors["properties"]["code"]["enum"]
        .as_array()
        .expect("error codes");
    for code in [
        "template_conflict",
        "template_builtin_immutable",
        "template_dependency_unsatisfied",
        "project_create_target_exists",
        "project_create_destination_changed",
    ] {
        assert!(codes.iter().any(|candidate| candidate == code), "{code}");
    }
    assert!(
        !codes
            .iter()
            .any(|candidate| candidate == "template_bundle_invalid")
    );
}

#[test]
fn m5_backup_create_contract_is_additive_and_published() {
    let contract = schema("m5-backup-create");
    assert_eq!(
        contract["x-alcomd-publication"],
        "implemented-additive-rpc-v1"
    );
    assert_eq!(
        contract["$defs"]["methodName"]["enum"],
        json!(["backups.list", "backups.get", "backups.create"])
    );
    assert_eq!(
        contract["$defs"]["capability"]["enum"],
        json!(["backups.read.v1", "backups.create.v1"])
    );
    assert_eq!(
        contract["$defs"]["createResult"]["required"],
        json!(["operationId", "backupId", "replayed"])
    );
    let choices = json!(["store", "fast", "level3", "level5", "level7", "maximum"]);
    assert_eq!(contract["$defs"]["compressionMode"]["enum"], choices);
    let settings: Value = serde_json::from_str(include_str!(
        "../../../specs/config/settings-v12.schema.json"
    ))
    .expect("current settings schema");
    for definition in ["backupSettings", "partialBackupsUpdate"] {
        assert_eq!(
            settings["$defs"][definition]["properties"]["compressionMode"]["enum"],
            choices
        );
    }
}

#[test]
fn m5_backup_restore_contract_is_additive_and_published() {
    let contract = schema("m5-backup-restore");
    assert_eq!(contract["x-alcomd-publication"], "implemented-published");
    assert_eq!(
        contract["$defs"]["methodName"]["enum"],
        json!(["backups.planRestore", "backups.applyRestore"])
    );
    assert_eq!(
        contract["$defs"]["capability"]["const"],
        "backups.restore.v1"
    );
    assert_eq!(
        contract["$defs"]["applyRestoreResult"]["required"],
        json!(["operationId", "projectId", "replayed"])
    );
}

#[test]
fn hello_schema_adds_extension_api_and_config_schema_as_optional() {
    let response = schema("system-hello.response");
    let properties = response["properties"]["result"]["properties"]
        .as_object()
        .expect("hello result properties");
    assert_eq!(
        properties.keys().cloned().collect::<Vec<_>>(),
        [
            "capabilities",
            "configSchema",
            "daemonVersion",
            "dataSchema",
            "extensionApi",
            "rpcVersion"
        ]
    );
    assert!(
        !response["properties"]["result"]["required"]
            .as_array()
            .expect("hello required fields")
            .iter()
            .any(|field| field == "extensionApi")
    );
    assert!(
        !response["properties"]["result"]["required"]
            .as_array()
            .expect("hello required fields")
            .iter()
            .any(|field| field == "configSchema")
    );
    assert_eq!(
        response["properties"]["result"]["properties"]["configSchema"]["const"],
        alcomd_protocol::CONFIG_SCHEMA_VERSION
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
        active_only: false,
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

#[test]
fn repository_list_approved_wire_shapes_are_closed_and_preserve_query_text() {
    use alcomd_protocol::*;
    let contract = schema("m7-repository-list");
    for definition in [
        "inspectListFileParams",
        "previewListExportParams",
        "exportListFileParams",
        "inspectListFileResult",
        "importListFileParams",
        "importListFileResult",
        "cancelListRegistrationParams",
        "cancelListRegistrationResult",
        "previewListExportResult",
        "exportListFileResult",
    ] {
        assert_eq!(contract["$defs"][definition]["additionalProperties"], false);
    }
    assert_eq!(
        CAPABILITY_REPOSITORIES_LIST_FILES_V1,
        "repositories.list-files.v1"
    );
    assert_eq!(
        CAPABILITY_REPOSITORIES_LIST_IMPORT_V1,
        "repositories.list-import.v1"
    );
    assert_eq!(
        METHOD_REPOSITORIES_IMPORT_LIST_FILE,
        "repositories.importListFile"
    );
    assert_eq!(
        METHOD_REPOSITORIES_CANCEL_LIST_REGISTRATION,
        "repositories.cancelListRegistration"
    );
    assert_eq!(
        contract["$defs"]["importListFileParams"]["required"],
        json!(["path", "expectedFingerprint", "idempotencyKey"])
    );
    assert_eq!(
        contract["$defs"]["importListFileResult"]["properties"]["items"]["maxItems"],
        4096
    );
    assert_eq!(
        contract["$defs"]["inspectListFileResult"]["required"],
        json!(["entries", "ignoredLines", "fingerprint"])
    );
    assert_eq!(
        contract["$defs"]["cancelListRegistrationParams"]["required"],
        json!(["idempotencyKey"])
    );
    assert_eq!(
        serde_json::to_value(RepositoryListImportParams {
            path: "C:/list.txt".to_owned(),
            expected_fingerprint: "a".repeat(64),
            idempotency_key: "import-key".to_owned(),
        })
        .expect("import params wire"),
        json!({"path":"C:/list.txt","expectedFingerprint":"a".repeat(64),"idempotencyKey":"import-key"})
    );
    assert_eq!(
        serde_json::to_value(RepositoryListCancelResult { stopped: true }).expect("cancel wire"),
        json!({"stopped":true})
    );
    assert_eq!(
        serde_json::to_value(RepositoryListImportItem::Failed {
            line: 7,
            code: "repository_unavailable".to_owned()
        })
        .expect("import failure wire"),
        json!({"line":7,"status":"failed","code":"repository_unavailable"})
    );
    assert!(
        serde_json::from_value::<RepositoryListImportItem>(
            json!({"line":7,"status":"added","code":"unexpected"})
        )
        .is_err()
    );
    assert!(serde_json::from_value::<RepositoryListImportItem>(json!({"status":"added"})).is_err());
    assert!(
        serde_json::from_value::<RepositoryListInspection>(json!({"entries":[],"ignoredLines":0}))
            .is_err()
    );
    assert_eq!(
        METHOD_REPOSITORIES_INSPECT_LIST_FILE,
        "repositories.inspectListFile"
    );
    assert_eq!(
        METHOD_REPOSITORIES_PREVIEW_LIST_EXPORT,
        "repositories.previewListExport"
    );
    assert_eq!(
        METHOD_REPOSITORIES_EXPORT_LIST_FILE,
        "repositories.exportListFile"
    );
    assert!(serde_json::from_value::<RepositoryListExportParams>(json!({"targetPath":"C:/list.txt", "expectedFingerprint":"a".repeat(64), "overwrite":true})).is_err());
    assert!(
        serde_json::from_value::<RepositoryListInspectParams>(
            json!({"path":"C:/list.txt", "register":true})
        )
        .is_err()
    );
    let entry = RepositoryListEntry {
        line: 1,
        credentials_configured: false,
        status: RepositoryListEntryStatus::Candidate,
        source: Some(RepositoryListSource::Remote {
            url: "https://example.test/repo?z=2&a=1&a=3".into(),
        }),
        reason: None,
    };
    assert_eq!(
        serde_json::to_value(entry).expect("repository list entry wire"),
        json!({"line":1,"status":"candidate","source":{"kind":"remote","url":"https://example.test/repo?z=2&a=1&a=3"}})
    );
}

#[test]
fn repository_credentials_are_write_only_and_clear_is_explicit() {
    use alcomd_protocol::RepositorySetCredentialsParams;
    let mut wire =
        json!({"repositoryId":"fixture","expectedRevision":1,"idempotencyKey":"fixture"});
    assert!(serde_json::from_value::<RepositorySetCredentialsParams>(wire.clone()).is_err());
    wire["credentials"] = Value::Null;
    assert!(
        serde_json::from_value::<RepositorySetCredentialsParams>(wire.clone())
            .expect("explicit clear")
            .credentials
            .is_none()
    );
    wire["credentials"] = json!({"headers":{"Authorization":"Bearer test-token"},"allowedOrigins":["https://example.test"]});
    assert!(serde_json::from_value::<RepositorySetCredentialsParams>(wire.clone()).is_err());
    wire["credentials"] = json!({"headers":{"Authorization":"Bearer test-token"}});
    let configured =
        serde_json::from_value::<RepositorySetCredentialsParams>(wire).expect("credential write");
    assert!(!format!("{configured:?}").contains("test-token"));
}

#[test]
fn repository_display_name_contract_requires_an_explicit_value_or_clear() {
    use alcomd_protocol::{METHOD_REPOSITORIES_SET_DISPLAY_NAME, RepositorySetDisplayNameParams};
    assert_eq!(
        METHOD_REPOSITORIES_SET_DISPLAY_NAME,
        "repositories.setDisplayName"
    );
    let contract = schema("m3-project-repository");
    let params = &contract["$defs"]["repositorySetDisplayNameParams"];
    assert_eq!(
        params["required"],
        json!([
            "repositoryId",
            "expectedRevision",
            "idempotencyKey",
            "displayName"
        ])
    );
    assert_eq!(params["properties"]["displayName"]["maxLength"], 128);
    assert_eq!(
        contract["$defs"]["repository"]["properties"]["displayName"]["maxLength"],
        128
    );
    let mut wire =
        json!({"repositoryId":"fixture","expectedRevision":1,"idempotencyKey":"fixture"});
    assert!(serde_json::from_value::<RepositorySetDisplayNameParams>(wire.clone()).is_err());
    for name in [Value::Null, json!("VRChat SDK")] {
        wire["displayName"] = name;
        let typed: RepositorySetDisplayNameParams =
            serde_json::from_value(wire.clone()).expect("display name update");
        assert_eq!(
            serde_json::to_value(typed).expect("serialize display name update"),
            wire
        );
    }
}

#[test]
fn config10_fullscreen_is_required_in_snapshots_and_optional_nonnullable_in_narrow_updates() {
    use alcomd_protocol::{SettingsUpdate, WindowSettings};
    for fullscreen in [false, true] {
        let wire = json!({"window": {"fullscreen": fullscreen}});
        let typed: SettingsUpdate = serde_json::from_value(wire.clone()).expect("fullscreen patch");
        let window = typed.window.expect("window patch");
        assert_eq!(window.fullscreen, Some(fullscreen));
        assert_eq!(window.remember_state, None);
        assert_eq!(serde_json::to_value(typed).expect("roundtrip"), wire);
        let complete: WindowSettings =
            serde_json::from_value(json!({"rememberState":true,"fullscreen":fullscreen}))
                .expect("complete snapshot");
        assert!(complete.remember_state);
        assert_eq!(complete.fullscreen, fullscreen);
    }
    for invalid in [
        Value::Null,
        json!({}),
        json!({"fullscreen":null}),
        json!({"fullscreen":"true"}),
        json!({"fullscreen":1}),
        json!({"fullscreen":true,"width":1000}),
    ] {
        assert!(
            serde_json::from_value::<SettingsUpdate>(json!({"window":invalid,"locale":"ja-JP"}))
                .is_err()
        );
    }
    for invalid in [
        json!({"rememberState":true}),
        json!({"fullscreen":false}),
        json!({"rememberState":false,"fullscreen":null}),
    ] {
        assert!(serde_json::from_value::<WindowSettings>(invalid).is_err());
    }
    let historical: Value = serde_json::from_str(include_str!(
        "../../../specs/config/settings-v9.schema.json"
    ))
    .expect("frozen Config9");
    assert_eq!(historical["properties"]["schema"]["const"], 9);
    assert!(
        historical["$defs"]["windowSettings"]["properties"]
            .get("fullscreen")
            .is_none()
    );
}
