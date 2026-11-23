//! Shared native project paths, including the configured project directory.
use alcomd_application as app;
use alcomd_protocol as rpc;
use std::path::{Component, Path, PathBuf};

use super::{Applications, ConnectionState, DispatchAction, error_action, invalid, success_action};

fn request_fingerprint(value: &impl serde::Serialize) -> Result<String, serde_json::Error> {
    use sha2::{Digest, Sha256};
    serde_json::to_string(&app::oauth_hex(&Sha256::digest(serde_json::to_vec(value)?)))
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
        || text.len() > 32_768
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

async fn resolve(
    directory: PathBuf,
    leaf: String,
) -> Result<app::ProjectDestination, rpc::RpcError> {
    resolve_directory(directory, leaf)
        .await?
        .map_err(|_| rpc::RpcError::permission_denied())
}

async fn resolve_directory(
    directory: PathBuf,
    leaf: String,
) -> Result<std::io::Result<app::ProjectDestination>, rpc::RpcError> {
    tokio::task::spawn_blocking(move || {
        let (parent, identity) = alcomd_platform::resolve_plain_project_directory(&directory)?;
        Ok(app::ProjectDestination {
            parent,
            identity,
            leaf,
        })
    })
    .await
    .map_err(|_| rpc::RpcError::store_unavailable())
}

async fn inspect_registration_directory(
    directory: PathBuf,
) -> Result<app::ProjectDestination, rpc::RpcError> {
    let path = directory.to_string_lossy().into_owned();
    match resolve_directory(directory, String::new()).await? {
        Ok(destination) => Ok(destination),
        Err(cause) => {
            let details = app::operation_log_error(&cause, Some(&path));
            app::log_external_fact(
                "projects.register",
                "inspect_project",
                app::LogOutcome::Failed,
                Some(app::log_path("project", path)),
                Some(details),
            )
            .await;
            Err(rpc::RpcError::permission_denied())
        }
    }
}

fn destination_input(destination: &app::ProjectDestination) -> String {
    destination.parent.to_string_lossy().into_owned()
}

pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    apps: &Applications,
    access: &app::AccessContext,
) -> DispatchAction {
    match request.method.as_str() {
        rpc::METHOD_PROJECTS_REGISTER => {
            dispatch_register(request, state, &apps.m3, access, default_directory(apps)).await
        }
        rpc::METHOD_PROJECTS_CREATE => {
            dispatch_create(
                request,
                state,
                &apps.project_create,
                access,
                default_directory(apps),
            )
            .await
        }
        rpc::METHOD_PROJECTS_PLAN_COPY => {
            let id = request.id;
            if let Some(error) =
                super::require_capability(&id, state, rpc::CAPABILITY_PROJECTS_COPY_V1)
            {
                return error;
            }
            let Ok(params) = serde_json::from_value(request.params) else {
                return invalid(id);
            };
            plan_copy(id, apps, access, params).await
        }
        _ => error_action(Some(request.id), rpc::RpcError::method_not_found(), false),
    }
}

pub(super) async fn dispatch_register(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    registry: &super::M3ReadApplication,
    access: &app::AccessContext,
    default_parent: impl Future<Output = Result<PathBuf, rpc::RpcError>>,
) -> DispatchAction {
    let id = request.id;
    if let Some(error) = super::require_capability(&id, state, rpc::CAPABILITY_PROJECTS_REGISTRY_V1)
    {
        return error;
    }
    let Ok(params) = serde_json::from_value::<rpc::ProjectRegisterParams>(request.params) else {
        return invalid(id);
    };
    let Ok(key) = app::IdempotencyKey::parse(params.idempotency_key.clone()) else {
        return invalid(id);
    };
    let Ok(fingerprint) = request_fingerprint(&params) else {
        return invalid(id);
    };
    match registry
        .replay_project_registration(access, fingerprint.clone(), key.clone())
        .await
    {
        Ok(Some(outcome)) => return registration_action(id, Ok(outcome)),
        Ok(None) => {}
        Err(error) => return super::m3_rpc::m3_error(id, error),
    }
    let resolved = match params.source {
        rpc::ProjectSource::Default { relative_path } => {
            if relative_path.len() > 4096 || !relative_path.split(['/', '\\']).all(ordinary_leaf) {
                return invalid(id);
            }
            let root = match default_parent.await {
                Ok(root) => root,
                Err(error) => return error_action(Some(id), error, false),
            };
            let relative: PathBuf = relative_path.split(['/', '\\']).collect();
            match inspect_registration_directory(root).await {
                Ok(base) => inspect_registration_directory(base.parent.join(relative))
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
        rpc::ProjectSource::Absolute { path } => {
            let path = match absolute_path(&path) {
                Ok(path) => path,
                Err(error) => return error_action(Some(id), error, false),
            };
            inspect_registration_directory(path).await
        }
    };
    let destination = match resolved {
        Ok(destination) => destination,
        Err(error) => return error_action(Some(id), error, false),
    };
    let path = destination_input(&destination);
    registration_action(
        id,
        registry
            .register_project(access, path, destination.identity, fingerprint, key)
            .await,
    )
}

fn registration_action(
    id: String,
    result: Result<app::SyncWrite<app::ProjectRecord>, app::M3Error>,
) -> DispatchAction {
    match result {
        Ok(value) => {
            let mut project = super::m3_rpc::project_record(value.value);
            project.directory_state = Some(rpc::ProjectDirectoryState::Available);
            success_action(
                id,
                rpc::ProjectWriteResult {
                    project,
                    replayed: value.replayed,
                },
                None,
            )
        }
        Err(error) => super::m3_rpc::m3_error(id, error),
    }
}

pub(super) async fn dispatch_create(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    application: &super::ProjectCreateApplication,
    access: &app::AccessContext,
    default_parent: impl Future<Output = Result<PathBuf, rpc::RpcError>>,
) -> DispatchAction {
    let id = request.id;
    if let Some(error) = super::require_capability(&id, state, rpc::CAPABILITY_PROJECTS_CREATE_V1) {
        return error;
    }
    let Ok(params) = serde_json::from_value::<rpc::ProjectsCreateParams>(request.params) else {
        return invalid(id);
    };
    if params.template.is_some()
        && let Some(error) =
            super::require_capability(&id, state, rpc::CAPABILITY_PROJECT_TEMPLATES_V1)
    {
        return error;
    }
    let Ok(key) = app::IdempotencyKey::parse(params.idempotency_key.clone()) else {
        return invalid(id);
    };
    let Ok(fingerprint) = request_fingerprint(&params) else {
        return invalid(id);
    };
    match application
        .replay_create(access, fingerprint.clone(), key.clone())
        .await
    {
        Ok(Some(outcome)) => return create_action(id, Ok(outcome)),
        Ok(None) => {}
        Err(error) => return super::project_create_rpc::project_create_error(id, error),
    }
    let destination = match project_target(default_parent, params.target).await {
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
        application
            .create_bound_project(access, input, key, fingerprint, destination.identity)
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

async fn default_directory(apps: &Applications) -> Result<PathBuf, rpc::RpcError> {
    apps.official_gui
        .effective_directories()
        .await
        .map(|directories| PathBuf::from(directories.project_directory))
        .map_err(|_| rpc::RpcError::store_unavailable())
}

async fn project_target(
    default_parent: impl Future<Output = Result<PathBuf, rpc::RpcError>>,
    target: rpc::ProjectTarget,
) -> Result<app::ProjectDestination, rpc::RpcError> {
    let (parent, leaf) = match target {
        rpc::ProjectTarget::Default { name } => (default_parent.await?, name),
        rpc::ProjectTarget::Absolute { parent_path, name } => (absolute_path(&parent_path)?, name),
    };
    if !ordinary_leaf(&leaf) {
        return Err(rpc::RpcError::invalid_request());
    }
    resolve(parent, leaf).await
}

async fn plan_copy(
    id: String,
    apps: &Applications,
    access: &app::AccessContext,
    params: rpc::ProjectsPlanCopyParams,
) -> DispatchAction {
    if access.require(app::Permission::ProjectsRead).is_err()
        || access.require(app::Permission::ProjectsCreate).is_err()
    {
        return error_action(Some(id), rpc::RpcError::permission_denied(), false);
    }
    let Ok(fingerprint) = request_fingerprint(&params) else {
        return invalid(id);
    };
    let (Ok(source), Some(revision), Ok(key)) = (
        app::ProjectId::parse(&params.source_project_id),
        app::Revision::new(params.expected_revision),
        app::IdempotencyKey::parse(params.idempotency_key),
    ) else {
        return invalid(id);
    };
    match apps
        .project_copy
        .replay_plan(access, fingerprint.clone(), key.clone())
        .await
    {
        Ok(Some(outcome)) => return copy_action(id, Ok(outcome)),
        Ok(None) => {}
        Err(error) => return super::m7_copy_rpc::copy_error(id, error),
    }
    let destination = match project_target(default_directory(apps), params.target).await {
        Ok(value) => value,
        Err(error) => return error_action(Some(id), error, false),
    };
    let result = apps
        .project_copy
        .plan_copy(access, source, revision, destination, key, fingerprint)
        .await;
    copy_action(id, result)
}

fn copy_action(
    id: String,
    result: Result<app::ProjectCopyPlanOutcome, app::M7CopyError>,
) -> DispatchAction {
    let result = match result {
        Ok(value) => value,
        Err(error) => return super::m7_copy_rpc::copy_error(id, error),
    };
    let plan = super::m7_copy_rpc::plan(result.plan);
    success_action(
        id,
        rpc::ProjectsPlanCopyResult {
            plan,
            replayed: result.replayed,
        },
        None,
    )
}
