use alcomd_application as app;
use alcomd_protocol as rpc;

use super::{
    AccessContext, ConnectionState, DispatchAction, IdempotencyKey, M3ReadApplication, OperationId,
    Revision, error_action, invalid, require_capability, success_action,
};

macro_rules! require {
    ($request:ident, $state:ident, $capability:expr) => {
        if let Some(action) = require_capability(&$request.id, $state, $capability) {
            return action;
        }
    };
}

macro_rules! parse {
    ($request:ident) => {
        match serde_json::from_value($request.params) {
            Ok(value) => value,
            Err(_) => return invalid($request.id),
        }
    };
}

macro_rules! key {
    ($request:ident, $value:expr) => {
        match IdempotencyKey::parse($value) {
            Ok(value) => value,
            Err(_) => return invalid($request.id),
        }
    };
}

pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    application: &M3ReadApplication,
    access: &AccessContext,
) -> DispatchAction {
    match request.method.as_str() {
        rpc::METHOD_PROJECTS_INSPECT => {
            require!(request, state, rpc::CAPABILITY_PROJECTS_READ_V1);
            let params: rpc::ProjectsInspectParams = parse!(request);
            match application
                .inspect_project(access, params.path, discovery(params.discovery_mode))
                .await
            {
                Ok(value) => success_action(
                    request.id,
                    rpc::ProjectResult {
                        project: project_observation(value),
                    },
                    None,
                ),
                Err(error) => m3_error(request.id, error),
            }
        }
        rpc::METHOD_PROJECTS_LIST => {
            require!(request, state, rpc::CAPABILITY_PROJECTS_READ_V1);
            let params: rpc::RegistryListParams = parse!(request);
            let cursor = match params.cursor.map(project_cursor).transpose() {
                Ok(value) => value,
                Err(()) => return invalid(request.id),
            };
            match application
                .list_projects(access, cursor, params.limit.unwrap_or(100))
                .await
            {
                Ok(page) => success_action(
                    request.id,
                    rpc::ProjectsListResult {
                        projects: page.projects.into_iter().map(project_record).collect(),
                        next_cursor: page.next_cursor.map(registry_cursor),
                    },
                    None,
                ),
                Err(error) => m3_error(request.id, error),
            }
        }
        rpc::METHOD_PROJECTS_GET => {
            require!(request, state, rpc::CAPABILITY_PROJECTS_READ_V1);
            let params: rpc::ProjectIdParams = parse!(request);
            let id = match app::ProjectId::parse(&params.project_id) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            match application.get_project(access, id).await {
                Ok(value) => success_action(
                    request.id,
                    rpc::ProjectResult {
                        project: project_record(value),
                    },
                    None,
                ),
                Err(error) => m3_error(request.id, error),
            }
        }
        rpc::METHOD_PROJECTS_REGISTER => {
            require!(request, state, rpc::CAPABILITY_PROJECTS_REGISTRY_V1);
            let params: rpc::ProjectRegisterParams = parse!(request);
            let key = key!(request, params.idempotency_key);
            match application.register_project(access, params.path, key).await {
                Ok(value) => success_action(
                    request.id,
                    rpc::ProjectWriteResult {
                        project: project_record(value.value),
                        replayed: value.replayed,
                    },
                    None,
                ),
                Err(error) => m3_error(request.id, error),
            }
        }
        rpc::METHOD_PROJECTS_REFRESH => {
            require!(request, state, rpc::CAPABILITY_PROJECTS_REGISTRY_V1);
            let params: rpc::ProjectMutationParams = parse!(request);
            let (id, expected, key) = match project_mutation(&params) {
                Ok(value) => value,
                Err(()) => return invalid(request.id),
            };
            match application.refresh_project(access, id, expected, key).await {
                Ok(value) => success_action(
                    request.id,
                    rpc::ProjectWriteResult {
                        project: project_record(value.value),
                        replayed: value.replayed,
                    },
                    None,
                ),
                Err(error) => m3_error(request.id, error),
            }
        }
        rpc::METHOD_PROJECTS_SET_FAVORITE => {
            require!(request, state, rpc::CAPABILITY_PROJECTS_REGISTRY_V1);
            let params: rpc::ProjectSetFavoriteParams = parse!(request);
            let parsed = app::ProjectId::parse(&params.project_id)
                .ok()
                .zip(Revision::new(params.expected_revision))
                .zip(IdempotencyKey::parse(params.idempotency_key).ok());
            let Some(((id, expected), key)) = parsed else {
                return invalid(request.id);
            };
            match application
                .set_project_favorite(access, id, params.favorite, expected, key)
                .await
            {
                Ok(value) => success_action(
                    request.id,
                    rpc::ProjectWriteResult {
                        project: project_record(value.value),
                        replayed: value.replayed,
                    },
                    None,
                ),
                Err(error) => m3_error(request.id, error),
            }
        }
        rpc::METHOD_PROJECTS_UNREGISTER => {
            require!(request, state, rpc::CAPABILITY_PROJECTS_REGISTRY_V1);
            let params: rpc::ProjectMutationParams = parse!(request);
            let (id, expected, key) = match project_mutation(&params) {
                Ok(value) => value,
                Err(()) => return invalid(request.id),
            };
            match application
                .unregister_project(access, id, expected, key)
                .await
            {
                Ok(value) => success_action(
                    request.id,
                    rpc::ProjectUnregisterResult {
                        project_id: value.id.to_string(),
                        revision: value.revision.get(),
                        unregistered: true,
                        replayed: value.replayed,
                    },
                    None,
                ),
                Err(error) => m3_error(request.id, error),
            }
        }
        rpc::METHOD_REPOSITORIES_INSPECT => {
            require!(request, state, rpc::CAPABILITY_REPOSITORIES_READ_V1);
            let params: rpc::RepositoryInspectParams = parse!(request);
            match application
                .inspect_repository(access, repository_source(params.source))
                .await
            {
                Ok(value) => success_action(
                    request.id,
                    rpc::RepositoryResult {
                        repository: repository_observation(value),
                    },
                    None,
                ),
                Err(error) => m3_error(request.id, error),
            }
        }
        rpc::METHOD_REPOSITORIES_LIST => {
            require!(request, state, rpc::CAPABILITY_REPOSITORIES_READ_V1);
            let params: rpc::RegistryListParams = parse!(request);
            let cursor = match params.cursor.map(repository_cursor).transpose() {
                Ok(value) => value,
                Err(()) => return invalid(request.id),
            };
            match application
                .list_repositories(access, cursor, params.limit.unwrap_or(100))
                .await
            {
                Ok(page) => success_action(
                    request.id,
                    rpc::RepositoriesListResult {
                        repositories: page
                            .repositories
                            .into_iter()
                            .map(repository_record)
                            .collect(),
                        next_cursor: page.next_cursor.map(registry_cursor),
                    },
                    None,
                ),
                Err(error) => m3_error(request.id, error),
            }
        }
        rpc::METHOD_REPOSITORIES_GET => {
            require!(request, state, rpc::CAPABILITY_REPOSITORIES_READ_V1);
            let params: rpc::RepositoryIdParams = parse!(request);
            let id = match app::RepositoryId::parse(&params.repository_id) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            match application.get_repository(access, id).await {
                Ok(value) => success_action(
                    request.id,
                    rpc::RepositoryResult {
                        repository: repository_record(value),
                    },
                    None,
                ),
                Err(error) => m3_error(request.id, error),
            }
        }
        rpc::METHOD_REPOSITORIES_PACKAGES => {
            require!(request, state, rpc::CAPABILITY_REPOSITORIES_READ_V1);
            let params: rpc::RepositoryPackagesParams = parse!(request);
            let id = match app::RepositoryId::parse(&params.repository_id) {
                Ok(value) => value,
                Err(_) => return invalid(request.id),
            };
            let cursor = params.cursor.map(|value| app::PackageCursor {
                package_id: value.package_id,
                version: value.version,
            });
            match application
                .list_repository_packages(access, id, cursor, params.limit.unwrap_or(100))
                .await
            {
                Ok(page) => success_action(
                    request.id,
                    rpc::RepositoryPackagesResult {
                        packages: page.packages.into_iter().map(package).collect(),
                        next_cursor: page.next_cursor.map(|value| rpc::PackageCursor {
                            package_id: value.package_id,
                            version: value.version,
                        }),
                    },
                    None,
                ),
                Err(error) => m3_error(request.id, error),
            }
        }
        rpc::METHOD_REPOSITORIES_REGISTER => {
            require!(request, state, rpc::CAPABILITY_REPOSITORIES_REGISTRY_V1);
            let params: rpc::RepositoryRegisterParams = parse!(request);
            let key = key!(request, params.idempotency_key);
            match application
                .register_repository(access, repository_source(params.source), key)
                .await
            {
                Ok(value) => success_action(
                    request.id,
                    rpc::RepositoryWriteResult {
                        repository: repository_record(value.value),
                        replayed: value.replayed,
                    },
                    None,
                ),
                Err(error) => m3_error(request.id, error),
            }
        }
        rpc::METHOD_REPOSITORIES_REFRESH => {
            require!(request, state, rpc::CAPABILITY_REPOSITORIES_REGISTRY_V1);
            let params: rpc::RepositoryMutationParams = parse!(request);
            let (id, expected, key) = match repository_mutation(&params) {
                Ok(value) => value,
                Err(()) => return invalid(request.id),
            };
            match application
                .refresh_repository(access, id, expected, key)
                .await
            {
                Ok(value) => success_action(
                    request.id,
                    rpc::RepositoryWriteResult {
                        repository: repository_record(value.value),
                        replayed: value.replayed,
                    },
                    None,
                ),
                Err(error) => m3_error(request.id, error),
            }
        }
        rpc::METHOD_REPOSITORIES_UNREGISTER => {
            require!(request, state, rpc::CAPABILITY_REPOSITORIES_REGISTRY_V1);
            let params: rpc::RepositoryMutationParams = parse!(request);
            let (id, expected, key) = match repository_mutation(&params) {
                Ok(value) => value,
                Err(()) => return invalid(request.id),
            };
            match application
                .unregister_repository(access, id, expected, key)
                .await
            {
                Ok(value) => success_action(
                    request.id,
                    rpc::RepositoryUnregisterResult {
                        repository_id: value.id.to_string(),
                        revision: value.revision.get(),
                        unregistered: true,
                        replayed: value.replayed,
                    },
                    None,
                ),
                Err(error) => m3_error(request.id, error),
            }
        }
        _ => error_action(Some(request.id), rpc::RpcError::method_not_found(), false),
    }
}

fn discovery(value: rpc::ProjectDiscoveryMode) -> app::ProjectDiscoveryMode {
    match value {
        rpc::ProjectDiscoveryMode::ExactRoot => app::ProjectDiscoveryMode::ExactRoot,
        rpc::ProjectDiscoveryMode::SearchParents => app::ProjectDiscoveryMode::SearchParents,
    }
}

fn repository_source(value: rpc::RepositorySource) -> app::RepositorySource {
    match value {
        rpc::RepositorySource::Local { path } => app::RepositorySource::Local { path },
        rpc::RepositorySource::Remote { url } => app::RepositorySource::Remote { url },
    }
}

fn project_mutation(
    value: &rpc::ProjectMutationParams,
) -> Result<(app::ProjectId, Revision, IdempotencyKey), ()> {
    Ok((
        app::ProjectId::parse(&value.project_id).map_err(|_| ())?,
        Revision::new(value.expected_revision).ok_or(())?,
        IdempotencyKey::parse(value.idempotency_key.clone()).map_err(|_| ())?,
    ))
}

fn repository_mutation(
    value: &rpc::RepositoryMutationParams,
) -> Result<(app::RepositoryId, Revision, IdempotencyKey), ()> {
    Ok((
        app::RepositoryId::parse(&value.repository_id).map_err(|_| ())?,
        Revision::new(value.expected_revision).ok_or(())?,
        IdempotencyKey::parse(value.idempotency_key.clone()).map_err(|_| ())?,
    ))
}

fn project_cursor(value: rpc::RegistryCursor) -> Result<app::RegistryCursor<app::ProjectId>, ()> {
    Ok(app::RegistryCursor {
        registered_at_ms: value.registered_at_ms,
        id: app::ProjectId::parse(&value.id).map_err(|_| ())?,
    })
}

fn repository_cursor(
    value: rpc::RegistryCursor,
) -> Result<app::RegistryCursor<app::RepositoryId>, ()> {
    Ok(app::RegistryCursor {
        registered_at_ms: value.registered_at_ms,
        id: app::RepositoryId::parse(&value.id).map_err(|_| ())?,
    })
}

fn registry_cursor<I: ToString>(value: app::RegistryCursor<I>) -> rpc::RegistryCursor {
    rpc::RegistryCursor {
        registered_at_ms: value.registered_at_ms,
        id: value.id.to_string(),
    }
}

fn project_observation(value: app::ProjectObservation) -> rpc::ProjectSnapshot {
    project_snapshot(None, None, value, None, None)
}

fn project_record(value: app::ProjectRecord) -> rpc::ProjectSnapshot {
    project_snapshot(
        Some(value.project_id.to_string()),
        Some(value.registered_at_ms),
        value.observation,
        Some(value.revision.get()),
        Some(value.favorite),
    )
}

fn project_snapshot(
    project_id: Option<String>,
    registered_at_ms: Option<u64>,
    value: app::ProjectObservation,
    revision: Option<u64>,
    favorite: Option<bool>,
) -> rpc::ProjectSnapshot {
    rpc::ProjectSnapshot {
        project_id,
        registered_at_ms,
        root_path: value.root_path,
        project_type: project_type(value.project_type),
        unity_version: value.unity_version,
        unity_revision: value.unity_revision,
        vpm_manifest: manifest(value.vpm_manifest),
        upm_manifest: manifest(value.upm_manifest),
        direct_dependencies: value
            .direct_dependencies
            .into_iter()
            .map(dependency)
            .collect(),
        locked_dependencies: value
            .locked_dependencies
            .into_iter()
            .map(dependency)
            .collect(),
        issues: value.issues.into_iter().map(issue).collect(),
        observed_at_ms: value.observed_at_ms,
        revision,
        favorite,
    }
}

fn project_type(value: app::ProjectType) -> rpc::ProjectType {
    use app::ProjectType as A;
    use rpc::ProjectType as R;
    match value {
        A::Avatars => R::Avatars,
        A::Worlds => R::Worlds,
        A::VpmStarter => R::VpmStarter,
        A::UpmAvatars => R::UpmAvatars,
        A::UpmWorlds => R::UpmWorlds,
        A::UpmStarter => R::UpmStarter,
        A::LegacySdk2 => R::LegacySdk2,
        A::LegacyWorlds => R::LegacyWorlds,
        A::LegacyAvatars => R::LegacyAvatars,
        A::Unknown => R::Unknown,
    }
}

fn manifest(value: app::ManifestState) -> rpc::ManifestState {
    match value {
        app::ManifestState::Missing => rpc::ManifestState::Missing,
        app::ManifestState::Valid => rpc::ManifestState::Valid,
    }
}

fn dependency(value: app::DependencyIdentity) -> rpc::DependencyIdentity {
    rpc::DependencyIdentity {
        package_id: value.package_id,
        value: value.value,
    }
}

fn issue(value: app::ReadIssue) -> rpc::ReadIssue {
    rpc::ReadIssue {
        code: value.code,
        component: value.component,
        item: value.item,
        line: value.line,
        column: value.column,
    }
}

fn repository_observation(value: app::RepositoryObservation) -> rpc::RepositorySnapshot {
    repository_snapshot(None, value, None)
}

fn repository_record(value: app::RepositoryRecord) -> rpc::RepositorySnapshot {
    repository_snapshot(
        Some(value.repository_id.to_string()),
        value.observation,
        Some(value.revision.get()),
    )
}

fn repository_snapshot(
    repository_id: Option<String>,
    value: app::RepositoryObservation,
    revision: Option<u64>,
) -> rpc::RepositorySnapshot {
    rpc::RepositorySnapshot {
        repository_id,
        source: match value.source {
            app::RepositorySource::Local { path } => rpc::RepositorySource::Local { path },
            app::RepositorySource::Remote { url } => rpc::RepositorySource::Remote { url },
        },
        declared_id: value.declared_id,
        name: value.name,
        declared_url: value.declared_url,
        issues: value.issues.into_iter().map(issue).collect(),
        revision,
        refreshed_at_ms: value.refreshed_at_ms,
    }
}

fn package(value: app::RepositoryPackageVersion) -> rpc::RepositoryPackageVersion {
    rpc::RepositoryPackageVersion {
        package_id: value.package_id,
        version: value.version,
        display_name: value.display_name,
        description: value.description,
        yanked: value.yanked,
        unity: value.unity,
    }
}

fn m3_error(id: String, error: app::M3Error) -> DispatchAction {
    use app::M3ErrorCode as C;
    let rpc_error = match error.code() {
        C::PermissionDenied => rpc::RpcError::permission_denied(),
        C::RevisionConflict => rpc::RpcError::revision_conflict(),
        C::IdempotencyConflict => rpc::RpcError::idempotency_conflict(),
        C::StoreUnavailable => rpc::RpcError::store_unavailable(),
        C::Internal => rpc::RpcError::internal(OperationId::new().to_string()),
        code => rpc::RpcError::m3_resource(m3_code(code)),
    };
    error_action(Some(id), rpc_error, false)
}

fn m3_code(code: app::M3ErrorCode) -> &'static str {
    use app::M3ErrorCode as C;
    match code {
        C::PathEncodingUnsupported => rpc::error_code::PATH_ENCODING_UNSUPPORTED,
        C::ProjectNotFound => rpc::error_code::PROJECT_NOT_FOUND,
        C::ProjectNotRegistered => rpc::error_code::PROJECT_NOT_REGISTERED,
        C::ProjectAlreadyRegistered => rpc::error_code::PROJECT_ALREADY_REGISTERED,
        C::ProjectInaccessible => rpc::error_code::PROJECT_INACCESSIBLE,
        C::ProjectVersionMissing => rpc::error_code::PROJECT_VERSION_MISSING,
        C::ProjectVersionInvalid => rpc::error_code::PROJECT_VERSION_INVALID,
        C::ProjectManifestInvalid => rpc::error_code::PROJECT_MANIFEST_INVALID,
        C::RepositoryNotFound => rpc::error_code::REPOSITORY_NOT_FOUND,
        C::RepositoryNotRegistered => rpc::error_code::REPOSITORY_NOT_REGISTERED,
        C::RepositoryAlreadyRegistered => rpc::error_code::REPOSITORY_ALREADY_REGISTERED,
        C::RepositorySourceInvalid => rpc::error_code::REPOSITORY_SOURCE_INVALID,
        C::RepositoryInaccessible => rpc::error_code::REPOSITORY_INACCESSIBLE,
        C::RepositoryUnavailable => rpc::error_code::REPOSITORY_UNAVAILABLE,
        C::RepositoryDocumentInvalid => rpc::error_code::REPOSITORY_DOCUMENT_INVALID,
        C::RepositoryDocumentTooLarge => rpc::error_code::REPOSITORY_DOCUMENT_TOO_LARGE,
        C::RepositoryCredentialsUnsupported => rpc::error_code::REPOSITORY_CREDENTIALS_UNSUPPORTED,
        C::RevisionConflict
        | C::IdempotencyConflict
        | C::PermissionDenied
        | C::StoreUnavailable
        | C::Internal => rpc::error_code::INTERNAL_ERROR,
    }
}
