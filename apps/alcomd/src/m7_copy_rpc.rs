use alcomd_application as app;
use alcomd_protocol as rpc;

use super::{
    AccessContext, ConnectionState, DispatchAction, IdempotencyKey, OperationId,
    ProjectCopyApplication, Revision, error_action, invalid, require_capability, success_action,
};

pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    application: &ProjectCopyApplication,
    access: &AccessContext,
) -> DispatchAction {
    if let Some(action) = require_capability(&request.id, state, rpc::CAPABILITY_PROJECTS_COPY_V1) {
        return action;
    }
    match request.method.as_str() {
        rpc::METHOD_PROJECTS_PLAN_COPY => {
            let params: rpc::ProjectsPlanCopyParams = match serde_json::from_value(request.params) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            let parsed = app::ProjectId::parse(&params.source_project_id)
                .ok()
                .zip(Revision::new(params.expected_revision))
                .zip(IdempotencyKey::parse(params.idempotency_key).ok());
            let Some(((project_id, revision), key)) = parsed else {
                return invalid(request.id);
            };
            match application
                .plan_copy(
                    access,
                    project_id,
                    revision,
                    std::path::PathBuf::from(params.target_parent_path),
                    params.target_leaf,
                    key,
                )
                .await
            {
                Ok(value) => success_action(
                    request.id,
                    rpc::ProjectsPlanCopyResult {
                        plan: plan(value.plan),
                        replayed: value.replayed,
                    },
                    None,
                ),
                Err(source) => copy_error(request.id, source),
            }
        }
        rpc::METHOD_PROJECTS_APPLY_COPY => {
            let params: rpc::ProjectsApplyCopyParams = match serde_json::from_value(request.params)
            {
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
            match application.apply_copy(access, plan_id, revision, key).await {
                Ok(value) => success_action(
                    request.id,
                    rpc::ProjectsApplyCopyResult {
                        operation_id: value.operation_id.to_string(),
                        target_project_id: value.target_project_id.to_string(),
                        replayed: value.replayed,
                    },
                    None,
                ),
                Err(source) => copy_error(request.id, source),
            }
        }
        _ => error_action(Some(request.id), rpc::RpcError::method_not_found(), false),
    }
}

fn plan(value: app::ProjectCopyPlanRecord) -> rpc::ProjectCopyPlan {
    let project = &value.draft.source_project;
    rpc::ProjectCopyPlan {
        plan_id: value.draft.plan_id.to_string(),
        owner_principal_id: value.owner.as_str().to_owned(),
        source_project_id: project.project_id.to_string(),
        source_project_revision: project.revision.get(),
        source_canonical_root_path: project.observation.root_path.clone(),
        source_filesystem_identity: hex(&value.draft.source_root_identity),
        source_project_kind: project_type(project.observation.project_type).to_owned(),
        expected_unity_version: project.observation.unity_version.clone(),
        expected_unity_revision: project.observation.unity_revision.clone(),
        writer_evidence: rpc::ProjectCopyWriterEvidence {
            state: writer_state(value.draft.writer_evidence.state).to_owned(),
            observed_at_ms: value.draft.writer_evidence.checked_at_ms,
        },
        target_parent_canonical_path: value.draft.target_parent_path.clone(),
        target_parent_filesystem_identity: hex(&value.draft.target_parent_identity),
        normalized_target_leaf: value.draft.target_leaf.clone(),
        target_must_not_exist: true,
        target_project_id: value.draft.target_project_id.to_string(),
        profile: rpc::ProjectCopyProfile {
            id: "alcomd-project-copy".to_owned(),
            version: 1,
            includes: [
                "ordinary-project-contents",
                "Library",
                "Library*",
                "Packages",
                "locked-vpm-packages",
                "Packages/manifest.json",
                "Packages/vpm-manifest.json",
                "ordinary-hidden-entries",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            excludes: [
                "root/Logs",
                "root/Obj",
                "root/Temp",
                "**/.git-case-insensitive-any-entry-type",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            rejects: [
                "symlink",
                "junction",
                "reparse-point",
                "hard-linked-regular-file",
                "special-file",
                "non-utf8-name",
                "absolute-device-unc-escape",
                "traversal",
                "unicode-collision",
                "case-collision",
                "file-directory-collision",
                "target-inside-source",
                "source-inside-target",
                "overwrite",
                "merge",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
            quota: rpc::ProjectCopyQuota {
                max_entries: 500_000,
                max_single_file_bytes: 34_359_738_368,
                max_total_regular_file_bytes: 137_438_953_472,
                max_depth: 128,
                max_normalized_path_utf8_bytes: 1_024,
            },
        },
        safe_exclusion_summary: ["root/Logs", "root/Obj", "root/Temp", "**/.git"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        plan_fingerprint: hex(&value.draft.plan_fingerprint),
        idempotency_key: value.draft.plan_idempotency_key.as_str().to_owned(),
        created_at_ms: value.draft.created_at_ms,
        expires_at_ms: value.draft.expires_at_ms,
    }
}

fn project_type(value: app::ProjectType) -> &'static str {
    match value {
        app::ProjectType::Avatars => "avatars",
        app::ProjectType::Worlds => "worlds",
        app::ProjectType::LegacyAvatars => "legacy_avatars",
        app::ProjectType::LegacyWorlds => "legacy_worlds",
        app::ProjectType::Unknown => "unknown",
        app::ProjectType::VpmStarter => "vpm_starter",
        app::ProjectType::UpmAvatars => "upm_avatars",
        app::ProjectType::UpmWorlds => "upm_worlds",
        app::ProjectType::UpmStarter => "upm_starter",
        app::ProjectType::LegacySdk2 => "legacy_sdk2",
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

fn copy_error(id: String, source: app::M7CopyError) -> DispatchAction {
    let error = match source.code() {
        app::M7CopyErrorCode::InvalidInput => rpc::RpcError::invalid_request(),
        app::M7CopyErrorCode::PermissionDenied => rpc::RpcError::permission_denied(),
        app::M7CopyErrorCode::RevisionConflict => rpc::RpcError::revision_conflict(),
        app::M7CopyErrorCode::IdempotencyConflict => rpc::RpcError::idempotency_conflict(),
        app::M7CopyErrorCode::StoreUnavailable => rpc::RpcError::store_unavailable(),
        app::M7CopyErrorCode::Internal => rpc::RpcError::internal(OperationId::new().to_string()),
        _ => rpc::RpcError::project_copy(app::project_copy_error_name(source.code())),
    };
    error_action(Some(id), error, false)
}

fn hex(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}
