use alcomd_application as app;
use alcomd_protocol as rpc;

use super::{
    AccessContext, ConnectionState, DispatchAction, IdempotencyKey, OperationId,
    ProjectCreateApplication, error_action, invalid, require_capability, success_action,
};

macro_rules! parse {
    ($request:ident, $type:ty) => {
        match serde_json::from_value::<$type>($request.params) {
            Ok(value) => value,
            Err(_) => return invalid($request.id),
        }
    };
}

pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    application: &ProjectCreateApplication,
    access: &AccessContext,
) -> DispatchAction {
    let capability = match request.method.as_str() {
        rpc::METHOD_PROJECTS_CREATE => rpc::CAPABILITY_PROJECTS_CREATE_V1,
        _ => rpc::CAPABILITY_PROJECT_TEMPLATES_V2,
    };
    if let Some(action) = require_capability(&request.id, state, capability) {
        return action;
    }
    match request.method.as_str() {
        rpc::METHOD_PROJECTS_CREATE => {
            let params = parse!(request, rpc::ProjectsCreateParams);
            if params.template.is_some()
                && let Some(action) =
                    require_capability(&request.id, state, rpc::CAPABILITY_PROJECT_TEMPLATES_V2)
            {
                return action;
            }
            let Ok(key) = IdempotencyKey::parse(params.idempotency_key) else {
                return invalid(request.id);
            };
            let input = app::ProjectCreateInput {
                name: params.name,
                target_parent: params.target_parent,
                unity_version: params.unity_version,
                template: params.template.map(|value| app::ProjectTemplateRef {
                    id: value.id,
                    revision: value.revision,
                }),
            };
            match application.create(access, input, key).await {
                Ok(value) => success_action(
                    request.id,
                    rpc::ProjectsCreateResult {
                        operation_id: value.operation_id.to_string(),
                        project_id: value.project_id.to_string(),
                        replayed: value.replayed,
                    },
                    None,
                ),
                Err(source) => project_create_error(request.id, source),
            }
        }
        rpc::METHOD_PROJECT_TEMPLATES_LIST => {
            let params = parse!(request, rpc::ProjectTemplatesListParams);
            if params.cursor.as_deref().is_some_and(str::is_empty) {
                return invalid(request.id);
            }
            let Ok(limit) = params.limit.map(u16::try_from).transpose() else {
                return invalid(request.id);
            };
            match application
                .list_templates(access, limit, params.cursor)
                .await
            {
                Ok(page) => success_action(
                    request.id,
                    rpc::ProjectTemplatesListResult {
                        templates: page.templates.into_iter().map(template_record).collect(),
                        next_cursor: page.next_cursor,
                    },
                    None,
                ),
                Err(source) => project_create_error(request.id, source),
            }
        }
        rpc::METHOD_PROJECT_TEMPLATES_SAVE => {
            let params = parse!(request, rpc::ProjectTemplatesSaveParams);
            let Ok(key) = IdempotencyKey::parse(params.idempotency_key) else {
                return invalid(request.id);
            };
            let Some(source) = template_source(params.source) else {
                return invalid(request.id);
            };
            let result = application
                .save_template(access, source, params.expected_revision, key)
                .await;
            template_job_action(request.id, result)
        }
        rpc::METHOD_PROJECT_TEMPLATES_EXPORT => {
            let params = parse!(request, rpc::ProjectTemplatesExportParams);
            let Ok(key) = IdempotencyKey::parse(params.idempotency_key) else {
                return invalid(request.id);
            };
            let reference = app::ProjectTemplateRef {
                id: params.id,
                revision: params.revision,
            };
            let result = application
                .export_template(access, reference, params.destination_file, key)
                .await;
            template_job_action(request.id, result)
        }
        rpc::METHOD_PROJECT_TEMPLATES_REMOVE => {
            let params = parse!(request, rpc::ProjectTemplatesRemoveParams);
            match application
                .remove_template(access, params.id.clone(), params.expected_revision)
                .await
            {
                Ok(revision) => success_action(
                    request.id,
                    rpc::ProjectTemplatesRemoveResult {
                        id: params.id,
                        revision,
                    },
                    None,
                ),
                Err(source) => project_create_error(request.id, source),
            }
        }
        _ => error_action(Some(request.id), rpc::RpcError::method_not_found(), false),
    }
}

fn template_source(value: rpc::ProjectTemplateSource) -> Option<app::ProjectTemplateSource> {
    Some(match value {
        rpc::ProjectTemplateSource::File { source_file } => {
            app::ProjectTemplateSource::File { source_file }
        }
        rpc::ProjectTemplateSource::Project {
            project_id,
            expected_project_revision,
            id,
            name,
            include_assets,
            lock_package_versions,
        } => app::ProjectTemplateSource::Project {
            project_id: app::ProjectId::parse(&project_id).ok()?,
            expected_project_revision,
            id,
            name,
            include_assets,
            lock_package_versions,
        },
        rpc::ProjectTemplateSource::Manual {
            id,
            name,
            unity_version,
            dependencies,
        } => app::ProjectTemplateSource::Manual {
            id,
            name,
            unity_version,
            dependencies: dependencies
                .into_iter()
                .map(|dependency| app::ProjectTemplateDependency {
                    package_id: dependency.package_id,
                    version_range: dependency.version_range,
                })
                .collect(),
        },
    })
}

fn template_job_action(
    id: String,
    result: Result<app::ProjectTemplateJobAccepted, app::ProjectCreateError>,
) -> DispatchAction {
    match result {
        Ok(job) => success_action(
            id,
            rpc::ProjectTemplatesSaveResult {
                operation_id: job.operation_id.to_string(),
                replayed: job.replayed,
            },
            None,
        ),
        Err(error) => project_create_error(id, error),
    }
}

fn template_record(value: app::ProjectTemplateRecord) -> rpc::ProjectTemplateRecord {
    let declaration = value.declaration;
    rpc::ProjectTemplateRecord {
        id: declaration.id,
        name: declaration.name,
        editor_version: declaration.editor_version,
        revision: value.revision,
        locked: value.locked,
    }
}

fn project_create_error(id: String, source: app::ProjectCreateError) -> DispatchAction {
    use app::ProjectCreateErrorCode;

    let mut error = match source.code() {
        ProjectCreateErrorCode::InvalidInput => rpc::RpcError::invalid_request(),
        ProjectCreateErrorCode::PermissionDenied => rpc::RpcError::permission_denied(),
        ProjectCreateErrorCode::IdempotencyConflict => rpc::RpcError::idempotency_conflict(),
        ProjectCreateErrorCode::TemplateStale => rpc::RpcError::revision_conflict(),
        ProjectCreateErrorCode::StoreUnavailable => rpc::RpcError::store_unavailable(),
        ProjectCreateErrorCode::Internal => rpc::RpcError::internal(OperationId::new().to_string()),
        code => rpc::RpcError::project_create(app::project_create_error_name(code)),
    };
    if let Some(diagnostic_id) = source.diagnostic_id() {
        error.diagnostic_id = Some(diagnostic_id.to_owned());
    }
    error_action(Some(id), error, false)
}

#[cfg(all(test, windows))]
#[path = "project_create_ipc_tests.rs"]
mod ipc_tests;
