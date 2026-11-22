//! Built-in MCP tools and extension dispatch through the authenticated typed Core client.
use alcomd_client::{AlcomdClient, ClientError};
use alcomd_protocol as rpc;
use rmcp::{
    ErrorData,
    model::{CallToolRequestParams, CallToolResult, ContentBlock, Tool},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};

pub const CONTRACT: &str = include_str!("../../../specs/mcp/package-removal-v1.schema.json");
const PACKAGE_CONTRACT: &str =
    include_str!("../../../specs/mcp/package-catalog-plan-v1.schema.json");
const WORKFLOW_CONTRACT: &str = include_str!("../../../specs/mcp/project-workflows-v1.schema.json");
const ENVIRONMENT_CONTRACT: &str = include_str!("../../../specs/mcp/environment-v1.schema.json");
const INVENTORY_CONTRACT: &str =
    include_str!("../../../specs/mcp/project-inventory-v1.schema.json");
const TEMPLATE_CONTRACT: &str = include_str!("../../../specs/mcp/template-edit-v1.schema.json");
mod environment;
mod extensions;
mod operations;
mod packages;
mod project_catalog;
mod tool_fields;
mod workflows;
const MAX_SAFE: u64 = 9_007_199_254_740_991;
static DIAGNOSTICS: AtomicU64 = AtomicU64::new(1);

/// Static HTTP admission metadata. Discovery still uses Core's authorized directory.
pub(crate) fn builtin_definition(name: &str) -> Result<Option<Tool>, ErrorData> {
    let source = if rpc::mcp_extension_management_capability(name).is_some() {
        include_str!("../../../specs/mcp/extensions-v1.schema.json")
    } else if rpc::mcp_project_catalog_capabilities(name).is_some() {
        include_str!("../../../specs/mcp/project-catalog-v1.schema.json")
    } else if packages::recognized(name) {
        PACKAGE_CONTRACT
    } else if rpc::mcp_environment_tool_capability(name) == Some(rpc::CAPABILITY_DESKTOP_ACTIONS_V1)
    {
        include_str!("../../../specs/mcp/desktop-actions-v1.schema.json")
    } else if matches!(
        rpc::mcp_environment_tool_capability(name),
        Some(
            rpc::CAPABILITY_MCP_CLIENTS_V1
                | rpc::CAPABILITY_MCP_OAUTH_V1
                | rpc::CAPABILITY_MCP_RUNTIME_READ_V1
                | rpc::CAPABILITY_MCP_TRANSPORT_READ_V1
        )
    ) {
        include_str!("../../../specs/mcp/ai-clients-v1.schema.json")
    } else if rpc::mcp_environment_tool_capability(name)
        == Some(rpc::CAPABILITY_CLIENTS_REGISTRY_V1)
    {
        include_str!("../../../specs/mcp/client-registry-v1.schema.json")
    } else if operations::recognized(name) {
        operations::CONTRACT
    } else if workflows::recognized(name) {
        WORKFLOW_CONTRACT
    } else if name == "alcomd_list_installed_packages" {
        INVENTORY_CONTRACT
    } else if matches!(
        name,
        "alcomd_get_project_template"
            | "alcomd_edit_project_template"
            | "alcomd_create_project_template"
            | "alcomd_export_project_template"
            | "alcomd_remove_project_template"
    ) {
        TEMPLATE_CONTRACT
    } else if environment::recognized(name) {
        ENVIRONMENT_CONTRACT
    } else {
        CONTRACT
    };
    let contract: Value = serde_json::from_str(source).map_err(|_| internal_protocol())?;
    let Some(mut entry) = contract["x-alcomd-tools"]
        .as_array()
        .ok_or_else(internal_protocol)?
        .iter()
        .find(|entry| entry["name"].as_str() == Some(name))
        .and_then(Value::as_object)
        .cloned()
    else {
        return Ok(None);
    };
    if let Some(effect) = entry.remove("effect") {
        let read_only = match effect.as_str() {
            Some("read") => true,
            Some("plan" | "apply") => false,
            _ => return Err(internal_protocol()),
        };
        entry.remove("requiredPermissions");
        entry.entry("annotations").or_insert_with(|| json!({
            "readOnlyHint":read_only,"destructiveHint":false,"idempotentHint":read_only,"openWorldHint":false,
        }));
    }
    entry.remove("x-alcomd-method");
    for schema in ["inputSchema", "outputSchema"] {
        let value = entry.get_mut(schema).ok_or_else(internal_protocol)?;
        *value = rpc::mcp_tool_schema(value, &contract);
    }
    serde_json::from_value(Value::Object(entry))
        .map(Some)
        .map_err(|_| internal_protocol())
}
fn internal_protocol() -> ErrorData {
    ErrorData::internal_error("The tool contract is unavailable.", None)
}
fn canonical_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_digit() || (b'a'..=b'f').contains(&b)
            }
        })
}
fn revision(value: u64) -> bool {
    (1..=MAX_SAFE).contains(&value)
}
fn key(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && value.is_ascii()
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    registered_at_ms: u64,
    project_id: String,
    #[serde(default, deserialize_with = "cursor_binding")]
    authorization_binding: Option<String>,
}
fn cursor_binding<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    String::deserialize(d).map(Some)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct List {
    cursor: Option<Cursor>,
    limit: Option<u32>,
    view: Option<rpc::ProjectListView>,
    expected_view_sequence: Option<u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Project {
    project_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    project_id: String,
    expected_revision: u64,
    package_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Apply {
    plan_id: String,
    expected_revision: u64,
    idempotency_key: String,
    confirm: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Operation {
    operation_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationsCursor {
    created_at_ms: u64,
    operation_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OperationsList {
    cursor: Option<OperationsCursor>,
    limit: Option<u32>,
    #[serde(default)]
    active_only: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cancel {
    operation_id: String,
    expected_revision: u64,
    idempotency_key: String,
}
enum Input {
    List(List),
    Project(Project),
    Plan(Plan),
    Apply(Apply),
    Operation(Operation),
    OperationsList(OperationsList),
    Cancel(Cancel),
}
fn parse(params: CallToolRequestParams) -> Result<Input, ErrorData> {
    if !matches!(
        params.name.as_ref(),
        "alcomd_list_projects"
            | "alcomd_get_project"
            | "alcomd_plan_package_uninstall"
            | "alcomd_apply_package_changes"
            | "alcomd_get_operation"
            | "alcomd_list_operations"
            | "alcomd_cancel_operation"
    ) {
        return Err(super::unsupported_method());
    }
    if params.input_responses.is_some() || params.request_state.is_some() {
        return Err(ErrorData::invalid_params(
            "This toolset does not accept multi-round input.",
            None,
        ));
    }
    let args = params.arguments.unwrap_or_default();
    if args.values().any(Value::is_null) {
        return Err(invalid());
    }
    let value = Value::Object(args);
    let input = match params.name.as_ref() {
        "alcomd_list_projects" => {
            Input::List(serde_json::from_value(value).map_err(|_| invalid())?)
        }
        "alcomd_get_project" => {
            Input::Project(serde_json::from_value(value).map_err(|_| invalid())?)
        }
        "alcomd_plan_package_uninstall" => {
            Input::Plan(serde_json::from_value(value).map_err(|_| invalid())?)
        }
        "alcomd_apply_package_changes" => {
            Input::Apply(serde_json::from_value(value).map_err(|_| invalid())?)
        }
        "alcomd_get_operation" => {
            Input::Operation(serde_json::from_value(value).map_err(|_| invalid())?)
        }
        "alcomd_list_operations" => {
            if value.get("cursor").is_some_and(|cursor| {
                cursor
                    .as_object()
                    .is_some_and(|fields| fields.values().any(Value::is_null))
            }) {
                return Err(invalid());
            }
            Input::OperationsList(serde_json::from_value(value).map_err(|_| invalid())?)
        }
        "alcomd_cancel_operation" => {
            Input::Cancel(serde_json::from_value(value).map_err(|_| invalid())?)
        }
        _ => return Err(super::unsupported_method()),
    };
    let valid = match &input {
        Input::List(p) => {
            p.limit.is_none_or(|v| (1..=1000).contains(&v))
                && p.expected_view_sequence.is_some() == (p.view.is_some() && p.cursor.is_some())
                && p.expected_view_sequence
                    .is_none_or(|value| value <= MAX_SAFE)
                && p.cursor
                    .as_ref()
                    .is_none_or(|c| c.registered_at_ms <= MAX_SAFE && canonical_id(&c.project_id))
        }
        Input::Project(p) => canonical_id(&p.project_id),
        Input::Plan(p) => {
            canonical_id(&p.project_id)
                && revision(p.expected_revision)
                && !p.package_id.is_empty()
                && p.package_id.chars().count() <= 128
        }
        Input::Apply(p) => {
            canonical_id(&p.plan_id)
                && revision(p.expected_revision)
                && key(&p.idempotency_key)
                && p.confirm
        }
        Input::Operation(p) => canonical_id(&p.operation_id),
        Input::OperationsList(p) => {
            p.limit.is_none_or(|v| (1..=1000).contains(&v))
                && p.cursor
                    .as_ref()
                    .is_none_or(|c| c.created_at_ms <= MAX_SAFE && canonical_id(&c.operation_id))
        }
        Input::Cancel(p) => {
            canonical_id(&p.operation_id)
                && revision(p.expected_revision)
                && key(&p.idempotency_key)
        }
    };
    if !valid {
        return Err(invalid());
    }
    Ok(input)
}
fn invalid() -> ErrorData {
    ErrorData::invalid_params("Arguments do not match the tool schema.", None)
}

enum Failure {
    Remote(ClientError, bool),
    InvalidCore,
}
fn read(error: ClientError) -> Failure {
    Failure::Remote(error, false)
}
fn write(error: ClientError) -> Failure {
    Failure::Remote(error, true)
}
fn snake_name(value: &str) -> String {
    let mut out = String::new();
    for c in value.chars() {
        if c.is_ascii_uppercase() {
            out.push('_');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}
/// Only called on explicitly typed public DTOs, never on arbitrary Core results.
// The tool accepts snake_case fields, while the same closed DTO uses camelCase on IPC.
fn ipc_fields(value: Value) -> Result<Value, ErrorData> {
    Ok(match value {
        Value::Object(fields) => {
            let mut mapped = serde_json::Map::new();
            for (name, value) in fields {
                if name.is_empty()
                    || !name
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte == b'_')
                {
                    return Err(invalid());
                }
                let mut parts = name.split('_');
                let mut key = parts.next().ok_or_else(invalid)?.to_owned();
                for part in parts {
                    let mut letters = part.chars();
                    key.push(letters.next().ok_or_else(invalid)?.to_ascii_uppercase());
                    key.extend(letters);
                }
                if mapped.insert(key, ipc_fields(value)?).is_some() {
                    return Err(invalid());
                }
            }
            Value::Object(mapped)
        }
        Value::Array(values) => Value::Array(
            values
                .into_iter()
                .map(ipc_fields)
                .collect::<Result<_, _>>()?,
        ),
        Value::Null => return Err(invalid()),
        other => other,
    })
}

fn snake(value: Value) -> Value {
    match value {
        Value::Object(o) => Value::Object(
            o.into_iter()
                .map(|(k, v)| (snake_name(&k), snake(v)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.into_iter().map(snake).collect()),
        value => value,
    }
}
fn project(value: rpc::ProjectSnapshot) -> Result<Value, Failure> {
    if value
        .project_id
        .as_deref()
        .is_none_or(|id| !canonical_id(id))
        || value.revision.is_none_or(|value| !revision(value))
        || value.directory_state.is_none()
    {
        return Err(Failure::InvalidCore);
    }
    serde_json::to_value(value)
        .map(snake)
        .map_err(|_| Failure::InvalidCore)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PackageResult {
    plan_id: String,
    change_set_fingerprint: String,
}
fn operation(mut value: rpc::Operation) -> Result<Value, Failure> {
    if !canonical_id(&value.operation_id) || !revision(value.revision) {
        return Err(Failure::InvalidCore);
    }
    if value.kind != "packages.apply" {
        if value.kind.is_empty()
            || value.kind.len() > 128
            || !value
                .kind
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
        {
            return Err(Failure::InvalidCore);
        }
        if workflows::operation_kind(&value.kind) {
            value.result = value
                .result
                .map(|result| workflows::operation_result(&value.kind, result))
                .transpose()?;
            return Ok(snake(
                serde_json::to_value(value).map_err(|_| Failure::InvalidCore)?,
            ));
        }
        if value.kind == "state.check"
            && let Some(result) = value.result.as_ref()
        {
            let fields = result.as_object().ok_or(Failure::InvalidCore)?;
            if fields.len() != 2
                || ["integrity", "foreignKeys"].into_iter().any(|key| {
                    !matches!(
                        fields.get(key).and_then(Value::as_str),
                        Some("ok" | "issues_detected" | "issues_truncated")
                    )
                })
            {
                return Err(Failure::InvalidCore);
            }
        }
        return Ok(snake(
            serde_json::to_value(value).map_err(|_| Failure::InvalidCore)?,
        ));
    }
    let result = value
        .result
        .as_ref()
        .map(|v| {
            serde_json::from_value::<PackageResult>(v.clone()).map_err(|_| Failure::InvalidCore)
        })
        .transpose()?;
    if let Some(result) = &result
        && (!canonical_id(&result.plan_id)
            || result.change_set_fingerprint.len() != 64
            || !result
                .change_set_fingerprint
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
    {
        return Err(Failure::InvalidCore);
    }
    Ok(snake(
        serde_json::to_value(value).map_err(|_| Failure::InvalidCore)?,
    ))
}
fn safe_numbers(value: &Value) -> bool {
    match value {
        Value::Number(n) => n.as_u64().is_some_and(|v| v <= MAX_SAFE),
        Value::Array(a) => a.iter().all(safe_numbers),
        Value::Object(o) => o.values().all(safe_numbers),
        _ => true,
    }
}
fn success(data: Value, summary: String, plan: bool) -> CallToolResult {
    if !safe_numbers(&data) {
        return failure(Failure::InvalidCore);
    }
    let Ok(bytes) = serde_json::to_vec(&data) else {
        return failure(Failure::InvalidCore);
    };
    if bytes.len() > 3 * 1024 * 1024 {
        return error(
            if plan {
                "plan_too_large"
            } else {
                "internal_error"
            },
            "The complete result exceeds the tool response budget.",
            false,
            None,
            false,
        );
    }
    let mut result = CallToolResult::success(vec![ContentBlock::text(summary)]);
    result.structured_content = Some(json!({"data":data}));
    result
}
fn error(
    code: &str,
    message: &str,
    retryable: bool,
    diagnostic: Option<String>,
    unknown: bool,
) -> CallToolResult {
    let mut value = json!({"code":code,"message":message,"retryable":retryable});
    if code == "internal_error" {
        let diagnostic = diagnostic.unwrap_or_else(|| {
            let time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |v| v.as_nanos());
            let id = format!("mcp-{time}-{}", DIAGNOSTICS.fetch_add(1, Ordering::Relaxed));
            eprintln!("MCP tool internal error: {id}");
            id
        });
        value["diagnostic_id"] = json!(diagnostic);
    }
    if unknown {
        value["outcome"] = json!("unknown");
    }
    let mut result = CallToolResult::error(vec![ContentBlock::text(message)]);
    result.structured_content = Some(json!({"error":value}));
    result
}
fn failure(cause: Failure) -> CallToolResult {
    failure_with_profile(cause, None)
}
fn failure_with_profile(cause: Failure, profile: Option<&str>) -> CallToolResult {
    match cause {
        Failure::Remote(ClientError::Remote(e), _)
            if matches!(
                e.code.as_str(),
                "mcp_tool_write_outcome_unknown"
                    | "desktop_result_unknown"
                    | "external_write_outcome_unknown"
            ) =>
        {
            error(
                "core_write_outcome_unknown",
                "Core's effect response is unavailable. Do not retry automatically; recover with the original tool, arguments and key.",
                false,
                None,
                true,
            )
        }
        Failure::Remote(ClientError::Remote(e), _)
            if [
                "mcp_tool_provider_unavailable",
                "mcp_tool_unavailable",
                "mcp_tool_interrupted",
                "mcp_tool_in_progress",
                "mcp_tool_output_invalid",
            ]
            .contains(&e.code.as_str()) =>
        {
            error(
                &e.code,
                "The extension tool is unavailable or did not complete. Inspect any accepted Core effect before retrying.",
                false,
                None,
                false,
            )
        }
        Failure::Remote(ClientError::Remote(e), writing) => {
            let contract: Value = match serde_json::from_str(CONTRACT) {
                Ok(v) => v,
                Err(_) => return failure(Failure::InvalidCore),
            };
            let profile_allows = profile.is_some_and(|profile| {
                static NATIVE: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
                let manifest = NATIVE.get_or_init(|| {
                    serde_json::from_str(include_str!(
                        "../../../specs/rpc/native-sdk-v1.methods.json"
                    ))
                    .unwrap_or(Value::Null)
                });
                manifest["errorProfiles"][profile]
                    .as_array()
                    .is_some_and(|codes| codes.iter().any(|code| code == &e.code))
            });
            if !contract["$defs"]["toolError"]["properties"]["code"]["enum"]
                .as_array()
                .is_some_and(|codes| {
                    profile_allows
                        || codes.iter().any(|code| code == &e.code)
                        || matches!(
                            e.code.as_str(),
                            "package_candidate_evidence_stale"
                                | "package_candidate_limit_exceeded"
                                | "client_registry_stale"
                                | "client_registry_not_found"
                                | "desktop_unavailable"
                                | "project_directory_missing"
                                | "project_directory_not_directory"
                                | "project_directory_open_failed"
                                | "resource_revision_conflict"
                                | "unity_hub_download_open_failed"
                                | "unity_window_not_found"
                                | "unity_window_focus_rejected"
                                | "unity_window_focus_failed"
                        )
                })
                || e.message.len() > 1024
            {
                return failure(Failure::InvalidCore);
            }
            let mut result = error(
                &e.code,
                &e.message,
                !writing && e.code == "store_unavailable",
                e.diagnostic_id,
                false,
            );
            if let Some(reason) = e
                .data
                .as_ref()
                .and_then(|v| v.get("subreason"))
                .and_then(Value::as_str)
                && reason.len() <= 128
                && reason
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_')
                && let Some(value) = result.structured_content.as_mut()
            {
                value["error"]["details"] = json!({"subreason":reason});
            }
            result
        }
        Failure::Remote(_, true) => error(
            "core_write_outcome_unknown",
            "Core's write response was lost. Do not retry automatically. For an idempotent write, recover with the original key and arguments. For a lost plan response, review the project before explicitly creating another plan.",
            false,
            None,
            true,
        ),
        Failure::Remote(_, false) => error(
            "core_unavailable",
            "The Core connection is unavailable. Reconnect with the same client identity.",
            true,
            None,
            false,
        ),
        Failure::InvalidCore => error(
            "internal_error",
            "Core returned a result this tool contract cannot safely represent.",
            false,
            None,
            false,
        ),
    }
}

pub(crate) fn interrupted(writing: bool) -> CallToolResult {
    failure(Failure::Remote(
        ClientError::Transport(std::io::Error::from(std::io::ErrorKind::TimedOut)),
        writing,
    ))
}

pub async fn execute(
    client: &mut AlcomdClient,
    params: CallToolRequestParams,
    events_config: Option<&alcomd_client::ClientConfig>,
) -> Result<CallToolResult, ErrorData> {
    let descriptor = match descriptor(client, params.name.as_ref()).await {
        Ok(value) => value,
        Err(error) => return Ok(failure(Failure::Remote(error, false))),
    };
    if let Some(descriptor) = descriptor.as_ref().filter(|entry| entry.origin.is_some()) {
        if params.input_responses.is_some() || params.request_state.is_some() {
            return Err(invalid());
        }
        let arguments = Value::Object(params.arguments.unwrap_or_default());
        let writing = descriptor_writes(descriptor) || arguments.get("idempotency_key").is_some();
        let value = match client
            .extension_mcp_tools_call(rpc::ExtensionMcpToolsCallParams {
                name: params.name.into_owned(),
                arguments,
            })
            .await
        {
            Ok(value) => value,
            Err(error) => return Ok(failure(Failure::Remote(error, writing))),
        };
        let accepted = value.core_effect.is_some();
        let mut result = if value.error.is_some() {
            CallToolResult::error(vec![ContentBlock::text(if accepted {
                "Core accepted an action, but the extension did not complete. Inspect coreEffect; do not retry automatically."
            } else {
                "The extension tool did not complete."
            })])
        } else {
            CallToolResult::success(vec![ContentBlock::text(if value.replayed {
                "Restored the accepted Core effect without rerunning the extension."
            } else {
                "Extension tool completed."
            })])
        };
        let mut output = json!({"data":value.data,"coreEffect":value.core_effect,"replayed":value.replayed,"error":null});
        if let Some(error) = value.error {
            output["error"] = json!({"code":error,"retryable":false});
        }
        result.structured_content = Some(output);
        return Ok(result);
    }
    if let Some(capability) = rpc::mcp_workflows_tool_capability(params.name.as_ref()) {
        if !client.supports_capability(capability) {
            return Err(super::unsupported_method());
        }
        return workflows::execute(client, params).await;
    }
    if let Some(capability) = rpc::mcp_extension_management_capability(params.name.as_ref()) {
        if !client.supports_capability(capability) {
            return Err(super::unsupported_method());
        }
        return extensions::execute(client, params).await;
    }
    if let Some(required) = rpc::mcp_project_catalog_capabilities(params.name.as_ref()) {
        if !required
            .iter()
            .any(|capability| client.supports_capability(capability))
        {
            return Err(super::unsupported_method());
        }
        return project_catalog::execute(client, params).await;
    }
    if operations::recognized(params.name.as_ref()) {
        if rpc::mcp_operation_tool_capability(params.name.as_ref())
            .is_some_and(|cap| !client.supports_capability(cap))
        {
            return Err(super::unsupported_method());
        }
        return operations::execute(client, params, events_config).await;
    }
    if environment::recognized(params.name.as_ref()) {
        if rpc::mcp_environment_tool_capability(params.name.as_ref())
            .is_some_and(|capability| !client.supports_capability(capability))
        {
            return Err(super::unsupported_method());
        }
        return environment::execute(client, params).await;
    }
    if packages::recognized(params.name.as_ref()) {
        if !client.supports_capability(rpc::CAPABILITY_MCP_PACKAGES_CATALOG_PLAN_V1) {
            return Err(super::unsupported_method());
        }
        return packages::execute(client, params).await;
    }
    execute_builtin(client, params).await
}
async fn execute_builtin(
    client: &mut AlcomdClient,
    params: CallToolRequestParams,
) -> Result<CallToolResult, ErrorData> {
    let input = parse(params)?;
    if !client.is_usable() {
        return Ok(error(
            "core_unavailable",
            "The interrupted Core connection cannot be reused. Establish a new connection with the same client identity.",
            true,
            None,
            false,
        ));
    }
    let result:Result<(Value,String,bool),Failure>=async {
        match input {
            Input::List(p) => {
                let page=client.projects_list_with_view(rpc::ProjectsListParams {
                    cursor:p.cursor.map(|c|rpc::ProjectRegistryCursor{registered_at_ms:c.registered_at_ms,id:c.project_id,authorization_binding:c.authorization_binding}),
                    limit:p.limit,view:p.view,expected_view_sequence:p.expected_view_sequence,
                }).await.map_err(read)?;
                let items=page.projects.into_iter().map(project).collect::<Result<Vec<_>,_>>()?;
                let mut data=json!({"projects":items});
                if let Some(sequence)=page.view_sequence {data["view_sequence"]=json!(sequence);}
                if let Some(c)=page.next_cursor {let mut cursor=json!({"registered_at_ms":c.registered_at_ms,"project_id":c.id});if let Some(binding)=c.authorization_binding{cursor["authorization_binding"]=json!(binding);}data["next_cursor"]=cursor;}
                Ok((data,"Authorized registered projects.".into(),false))
            }
            Input::Project(p) => Ok((project(client.project_get(p.project_id).await.map_err(read)?.project)?,"Registered package snapshot; planning rechecks current project files.".into(),false)),
            Input::Plan(p) => {
                let plan=client.package_plan_remove(rpc::PackagePlanRemoveParams{project_id:p.project_id,expected_revision:p.expected_revision,package_id:p.package_id}).await.map_err(write)?;
                if plan.action!=rpc::PackagePlanAction::Remove || plan.change_set.mutations.iter().any(|m|m.kind!=rpc::PackageMutationKind::Remove || m.source.is_some()) {return Err(Failure::InvalidCore);}
                let summary=format!("Removal plan {}: {} package changes, {} dependency-removal warnings, {} legacy paths. Review the entire structured change set and confirm before applying. No project files have been changed.",plan.plan_id,plan.change_set.mutations.len(),plan.change_set.removal_conflicts.len(),plan.change_set.legacy_removals.len());
                Ok((snake(serde_json::to_value(plan).map_err(|_|Failure::InvalidCore)?),summary,true))
            }
            Input::Apply(p) => {
                let value=client.package_apply_plan(rpc::PackageApplyPlanParams{plan_id:p.plan_id,expected_revision:p.expected_revision,idempotency_key:p.idempotency_key}).await.map_err(write)?;
                Ok((json!({"operation_id":value.operation_id,"replayed":value.replayed}),"Operation accepted. Query its state to learn whether the package changes succeeded.".into(),false))
            }
            Input::Operation(p) => Ok((operation(client.operation_get(p.operation_id).await.map_err(read)?)?,"Authoritative operation state and available result.".into(),false)),
            Input::OperationsList(p) => {
                let cursor = p.cursor.map(|c| rpc::OperationsListCursor { created_at_ms: c.created_at_ms, operation_id: c.operation_id });
                let page = client.operations_list(cursor, p.limit, p.active_only).await.map_err(read)?;
                let operations = page.operations.into_iter().map(operation).collect::<Result<Vec<_>, _>>()?;
                let mut data = json!({"operations": operations, "active_count": page.active_count});
                if let Some(next) = page.next_cursor {
                    data["next_cursor"] = json!({"created_at_ms":next.created_at_ms,"operation_id":next.operation_id});
                }
                Ok((data,"Authoritative operations owned by this client.".into(),false))
            }
            Input::Cancel(p) => {
                let result=client.operation_cancel(p.operation_id,p.expected_revision,p.idempotency_key).await.map_err(write)?;
                Ok((json!({"operation":operation(result.operation)?,"replayed":result.replayed}),"Cancellation response. Only the returned state determines whether the task is cancelled.".into(),false))
            }
        }
    }.await;
    Ok(match result {
        Ok((data, text, plan)) => success(data, text, plan),
        Err(e) => failure(e),
    })
}

pub(crate) fn descriptor_writes(value: &rpc::McpToolDescriptor) -> bool {
    matches!(value.effect.as_str(), "plan" | "apply" | "cancel")
}
pub(crate) async fn descriptor(
    client: &mut AlcomdClient,
    name: &str,
) -> Result<Option<rpc::McpToolDescriptor>, ClientError> {
    let page = client
        .mcp_tools_catalog(rpc::McpToolsCatalogParams {
            name: Some(name.into()),
            cursor: None,
            limit: None,
        })
        .await?;
    Ok(page.tools.into_iter().find(|t| t.name == name))
}
pub(crate) async fn list(
    client: &mut AlcomdClient,
    cursor: Option<String>,
) -> Result<rmcp::model::ListToolsResult, ErrorData> {
    let page = client
        .mcp_tools_catalog(rpc::McpToolsCatalogParams {
            name: None,
            cursor,
            limit: Some(16),
        })
        .await
        .map_err(|_| {
            ErrorData::internal_error(
                "The current authorized tool catalogue is unavailable.",
                None,
            )
        })?;
    let tools = page.tools.into_iter().map(|tool| {
        let writing = descriptor_writes(&tool);
        let annotations = if tool.origin.is_none() {
            builtin_definition(&tool.name)?.and_then(|definition| definition.annotations)
        } else {
            None
        };
        let annotations = annotations.map_or_else(
            || json!({"readOnlyHint":!writing,"destructiveHint":writing,"idempotentHint":!writing,"openWorldHint":false}),
            |value| json!(value),
        );
        serde_json::from_value::<Tool>(json!({"name":tool.name,"title":tool.title,"description":tool.description,"inputSchema":tool.input_schema,"outputSchema":tool.output_schema,"annotations":annotations})).map_err(|_|internal_protocol())
    }).collect::<Result<Vec<_>,_>>()?;
    let mut result = rmcp::model::ListToolsResult::with_all_items(tools)
        .with_ttl_ms(0)
        .with_cache_scope(rmcp::model::CacheScope::Private);
    result.next_cursor = page.next_cursor;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn template_write_hints_reflect_destructive_and_idempotent_behavior() {
        for (name, destructive) in [
            ("alcomd_edit_project_template", true),
            ("alcomd_create_project_template", true),
            ("alcomd_remove_project_template", true),
            ("alcomd_export_project_template", false),
        ] {
            let tool = builtin_definition(name)
                .expect("valid template tool definition")
                .expect("published template tool");
            let flags = serde_json::to_value(tool.annotations).expect("published hints");
            assert_eq!(flags["readOnlyHint"], false);
            assert_eq!(flags["destructiveHint"], destructive);
            assert_eq!(flags["idempotentHint"], true);
        }
    }

    #[test]
    fn input_contract_rejects_missing_confirmation_unknown_fields_and_unsafe_revisions() {
        let id = "11111111-1111-4111-8111-111111111111";
        for args in [
            json!({"plan_id":id,"expected_revision":1,"idempotency_key":"key"}),
            json!({"plan_id":id,"expected_revision":1,"idempotency_key":"key","confirm":false}),
            json!({"plan_id":id,"expected_revision":MAX_SAFE+1,"idempotency_key":"key","confirm":true}),
            json!({"plan_id":id,"expected_revision":1,"idempotency_key":"key","confirm":true,"force":true}),
        ] {
            assert!(
                parse(
                    CallToolRequestParams::new("alcomd_apply_package_changes")
                        .with_arguments(args.as_object().expect("object").clone())
                )
                .is_err()
            );
        }
        assert!(
            parse(
                CallToolRequestParams::new("alcomd_list_projects")
                    .with_arguments(json!({"cursor":null}).as_object().expect("object").clone())
            )
            .is_err()
        );
    }
    #[test]
    fn definitions_are_exact_and_errors_do_not_turn_unknown_writes_into_failures() {
        let names = [
            "alcomd_list_projects",
            "alcomd_get_project",
            "alcomd_plan_package_uninstall",
            "alcomd_apply_package_changes",
            "alcomd_get_operation",
            "alcomd_cancel_operation",
            "alcomd_list_package_sources",
            "alcomd_search_packages",
            "alcomd_get_package",
            "alcomd_plan_package_install",
            "alcomd_plan_package_reinstall",
            "alcomd_plan_package_changes",
            "alcomd_list_operations",
        ];
        for name in names {
            let tool = builtin_definition(name)
                .expect("approved contract")
                .expect("known builtin");
            assert_eq!(tool.name, name);
        }
        assert!(
            builtin_definition("alcomd_any_rpc")
                .expect("static admission")
                .is_none()
        );
        assert_eq!(
            builtin_definition("alcomd_plan_package_install")
                .expect("valid contract")
                .expect("known plan")
                .annotations
                .and_then(|value| value.read_only_hint),
            Some(false)
        );
        let result = failure(Failure::Remote(
            ClientError::Transport(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "secret-path-must-not-leak",
            )),
            true,
        ));
        let wire = serde_json::to_value(result).expect("result");
        assert_eq!(wire["isError"], true);
        assert_eq!(wire["resultType"], "complete");
        assert_eq!(wire["structuredContent"]["error"]["outcome"], "unknown");
        assert_eq!(wire["structuredContent"]["error"]["retryable"], false);
        assert!(
            wire["structuredContent"]["error"]["message"]
                .as_str()
                .expect("message")
                .contains("lost plan response")
        );
        assert!(!wire.to_string().contains("secret-path-must-not-leak"));
        let unknown = parse(
            CallToolRequestParams::new("alcomd_any_rpc")
                .with_arguments(json!({"value":null}).as_object().expect("object").clone()),
        );
        assert!(matches!(unknown,Err(e) if e.code==rmcp::model::ErrorCode::METHOD_NOT_FOUND));
        assert!(safe_numbers(&json!({"revision":MAX_SAFE})));
        assert!(!safe_numbers(&json!({"revision":MAX_SAFE+1})));
    }
    #[test]
    fn project_cursor_binding_is_exposed_and_preserved_without_adapter_authority() {
        let contract: Value = serde_json::from_str(CONTRACT).expect("contract");
        assert_eq!(
            contract["$defs"]["cursor"]["properties"]["authorization_binding"]["maxLength"],
            64
        );
        for binding in [None, Some(json!("a".repeat(64)))] {
            let mut cursor =
                json!({"registered_at_ms":1,"project_id":"00000000-0000-4000-8000-000000000101"});
            if let Some(value) = binding {
                cursor["authorization_binding"] = value;
            }
            let result = parse(
                CallToolRequestParams::new("alcomd_list_projects")
                    .with_arguments(json!({"cursor":cursor}).as_object().expect("args").clone()),
            )
            .expect("optional field");
            let Input::List(list) = result else {
                panic!("list")
            };
            assert_eq!(
                list.cursor
                    .expect("cursor")
                    .authorization_binding
                    .as_deref(),
                cursor.get("authorization_binding").and_then(Value::as_str)
            );
        }
        let invalid = json!({"cursor":{"registered_at_ms":1,"project_id":"00000000-0000-4000-8000-000000000101","authorization_binding":null}});
        assert!(
            parse(
                CallToolRequestParams::new("alcomd_list_projects")
                    .with_arguments(invalid.as_object().expect("args").clone())
            )
            .is_err()
        );
    }
}
