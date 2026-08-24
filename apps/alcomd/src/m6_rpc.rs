use alcomd_application as app;
use alcomd_protocol as rpc;

use super::{
    AccessContext, ConnectionState, DispatchAction, IdempotencyKey, M6ExtensionApplication,
    OperationId, Revision, error_action, invalid, require_capability, success_action,
};

pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    application: &M6ExtensionApplication,
    access: &AccessContext,
) -> DispatchAction {
    let capability = if matches!(
        request.method.as_str(),
        rpc::METHOD_EXTENSIONS_SET_GRANT | rpc::METHOD_EXTENSIONS_REVOKE_GRANT
    ) {
        rpc::CAPABILITY_EXTENSIONS_PERMISSIONS_V1
    } else {
        rpc::CAPABILITY_EXTENSIONS_LIFECYCLE_V1
    };
    if let Some(action) = require_capability(&request.id, state, capability) {
        return action;
    }
    match request.method.as_str() {
        rpc::METHOD_EXTENSIONS_LIST => {
            let params: rpc::ExtensionsListParams = match serde_json::from_value(request.params) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
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
                    rpc::ExtensionsListResult {
                        extensions: page.extensions.into_iter().map(record).collect(),
                        next_cursor: page.next_cursor.map(format_cursor),
                    },
                    None,
                ),
                Err(error) => extension_error(request.id, error),
            }
        }
        rpc::METHOD_EXTENSIONS_GET => {
            let params: rpc::ExtensionIdParams = match serde_json::from_value(request.params) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            match application.get(access, params.extension_id).await {
                Ok(value) => success_action(
                    request.id,
                    rpc::ExtensionResult {
                        extension: record(value),
                    },
                    None,
                ),
                Err(error) => extension_error(request.id, error),
            }
        }
        rpc::METHOD_EXTENSIONS_PLAN_INSTALL => {
            let params: rpc::ExtensionPlanInstallParams =
                match serde_json::from_value(request.params) {
                    Ok(value) => value,
                    Err(_) => return invalid(request.id),
                };
            let source_kind = match params.source_kind {
                rpc::ExtensionSourceKind::LocalOwnerSelected => {
                    app::ExtensionSourceKind::LocalOwnerSelected
                }
                rpc::ExtensionSourceKind::FirstPartyPackaged => {
                    app::ExtensionSourceKind::FirstPartyPackaged
                }
            };
            let evidence = match application
                .inspect_install(access, source_kind, params.package_path)
                .await
            {
                Ok(value) => value,
                Err(error) => return extension_error(request.id, error),
            };
            let trust = match (source_kind, params.publisher_approval) {
                (
                    app::ExtensionSourceKind::LocalOwnerSelected,
                    rpc::ExtensionPublisherApproval::ApproveForExtension,
                ) => app::ExtensionTrustDecision::UserApprovedForExtension,
                (
                    app::ExtensionSourceKind::LocalOwnerSelected,
                    rpc::ExtensionPublisherApproval::None,
                ) => {
                    return extension_error(
                        request.id,
                        app::M6Error::new(app::M6ErrorCode::PublisherConfirmationRequired),
                    );
                }
                _ => {
                    return extension_error(
                        request.id,
                        app::M6Error::new(app::M6ErrorCode::PackageUntrusted),
                    );
                }
            };
            let expected = if params.expected_revision == 0 {
                None
            } else {
                match Revision::new(params.expected_revision) {
                    Some(value) => Some(value),
                    None => return invalid(request.id),
                }
            };
            let fingerprint = evidence.package_digest;
            match application
                .plan_install(access, evidence, trust, expected, fingerprint, now_ms())
                .await
            {
                Ok(value) => success_action(
                    request.id,
                    rpc::ExtensionPlanResult { plan: plan(value) },
                    None,
                ),
                Err(error) => extension_error(request.id, error),
            }
        }
        rpc::METHOD_EXTENSIONS_PLAN_UNINSTALL => {
            let params: rpc::ExtensionPlanUninstallParams =
                match serde_json::from_value(request.params) {
                    Ok(value) => value,
                    Err(_) => return invalid(request.id),
                };
            let Some(expected) = Revision::new(params.expected_revision) else {
                return invalid(request.id);
            };
            let disposition = match params.data_disposition {
                rpc::ExtensionDataDisposition::RetainData => {
                    app::ExtensionDataDisposition::RetainData
                }
                rpc::ExtensionDataDisposition::DeleteData => {
                    app::ExtensionDataDisposition::DeleteData
                }
            };
            let mut fingerprint = [0_u8; 32];
            fingerprint[..8].copy_from_slice(&expected.get().to_le_bytes());
            match application
                .plan_uninstall(
                    access,
                    params.extension_id,
                    expected,
                    disposition,
                    fingerprint,
                    now_ms(),
                )
                .await
            {
                Ok(value) => success_action(
                    request.id,
                    rpc::ExtensionPlanResult { plan: plan(value) },
                    None,
                ),
                Err(error) => extension_error(request.id, error),
            }
        }
        rpc::METHOD_EXTENSIONS_APPLY_INSTALL | rpc::METHOD_EXTENSIONS_APPLY_UNINSTALL => {
            let params: rpc::ExtensionApplyParams = match serde_json::from_value(request.params) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            let parsed = app::PlanId::parse(&params.plan_id)
                .ok()
                .zip(IdempotencyKey::parse(params.idempotency_key).ok());
            let Some((plan_id, key)) = parsed else {
                return invalid(request.id);
            };
            let expected_action = if request.method == rpc::METHOD_EXTENSIONS_APPLY_INSTALL {
                "install"
            } else {
                "uninstall"
            };
            match application
                .apply(access, plan_id, key, expected_action, now_ms())
                .await
            {
                Ok(value) => success_action(
                    request.id,
                    rpc::ExtensionOperationResult {
                        operation_id: value.operation_id.to_string(),
                        replayed: value.replayed,
                    },
                    None,
                ),
                Err(error) => extension_error(request.id, error),
            }
        }
        rpc::METHOD_EXTENSIONS_ENABLE | rpc::METHOD_EXTENSIONS_DISABLE => {
            let params: rpc::ExtensionLifecycleParams = match serde_json::from_value(request.params)
            {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            let parsed = Revision::new(params.expected_revision)
                .zip(IdempotencyKey::parse(params.idempotency_key).ok());
            let Some((expected, key)) = parsed else {
                return invalid(request.id);
            };
            let result = if request.method == rpc::METHOD_EXTENSIONS_ENABLE {
                application
                    .enable(access, params.extension_id, expected, key, now_ms())
                    .await
            } else {
                application
                    .disable(access, params.extension_id, expected, key, now_ms())
                    .await
            };
            match result {
                Ok(value) => success_action(
                    request.id,
                    rpc::ExtensionResult {
                        extension: record(value),
                    },
                    None,
                ),
                Err(error) => extension_error(request.id, error),
            }
        }
        rpc::METHOD_EXTENSIONS_SET_GRANT | rpc::METHOD_EXTENSIONS_REVOKE_GRANT => {
            let params: rpc::ExtensionGrantParams = match serde_json::from_value(request.params) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            let parsed = Revision::new(params.expected_grant_revision)
                .zip(IdempotencyKey::parse(params.idempotency_key).ok());
            let Some((expected, key)) = parsed else {
                return invalid(request.id);
            };
            match application
                .set_grant(
                    access,
                    params.extension_id,
                    params.permission,
                    params.resource_kind,
                    params.resource_id,
                    expected,
                    key,
                    request.method == rpc::METHOD_EXTENSIONS_SET_GRANT,
                    now_ms(),
                )
                .await
            {
                Ok(value) => success_action(
                    request.id,
                    rpc::ExtensionGrantResult {
                        extension_id: value.extension_id,
                        grant_revision: value.grant_revision.get(),
                        state: if value.granted { "granted" } else { "revoked" }.to_owned(),
                        replayed: value.replayed,
                    },
                    None,
                ),
                Err(error) => extension_error(request.id, error),
            }
        }
        _ => error_action(Some(request.id), rpc::RpcError::method_not_found(), false),
    }
}

fn parse_cursor(value: String) -> Result<app::ExtensionCursor, ()> {
    let encoded = value.strip_prefix("m6e1:").ok_or(())?;
    if encoded.is_empty() || encoded.len() > 510 || encoded.len() % 2 != 0 {
        return Err(());
    }
    let mut bytes = Vec::with_capacity(encoded.len() / 2);
    for pair in encoded.as_bytes().chunks_exact(2) {
        let pair = std::str::from_utf8(pair).map_err(|_| ())?;
        bytes.push(u8::from_str_radix(pair, 16).map_err(|_| ())?);
    }
    let extension_id = String::from_utf8(bytes).map_err(|_| ())?;
    app::ExtensionCursor::new(extension_id).map_err(|_| ())
}

fn format_cursor(value: app::ExtensionCursor) -> String {
    format!("m6e1:{}", hex(value.last_extension_id().as_bytes()))
}

fn record(value: app::ExtensionRecord) -> rpc::ExtensionRecord {
    rpc::ExtensionRecord {
        extension_id: value.extension_id,
        version: value.version,
        api_major: value.api_major,
        package_digest: hex(&value.package_digest),
        publisher_fingerprint: value.publisher_fingerprint,
        trust_decision: match value.trust_decision {
            app::ExtensionTrustDecision::Official => rpc::ExtensionTrustDecision::Official,
            app::ExtensionTrustDecision::UserApprovedForExtension => {
                rpc::ExtensionTrustDecision::UserApprovedForExtension
            }
        },
        desired_state: match value.desired_state {
            app::ExtensionDesiredState::InstalledDisabled => {
                rpc::ExtensionDesiredState::InstalledDisabled
            }
            app::ExtensionDesiredState::Enabled => rpc::ExtensionDesiredState::Enabled,
            app::ExtensionDesiredState::Uninstalling => rpc::ExtensionDesiredState::Uninstalling,
        },
        quarantine_state: match value.quarantine_state {
            app::ExtensionQuarantineState::Clear => rpc::ExtensionQuarantineState::Clear,
            app::ExtensionQuarantineState::Quarantined => {
                rpc::ExtensionQuarantineState::Quarantined
            }
        },
        runtime_state: match value.runtime_state {
            app::ExtensionRuntimeState::Stopped => rpc::ExtensionRuntimeState::Stopped,
            app::ExtensionRuntimeState::Starting => rpc::ExtensionRuntimeState::Starting,
            app::ExtensionRuntimeState::Running => rpc::ExtensionRuntimeState::Running,
            app::ExtensionRuntimeState::Stopping => rpc::ExtensionRuntimeState::Stopping,
            app::ExtensionRuntimeState::Crashed => rpc::ExtensionRuntimeState::Crashed,
        },
        grant_revision: value.grant_revision.get(),
        lifecycle_generation: value.lifecycle_generation.get(),
        revision: value.revision.get(),
    }
}

fn plan(value: app::ExtensionPlanRecord) -> rpc::ExtensionPlan {
    rpc::ExtensionPlan {
        plan_id: value.plan_id.to_string(),
        action: value.action,
        state: value.state,
        source_kind: match value.evidence.source_kind {
            app::ExtensionSourceKind::NotApplicable => "not_applicable",
            app::ExtensionSourceKind::LocalOwnerSelected => "local_owner_selected",
            app::ExtensionSourceKind::FirstPartyPackaged => "first_party_packaged",
        }
        .to_owned(),
        extension_id: value.evidence.extension_id,
        version: value.evidence.version,
        api_major: value.evidence.api_major,
        profile_version: value.evidence.profile_version,
        package_digest: hex(&value.evidence.package_digest),
        publisher_fingerprint: value.evidence.publisher_fingerprint,
        trust_decision: match value.trust_decision {
            app::ExtensionTrustDecision::Official => rpc::ExtensionTrustDecision::Official,
            app::ExtensionTrustDecision::UserApprovedForExtension => {
                rpc::ExtensionTrustDecision::UserApprovedForExtension
            }
        },
        data_disposition: match value.data_disposition {
            None => "not_applicable",
            Some(app::ExtensionDataDisposition::RetainData) => "retain_data",
            Some(app::ExtensionDataDisposition::DeleteData) => "delete_data",
        }
        .to_owned(),
        plan_fingerprint: hex(&value.plan_fingerprint),
    }
}

fn extension_error(id: String, source: app::M6Error) -> DispatchAction {
    let error = match source.code() {
        app::M6ErrorCode::InvalidInput => rpc::RpcError::invalid_request(),
        app::M6ErrorCode::PermissionDenied => rpc::RpcError::permission_denied(),
        app::M6ErrorCode::RevisionConflict => rpc::RpcError::revision_conflict(),
        app::M6ErrorCode::IdempotencyConflict => rpc::RpcError::idempotency_conflict(),
        app::M6ErrorCode::StoreUnavailable => rpc::RpcError::store_unavailable(),
        app::M6ErrorCode::Internal => rpc::RpcError::internal(OperationId::new().to_string()),
        code => rpc::RpcError::extension(error_name(code)),
    };
    error_action(Some(id), error, false)
}

fn error_name(code: app::M6ErrorCode) -> &'static str {
    match code {
        app::M6ErrorCode::ManifestInvalid => rpc::error_code::EXTENSION_MANIFEST_INVALID,
        app::M6ErrorCode::PackageInvalid => rpc::error_code::EXTENSION_PACKAGE_INVALID,
        app::M6ErrorCode::PackageUntrusted => rpc::error_code::EXTENSION_PACKAGE_UNTRUSTED,
        app::M6ErrorCode::PublisherConfirmationRequired => {
            rpc::error_code::EXTENSION_PUBLISHER_CONFIRMATION_REQUIRED
        }
        app::M6ErrorCode::SignatureInvalid => rpc::error_code::EXTENSION_SIGNATURE_INVALID,
        app::M6ErrorCode::AlreadyInstalled => rpc::error_code::EXTENSION_ALREADY_INSTALLED,
        app::M6ErrorCode::NotInstalled => rpc::error_code::EXTENSION_NOT_INSTALLED,
        app::M6ErrorCode::ProjectNotFound => rpc::error_code::PROJECT_NOT_FOUND,
        app::M6ErrorCode::ScopeDenied => rpc::error_code::EXTENSION_SCOPE_DENIED,
        app::M6ErrorCode::ApiUnsupported => rpc::error_code::EXTENSION_API_UNSUPPORTED,
        app::M6ErrorCode::InstanceStale => rpc::error_code::EXTENSION_INSTANCE_STALE,
        app::M6ErrorCode::ResourceLimit => rpc::error_code::EXTENSION_RESOURCE_LIMIT,
        app::M6ErrorCode::Crashed => rpc::error_code::EXTENSION_CRASHED,
        app::M6ErrorCode::Quarantined => rpc::error_code::EXTENSION_QUARANTINED,
        app::M6ErrorCode::PlanStale => rpc::error_code::EXTENSION_PLAN_STALE,
        app::M6ErrorCode::DataQuotaExceeded => rpc::error_code::EXTENSION_DATA_QUOTA_EXCEEDED,
        app::M6ErrorCode::DataOwnerMismatch => rpc::error_code::EXTENSION_DATA_OWNER_MISMATCH,
        app::M6ErrorCode::RecoveryRequired => rpc::error_code::EXTENSION_RECOVERY_REQUIRED,
        app::M6ErrorCode::InvalidInput
        | app::M6ErrorCode::PermissionDenied
        | app::M6ErrorCode::RevisionConflict
        | app::M6ErrorCode::IdempotencyConflict
        | app::M6ErrorCode::StoreUnavailable
        | app::M6ErrorCode::Internal => rpc::error_code::INTERNAL_ERROR,
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

fn hex(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_cursor_is_versioned_opaque_and_fail_closed() {
        let cursor =
            app::ExtensionCursor::new("dev.example.fixture".to_owned()).expect("extension cursor");
        let encoded = format_cursor(cursor);
        assert!(encoded.starts_with("m6e1:"));
        assert!(!encoded.contains("dev.example.fixture"));
        assert_eq!(
            parse_cursor(encoded)
                .expect("parse cursor")
                .last_extension_id(),
            "dev.example.fixture"
        );
        for invalid in [
            "",
            "m6e2:6465762e6578616d706c652e66697874757265",
            "m6e1:",
            "m6e1:0",
            "m6e1:zz",
            "m6e1:4445562e4558414d504c452e46495854555245",
        ] {
            assert!(parse_cursor(invalid.to_owned()).is_err(), "{invalid}");
        }
    }
}
