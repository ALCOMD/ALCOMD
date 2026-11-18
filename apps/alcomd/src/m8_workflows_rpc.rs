use alcomd_application::{self as app, M7CopyStore, McpIdentityStore};
use alcomd_protocol as rpc;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

use super::{Applications, ConnectionState, DispatchAction, error_action, invalid, success_action};

const APPROVAL_LIFETIME: Duration = Duration::from_secs(600);
const MAX_APPROVALS: usize = 64;

#[derive(Clone)]
struct Destination {
    parent: PathBuf,
    identity: Vec<u8>,
    leaf: String,
}

struct Approval {
    client: app::PrincipalId,
    purpose: rpc::McpProjectPathPurpose,
    destination: Destination,
    expires: Instant,
    request: Option<String>,
    plan: Option<app::PlanId>,
}

#[derive(Default)]
pub(super) struct PathApprovals(Mutex<HashMap<app::OperationId, Approval>>);

impl PathApprovals {
    async fn approve(
        &self,
        client: app::PrincipalId,
        purpose: rpc::McpProjectPathPurpose,
        path: PathBuf,
    ) -> Result<rpc::McpProjectPathApproveResult, rpc::RpcError> {
        if !path.is_absolute() {
            return Err(rpc::RpcError::invalid_request());
        }
        let leaf = path
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| ordinary_leaf(name))
            .ok_or_else(rpc::RpcError::invalid_request)?
            .to_owned();
        let directory = if purpose == rpc::McpProjectPathPurpose::Register {
            path
        } else {
            path.parent()
                .ok_or_else(rpc::RpcError::invalid_request)?
                .to_owned()
        };
        let destination = resolve(directory, leaf).await?;
        let expires_at_ms = super::current_time_ms()
            .map_err(|_| rpc::RpcError::store_unavailable())?
            .saturating_add(600_000);
        let mut approvals = self.0.lock().await;
        let now = Instant::now();
        approvals.retain(|_, entry| now < entry.expires);
        if approvals.len() >= MAX_APPROVALS {
            return Err(rpc::RpcError::invalid_request());
        }
        let id = app::OperationId::new();
        approvals.insert(
            id,
            Approval {
                client,
                purpose,
                destination,
                expires: now + APPROVAL_LIFETIME,
                request: None,
                plan: None,
            },
        );
        Ok(rpc::McpProjectPathApproveResult {
            approval_id: id.to_string(),
            expires_at_ms,
        })
    }

    async fn bind(
        &self,
        id: &str,
        client: &app::PrincipalId,
        purpose: rpc::McpProjectPathPurpose,
        fingerprint: &str,
    ) -> Result<Destination, rpc::RpcError> {
        self.select(id, client, purpose, Some(fingerprint)).await
    }

    async fn select(
        &self,
        id: &str,
        client: &app::PrincipalId,
        purpose: rpc::McpProjectPathPurpose,
        fingerprint: Option<&str>,
    ) -> Result<Destination, rpc::RpcError> {
        let id = app::OperationId::parse(id).map_err(|_| rpc::RpcError::invalid_request())?;
        let destination = {
            let mut approvals = self.0.lock().await;
            let approval = approvals
                .get_mut(&id)
                .filter(|entry| {
                    &entry.client == client
                        && entry.purpose == purpose
                        && Instant::now() < entry.expires
                        && entry
                            .request
                            .as_deref()
                            .is_none_or(|bound| Some(bound) == fingerprint)
                })
                .ok_or_else(rpc::RpcError::permission_denied)?;
            if let Some(fingerprint) = fingerprint {
                approval
                    .request
                    .get_or_insert_with(|| fingerprint.to_owned());
            }
            approval.destination.clone()
        };
        let current = resolve(destination.parent.clone(), destination.leaf.clone()).await?;
        if current.identity != destination.identity {
            return Err(rpc::RpcError::permission_denied());
        }
        Ok(current)
    }

    async fn bind_plan(
        &self,
        id: &str,
        client: &app::PrincipalId,
        purpose: rpc::McpProjectPathPurpose,
        plan: app::PlanId,
    ) -> Result<(), rpc::RpcError> {
        let id = app::OperationId::parse(id).map_err(|_| rpc::RpcError::invalid_request())?;
        let mut approvals = self.0.lock().await;
        let approval = approvals
            .get_mut(&id)
            .filter(|entry| {
                &entry.client == client
                    && entry.purpose == purpose
                    && Instant::now() < entry.expires
            })
            .ok_or_else(rpc::RpcError::permission_denied)?;
        if approval.plan.is_some_and(|existing| existing != plan) {
            return Err(rpc::RpcError::permission_denied());
        }
        approval.plan = Some(plan);
        Ok(())
    }
}

fn ordinary_leaf(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.ends_with(['.', ' '])
        && !name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|'])
        && !name.chars().any(char::is_control)
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
        super::require_capability(&request.id, state, rpc::CAPABILITY_MCP_PROJECT_WORKFLOWS_V1)
    {
        return error;
    }
    let id = request.id;
    if request.method == rpc::METHOD_MCP_PROJECT_PATH_APPROVE {
        if access.principal() != &app::PrincipalId::local_owner() {
            return error_action(Some(id), rpc::RpcError::permission_denied(), false);
        }
        let Ok(params) = serde_json::from_value::<rpc::McpProjectPathApproveParams>(request.params)
        else {
            return invalid(id);
        };
        let Ok(client_id) = app::McpClientId::parse(&params.client_id) else {
            return invalid(id);
        };
        if !params.confirm {
            return invalid(id);
        }
        let clients = match apps.store.mcp_clients().await {
            Ok(clients) => clients,
            Err(error) => return super::m8_mcp_rpc::error(id, error, false),
        };
        let Some(client) = clients
            .into_iter()
            .find(|client| client.client_id == client_id && client.revoked_at_ms.is_none())
        else {
            return error_action(Some(id), rpc::RpcError::permission_denied(), false);
        };
        return match apps
            .workflow_paths
            .approve(
                client.principal_id,
                params.purpose,
                PathBuf::from(params.path),
            )
            .await
        {
            Ok(result) => success_action(id, result, None),
            Err(error) => error_action(Some(id), error, false),
        };
    }
    let Ok(authentication) = super::m8_tools_rpc::auth(state, access) else {
        return error_action(Some(id), rpc::RpcError::permission_denied(), false);
    };
    let permissions = super::m8_oauth_rpc::required_scopes(&request.method)
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    if app::require_tool_permissions(access, &permissions).is_err() {
        return error_action(Some(id), rpc::RpcError::permission_denied(), false);
    }
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
                rpc::METHOD_MCP_BACKUPS_PLAN_RESTORE => {
                    let Ok(params) = serde_json::from_value(request.params) else {
                        return invalid(id);
                    };
                    plan_restore(id, apps, access, params).await
                }
                rpc::METHOD_MCP_BACKUPS_APPLY_RESTORE => {
                    let Ok(params) = serde_json::from_value(request.params) else {
                        return invalid(id);
                    };
                    apply_restore(id, apps, access, params).await
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
            let leaf = relative
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_owned();
            match resolve(root.clone(), String::new()).await {
                Ok(base) => resolve(base.parent.join(relative), leaf)
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
        rpc::McpProjectSource::Approved { approval_id } => {
            apps.workflow_paths
                .bind(
                    &approval_id,
                    access.principal(),
                    rpc::McpProjectPathPurpose::Register,
                    &fingerprint,
                )
                .await
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
    let destination = match params.target {
        rpc::McpProjectTarget::Default { name } => {
            if !ordinary_leaf(&name) {
                return invalid(id);
            }
            match apps.official_gui.effective_directories().await {
                Ok(directories) => {
                    resolve(PathBuf::from(directories.project_directory), name).await
                }
                Err(_) => Err(rpc::RpcError::store_unavailable()),
            }
        }
        rpc::McpProjectTarget::Approved { approval_id } => {
            apps.workflow_paths
                .bind(
                    &approval_id,
                    access.principal(),
                    rpc::McpProjectPathPurpose::Create,
                    &fingerprint,
                )
                .await
        }
    };
    let destination = match destination {
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

fn request_hash(value: &impl serde::Serialize) -> Result<String, rpc::RpcError> {
    serde_json::to_string(value)
        .map(|json| app::oauth_hex(&app::mcp_credential_hash(&json)))
        .map_err(|_| rpc::RpcError::invalid_request())
}

fn proof_for(approval_id: Option<&str>) -> app::WorkflowPlanPath {
    approval_id.map_or(app::WorkflowPlanPath::Default, |id| {
        app::WorkflowPlanPath::Approved {
            digest: app::oauth_hex(&app::mcp_credential_hash(id)),
        }
    })
}

async fn default_target(apps: &Applications, leaf: String) -> Result<Destination, rpc::RpcError> {
    if !ordinary_leaf(&leaf) {
        return Err(rpc::RpcError::invalid_request());
    }
    let directories = apps
        .official_gui
        .effective_directories()
        .await
        .map_err(|_| rpc::RpcError::store_unavailable())?;
    resolve(PathBuf::from(directories.project_directory), leaf).await
}

async fn plan_target(
    apps: &Applications,
    access: &app::AccessContext,
    purpose: rpc::McpProjectPathPurpose,
    target: rpc::McpProjectTarget,
) -> Result<(Destination, Option<String>), rpc::RpcError> {
    match target {
        rpc::McpProjectTarget::Default { name } => Ok((default_target(apps, name).await?, None)),
        rpc::McpProjectTarget::Approved { approval_id } => {
            let destination = apps
                .workflow_paths
                .select(&approval_id, access.principal(), purpose, None)
                .await?;
            Ok((destination, Some(approval_id)))
        }
    }
}

async fn remember_plan(
    apps: &Applications,
    access: &app::AccessContext,
    purpose: rpc::McpProjectPathPurpose,
    approval: Option<&str>,
    plan: app::PlanId,
) -> Result<(), rpc::RpcError> {
    if let Some(id) = approval {
        apps.workflow_paths
            .bind_plan(id, access.principal(), purpose, plan)
            .await?;
    }
    Ok(())
}

struct PlannedPath<'a> {
    parent: &'a str,
    identity: &'a [u8],
    leaf: &'a str,
}

async fn check_apply_target(
    apps: &Applications,
    access: &app::AccessContext,
    purpose: rpc::McpProjectPathPurpose,
    plan: app::PlanId,
    approval: Option<&str>,
    fingerprint: &str,
    expected: PlannedPath<'_>,
) -> Result<(), rpc::RpcError> {
    let current = if let Some(id) = approval {
        remember_plan(apps, access, purpose, Some(id), plan).await?;
        apps.workflow_paths
            .bind(id, access.principal(), purpose, fingerprint)
            .await?
    } else {
        default_target(apps, expected.leaf.to_owned()).await?
    };
    if current.parent != std::path::Path::new(expected.parent)
        || current.identity != expected.identity
        || current.leaf != expected.leaf
    {
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
    let (Ok(source), Some(revision), Ok(key)) = (
        app::ProjectId::parse(&params.source_project_id),
        app::Revision::new(params.expected_revision),
        app::IdempotencyKey::parse(params.idempotency_key),
    ) else {
        return invalid(id);
    };
    let purpose = rpc::McpProjectPathPurpose::Copy;
    let (destination, approval) = match plan_target(apps, access, purpose, params.target).await {
        Ok(value) => value,
        Err(error) => return error_action(Some(id), error, false),
    };
    let result = app::with_workflow_plan_path(
        proof_for(approval.as_deref()),
        Some(destination.identity),
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
    if let Err(error) = remember_plan(
        apps,
        access,
        purpose,
        approval.as_deref(),
        result.plan.draft.plan_id,
    )
    .await
    {
        return error_action(Some(id), error, false);
    }
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
    let (true, Ok(plan_id), Some(revision), Ok(key), Ok(fingerprint)) = (
        params.confirm,
        app::PlanId::parse(&params.plan_id),
        app::Revision::new(params.expected_revision),
        app::IdempotencyKey::parse(params.idempotency_key.clone()),
        request_hash(&params),
    ) else {
        return invalid(id);
    };
    let approval = params.path_approval_id.as_deref();
    app::with_workflow_plan_path(proof_for(approval), None, async {
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
        let path = PlannedPath {
            parent: &plan.draft.target_parent_path,
            identity: &plan.draft.target_parent_identity,
            leaf: &plan.draft.target_leaf,
        };
        if let Err(error) = check_apply_target(
            apps,
            access,
            rpc::McpProjectPathPurpose::Copy,
            plan_id,
            approval,
            &fingerprint,
            path,
        )
        .await
        {
            return error_action(Some(id), error, false);
        }
        copy_action(
            id,
            apps.project_copy
                .apply_copy(access, plan_id, revision, key)
                .await,
        )
    })
    .await
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

async fn plan_restore(
    id: String,
    apps: &Applications,
    access: &app::AccessContext,
    params: rpc::McpBackupPlanRestoreParams,
) -> DispatchAction {
    let Ok(backup_id) = app::BackupId::parse(&params.backup_id) else {
        return invalid(id);
    };
    let purpose = rpc::McpProjectPathPurpose::Restore;
    let (destination, approval) = match plan_target(apps, access, purpose, params.target).await {
        Ok(value) => value,
        Err(error) => return error_action(Some(id), error, false),
    };
    let result = app::with_workflow_plan_path(
        proof_for(approval.as_deref()),
        Some(destination.identity),
        apps.backups
            .plan_restore(access, backup_id, destination.parent, destination.leaf),
    )
    .await;
    let result = match result {
        Ok(value) => value,
        Err(error) => return super::m5_backup_rpc::backup_error(id, error),
    };
    if let Err(error) =
        remember_plan(apps, access, purpose, approval.as_deref(), result.plan_id).await
    {
        return error_action(Some(id), error, false);
    }
    success_action(
        id,
        rpc::McpBackupRestorePlan {
            plan_id: result.plan_id.to_string(),
            project_id: result.project_id.to_string(),
            backup_id: result.backup_id.to_string(),
            target_name: result.target.leaf,
            archive_sha256: app::oauth_hex(&result.archive_sha256),
            packages_require_resolve: result.packages_require_resolve,
            excluded_packages: result
                .excluded_packages
                .into_iter()
                .map(|package| rpc::BackupRestoreExcludedPackage {
                    package_id: package.package_id,
                    version: package.version,
                })
                .collect(),
            plan_fingerprint: app::oauth_hex(&result.plan_fingerprint),
        },
        None,
    )
}

async fn apply_restore(
    id: String,
    apps: &Applications,
    access: &app::AccessContext,
    params: rpc::McpBackupApplyRestoreParams,
) -> DispatchAction {
    let (true, Ok(plan_id), Ok(key), Ok(fingerprint)) = (
        params.confirm,
        app::PlanId::parse(&params.plan_id),
        app::IdempotencyKey::parse(params.idempotency_key.clone()),
        request_hash(&params),
    ) else {
        return invalid(id);
    };
    let approval = params.path_approval_id.as_deref();
    app::with_workflow_plan_path(proof_for(approval), None, async {
        let caller = access.principal();
        match apps
            .store
            .replay_mcp_backup_restore(caller.clone(), plan_id, key.clone())
            .await
        {
            Ok(Some(value)) => return restore_action(id, Ok(value)),
            Ok(None) => {}
            Err(error) => return super::m5_backup_rpc::backup_error(id, error),
        }
        let plan = match apps
            .store
            .mcp_backup_restore_plan(caller.clone(), plan_id)
            .await
        {
            Ok(value) => value,
            Err(error) => return super::m5_backup_rpc::backup_error(id, error),
        };
        let path = PlannedPath {
            parent: &plan.draft.target.parent,
            identity: &plan.draft.target_parent_identity,
            leaf: &plan.draft.target.leaf,
        };
        if let Err(error) = check_apply_target(
            apps,
            access,
            rpc::McpProjectPathPurpose::Restore,
            plan_id,
            approval,
            &fingerprint,
            path,
        )
        .await
        {
            return error_action(Some(id), error, false);
        }
        restore_action(id, apps.backups.apply_restore(access, plan_id, key).await)
    })
    .await
}

fn restore_action(
    id: String,
    result: Result<app::BackupRestoreApplyOutcome, app::M5BackupError>,
) -> DispatchAction {
    match result {
        Ok(value) => success_action(
            id,
            rpc::BackupApplyRestoreResult {
                operation_id: value.operation_id.to_string(),
                project_id: value.project_id.to_string(),
                replayed: value.replayed,
            },
            None,
        ),
        Err(error) => super::m5_backup_rpc::backup_error(id, error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn path_approval_binds_client_purpose_intent_and_directory_identity() {
        let root = std::env::temp_dir()
            .canonicalize()
            .expect("canonical temporary directory")
            .join(format!("alcomd-workflow-path-{}", app::OperationId::new()));
        let parent = root.join("parent");
        std::fs::create_dir(&root).expect("unique test root");
        std::fs::create_dir(&parent).expect("test parent");
        let service = PathApprovals::default();
        let client =
            app::PrincipalId::parse(format!("mcp:{}", app::McpClientId::new())).expect("client");
        let other = app::PrincipalId::parse(format!("mcp:{}", app::McpClientId::new()))
            .expect("other client");
        let purpose = rpc::McpProjectPathPurpose::Create;
        let approval = service
            .approve(client.clone(), purpose, parent.join("new"))
            .await
            .expect("approve");
        let id = approval.approval_id;
        assert!(service.bind(&id, &other, purpose, "exact").await.is_err());
        assert!(
            service
                .bind(&id, &client, rpc::McpProjectPathPurpose::Copy, "exact")
                .await
                .is_err()
        );
        let first = service
            .bind(&id, &client, purpose, "exact")
            .await
            .expect("initial bind");
        let replay = service
            .bind(&id, &client, purpose, "exact")
            .await
            .expect("same intent");
        assert_eq!(first.identity, replay.identity);
        assert!(
            service
                .bind(&id, &client, purpose, "changed")
                .await
                .is_err()
        );
        let moved = root.join("moved");
        std::fs::rename(&parent, &moved).expect("move fixture");
        std::fs::create_dir(&parent).expect("replace path with a distinct object");
        assert!(service.bind(&id, &client, purpose, "exact").await.is_err());
        std::fs::remove_dir(parent).expect("remove empty replacement");
        std::fs::remove_dir(moved).expect("remove empty original");
        std::fs::remove_dir(root).expect("remove fixture");
    }
}
