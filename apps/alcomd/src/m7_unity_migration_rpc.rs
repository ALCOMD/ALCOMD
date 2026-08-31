use alcomd_application as app;
use alcomd_protocol as rpc;

use super::{
    AccessContext, ConnectionState, DispatchAction, IdempotencyKey, OperationId,
    UnityMigrationApplication, error_action, invalid, require_capability, success_action,
};

pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    application: &UnityMigrationApplication,
    access: &AccessContext,
) -> DispatchAction {
    if let Some(action) = require_capability(
        &request.id,
        state,
        rpc::CAPABILITY_PROJECTS_UNITY_MIGRATION_V1,
    ) {
        return action;
    }
    match request.method.as_str() {
        rpc::METHOD_PROJECTS_PLAN_UNITY_MIGRATION => {
            let params: rpc::ProjectsPlanUnityMigrationParams =
                match serde_json::from_value(request.params) {
                    Ok(value) => value,
                    Err(_) => return invalid(request.id),
                };
            let parsed = app::ProjectId::parse(&params.project_id)
                .ok()
                .zip(app::UnityInstallationId::parse(&params.target_installation_id).ok())
                .zip(app::Revision::new(params.expected_project_revision))
                .zip(IdempotencyKey::parse(params.idempotency_key).ok());
            let Some((((project_id, target_installation_id), revision), key)) = parsed else {
                return invalid(request.id);
            };
            match application
                .plan(access, project_id, target_installation_id, revision, key)
                .await
            {
                Ok(app::UnityMigrationPlanOutcome::NoChange { current_version }) => success_action(
                    request.id,
                    rpc::ProjectsPlanUnityMigrationResult::NoChange { current_version },
                    None,
                ),
                Ok(app::UnityMigrationPlanOutcome::Planned { plan, replayed }) => success_action(
                    request.id,
                    rpc::ProjectsPlanUnityMigrationResult::Planned {
                        plan: migration_plan(*plan),
                        replayed,
                    },
                    None,
                ),
                Err(source) => migration_error(request.id, source),
            }
        }
        rpc::METHOD_PROJECTS_APPLY_UNITY_MIGRATION => {
            let params: rpc::ProjectsApplyUnityMigrationParams =
                match serde_json::from_value(request.params) {
                    Ok(value) => value,
                    Err(_) => return invalid(request.id),
                };
            let parsed = app::PlanId::parse(&params.plan_id)
                .ok()
                .zip(IdempotencyKey::parse(params.idempotency_key).ok());
            let Some((plan_id, key)) = parsed else {
                return invalid(request.id);
            };
            match application.apply(access, plan_id, key).await {
                Ok(value) => success_action(
                    request.id,
                    rpc::ProjectsApplyUnityMigrationResult {
                        operation_id: value.operation_id.to_string(),
                        replayed: value.replayed,
                    },
                    None,
                ),
                Err(source) => migration_error(request.id, source),
            }
        }
        _ => error_action(Some(request.id), rpc::RpcError::method_not_found(), false),
    }
}

fn migration_plan(value: app::UnityMigrationPlanRecord) -> rpc::ProjectUnityMigrationPlan {
    rpc::ProjectUnityMigrationPlan {
        plan_id: value.draft.plan_id.to_string(),
        project_id: value.draft.project.project_id.to_string(),
        source_unity_version: value.draft.source_unity_version,
        target_unity_version: value.draft.target_unity_version,
        target_installation_id: value.draft.target_installation.installation_id.to_string(),
        classification: rpc::ProjectUnityMigrationClassification {
            kind: match value.draft.classification {
                app::UnityMigrationClassificationKind::PatchOrMinorUpgrade => {
                    rpc::ProjectUnityMigrationClassificationKind::PatchOrMinorUpgrade
                }
                app::UnityMigrationClassificationKind::MajorUpgrade => {
                    rpc::ProjectUnityMigrationClassificationKind::MajorUpgrade
                }
                app::UnityMigrationClassificationKind::PatchOrMinorDowngrade => {
                    rpc::ProjectUnityMigrationClassificationKind::PatchOrMinorDowngrade
                }
                app::UnityMigrationClassificationKind::MajorDowngrade => {
                    rpc::ProjectUnityMigrationClassificationKind::MajorDowngrade
                }
                app::UnityMigrationClassificationKind::ChinaVariantChange => {
                    rpc::ProjectUnityMigrationClassificationKind::ChinaVariantChange
                }
            },
            supported_for_apply: value.draft.classification.supported_for_apply(),
        },
        expires_at_ms: value.draft.expires_at_ms,
    }
}

fn migration_error(id: String, source: app::M7UnityMigrationError) -> DispatchAction {
    let error = match source.code() {
        app::M7UnityMigrationErrorCode::InvalidInput => rpc::RpcError::invalid_request(),
        app::M7UnityMigrationErrorCode::PermissionDenied => rpc::RpcError::permission_denied(),
        app::M7UnityMigrationErrorCode::RevisionConflict => rpc::RpcError::revision_conflict(),
        app::M7UnityMigrationErrorCode::IdempotencyConflict => {
            rpc::RpcError::idempotency_conflict()
        }
        app::M7UnityMigrationErrorCode::StoreUnavailable => rpc::RpcError::store_unavailable(),
        app::M7UnityMigrationErrorCode::Internal => {
            rpc::RpcError::internal(OperationId::new().to_string())
        }
        code => rpc::RpcError::unity_migration(migration_error_code(code)),
    };
    error_action(Some(id), error, false)
}

const fn migration_error_code(value: app::M7UnityMigrationErrorCode) -> &'static str {
    match value {
        app::M7UnityMigrationErrorCode::ProjectNotRegistered => {
            rpc::error_code::PROJECT_NOT_REGISTERED
        }
        app::M7UnityMigrationErrorCode::InstallationNotFound => {
            rpc::error_code::UNITY_INSTALLATION_NOT_FOUND
        }
        app::M7UnityMigrationErrorCode::ProjectRunning => rpc::error_code::UNITY_PROJECT_RUNNING,
        app::M7UnityMigrationErrorCode::PlanNotFound => {
            rpc::error_code::PROJECT_UNITY_MIGRATION_PLAN_NOT_FOUND
        }
        app::M7UnityMigrationErrorCode::PlanStale => {
            rpc::error_code::PROJECT_UNITY_MIGRATION_PLAN_STALE
        }
        app::M7UnityMigrationErrorCode::Unsupported => {
            rpc::error_code::PROJECT_UNITY_MIGRATION_UNSUPPORTED
        }
        app::M7UnityMigrationErrorCode::SourceChanged => {
            rpc::error_code::PROJECT_UNITY_MIGRATION_SOURCE_CHANGED
        }
        app::M7UnityMigrationErrorCode::RecoveryRequired => {
            rpc::error_code::PROJECT_UNITY_MIGRATION_RECOVERY_REQUIRED
        }
        app::M7UnityMigrationErrorCode::InvalidInput
        | app::M7UnityMigrationErrorCode::PermissionDenied
        | app::M7UnityMigrationErrorCode::RevisionConflict
        | app::M7UnityMigrationErrorCode::IdempotencyConflict
        | app::M7UnityMigrationErrorCode::StoreUnavailable
        | app::M7UnityMigrationErrorCode::Internal => "internal_error",
    }
}
