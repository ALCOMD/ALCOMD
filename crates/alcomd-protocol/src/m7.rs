//! M7 Portable UI RPC v1 data-transfer objects.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

pub const PORTABLE_UI_IDLE_TIMEOUT_MS: u64 = 300_000;
pub const PORTABLE_UI_ABSOLUTE_TIMEOUT_MS: u64 = 3_600_000;
pub const PORTABLE_UI_SNAPSHOT_BYTES: usize = 262_144;
pub const PORTABLE_UI_DISPATCH_BYTES: usize = 65_536;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortableUiValidationError {
    Invalid,
    LimitExceeded,
}

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

impl UiDocument {
    pub fn validate(&self) -> Result<(), PortableUiValidationError> {
        if serde_json::to_vec(self)
            .map_or(true, |encoded| encoded.len() > PORTABLE_UI_SNAPSHOT_BYTES)
        {
            return Err(PortableUiValidationError::LimitExceeded);
        }
        if self.nodes.is_empty() {
            return Err(PortableUiValidationError::Invalid);
        }
        if self.nodes.len() > 256 {
            return Err(PortableUiValidationError::LimitExceeded);
        }
        let mut text_bytes = checked_text(&self.title, false, 4_096)?;
        let mut nodes = HashMap::<&str, NodeEvidence<'_>>::new();
        let mut node_ids = HashSet::new();
        let mut field_ids = HashSet::new();
        let mut action_ids = HashSet::new();
        let mut option_ids = HashSet::new();
        let mut sibling_orders = HashMap::<Option<&str>, u16>::new();
        let mut field_count = 0_usize;

        for (index, node) in self.nodes.iter().enumerate() {
            let id = node.node_id();
            if !valid_id(id) || !node_ids.insert(id) {
                return Err(PortableUiValidationError::Invalid);
            }
            let parent_id = node.parent_id();
            let expected_order = sibling_orders.entry(parent_id).or_default();
            if node.order() != *expected_order {
                return Err(PortableUiValidationError::Invalid);
            }
            *expected_order = expected_order
                .checked_add(1)
                .ok_or(PortableUiValidationError::LimitExceeded)?;

            let kind = node.kind();
            let (depth, form_ancestor) = if index == 0 {
                if kind != NodeKind::Page || parent_id.is_some() {
                    return Err(PortableUiValidationError::Invalid);
                }
                (1, None)
            } else {
                let parent_id = parent_id.ok_or(PortableUiValidationError::Invalid)?;
                let parent = nodes
                    .get(parent_id)
                    .ok_or(PortableUiValidationError::Invalid)?;
                if kind == NodeKind::Page || !allows_child(parent.kind, kind) {
                    return Err(PortableUiValidationError::Invalid);
                }
                let depth = parent
                    .depth
                    .checked_add(1)
                    .ok_or(PortableUiValidationError::LimitExceeded)?;
                if depth > 8 {
                    return Err(PortableUiValidationError::LimitExceeded);
                }
                let form = if kind == NodeKind::Form {
                    if parent.form_ancestor.is_some() || parent.kind == NodeKind::Form {
                        return Err(PortableUiValidationError::Invalid);
                    }
                    Some(id)
                } else if parent.kind == NodeKind::Form {
                    Some(parent.id)
                } else {
                    parent.form_ancestor
                };
                (depth, form)
            };
            if kind.is_field() && form_ancestor.is_none() {
                return Err(PortableUiValidationError::Invalid);
            }

            text_bytes = text_bytes
                .checked_add(validate_node_payload(
                    node,
                    &mut field_ids,
                    &mut action_ids,
                    &mut option_ids,
                    &mut field_count,
                )?)
                .ok_or(PortableUiValidationError::LimitExceeded)?;
            if text_bytes > 65_536 {
                return Err(PortableUiValidationError::LimitExceeded);
            }
            nodes.insert(
                id,
                NodeEvidence {
                    id,
                    kind,
                    depth,
                    form_ancestor,
                },
            );
        }
        if self.nodes[0].order() != 0
            || self
                .nodes
                .iter()
                .filter(|node| node.parent_id().is_none())
                .count()
                != 1
        {
            return Err(PortableUiValidationError::Invalid);
        }
        Ok(())
    }

    pub fn validate_action(&self, action: &UiAction) -> Result<(), PortableUiValidationError> {
        self.validate()?;
        if serde_json::to_vec(action)
            .map_or(true, |encoded| encoded.len() > PORTABLE_UI_DISPATCH_BYTES)
        {
            return Err(PortableUiValidationError::LimitExceeded);
        }
        match action {
            UiAction::Activate { action_id } => {
                if !valid_id(action_id) {
                    return Err(PortableUiValidationError::Invalid);
                }
                let matches = self.nodes.iter().filter(|node| {
                    matches!(
                        node,
                        UiNode::Button { payload, .. }
                            if payload.action_id == *action_id && !payload.disabled
                    )
                });
                if matches.count() != 1 {
                    return Err(PortableUiValidationError::Invalid);
                }
            }
            UiAction::SubmitForm { action_id, values } => {
                if !valid_id(action_id) || values.len() > 64 {
                    return Err(PortableUiValidationError::Invalid);
                }
                let form_id = self
                    .nodes
                    .iter()
                    .find_map(|node| match node {
                        UiNode::Form {
                            node_id, payload, ..
                        } if payload.submit_action_id == *action_id && !payload.disabled => {
                            Some(node_id.as_str())
                        }
                        _ => None,
                    })
                    .ok_or(PortableUiValidationError::Invalid)?;
                let expected = self.editable_fields(form_id)?;
                if expected.len() != values.len() {
                    return Err(PortableUiValidationError::Invalid);
                }
                for (node, submitted) in expected.into_iter().zip(values) {
                    if node.field_id() != Some(submitted.field_id.as_str())
                        || !node.accepts_value(&submitted.value)
                    {
                        return Err(PortableUiValidationError::Invalid);
                    }
                }
            }
        }
        Ok(())
    }

    fn editable_fields(&self, form_id: &str) -> Result<Vec<&UiNode>, PortableUiValidationError> {
        let mut parents = HashMap::<&str, Option<&str>>::new();
        let mut result = Vec::new();
        for node in &self.nodes {
            parents.insert(node.node_id(), node.parent_id());
            if !node.kind().is_field() || !node.is_editable() {
                continue;
            }
            let mut parent = node.parent_id();
            let mut owner = None;
            while let Some(parent_id) = parent {
                if parent_id == form_id {
                    owner = Some(parent_id);
                    break;
                }
                parent = *parents
                    .get(parent_id)
                    .ok_or(PortableUiValidationError::Invalid)?;
            }
            if owner.is_some() {
                result.push(node);
            }
        }
        Ok(result)
    }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NodeKind {
    Page,
    Section,
    Stack,
    Group,
    Form,
    List,
    ListItem,
    Text,
    Status,
    KeyValue,
    Progress,
    Divider,
    Button,
    Switch,
    TextField,
    IntegerField,
    Select,
}

impl NodeKind {
    const fn is_field(self) -> bool {
        matches!(
            self,
            Self::Switch | Self::TextField | Self::IntegerField | Self::Select
        )
    }
}

struct NodeEvidence<'a> {
    id: &'a str,
    kind: NodeKind,
    depth: u8,
    form_ancestor: Option<&'a str>,
}

impl UiNode {
    fn node_id(&self) -> &str {
        match self {
            Self::Page { node_id, .. }
            | Self::Section { node_id, .. }
            | Self::Stack { node_id, .. }
            | Self::Group { node_id, .. }
            | Self::Form { node_id, .. }
            | Self::List { node_id, .. }
            | Self::ListItem { node_id, .. }
            | Self::Text { node_id, .. }
            | Self::Status { node_id, .. }
            | Self::KeyValue { node_id, .. }
            | Self::Progress { node_id, .. }
            | Self::Divider { node_id, .. }
            | Self::Button { node_id, .. }
            | Self::Switch { node_id, .. }
            | Self::TextField { node_id, .. }
            | Self::IntegerField { node_id, .. }
            | Self::Select { node_id, .. } => node_id,
        }
    }

    fn parent_id(&self) -> Option<&str> {
        match self {
            Self::Page { .. } => None,
            Self::Section { parent_id, .. }
            | Self::Stack { parent_id, .. }
            | Self::Group { parent_id, .. }
            | Self::Form { parent_id, .. }
            | Self::List { parent_id, .. }
            | Self::ListItem { parent_id, .. }
            | Self::Text { parent_id, .. }
            | Self::Status { parent_id, .. }
            | Self::KeyValue { parent_id, .. }
            | Self::Progress { parent_id, .. }
            | Self::Divider { parent_id, .. }
            | Self::Button { parent_id, .. }
            | Self::Switch { parent_id, .. }
            | Self::TextField { parent_id, .. }
            | Self::IntegerField { parent_id, .. }
            | Self::Select { parent_id, .. } => Some(parent_id),
        }
    }

    const fn order(&self) -> u16 {
        match self {
            Self::Page { order, .. }
            | Self::Section { order, .. }
            | Self::Stack { order, .. }
            | Self::Group { order, .. }
            | Self::Form { order, .. }
            | Self::List { order, .. }
            | Self::ListItem { order, .. }
            | Self::Text { order, .. }
            | Self::Status { order, .. }
            | Self::KeyValue { order, .. }
            | Self::Progress { order, .. }
            | Self::Divider { order, .. }
            | Self::Button { order, .. }
            | Self::Switch { order, .. }
            | Self::TextField { order, .. }
            | Self::IntegerField { order, .. }
            | Self::Select { order, .. } => *order,
        }
    }

    const fn kind(&self) -> NodeKind {
        match self {
            Self::Page { .. } => NodeKind::Page,
            Self::Section { .. } => NodeKind::Section,
            Self::Stack { .. } => NodeKind::Stack,
            Self::Group { .. } => NodeKind::Group,
            Self::Form { .. } => NodeKind::Form,
            Self::List { .. } => NodeKind::List,
            Self::ListItem { .. } => NodeKind::ListItem,
            Self::Text { .. } => NodeKind::Text,
            Self::Status { .. } => NodeKind::Status,
            Self::KeyValue { .. } => NodeKind::KeyValue,
            Self::Progress { .. } => NodeKind::Progress,
            Self::Divider { .. } => NodeKind::Divider,
            Self::Button { .. } => NodeKind::Button,
            Self::Switch { .. } => NodeKind::Switch,
            Self::TextField { .. } => NodeKind::TextField,
            Self::IntegerField { .. } => NodeKind::IntegerField,
            Self::Select { .. } => NodeKind::Select,
        }
    }

    fn field_id(&self) -> Option<&str> {
        match self {
            Self::Switch { payload, .. } => Some(&payload.field_id),
            Self::TextField { payload, .. } => Some(&payload.field_id),
            Self::IntegerField { payload, .. } => Some(&payload.field_id),
            Self::Select { payload, .. } => Some(&payload.field_id),
            _ => None,
        }
    }

    const fn is_editable(&self) -> bool {
        match self {
            Self::Switch { payload, .. } => !payload.disabled && !payload.read_only,
            Self::TextField { payload, .. } => !payload.disabled && !payload.read_only,
            Self::IntegerField { payload, .. } => !payload.disabled && !payload.read_only,
            Self::Select { payload, .. } => !payload.disabled && !payload.read_only,
            _ => false,
        }
    }

    fn accepts_value(&self, value: &UiFieldValue) -> bool {
        match (self, value) {
            (Self::Switch { .. }, UiFieldValue::Boolean { .. }) => true,
            (Self::TextField { payload, .. }, UiFieldValue::Text { value }) => {
                let length = value.chars().count();
                (!payload.required || !value.is_empty())
                    && length >= usize::from(payload.min_length)
                    && length <= usize::from(payload.max_length)
                    && checked_text(value, payload.multiline, 4_096).is_ok()
            }
            (Self::IntegerField { payload, .. }, UiFieldValue::Integer { value }) => {
                *value >= payload.minimum
                    && *value <= payload.maximum
                    && (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(value)
            }
            (Self::Select { payload, .. }, UiFieldValue::Selection { value }) => payload
                .options
                .iter()
                .any(|option| option.option_id == *value),
            _ => false,
        }
    }
}

fn allows_child(parent: NodeKind, child: NodeKind) -> bool {
    use NodeKind::{
        Button, Divider, Form, Group, IntegerField, KeyValue, List, ListItem, Page, Progress,
        Section, Select, Stack, Status, Switch, Text, TextField,
    };
    match parent {
        Page => matches!(
            child,
            Section
                | Stack
                | Group
                | Form
                | List
                | Text
                | Status
                | KeyValue
                | Progress
                | Divider
                | Button
        ),
        Section | Stack | Group => matches!(
            child,
            Section
                | Stack
                | Group
                | Form
                | List
                | Text
                | Status
                | KeyValue
                | Progress
                | Divider
                | Button
                | Switch
                | TextField
                | IntegerField
                | Select
        ),
        Form => matches!(
            child,
            Section
                | Stack
                | Group
                | Text
                | Status
                | KeyValue
                | Progress
                | Divider
                | Switch
                | TextField
                | IntegerField
                | Select
        ),
        List => child == ListItem,
        ListItem => matches!(
            child,
            Section
                | Stack
                | Group
                | Form
                | List
                | Text
                | Status
                | KeyValue
                | Progress
                | Divider
                | Button
        ),
        Text | Status | KeyValue | Progress | Divider | Button | Switch | TextField
        | IntegerField | Select => false,
    }
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

fn validate_node_payload<'a>(
    node: &'a UiNode,
    field_ids: &mut HashSet<&'a str>,
    action_ids: &mut HashSet<&'a str>,
    option_ids: &mut HashSet<&'a str>,
    field_count: &mut usize,
) -> Result<usize, PortableUiValidationError> {
    let mut total = 0_usize;
    let mut add = |value: &str, allow_lf: bool, maximum: usize| {
        let length = checked_text(value, allow_lf, maximum)?;
        total = total
            .checked_add(length)
            .ok_or(PortableUiValidationError::LimitExceeded)?;
        Ok::<(), PortableUiValidationError>(())
    };
    match node {
        UiNode::Page { payload, .. } => add(&payload.title, false, 4_096)?,
        UiNode::Section { payload, .. } => add(&payload.label, false, 4_096)?,
        UiNode::Stack { .. } | UiNode::Divider { .. } => {}
        UiNode::Group { payload, .. }
        | UiNode::List { payload, .. }
        | UiNode::ListItem { payload, .. } => {
            if let Some(label) = &payload.label {
                add(label, false, 4_096)?;
            }
        }
        UiNode::Form { payload, .. } => {
            if !valid_id(&payload.submit_action_id) || !action_ids.insert(&payload.submit_action_id)
            {
                return Err(PortableUiValidationError::Invalid);
            }
            add(&payload.submit_label, false, 4_096)?;
        }
        UiNode::Text { payload, .. } => add(&payload.text, true, 4_096)?,
        UiNode::Status { payload, .. } => add(&payload.label, false, 4_096)?,
        UiNode::KeyValue { payload, .. } => {
            add(&payload.label, false, 4_096)?;
            add(&payload.value, false, 4_096)?;
        }
        UiNode::Progress { payload, .. } => {
            add(&payload.label, false, 4_096)?;
            if matches!(
                payload.value,
                UiProgressValue::Determinate { basis_points } if basis_points > 10_000
            ) {
                return Err(PortableUiValidationError::Invalid);
            }
        }
        UiNode::Button { payload, .. } => {
            if !valid_id(&payload.action_id) || !action_ids.insert(&payload.action_id) {
                return Err(PortableUiValidationError::Invalid);
            }
            add(&payload.label, false, 4_096)?;
        }
        UiNode::Switch { payload, .. } => {
            register_field(&payload.field_id, field_ids, field_count)?;
            add(&payload.label, false, 4_096)?;
            add_validation(payload.validation.as_ref(), &mut add)?;
        }
        UiNode::TextField { payload, .. } => {
            register_field(&payload.field_id, field_ids, field_count)?;
            if payload.min_length > payload.max_length
                || usize::from(payload.max_length) > 4_096
                || (!payload.multiline && payload.initial_value.contains('\n'))
            {
                return Err(PortableUiValidationError::Invalid);
            }
            let initial_length = payload.initial_value.chars().count();
            if initial_length > usize::from(payload.max_length) {
                return Err(PortableUiValidationError::Invalid);
            }
            add(&payload.label, false, 4_096)?;
            add(&payload.initial_value, payload.multiline, 4_096)?;
            add_validation(payload.validation.as_ref(), &mut add)?;
        }
        UiNode::IntegerField { payload, .. } => {
            register_field(&payload.field_id, field_ids, field_count)?;
            if payload.minimum > payload.maximum
                || !(-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&payload.minimum)
                || !(-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&payload.maximum)
                || payload
                    .initial_value
                    .is_some_and(|value| value < payload.minimum || value > payload.maximum)
            {
                return Err(PortableUiValidationError::Invalid);
            }
            add(&payload.label, false, 4_096)?;
            add_validation(payload.validation.as_ref(), &mut add)?;
        }
        UiNode::Select { payload, .. } => {
            register_field(&payload.field_id, field_ids, field_count)?;
            if payload.options.is_empty() || payload.options.len() > 64 {
                return Err(PortableUiValidationError::LimitExceeded);
            }
            add(&payload.label, false, 4_096)?;
            for option in &payload.options {
                if !valid_id(&option.option_id) || !option_ids.insert(&option.option_id) {
                    return Err(PortableUiValidationError::Invalid);
                }
                add(&option.label, false, 256)?;
            }
            if payload.initial_option_id.as_ref().is_some_and(|selected| {
                !payload
                    .options
                    .iter()
                    .any(|option| option.option_id == *selected)
            }) {
                return Err(PortableUiValidationError::Invalid);
            }
            add_validation(payload.validation.as_ref(), &mut add)?;
        }
    }
    Ok(total)
}

fn register_field<'a>(
    field_id: &'a str,
    field_ids: &mut HashSet<&'a str>,
    field_count: &mut usize,
) -> Result<(), PortableUiValidationError> {
    if !valid_id(field_id) || !field_ids.insert(field_id) {
        return Err(PortableUiValidationError::Invalid);
    }
    *field_count = field_count
        .checked_add(1)
        .ok_or(PortableUiValidationError::LimitExceeded)?;
    if *field_count > 64 {
        return Err(PortableUiValidationError::LimitExceeded);
    }
    Ok(())
}

fn add_validation(
    validation: Option<&UiValidation>,
    add: &mut impl FnMut(&str, bool, usize) -> Result<(), PortableUiValidationError>,
) -> Result<(), PortableUiValidationError> {
    if let Some(UiValidation::Invalid { message }) = validation {
        add(message, false, 512)?;
    }
    Ok(())
}

fn valid_id(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value.is_ascii()
        && value.as_bytes()[0].is_ascii_lowercase()
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
}

fn checked_text(
    value: &str,
    allow_lf: bool,
    maximum: usize,
) -> Result<usize, PortableUiValidationError> {
    let length = value.len();
    if length > maximum {
        return Err(PortableUiValidationError::LimitExceeded);
    }
    if value.chars().any(|character| {
        let code = u32::from(character);
        (code <= 0x1f && !(allow_lf && character == '\n'))
            || (0x7f..=0x9f).contains(&code)
            || matches!(
                code,
                0x061c | 0x200e | 0x200f | 0x202a..=0x202e | 0x2066..=0x2069
            )
    }) {
        return Err(PortableUiValidationError::Invalid);
    }
    Ok(length)
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

    fn valid_document() -> UiDocument {
        serde_json::from_value(json!({
            "protocol": "portable-v1",
            "title": "Settings",
            "nodes": [
                {"kind":"page","nodeId":"root","order":0,"payload":{"title":"Settings"}},
                {"kind":"form","nodeId":"settings","parentId":"root","order":0,
                 "payload":{"submitActionId":"save","submitLabel":"Save","disabled":false}},
                {"kind":"text-field","nodeId":"name","parentId":"settings","order":0,
                 "payload":{"fieldId":"name","label":"Name","initialValue":"A","required":true,
                 "minLength":1,"maxLength":64,"multiline":false,"disabled":false,"readOnly":false}},
                {"kind":"switch","nodeId":"enabled","parentId":"settings","order":1,
                 "payload":{"fieldId":"enabled","label":"Enabled","initialValue":true,
                 "disabled":false,"readOnly":false}}
            ]
        }))
        .expect("valid DTO")
    }

    #[test]
    fn portable_ui_validator_enforces_tree_and_plain_text() {
        assert_eq!(valid_document().validate(), Ok(()));
        let empty = UiDocument {
            protocol: ExtensionUiProtocol::PortableV1,
            title: String::new(),
            nodes: Vec::new(),
        };
        assert_eq!(empty.validate(), Err(PortableUiValidationError::Invalid));
        let invalid: UiDocument = serde_json::from_value(json!({
            "protocol": "portable-v1",
            "title": "Bad\u{202e}Title",
            "nodes": [
                {"kind":"page","nodeId":"root","order":0,"payload":{"title":"Bad"}},
                {"kind":"text","nodeId":"child","parentId":"later","order":0,
                 "payload":{"text":"value","tone":"neutral"}}
            ]
        }))
        .expect("closed DTO");
        assert_eq!(invalid.validate(), Err(PortableUiValidationError::Invalid));
    }

    #[test]
    fn portable_ui_action_requires_complete_document_order() {
        let document = valid_document();
        let valid: UiAction = serde_json::from_value(json!({
            "kind":"submit-form",
            "actionId":"save",
            "values":[
                {"fieldId":"name","value":{"kind":"text","value":"B"}},
                {"fieldId":"enabled","value":{"kind":"boolean","value":false}}
            ]
        }))
        .expect("action");
        assert_eq!(document.validate_action(&valid), Ok(()));

        let reordered: UiAction = serde_json::from_value(json!({
            "kind":"submit-form",
            "actionId":"save",
            "values":[
                {"fieldId":"enabled","value":{"kind":"boolean","value":false}},
                {"fieldId":"name","value":{"kind":"text","value":"B"}}
            ]
        }))
        .expect("action");
        assert_eq!(
            document.validate_action(&reordered),
            Err(PortableUiValidationError::Invalid)
        );
    }

    #[test]
    fn required_fields_may_render_empty_but_must_validate_on_submit() {
        let document: UiDocument = serde_json::from_value(json!({
            "protocol": "portable-v1",
            "title": "Required values",
            "nodes": [
                {"kind":"page","nodeId":"root","order":0,"payload":{"title":"Required values"}},
                {"kind":"form","nodeId":"form","parentId":"root","order":0,
                 "payload":{"submitActionId":"save","submitLabel":"Save","disabled":false}},
                {"kind":"text-field","nodeId":"name","parentId":"form","order":0,
                 "payload":{"fieldId":"name","label":"Name","initialValue":"","required":true,
                 "minLength":1,"maxLength":64,"multiline":false,"disabled":false,"readOnly":false}},
                {"kind":"integer-field","nodeId":"count","parentId":"form","order":1,
                 "payload":{"fieldId":"count","label":"Count","required":true,
                 "minimum":1,"maximum":10,"disabled":false,"readOnly":false}},
                {"kind":"select","nodeId":"choice","parentId":"form","order":2,
                 "payload":{"fieldId":"choice","label":"Choice","required":true,
                 "options":[{"optionId":"one","label":"One"}],"disabled":false,"readOnly":false}}
            ]
        }))
        .expect("closed DTO");
        assert_eq!(document.validate(), Ok(()));

        let invalid: UiAction = serde_json::from_value(json!({
            "kind":"submit-form",
            "actionId":"save",
            "values":[
                {"fieldId":"name","value":{"kind":"text","value":""}},
                {"fieldId":"count","value":{"kind":"integer","value":1}},
                {"fieldId":"choice","value":{"kind":"selection","value":"one"}}
            ]
        }))
        .expect("action");
        assert_eq!(
            document.validate_action(&invalid),
            Err(PortableUiValidationError::Invalid)
        );
    }
}
