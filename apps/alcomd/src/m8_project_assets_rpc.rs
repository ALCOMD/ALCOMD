use alcomd_application as app;
use alcomd_protocol as rpc;

use super::{Applications, ConnectionState, DispatchAction};

pub(super) async fn templates(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    apps: &Applications,
    access: &app::AccessContext,
) -> DispatchAction {
    if let Some(action) =
        super::require_capability(&request.id, state, rpc::CAPABILITY_TEMPLATE_EDIT_V1)
    {
        return action;
    }
    let id = request.id;
    if request.method == rpc::METHOD_TEMPLATE_GET {
        let Ok(params) = serde_json::from_value::<rpc::ProjectTemplateGetParams>(request.params)
        else {
            return super::invalid(id);
        };
        let Ok(limit) = validate_page(params.limit, params.cursor.as_deref()) else {
            return super::invalid(id);
        };
        let connection = match binding(state, access) {
            Ok(connection) => connection,
            Err(error) => return super::m7_candidates_rpc::failure(id, error),
        };
        let (record, mut attachments) =
            match apps.project_create.template_detail(access, params.id).await {
                Ok(detail) => detail,
                Err(error) => return super::project_create_rpc::project_create_error(id, error),
            };
        let snapshot = fingerprint(
            &apps.tools_cursor_key,
            &(
                "templateAttachments",
                connection,
                &record.declaration.id,
                record.revision,
                &record.declaration.archive_sha256,
            ),
        );
        let (start, end, next_cursor) = match snapshot.and_then(|snapshot| {
            page(
                &apps.tools_cursor_key,
                &snapshot,
                params.cursor.as_deref(),
                limit,
                attachments.len(),
            )
        }) {
            Ok(page) => page,
            Err(error) => return super::m7_candidates_rpc::failure(id, error),
        };
        let dependencies = record
            .declaration
            .dependencies
            .iter()
            .map(|dependency| rpc::ProjectTemplateDependency {
                package_id: dependency.package_id.clone(),
                version_range: dependency.version_range.clone(),
            })
            .collect();
        return super::success_action(
            id,
            rpc::ProjectTemplateGetResult {
                template: super::project_create_rpc::template_record(record),
                dependencies,
                attachments: attachments.drain(start..end).collect(),
                next_cursor,
            },
            None,
        );
    }

    let Ok(params) = serde_json::from_value::<rpc::ProjectTemplateEditParams>(request.params)
    else {
        return super::invalid(id);
    };
    let Ok(key) = app::IdempotencyKey::parse(params.idempotency_key) else {
        return super::invalid(id);
    };
    if !params.confirm {
        return super::invalid(id);
    }
    let reference = app::ProjectTemplateRef {
        id: params.id,
        revision: params.expected_revision,
    };
    let changes = app::ProjectTemplateChanges {
        name: params.changes.name,
        set_dependencies: params
            .changes
            .set_dependencies
            .into_iter()
            .map(|dependency| app::ProjectTemplateDependency {
                package_id: dependency.package_id,
                version_range: dependency.version_range,
            })
            .collect(),
        remove_dependencies: params.changes.remove_dependencies,
        add_attachments: params
            .changes
            .add_attachments
            .into_iter()
            .map(|attachment| app::ProjectTemplateAttachment {
                source_file: attachment.source_file,
                target_path: attachment.target_path,
            })
            .collect(),
        remove_attachments: params.changes.remove_attachments,
    };
    let edit = apps
        .project_create
        .edit_template(access, reference, changes, key);
    let result = if access.is_mcp_client() {
        let Ok(authentication) = super::m8_tools_rpc::auth(state, access) else {
            return super::error_action(Some(id), rpc::RpcError::permission_denied(), false);
        };
        app::with_workflow_authentication(authentication, edit).await
    } else {
        edit.await
    };
    super::project_create_rpc::template_job_action(id, result)
}

pub(super) async fn inventory(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    apps: &Applications,
    access: &app::AccessContext,
) -> DispatchAction {
    if let Some(action) =
        super::require_capability(&request.id, state, rpc::CAPABILITY_PROJECT_INVENTORY_V1)
    {
        return action;
    }
    let id = request.id;
    let Ok(params) = serde_json::from_value::<rpc::ProjectInventoryParams>(request.params) else {
        return super::invalid(id);
    };
    match inventory_page(&apps.m3, &apps.tools_cursor_key, state, access, params).await {
        Ok(page) => super::success_action(id, page, None),
        Err(error) => super::m7_candidates_rpc::failure(id, error),
    }
}

async fn inventory_page(
    registry: &super::M3ReadApplication,
    key: &[u8; 32],
    state: &ConnectionState,
    access: &app::AccessContext,
    params: rpc::ProjectInventoryParams,
) -> Result<rpc::ProjectInventoryResult, app::CandidateQueryError> {
    use app::CandidateQueryError as Error;
    let project = app::ProjectId::parse(&params.project_id).map_err(|_| Error::InvalidInput)?;
    let limit = validate_page(params.limit, params.cursor.as_deref())?;
    if !(1..=9_007_199_254_740_991).contains(&params.expected_revision) {
        return Err(Error::InvalidInput);
    }
    let connection = binding(state, access)?;
    let evidence = registry
        .project_inventory(access, project, params.expected_revision)
        .await?;
    let snapshot = fingerprint(
        key,
        &(
            "projectInventory",
            connection,
            project,
            params.expected_revision,
            &evidence,
        ),
    )?;
    let (start, end, next_cursor) = page(
        key,
        &snapshot,
        params.cursor.as_deref(),
        limit,
        evidence.entries.len(),
    )?;
    let entries = evidence
        .entries
        .into_iter()
        .skip(start)
        .take(end - start)
        .map(|entry| {
            let state = match entry.state {
                app::InstalledEvidence::Matched => rpc::InstalledEvidence::Matched,
                app::InstalledEvidence::Missing => rpc::InstalledEvidence::Missing,
                app::InstalledEvidence::VersionMismatch => rpc::InstalledEvidence::VersionMismatch,
                app::InstalledEvidence::Unknown => rpc::InstalledEvidence::Unknown,
            };
            rpc::ProjectInventoryEntry {
                package_id: entry.package_id,
                declared_range: entry.declared_range,
                locked_version: entry.locked_version,
                observed_version: entry.observed_version,
                state,
            }
        })
        .collect();
    let resolution = match evidence.resolution {
        app::ProjectResolutionEvidence::Needed => rpc::ProjectResolutionEvidence::Needed,
        app::ProjectResolutionEvidence::NoIssueObserved => {
            rpc::ProjectResolutionEvidence::NoIssueObserved
        }
        app::ProjectResolutionEvidence::Unknown => rpc::ProjectResolutionEvidence::Unknown,
    };
    Ok(rpc::ProjectInventoryResult {
        entries,
        resolution,
        reason: evidence.reason,
        next_cursor,
    })
}

type PageError = app::CandidateQueryError;

pub(super) fn binding(
    state: &ConnectionState,
    access: &app::AccessContext,
) -> Result<String, PageError> {
    if access.is_mcp_client() {
        super::m8_tools_rpc::auth(state, access)
            .map(|auth| auth.binding())
            .map_err(|_| PageError::PermissionDenied)
    } else {
        Ok(access.principal().as_str().into())
    }
}

pub(super) fn validate_page(
    limit: Option<usize>,
    cursor: Option<&str>,
) -> Result<usize, PageError> {
    let limit = limit.unwrap_or(50);
    if !(1..=200).contains(&limit) || cursor.is_some_and(|s| s.is_empty() || s.len() > 96) {
        return Err(PageError::InvalidInput);
    }
    Ok(limit)
}

pub(super) fn fingerprint(
    key: &[u8; 32],
    input: &impl serde::Serialize,
) -> Result<String, PageError> {
    serde_json::to_vec(input)
        .map(|bytes| app::tool_cursor_mac(key, &bytes))
        .map_err(|_| PageError::Unavailable)
}

pub(super) fn page(
    key: &[u8; 32],
    snapshot: &str,
    cursor: Option<&str>,
    limit: usize,
    size: usize,
) -> Result<(usize, usize, Option<String>), PageError> {
    let signature = |offset: usize| fingerprint(key, &("page", snapshot, offset));
    let start = if let Some(cursor) = cursor {
        let (offset, bound) = cursor.split_once(':').ok_or(PageError::InvalidInput)?;
        let offset = offset
            .parse::<usize>()
            .map_err(|_| PageError::InvalidInput)?;
        if bound != signature(offset)? || offset == 0 || offset >= size {
            return Err(PageError::EvidenceStale);
        }
        offset
    } else {
        0
    };
    let end = (start + limit).min(size);
    let next = if end < size {
        Some(format!("{end}:{}", signature(end)?))
    } else {
        None
    };
    Ok((start, end, next))
}
