use alcomd_application as app;
use alcomd_protocol as rpc;

use super::{
    AccessContext, ConnectionState, DispatchAction, IdempotencyKey, OperationId,
    ProjectDeleteApplication, Revision, error_action, invalid, require_capability, success_action,
};

pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    application: &ProjectDeleteApplication,
    access: &AccessContext,
) -> DispatchAction {
    if let Some(action) = require_capability(&request.id, state, rpc::CAPABILITY_PROJECTS_DELETE_V1)
    {
        return action;
    }
    match request.method.as_str() {
        rpc::METHOD_PROJECTS_PLAN_DELETE_DIRECTORY => {
            let params: rpc::ProjectsPlanDeleteDirectoryParams =
                match serde_json::from_value(request.params) {
                    Ok(value) => value,
                    Err(_) => return invalid(request.id),
                };
            let parsed = app::ProjectId::parse(&params.project_id)
                .ok()
                .zip(Revision::new(params.expected_revision))
                .zip(IdempotencyKey::parse(params.idempotency_key).ok());
            let Some(((project_id, revision), key)) = parsed else {
                return invalid(request.id);
            };
            match application
                .plan_delete(access, project_id, revision, key)
                .await
            {
                Ok(value) => success_action(
                    request.id,
                    rpc::ProjectsPlanDeleteDirectoryResult {
                        plan: plan(value.plan),
                        replayed: value.replayed,
                    },
                    None,
                ),
                Err(source) => delete_error(request.id, source),
            }
        }
        rpc::METHOD_PROJECTS_APPLY_DELETE_DIRECTORY => {
            let params: rpc::ProjectsApplyDeleteDirectoryParams =
                match serde_json::from_value(request.params) {
                    Ok(value) => value,
                    Err(_) => return invalid(request.id),
                };
            let parsed = app::PlanId::parse(&params.plan_id)
                .ok()
                .zip(Revision::new(params.expected_revision))
                .zip(IdempotencyKey::parse(params.idempotency_key).ok());
            let Some(((plan_id, revision), key)) = parsed else {
                return invalid(request.id);
            };
            match application
                .apply_delete(access, plan_id, revision, key)
                .await
            {
                Ok(value) => success_action(
                    request.id,
                    rpc::ProjectsApplyDeleteDirectoryResult {
                        operation_id: value.operation_id.to_string(),
                        project_id: value.project_id.to_string(),
                        replayed: value.replayed,
                    },
                    None,
                ),
                Err(source) => delete_error(request.id, source),
            }
        }
        _ => error_action(Some(request.id), rpc::RpcError::method_not_found(), false),
    }
}

fn plan(value: app::ProjectDeletePlanRecord) -> rpc::ProjectDeletePlan {
    let project = &value.draft.project;
    rpc::ProjectDeletePlan {
        plan_id: value.draft.plan_id.to_string(),
        owner_principal_id: value.owner.as_str().to_owned(),
        project_id: project.project_id.to_string(),
        project_revision: project.revision.get(),
        canonical_root_path: project.observation.root_path.clone(),
        root_filesystem_identity: hex(&value.draft.root_identity),
        canonical_parent_path: value.draft.canonical_parent_path.clone(),
        parent_filesystem_identity: hex(&value.draft.parent_identity),
        parent_identity_sha256: hex(&value.draft.parent_identity_sha256),
        normalized_leaf: value.draft.normalized_leaf.clone(),
        project_marker_sha256: hex(&value.draft.project_marker_sha256),
        expected_unity_version: project.observation.unity_version.clone(),
        expected_unity_revision: project.observation.unity_revision.clone(),
        writer_evidence: rpc::ProjectDeleteWriterEvidence {
            state: writer_state(value.draft.writer_evidence.state).to_owned(),
            observed_at_ms: value.draft.writer_evidence.checked_at_ms,
            safe_evidence: value
                .draft
                .writer_evidence
                .evidence
                .into_iter()
                .map(writer_evidence)
                .map(str::to_owned)
                .collect(),
        },
        profile: rpc::ProjectDeleteProfile {
            id: "alcomd-project-delete".to_owned(),
            version: 1,
            mode: "sibling-quarantine-permanent-v1".to_owned(),
            protected_root_profile_version: 1,
            progress: "phase-only".to_owned(),
        },
        plan_fingerprint: hex(&value.draft.plan_fingerprint),
        idempotency_key: value.draft.plan_idempotency_key.as_str().to_owned(),
        created_at_ms: value.draft.created_at_ms,
        expires_at_ms: value.draft.expires_at_ms,
    }
}

fn writer_state(value: app::UnityWriterStateKind) -> &'static str {
    match value {
        app::UnityWriterStateKind::RunningConfirmed => "running_confirmed",
        app::UnityWriterStateKind::RunningSuspected => "running_suspected",
        app::UnityWriterStateKind::NotObserved => "not_observed",
        app::UnityWriterStateKind::Unknown => "unknown",
    }
}

fn writer_evidence(value: app::UnityWriterEvidenceKind) -> &'static str {
    match value {
        app::UnityWriterEvidenceKind::ProcessProjectArgument => "process_project_argument",
        app::UnityWriterEvidenceKind::ProcessUnreadable => "process_unreadable",
        app::UnityWriterEvidenceKind::InspectionError => "inspection_error",
    }
}

fn delete_error(id: String, source: app::M7DeleteError) -> DispatchAction {
    let error = match source.code() {
        app::M7DeleteErrorCode::InvalidInput => rpc::RpcError::invalid_request(),
        app::M7DeleteErrorCode::PermissionDenied => rpc::RpcError::permission_denied(),
        app::M7DeleteErrorCode::RevisionConflict => rpc::RpcError::revision_conflict(),
        app::M7DeleteErrorCode::IdempotencyConflict => rpc::RpcError::idempotency_conflict(),
        app::M7DeleteErrorCode::StoreUnavailable => rpc::RpcError::store_unavailable(),
        app::M7DeleteErrorCode::Internal => rpc::RpcError::internal(OperationId::new().to_string()),
        _ => rpc::RpcError::project_delete(app::project_delete_error_name(source.code())),
    };
    error_action(Some(id), error, false)
}

fn hex(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}
