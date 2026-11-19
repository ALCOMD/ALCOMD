//! Core-owned extension tool declarations and invocation authority.
use crate::{AccessContext, ExtensionInstanceLease, OAuthAccessGuard, Permission, PrincipalId};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub const MCP_TOOLS_BYTES: usize = 65_536;
pub const MCP_TOOLS_PATH: &str = "component/mcp-tools.json";
pub const MCP_TOOLS_PROVIDE: &str = "mcp.tools.provide";
pub const MCP_TOOLS_PROVIDER: &str = "alcomd:mcp-tools/provider@1.0.0";
pub const MCP_TOOLS_CORE: &str = "alcomd:mcp-tools/core@1.0.0";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub enum ToolOperation {
    #[serde(rename = "projects.list")]
    ProjectsList,
    #[serde(rename = "projects.get")]
    ProjectsGet,
    #[serde(rename = "packages.planUninstall")]
    PlanUninstall,
    #[serde(rename = "packages.applyPlan")]
    ApplyPlan,
    #[serde(rename = "operations.get")]
    OperationGet,
    #[serde(rename = "operations.cancel")]
    OperationCancel,
}
impl ToolOperation {
    pub fn permissions(self) -> &'static [Permission] {
        use Permission::*;
        match self {
            Self::ProjectsList | Self::ProjectsGet => &[ProjectsRead],
            Self::PlanUninstall => &[ProjectsRead, RepositoriesRead, PackagesRead],
            Self::ApplyPlan => &[ProjectsRead, RepositoriesRead, PackagesRead, PackagesManage],
            Self::OperationGet => &[OperationsRead],
            Self::OperationCancel => &[OperationsCancel],
        }
    }
    pub fn effect(self) -> ToolEffect {
        match self {
            Self::PlanUninstall => ToolEffect::Plan,
            Self::ApplyPlan => ToolEffect::Apply,
            Self::OperationCancel => ToolEffect::Cancel,
            _ => ToolEffect::Read,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolEffect {
    Pure,
    Read,
    Plan,
    Apply,
    Cancel,
}
impl ToolEffect {
    pub fn writes(self) -> bool {
        matches!(self, Self::Plan | Self::Apply | Self::Cancel)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolDeclaration {
    pub name: String,
    pub title: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub uses: Vec<ToolOperation>,
}
impl ToolDeclaration {
    pub fn effect(&self) -> Result<ToolEffect, ToolError> {
        let mut effect = if self.uses.is_empty() {
            ToolEffect::Pure
        } else {
            ToolEffect::Read
        };
        for op in &self.uses {
            let next = op.effect();
            if next.writes() {
                if effect.writes() && effect != next {
                    return Err(ToolError::InvalidInput);
                }
                effect = next;
            }
        }
        Ok(effect)
    }
    pub fn permissions(&self) -> Vec<String> {
        let mut items = self
            .uses
            .iter()
            .flat_map(|op| op.permissions())
            .map(|p| p.as_str().to_owned())
            .collect::<Vec<_>>();
        items.sort();
        items.dedup();
        items
    }
    pub fn validate(&self) -> Result<(), ToolError> {
        if self.name.is_empty()
            || self.name.len() > 32
            || !self.name.bytes().enumerate().all(|(i, b)| {
                b.is_ascii_lowercase() || (i > 0 && (b.is_ascii_digit() || b == b'_'))
            })
            || self.title.trim().is_empty()
            || self.title.len() > 256
            || self.description.len() > 2048
            || self
                .title
                .chars()
                .chain(self.description.chars())
                .any(char::is_control)
            || !self.input_schema.is_object()
            || !self.output_schema.is_object()
            || self.uses.len() > 6
        {
            return Err(ToolError::InvalidInput);
        }
        let mut unique = self.uses.clone();
        unique.sort();
        unique.dedup();
        if unique.len() != self.uses.len() {
            return Err(ToolError::InvalidInput);
        }
        let effect = self.effect()?;
        if effect.writes()
            && (self.input_schema["type"] != "object"
                || self.input_schema.as_object().is_none_or(|o| {
                    o.keys().any(|k| {
                        ![
                            "type",
                            "properties",
                            "required",
                            "additionalProperties",
                            "$defs",
                            "$schema",
                            "title",
                            "description",
                        ]
                        .contains(&k.as_str())
                    })
                }))
        {
            return Err(ToolError::InvalidInput);
        }

        let reserved: &[&str] = match effect {
            ToolEffect::Plan => &["idempotency_key"],
            ToolEffect::Apply => &["idempotency_key", "plan_id", "expected_revision", "confirm"],
            ToolEffect::Cancel => &["idempotency_key", "operation_id", "expected_revision"],
            _ => &[],
        };
        if reserved
            .iter()
            .any(|name| self.input_schema["properties"].get(*name).is_some())
        {
            return Err(ToolError::InvalidInput);
        }
        Ok(())
    }
    pub fn public_input_schema(&self) -> Value {
        let mut schema = self.input_schema.clone();
        let Ok(effect) = self.effect() else {
            return schema;
        };
        if !effect.writes() {
            return schema;
        }
        let mut properties = schema["properties"]
            .as_object()
            .cloned()
            .unwrap_or_default();
        properties.insert(
            "idempotency_key".into(),
            json!({"type":"string","minLength":1,"maxLength":128}),
        );
        let mut required = schema["required"].as_array().cloned().unwrap_or_default();
        required.push(json!("idempotency_key"));
        if effect == ToolEffect::Apply || effect == ToolEffect::Cancel {
            let field = if effect == ToolEffect::Apply {
                "plan_id"
            } else {
                "operation_id"
            };
            properties.insert(field.into(), json!({"type":"string","format":"uuid"}));
            required.push(json!(field));
            properties.insert(
                "expected_revision".into(),
                json!({"type":"integer","minimum":1,"maximum":9_007_199_254_740_991_u64}),
            );
            required.push(json!("expected_revision"));
        }
        if effect == ToolEffect::Apply {
            properties.insert("confirm".into(), json!({"const":true}));
            required.push(json!("confirm"));
        }
        schema["type"] = json!("object");
        schema["properties"] = json!(properties);
        schema["required"] = json!(required);
        schema
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolDeclarations {
    pub version: u32,
    pub tools: Vec<ToolDeclaration>,
}
impl ToolDeclarations {
    pub fn parse(bytes: &[u8]) -> Result<Vec<ToolDeclaration>, ToolError> {
        if bytes.len() > MCP_TOOLS_BYTES {
            return Err(ToolError::ResourceLimit);
        }
        let value: Self = serde_json::from_slice(bytes).map_err(|_| ToolError::InvalidInput)?;
        if value.version != 1 || value.tools.is_empty() || value.tools.len() > 32 {
            return Err(ToolError::InvalidInput);
        }
        let mut names = std::collections::HashSet::new();
        for tool in &value.tools {
            tool.validate()?;
            if !names.insert(tool.name.clone()) {
                return Err(ToolError::InvalidInput);
            }
        }
        Ok(value.tools)
    }
}
pub fn tool_public_name(extension: &str, local: &str) -> String {
    format!(
        "alcomd_ext_{}_{}",
        &digest(extension.as_bytes())[..20],
        local
    )
}
pub fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolOrigin {
    pub extension_id: String,
    pub publisher_fingerprint: String,
    pub package_digest: String,
    pub version: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolDescriptor {
    pub name: String,
    pub title: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub effect: ToolEffect,
    pub required_permissions: Vec<String>,
    pub origin: Option<ToolOrigin>,
}
impl ToolDescriptor {
    pub fn extension(origin: ToolOrigin, tool: &ToolDeclaration) -> Result<Self, ToolError> {
        Ok(Self {
            name: tool_public_name(&origin.extension_id, &tool.name),
            title: format!("{} · {}", origin.extension_id, tool.title),
            description: tool.description.clone(),
            input_schema: tool.public_input_schema(),
            output_schema: json!({"type":"object","properties":{"data":{"anyOf":[tool.output_schema,{"type":"null"}]},"coreEffect":{"type":["object","null"]},"replayed":{"type":"boolean"},"error":{"type":["object","null"]}},"required":["data","coreEffect","replayed","error"],"additionalProperties":false}),
            effect: tool.effect()?,
            required_permissions: tool.permissions(),
            origin: Some(origin),
        })
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolCatalogRequest {
    pub name: Option<String>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCatalogPage {
    pub tools: Vec<ToolDescriptor>,
    pub next_cursor: Option<String>,
    pub catalog_fingerprint: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolCallRequest {
    pub name: String,
    pub arguments: Value,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallResult {
    pub data: Value,
    pub core_effect: Option<Value>,
    pub replayed: bool,
    pub error: Option<ToolError>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolError {
    InvalidInput,
    PermissionDenied,
    RevisionConflict,
    IdempotencyConflict,
    ResourceLimit,
    Unavailable,
    ProviderUnavailable,
    OutputInvalid,
    InProgress,
    Interrupted,
    WriteOutcomeUnknown,
}
impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Extension tool request failed: {self:?}")
    }
}
impl std::error::Error for ToolError {}

#[derive(Clone)]
pub enum ToolAuthentication {
    Credential([u8; 32]),
    OAuth(OAuthAccessGuard),
}
#[derive(Clone)]
pub struct ToolInvocation {
    pub call_id: String,
    pub principal: PrincipalId,
    pub origin: ToolOrigin,
    pub declaration: ToolDeclaration,
    pub authentication: ToolAuthentication,
    pub lease: Option<ExtensionInstanceLease>,
    pub control: Value,
    pub active: Arc<AtomicBool>,
    pub deadline_ms: u64,
}
impl ToolInvocation {
    pub fn is_current(&self, now: u64) -> bool {
        self.active.load(Ordering::Acquire) && now < self.deadline_ms
    }
    pub fn cancel(&self) {
        self.active.store(false, Ordering::Release);
    }
    pub fn allows(&self, op: ToolOperation) -> bool {
        self.declaration.uses.contains(&op)
    }
}
tokio::task_local! {static TOOL_INVOCATION: ToolInvocation;}
pub async fn with_tool_invocation<T>(
    authority: ToolInvocation,
    future: impl std::future::Future<Output = T>,
) -> T {
    TOOL_INVOCATION.scope(authority, future).await
}
pub fn tool_invocation() -> Option<ToolInvocation> {
    TOOL_INVOCATION.try_with(Clone::clone).ok()
}
pub fn require_tool_permissions(
    access: &AccessContext,
    permissions: &[String],
) -> Result<(), ToolError> {
    for name in permissions {
        let permission: Permission =
            serde_json::from_value(json!(name)).map_err(|_| ToolError::PermissionDenied)?;
        access
            .require(permission)
            .map_err(|_| ToolError::PermissionDenied)?;
    }
    Ok(())
}

impl ToolAuthentication {
    pub fn access(&self, client: crate::McpClient) -> Result<AccessContext, ToolError> {
        let access = crate::mcp_access(&client)
            .map_err(|_| ToolError::PermissionDenied)?
            .with_actor("MCP".into());
        Ok(match self {
            Self::Credential(_) => access,
            Self::OAuth(guard) => access.with_oauth_guard(guard.clone()),
        })
    }
    pub fn binding(&self) -> String {
        match self {
            Self::Credential(hash) => digest(hash),
            Self::OAuth(g) => {
                digest(format!("{}:{}", crate::oauth_hex(&g.token_hash), g.resource).as_bytes())
            }
        }
    }
}
pub fn tool_cursor_mac(key: &[u8; 32], message: &[u8]) -> String {
    crate::oauth_hex(&crate::m8_oauth::cursor_hmac(key, message))
}
#[derive(Clone, Debug)]
pub struct ToolProvider {
    pub record: crate::ExtensionRecord,
    pub owner: PrincipalId,
    pub locator: String,
    pub granted: Vec<String>,
}
#[derive(Clone)]
pub struct ToolStoreSnapshot {
    pub client: crate::McpClient,
    pub providers: Vec<ToolProvider>,
}
/// Private trusted Host reply. This envelope is constructed by native Host code,
/// not deserialized from guest data; it contains no authority or credentials.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolProviderOutcome {
    pub data: Value,
    pub error: Option<ToolError>,
    pub fault: bool,
}

#[derive(Clone)]
pub struct ToolWriteAdmission {
    pub origin: ToolOrigin,
    pub name: String,
    pub key: String,
    pub fingerprint: String,
    pub permissions: Vec<String>,
    pub effect: ToolEffect,
}

pub fn tool_request_fingerprint(name: &str, arguments: &Value) -> Result<String, ToolError> {
    fn canonical(value: &Value) -> Value {
        match value {
            Value::Object(object) => {
                let mut entries = object.iter().collect::<Vec<_>>();
                entries.sort_by(|a, b| a.0.cmp(b.0));
                Value::Object(
                    entries
                        .into_iter()
                        .map(|(k, v)| (k.clone(), canonical(v)))
                        .collect(),
                )
            }
            Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
            other => other.clone(),
        }
    }
    serde_json::to_vec(&(name, canonical(arguments)))
        .map(|bytes| digest(&bytes))
        .map_err(|_| ToolError::InvalidInput)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tool(uses: Vec<ToolOperation>) -> ToolDeclaration {
        ToolDeclaration {
            name: "example".into(),
            title: "Example".into(),
            description: "A declared tool".into(),
            input_schema: json!({"type":"object","properties":{},"additionalProperties":false}),
            output_schema: json!({"type":"object"}),
            uses,
        }
    }
    #[test]
    fn argument_digest_ignores_object_order_but_not_input_changes() {
        let a: Value = serde_json::from_str(r#"{"b":{"x":1,"y":2},"a":0}"#).expect("args");
        let b: Value = serde_json::from_str(r#"{"a":0,"b":{"y":2,"x":1}}"#).expect("args");
        assert_eq!(
            tool_request_fingerprint("x", &a),
            tool_request_fingerprint("x", &b)
        );
        assert_ne!(
            tool_request_fingerprint("x", &a),
            tool_request_fingerprint("y", &a)
        );
    }
    #[test]
    fn names_are_stable_bounded_and_extension_isolated() {
        let a = tool_public_name("dev.example.a", "x");
        assert_ne!(a, tool_public_name("dev.example.b", "x"));
        assert_eq!(a, tool_public_name("dev.example.a", "x"));
        assert!(tool_public_name("dev.example.a", &"x".repeat(32)).len() <= 64);
    }
    #[test]
    fn one_persistent_effect_and_reserved_controls() {
        assert!(
            tool(vec![ToolOperation::PlanUninstall, ToolOperation::ApplyPlan])
                .validate()
                .is_err()
        );
        let mut value = tool(vec![ToolOperation::ApplyPlan]);
        assert!(value.validate().is_ok());
        value.input_schema["properties"]["confirm"] = json!({"const":false});
        assert!(value.validate().is_err());
    }
    #[test]
    fn core_supplies_confirmation_and_keeps_custom_fields() {
        let mut value = tool(vec![ToolOperation::ApplyPlan]);
        value.input_schema["properties"]["reason"] = json!({"type":"string"});
        let public = value.public_input_schema();
        assert_eq!(public["properties"]["confirm"]["const"], true);
        assert_eq!(public["properties"]["reason"]["type"], "string");
        assert!(
            public["required"]
                .as_array()
                .expect("valid generated tool contract")
                .contains(&json!("plan_id"))
        );
    }
    #[test]
    fn declarations_are_not_grants_and_cannot_add_operations() {
        assert!(tool(vec![]).permissions().is_empty());
        let value = tool(vec![ToolOperation::ApplyPlan]);
        assert!(value.permissions().contains(&"packages.manage".to_owned()));
        assert!(serde_json::from_value::<ToolOperation>(json!("rpc.anything")).is_err());
        let access = AccessContext::new(PrincipalId::local_owner(), []);
        assert_eq!(
            require_tool_permissions(&access, &value.permissions()),
            Err(ToolError::PermissionDenied)
        );
    }
}
