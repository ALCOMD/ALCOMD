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
    store: &alcomd_store::StateStoreHandle,
) -> DispatchAction {
    let capability = if matches!(
        request.method.as_str(),
        rpc::METHOD_EXTENSIONS_INSPECT_SOURCE | rpc::METHOD_EXTENSIONS_PLAN_INSTALL_FROM_SOURCE
    ) {
        rpc::CAPABILITY_EXTENSIONS_REMOTE_INSTALL_V1
    } else if request.method == rpc::METHOD_EXTENSIONS_PERMISSIONS_LIST {
        rpc::CAPABILITY_EXTENSIONS_PERMISSIONS_READ_V1
    } else if matches!(
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
        rpc::METHOD_EXTENSIONS_INSPECT_SOURCE => {
            let params: rpc::ExtensionInspectSourceParams =
                match serde_json::from_value(request.params) {
                    Ok(v) => v,
                    Err(_) => return invalid(request.id),
                };
            match application.inspect_source(access, params.url).await {
                Ok(value) => success_action(
                    request.id,
                    rpc::ExtensionSourcePreview {
                        url: value.url,
                        extension_id: value.extension_id,
                        display_name: value.display_name,
                        publisher_fingerprint: value.publisher_fingerprint,
                        description: value.description,
                        versions: value
                            .versions
                            .into_iter()
                            .map(|v| rpc::ExtensionSourceVersion {
                                version: v.version,
                                api_major: v.api_major,
                            })
                            .collect(),
                        source_fingerprint: value.source_fingerprint,
                    },
                    None,
                ),
                Err(e) => extension_error(request.id, e),
            }
        }
        rpc::METHOD_EXTENSIONS_PLAN_INSTALL_FROM_SOURCE => {
            let params: rpc::ExtensionPlanInstallFromSourceParams =
                match serde_json::from_value(request.params) {
                    Ok(v) => v,
                    Err(_) => return invalid(request.id),
                };
            match application
                .plan_install_from_source(
                    access,
                    params.url,
                    params.version,
                    params.expected_source_fingerprint,
                    now_ms(),
                )
                .await
            {
                Ok(value) => match plan_with_tools(value).await {
                    Ok(plan) => success_action(request.id, rpc::ExtensionPlanResult { plan }, None),
                    Err(e) => extension_error(request.id, e),
                },
                Err(e) => extension_error(request.id, e),
            }
        }
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
                Ok(value) => {
                    use app::M6Store;
                    let preview = async {
                        let locator = store
                            .live_package_locator(
                                access.principal().clone(),
                                value.extension_id.clone(),
                            )
                            .await?;
                        let expected = value.clone();
                        let tools = tokio::task::spawn_blocking(move || {
                            let package = alcomd_extensions::inspect_extension_directory(
                                std::path::Path::new(&locator),
                            )
                            .map_err(|_| app::M6Error::new(app::M6ErrorCode::PackageInvalid))?;
                            if package.package_digest != expected.package_digest
                                || package.publisher_fingerprint != expected.publisher_fingerprint
                            {
                                return Err(app::M6Error::new(app::M6ErrorCode::PackageInvalid));
                            }
                            let consent = package
                                .manifest
                                .background_integration()
                                .consent_preview(package.package_digest)
                                .map(|consent| rpc::ExtensionActivationConsent {
                                    package_digest: consent.package_digest,
                                    targets: consent.targets,
                                });
                            Ok((previews(&package.manifest.id, package.mcp_tools)?, consent))
                        })
                        .await
                        .map_err(|_| app::M6Error::new(app::M6ErrorCode::Internal))??;
                        let current = application.get(access, value.extension_id.clone()).await?;
                        // Tool declarations bind the signed package, not mutable runtime
                        // or grant state. Return the latest status without making normal
                        // background/crash transitions fail an otherwise valid read.
                        if current.package_digest != value.package_digest
                            || current.publisher_fingerprint != value.publisher_fingerprint
                        {
                            return Err(app::M6Error::new(app::M6ErrorCode::RevisionConflict));
                        }
                        let source = store
                            .installation_source(access.principal().clone(), current.clone())
                            .await?;
                        Ok((current, tools.0, source, tools.1))
                    }
                    .await;
                    match preview {
                        Ok((current, tools, source, activation_consent)) => success_action(
                            request.id,
                            rpc::ExtensionResult {
                                installation_source: source.map(|s| {
                                    rpc::ExtensionInstallationSource {
                                        url: s.url,
                                        version: s.version,
                                    }
                                }),
                                extension: record(current),
                                mcp_tools: Some(tools),
                                activation_consent,
                            },
                            None,
                        ),
                        Err(e) => extension_error(request.id, e),
                    }
                }
                Err(error) => extension_error(request.id, error),
            }
        }
        rpc::METHOD_EXTENSIONS_PERMISSIONS_LIST => {
            let params: rpc::ExtensionPermissionsListParams =
                match serde_json::from_value(request.params) {
                    Ok(value) => value,
                    Err(_) => return invalid(request.id),
                };
            let cursor = match params.cursor.map(parse_permissions_cursor).transpose() {
                Ok(value) => value,
                Err(()) => return invalid(request.id),
            };
            match application
                .list_permissions(
                    access,
                    params.extension_id,
                    cursor,
                    params.limit.unwrap_or(100),
                )
                .await
            {
                Ok(page) => success_action(
                    request.id,
                    rpc::ExtensionPermissionsListResult {
                        available_permissions: app::EXTENSION_PERMISSIONS
                            .iter()
                            .map(|p| (*p).to_owned())
                            .collect(),
                        extension_id: page.extension_id,
                        grant_revision: page.grant_revision.get(),
                        required_permissions: page.required_permissions,
                        optional_permissions: page.optional_permissions,
                        grants: page
                            .grants
                            .into_iter()
                            .map(|grant| rpc::ExtensionPermissionGrant {
                                permission: grant.permission,
                                resource_kind: grant.resource_kind,
                                resource_id: grant.resource_id,
                            })
                            .collect(),
                        next_cursor: page.next_cursor.map(format_permissions_cursor),
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
            if params.source_kind == rpc::ExtensionSourceKind::RemoteExtensionSource {
                return invalid(request.id);
            }
            let source_kind = match params.source_kind {
                rpc::ExtensionSourceKind::LocalOwnerSelected => {
                    app::ExtensionSourceKind::LocalOwnerSelected
                }
                rpc::ExtensionSourceKind::FirstPartyPackaged => {
                    app::ExtensionSourceKind::FirstPartyPackaged
                }
                rpc::ExtensionSourceKind::RemoteExtensionSource => return invalid(request.id),
            };
            let evidence = match application
                .inspect_install(access, source_kind, params.package_path)
                .await
            {
                Ok(value) => value,
                Err(error) => return extension_error(request.id, error),
            };
            // Preserve permission/package validation before the existing source
            // restriction; callers cannot claim the controlled bundled source.
            if source_kind != app::ExtensionSourceKind::LocalOwnerSelected {
                return extension_error(
                    request.id,
                    app::M6Error::new(app::M6ErrorCode::PackageUntrusted),
                );
            }
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
                .plan_install(access, evidence, expected, fingerprint, now_ms())
                .await
            {
                Ok(value) => {
                    let evidence = value.evidence.clone();
                    let preview = tokio::task::spawn_blocking(move || {
                        let verified = alcomd_extensions::inspect_extension_package(
                            std::path::Path::new(&evidence.source_locator),
                        )
                        .map_err(|_| app::M6Error::new(app::M6ErrorCode::PlanStale))?;
                        if verified.package_digest != evidence.package_digest
                            || verified.manifest_digest != evidence.manifest_digest
                            || verified.component_digest != evidence.component_digest
                        {
                            return Err(app::M6Error::new(app::M6ErrorCode::PlanStale));
                        }
                        previews(&verified.manifest.id, verified.mcp_tools)
                    })
                    .await;
                    match preview {
                        Ok(Ok(tools)) => {
                            let mut plan = plan(value);
                            plan.mcp_tools = tools;
                            success_action(request.id, rpc::ExtensionPlanResult { plan }, None)
                        }
                        Ok(Err(e)) => extension_error(request.id, e),
                        Err(_) => extension_error(
                            request.id,
                            app::M6Error::new(app::M6ErrorCode::Internal),
                        ),
                    }
                }
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
            let (plan_id, key, consent) = if request.method == rpc::METHOD_EXTENSIONS_APPLY_INSTALL
            {
                let params: rpc::ExtensionInstallApplyParams =
                    match serde_json::from_value(request.params) {
                        Ok(value) => value,
                        Err(_) => return invalid(request.id),
                    };
                (
                    params.plan_id,
                    params.idempotency_key,
                    Some(app::ExtensionInstallConsent {
                        plan_fingerprint: params.consent.plan_fingerprint,
                        installation_terms_accepted: params.consent.installation_terms_accepted,
                        required_permissions_accepted: params.consent.required_permissions_accepted,
                    }),
                )
            } else {
                let params: rpc::ExtensionApplyParams = match serde_json::from_value(request.params)
                {
                    Ok(value) => value,
                    Err(_) => return invalid(request.id),
                };
                (params.plan_id, params.idempotency_key, None)
            };
            let parsed = app::PlanId::parse(&plan_id)
                .ok()
                .zip(IdempotencyKey::parse(key).ok());
            let Some((plan_id, key)) = parsed else {
                return invalid(request.id);
            };
            let expected_action = if request.method == rpc::METHOD_EXTENSIONS_APPLY_INSTALL {
                "install"
            } else {
                "uninstall"
            };
            match application
                .apply(access, plan_id, key, expected_action, consent, now_ms())
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
            let params: rpc::ExtensionEnableParams = match serde_json::from_value(request.params) {
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
                    .enable_with_consent(
                        access,
                        params.extension_id,
                        expected,
                        key,
                        now_ms(),
                        params.activation_package_digest,
                    )
                    .await
            } else {
                if params.activation_package_digest.is_some() {
                    return invalid(request.id);
                }
                application
                    .disable(access, params.extension_id, expected, key, now_ms())
                    .await
            };
            match result {
                Ok(value) => success_action(
                    request.id,
                    rpc::ExtensionResult {
                        installation_source: None,
                        mcp_tools: None,
                        activation_consent: None,
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

fn parse_permissions_cursor(value: String) -> Result<app::ExtensionPermissionsCursor, ()> {
    if value.len() > 4_096 {
        return Err(());
    }
    let encoded = value.strip_prefix("m6p1:").ok_or(())?;
    if encoded.is_empty() || !encoded.len().is_multiple_of(2) {
        return Err(());
    }
    let bytes = encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            u8::from_str_radix(std::str::from_utf8(pair).map_err(|_| ())?, 16).map_err(|_| ())
        })
        .collect::<Result<Vec<_>, _>>()?;
    let cursor: app::ExtensionPermissionsCursor = serde_json::from_slice(&bytes).map_err(|_| ())?;
    cursor.validate().map_err(|_| ())?;
    Ok(cursor)
}

fn format_permissions_cursor(value: app::ExtensionPermissionsCursor) -> String {
    let encoded = serde_json::to_vec(&value).expect("bounded extension permissions cursor");
    format!("m6p1:{}", hex(&encoded))
}

fn record(value: app::ExtensionRecord) -> rpc::ExtensionRecord {
    rpc::ExtensionRecord {
        extension_id: value.extension_id,
        version: value.version,
        api_major: value.api_major,
        package_digest: hex(&value.package_digest),
        publisher_fingerprint: value.publisher_fingerprint,
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
        ui: value
            .ui_protocol
            .map(|protocol| rpc::ExtensionUiDeclaration {
                protocol: match protocol {
                    app::ExtensionUiProtocol::PortableV1 => rpc::ExtensionUiProtocol::PortableV1,
                },
            }),
    }
}

fn plan(value: app::ExtensionPlanRecord) -> rpc::ExtensionPlan {
    rpc::ExtensionPlan {
        installation_source: value.evidence.remote_source.as_ref().map(|source| {
            rpc::ExtensionInstallationSource {
                url: source.url.clone(),
                version: source.version.clone(),
            }
        }),
        installation_terms: value.evidence.installation_terms.map(|terms| {
            rpc::ExtensionInstallationTerms {
                title: terms.title,
                text: terms.text,
            }
        }),
        mcp_tools: Vec::new(),
        plan_id: value.plan_id.to_string(),
        action: value.action,
        state: value.state,
        source_kind: match value.evidence.source_kind {
            app::ExtensionSourceKind::NotApplicable => "not_applicable",
            app::ExtensionSourceKind::LocalOwnerSelected => "local_owner_selected",
            app::ExtensionSourceKind::FirstPartyPackaged => "first_party_packaged",
            app::ExtensionSourceKind::RemoteExtensionSource => "remote_extension_source",
        }
        .to_owned(),
        extension_id: value.evidence.extension_id,
        version: value.evidence.version,
        api_major: value.evidence.api_major,
        profile_version: value.evidence.profile_version,
        package_digest: hex(&value.evidence.package_digest),
        publisher_fingerprint: value.evidence.publisher_fingerprint,
        data_disposition: match value.data_disposition {
            None => "not_applicable",
            Some(app::ExtensionDataDisposition::RetainData) => "retain_data",
            Some(app::ExtensionDataDisposition::DeleteData) => "delete_data",
        }
        .to_owned(),
        plan_fingerprint: hex(&value.plan_fingerprint),
        required_permissions: value.evidence.required_permissions,
        optional_permissions: value.evidence.optional_permissions,
        ui_protocol: value.evidence.ui_protocol.map(|protocol| match protocol {
            app::ExtensionUiProtocol::PortableV1 => rpc::ExtensionUiProtocol::PortableV1,
        }),
    }
}

fn extension_error(id: String, source: app::M6Error) -> DispatchAction {
    let diagnostic_id = source.diagnostic_id().map(str::to_owned);
    let mut error = match source.code() {
        app::M6ErrorCode::InvalidInput => rpc::RpcError::invalid_request(),
        app::M6ErrorCode::PermissionDenied => rpc::RpcError::permission_denied(),
        app::M6ErrorCode::RevisionConflict => rpc::RpcError::revision_conflict(),
        app::M6ErrorCode::IdempotencyConflict => rpc::RpcError::idempotency_conflict(),
        app::M6ErrorCode::StoreUnavailable => rpc::RpcError::store_unavailable(),
        app::M6ErrorCode::Internal => rpc::RpcError::internal(OperationId::new().to_string()),
        code => rpc::RpcError::extension(error_name(code)),
    };
    if diagnostic_id.is_some() {
        error.diagnostic_id = diagnostic_id;
    }
    error_action(Some(id), error, false)
}

fn error_name(code: app::M6ErrorCode) -> &'static str {
    match code {
        app::M6ErrorCode::ManifestInvalid => rpc::error_code::EXTENSION_MANIFEST_INVALID,
        app::M6ErrorCode::PackageInvalid => rpc::error_code::EXTENSION_PACKAGE_INVALID,
        app::M6ErrorCode::PackageUntrusted => rpc::error_code::EXTENSION_PACKAGE_UNTRUSTED,
        app::M6ErrorCode::SignatureInvalid => rpc::error_code::EXTENSION_SIGNATURE_INVALID,
        app::M6ErrorCode::AlreadyInstalled => rpc::error_code::EXTENSION_ALREADY_INSTALLED,
        app::M6ErrorCode::NotInstalled => rpc::error_code::EXTENSION_NOT_INSTALLED,
        app::M6ErrorCode::ProjectNotFound => rpc::error_code::PROJECT_NOT_FOUND,
        app::M6ErrorCode::ScopeDenied => rpc::error_code::EXTENSION_SCOPE_DENIED,
        app::M6ErrorCode::ApiUnsupported => rpc::error_code::EXTENSION_API_UNSUPPORTED,
        app::M6ErrorCode::SourceInvalid => rpc::error_code::EXTENSION_SOURCE_INVALID,
        app::M6ErrorCode::SourceUnavailable => rpc::error_code::EXTENSION_SOURCE_UNAVAILABLE,
        app::M6ErrorCode::IntegrityMismatch => rpc::error_code::PACKAGE_INTEGRITY_MISMATCH,
        app::M6ErrorCode::DownloadTooLarge => rpc::error_code::PACKAGE_DOWNLOAD_TOO_LARGE,
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

fn previews(
    id: &str,
    tools: Vec<app::ToolDeclaration>,
) -> Result<Vec<rpc::McpToolPreview>, app::M6Error> {
    tools
        .into_iter()
        .map(|tool| {
            Ok(rpc::McpToolPreview {
                name: app::tool_public_name(id, &tool.name),
                title: tool.title.clone(),
                description: tool.description.clone(),
                required_permissions: tool.permissions(),
                effect: serde_json::to_value(
                    tool.effect()
                        .map_err(|_| app::M6Error::new(app::M6ErrorCode::PackageInvalid))?,
                )
                .map_err(|_| app::M6Error::new(app::M6ErrorCode::Internal))?
                .as_str()
                .ok_or_else(|| app::M6Error::new(app::M6ErrorCode::Internal))?
                .into(),
            })
        })
        .collect()
}

async fn plan_with_tools(
    value: app::ExtensionPlanRecord,
) -> Result<rpc::ExtensionPlan, app::M6Error> {
    let evidence = value.evidence.clone();
    let tools = tokio::task::spawn_blocking(move || {
        let verified = alcomd_extensions::inspect_extension_package(std::path::Path::new(
            &evidence.source_locator,
        ))
        .map_err(|_| app::M6Error::new(app::M6ErrorCode::PlanStale))?;
        if verified.package_digest != evidence.package_digest
            || verified.publisher_fingerprint != evidence.publisher_fingerprint
        {
            return Err(app::M6Error::new(app::M6ErrorCode::PlanStale));
        }
        previews(&verified.manifest.id, verified.mcp_tools)
    })
    .await
    .map_err(|_| app::M6Error::new(app::M6ErrorCode::Internal))??;
    let mut result = plan(value);
    result.mcp_tools = tools;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use app::M6Store;

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

    #[test]
    fn permissions_cursor_binds_package_record_and_exact_keyset_without_exposing_paths() {
        let extension_id = format!("dev.{}", "x".repeat(251));
        let cursor = app::ExtensionPermissionsCursor {
            extension_id: extension_id.clone(),
            grant_revision: Revision::INITIAL,
            record_revision: Revision::INITIAL,
            package_digest: [255; 32],
            manifest_digest: [254; 32],
            component_digest: [253; 32],
            installed_at_ms: i64::MAX as u64,
            install_plan_id: app::PlanId::new(),
            after: app::ExtensionPermissionGrant {
                permission: "background.run".to_owned(),
                resource_kind: "Extension".to_owned(),
                resource_id: extension_id,
            },
        };
        let encoded = format_permissions_cursor(cursor.clone());
        assert!(encoded.len() <= 4_096);
        assert!(encoded.starts_with("m6p1:"));
        assert!(!encoded.contains("dev."));
        assert_eq!(
            parse_permissions_cursor(encoded).expect("cursor round trip"),
            cursor
        );
        for invalid in ["", "m6p2:00", "m6p1:", "m6p1:0", "m6p1:zz"] {
            assert!(parse_permissions_cursor(invalid.to_owned()).is_err());
        }
        let mut malformed = serde_json::to_value(cursor).expect("cursor JSON");
        malformed["grantRevision"] = serde_json::json!(0);
        assert!(
            parse_permissions_cursor(format!(
                "m6p1:{}",
                hex(&serde_json::to_vec(&malformed).expect("malformed cursor"))
            ))
            .is_err()
        );
    }

    fn permissions_evidence() -> app::ExtensionPackageEvidence {
        app::ExtensionPackageEvidence {
            source_kind: app::ExtensionSourceKind::LocalOwnerSelected,
            source_locator: "C:/fixture/permissions.alcomdextension".to_owned(),
            source_identity: vec![1; 16],
            extension_id: "dev.example.permissions".to_owned(),
            version: "1.0.0".to_owned(),
            api_major: 1,
            profile_version: 1,
            package_digest: [1; 32],
            manifest_digest: [2; 32],
            component_digest: [3; 32],
            publisher_fingerprint: format!("ed25519-sha256:{}", "a".repeat(64)),
            installation_terms: None,
            remote_source: None,
            required_permissions: vec!["background.run".to_owned()],
            optional_permissions: vec!["projects.read".to_owned()],
            required_interfaces: vec![],
            optional_interfaces: vec![],
            ui_protocol: None,
        }
    }

    #[tokio::test]
    async fn permissions_read_rpc_requires_capability_permission_owner_and_preserves_frozen_plan_fields()
     {
        let root =
            std::env::temp_dir().join(format!("alcomd-h4-permissions-rpc-{}", OperationId::new()));
        std::fs::create_dir(&root).expect("own permissions test root");
        let store = alcomd_store::StateStoreHandle::open(root.join("state.db"))
            .expect("own permissions store");
        let evidence = permissions_evidence();
        let install = store
            .create_install_plan(
                app::PrincipalId::local_owner(),
                app::ExtensionInstallPlanDraft {
                    evidence: evidence.clone(),
                    expected_revision: None,
                    plan_fingerprint: [4; 32],
                },
                1,
            )
            .await
            .expect("frozen plan");
        let projected = plan(install.clone());
        assert_eq!(
            projected.required_permissions,
            evidence.required_permissions
        );
        assert_eq!(
            projected.optional_permissions,
            evidence.optional_permissions
        );
        let accepted = store
            .accept_plan(
                app::PrincipalId::local_owner(),
                install.plan_id,
                if install.action == "install" {
                    Some(alcomd_application::ExtensionInstallConsent {
                        plan_fingerprint: install
                            .plan_fingerprint
                            .iter()
                            .map(|b| format!("{b:02x}"))
                            .collect(),
                        installation_terms_accepted: install.evidence.installation_terms.is_some(),
                        required_permissions_accepted: !install
                            .evidence
                            .required_permissions
                            .is_empty(),
                    })
                } else {
                    None
                },
                IdempotencyKey::parse("permissions-read-install").expect("key"),
                2,
            )
            .await
            .expect("accept");
        store
            .begin_apply(accepted.operation_id, 3)
            .await
            .expect("begin");
        store
            .finish_install(accepted.operation_id, "<PERMISSIONS_PACKAGE>".to_owned(), 4)
            .await
            .expect("install state");
        store
            .complete_operation(accepted.operation_id, 5)
            .await
            .expect("complete");
        let runtime = super::super::m6_runtime::PlatformExtensionRuntime::new(store.clone())
            .expect("runtime adapter without starting a host");
        let engine =
            alcomd_extensions::ExtensionEngine::new(store.clone(), root.join("extensions"))
                .expect("package adapter");
        let application = app::M6Application::new(
            store.clone(),
            engine,
            runtime,
            std::sync::Arc::new(app::ResourceLockCoordinator::default()),
        );
        let state = ConnectionState {
            mcp_hash: None,
            oauth_endpoint: false,
            transport_observer: false,
            oauth_resource: None,
            connection_id: "permissions-read-test".to_owned(),
            client_instance_id: Some("permissions-test".to_owned()),
            actor: "Local IPC".to_owned(),
            handshake_complete: true,
            capabilities: std::collections::HashSet::from([
                rpc::CAPABILITY_EXTENSIONS_PERMISSIONS_READ_V1.to_owned(),
            ]),
        };
        let request = |params| rpc::RequestEnvelope {
            id: "permissions-read".to_owned(),
            method: rpc::METHOD_EXTENSIONS_PERMISSIONS_LIST.to_owned(),
            params,
        };
        let params = serde_json::json!({"extensionId":evidence.extension_id});
        let result = dispatch(
            request(params.clone()),
            &state,
            &application,
            &AccessContext::local_owner(),
            &store,
        )
        .await;
        let typed: rpc::SuccessResponse<rpc::ExtensionPermissionsListResult> =
            serde_json::from_value(result.response).expect("typed current permission read");
        assert_eq!(
            typed.result.available_permissions,
            app::EXTENSION_PERMISSIONS
                .iter()
                .map(|p| (*p).to_owned())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            typed
                .result
                .grants
                .iter()
                .map(|g| g.permission.clone())
                .collect::<Vec<_>>(),
            evidence.required_permissions,
            "only explicitly confirmed required permissions are granted during install"
        );
        assert_eq!(
            typed.result.required_permissions,
            evidence.required_permissions
        );
        assert_eq!(
            typed.result.optional_permissions,
            evidence.optional_permissions
        );
        let denied = AccessContext::new(app::PrincipalId::local_owner(), []);
        let result = dispatch(
            request(params.clone()),
            &state,
            &application,
            &denied,
            &store,
        )
        .await;
        assert_eq!(result.response["error"]["code"], "permission_denied");
        let other = AccessContext::new(
            app::PrincipalId::parse("fixture:other-owner").expect("other owner"),
            [app::Permission::ExtensionsRead],
        );
        let result = dispatch(
            request(params.clone()),
            &state,
            &application,
            &other,
            &store,
        )
        .await;
        assert_eq!(result.response["error"]["code"], "extension_not_installed");
        for limit in [0, 1_001] {
            let result = dispatch(
                request(serde_json::json!({"extensionId":evidence.extension_id,"limit":limit})),
                &state,
                &application,
                &AccessContext::local_owner(),
                &store,
            )
            .await;
            assert_eq!(result.response["error"]["code"], "invalid_request");
        }
        let no_capability = ConnectionState {
            capabilities: std::collections::HashSet::new(),
            ..state
        };
        let result = dispatch(
            request(params),
            &no_capability,
            &application,
            &AccessContext::local_owner(),
            &store,
        )
        .await;
        assert!(
            result.response.get("error").is_some(),
            "no read capability means no query"
        );
        let mut pending_evidence = evidence.clone();
        pending_evidence.extension_id = "dev.example.consent-authority".into();
        let pending = store
            .create_install_plan(
                app::PrincipalId::local_owner(),
                app::ExtensionInstallPlanDraft {
                    evidence: pending_evidence,
                    expected_revision: None,
                    plan_fingerprint: [4; 32],
                },
                20,
            )
            .await
            .expect("consent caller authority fixture");
        let installer_without_grants = AccessContext::new(
            app::PrincipalId::local_owner(),
            [app::Permission::ExtensionsManage],
        );
        let refusal = application
            .apply(
                &installer_without_grants,
                pending.plan_id,
                IdempotencyKey::parse("no-grant-authority")
                    .expect("consent caller authority fixture"),
                "install",
                Some(app::ExtensionInstallConsent {
                    plan_fingerprint: "04".repeat(32),
                    installation_terms_accepted: false,
                    required_permissions_accepted: true,
                }),
                21,
            )
            .await
            .expect_err("consent caller authority fixture must reject");
        assert_eq!(refusal.code(), app::M6ErrorCode::PermissionDenied);
        assert_eq!(
            store
                .get_plan(pending.plan_id)
                .await
                .expect("consent caller authority fixture")
                .state,
            "unapplied"
        );
        drop(application);
        drop(store);
        std::fs::remove_dir_all(&root).expect("remove owned isolated permissions fixture");
    }
}
