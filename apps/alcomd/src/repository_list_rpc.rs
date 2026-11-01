use super::{
    AccessContext, ConnectionState, DispatchAction, RepositoryListApplication, error_action,
    invalid, require_capability, success_action,
};
use alcomd_application as app;
use alcomd_protocol as rpc;

pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    application: &RepositoryListApplication,
    access: &AccessContext,
) -> DispatchAction {
    if let Some(action) = require_capability(
        &request.id,
        state,
        rpc::CAPABILITY_REPOSITORIES_LIST_FILES_V1,
    ) {
        return action;
    }
    match request.method.as_str() {
        rpc::METHOD_REPOSITORIES_INSPECT_LIST_FILE => {
            let Ok(params) =
                serde_json::from_value::<rpc::RepositoryListInspectParams>(request.params)
            else {
                return invalid(request.id);
            };
            match application.inspect(access, params.path).await {
                Ok(result) => success_action(
                    request.id,
                    rpc::RepositoryListInspection {
                        ignored_lines: result.ignored_lines,
                        entries: result
                            .entries
                            .into_iter()
                            .map(|entry| rpc::RepositoryListEntry {
                                line: entry.line,
                                status: match entry.status {
                                    app::RepositoryListEntryStatus::Candidate => {
                                        rpc::RepositoryListEntryStatus::Candidate
                                    }
                                    app::RepositoryListEntryStatus::Duplicate => {
                                        rpc::RepositoryListEntryStatus::Duplicate
                                    }
                                    app::RepositoryListEntryStatus::Invalid => {
                                        rpc::RepositoryListEntryStatus::Invalid
                                    }
                                    app::RepositoryListEntryStatus::Unsupported => {
                                        rpc::RepositoryListEntryStatus::Unsupported
                                    }
                                },
                                source: match entry.source {
                                    Some(app::RepositorySource::Remote { url }) => {
                                        Some(rpc::RepositoryListSource::Remote { url })
                                    }
                                    _ => None,
                                },
                                reason: entry.reason.map(str::to_owned),
                            })
                            .collect(),
                    },
                    None,
                ),
                Err(error) => failure(request.id, error),
            }
        }
        rpc::METHOD_REPOSITORIES_PREVIEW_LIST_EXPORT => {
            if serde_json::from_value::<rpc::RepositoryListPreviewParams>(request.params).is_err() {
                return invalid(request.id);
            }
            match application.preview(access).await {
                Ok(result) => success_action(request.id, rpc::RepositoryListPreview {
                    exportable_count: result.exportable_count, byte_count: result.byte_count, fingerprint: result.fingerprint,
                    excluded: result.excluded.into_iter().map(|entry| rpc::RepositoryListExclusion {
                        repository_id: entry.repository_id,
                        reason: match entry.reason {
                            "local_source" => rpc::RepositoryListExclusionReason::LocalSource,
                            "authentication_not_supported" => rpc::RepositoryListExclusionReason::AuthenticationNotSupported,
                            _ => rpc::RepositoryListExclusionReason::UnsupportedSource,
                        },
                    }).collect(),
                }, None),
                Err(error) => failure(request.id, error),
            }
        }
        rpc::METHOD_REPOSITORIES_EXPORT_LIST_FILE => {
            let Ok(params) =
                serde_json::from_value::<rpc::RepositoryListExportParams>(request.params)
            else {
                return invalid(request.id);
            };
            if params.expected_fingerprint.len() != 64
                || !params
                    .expected_fingerprint
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return invalid(request.id);
            }
            match application
                .export(access, params.target_path, params.expected_fingerprint)
                .await
            {
                Ok(result) => success_action(
                    request.id,
                    rpc::RepositoryListExportResult {
                        exported_count: result.exported_count,
                        byte_count: result.byte_count,
                        sha256: result.sha256,
                    },
                    None,
                ),
                Err(error) => failure(request.id, error),
            }
        }
        _ => error_action(Some(request.id), rpc::RpcError::method_not_found(), false),
    }
}

fn failure(id: String, error: app::RepositoryListError) -> DispatchAction {
    error_action(
        Some(id),
        rpc::RpcError {
            code: error.0.into(),
            message: "Repository list request could not be completed".into(),
            diagnostic_id: None,
            data: None,
        },
        false,
    )
}
