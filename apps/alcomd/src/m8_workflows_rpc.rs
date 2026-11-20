use alcomd_application::{self as app, M7CopyStore};
use alcomd_protocol as rpc;
use std::path::{Component, Path, PathBuf};

use super::{Applications, ConnectionState, DispatchAction, error_action, invalid, success_action};

struct Destination {
    parent: PathBuf,
    identity: Vec<u8>,
    leaf: String,
}

fn ordinary_leaf(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.ends_with(['.', ' '])
        && !name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
        && !name.chars().any(char::is_control)
}

fn absolute_path(text: &str) -> Result<PathBuf, rpc::RpcError> {
    let path = Path::new(text);
    if !path.is_absolute()
        || text.len() > 4096
        || text.chars().any(char::is_control)
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(rpc::RpcError::invalid_request());
    }
    #[cfg(windows)]
    if !matches!(
        path.components().next(),
        Some(Component::Prefix(prefix))
            if matches!(
                prefix.kind(),
                std::path::Prefix::Disk(_)
                    | std::path::Prefix::VerbatimDisk(_)
                    | std::path::Prefix::UNC(_, _)
                    | std::path::Prefix::VerbatimUNC(_, _)
            )
    ) {
        return Err(rpc::RpcError::invalid_request());
    }
    Ok(path.to_owned())
}

async fn resolve(directory: PathBuf, leaf: String) -> Result<Destination, rpc::RpcError> {
    tokio::task::spawn_blocking(move || {
        let (parent, identity) = alcomd_platform::resolve_plain_storage_directory(&directory)
            .map_err(|_| rpc::RpcError::permission_denied())?;
        Ok(Destination {
            parent,
            identity,
            leaf,
        })
    })
    .await
    .map_err(|_| rpc::RpcError::store_unavailable())?
}

fn destination_input(destination: &Destination) -> String {
    destination.parent.to_string_lossy().into_owned()
}

pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    apps: &Applications,
    access: &app::AccessContext,
) -> DispatchAction {
    if let Some(error) =
        super::require_capability(&request.id, state, rpc::CAPABILITY_MCP_PROJECT_WORKFLOWS_V2)
    {
        return error;
    }
    let id = request.id;
    let Ok(authentication) = super::m8_tools_rpc::auth(state, access) else {
        return error_action(Some(id), rpc::RpcError::permission_denied(), false);
    };
    app::with_workflow_authentication(
        authentication,
        app::with_log_caller(apps.store.clone(), access, async {
            match request.method.as_str() {
                rpc::METHOD_MCP_PROJECTS_REGISTER => {
                    let Ok(params) = serde_json::from_value(request.params) else {
                        return invalid(id);
                    };
                    register(id, apps, access, params).await
                }
                rpc::METHOD_MCP_PROJECTS_CREATE => {
                    let Ok(params) = serde_json::from_value(request.params) else {
                        return invalid(id);
                    };
                    create(id, apps, access, params).await
                }
                rpc::METHOD_MCP_PROJECTS_PLAN_COPY => {
                    let Ok(params) = serde_json::from_value(request.params) else {
                        return invalid(id);
                    };
                    plan_copy(id, apps, access, params).await
                }
                rpc::METHOD_MCP_PROJECTS_APPLY_COPY => {
                    let Ok(params) = serde_json::from_value(request.params) else {
                        return invalid(id);
                    };
                    apply_copy(id, apps, access, params).await
                }
                _ => error_action(Some(id), rpc::RpcError::method_not_found(), false),
            }
        }),
    )
    .await
}

async fn register(
    id: String,
    apps: &Applications,
    access: &app::AccessContext,
    params: rpc::McpProjectsRegisterParams,
) -> DispatchAction {
    let Ok(key) = app::IdempotencyKey::parse(params.idempotency_key.clone()) else {
        return invalid(id);
    };
    let Ok(encoded) = serde_json::to_string(&params) else {
        return invalid(id);
    };
    let fingerprint = format!(
        "\"{}\"",
        app::oauth_hex(&app::mcp_credential_hash(&encoded))
    );
    match apps
        .m3
        .replay_mcp_project_registration(access, fingerprint.clone(), key.clone())
        .await
    {
        Ok(Some(outcome)) => return registration_action(id, Ok(outcome)),
        Ok(None) => {}
        Err(error) => return super::m3_rpc::m3_error(id, error),
    }
    let resolved = match params.source {
        rpc::McpProjectSource::Default { relative_path } => {
            if relative_path.len() > 4096 || !relative_path.split(['/', '\\']).all(ordinary_leaf) {
                return invalid(id);
            }
            let directories = match apps.official_gui.effective_directories().await {
                Ok(dirs) => dirs,
                Err(_) => return error_action(Some(id), rpc::RpcError::store_unavailable(), false),
            };
            let relative: PathBuf = relative_path.split(['/', '\\']).collect();
            let root = PathBuf::from(directories.project_directory);
            match resolve(root, String::new()).await {
                Ok(base) => resolve(base.parent.join(relative), String::new())
                    .await
                    .and_then(|value| {
                        value
                            .parent
                            .starts_with(&base.parent)
                            .then_some(value)
                            .ok_or_else(rpc::RpcError::permission_denied)
                    }),
                Err(error) => Err(error),
            }
        }
        rpc::McpProjectSource::Absolute { path } => {
            let path = match absolute_path(&path) {
                Ok(path) => path,
                Err(error) => return error_action(Some(id), error, false),
            };
            resolve(path, String::new()).await
        }
    };
    let destination = match resolved {
        Ok(destination) => destination,
        Err(error) => return error_action(Some(id), error, false),
    };
    let path = destination_input(&destination);
    registration_action(
        id,
        apps.m3
            .register_mcp_project(access, path, destination.identity, fingerprint, key)
            .await,
    )
}

fn registration_action(
    id: String,
    result: Result<app::SyncWrite<app::McpRegisteredProject>, app::M3Error>,
) -> DispatchAction {
    match result {
        Ok(value) => match serde_json::to_value(value.value).and_then(serde_json::from_value) {
            Ok(project) => success_action(
                id,
                rpc::McpProjectsRegisterResult {
                    project,
                    replayed: value.replayed,
                },
                None,
            ),
            Err(_) => error_action(
                Some(id),
                rpc::RpcError::internal(app::OperationId::new().to_string()),
                false,
            ),
        },
        Err(error) => super::m3_rpc::m3_error(id, error),
    }
}

async fn create(
    id: String,
    apps: &Applications,
    access: &app::AccessContext,
    params: rpc::McpProjectsCreateParams,
) -> DispatchAction {
    let Ok(key) = app::IdempotencyKey::parse(params.idempotency_key.clone()) else {
        return invalid(id);
    };
    let Ok(encoded) = serde_json::to_string(&params) else {
        return invalid(id);
    };
    let fingerprint = format!(
        "\"{}\"",
        app::oauth_hex(&app::mcp_credential_hash(&encoded))
    );
    match apps
        .project_create
        .replay_mcp_create(access, fingerprint.clone(), key.clone())
        .await
    {
        Ok(Some(outcome)) => return create_action(id, Ok(outcome)),
        Ok(None) => {}
        Err(error) => return super::project_create_rpc::project_create_error(id, error),
    }
    let destination = match project_target(apps, params.target).await {
        Ok(destination) => destination,
        Err(error) => return error_action(Some(id), error, false),
    };
    let parent = destination_input(&destination);
    let input = app::ProjectCreateInput {
        name: destination.leaf,
        target_parent: parent,
        unity_version: params.unity_version,
        template: params.template.map(|template| app::ProjectTemplateRef {
            id: template.id,
            revision: template.revision,
        }),
    };
    create_action(
        id,
        apps.project_create
            .create_mcp_project(access, input, key, fingerprint, destination.identity)
            .await,
    )
}

fn create_action(
    id: String,
    result: Result<app::ProjectCreateAccepted, app::ProjectCreateError>,
) -> DispatchAction {
    match result {
        Ok(value) => success_action(
            id,
            rpc::ProjectsCreateResult {
                operation_id: value.operation_id.to_string(),
                project_id: value.project_id.to_string(),
                replayed: value.replayed,
            },
            None,
        ),
        Err(error) => super::project_create_rpc::project_create_error(id, error),
    }
}

async fn project_target(
    apps: &Applications,
    target: rpc::McpProjectTarget,
) -> Result<Destination, rpc::RpcError> {
    let (parent, leaf) = match target {
        rpc::McpProjectTarget::Default { name } => {
            let directories = apps
                .official_gui
                .effective_directories()
                .await
                .map_err(|_| rpc::RpcError::store_unavailable())?;
            (PathBuf::from(directories.project_directory), name)
        }
        rpc::McpProjectTarget::Absolute { path } => {
            let path = absolute_path(&path)?;
            let parent = path.parent().ok_or_else(rpc::RpcError::invalid_request)?;
            let leaf = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(rpc::RpcError::invalid_request)?;
            (parent.to_owned(), leaf.to_owned())
        }
    };
    if !ordinary_leaf(&leaf) {
        return Err(rpc::RpcError::invalid_request());
    }
    resolve(parent, leaf).await
}

async fn check_apply_target(expected: &app::ProjectCopyPlanDraft) -> Result<(), rpc::RpcError> {
    let parent = PathBuf::from(&expected.target_parent_path);
    let current = resolve(parent.clone(), expected.target_leaf.clone()).await?;
    if current.parent != parent || current.identity != expected.target_parent_identity {
        return Err(rpc::RpcError::revision_conflict());
    }
    let path = current.parent.join(current.leaf);
    match tokio::fs::symlink_metadata(path).await {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(rpc::RpcError::revision_conflict()),
        Err(_) => Err(rpc::RpcError::permission_denied()),
    }
}

async fn plan_copy(
    id: String,
    apps: &Applications,
    access: &app::AccessContext,
    params: rpc::McpProjectsPlanCopyParams,
) -> DispatchAction {
    if access.require(app::Permission::ProjectsRead).is_err()
        || access.require(app::Permission::ProjectsCreate).is_err()
    {
        return error_action(Some(id), rpc::RpcError::permission_denied(), false);
    }
    let (Ok(source), Some(revision), Ok(key)) = (
        app::ProjectId::parse(&params.source_project_id),
        app::Revision::new(params.expected_revision),
        app::IdempotencyKey::parse(params.idempotency_key),
    ) else {
        return invalid(id);
    };
    let destination = match project_target(apps, params.target).await {
        Ok(value) => value,
        Err(error) => return error_action(Some(id), error, false),
    };
    let result = app::with_workflow_parent_identity(
        destination.identity,
        apps.project_copy.plan_copy(
            access,
            source,
            revision,
            destination.parent,
            destination.leaf,
            key,
        ),
    )
    .await;
    let result = match result {
        Ok(value) => value,
        Err(error) => return super::m7_copy_rpc::copy_error(id, error),
    };
    let plan = super::m7_copy_rpc::plan(result.plan);
    success_action(
        id,
        rpc::McpProjectsCopyPlan {
            plan_id: plan.plan_id,
            source_project_id: plan.source_project_id,
            expected_revision: plan.source_project_revision,
            target_name: plan.normalized_target_leaf,
            target_project_id: plan.target_project_id,
            writer_evidence: plan.writer_evidence,
            profile: plan.profile,
            expires_at_ms: plan.expires_at_ms,
            plan_fingerprint: plan.plan_fingerprint,
            replayed: result.replayed,
        },
        None,
    )
}

async fn apply_copy(
    id: String,
    apps: &Applications,
    access: &app::AccessContext,
    params: rpc::McpProjectsApplyCopyParams,
) -> DispatchAction {
    let (true, Ok(plan_id), Some(revision), Ok(key)) = (
        params.confirm,
        app::PlanId::parse(&params.plan_id),
        app::Revision::new(params.expected_revision),
        app::IdempotencyKey::parse(params.idempotency_key),
    ) else {
        return invalid(id);
    };
    let caller = access.principal();
    match apps
        .store
        .replay_project_copy_apply(caller.clone(), plan_id, revision, key.clone())
        .await
    {
        Ok(Some(value)) => return copy_action(id, Ok(value)),
        Ok(None) => {}
        Err(error) => return super::m7_copy_rpc::copy_error(id, error),
    }
    let plan = match apps
        .store
        .get_project_copy_plan(caller.clone(), plan_id)
        .await
    {
        Ok(value) => value,
        Err(error) => return super::m7_copy_rpc::copy_error(id, error),
    };
    if let Err(error) = check_apply_target(&plan.draft).await {
        return error_action(Some(id), error, false);
    }
    copy_action(
        id,
        apps.project_copy
            .apply_copy(access, plan_id, revision, key)
            .await,
    )
}

fn copy_action(
    id: String,
    result: Result<app::ProjectCopyApplyOutcome, app::M7CopyError>,
) -> DispatchAction {
    match result {
        Ok(value) => success_action(
            id,
            rpc::ProjectsApplyCopyResult {
                operation_id: value.operation_id.to_string(),
                target_project_id: value.target_project_id.to_string(),
                replayed: value.replayed,
            },
            None,
        ),
        Err(error) => super::m7_copy_rpc::copy_error(id, error),
    }
}
