use alcomd_application as app;
use alcomd_protocol as rpc;
use serde_json::{Map, Value};

use super::{
    AccessContext, ConnectionState, DispatchAction, IdempotencyKey, OperationId, Revision,
    TemplateApplication, error_action, invalid, require_capability, success_action,
};

macro_rules! require {
    ($request:ident, $state:ident, $capability:expr) => {
        if let Some(action) = require_capability(&$request.id, $state, $capability) {
            return action;
        }
    };
}

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
    application: &TemplateApplication,
    access: &AccessContext,
) -> DispatchAction {
    match request.method.as_str() {
        rpc::METHOD_TEMPLATES_LIST => {
            require!(request, state, rpc::CAPABILITY_TEMPLATES_READ_V1);
            let params = parse!(request, rpc::TemplatesListParams);
            let cursor = match params.cursor.map(parse_cursor).transpose() {
                Ok(value) => value,
                Err(()) => return invalid(request.id),
            };
            match application
                .list(access, cursor, params.limit.unwrap_or(100))
                .await
            {
                Ok(page) => success_action(
                    request.id,
                    rpc::TemplatesListResult {
                        templates: page.templates.into_iter().map(template).collect(),
                        next_cursor: page.next_cursor.map(format_cursor),
                    },
                    None,
                ),
                Err(error) => template_error(request.id, error),
            }
        }
        rpc::METHOD_TEMPLATES_GET => {
            require!(request, state, rpc::CAPABILITY_TEMPLATES_READ_V1);
            let params = parse!(request, rpc::TemplateIdParams);
            let id = match app::TemplateId::parse(&params.template_id) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            match application.get(access, id).await {
                Ok(value) => success_action(
                    request.id,
                    rpc::TemplateRecordResult {
                        template: template(value),
                        replayed: false,
                    },
                    None,
                ),
                Err(error) => template_error(request.id, error),
            }
        }
        rpc::METHOD_TEMPLATES_INSPECT_BUNDLE => {
            require!(request, state, rpc::CAPABILITY_TEMPLATES_READ_V1);
            let params = parse!(request, rpc::TemplateInspectBundleParams);
            match application.inspect(access, params.bundle_path).await {
                Ok(value) => success_action(request.id, inspection(value), None),
                Err(error) => template_error(request.id, error),
            }
        }
        rpc::METHOD_TEMPLATES_PLAN_IMPORT => {
            require!(request, state, rpc::CAPABILITY_TEMPLATES_MANAGE_V1);
            let params = parse!(request, rpc::TemplatePlanImportParams);
            let expected = if params.expected_revision == 0 {
                None
            } else {
                Revision::new(params.expected_revision)
            };
            if params.expected_revision != 0 && expected.is_none() {
                return invalid(request.id);
            }
            match application
                .plan_import(
                    access,
                    params.bundle_path,
                    params.override_existing,
                    expected,
                )
                .await
            {
                Ok(value) => match plan(value) {
                    Ok(value) => success_action(request.id, value, None),
                    Err(()) => internal(request.id),
                },
                Err(error) => template_error(request.id, error),
            }
        }
        rpc::METHOD_TEMPLATES_APPLY_IMPORT => {
            require!(request, state, rpc::CAPABILITY_TEMPLATES_MANAGE_V1);
            apply(request, application, access, ApplyKind::Import).await
        }
        rpc::METHOD_TEMPLATES_PLAN_DERIVE => {
            require!(request, state, rpc::CAPABILITY_TEMPLATES_MANAGE_V1);
            let params = parse!(request, rpc::TemplatePlanDeriveParams);
            let parsed = app::ProjectId::parse(&params.project_id)
                .ok()
                .zip(Revision::new(params.expected_project_revision))
                .zip(app::TemplateId::parse(&params.template_id).ok());
            let Some(((project_id, project_revision), template_id)) = parsed else {
                return invalid(request.id);
            };
            match application
                .plan_derive(
                    access,
                    project_id,
                    project_revision,
                    template_id,
                    params.template_version,
                    params.display_name,
                    params.description,
                )
                .await
            {
                Ok(value) => match plan(value) {
                    Ok(value) => success_action(request.id, value, None),
                    Err(()) => internal(request.id),
                },
                Err(error) => template_error(request.id, error),
            }
        }
        rpc::METHOD_TEMPLATES_APPLY_DERIVE => {
            require!(request, state, rpc::CAPABILITY_TEMPLATES_MANAGE_V1);
            apply(request, application, access, ApplyKind::Derive).await
        }
        rpc::METHOD_TEMPLATES_EXPORT => {
            require!(request, state, rpc::CAPABILITY_TEMPLATES_READ_V1);
            let params = parse!(request, rpc::TemplateExportParams);
            let parsed = app::TemplateId::parse(&params.template_id)
                .ok()
                .zip(Revision::new(params.expected_revision));
            let Some((template_id, revision)) = parsed else {
                return invalid(request.id);
            };
            match application
                .export(access, template_id, revision, params.target_path)
                .await
            {
                Ok(()) => success_action(
                    request.id,
                    rpc::TemplateExportResult { exported: true },
                    None,
                ),
                Err(error) => template_error(request.id, error),
            }
        }
        rpc::METHOD_TEMPLATES_SET_FAVORITE => {
            require!(request, state, rpc::CAPABILITY_TEMPLATES_MANAGE_V1);
            let params = parse!(request, rpc::TemplateSetFavoriteParams);
            let parsed = app::TemplateId::parse(&params.template_id)
                .ok()
                .zip(Revision::new(params.expected_revision))
                .zip(IdempotencyKey::parse(params.idempotency_key).ok());
            let Some(((template_id, revision), key)) = parsed else {
                return invalid(request.id);
            };
            match application
                .set_favorite(access, template_id, params.favorite, revision, key)
                .await
            {
                Ok((value, replayed)) => success_action(
                    request.id,
                    rpc::TemplateRecordResult {
                        template: template(value),
                        replayed,
                    },
                    None,
                ),
                Err(error) => template_error(request.id, error),
            }
        }
        rpc::METHOD_TEMPLATES_REMOVE => {
            require!(request, state, rpc::CAPABILITY_TEMPLATES_MANAGE_V1);
            let params = parse!(request, rpc::TemplateRemoveParams);
            let parsed = app::TemplateId::parse(&params.template_id)
                .ok()
                .zip(Revision::new(params.expected_revision))
                .zip(IdempotencyKey::parse(params.idempotency_key).ok());
            let Some(((template_id, revision), key)) = parsed else {
                return invalid(request.id);
            };
            match application.remove(access, template_id, revision, key).await {
                Ok((removed, replayed)) => success_action(
                    request.id,
                    rpc::TemplateRemoveResult {
                        template_id: template_id.to_string(),
                        removed,
                        replayed,
                    },
                    None,
                ),
                Err(error) => template_error(request.id, error),
            }
        }
        rpc::METHOD_TEMPLATES_PLAN_CREATE_PROJECT => {
            require!(request, state, rpc::CAPABILITY_TEMPLATES_CREATE_PROJECT_V1);
            let params = parse!(request, rpc::TemplatePlanCreateProjectParams);
            let parsed = app::TemplateId::parse(&params.template_id)
                .ok()
                .zip(Revision::new(params.expected_template_revision));
            let Some((template_id, revision)) = parsed else {
                return invalid(request.id);
            };
            match application
                .plan_create_project(
                    access,
                    template_id,
                    revision,
                    params.target_parent,
                    params.target_leaf,
                )
                .await
            {
                Ok(value) => match plan(value) {
                    Ok(value) => success_action(request.id, value, None),
                    Err(()) => internal(request.id),
                },
                Err(error) => template_error(request.id, error),
            }
        }
        rpc::METHOD_TEMPLATES_APPLY_CREATE_PROJECT => {
            require!(request, state, rpc::CAPABILITY_TEMPLATES_CREATE_PROJECT_V1);
            apply(request, application, access, ApplyKind::CreateProject).await
        }
        _ => error_action(Some(request.id), rpc::RpcError::method_not_found(), false),
    }
}

#[derive(Clone, Copy)]
enum ApplyKind {
    Import,
    Derive,
    CreateProject,
}

async fn apply(
    request: rpc::RequestEnvelope,
    application: &TemplateApplication,
    access: &AccessContext,
    kind: ApplyKind,
) -> DispatchAction {
    let params = match serde_json::from_value::<rpc::TemplateApplyPlanParams>(request.params) {
        Ok(value) => value,
        Err(_) => return invalid(request.id),
    };
    let parsed = app::PlanId::parse(&params.plan_id)
        .ok()
        .zip(IdempotencyKey::parse(params.idempotency_key).ok());
    let Some((plan_id, key)) = parsed else {
        return invalid(request.id);
    };
    let outcome = match kind {
        ApplyKind::Import => application.apply_import(access, plan_id, key).await,
        ApplyKind::Derive => application.apply_derive(access, plan_id, key).await,
        ApplyKind::CreateProject => application.apply_create_project(access, plan_id, key).await,
    };
    match outcome {
        Ok(value) => success_action(
            request.id,
            rpc::TemplateApplyResult {
                operation_id: value.operation_id.to_string(),
                replayed: value.replayed,
            },
            None,
        ),
        Err(error) => template_error(request.id, error),
    }
}

fn template(value: app::TemplateRecord) -> rpc::TemplateRecord {
    rpc::TemplateRecord {
        template_id: value.template_id.to_string(),
        source_kind: match value.source_kind {
            app::TemplateSourceKind::Builtin => rpc::TemplateSourceKind::Builtin,
            app::TemplateSourceKind::User => rpc::TemplateSourceKind::User,
        },
        template_version: value.template_version,
        display_name: value.display_name,
        description: value.description,
        provenance: value.provenance,
        favorite: value.favorite,
        bundle_sha256: hex(&value.bundle_sha256),
        manifest_fingerprint: hex(&value.manifest_fingerprint),
        revision: value.revision.get(),
        created_at_ms: value.created_at_ms,
        updated_at_ms: value.updated_at_ms,
    }
}

fn inspection(value: app::TemplateBundleEvidence) -> rpc::TemplateBundleInspection {
    rpc::TemplateBundleInspection {
        format_version: 1,
        template_id: value.template_id.to_string(),
        template_version: value.template_version,
        display_name: value.display_name,
        description: value.description,
        provenance: value.provenance,
        bundle_sha256: hex(&value.bundle_sha256),
        manifest_fingerprint: hex(&value.manifest_fingerprint),
        payload_tree_sha256: hex(&value.payload_tree_sha256),
        entry_count: value.entry_count,
        total_bytes: value.total_bytes,
    }
}

fn plan(value: app::TemplatePlanRecord) -> Result<rpc::TemplatePlan, ()> {
    let authority = serde_json::from_str::<Value>(&value.plan_json)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .ok_or(())?;
    let kind = authority.get("kind").and_then(Value::as_str).ok_or(())?;
    let action = if kind == "import"
        && authority
            .get("oldBundleSha256")
            .is_some_and(|value| !value.is_null())
    {
        "override"
    } else if kind == "create-project" {
        "create_project"
    } else {
        kind
    };
    let allowed: &[&str] = match kind {
        "import" => &[
            "templateId",
            "expectedRevision",
            "sourceFilesystemIdentity",
            "oldBundleSha256",
            "newBundleSha256",
            "manifestFingerprint",
        ],
        "derive" => &[
            "templateId",
            "sourceProjectId",
            "sourceProjectRevision",
            "sourceProjectFingerprint",
            "includePolicyVersion",
        ],
        "create-project" => &[
            "templateId",
            "templateRevision",
            "bundleSha256",
            "manifestFingerprint",
            "payloadTreeSha256",
            "parentFilesystemIdentity",
            "targetLeaf",
            "targetMustBeAbsent",
            "packageChangeSetFingerprint",
            "resourceDigests",
            "projectSummaryFingerprint",
        ],
        _ => return Err(()),
    };
    let evidence = allowed
        .iter()
        .filter_map(|key| {
            authority
                .get(*key)
                .cloned()
                .map(|value| ((*key).to_owned(), value))
        })
        .collect::<Map<_, _>>();
    Ok(rpc::TemplatePlan {
        plan_id: value.plan_id.to_string(),
        action: action.to_owned(),
        state: match value.state {
            app::TemplatePlanState::Unapplied => "unapplied",
            app::TemplatePlanState::Applied => "applied",
        }
        .to_owned(),
        plan_fingerprint: hex(&value.plan_fingerprint),
        evidence,
    })
}

fn parse_cursor(value: String) -> Result<app::TemplateCursor, ()> {
    let (time, id) = value.split_once(':').ok_or(())?;
    Ok(app::TemplateCursor {
        updated_at_ms: time.parse().map_err(|_| ())?,
        template_id: app::TemplateId::parse(id).map_err(|_| ())?,
    })
}

fn format_cursor(value: app::TemplateCursor) -> String {
    format!("{}:{}", value.updated_at_ms, value.template_id)
}

fn template_error(id: String, error: app::M5TemplateError) -> DispatchAction {
    let code = app::template_error_name(error.code());
    let rpc_error = match error.code() {
        app::M5TemplateErrorCode::InvalidInput => rpc::RpcError::invalid_request(),
        app::M5TemplateErrorCode::PermissionDenied => rpc::RpcError::permission_denied(),
        app::M5TemplateErrorCode::TemplateRevisionConflict => rpc::RpcError::revision_conflict(),
        app::M5TemplateErrorCode::StoreUnavailable => rpc::RpcError::store_unavailable(),
        app::M5TemplateErrorCode::Internal => {
            rpc::RpcError::internal(OperationId::new().to_string())
        }
        _ => rpc::RpcError::template(code),
    };
    error_action(Some(id), rpc_error, false)
}

fn internal(id: String) -> DispatchAction {
    error_action(
        Some(id),
        rpc::RpcError::internal(OperationId::new().to_string()),
        false,
    )
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(char::from(HEX[(byte >> 4) as usize]));
        value.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    value
}
