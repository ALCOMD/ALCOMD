//! M7 Portable UI RPC v1 data-transfer objects.

use serde::{Deserialize, Serialize};

pub const PORTABLE_UI_IDLE_TIMEOUT_MS: u64 = 300_000;
pub const PORTABLE_UI_ABSOLUTE_TIMEOUT_MS: u64 = 3_600_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExtensionUiProtocol {
    PortableV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionUiDeclaration {
    pub protocol: ExtensionUiProtocol,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionUiOpenParams {
    pub extension_id: String,
    pub locale: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionUiRefreshParams {
    pub session_id: String,
    pub expected_snapshot_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionUiDispatchParams {
    pub session_id: String,
    pub expected_snapshot_revision: u64,
    pub sequence: u64,
    pub request_id: String,
    pub action: UiAction,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionUiCloseParams {
    pub session_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionUiSession {
    pub session_id: String,
    pub extension_id: String,
    pub locale: String,
    pub idle_timeout_ms: u64,
    pub absolute_timeout_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionUiOpenResult {
    pub session: ExtensionUiSession,
    pub snapshot: UiSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionUiSnapshotResult {
    pub snapshot: UiSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionUiDispatchResult {
    pub snapshot: UiSnapshot,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionUiCloseResult {
    pub closed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiSnapshot {
    pub session_id: String,
    pub snapshot_revision: u64,
    pub document: UiDocument,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiDocument {
    pub protocol: ExtensionUiProtocol,
    pub title: String,
    pub nodes: Vec<UiNode>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum UiNode {
    Page {
        #[serde(rename = "nodeId")]
        node_id: String,
        order: u16,
        payload: UiPagePayload,
    },
    Section {
        #[serde(rename = "nodeId")]
        node_id: String,
        #[serde(rename = "parentId")]
        parent_id: String,
        order: u16,
        payload: UiSectionPayload,
    },
    Stack {
        #[serde(rename = "nodeId")]
        node_id: String,
        #[serde(rename = "parentId")]
        parent_id: String,
        order: u16,
        payload: UiStackPayload,
    },
    Group {
        #[serde(rename = "nodeId")]
        node_id: String,
        #[serde(rename = "parentId")]
        parent_id: String,
        order: u16,
        payload: UiOptionalLabelPayload,
    },
    Form {
        #[serde(rename = "nodeId")]
        node_id: String,
        #[serde(rename = "parentId")]
        parent_id: String,
        order: u16,
        payload: UiFormPayload,
    },
    List {
        #[serde(rename = "nodeId")]
        node_id: String,
        #[serde(rename = "parentId")]
        parent_id: String,
        order: u16,
        payload: UiOptionalLabelPayload,
    },
    ListItem {
        #[serde(rename = "nodeId")]
        node_id: String,
        #[serde(rename = "parentId")]
        parent_id: String,
        order: u16,
        payload: UiOptionalLabelPayload,
    },
    Text {
        #[serde(rename = "nodeId")]
        node_id: String,
        #[serde(rename = "parentId")]
        parent_id: String,
        order: u16,
        payload: UiTextPayload,
    },
    Status {
        #[serde(rename = "nodeId")]
        node_id: String,
        #[serde(rename = "parentId")]
        parent_id: String,
        order: u16,
        payload: UiStatusPayload,
    },
    KeyValue {
        #[serde(rename = "nodeId")]
        node_id: String,
        #[serde(rename = "parentId")]
        parent_id: String,
        order: u16,
        payload: UiKeyValuePayload,
    },
    Progress {
        #[serde(rename = "nodeId")]
        node_id: String,
        #[serde(rename = "parentId")]
        parent_id: String,
        order: u16,
        payload: UiProgressPayload,
    },
    Divider {
        #[serde(rename = "nodeId")]
        node_id: String,
        #[serde(rename = "parentId")]
        parent_id: String,
        order: u16,
        payload: UiEmptyPayload,
    },
    Button {
        #[serde(rename = "nodeId")]
        node_id: String,
        #[serde(rename = "parentId")]
        parent_id: String,
        order: u16,
        payload: UiButtonPayload,
    },
    Switch {
        #[serde(rename = "nodeId")]
        node_id: String,
        #[serde(rename = "parentId")]
        parent_id: String,
        order: u16,
        payload: UiSwitchPayload,
    },
    TextField {
        #[serde(rename = "nodeId")]
        node_id: String,
        #[serde(rename = "parentId")]
        parent_id: String,
        order: u16,
        payload: UiTextFieldPayload,
    },
    IntegerField {
        #[serde(rename = "nodeId")]
        node_id: String,
        #[serde(rename = "parentId")]
        parent_id: String,
        order: u16,
        payload: UiIntegerFieldPayload,
    },
    Select {
        #[serde(rename = "nodeId")]
        node_id: String,
        #[serde(rename = "parentId")]
        parent_id: String,
        order: u16,
        payload: UiSelectPayload,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiPagePayload {
    pub title: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiSectionPayload {
    pub label: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiStackOrientation {
    Vertical,
    Horizontal,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiStackPayload {
    pub orientation: UiStackOrientation,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiOptionalLabelPayload {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiFormPayload {
    pub submit_action_id: String,
    pub submit_label: String,
    pub disabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UiTone {
    Neutral,
    Info,
    Success,
    Warning,
    Danger,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiTextPayload {
    pub text: String,
    pub tone: UiTone,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiStatusPayload {
    pub label: String,
    pub tone: UiTone,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiKeyValuePayload {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "lowercase", deny_unknown_fields)]
pub enum UiProgressValue {
    Indeterminate,
    Determinate {
        #[serde(rename = "basisPoints")]
        basis_points: u16,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiProgressPayload {
    pub label: String,
    pub value: UiProgressValue,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiEmptyPayload {}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiButtonPayload {
    pub label: String,
    pub action_id: String,
    pub disabled: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "lowercase", deny_unknown_fields)]
pub enum UiValidation {
    Valid,
    Invalid { message: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiSwitchPayload {
    pub field_id: String,
    pub label: String,
    pub initial_value: bool,
    pub disabled: bool,
    pub read_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<UiValidation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiTextFieldPayload {
    pub field_id: String,
    pub label: String,
    pub initial_value: String,
    pub required: bool,
    pub min_length: u16,
    pub max_length: u16,
    pub multiline: bool,
    pub disabled: bool,
    pub read_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<UiValidation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiIntegerFieldPayload {
    pub field_id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_value: Option<i64>,
    pub required: bool,
    pub minimum: i64,
    pub maximum: i64,
    pub disabled: bool,
    pub read_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<UiValidation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiSelectOption {
    pub option_id: String,
    pub label: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiSelectPayload {
    pub field_id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_option_id: Option<String>,
    pub required: bool,
    pub options: Vec<UiSelectOption>,
    pub disabled: bool,
    pub read_only: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation: Option<UiValidation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum UiFieldValue {
    Boolean { value: bool },
    Text { value: String },
    Integer { value: i64 },
    Selection { value: String },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UiSubmittedField {
    pub field_id: String,
    pub value: UiFieldValue,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum UiAction {
    Activate {
        #[serde(rename = "actionId")]
        action_id: String,
    },
    SubmitForm {
        #[serde(rename = "actionId")]
        action_id: String,
        values: Vec<UiSubmittedField>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn portable_ui_dtos_use_closed_tagged_shapes() {
        let action: UiAction = serde_json::from_value(json!({
            "kind": "submit-form",
            "actionId": "save",
            "values": [{"fieldId": "name", "value": {"kind": "text", "value": "A"}}]
        }))
        .expect("parse action");
        assert!(matches!(action, UiAction::SubmitForm { .. }));

        assert!(
            serde_json::from_value::<UiAction>(json!({
                "kind": "activate",
                "actionId": "save",
                "extra": true
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<UiNode>(json!({
                "kind": "webview",
                "nodeId": "root",
                "order": 0,
                "payload": {}
            }))
            .is_err()
        );
    }

    #[test]
    fn ui_rpc_params_reject_unknown_fields() {
        assert!(
            serde_json::from_value::<ExtensionUiOpenParams>(json!({
                "extensionId": "dev.example.extension",
                "locale": "en-US",
                "path": "C:/private"
            }))
            .is_err()
        );
    }
}
