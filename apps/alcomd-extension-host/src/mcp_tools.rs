//! Trusted, bounded Schema validation and the independent MCP provider interface.
use super::HostState;
use alcomd_application::{MCP_TOOLS_BYTES, ToolDeclaration};
use anyhow::{Result, bail};
use serde_json::{Value, json};
use wasmtime::component::{Component, Instance, Linker};
use wasmtime::{Engine, Store};

mod bindings {
    wasmtime::component::bindgen!({
        path: "../../specs/extensions/wit/mcp-tools-v1", world: "provider-v1",
        imports: { default: async }, exports: { default: async },
    });
}
mod cli {
    wasmtime::component::bindgen!({
        path: "../../specs/extensions/wit/cli-commands-v1", world: "provider-v1",
        imports: { default: async }, exports: { default: async },
    });
}
use bindings::alcomd::extension::host_operations as wit;

pub(super) fn add_to_linker(linker: &mut Linker<HostState>) -> Result<()> {
    wit::add_to_linker::<_, wasmtime::component::HasSelf<_>>(linker, |s| s)
        .map_err(|_| anyhow::anyhow!("link"))?;
    Ok(())
}

struct NoExternal;
impl jsonschema::Retrieve for NoExternal {
    fn retrieve(
        &self,
        _: &jsonschema::Uri<String>,
    ) -> std::result::Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "external schema references are forbidden",
        )
        .into())
    }
}
// This is a resource-resolution policy walk, not a second schema evaluator.
// Only actual subschema positions are visited; const/default/example data is opaque.
fn schema_children(value: &Value) -> Vec<&Value> {
    let mut result = Vec::new();
    for key in [
        "$defs",
        "definitions",
        "properties",
        "patternProperties",
        "dependentSchemas",
    ] {
        if let Some(map) = value.get(key).and_then(Value::as_object) {
            result.extend(map.values());
        }
    }
    for key in ["allOf", "anyOf", "oneOf", "prefixItems"] {
        if let Some(items) = value.get(key).and_then(Value::as_array) {
            result.extend(items);
        }
    }
    for key in [
        "items",
        "contains",
        "additionalProperties",
        "unevaluatedProperties",
        "propertyNames",
        "if",
        "then",
        "else",
        "not",
        "unevaluatedItems",
        "contentSchema",
        "additionalItems",
    ] {
        if let Some(child) = value.get(key) {
            result.push(child);
        }
    }
    if let Some(map) = value.get("dependencies").and_then(Value::as_object) {
        result.extend(map.values().filter(|v| !v.is_array()));
    }
    result
}
fn schema(value: &Value) -> Result<jsonschema::Validator> {
    use std::collections::BTreeMap;
    fn collect<'a>(
        value: &'a Value,
        anchors: &mut BTreeMap<String, &'a Value>,
        depth: usize,
        budget: &mut usize,
    ) -> Result<()> {
        if depth > 64 || *budget == 0 {
            bail!("schema limit");
        }
        *budget -= 1;
        if let Some(anchor) = value.get("$anchor") {
            let anchor = anchor
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("schema anchor"))?;
            if anchors.insert(anchor.to_owned(), value).is_some() {
                bail!("duplicate schema anchor");
            }
        }
        for child in schema_children(value) {
            collect(child, anchors, depth + 1, budget)?;
        }
        Ok(())
    }
    fn walk(
        root: &Value,
        value: &Value,
        anchors: &BTreeMap<String, &Value>,
        stack: &mut Vec<String>,
        depth: usize,
        budget: &mut usize,
    ) -> Result<()> {
        if depth > 64 || *budget == 0 {
            bail!("schema limit");
        }
        *budget -= 1;
        if let Some(object) = value.as_object() {
            if (!std::ptr::eq(root, value) && object.contains_key("$id"))
                || [
                    "$dynamicRef",
                    "$dynamicAnchor",
                    "$recursiveRef",
                    "$recursiveAnchor",
                    "x-mcp-header",
                ]
                .iter()
                .any(|k| object.contains_key(*k))
            {
                bail!("unsupported schema resource");
            }
            if let Some(version) = object.get("$schema")
                && version != "https://json-schema.org/draft/2020-12/schema"
            {
                bail!("schema draft");
            }
            if let Some(reference) = object.get("$ref") {
                let reference = reference
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("schema reference"))?;
                if !reference.starts_with('#') {
                    bail!("external schema reference");
                }
                let uri = jsonschema::uri::from_str(reference)
                    .map_err(|_| anyhow::anyhow!("schema URI"))?;
                let fragment = uri
                    .fragment()
                    .ok_or_else(|| anyhow::anyhow!("schema fragment"))?
                    .decode()
                    .to_string()
                    .map_err(|_| anyhow::anyhow!("schema fragment UTF-8"))?;
                if stack.contains(&fragment.to_string()) {
                    bail!("cyclic schema reference");
                }
                let target = if fragment.is_empty() {
                    Some(root)
                } else if fragment.starts_with('/') {
                    root.pointer(&fragment)
                } else {
                    anchors.get(fragment.as_ref()).copied()
                }
                .ok_or_else(|| anyhow::anyhow!("schema reference target"))?;
                stack.push(fragment.into_owned());
                walk(root, target, anchors, stack, depth + 1, budget)?;
                stack.pop();
            }
        }
        for child in schema_children(value) {
            walk(root, child, anchors, stack, depth + 1, budget)?;
        }
        Ok(())
    }
    let mut anchors = BTreeMap::new();
    collect(value, &mut anchors, 0, &mut 16384)?;
    walk(value, value, &anchors, &mut Vec::new(), 0, &mut 16384)?;
    jsonschema::options()
        .with_retriever(NoExternal)
        .with_draft(jsonschema::Draft::Draft202012)
        .should_validate_formats(true)
        .build(value)
        .map_err(|_| anyhow::anyhow!("invalid schema"))
}
fn bounded(value: &Value) -> bool {
    serde_json::to_vec(value).is_ok_and(|v| v.len() <= MCP_TOOLS_BYTES)
}
/// Type-check declared exports and bounded schemas without instantiating or running the guest.
pub(super) fn preflight(engine: &Engine, component: &Component, bytes: &[u8]) -> Result<()> {
    let abi = |_| anyhow::anyhow!("invalid component or declared entry ABI");
    let entries: alcomd_extensions::EntryPreflight = serde_json::from_slice(bytes)?;
    let mut linker = Linker::<HostState>::new(engine);
    super::ExtensionV1::add_to_linker::<_, wasmtime::component::HasSelf<_>>(&mut linker, |s| s)
        .map_err(abi)?;
    super::mcp_management::add_to_linker(&mut linker)?;
    super::mcp_runtime::add_to_linker(&mut linker)?;
    super::mcp_transport::add_to_linker(&mut linker)?;
    if entries.background {
        super::background_integration::add_to_linker(&mut linker)?;
    }
    add_to_linker(&mut linker)?;
    let instance = linker.instantiate_pre(component).map_err(abi)?;
    super::ExtensionV1Pre::new(instance.clone()).map_err(abi)?;
    if entries.ui {
        super::portable::PortableUiV1Pre::new(instance.clone()).map_err(abi)?;
    }
    if entries.background {
        super::background_integration::preflight(instance.clone())?;
    }
    if !entries.tools.is_empty() {
        bindings::ProviderV1Pre::new(instance.clone()).map_err(abi)?;
    }
    if !entries.commands.is_empty() {
        cli::ProviderV1Pre::new(instance).map_err(abi)?;
    }
    for declaration in entries.tools.iter().chain(&entries.commands) {
        declaration.validate()?;
        schema(&declaration.input_schema)?;
        schema(&declaration.public_input_schema())?;
        schema(&declaration.output_schema)?;
    }
    Ok(())
}
pub(super) async fn invoke(
    instance: &Instance,
    store: &mut Store<HostState>,
    input: Value,
    kind: alcomd_application::ExtensionEntryKind,
) -> Result<Value, super::HostStableError> {
    use alcomd_application::{ToolError as E, ToolProviderOutcome as Reply};
    let result: std::result::Result<Value, (E, bool)> = async {
        let declaration: ToolDeclaration = serde_json::from_value(input["declaration"].clone())
            .map_err(|_| (E::InvalidInput, false))?;
        declaration
            .validate()
            .map_err(|_| (E::InvalidInput, false))?;
        let arguments = input["arguments"].clone();
        if !bounded(&arguments)
            || !schema(&declaration.input_schema)
                .map_err(|_| (E::InvalidInput, false))?
                .is_valid(&arguments)
        {
            return Err((E::InvalidInput, false));
        }
        let bytes = serde_json::to_vec(&arguments).map_err(|_| (E::InvalidInput, false))?;
        let output = match kind {
            alcomd_application::ExtensionEntryKind::Mcp => {
                let p = bindings::ProviderV1::new(&mut *store, instance)
                    .map_err(|_| (E::ProviderUnavailable, true))?;
                p.alcomd_mcp_tools_provider()
                    .call_invoke(&mut *store, &declaration.name, &bytes)
                    .await
            }
            alcomd_application::ExtensionEntryKind::Cli => {
                let p = cli::ProviderV1::new(&mut *store, instance)
                    .map_err(|_| (E::ProviderUnavailable, true))?;
                p.alcomd_cli_commands_provider()
                    .call_invoke(&mut *store, &declaration.name, &bytes)
                    .await
            }
        }
        .map_err(|_| (E::ProviderUnavailable, true))?
        .map_err(|_| (E::ProviderUnavailable, false))?;
        if output.len() > MCP_TOOLS_BYTES {
            return Err((E::OutputInvalid, true));
        }
        let data: Value = serde_json::from_slice(&output).map_err(|_| (E::OutputInvalid, true))?;
        if !schema(&declaration.output_schema)
            .map_err(|_| (E::OutputInvalid, true))?
            .is_valid(&data)
        {
            return Err((E::OutputInvalid, true));
        }
        Ok(data)
    }
    .await;
    let reply = match result {
        Ok(data) => Reply {
            data,
            error: None,
            fault: false,
        },
        Err((error, fault)) => Reply {
            data: Value::Null,
            error: Some(error),
            fault,
        },
    };
    serde_json::to_value(reply).map_err(|_| super::resource_error())
}
impl HostState {
    fn tool(&mut self, method: &str, input: Value) -> Result<Vec<u8>, wit::Error> {
        if !bounded(&input) {
            return Err(wit::Error::ResourceLimit);
        }
        let result = self
            .protocol
            .capability(&format!("host-operations.{method}"), input)
            .map_err(|_| wit::Error::Unavailable)?;
        if !bounded(&result) {
            return Err(wit::Error::ResourceLimit);
        }
        if let Some(error) = result.get("error").and_then(Value::as_str) {
            return Err(match error {
                "invalid_input" => wit::Error::InvalidInput,
                "permission_denied" => wit::Error::PermissionDenied,
                "revision_conflict" => wit::Error::RevisionConflict,
                "idempotency_conflict" => wit::Error::IdempotencyConflict,
                "resource_limit" => wit::Error::ResourceLimit,
                "interrupted" => wit::Error::Interrupted,
                "write_outcome_unknown" => wit::Error::WriteOutcomeUnknown,
                _ => wit::Error::Unavailable,
            });
        }
        serde_json::to_vec(&result["ok"]).map_err(|_| wit::Error::Unavailable)
    }
}
impl wit::Host for HostState {
    async fn list_projects(
        &mut self,
        cursor: Option<Vec<u8>>,
        limit: Option<u32>,
    ) -> Result<Vec<u8>, wit::Error> {
        let cursor = cursor
            .map(|v| serde_json::from_slice::<Value>(&v))
            .transpose()
            .map_err(|_| wit::Error::InvalidInput)?;
        self.tool("projects.list", json!({"cursor":cursor,"limit":limit}))
    }
    async fn get_project(&mut self, project_id: String) -> Result<Vec<u8>, wit::Error> {
        self.tool("projects.get", json!({"projectId":project_id}))
    }
    async fn plan_uninstall(
        &mut self,
        project_id: String,
        expected_revision: u64,
        package_id: String,
    ) -> Result<Vec<u8>, wit::Error> {
        self.tool("packages.planUninstall",json!({"projectId":project_id,"expectedRevision":expected_revision,"packageId":package_id}))
    }
    async fn apply_plan(&mut self) -> Result<Vec<u8>, wit::Error> {
        self.tool("packages.applyPlan", json!({}))
    }
    async fn get_operation(&mut self, operation_id: String) -> Result<Vec<u8>, wit::Error> {
        self.tool("operations.get", json!({"operationId":operation_id}))
    }
    async fn cancel_operation(&mut self) -> Result<Vec<u8>, wit::Error> {
        self.tool("operations.cancel", json!({}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordinary_data_and_property_names_are_not_interpreted_as_schema_resources() {
        let validator=schema(&json!({"type":"object","properties":{"$ref":{"type":"string"}},"const":{"$ref":"https://example.invalid/ordinary-data"}})).expect("const data is opaque");
        assert!(validator.is_valid(&json!({"$ref":"https://example.invalid/ordinary-data"})));
    }
    #[test]
    fn local_anchor_and_encoded_pointer_references_keep_2020_12_semantics() {
        let validator=schema(&json!({"$id":"https://example.invalid/tool","$defs":{"a b":{"$anchor":"text","type":"string"}},"type":"object","properties":{"a":{"$ref":"#text"},"b":{"$ref":"#/$defs/a%20b"}}})).expect("local non-cyclic resources");
        assert!(validator.is_valid(&json!({"a":"yes","b":"yes"})));
        assert!(!validator.is_valid(&json!({"a":1})));
        assert!(schema(&json!({"$defs":{"loop":{"$anchor":"loop","$ref":"#loop"}}})).is_err());
    }

    #[test]
    fn schema_rejects_external_dynamic_and_cycles() {
        for value in [
            json!({"$ref":"https://example.invalid/x"}),
            json!({"$ref":"file:///tmp/x"}),
            json!({"$dynamicRef":"#x"}),
            json!({"$defs":{"x":{"$ref":"#/$defs/x"}},"$ref":"#/$defs/x"}),
            json!({"x-mcp-header":"Authorization"}),
        ] {
            assert!(schema(&value).is_err());
        }
        let valid=schema(&json!({"type":"object","$defs":{"text":{"type":"string"}},"properties":{"v":{"$ref":"#/$defs/text"}},"required":["v"]})).expect("valid isolated Provider fixture");
        assert!(valid.is_valid(&json!({"v":"ok"})));
        assert!(!valid.is_valid(&json!({"v":42})));
    }
}
