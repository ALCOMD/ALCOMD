//! Extension tools use the authenticated MCP caller and the existing six Core use cases.
use super::{Applications, ConnectionState, DispatchAction, error_action, success_action};
use alcomd_application::{self as app, ToolError as E};
use alcomd_protocol as rpc;
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    sync::{Arc, atomic::AtomicBool},
};

fn now() -> Result<u64, E> {
    super::current_time_ms().map_err(|_| E::Unavailable)
}
pub(super) fn auth(
    state: &ConnectionState,
    access: &app::AccessContext,
) -> Result<app::ToolAuthentication, E> {
    if !access.is_mcp_client() {
        return Err(E::PermissionDenied);
    }
    if let Some(guard) = access.oauth_guard() {
        return Ok(app::ToolAuthentication::OAuth(guard.clone()));
    }
    state
        .mcp_hash
        .map(app::ToolAuthentication::Credential)
        .ok_or(E::PermissionDenied)
}
fn failure(id: String, error: E) -> DispatchAction {
    let error=match error {
        E::PermissionDenied=>rpc::RpcError::permission_denied(),E::RevisionConflict=>rpc::RpcError::revision_conflict(),E::IdempotencyConflict=>rpc::RpcError::idempotency_conflict(),E::InvalidInput|E::ResourceLimit=>rpc::RpcError::invalid_request(),
        _=>rpc::RpcError{code:format!("mcp_tool_{}",serde_json::to_value(error).ok().and_then(|v|v.as_str().map(str::to_owned)).unwrap_or_else(||"unavailable".into())),message:"The extension tool could not complete. Inspect any accepted Core effect before retrying.".into(),diagnostic_id:None,data:None},
    };
    error_action(Some(id), error, false)
}
#[derive(Clone)]
struct Entry {
    descriptor: app::ToolDescriptor,
    provider: Option<app::ToolProvider>,
    declaration: Option<app::ToolDeclaration>,
}
fn builtin() -> Result<Vec<Entry>, E> {
    use app::ToolOperation as O;
    let root: Value = serde_json::from_str(include_str!(
        "../../../specs/mcp/package-removal-v1.schema.json"
    ))
    .map_err(|_| E::Unavailable)?;
    root["x-alcomd-tools"]
        .as_array()
        .ok_or(E::Unavailable)?
        .iter()
        .map(|item| {
            let name = item["name"].as_str().ok_or(E::Unavailable)?;
            let op = match name {
                "alcomd_list_projects" => O::ProjectsList,
                "alcomd_get_project" => O::ProjectsGet,
                "alcomd_plan_package_uninstall" => O::PlanUninstall,
                "alcomd_apply_package_changes" => O::ApplyPlan,
                "alcomd_get_operation" | "alcomd_list_operations" => O::OperationGet,
                "alcomd_cancel_operation" => O::OperationCancel,
                _ => return Err(E::Unavailable),
            };
            let mut input = item["inputSchema"].clone();
            input["$defs"] = root["$defs"].clone();
            let mut output = item["outputSchema"].clone();
            output["$defs"] = root["$defs"].clone();
            Ok(Entry {
                descriptor: app::ToolDescriptor {
                    name: name.into(),
                    title: item["title"].as_str().ok_or(E::Unavailable)?.into(),
                    description: item["description"].as_str().ok_or(E::Unavailable)?.into(),
                    input_schema: input,
                    output_schema: output,
                    effect: op.effect(),
                    required_permissions: op
                        .permissions()
                        .iter()
                        .map(|p| p.as_str().into())
                        .collect(),
                    origin: None,
                },
                provider: None,
                declaration: None,
            })
        })
        .collect()
}
fn append_builtin_tools(entries: &mut Vec<Entry>, contract: &str) -> Result<(), E> {
    let schema: Value = serde_json::from_str(contract).map_err(|_| E::Unavailable)?;
    for entry in schema["x-alcomd-tools"].as_array().ok_or(E::Unavailable)? {
        let field = |key: &str| entry[key].as_str().map(str::to_owned).ok_or(E::Unavailable);
        let effect = match field("effect")?.as_str() {
            "read" => app::ToolOperation::ProjectsList.effect(),
            "plan" => app::ToolOperation::PlanUninstall.effect(),
            "apply" => app::ToolOperation::ApplyPlan.effect(),
            _ => return Err(E::Unavailable),
        };
        let schema_field = |key: &str| {
            let mut value = entry[key].clone();
            value["$defs"] = schema["$defs"].clone();
            value
        };
        entries.push(Entry {
            descriptor: app::ToolDescriptor {
                name: field("name")?,
                title: field("title")?,
                description: field("description")?,
                input_schema: schema_field("inputSchema"),
                output_schema: schema_field("outputSchema"),
                effect,
                required_permissions: serde_json::from_value(entry["requiredPermissions"].clone())
                    .map_err(|_| E::Unavailable)?,
                origin: None,
            },
            provider: None,
            declaration: None,
        });
    }
    Ok(())
}

fn builtin_entries(capabilities: &HashSet<String>) -> Result<Vec<Entry>, E> {
    let mut entries = builtin()?;
    append_builtin_tools(
        &mut entries,
        include_str!("../../../specs/mcp/environment-v1.schema.json"),
    )?;
    if capabilities.contains(rpc::CAPABILITY_MCP_PACKAGES_CATALOG_PLAN_V1) {
        append_builtin_tools(
            &mut entries,
            include_str!("../../../specs/mcp/package-catalog-plan-v1.schema.json"),
        )?;
        if let Some(entry) = entries
            .iter_mut()
            .find(|entry| entry.descriptor.name == "alcomd_apply_package_changes")
        {
            entry.descriptor.title = "Apply a reviewed package plan".into();
            entry.descriptor.description = "Only call after the operator has reviewed and confirmed the entire original plan. Applies that plan without replanning and returns an operation to check.".into();
        }
    }
    append_builtin_tools(
        &mut entries,
        include_str!("../../../specs/mcp/repositories-v1.schema.json"),
    )?;
    append_builtin_tools(
        &mut entries,
        include_str!("../../../specs/mcp/project-workflows-v1.schema.json"),
    )?;
    append_builtin_tools(
        &mut entries,
        include_str!("../../../specs/mcp/project-inventory-v1.schema.json"),
    )?;
    append_builtin_tools(
        &mut entries,
        include_str!("../../../specs/mcp/template-edit-v1.schema.json"),
    )?;
    entries.retain(|entry| {
        let name = &entry.descriptor.name;
        rpc::mcp_workflows_tool_capability(name)
            .or_else(|| rpc::mcp_repositories_tool_capability(name))
            .or_else(|| rpc::mcp_environment_tool_capability(name))
            .is_none_or(|capability| capabilities.contains(capability))
    });
    Ok(entries)
}

async fn entries(
    apps: &Applications,
    authentication: app::ToolAuthentication,
    capabilities: &HashSet<String>,
) -> Result<(Vec<Entry>, String), E> {
    let snapshot = apps.store.tool_snapshot(authentication.clone()).await?;
    let mut evidence =
        json!({"auth":authentication.binding(),"client":snapshot.client.revision,"providers":[]});
    let mut entries = builtin_entries(capabilities)?;
    for provider in snapshot.providers {
        let path = std::path::PathBuf::from(&provider.locator);
        let verified = tokio::task::spawn_blocking(move || {
            alcomd_extensions::inspect_extension_directory(&path)
        })
        .await
        .map_err(|_| E::Unavailable)?
        .map_err(|_| E::ProviderUnavailable)?;
        if verified.package_digest != provider.record.package_digest
            || verified.manifest.id != provider.record.extension_id
            || verified.publisher_fingerprint != provider.record.publisher_fingerprint
        {
            return Err(E::ProviderUnavailable);
        }
        evidence["providers"].as_array_mut().ok_or(E::Unavailable)?.push(json!({"id":provider.record.extension_id,"digest":provider.record.package_digest,"revision":provider.record.revision,"grantRevision":provider.record.grant_revision,"grants":provider.granted}));
        let origin = app::ToolOrigin {
            extension_id: provider.record.extension_id.clone(),
            publisher_fingerprint: provider.record.publisher_fingerprint.clone(),
            package_digest: app::oauth_hex(&provider.record.package_digest),
            version: provider.record.version.clone(),
        };
        for declaration in verified.mcp_tools {
            let required = declaration.permissions();
            if required.iter().any(|p| !provider.granted.contains(p)) {
                continue;
            }
            entries.push(Entry {
                descriptor: app::ToolDescriptor::extension(origin.clone(), &declaration)?,
                provider: Some(provider.clone()),
                declaration: Some(declaration),
            });
        }
    }
    entries.sort_by(|a, b| a.descriptor.name.cmp(&b.descriptor.name));
    if entries
        .windows(2)
        .any(|pair| pair[0].descriptor.name == pair[1].descriptor.name)
    {
        return Err(E::ProviderUnavailable);
    }
    evidence["tools"] =
        serde_json::to_value(entries.iter().map(|e| &e.descriptor).collect::<Vec<_>>())
            .map_err(|_| E::Unavailable)?;
    let fingerprint = app::tool_cursor_mac(
        &apps.tools_cursor_key,
        &serde_json::to_vec(&evidence).map_err(|_| E::Unavailable)?,
    );
    Ok((entries, fingerprint))
}

fn catalog(
    entries: Vec<Entry>,
    fingerprint: String,
    request: app::ToolCatalogRequest,
    key: &[u8; 32],
) -> Result<app::ToolCatalogPage, E> {
    if let Some(name) = request.name {
        if request.cursor.is_some() || request.limit.is_some() || name.len() > 64 {
            return Err(E::InvalidInput);
        }
        return Ok(app::ToolCatalogPage {
            tools: entries
                .into_iter()
                .filter(|e| e.descriptor.name == name)
                .map(|e| e.descriptor)
                .collect(),
            next_cursor: None,
            catalog_fingerprint: fingerprint,
        });
    }
    let limit = request.limit.unwrap_or(16);
    if !(1..=32).contains(&limit) {
        return Err(E::InvalidInput);
    }
    let mut offset = 0usize;
    if let Some(cursor) = request.cursor {
        if cursor.len() > 256 {
            return Err(E::InvalidInput);
        }
        let parts = cursor.split('.').collect::<Vec<_>>();
        if parts.len() != 3 || parts[1] != fingerprint {
            return Err(E::RevisionConflict);
        }
        offset = parts[0].parse().map_err(|_| E::RevisionConflict)?;
        let expected = app::tool_cursor_mac(key, format!("{}.{}", offset, fingerprint).as_bytes());
        if expected != parts[2] || offset > entries.len() {
            return Err(E::RevisionConflict);
        }
    }
    let mut tools = Vec::new();
    let mut bytes = 0;
    for entry in entries.iter().skip(offset).take(limit as usize) {
        let size = serde_json::to_vec(&entry.descriptor)
            .map_err(|_| E::Unavailable)?
            .len();
        if size > 262144 {
            return Err(E::ResourceLimit);
        }
        if bytes + size > 524288 && !tools.is_empty() {
            break;
        }
        bytes += size;
        tools.push(entry.descriptor.clone());
    }
    let after = offset + tools.len();
    let next_cursor = (after < entries.len()).then(|| {
        let text = format!("{after}.{fingerprint}");
        format!("{text}.{}", app::tool_cursor_mac(key, text.as_bytes()))
    });
    Ok(app::ToolCatalogPage {
        tools,
        next_cursor,
        catalog_fingerprint: fingerprint,
    })
}
struct CancelOnDrop(Arc<AtomicBool>);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::Release);
    }
}
async fn call(
    apps: &Applications,
    state: &ConnectionState,
    authentication: app::ToolAuthentication,
    request: app::ToolCallRequest,
) -> Result<app::ToolCallResult, E> {
    if request.name.len() > 64
        || serde_json::to_vec(&request.arguments)
            .map_err(|_| E::InvalidInput)?
            .len()
            > app::MCP_TOOLS_BYTES
        || !request.arguments.is_object()
    {
        return Err(E::InvalidInput);
    }
    let request_fingerprint = app::tool_request_fingerprint(&request.name, &request.arguments)?;
    if let Some(key) = request
        .arguments
        .get("idempotency_key")
        .and_then(Value::as_str)
    {
        app::IdempotencyKey::parse(key.to_owned()).map_err(|_| E::InvalidInput)?;
        if let Some(saved) = apps
            .store
            .tool_replay(
                authentication.clone(),
                request.name.clone(),
                key.into(),
                request_fingerprint.clone(),
            )
            .await?
        {
            return Ok(saved);
        }
    }
    let (items, _) = entries(apps, authentication.clone(), &HashSet::new()).await?;
    let entry = items
        .into_iter()
        .find(|e| e.descriptor.name == request.name)
        .ok_or(E::PermissionDenied)?;
    let provider = entry.provider.ok_or(E::InvalidInput)?;
    let declaration = entry.declaration.ok_or(E::InvalidInput)?;
    let extension_lock = apps
        .locks
        .acquire(vec![app::ResourceKey::Extension(
            provider.record.extension_id.clone(),
        )])
        .await;
    use app::M6Store;
    let current = apps
        .store
        .get_extension(provider.owner.clone(), provider.record.extension_id.clone())
        .await
        .map_err(|_| E::ProviderUnavailable)?;
    if current.package_digest != provider.record.package_digest
        || current.desired_state != app::ExtensionDesiredState::Enabled
        || current.quarantine_state != app::ExtensionQuarantineState::Clear
    {
        return Err(E::ProviderUnavailable);
    }

    let effect = declaration.effect()?;
    let mut arguments = request.arguments.clone();
    let mut control = json!({});
    let mut call_id = app::OperationId::new().to_string();
    if effect.writes() {
        let fields: &[&str] = match effect {
            app::ToolEffect::Plan => &["idempotency_key"],
            app::ToolEffect::Apply => {
                &["plan_id", "expected_revision", "idempotency_key", "confirm"]
            }
            app::ToolEffect::Cancel => &["operation_id", "expected_revision", "idempotency_key"],
            _ => &[],
        };
        for field in fields {
            control[*field] = arguments
                .as_object_mut()
                .ok_or(E::InvalidInput)?
                .remove(*field)
                .ok_or(E::InvalidInput)?;
        }
        let key = control["idempotency_key"].as_str().ok_or(E::InvalidInput)?;
        app::IdempotencyKey::parse(key.to_owned()).map_err(|_| E::InvalidInput)?;
        if effect == app::ToolEffect::Apply || effect == app::ToolEffect::Cancel {
            let field = if effect == app::ToolEffect::Apply {
                "plan_id"
            } else {
                "operation_id"
            };
            app::OperationId::parse(control[field].as_str().ok_or(E::InvalidInput)?)
                .map_err(|_| E::InvalidInput)?;
            let rev = control["expected_revision"]
                .as_u64()
                .ok_or(E::InvalidInput)?;
            if !(1..=9_007_199_254_740_991).contains(&rev) {
                return Err(E::InvalidInput);
            }
        }
        if effect == app::ToolEffect::Apply && control["confirm"] != true {
            return Err(E::InvalidInput);
        }
        let origin = entry.descriptor.origin.clone().ok_or(E::Unavailable)?;
        let fingerprint = request_fingerprint;
        let (id, accepted, status) = apps
            .store
            .tool_begin(
                authentication.clone(),
                app::ToolWriteAdmission {
                    origin,
                    name: request.name,
                    key: key.to_owned(),
                    fingerprint,
                    permissions: declaration.permissions(),
                    effect,
                },
            )
            .await?;
        call_id = id;
        if status != "new" {
            return Ok(app::ToolCallResult {
                data: Value::Null,
                core_effect: accepted.clone(),
                replayed: true,
                error: if status == "running" {
                    Some(E::InProgress)
                } else if accepted.is_none() {
                    Some(E::Interrupted)
                } else {
                    None
                },
            });
        }
    }
    let snapshot = apps.store.tool_snapshot(authentication.clone()).await?;
    let active = Arc::new(AtomicBool::new(true));
    let _cancel = CancelOnDrop(active.clone());
    let authority = app::ToolInvocation {
        call_id: call_id.clone(),
        principal: snapshot.client.principal_id.clone(),
        origin: entry.descriptor.origin.ok_or(E::Unavailable)?,
        declaration,
        authentication,
        lease: None,
        control,
        active,
        deadline_ms: now()?.checked_add(10000).ok_or(E::Unavailable)?,
    };
    // Keep the finalizer alive if the protocol future is dropped. CancelOnDrop only
    // revokes this invocation; it never cancels an already accepted Core Operation.
    let runtime = apps.extension_runtime.clone();
    let store = apps.store.clone();
    let connection = state.clone();
    let principal = snapshot.client.principal_id;
    tokio::spawn(async move {
        let _extension_lock = extension_lock;
        let result = runtime
            .invoke_tool(provider, authority, arguments, connection)
            .await;
        let core_effect = if effect.writes() {
            store
                .tool_finish(call_id, principal, result.is_err())
                .await
                .map_err(|_| E::WriteOutcomeUnknown)?
        } else {
            None
        };
        Ok(app::ToolCallResult {
            data: result.as_ref().cloned().unwrap_or(Value::Null),
            core_effect,
            replayed: false,
            error: result.err(),
        })
    })
    .await
    .map_err(|_| {
        if effect.writes() {
            E::WriteOutcomeUnknown
        } else {
            E::ProviderUnavailable
        }
    })?
}
pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    apps: &Applications,
    access: &app::AccessContext,
) -> DispatchAction {
    if let Some(action) =
        super::require_capability(&request.id, state, rpc::CAPABILITY_MCP_EXTENSION_TOOLS_V1)
    {
        return action;
    }
    let id = request.id;
    let result = async {
        let authentication = auth(state, access)?;
        match request.method.as_str() {
            rpc::METHOD_MCP_TOOLS_CATALOG => {
                let params = serde_json::from_value(request.params).map_err(|_| E::InvalidInput)?;
                let (items, fingerprint) =
                    entries(apps, authentication, &state.capabilities).await?;
                serde_json::to_value(catalog(items, fingerprint, params, &apps.tools_cursor_key)?)
                    .map_err(|_| E::Unavailable)
            }
            rpc::METHOD_EXTENSION_MCP_TOOLS_CALL => {
                let mut result = call(
                    apps,
                    state,
                    authentication,
                    serde_json::from_value(request.params).map_err(|_| E::InvalidInput)?,
                )
                .await?;
                if let Some(effect) = result.core_effect.as_mut()
                    && effect["kind"] == "plan"
                {
                    use app::M4Store;
                    let id = app::PlanId::parse(effect["planId"].as_str().ok_or(E::Unavailable)?)
                        .map_err(|_| E::Unavailable)?;
                    let plan = apps
                        .store
                        .get_package_plan(access.principal().clone(), id)
                        .await
                        .map_err(|_| E::Unavailable)?;
                    effect["plan"] = serde_json::to_value(super::m4_rpc::plan_to_rpc(plan))
                        .map_err(|_| E::Unavailable)?;
                }
                if serde_json::to_vec(&result)
                    .map_err(|_| E::Unavailable)?
                    .len()
                    > app::MCP_TOOLS_BYTES
                {
                    result.data = Value::Null;
                    if let Some(effect) = result.core_effect.as_mut()
                        && let Some(object) = effect.as_object_mut()
                    {
                        object.remove("plan");
                    }
                    result.error = Some(E::ResourceLimit);
                }
                serde_json::to_value(result).map_err(|_| E::Unavailable)
            }
            _ => Err(E::InvalidInput),
        }
    }
    .await;
    match result {
        Ok(v) => success_action(id, v, None),
        Err(e) => failure(id, e),
    }
}
/// Closed typed Host operations; no arbitrary RPC pass-through and no owner context.
pub(super) async fn business(
    apps: &Applications,
    state: &ConnectionState,
    authority: app::ToolInvocation,
    capability: &str,
    mut input: Value,
) -> Result<Value, app::M6Error> {
    use app::ToolOperation as O;
    let (op, method) = match capability {
        "host-mcp-tools.projects.list" => (O::ProjectsList, rpc::METHOD_PROJECTS_LIST),
        "host-mcp-tools.projects.get" => (O::ProjectsGet, rpc::METHOD_PROJECTS_GET),
        "host-mcp-tools.packages.planUninstall" => {
            (O::PlanUninstall, rpc::METHOD_PACKAGES_PLAN_REMOVE)
        }
        "host-mcp-tools.packages.applyPlan" => (O::ApplyPlan, rpc::METHOD_PACKAGES_APPLY_PLAN),
        "host-mcp-tools.operations.get" => (O::OperationGet, rpc::METHOD_OPERATIONS_GET),
        "host-mcp-tools.operations.cancel" => (O::OperationCancel, rpc::METHOD_OPERATIONS_CANCEL),
        _ => return Ok(json!({"error":"permission_denied"})),
    };
    let access = match apps.store.tool_check(authority.clone(), Some(op)).await {
        Ok(v) => v,
        Err(e) => return Ok(json!({"error":e})),
    };
    if op == O::ApplyPlan || op == O::OperationCancel {
        if input != json!({}) {
            return Ok(json!({"error":"invalid_input"}));
        }
        let control = &authority.control;
        input = json!({"expectedRevision":control["expected_revision"],"idempotencyKey":control["idempotency_key"]});
        if op == O::ApplyPlan {
            input["planId"] = control["plan_id"].clone();
        } else {
            input["operationId"] = control["operation_id"].clone();
        }
    }
    let dispatch = app::with_tool_invocation(
        authority.clone(),
        app::with_log_caller(
            apps.store.clone(),
            &access,
            super::dispatch_m2(
                rpc::RequestEnvelope {
                    id: app::OperationId::new().to_string(),
                    method: method.into(),
                    params: input,
                },
                state,
                apps,
                &access,
            ),
        ),
    );
    let response = app::with_workflow_authentication(authority.authentication.clone(), dispatch)
        .await
        .response;
    if !op.effect().writes()
        && let Err(e) = apps.store.tool_check(authority, Some(op)).await
    {
        return Ok(json!({"error":e}));
    }
    if let Some(result) = response.get("result") {
        // Expose the same safe project information as the builtin MCP tools, never local paths.
        fn project(value: &Value, detail: bool) -> Value {
            let mut output = json!({});
            for field in [
                "projectId",
                "revision",
                "displayName",
                "projectType",
                "unityVersion",
                "directoryState",
            ] {
                if let Some(v) = value.get(field) {
                    output[field] = v.clone();
                }
            }
            if detail {
                for field in [
                    "vpmManifest",
                    "upmManifest",
                    "directDependencies",
                    "lockedDependencies",
                ] {
                    if let Some(v) = value.get(field) {
                        output[field] = v.clone();
                    }
                }
            }
            output
        }
        let result = match op {
            O::ProjectsList => {
                json!({"projects":result["projects"].as_array().map(|a|a.iter().map(|v|project(v,false)).collect::<Vec<_>>()).unwrap_or_default(),"nextCursor":result.get("nextCursor")})
            }
            O::ProjectsGet => json!({"project":project(&result["project"],true)}),
            _ => result.clone(),
        };
        return Ok(json!({"ok":result}));
    }
    let code = response["error"]["code"].as_str().unwrap_or("unavailable");
    Ok(
        json!({"error":match code{"permission_denied"=>"permission_denied","revision_conflict"=>"revision_conflict","idempotency_conflict"=>"idempotency_conflict","invalid_request"=>"invalid_input",_=>"unavailable"}}),
    )
}

#[cfg(test)]
mod connection_access_tests {
    use super::*;

    #[test]
    fn catalog_uses_the_same_negotiated_capability_as_each_native_method() {
        for (capability, published, absent) in [
            (
                rpc::CAPABILITY_PROJECT_TEMPLATES_V1,
                "alcomd_create_project_template",
                "alcomd_edit_project_template",
            ),
            (
                rpc::CAPABILITY_TEMPLATE_EDIT_V1,
                "alcomd_edit_project_template",
                "alcomd_remove_project_template",
            ),
            (
                rpc::CAPABILITY_PROJECTS_COPY_V1,
                "alcomd_apply_project_copy",
                "alcomd_plan_project_copy",
            ),
            (
                rpc::CAPABILITY_REPOSITORIES_REGISTRY_V1,
                "alcomd_refresh_repository",
                "alcomd_inspect_repository",
            ),
        ] {
            let entries =
                builtin_entries(&HashSet::from([capability.to_owned()])).expect("tool catalog");
            assert!(
                entries
                    .iter()
                    .any(|entry| entry.descriptor.name == published)
            );
            assert!(!entries.iter().any(|entry| entry.descriptor.name == absent));
        }
    }

    #[test]
    fn connection_authorizes_every_published_builtin_tool() {
        let client_id = app::McpClientId::new();
        let client = app::McpClient {
            client_id,
            principal_id: app::PrincipalId::parse(format!("mcp:{client_id}")).expect("principal"),
            display_name: "Tool contract fixture".into(),
            revision: 1,
            created_at_ms: 0,
            revoked_at_ms: None,
        };
        let access = app::mcp_access(&client).expect("authenticated client");
        for tool in builtin_entries(
            &[
                rpc::CAPABILITY_BACKUPS_CREATE_V1,
                rpc::CAPABILITY_BACKUPS_RESTORE_V1,
                rpc::CAPABILITY_MCP_PACKAGES_CATALOG_PLAN_V1,
                rpc::CAPABILITY_MCP_PROJECT_WORKFLOWS_V1,
                rpc::CAPABILITY_MCP_REPOSITORIES_V1,
                rpc::CAPABILITY_PROJECTS_COPY_V1,
                rpc::CAPABILITY_PROJECT_INVENTORY_V1,
                rpc::CAPABILITY_PROJECT_TEMPLATES_V1,
                rpc::CAPABILITY_REPOSITORIES_REGISTRY_V1,
                rpc::CAPABILITY_TEMPLATE_EDIT_V1,
                rpc::CAPABILITY_UNITY_READ_V1,
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        )
        .expect("published tools")
        {
            assert!(
                app::require_tool_permissions(&access, &tool.descriptor.required_permissions)
                    .is_ok(),
                "{}",
                tool.descriptor.name
            );
        }
    }
}
