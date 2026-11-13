//! The six approved tools. Every business action uses the existing typed Core client.
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
const MAX_SAFE: u64 = 9_007_199_254_740_991;
static DIAGNOSTICS: AtomicU64 = AtomicU64::new(1);

pub fn definitions() -> Result<Vec<Tool>, ErrorData> {
    let contract: Value = serde_json::from_str(CONTRACT).map_err(|_| internal_protocol())?;
    let entries = contract["x-alcomd-tools"]
        .as_array()
        .ok_or_else(internal_protocol)?;
    entries
        .iter()
        .map(|entry| {
            let mut entry = entry.clone();
            for schema in ["inputSchema", "outputSchema"] {
                entry[schema]["$defs"] = contract["$defs"].clone();
            }
            serde_json::from_value(entry).map_err(|_| internal_protocol())
        })
        .collect()
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
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct List {
    cursor: Option<Cursor>,
    limit: Option<u32>,
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
        "alcomd_cancel_operation" => {
            Input::Cancel(serde_json::from_value(value).map_err(|_| invalid())?)
        }
        _ => return Err(super::unsupported_method()),
    };
    let valid = match &input {
        Input::List(p) => {
            p.limit.is_none_or(|v| (1..=1000).contains(&v))
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
    UnsupportedKind,
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
fn project(value: rpc::ProjectSnapshot, detail: bool) -> Result<Value, Failure> {
    let id = value
        .project_id
        .filter(|s| canonical_id(s))
        .ok_or(Failure::InvalidCore)?;
    let rev = value
        .revision
        .filter(|v| revision(*v))
        .ok_or(Failure::InvalidCore)?;
    let state = value.directory_state.ok_or(Failure::InvalidCore)?;
    let mut output = json!({"project_id":id,"revision":rev,"project_type":value.project_type,"unity_version":value.unity_version,"directory_state":state});
    if let Some(name) = value.display_name {
        output["display_name"] = json!(name);
    }
    if detail {
        output["vpm_manifest"] = json!(value.vpm_manifest);
        output["upm_manifest"] = json!(value.upm_manifest);
        output["direct_dependencies"] = snake(json!(value.direct_dependencies));
        output["locked_dependencies"] = snake(json!(value.locked_dependencies));
        output["issues"] = Value::Array(
            value
                .issues
                .into_iter()
                .map(|issue| {
                    let mut issue_value = json!({"code":issue.code,"component":issue.component});
                    if let Some(line) = issue.line {
                        issue_value["line"] = json!(line);
                    }
                    if let Some(column) = issue.column {
                        issue_value["column"] = json!(column);
                    }
                    issue_value
                })
                .collect(),
        );
    }
    Ok(output)
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PackageResult {
    plan_id: String,
    change_set_fingerprint: String,
}
fn operation(value: rpc::Operation) -> Result<Value, Failure> {
    if !canonical_id(&value.operation_id) || !revision(value.revision) {
        return Err(Failure::InvalidCore);
    }
    if value.kind != "packages.apply" {
        return Err(Failure::UnsupportedKind);
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
    match cause {
        Failure::Remote(ClientError::Remote(e), writing) => {
            let contract: Value = match serde_json::from_str(CONTRACT) {
                Ok(v) => v,
                Err(_) => return failure(Failure::InvalidCore),
            };
            if !contract["$defs"]["toolError"]["properties"]["code"]["enum"]
                .as_array()
                .is_some_and(|codes| codes.iter().any(|code| code == &e.code))
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
            "Core's write response was lost. The outcome is unknown; do not retry automatically. Recover Apply/Cancel with the original key and arguments. For a lost plan response, review the project before explicitly creating another plan.",
            false,
            None,
            true,
        ),
        Failure::Remote(_, false) => error(
            "core_unavailable",
            "The Core connection is unavailable. Restart this controlled STDIO client to reconnect with the same identity.",
            true,
            None,
            false,
        ),
        Failure::UnsupportedKind => error(
            "invalid_request",
            "This toolset supports package apply operations only.",
            false,
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

pub async fn execute(
    client: &mut AlcomdClient,
    params: CallToolRequestParams,
) -> Result<CallToolResult, ErrorData> {
    let input = parse(params)?;
    if !client.is_usable() {
        return Ok(error(
            "core_unavailable",
            "The interrupted Core connection cannot be reused. Restart this controlled STDIO client.",
            true,
            None,
            false,
        ));
    }
    let result:Result<(Value,String,bool),Failure>=async {
        match input {
            Input::List(p) => {
                let page=client.projects_list(p.cursor.map(|c|rpc::RegistryCursor{registered_at_ms:c.registered_at_ms,id:c.project_id}),p.limit).await.map_err(read)?;
                let items=page.projects.into_iter().map(|p|project(p,false)).collect::<Result<Vec<_>,_>>()?;
                let mut data=json!({"projects":items});
                if let Some(c)=page.next_cursor {data["next_cursor"]=json!({"registered_at_ms":c.registered_at_ms,"project_id":c.id});}
                Ok((data,"Authorized registered projects.".into(),false))
            }
            Input::Project(p) => Ok((project(client.project_get(p.project_id).await.map_err(read)?.project,true)?,"Registered package snapshot; planning rechecks current project files.".into(),false)),
            Input::Plan(p) => {
                let plan=client.package_plan_remove(rpc::PackagePlanRemoveParams{project_id:p.project_id,expected_revision:p.expected_revision,package_id:p.package_id}).await.map_err(write)?;
                if plan.action!=rpc::PackagePlanAction::Remove || plan.change_set.mutations.iter().any(|m|m.kind!=rpc::PackageMutationKind::Remove || m.source.is_some()) {return Err(Failure::InvalidCore);}
                let summary=format!("Removal plan {}: {} package changes, {} dependency-removal warnings, {} legacy paths. Review the entire structured change set and confirm before applying. No project files have been changed.",plan.plan_id,plan.change_set.mutations.len(),plan.change_set.removal_conflicts.len(),plan.change_set.legacy_removals.len());
                Ok((snake(serde_json::to_value(plan).map_err(|_|Failure::InvalidCore)?),summary,true))
            }
            Input::Apply(p) => {
                let value=client.package_apply_plan(rpc::PackageApplyPlanParams{plan_id:p.plan_id,expected_revision:p.expected_revision,idempotency_key:p.idempotency_key}).await.map_err(write)?;
                Ok((json!({"operation_id":value.operation_id,"replayed":value.replayed}),"Operation accepted. Query its state to learn whether the removal succeeds.".into(),false))
            }
            Input::Operation(p) => Ok((operation(client.operation_get(p.operation_id).await.map_err(read)?)?,"Authoritative package operation state and result.".into(),false)),
            Input::Cancel(p) => {
                let current=client.operation_get(p.operation_id.clone()).await.map_err(read)?;
                if current.kind!="packages.apply" {return Err(Failure::UnsupportedKind);}
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

#[cfg(test)]
mod tests {
    use super::*;
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
        let definitions = definitions().expect("approved contract");
        assert_eq!(definitions.len(), 6);
        assert!(!definitions.iter().any(|t| t.name.contains("rpc")));
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
}
