use alcomd_application as app;
use alcomd_protocol as rpc;

use super::{
    AccessContext, ConnectionState, DispatchAction, IdempotencyKey, M4PackageApplication,
    OperationId, Revision, error_action, invalid, require_capability, success_action,
};

pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    application: &M4PackageApplication,
    access: &AccessContext,
) -> DispatchAction {
    let (action, v2_only) = match request.method.as_str() {
        rpc::METHOD_PACKAGES_PLAN_INSTALL => (app::PlanAction::Install, false),
        rpc::METHOD_PACKAGES_PLAN_REMOVE => (app::PlanAction::Remove, false),
        rpc::METHOD_PACKAGES_PLAN_UPGRADE => (app::PlanAction::Upgrade, false),
        rpc::METHOD_PACKAGES_PLAN_DOWNGRADE => (app::PlanAction::Downgrade, false),
        rpc::METHOD_PACKAGES_PLAN_RESOLVE => (app::PlanAction::Resolve, false),
        rpc::METHOD_PACKAGES_PLAN_REINSTALL => (app::PlanAction::Reinstall, true),
        rpc::METHOD_PACKAGES_PLAN_BULK => (app::PlanAction::Bulk, true),
        rpc::METHOD_PACKAGES_APPLY_PLAN => return apply(request, state, application, access).await,
        _ => return error_action(Some(request.id), rpc::RpcError::method_not_found(), false),
    };
    if v2_only {
        if let Some(value) =
            require_capability(&request.id, state, rpc::CAPABILITY_PACKAGES_PLAN_V2)
        {
            return value;
        }
    } else if !state
        .capabilities
        .contains(rpc::CAPABILITY_PACKAGES_PLAN_V1)
        && !state
            .capabilities
            .contains(rpc::CAPABILITY_PACKAGES_PLAN_V2)
        && let Some(value) =
            require_capability(&request.id, state, rpc::CAPABILITY_PACKAGES_PLAN_V1)
    {
        return value;
    }
    let plan_version = if state
        .capabilities
        .contains(rpc::CAPABILITY_PACKAGES_PLAN_V2)
    {
        app::PackagePlanVersion::V2
    } else {
        app::PackagePlanVersion::V1
    };
    let plan_request = match plan_request(action, plan_version, request.params) {
        Ok(value) => value,
        Err(()) => return invalid(request.id),
    };
    match application.plan(access, plan_request).await {
        Ok(plan) => success_action(request.id, plan_to_rpc(plan), None),
        Err(error) => m4_error(request.id, error),
    }
}

async fn apply(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    application: &M4PackageApplication,
    access: &AccessContext,
) -> DispatchAction {
    if let Some(value) = require_capability(&request.id, state, rpc::CAPABILITY_PACKAGES_APPLY_V1) {
        return value;
    }
    let params: rpc::PackageApplyPlanParams = match serde_json::from_value(request.params) {
        Ok(value) => value,
        Err(_) => return invalid(request.id),
    };
    let plan_id = match app::PlanId::parse(&params.plan_id) {
        Ok(value) => value,
        Err(_) => return invalid(request.id),
    };
    let revision = match Revision::new(params.expected_revision) {
        Some(value) => value,
        None => return invalid(request.id),
    };
    let key = match IdempotencyKey::parse(params.idempotency_key) {
        Ok(value) => value,
        Err(_) => return invalid(request.id),
    };
    match application.apply_plan(access, plan_id, revision, key).await {
        Ok(value) => success_action(
            request.id,
            rpc::PackageApplyPlanResult {
                operation_id: value.operation_id.to_string(),
                replayed: value.replayed,
            },
            None,
        ),
        Err(error) => m4_error(request.id, error),
    }
}

fn plan_request(
    action: app::PlanAction,
    plan_version: app::PackagePlanVersion,
    params: serde_json::Value,
) -> Result<app::PackagePlanRequest, ()> {
    if action == app::PlanAction::Reinstall {
        let value: rpc::PackagePlanReinstallParams =
            serde_json::from_value(params).map_err(|_| ())?;
        let selection = match value.selection {
            rpc::PackageReinstallSelection::Packages { package_ids } => {
                app::ReinstallSelection::Packages(package_ids)
            }
            rpc::PackageReinstallSelection::All => app::ReinstallSelection::All,
        };
        let sources = value
            .sources
            .into_iter()
            .map(|value| Ok((value.package_id, source_selector(value.source)?)))
            .collect::<Result<Vec<_>, ()>>()?;
        return Ok(app::PackagePlanRequest {
            allow_unity_incompatible: false,
            action,
            project_id: app::ProjectId::parse(&value.project_id).map_err(|_| ())?,
            expected_revision: Revision::new(value.expected_revision).ok_or(())?,
            package_id: None,
            version_range: None,
            repository_id: None,
            include_prerelease: false,
            plan_version,
            source_selector: None,
            reinstall_selection: Some(selection),
            reinstall_sources: sources,
            bulk_intents: Vec::new(),
        });
    }
    if action == app::PlanAction::Bulk {
        let value: rpc::PackagePlanBulkParams = serde_json::from_value(params).map_err(|_| ())?;
        let intents = value
            .intents
            .into_iter()
            .map(bulk_intent)
            .collect::<Result<Vec<_>, ()>>()?;
        return Ok(app::PackagePlanRequest {
            allow_unity_incompatible: false,
            action,
            project_id: app::ProjectId::parse(&value.project_id).map_err(|_| ())?,
            expected_revision: Revision::new(value.expected_revision).ok_or(())?,
            package_id: None,
            version_range: None,
            repository_id: None,
            include_prerelease: false,
            plan_version,
            source_selector: None,
            reinstall_selection: None,
            reinstall_sources: Vec::new(),
            bulk_intents: intents,
        });
    }
    let (
        project_id,
        expected_revision,
        package_id,
        version_range,
        repository_id,
        source,
        prerelease,
        allow_unity_incompatible,
    ) = match action {
        app::PlanAction::Install | app::PlanAction::Upgrade => {
            let value: rpc::PackagePlanInstallParams =
                serde_json::from_value(params).map_err(|_| ())?;
            (
                value.project_id,
                value.expected_revision,
                Some(value.package_id),
                value.version_range,
                value.repository_id,
                value.source,
                value.include_prerelease,
                value.allow_unity_incompatible,
            )
        }
        app::PlanAction::Remove => {
            let value: rpc::PackagePlanRemoveParams =
                serde_json::from_value(params).map_err(|_| ())?;
            (
                value.project_id,
                value.expected_revision,
                Some(value.package_id),
                None,
                None,
                None,
                false,
                false,
            )
        }
        app::PlanAction::Downgrade => {
            let value: rpc::PackagePlanDowngradeParams =
                serde_json::from_value(params).map_err(|_| ())?;
            (
                value.project_id,
                value.expected_revision,
                Some(value.package_id),
                Some(format!("={}", value.version)),
                value.repository_id,
                value.source,
                false,
                value.allow_unity_incompatible,
            )
        }
        app::PlanAction::Resolve => {
            let value: rpc::PackagePlanResolveParams =
                serde_json::from_value(params).map_err(|_| ())?;
            (
                value.project_id,
                value.expected_revision,
                None,
                None,
                None,
                None,
                value.include_prerelease,
                false,
            )
        }
        app::PlanAction::Reinstall | app::PlanAction::Bulk => return Err(()),
    };
    if source.is_some() && plan_version == app::PackagePlanVersion::V1 {
        return Err(());
    }
    if source.is_some() && repository_id.is_some() {
        return Err(());
    }
    Ok(app::PackagePlanRequest {
        allow_unity_incompatible,
        action,
        project_id: app::ProjectId::parse(&project_id).map_err(|_| ())?,
        expected_revision: Revision::new(expected_revision).ok_or(())?,
        package_id,
        version_range,
        repository_id,
        include_prerelease: prerelease,
        plan_version,
        source_selector: source.map(source_selector).transpose()?,
        reinstall_selection: None,
        reinstall_sources: Vec::new(),
        bulk_intents: Vec::new(),
    })
}

fn source_selector(value: rpc::PackageSourceSelector) -> Result<app::PackageSourceSelector, ()> {
    match value {
        rpc::PackageSourceSelector::Repository { repository_id } => {
            Ok(app::PackageSourceSelector::Repository { repository_id })
        }
        rpc::PackageSourceSelector::UserPackage { user_package_id } => {
            Ok(app::PackageSourceSelector::UserPackage {
                user_package_id: app::UserPackageId::parse(&user_package_id).map_err(|_| ())?,
            })
        }
    }
}

fn bulk_intent(value: rpc::PackageBulkIntent) -> Result<app::PackageBulkIntent, ()> {
    Ok(match value {
        rpc::PackageBulkIntent::Install {
            package_id,
            version_range,
            source,
            include_prerelease,
        } => app::PackageBulkIntent::Install {
            package_id,
            version_range,
            source: source.map(source_selector).transpose()?,
            include_prerelease,
        },
        rpc::PackageBulkIntent::Upgrade {
            package_id,
            version_range,
            source,
            include_prerelease,
        } => app::PackageBulkIntent::Upgrade {
            package_id,
            version_range,
            source: source.map(source_selector).transpose()?,
            include_prerelease,
        },
        rpc::PackageBulkIntent::Remove { package_id } => {
            app::PackageBulkIntent::Remove { package_id }
        }
        rpc::PackageBulkIntent::Reinstall { package_id, source } => {
            app::PackageBulkIntent::Reinstall {
                package_id,
                source: source.map(source_selector).transpose()?,
            }
        }
    })
}

fn plan_to_rpc(value: app::PackagePlanRecord) -> rpc::PackagePlan {
    rpc::PackagePlan {
        plan_id: value.plan_id.to_string(),
        action: match value.action {
            app::PlanAction::Install => rpc::PackagePlanAction::Install,
            app::PlanAction::Remove => rpc::PackagePlanAction::Remove,
            app::PlanAction::Upgrade => rpc::PackagePlanAction::Upgrade,
            app::PlanAction::Downgrade => rpc::PackagePlanAction::Downgrade,
            app::PlanAction::Resolve => rpc::PackagePlanAction::Resolve,
            app::PlanAction::Reinstall => rpc::PackagePlanAction::Reinstall,
            app::PlanAction::Bulk => rpc::PackagePlanAction::Bulk,
        },
        state: match value.state {
            app::PlanState::Unapplied => rpc::PackagePlanState::Unapplied,
            app::PlanState::Applied => rpc::PackagePlanState::Applied,
        },
        project_id: value.project_id.to_string(),
        project_revision: value.project_revision.get(),
        change_set_fingerprint: hex(&value.change_set_fingerprint),
        change_set: rpc::PackageChangeSet {
            removal_conflicts: value
                .change_set
                .removal_conflicts
                .into_iter()
                .map(|conflict| rpc::PackageRemovalConflict {
                    package_id: conflict.package_id,
                    required_by: conflict
                        .required_by
                        .into_iter()
                        .map(|required| rpc::PackageRemovalRequirement {
                            package_id: required.package_id,
                            range: required.range,
                        })
                        .collect(),
                })
                .collect(),
            unity_compatibility_overrides: value
                .change_set
                .unity_compatibility_overrides
                .into_iter()
                .map(|item| rpc::PackageUnityCompatibilityOverride {
                    package_id: item.package_id,
                    version: item.version,
                })
                .collect(),
            format_version: value.change_set.format_version,
            mutations: value
                .change_set
                .mutations
                .into_iter()
                .map(mutation)
                .collect(),
            dependency_edges: value
                .change_set
                .dependency_edges
                .into_iter()
                .map(|edge| rpc::PackageDependencyEdge {
                    from_package_id: edge.from_package_id,
                    to_package_id: edge.to_package_id,
                    range: edge.range,
                    direct: edge.direct,
                })
                .collect(),
            legacy_removals: value
                .change_set
                .legacy_removals
                .into_iter()
                .map(|removal| rpc::PackageLegacyRemoval {
                    path: removal.path,
                    kind: match removal.kind {
                        app::LegacyRemovalKind::File => rpc::PackageLegacyRemovalKind::File,
                        app::LegacyRemovalKind::Directory => {
                            rpc::PackageLegacyRemovalKind::Directory
                        }
                    },
                    package_id: removal.package_id,
                })
                .collect(),
            vpm_manifest_sha256: hex(&value.change_set.vpm_manifest_sha256),
        },
    }
}

fn mutation(value: app::PackageMutation) -> rpc::PackageMutation {
    rpc::PackageMutation {
        kind: match value.kind {
            app::PackageMutationKind::Install => rpc::PackageMutationKind::Install,
            app::PackageMutationKind::Remove => rpc::PackageMutationKind::Remove,
            app::PackageMutationKind::Replace => rpc::PackageMutationKind::Replace,
        },
        package_id: value.package_id,
        from_version: value.from_version,
        to_version: value.to_version,
        source: value.source.map(source_pin),
    }
}

fn source_pin(value: app::PackageSourcePin) -> rpc::PackageSourcePin {
    match value {
        app::PackageSourcePin::Repository(source) => {
            rpc::PackageSourcePin::Repository(rpc::RepositoryPackageSourcePin {
                repository_id: source.repository_id,
                repository_revision: source.repository_revision,
                source_identity: source.source_identity,
                manifest_fingerprint: hex(&source.manifest_fingerprint),
                package_id: source.package_id,
                version: source.version,
                artifact_url: source.artifact_url,
                archive_sha256: source.archive_sha256.map(|digest| hex(&digest)),
            })
        }
        app::PackageSourcePin::UserPackage(source) => {
            rpc::PackageSourcePin::UserPackage(rpc::UserPackageSourcePin {
                user_package_id: source.user_package_id.to_string(),
                source_revision: source.source_revision,
                source_identity: source.source_identity,
                manifest_fingerprint: hex(&source.manifest_fingerprint),
                package_id: source.package_id,
                version: source.version,
                archive_sha256: hex(&source.archive_sha256),
            })
        }
    }
}

fn m4_error(id: String, error: app::M4Error) -> DispatchAction {
    let diagnostic_id = error.diagnostic_id().map(str::to_owned);
    let mut rpc_error = match error.code() {
        app::M4ErrorCode::InvalidInput => rpc::RpcError::invalid_request(),
        app::M4ErrorCode::PermissionDenied => rpc::RpcError::permission_denied(),
        app::M4ErrorCode::RevisionConflict => rpc::RpcError::revision_conflict(),
        app::M4ErrorCode::IdempotencyConflict => rpc::RpcError::idempotency_conflict(),
        app::M4ErrorCode::StoreUnavailable => rpc::RpcError::store_unavailable(),
        app::M4ErrorCode::Internal => rpc::RpcError::internal(OperationId::new().to_string()),
        app::M4ErrorCode::OperationCancelled => {
            rpc::RpcError::internal(OperationId::new().to_string())
        }
        code => rpc::RpcError::m4_resource(code.as_str(), error.subreason()),
    };
    if diagnostic_id.is_some() {
        rpc_error.diagnostic_id = diagnostic_id;
    }
    error_action(Some(id), rpc_error, false)
}

fn hex(value: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for byte in value {
        result.push(char::from(HEX[(byte >> 4) as usize]));
        result.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removal_conflicts_survive_the_core_to_rpc_projection() {
        let change_set: app::PackageChangeSet = serde_json::from_value(serde_json::json!({
            "formatVersion":4,"mutations":[{"kind":"remove","packageId":"com.example.sdk","fromVersion":"1.0.0","toVersion":null,"source":null}],
            "dependencyEdges":[{"fromPackageId":"com.example.addon","toPackageId":"com.example.sdk","range":"^1.0.0","direct":false}],
            "vpmManifestSha256":"00".repeat(32),
            "removalConflicts":[{"packageId":"com.example.sdk","requiredBy":[{"packageId":"com.example.addon","range":"^1.0.0"}]}],
            "absentRemovalPackages":["com.example.sdk"]
        })).expect("internal frozen evidence");
        change_set
            .validate_bounds()
            .expect("valid conflict authority");
        let wire = plan_to_rpc(app::PackagePlanRecord {
            plan_id: app::PlanId::new(),
            owner: app::PrincipalId::local_owner(),
            project_id: app::ProjectId::new(),
            action: app::PlanAction::Remove,
            state: app::PlanState::Unapplied,
            project_revision: app::Revision::INITIAL,
            project_snapshot_fingerprint: [1; 32],
            change_set_fingerprint: [2; 32],
            change_set,
            source_set: Vec::new(),
            apply_operation_id: None,
            created_at_ms: 1,
        });
        let encoded = serde_json::to_value(&wire).expect("RPC response");
        assert!(encoded["changeSet"].get("absentRemovalPackages").is_none());
        assert_eq!(
            encoded["changeSet"]["removalConflicts"],
            serde_json::json!([{
                "packageId":"com.example.sdk","requiredBy":[{"packageId":"com.example.addon","range":"^1.0.0"}]
            }])
        );
        let decoded: rpc::PackagePlan =
            serde_json::from_value(encoded).expect("SDK/client response");
        assert_eq!(decoded, wire);
    }

    #[test]
    fn manual_unity_flag_defaults_to_false_and_only_enters_the_three_approved_methods() {
        let base = serde_json::json!({
            "projectId": "00000000-0000-4000-8000-000000000001",
            "expectedRevision": 1,
            "packageId": "com.example.package",
            "versionRange": "=2.0.0"
        });
        for action in [app::PlanAction::Install, app::PlanAction::Upgrade] {
            assert!(
                !plan_request(action, app::PackagePlanVersion::V2, base.clone())
                    .expect("default")
                    .allow_unity_incompatible
            );
            let mut explicit = base.clone();
            explicit["allowUnityIncompatible"] = serde_json::json!(true);
            assert!(
                plan_request(action, app::PackagePlanVersion::V2, explicit.clone())
                    .expect("explicit")
                    .allow_unity_incompatible
            );
            explicit["allowUnityIncompatible"] = serde_json::json!("true");
            assert!(plan_request(action, app::PackagePlanVersion::V2, explicit).is_err());
        }
        let mut downgrade = base.clone();
        downgrade
            .as_object_mut()
            .expect("object")
            .remove("versionRange");
        downgrade["version"] = serde_json::json!("2.0.0");
        downgrade["allowUnityIncompatible"] = serde_json::json!(true);
        let parsed = plan_request(
            app::PlanAction::Downgrade,
            app::PackagePlanVersion::V2,
            downgrade,
        )
        .expect("downgrade");
        assert!(parsed.allow_unity_incompatible);
        assert_eq!(parsed.version_range.as_deref(), Some("=2.0.0"));
        for action in [
            app::PlanAction::Remove,
            app::PlanAction::Resolve,
            app::PlanAction::Reinstall,
            app::PlanAction::Bulk,
        ] {
            let value = serde_json::json!({"projectId": "00000000-0000-4000-8000-000000000001", "expectedRevision": 1, "packageId": "com.example.package", "allowUnityIncompatible": true});
            assert!(plan_request(action, app::PackagePlanVersion::V2, value).is_err());
        }
    }
}
