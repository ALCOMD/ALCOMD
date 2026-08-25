use std::collections::BTreeSet;

use alcomd_protocol::{
    PortableUiValidationError, UiAction, UiDocument, UiFieldValue, UiNode, UiSubmittedField,
};
use serde::Serialize;
use serde_json::{Value, json};

const MCP_DOCUMENT: &str = include_str!("../fixtures/m7/mcp-management-snapshot.json");
const DISCORD_DOCUMENT: &str = include_str!("../fixtures/m7/discord-presence-snapshot.json");
const CONFORMANCE: &str = include_str!("../fixtures/m7/headless-renderer-conformance.json");

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SemanticDocument {
    protocol: &'static str,
    title: String,
    nodes: Vec<SemanticNode>,
    action_ids: Vec<String>,
    field_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct SemanticNode {
    kind: &'static str,
    node_id: String,
    parent_id: Option<String>,
    order: u64,
    payload: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ConsumerError {
    ProtocolUnsupported,
    DocumentInvalid,
    LimitExceeded,
}

#[test]
fn independent_headless_consumer_matches_shared_fixture_semantics() {
    let contract: Value = serde_json::from_str(CONFORMANCE).expect("conformance fixture");
    let mut all_kinds = BTreeSet::new();
    for (index, source) in [MCP_DOCUMENT, DISCORD_DOCUMENT].into_iter().enumerate() {
        let document = parse_document(source).expect("public Portable UI document");
        let first = semantic_summary(&document).expect("semantic summary");
        let second = semantic_summary(&document).expect("deterministic semantic summary");
        assert_eq!(first, second);

        let expected = &contract["fixtures"][index];
        assert_eq!(
            first.nodes.len(),
            expected["nodeCount"].as_u64().expect("node count") as usize
        );
        let kinds = first.nodes.iter().map(|node| node.kind).collect::<Vec<_>>();
        assert_eq!(kinds, strings(&expected["orderedKinds"]));
        assert_eq!(first.action_ids, strings(&expected["actionIds"]));
        assert_eq!(first.field_ids, strings(&expected["fieldIds"]));
        all_kinds.extend(kinds);
    }
    assert_eq!(
        all_kinds,
        strings(&contract["requiredNodeKinds"])
            .into_iter()
            .collect::<BTreeSet<_>>()
    );
    assert_eq!(contract["tauriDependency"], false);
    assert_eq!(contract["guiDependency"], false);
}

#[test]
fn headless_consumer_validates_both_action_kinds_and_typed_complete_forms() {
    let mcp = parse_document(MCP_DOCUMENT).expect("MCP document");
    let activate = UiAction::Activate {
        action_id: "refresh-connections".to_owned(),
    };
    assert_eq!(action_kind(&activate), "activate");
    assert_eq!(mcp.validate_action(&activate), Ok(()));

    let submit = UiAction::SubmitForm {
        action_id: "apply-client-action".to_owned(),
        values: vec![
            UiSubmittedField {
                field_id: "client-id".to_owned(),
                value: UiFieldValue::Text {
                    value: "client-01".to_owned(),
                },
            },
            UiSubmittedField {
                field_id: "client-action".to_owned(),
                value: UiFieldValue::Selection {
                    value: "disable".to_owned(),
                },
            },
        ],
    };
    assert_eq!(action_kind(&submit), "submit-form");
    assert_eq!(mcp.validate_action(&submit), Ok(()));

    let incomplete = UiAction::SubmitForm {
        action_id: "apply-client-action".to_owned(),
        values: vec![UiSubmittedField {
            field_id: "client-id".to_owned(),
            value: UiFieldValue::Text {
                value: "client-01".to_owned(),
            },
        }],
    };
    assert_eq!(
        mcp.validate_action(&incomplete),
        Err(PortableUiValidationError::Invalid)
    );
}

#[test]
fn headless_consumer_fails_closed_for_unknown_protocol_node_and_malformed_tree() {
    let unknown_protocol = MCP_DOCUMENT.replacen("portable-v1", "portable-v2", 1);
    assert_eq!(
        parse_document(&unknown_protocol),
        Err(ConsumerError::ProtocolUnsupported)
    );

    let unknown_node = MCP_DOCUMENT.replacen("\"section\"", "\"custom-html\"", 1);
    assert_eq!(
        parse_document(&unknown_node),
        Err(ConsumerError::DocumentInvalid)
    );

    let mut missing_parent: Value = serde_json::from_str(MCP_DOCUMENT).expect("fixture value");
    missing_parent["nodes"][1]["parentId"] = json!("missing");
    assert_eq!(
        parse_document(&serde_json::to_string(&missing_parent).expect("malformed tree")),
        Err(ConsumerError::DocumentInvalid)
    );
}

#[test]
fn production_validator_rejects_hostile_document_and_action_matrix() {
    let base: Value = serde_json::from_str(MCP_DOCUMENT).expect("MCP value");
    let mut duplicate_node = base.clone();
    duplicate_node["nodes"][1]["nodeId"] = json!("root");
    let mut duplicate_action = base.clone();
    duplicate_action["nodes"][12]["payload"]["submitActionId"] = json!("refresh-connections");
    let mut duplicate_field = base.clone();
    duplicate_field["nodes"][14]["payload"]["fieldId"] = json!("client-id");
    let mut later_parent = base.clone();
    later_parent["nodes"][1]["parentId"] = json!("connections");
    let mut unknown_payload = base.clone();
    unknown_payload["nodes"][1]["payload"]["html"] = json!("<script>");
    let mut forbidden_text = base.clone();
    forbidden_text["nodes"][8]["payload"]["text"] = json!("safe\u{202e}spoof");
    let mut validation_limit = base.clone();
    validation_limit["nodes"][13]["payload"]["validation"] = json!({
        "state": "invalid",
        "message": "é".repeat(257)
    });

    for value in [
        duplicate_node,
        duplicate_action,
        duplicate_field,
        later_parent,
        unknown_payload,
        forbidden_text,
    ] {
        assert_eq!(
            validate_document_value(value),
            Err(ConsumerError::DocumentInvalid)
        );
    }
    assert_eq!(
        validate_document_value(validation_limit),
        Err(ConsumerError::LimitExceeded)
    );

    let depth_nine = json!({
        "protocol": "portable-v1",
        "title": "depth",
        "nodes": (0..9).map(|index| {
            if index == 0 {
                json!({"kind":"page","nodeId":"root","order":0,"payload":{"title":"root"}})
            } else {
                json!({
                    "kind":"group",
                    "nodeId":format!("group-{index}"),
                    "parentId":if index == 1 { "root".to_owned() } else { format!("group-{}", index - 1) },
                    "order":0,
                    "payload":{}
                })
            }
        }).collect::<Vec<_>>()
    });
    assert_eq!(
        validate_document_value(depth_nine),
        Err(ConsumerError::LimitExceeded)
    );

    let mut many_nodes = vec![json!({
        "kind":"page","nodeId":"root","order":0,"payload":{"title":"root"}
    })];
    many_nodes.extend((0..256).map(|index| {
        json!({
            "kind":"text",
            "nodeId":format!("text-{index}"),
            "parentId":"root",
            "order":index,
            "payload":{"text":"bounded","tone":"neutral"}
        })
    }));
    assert_eq!(
        validate_document_value(json!({
            "protocol":"portable-v1","title":"many","nodes":many_nodes
        })),
        Err(ConsumerError::LimitExceeded)
    );

    let document = parse_document(MCP_DOCUMENT).expect("valid action document");
    let valid_values = [
        UiSubmittedField {
            field_id: "client-id".to_owned(),
            value: UiFieldValue::Text {
                value: "client-01".to_owned(),
            },
        },
        UiSubmittedField {
            field_id: "client-action".to_owned(),
            value: UiFieldValue::Selection {
                value: "disable".to_owned(),
            },
        },
    ];
    let invalid_actions = [
        UiAction::Activate {
            action_id: "unknown-action".to_owned(),
        },
        UiAction::SubmitForm {
            action_id: "apply-client-action".to_owned(),
            values: valid_values[..1].to_vec(),
        },
        UiAction::SubmitForm {
            action_id: "apply-client-action".to_owned(),
            values: vec![
                UiSubmittedField {
                    field_id: "client-id".to_owned(),
                    value: UiFieldValue::Boolean { value: true },
                },
                valid_values[1].clone(),
            ],
        },
        UiAction::SubmitForm {
            action_id: "apply-client-action".to_owned(),
            values: vec![
                valid_values[0].clone(),
                UiSubmittedField {
                    field_id: "client-action".to_owned(),
                    value: UiFieldValue::Selection {
                        value: "unknown".to_owned(),
                    },
                },
            ],
        },
        UiAction::SubmitForm {
            action_id: "apply-client-action".to_owned(),
            values: vec![
                valid_values[0].clone(),
                valid_values[1].clone(),
                UiSubmittedField {
                    field_id: "extra".to_owned(),
                    value: UiFieldValue::Text {
                        value: "extra".to_owned(),
                    },
                },
            ],
        },
    ];
    for action in invalid_actions {
        assert_eq!(
            document.validate_action(&action),
            Err(PortableUiValidationError::Invalid)
        );
    }
    let oversized = UiAction::SubmitForm {
        action_id: "apply-client-action".to_owned(),
        values: vec![
            UiSubmittedField {
                field_id: "client-id".to_owned(),
                value: UiFieldValue::Text {
                    value: "x".repeat(65_536),
                },
            },
            valid_values[1].clone(),
        ],
    };
    assert_eq!(
        document.validate_action(&oversized),
        Err(PortableUiValidationError::LimitExceeded)
    );
}

fn parse_document(source: &str) -> Result<UiDocument, ConsumerError> {
    let value: Value = serde_json::from_str(source).map_err(|_| ConsumerError::DocumentInvalid)?;
    if value.get("protocol").and_then(Value::as_str) != Some("portable-v1") {
        return Err(ConsumerError::ProtocolUnsupported);
    }
    let document: UiDocument =
        serde_json::from_value(value).map_err(|_| ConsumerError::DocumentInvalid)?;
    document.validate().map_err(|error| match error {
        PortableUiValidationError::Invalid => ConsumerError::DocumentInvalid,
        PortableUiValidationError::LimitExceeded => ConsumerError::LimitExceeded,
    })?;
    Ok(document)
}

fn validate_document_value(value: Value) -> Result<(), ConsumerError> {
    let document: UiDocument =
        serde_json::from_value(value).map_err(|_| ConsumerError::DocumentInvalid)?;
    document.validate().map_err(|error| match error {
        PortableUiValidationError::Invalid => ConsumerError::DocumentInvalid,
        PortableUiValidationError::LimitExceeded => ConsumerError::LimitExceeded,
    })
}

fn semantic_summary(document: &UiDocument) -> Result<SemanticDocument, ConsumerError> {
    document.validate().map_err(|error| match error {
        PortableUiValidationError::Invalid => ConsumerError::DocumentInvalid,
        PortableUiValidationError::LimitExceeded => ConsumerError::LimitExceeded,
    })?;
    let mut nodes = Vec::with_capacity(document.nodes.len());
    let mut action_ids = Vec::new();
    let mut field_ids = Vec::new();
    for node in &document.nodes {
        let encoded = serde_json::to_value(node).map_err(|_| ConsumerError::DocumentInvalid)?;
        let payload = encoded
            .get("payload")
            .cloned()
            .ok_or(ConsumerError::DocumentInvalid)?;
        if let Some(action_id) = payload
            .get("actionId")
            .or_else(|| payload.get("submitActionId"))
            .and_then(Value::as_str)
        {
            action_ids.push(action_id.to_owned());
        }
        if let Some(field_id) = payload.get("fieldId").and_then(Value::as_str) {
            field_ids.push(field_id.to_owned());
        }
        nodes.push(SemanticNode {
            kind: node_kind(node),
            node_id: required_string(&encoded, "nodeId")?,
            parent_id: encoded
                .get("parentId")
                .and_then(Value::as_str)
                .map(str::to_owned),
            order: encoded
                .get("order")
                .and_then(Value::as_u64)
                .ok_or(ConsumerError::DocumentInvalid)?,
            payload,
        });
    }
    Ok(SemanticDocument {
        protocol: "portable-v1",
        title: document.title.clone(),
        nodes,
        action_ids,
        field_ids,
    })
}

const fn node_kind(node: &UiNode) -> &'static str {
    match node {
        UiNode::Page { .. } => "page",
        UiNode::Section { .. } => "section",
        UiNode::Stack { .. } => "stack",
        UiNode::Group { .. } => "group",
        UiNode::Form { .. } => "form",
        UiNode::List { .. } => "list",
        UiNode::ListItem { .. } => "list-item",
        UiNode::Text { .. } => "text",
        UiNode::Status { .. } => "status",
        UiNode::KeyValue { .. } => "key-value",
        UiNode::Progress { .. } => "progress",
        UiNode::Divider { .. } => "divider",
        UiNode::Button { .. } => "button",
        UiNode::Switch { .. } => "switch",
        UiNode::TextField { .. } => "text-field",
        UiNode::IntegerField { .. } => "integer-field",
        UiNode::Select { .. } => "select",
    }
}

const fn action_kind(action: &UiAction) -> &'static str {
    match action {
        UiAction::Activate { .. } => "activate",
        UiAction::SubmitForm { .. } => "submit-form",
    }
}

fn required_string(value: &Value, field: &str) -> Result<String, ConsumerError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(ConsumerError::DocumentInvalid)
}

fn strings(value: &Value) -> Vec<&str> {
    value
        .as_array()
        .expect("string array")
        .iter()
        .map(|entry| entry.as_str().expect("string"))
        .collect()
}
