use alcomd_application as app;
use alcomd_application::{
    IdempotencyKey, M3Error, M3ErrorCode, PackageCursor, PackagePage, PrincipalId, ProjectId,
    ProjectObservation, ProjectPage, ProjectRecord, RegistryCursor, RepositoryId,
    RepositoryObservation, RepositoryPackageVersion, RepositoryPage, RepositoryRecord,
    RepositorySource, RepositoryValidators, ResolverPackageMetadata, Revision, SyncWrite,
    UnregisterResult, VRCHAT_CURATED_REPOSITORY_ID, VRCHAT_OFFICIAL_REPOSITORY_ID,
};
use alcomd_application::{ProjectListSort, ProjectListView, ProjectViewPage, project_display_name};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, TransactionBehavior, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::operation_logs::{CommittedStep, committed};

fn project_target(id: ProjectId, path: &str) -> alcomd_application::LogTarget {
    let mut target = alcomd_application::log_path("project", path.to_owned());
    target.id = Some(id.to_string());
    target
}

fn repository_target(
    id: RepositoryId,
    value: &RepositoryObservation,
) -> alcomd_application::LogTarget {
    alcomd_application::LogTarget {
        kind: "repository".to_owned(),
        id: Some(id.to_string()),
        name: value.name.clone(),
        path: match &value.source {
            RepositorySource::Local { path } => Some(path.clone()),
            RepositorySource::Remote { .. } => None,
        },
    }
}

const PROJECT_REGISTER: &str = "projects.register";
const MCP_PROJECT_REGISTER: &str = "mcp.projects.register";
const PROJECT_REFRESH: &str = "projects.refresh";
const PROJECT_SET_FAVORITE: &str = "projects.setFavorite";
const PROJECT_UNREGISTER: &str = "projects.unregister";
const REPOSITORY_REGISTER: &str = "repositories.register";
const REPOSITORY_REFRESH: &str = "repositories.refresh";
const REPOSITORY_UNREGISTER: &str = "repositories.unregister";
const REPOSITORY_SET_CREDENTIALS: &str = "repositories.setCredentials";
const REPOSITORY_MOVE: &str = "repositories.move";
const REPOSITORY_SET_DISPLAY_NAME: &str = "repositories.setDisplayName";

struct RepositoryWriteScope<'a> {
    owner: &'a PrincipalId,
    authority: Option<&'a app::McpRepositoryAuthority>,
    command: Option<app::McpRepositoryCommand>,
}

impl<'a> RepositoryWriteScope<'a> {
    fn caller(&self) -> &PrincipalId {
        self.authority
            .map_or(self.owner, |authority| &authority.caller)
    }

    fn require(&self, connection: &Connection) -> Result<(), M3Error> {
        self.require_permission(connection, app::Permission::RepositoriesManage)
    }

    fn require_permission(
        &self,
        connection: &Connection,
        permission: app::Permission,
    ) -> Result<(), M3Error> {
        if let Some(authority) = self.authority {
            let denied = || M3Error::new(M3ErrorCode::PermissionDenied);
            let client = super::m8_tools::authenticate(connection, &authority.authentication)
                .map_err(|_| denied())?;
            if client.principal_id != authority.caller {
                return Err(denied());
            }
            authority
                .authentication
                .access(client)
                .map_err(|_| denied())?
                .require(permission)
                .map_err(|_| denied())?;
        }
        Ok(())
    }

    fn identity(
        &self,
        owner_method: &'static str,
        owner_fingerprint: impl FnOnce() -> Result<String, M3Error>,
    ) -> Result<(&'static str, String), M3Error> {
        self.command.map_or_else(
            || Ok((owner_method, owner_fingerprint()?)),
            repository_command_identity,
        )
    }
}

pub(super) struct RepositoryAccess {
    owner: PrincipalId,
    authority: Option<app::McpRepositoryAuthority>,
}

impl RepositoryAccess {
    // Capture the authenticated credential on the calling task before handing work to SQLite.
    pub(super) fn new(caller: PrincipalId) -> Result<Self, M3Error> {
        let owner = app::workflow_resource_owner(&caller);
        let authority = if owner == caller {
            None
        } else {
            Some(app::McpRepositoryAuthority {
                caller,
                authentication: app::workflow_authentication()
                    .ok_or_else(|| M3Error::new(M3ErrorCode::PermissionDenied))?,
            })
        };
        Ok(Self { owner, authority })
    }

    fn scope(&self) -> RepositoryWriteScope<'_> {
        RepositoryWriteScope {
            owner: &self.owner,
            authority: self.authority.as_ref(),
            command: None,
        }
    }

    pub(super) fn read<T>(
        &self,
        connection: &mut Connection,
        read: impl FnOnce(&Connection, &PrincipalId) -> Result<T, M3Error>,
    ) -> Result<T, M3Error> {
        let transaction = connection.transaction().map_err(sql_failure)?;
        self.scope()
            .require_permission(&transaction, app::Permission::RepositoriesRead)?;
        let result = read(&transaction, &self.owner)?;
        transaction.commit().map_err(sql_failure)?;
        Ok(result)
    }
}

fn registration_fingerprint(
    source: &RepositorySource,
    authenticated: bool,
) -> Result<String, M3Error> {
    // Keep the original locator bound to the request without persisting it or a write-only header.
    let source = app::digest(json(source)?.as_bytes());
    json(&serde_json::json!({ "source": source, "authenticated": authenticated, "version": 1 }))
}

fn credential_fingerprint(
    id: RepositoryId,
    expected: Revision,
    authenticated: bool,
) -> Result<String, M3Error> {
    json(&serde_json::json!({
        "repositoryId": id.to_string(), "expectedRevision": expected.get(),
        "authenticated": authenticated, "version": 1
    }))
}

pub(super) fn replay_repository_write(
    connection: &mut Connection,
    access: &RepositoryAccess,
    request: app::RepositoryWriteRequest,
    key: &IdempotencyKey,
) -> Result<Option<SyncWrite<RepositoryRecord>>, M3Error> {
    let tx = connection.transaction().map_err(sql_failure)?;
    let scope = access.scope();
    scope.require(&tx)?;
    let (method, fingerprint) = match request {
        app::RepositoryWriteRequest::Register {
            source,
            authenticated,
        } => (
            REPOSITORY_REGISTER,
            registration_fingerprint(&source, authenticated)?,
        ),
        app::RepositoryWriteRequest::SetCredentials {
            id,
            expected,
            authenticated,
        } => (
            REPOSITORY_SET_CREDENTIALS,
            credential_fingerprint(id, expected, authenticated)?,
        ),
    };
    let mut result = existing_response::<SyncWrite<RepositoryRecord>>(
        &tx,
        scope.caller(),
        method,
        key,
        &fingerprint,
    )?;
    if let Some(receipt) = &mut result {
        receipt.replayed = true;
    }
    tx.commit().map_err(sql_failure)?;
    Ok(result)
}

pub(super) fn replay_repository_refresh(
    connection: &Connection,
    access: &RepositoryAccess,
    id: RepositoryId,
    expected: Revision,
    key: &IdempotencyKey,
) -> Result<Option<SyncWrite<app::RepositoryRefreshReceipt>>, M3Error> {
    let scope = access.scope();
    let transaction = connection.unchecked_transaction().map_err(sql_failure)?;
    scope.require(&transaction)?;
    let mut result = existing_response::<SyncWrite<app::RepositoryRefreshReceipt>>(
        &transaction,
        scope.caller(),
        REPOSITORY_REFRESH,
        key,
        &aggregate_fingerprint(id, expected)?,
    )?;
    if let Some(receipt) = &mut result {
        receipt.replayed = true;
    }
    transaction.commit().map_err(sql_failure)?;
    Ok(result)
}

fn repository_command_identity(
    command: app::McpRepositoryCommand,
) -> Result<(&'static str, String), M3Error> {
    use app::McpRepositoryCommand::*;
    Ok(match command {
        Register { preview_id } => (
            "mcp.repositories.register",
            json(&serde_json::json!({"previewId":preview_id,"version":1}))?,
        ),
        Remove { id, expected } => (
            "mcp.repositories.remove",
            aggregate_fingerprint(id, expected)?,
        ),
    })
}

pub(super) fn mcp_repository_replay(
    connection: &mut Connection,
    authority: &app::McpRepositoryAuthority,
    command: app::McpRepositoryCommand,
    key: &IdempotencyKey,
) -> Result<Option<app::McpRepositoryReceipt>, M3Error> {
    let owner = PrincipalId::local_owner();
    let scope = RepositoryWriteScope {
        owner: &owner,
        authority: Some(authority),
        command: Some(command),
    };
    let tx = connection.transaction().map_err(sql_failure)?;
    scope.require(&tx)?;
    let (method, fingerprint) = repository_command_identity(command)?;
    let receipt = match command {
        app::McpRepositoryCommand::Remove { .. } => {
            existing_response::<UnregisterResult<RepositoryId>>(
                &tx,
                scope.caller(),
                method,
                key,
                &fingerprint,
            )?
            .map(|mut value| {
                value.replayed = true;
                removed_repository_receipt(value)
            })
        }
        _ => existing_response::<SyncWrite<app::McpRepositoryEntry>>(
            &tx,
            scope.caller(),
            method,
            key,
            &fingerprint,
        )?
        .map(|mut value| {
            value.replayed = true;
            repository_receipt(value)
        }),
    };
    tx.commit().map_err(sql_failure)?;
    Ok(receipt)
}

pub(super) fn mcp_repository_commit(
    connection: &mut Connection,
    authority: &app::McpRepositoryAuthority,
    command: app::McpRepositoryCommand,
    observation: Option<app::RepositoryReadOutcome>,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<app::McpRepositoryReceipt, M3Error> {
    use app::{McpRepositoryCommand::*, RepositoryReadOutcome::*};
    let owner = PrincipalId::local_owner();
    let scope = RepositoryWriteScope {
        owner: &owner,
        authority: Some(authority),
        command: Some(command),
    };
    match (command, observation) {
        (Register { .. }, Some(Fresh(observation))) => {
            register_repository_projected(connection, scope, observation.into(), None, key, now_ms)
                .map(repository_receipt)
        }
        (Remove { id, expected }, None) if app::builtin_repository(id).is_none() => {
            unregister_repository_scoped(connection, scope, id, expected, key, now_ms)
                .map(removed_repository_receipt)
        }
        _ => Err(M3Error::new(M3ErrorCode::PermissionDenied)),
    }
}

fn repository_receipt(value: SyncWrite<app::McpRepositoryEntry>) -> app::McpRepositoryReceipt {
    app::McpRepositoryReceipt {
        repository_id: value.value.repository_id,
        revision: value.value.revision,
        repository: Some(value.value),
        replayed: value.replayed,
    }
}

fn removed_repository_receipt(value: UnregisterResult<RepositoryId>) -> app::McpRepositoryReceipt {
    app::McpRepositoryReceipt {
        repository_id: value.id,
        revision: value.revision,
        repository: None,
        replayed: value.replayed,
    }
}

pub(super) fn repository_list_snapshot(
    connection: &Connection,
    owner: &PrincipalId,
) -> Result<alcomd_application::RepositoryListSnapshot, M3Error> {
    let mut statement = connection
        .prepare(&ordered_repository_query(
            "SELECT r.repository_id, r.revision, r.source_kind, r.source_locator, r.credential_reference, r.display_name, r.priority
             FROM repository_order o JOIN repositories r USING(repository_id) ORDER BY o.list_rank",
        ))
        .map_err(sql_failure)?;
    let rows = statement
        .query_map([owner.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, i64>(6)?,
            ))
        })
        .map_err(sql_failure)?;
    let mut repositories = Vec::new();
    for row in rows {
        let (repository_id, rev, kind, locator, credential_reference, display_name, priority) =
            row.map_err(sql_failure)?;
        let source = match kind.as_str() {
            "local" => RepositorySource::Local { path: locator },
            "remote" => RepositorySource::Remote { url: locator },
            _ => return Err(internal()),
        };
        repositories.push(alcomd_application::RepositoryListRecord {
            repository_id,
            revision: revision(rev)?.get(),
            order_key: u64::try_from(priority).map_err(|_| internal())?,
            source,
            credential_reference,
            configuration: alcomd_application::RepositoryConfiguration {
                display_name,
                enabled: true,
                disabled_package_ids: Vec::new(),
            },
        });
        // One overflow item lets the application reject instead of silently truncating.
        if repositories.len() > alcomd_application::REPOSITORY_LIST_MAX_ENTRIES {
            break;
        }
    }
    drop(statement);
    let mut statement = connection
        .prepare("SELECT root_path FROM projects ORDER BY project_id")
        .map_err(sql_failure)?;
    let project_roots = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(sql_failure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_failure)?;
    Ok(alcomd_application::RepositoryListSnapshot {
        repositories,
        project_roots,
    })
}

pub(super) fn unavailable() -> M3Error {
    M3Error::new(M3ErrorCode::StoreUnavailable)
}

fn sql_failure(source: impl std::error::Error + Send + Sync + 'static) -> M3Error {
    unavailable().with_source(source)
}

fn internal() -> M3Error {
    M3Error::new(M3ErrorCode::Internal)
}

fn integer(value: u64) -> Result<i64, M3Error> {
    i64::try_from(value).map_err(|_| internal())
}

fn revision(value: i64) -> Result<Revision, M3Error> {
    u64::try_from(value)
        .ok()
        .and_then(Revision::new)
        .ok_or_else(internal)
}

fn json<T: Serialize>(value: &T) -> Result<String, M3Error> {
    serde_json::to_string(value).map_err(|_| internal())
}

fn parse_json<T: DeserializeOwned>(value: &str) -> Result<T, M3Error> {
    serde_json::from_str(value).map_err(|_| internal())
}

fn begin(connection: &mut Connection) -> Result<Transaction<'_>, M3Error> {
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(sql_failure)
}

fn existing_response<T: DeserializeOwned>(
    transaction: &Connection,
    owner: &PrincipalId,
    method: &str,
    key: &IdempotencyKey,
    fingerprint: &str,
) -> Result<Option<T>, M3Error> {
    let row = transaction
        .query_row(
            "SELECT request_fingerprint, response_json FROM idempotency_records
             WHERE principal_id=?1 AND method=?2 AND idempotency_key=?3",
            params![owner.as_str(), method, key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(sql_failure)?;
    match row {
        Some((stored, _)) if stored != fingerprint => {
            Err(M3Error::new(M3ErrorCode::IdempotencyConflict))
        }
        Some((_, response)) => parse_json(&response).map(Some),
        None => Ok(None),
    }
}

fn save_response<T: Serialize>(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    method: &str,
    key: &IdempotencyKey,
    fingerprint: &str,
    response: &T,
    now_ms: u64,
) -> Result<(), M3Error> {
    transaction
        .execute(
            "INSERT INTO idempotency_records (
                principal_id, method, idempotency_key, request_fingerprint, state,
                operation_id, response_json, created_at_ms
             ) VALUES (?1, ?2, ?3, ?4, 'completed', NULL, ?5, ?6)",
            params![
                owner.as_str(),
                method,
                key.as_str(),
                fingerprint,
                json(response)?,
                integer(now_ms)?
            ],
        )
        .map_err(sql_failure)?;
    Ok(())
}

fn insert_event(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    aggregate_kind: &str,
    aggregate_id: &str,
    aggregate_revision: Revision,
    kind: &str,
    now_ms: u64,
) -> Result<(), M3Error> {
    transaction
        .execute(
            "INSERT INTO events (
                event_id, kind, aggregate_kind, aggregate_id, aggregate_revision,
                principal_id, occurred_at_ms, payload_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '{}')",
            params![
                Uuid::new_v4().to_string(),
                kind,
                aggregate_kind,
                aggregate_id,
                integer(aggregate_revision.get())?,
                owner.as_str(),
                integer(now_ms)?
            ],
        )
        .map_err(sql_failure)?;
    Ok(())
}

fn project_fingerprint(observation: &ProjectObservation) -> Result<String, M3Error> {
    json(&serde_json::json!({
        "pathIdentityKey": observation.path_identity_key,
        "rootPath": observation.root_path,
        "version": 1
    }))
}

fn aggregate_fingerprint<I: ToString>(id: I, expected: Revision) -> Result<String, M3Error> {
    json(&serde_json::json!({
        "expectedRevision": expected.get(),
        "id": id.to_string(),
        "version": 1
    }))
}

pub(super) fn project_type(value: alcomd_application::ProjectType) -> &'static str {
    use alcomd_application::ProjectType;
    match value {
        ProjectType::Avatars => "avatars",
        ProjectType::Worlds => "worlds",
        ProjectType::VpmStarter => "vpm-starter",
        ProjectType::UpmAvatars => "upm-avatars",
        ProjectType::UpmWorlds => "upm-worlds",
        ProjectType::UpmStarter => "upm-starter",
        ProjectType::LegacySdk2 => "legacy-sdk2",
        ProjectType::LegacyWorlds => "legacy-worlds",
        ProjectType::LegacyAvatars => "legacy-avatars",
        ProjectType::Unknown => "unknown",
    }
}

fn load_project(
    connection: &Connection,
    owner: &PrincipalId,
    id: ProjectId,
) -> Result<ProjectRecord, M3Error> {
    let row = connection
        .query_row(
            "SELECT snapshot_json, revision, registered_at_ms, favorite, last_modified_at_ms
             FROM projects WHERE owner_principal_id=?1 AND project_id=?2",
            params![owner.as_str(), id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                ))
            },
        )
        .optional()
        .map_err(sql_failure)?
        .ok_or_else(|| M3Error::new(M3ErrorCode::ProjectNotRegistered))?;
    let observation: ProjectObservation = parse_json(&row.0)?;
    Ok(ProjectRecord {
        project_id: id,
        observation,
        revision: revision(row.1)?,
        registered_at_ms: u64::try_from(row.2).map_err(|_| internal())?,
        last_modified_at_ms: row
            .4
            .map(u64::try_from)
            .transpose()
            .map_err(|_| internal())?,
        favorite: row.3 != 0,
    })
}

pub(super) fn register_project(
    connection: &mut Connection,
    caller: &PrincipalId,
    observation: ProjectObservation,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<SyncWrite<ProjectRecord>, M3Error> {
    let fingerprint = project_fingerprint(&observation)?;
    let transaction = begin(connection)?;
    let owner = project_resource_owner(&transaction, caller, app::Permission::ProjectsManage)?;
    if let Some(mut response) = existing_response::<SyncWrite<ProjectRecord>>(
        &transaction,
        caller,
        PROJECT_REGISTER,
        key,
        &fingerprint,
    )? {
        response.replayed = true;
        transaction.commit().map_err(sql_failure)?;
        return Ok(response);
    }
    let response = SyncWrite {
        value: insert_project(
            &transaction,
            &owner,
            caller,
            observation,
            now_ms,
            Some(now_ms),
        )?,
        replayed: false,
    };
    save_response(
        &transaction,
        caller,
        PROJECT_REGISTER,
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(sql_failure)?;
    Ok(response)
}

pub(super) fn replay_project_registration(
    db: &Connection,
    caller: &PrincipalId,
    fingerprint: &str,
    key: &IdempotencyKey,
) -> Result<Option<SyncWrite<app::McpRegisteredProject>>, M3Error> {
    super::m8_workflows::require(db, caller, &[app::Permission::ProjectsManage])
        .map_err(|cause| M3Error::new(M3ErrorCode::PermissionDenied).with_source(cause))?;
    existing_response::<SyncWrite<app::McpRegisteredProject>>(
        db,
        caller,
        MCP_PROJECT_REGISTER,
        key,
        fingerprint,
    )
    .map(|existing| {
        existing.map(|mut value| {
            value.replayed = true;
            value
        })
    })
}

pub(super) fn register_mcp_project(
    db: &mut Connection,
    caller: &PrincipalId,
    observation: ProjectObservation,
    fingerprint: &str,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<SyncWrite<app::McpRegisteredProject>, M3Error> {
    let transaction = begin(db)?;
    if let Some(existing) = replay_project_registration(&transaction, caller, fingerprint, key)? {
        return Ok(existing);
    }
    let owner = app::workflow_resource_owner(caller);
    let record = insert_project(
        &transaction,
        &owner,
        caller,
        observation,
        now_ms,
        Some(now_ms),
    )?;
    let response = SyncWrite {
        value: app::McpRegisteredProject {
            project_id: record.project_id,
            revision: record.revision,
            display_name: project_display_name(&record.observation.root_path).map(str::to_owned),
            project_type: record.observation.project_type,
            unity_version: record.observation.unity_version,
        },
        replayed: false,
    };
    save_response(
        &transaction,
        caller,
        MCP_PROJECT_REGISTER,
        key,
        fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(sql_failure)?;
    Ok(response)
}

impl super::StateStoreHandle {
    /// Registers a project in the offline import stage with its recorded activity time.
    pub async fn import_project(
        &self,
        observation: ProjectObservation,
        registered_at_ms: u64,
        last_modified_at_ms: Option<u64>,
    ) -> Result<ProjectRecord, M3Error> {
        self.request_m3(move |connection| {
            let owner = PrincipalId::local_owner();
            let transaction = begin(connection)?;
            let record = insert_project(
                &transaction,
                &owner,
                &owner,
                observation,
                registered_at_ms,
                last_modified_at_ms,
            )?;
            transaction.commit().map_err(sql_failure)?;
            Ok(record)
        })
        .await
    }
}

fn insert_project(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    caller: &PrincipalId,
    observation: ProjectObservation,
    now_ms: u64,
    last_modified_at_ms: Option<u64>,
) -> Result<ProjectRecord, M3Error> {
    let duplicate = transaction
        .query_row(
            "SELECT 1 FROM projects WHERE path_identity_key=?1",
            params![observation.path_identity_key],
            |_| Ok(()),
        )
        .optional()
        .map_err(sql_failure)?
        .is_some();
    if duplicate {
        return Err(M3Error::new(M3ErrorCode::ProjectAlreadyRegistered));
    }
    if super::m5_backup_restore::target_reserved(
        transaction,
        std::path::Path::new(&observation.root_path),
    )
    .map_err(|cause| M3Error::new(M3ErrorCode::StoreUnavailable).with_source(cause))?
    {
        return Err(M3Error::new(M3ErrorCode::ProjectInaccessible)
            .with_reason("project destination belongs to an unfinished restore"));
    }
    let id = ProjectId::new();
    transaction
        .execute(
            "INSERT INTO projects (
                project_id, owner_principal_id, root_path, path_identity_key, project_type,
                unity_version, unity_revision, snapshot_json, revision, registered_at_ms,
                updated_at_ms, last_modified_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?9, ?10)",
            params![
                id.to_string(),
                owner.as_str(),
                observation.root_path,
                observation.path_identity_key,
                project_type(observation.project_type),
                observation.unity_version,
                observation.unity_revision,
                json(&observation)?,
                integer(now_ms)?,
                last_modified_at_ms.map(integer).transpose()?
            ],
        )
        .map_err(sql_failure)?;
    let record = ProjectRecord {
        project_id: id,
        observation,
        revision: Revision::INITIAL,
        registered_at_ms: now_ms,
        last_modified_at_ms,
        favorite: false,
    };

    insert_event(
        transaction,
        owner,
        "project",
        &id.to_string(),
        Revision::INITIAL,
        "project.registered",
        now_ms,
    )?;
    committed(
        transaction,
        caller,
        now_ms,
        CommittedStep::succeeded(
            PROJECT_REGISTER,
            "registered",
            Some(project_target(id, &record.observation.root_path)),
        ),
    )
    .map_err(sql_failure)?;
    Ok(record)
}

pub(super) fn project_resource_owner(
    db: &Connection,
    caller: &PrincipalId,
    permission: app::Permission,
) -> Result<PrincipalId, M3Error> {
    super::m8_workflows::require(db, caller, &[permission])
        .map_err(|cause| M3Error::new(M3ErrorCode::PermissionDenied).with_source(cause))?;
    Ok(app::workflow_resource_owner(caller))
}

pub(super) fn get_project_for_caller(
    db: &mut Connection,
    caller: &PrincipalId,
    id: ProjectId,
) -> Result<ProjectRecord, M3Error> {
    let transaction = db.transaction().map_err(sql_failure)?;
    let owner = project_resource_owner(&transaction, caller, app::Permission::ProjectsRead)?;
    let project = load_project(&transaction, &owner, id)?;
    transaction.commit().map_err(sql_failure)?;
    Ok(project)
}

pub(super) fn get_project(
    connection: &Connection,
    owner: &PrincipalId,
    id: ProjectId,
) -> Result<ProjectRecord, M3Error> {
    load_project(connection, owner, id)
}

pub(super) fn list_projects(
    connection: &Connection,
    owner: &PrincipalId,
    cursor: Option<RegistryCursor<ProjectId>>,
    limit: u32,
) -> Result<ProjectPage, M3Error> {
    let mut statement = connection
        .prepare(
            "SELECT project_id FROM projects
             WHERE owner_principal_id=?1 AND
               (?2 IS NULL OR registered_at_ms < ?2 OR (registered_at_ms = ?2 AND project_id < ?3))
             ORDER BY registered_at_ms DESC, project_id DESC LIMIT ?4",
        )
        .map_err(sql_failure)?;
    let cursor_time = cursor
        .as_ref()
        .map(|value| integer(value.registered_at_ms))
        .transpose()?;
    let cursor_id = cursor.as_ref().map(|value| value.id.to_string());
    let rows = statement
        .query_map(
            params![owner.as_str(), cursor_time, cursor_id, i64::from(limit) + 1],
            |row| row.get::<_, String>(0),
        )
        .map_err(sql_failure)?;
    let mut projects = Vec::new();
    for row in rows {
        let id = ProjectId::parse(&row.map_err(sql_failure)?).map_err(|_| internal())?;
        projects.push(load_project(connection, owner, id)?);
    }
    let has_more = projects.len() > limit as usize;
    if has_more {
        projects.pop();
    }
    let next_cursor = has_more
        .then(|| {
            projects.last().map(|record| RegistryCursor {
                registered_at_ms: record.registered_at_ms,
                id: record.project_id,
                authorization_binding: None,
            })
        })
        .flatten();
    Ok(ProjectPage {
        projects,
        next_cursor,
        directory_states: Default::default(),
    })
}

struct ProjectListRow {
    id: String,
    name_key: String,
    snapshot_sort_key: String,
    registered_at_ms: i64,
    last_modified_at_ms: Option<i64>,
    favorite: bool,
}

pub(super) fn list_project_view(
    connection: &mut Connection,
    access: &app::AccessContext,
    authentication: Option<&app::ToolAuthentication>,
    cursor: Option<RegistryCursor<ProjectId>>,
    limit: u32,
    view: &ProjectListView,
    expected_sequence: Option<u64>,
) -> Result<ProjectViewPage, M3Error> {
    let transaction = connection.transaction().map_err(sql_failure)?;
    let owner = &app::workflow_resource_owner(access.principal());
    if access.is_mcp_client() {
        let denied = || M3Error::new(M3ErrorCode::PermissionDenied);
        let client =
            super::m8_tools::authenticate(&transaction, authentication.ok_or_else(denied)?)
                .map_err(|_| denied())?;
        if &client.principal_id != access.principal() {
            return Err(denied());
        }
    }
    super::m8_mcp::validate_project_cursor(
        &transaction,
        access.principal(),
        cursor.as_ref(),
        access.oauth_guard(),
    )?;
    let sequence: i64 = transaction
        .query_row(
            "SELECT COALESCE(MAX(sequence),0) FROM events
             WHERE principal_id=?1 AND aggregate_kind='project'",
            [owner.as_str()],
            |row| row.get(0),
        )
        .map_err(sql_failure)?;
    let sequence = u64::try_from(sequence).map_err(|_| internal())?;
    if expected_sequence.is_some_and(|expected| expected != sequence) {
        return Err(M3Error::new(M3ErrorCode::RevisionConflict));
    }

    let search = view
        .search
        .as_deref()
        .filter(|text| !text.is_empty())
        .map(str::to_lowercase);
    let read_name = search.is_some() || view.sort == ProjectListSort::Name;
    let snapshot_sort_field = match view.sort {
        ProjectListSort::Type => Some("$.project_type"),
        ProjectListSort::Unity => Some("$.unity_version"),
        _ => None,
    };
    let mut statement = transaction
        .prepare(
            "SELECT project_id,CASE WHEN ?2 THEN root_path ELSE '' END,
                    CASE WHEN ?3 IS NOT NULL THEN json_extract(snapshot_json,?3) ELSE '' END,
                    registered_at_ms,last_modified_at_ms,favorite
             FROM projects WHERE owner_principal_id=?1",
        )
        .map_err(sql_failure)?;
    let mut rows = Vec::new();
    let entries = statement
        .query_map(
            params![owner.as_str(), read_name, snapshot_sort_field],
            |row| {
                let path = row.get::<_, String>(1)?;
                Ok(ProjectListRow {
                    id: row.get(0)?,
                    name_key: project_display_name(&path)
                        .unwrap_or_default()
                        .to_lowercase(),
                    snapshot_sort_key: row.get(2)?,
                    registered_at_ms: row.get(3)?,
                    last_modified_at_ms: row.get(4)?,
                    favorite: row.get(5)?,
                })
            },
        )
        .map_err(sql_failure)?;
    for entry in entries {
        let entry = entry.map_err(sql_failure)?;
        if search
            .as_ref()
            .is_none_or(|search| entry.name_key.contains(search))
        {
            rows.push(entry);
        }
    }
    drop(statement);
    rows.sort_unstable_by(|left, right| {
        right.favorite.cmp(&left.favorite).then_with(|| {
            if view.sort == ProjectListSort::Modified {
                let missing = left
                    .last_modified_at_ms
                    .is_none()
                    .cmp(&right.last_modified_at_ms.is_none());
                if !missing.is_eq() {
                    return missing;
                }
            }
            let comparison = match view.sort {
                ProjectListSort::Name => left.name_key.cmp(&right.name_key),
                ProjectListSort::Type | ProjectListSort::Unity => {
                    left.snapshot_sort_key.cmp(&right.snapshot_sort_key)
                }
                ProjectListSort::Added => left.registered_at_ms.cmp(&right.registered_at_ms),
                ProjectListSort::Modified => {
                    left.last_modified_at_ms.cmp(&right.last_modified_at_ms)
                }
            };
            (if view.descending {
                comparison.reverse()
            } else {
                comparison
            })
            .then_with(|| left.id.cmp(&right.id))
        })
    });

    let offset = if let Some(cursor) = cursor {
        let id = cursor.id.to_string();
        let time = integer(cursor.registered_at_ms)?;
        rows.iter()
            .position(|row| row.id == id && row.registered_at_ms == time)
            .ok_or_else(|| M3Error::new(M3ErrorCode::RevisionConflict))?
            + 1
    } else {
        0
    };
    let mut projects = Vec::new();
    for row in rows.iter().skip(offset).take(limit as usize) {
        let id = ProjectId::parse(&row.id).map_err(|_| internal())?;
        projects.push(load_project(&transaction, owner, id)?);
    }
    let mut next_cursor = if rows.len() > offset + projects.len() {
        projects.last().map(|record| RegistryCursor {
            registered_at_ms: record.registered_at_ms,
            id: record.project_id,
            authorization_binding: None,
        })
    } else {
        None
    };
    super::m8_mcp::bind_project_cursor(&transaction, access.oauth_guard(), next_cursor.as_mut())?;
    Ok(ProjectViewPage {
        page: ProjectPage {
            projects,
            next_cursor,
            directory_states: Default::default(),
        },
        sequence,
    })
}

pub(super) fn refresh_project(
    connection: &mut Connection,
    caller: &PrincipalId,
    id: ProjectId,
    expected: Revision,
    observation: ProjectObservation,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<SyncWrite<ProjectRecord>, M3Error> {
    let fingerprint = aggregate_fingerprint(id, expected)?;
    let transaction = begin(connection)?;
    let owner = project_resource_owner(&transaction, caller, app::Permission::ProjectsManage)?;
    if let Some(mut response) = existing_response::<SyncWrite<ProjectRecord>>(
        &transaction,
        caller,
        PROJECT_REFRESH,
        key,
        &fingerprint,
    )? {
        response.replayed = true;
        transaction.commit().map_err(sql_failure)?;
        return Ok(response);
    }
    let current = load_project(&transaction, &owner, id)?;
    if current.revision != expected {
        return Err(M3Error::new(M3ErrorCode::RevisionConflict));
    }
    if current.observation.path_identity_key != observation.path_identity_key {
        return Err(M3Error::new(M3ErrorCode::ProjectNotFound));
    }
    let changed = current.observation != observation;
    let next_revision = if changed {
        current.revision.checked_next().ok_or_else(internal)?
    } else {
        current.revision
    };
    transaction
        .execute(
            "UPDATE projects SET root_path=?1, project_type=?2, unity_version=?3,
                unity_revision=?4, snapshot_json=?5, revision=?6,
                updated_at_ms=?7 WHERE owner_principal_id=?8 AND project_id=?9",
            params![
                observation.root_path,
                project_type(observation.project_type),
                observation.unity_version,
                observation.unity_revision,
                json(&observation)?,
                integer(next_revision.get())?,
                integer(now_ms)?,
                owner.as_str(),
                id.to_string()
            ],
        )
        .map_err(sql_failure)?;

    if changed {
        insert_event(
            &transaction,
            &owner,
            "project",
            &id.to_string(),
            next_revision,
            "project.refreshed",
            now_ms,
        )?;
    }
    committed(
        &transaction,
        caller,
        now_ms,
        CommittedStep::succeeded(
            PROJECT_REFRESH,
            if changed { "updated" } else { "no_changes" },
            Some(project_target(id, &observation.root_path)),
        ),
    )
    .map_err(sql_failure)?;
    let record = ProjectRecord {
        project_id: id,
        observation,
        revision: next_revision,
        registered_at_ms: current.registered_at_ms,
        last_modified_at_ms: current.last_modified_at_ms,
        favorite: current.favorite,
    };
    let response = SyncWrite {
        value: record,
        replayed: false,
    };
    save_response(
        &transaction,
        caller,
        PROJECT_REFRESH,
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(sql_failure)?;
    Ok(response)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn set_project_favorite(
    connection: &mut Connection,
    caller: &PrincipalId,
    id: ProjectId,
    favorite: bool,
    expected: Revision,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<SyncWrite<ProjectRecord>, M3Error> {
    let fingerprint = format!(
        r#"{{"expectedRevision":{},"favorite":{},"projectId":"{}","version":1}}"#,
        expected.get(),
        favorite,
        id
    );
    let transaction = begin(connection)?;
    let owner = project_resource_owner(&transaction, caller, app::Permission::ProjectsManage)?;
    if let Some(mut response) = existing_response::<SyncWrite<ProjectRecord>>(
        &transaction,
        caller,
        PROJECT_SET_FAVORITE,
        key,
        &fingerprint,
    )? {
        response.replayed = true;
        transaction.commit().map_err(sql_failure)?;
        return Ok(response);
    }
    let current = load_project(&transaction, &owner, id)?;
    if current.revision != expected {
        return Err(M3Error::new(M3ErrorCode::RevisionConflict));
    }
    let record = if current.favorite == favorite {
        current
    } else {
        let revision = current.revision.checked_next().ok_or_else(internal)?;
        transaction
            .execute(
                "UPDATE projects SET favorite=?1,revision=?2,updated_at_ms=?3
                 WHERE owner_principal_id=?4 AND project_id=?5",
                params![
                    i64::from(favorite),
                    integer(revision.get())?,
                    integer(now_ms)?,
                    owner.as_str(),
                    id.to_string()
                ],
            )
            .map_err(sql_failure)?;

        insert_event(
            &transaction,
            &owner,
            "project",
            &id.to_string(),
            revision,
            "project.favorite_changed",
            now_ms,
        )?;
        committed(
            &transaction,
            caller,
            now_ms,
            CommittedStep::succeeded(
                PROJECT_SET_FAVORITE,
                if favorite { "favorited" } else { "unfavorited" },
                Some(project_target(id, &current.observation.root_path)),
            ),
        )
        .map_err(sql_failure)?;
        ProjectRecord {
            favorite,
            revision,
            ..current
        }
    };
    let response = SyncWrite {
        value: record,
        replayed: false,
    };
    save_response(
        &transaction,
        caller,
        PROJECT_SET_FAVORITE,
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(sql_failure)?;
    Ok(response)
}

pub(super) fn unregister_project(
    connection: &mut Connection,
    caller: &PrincipalId,
    id: ProjectId,
    expected: Revision,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<UnregisterResult<ProjectId>, M3Error> {
    let fingerprint = aggregate_fingerprint(id, expected)?;
    let transaction = begin(connection)?;
    let owner = project_resource_owner(&transaction, caller, app::Permission::ProjectsManage)?;
    if let Some(mut response) = existing_response::<UnregisterResult<ProjectId>>(
        &transaction,
        caller,
        PROJECT_UNREGISTER,
        key,
        &fingerprint,
    )? {
        response.replayed = true;
        transaction.commit().map_err(sql_failure)?;
        return Ok(response);
    }
    let current = load_project(&transaction, &owner, id)?;
    if current.revision != expected {
        return Err(M3Error::new(M3ErrorCode::RevisionConflict));
    }
    let next = current.revision.checked_next().ok_or_else(internal)?;
    transaction
        .execute(
            "DELETE FROM projects WHERE owner_principal_id=?1 AND project_id=?2",
            params![owner.as_str(), id.to_string()],
        )
        .map_err(sql_failure)?;
    insert_event(
        &transaction,
        &owner,
        "project",
        &id.to_string(),
        next,
        "project.unregistered",
        now_ms,
    )?;
    committed(
        &transaction,
        caller,
        now_ms,
        CommittedStep::succeeded(
            PROJECT_UNREGISTER,
            "unregistered",
            Some(project_target(id, &current.observation.root_path)),
        ),
    )
    .map_err(sql_failure)?;
    let response = UnregisterResult {
        id,
        revision: next,
        replayed: false,
    };
    save_response(
        &transaction,
        caller,
        PROJECT_UNREGISTER,
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(sql_failure)?;
    Ok(response)
}

fn source_parts(source: &RepositorySource) -> (&'static str, &str) {
    match source {
        RepositorySource::Local { path } => ("local", path),
        RepositorySource::Remote { url } => ("remote", url),
    }
}

fn resolver_metadata(
    row: &Row<'_>,
    offset: usize,
) -> rusqlite::Result<Option<ResolverPackageMetadata>> {
    if row.get::<_, i64>(offset + 9)? == 0 {
        return Ok(None);
    }
    Ok(Some(ResolverPackageMetadata {
        semantic_version: row.get(offset)?,
        author_name: row.get(offset + 1)?,
        author_email: row.get(offset + 2)?,
        artifact_url: row.get(offset + 3)?,
        zip_sha256: row.get(offset + 4)?,
        unity_release: row.get(offset + 5)?,
        dependencies_json: row.get(offset + 6)?,
        manifest_fingerprint: row.get(offset + 7)?,
        legacy_metadata_present: row.get::<_, i64>(offset + 8)? != 0,
        download_headers_present: row.get(offset + 12)?,
    }))
}

fn load_packages(
    connection: &Connection,
    id: RepositoryId,
) -> Result<Vec<RepositoryPackageVersion>, M3Error> {
    let mut statement = connection
        .prepare(
            "SELECT package_id, version_text, display_name, description, yanked, unity_text,
                    semantic_version, author_name, author_email, artifact_url, zip_sha256,
                    unity_release_text, dependencies_json, manifest_fingerprint,
                    legacy_metadata_present, resolver_ready, documentation_url, changelog_url, download_headers_present, presentation_json
         FROM repository_package_versions WHERE repository_id=?1
         ORDER BY package_id ASC, version_text ASC",
        )
        .map_err(sql_failure)?;
    let rows = statement
        .query_map(params![id.to_string()], |row| {
            Ok(RepositoryPackageVersion {
                presentation: row
                    .get::<_, Option<String>>(19)?
                    .map(|v| serde_json::from_str(&v))
                    .transpose()
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                package_id: row.get(0)?,
                version: row.get(1)?,
                display_name: row.get(2)?,
                description: row.get(3)?,
                yanked: row.get::<_, i64>(4)? != 0,
                unity: row.get(5)?,
                resolver: resolver_metadata(row, 6)?,
                links: repository_package_links(row.get(16)?, row.get(17)?),
            })
        })
        .map_err(sql_failure)?;
    let mut packages = Vec::new();
    for row in rows {
        packages.push(row.map_err(sql_failure)?);
    }
    Ok(packages)
}

fn load_repository(
    connection: &Connection,
    owner: &PrincipalId,
    id: RepositoryId,
) -> Result<RepositoryRecord, M3Error> {
    let mut record = load_repository_summary(connection, owner, id)?;
    record.observation.packages = load_packages(connection, id)?;
    Ok(record)
}

fn load_repository_summary(
    connection: &Connection,
    owner: &PrincipalId,
    id: RepositoryId,
) -> Result<RepositoryRecord, M3Error> {
    type Row = (
        String,
        String,
        Vec<u8>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        String,
        i64,
        i64,
        i64,
        bool,
        bool,
        Option<String>,
    );
    let row: Row = connection.query_row(
        "SELECT source_kind, source_locator, source_identity_key, declared_id, name, declared_url,
                etag, last_modified, issues_json, revision, registered_at_ms, refreshed_at_ms,
                resolver_indexed, credential_reference IS NOT NULL, display_name
         FROM repositories WHERE owner_principal_id=?1 AND repository_id=?2",
        params![owner.as_str(), id.to_string()],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?,row.get(10)?,row.get(11)?,row.get(12)?,row.get(13)?,row.get(14)?)),
    ).optional().map_err(sql_failure)?.ok_or_else(|| M3Error::new(M3ErrorCode::RepositoryNotRegistered))?;
    let source = match row.0.as_str() {
        "local" => RepositorySource::Local { path: row.1 },
        "remote" => RepositorySource::Remote { url: row.1 },
        _ => return Err(internal()),
    };
    Ok(RepositoryRecord {
        repository_id: id,
        display_name: row.14,
        observation: RepositoryObservation {
            source,
            source_identity_key: row.2,
            declared_id: row.3,
            name: row.4,
            declared_url: row.5,
            issues: parse_json(&row.8)?,
            packages: Vec::new(),
            validators: RepositoryValidators {
                etag: row.6,
                last_modified: row.7,
            },
            refreshed_at_ms: u64::try_from(row.11).map_err(|_| internal())?,
        },
        revision: revision(row.9)?,
        registered_at_ms: u64::try_from(row.10).map_err(|_| internal())?,
        resolver_indexed: row.12,
        credentials_configured: row.13,
    })
}

fn insert_packages(
    transaction: &Transaction<'_>,
    id: RepositoryId,
    packages: &[RepositoryPackageVersion],
    resolver_indexed: bool,
) -> Result<(), M3Error> {
    let mut insert = transaction
        .prepare(
            "INSERT INTO repository_package_versions (
                repository_id, package_id, version_text, display_name, description, yanked,
                unity_text, semantic_version, author_name, author_email, artifact_url,
                zip_sha256, unity_release_text, dependencies_json, manifest_fingerprint,
                legacy_metadata_present, resolver_ready, documentation_url, changelog_url, download_headers_present, presentation_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                       coalesce(?14, '{}'), ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
        )
        .map_err(sql_failure)?;
    let id = id.to_string();
    for package in packages {
        let resolver = if resolver_indexed {
            package.resolver.as_ref()
        } else {
            None
        };
        insert
            .execute(params![
                id,
                package.package_id,
                package.version,
                package.display_name,
                package.description,
                i64::from(package.yanked),
                package.unity,
                resolver.map(|value| value.semantic_version.as_str()),
                resolver.and_then(|value| value.author_name.as_deref()),
                resolver.and_then(|value| value.author_email.as_deref()),
                resolver.map(|value| value.artifact_url.as_str()),
                resolver.and_then(|value| value.zip_sha256.as_deref()),
                resolver.and_then(|value| value.unity_release.as_deref()),
                resolver.map(|value| value.dependencies_json.as_str()),
                resolver.map(|value| value.manifest_fingerprint.as_slice()),
                i64::from(resolver.is_some_and(|value| value.legacy_metadata_present)),
                i64::from(resolver.is_some()),
                package
                    .links
                    .as_ref()
                    .and_then(|value| value.documentation.as_deref()),
                package
                    .links
                    .as_ref()
                    .and_then(|value| value.changelog.as_deref()),
                resolver.is_some_and(|value| value.download_headers_present),
                package
                    .presentation
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()
                    .map_err(|_| internal())?,
            ])
            .map_err(sql_failure)?;
    }
    Ok(())
}

struct RepositoryInsertOptions<'a> {
    registered_at_ms: u64,
    now_ms: u64,
    priority: u64,
    resolver_indexed: bool,
    display_name: Option<&'a str>,
}

fn next_repository_priority(connection: &Connection, owner: &PrincipalId) -> Result<u64, M3Error> {
    let priority = connection
        .query_row(
            "SELECT coalesce(max(priority), 0) + 1 FROM repositories WHERE owner_principal_id=?1",
            [owner.as_str()],
            |row| row.get::<_, i64>(0),
        )
        .map_err(sql_failure)?;
    u64::try_from(priority).map_err(|_| internal())
}

fn insert_repository(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    id: RepositoryId,
    observation: &RepositoryObservation,
    revision: Revision,
    options: RepositoryInsertOptions<'_>,
) -> Result<(), M3Error> {
    let (kind, locator) = source_parts(&observation.source);
    transaction
        .execute(
            "INSERT INTO repositories (
            repository_id, owner_principal_id, source_kind, source_locator, source_identity_key,
            declared_id, name, declared_url, etag, last_modified, issues_json, revision,
            registered_at_ms, refreshed_at_ms, updated_at_ms, priority, resolver_indexed, display_name
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                id.to_string(),
                owner.as_str(),
                kind,
                locator,
                observation.source_identity_key,
                observation.declared_id,
                observation.name,
                observation.declared_url,
                observation.validators.etag,
                observation.validators.last_modified,
                json(&observation.issues)?,
                integer(revision.get())?,
                integer(options.registered_at_ms)?,
                integer(observation.refreshed_at_ms)?,
                integer(options.now_ms)?,
                integer(options.priority)?,
                options.resolver_indexed,
                options.display_name,
            ],
        )
        .map_err(sql_failure)?;
    insert_packages(
        transaction,
        id,
        &observation.packages,
        options.resolver_indexed,
    )
}

pub(super) fn ensure_builtin_repositories(
    connection: &mut Connection,
    owner: &PrincipalId,
    defaults: Vec<(RepositoryId, RepositoryObservation)>,
    now_ms: u64,
) -> Result<Vec<RepositoryRecord>, M3Error> {
    let transaction = begin(connection)?;
    let mut records = Vec::with_capacity(defaults.len());
    for (id, observation) in defaults {
        let existing = transaction
            .query_row(
                "SELECT repository_id FROM repositories
                 WHERE owner_principal_id=?1 AND source_identity_key=?2",
                params![owner.as_str(), observation.source_identity_key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(sql_failure)?;
        let id = if let Some(existing) = existing {
            RepositoryId::parse(&existing).map_err(|_| internal())?
        } else {
            insert_repository(
                &transaction,
                owner,
                id,
                &observation,
                Revision::INITIAL,
                RepositoryInsertOptions {
                    registered_at_ms: now_ms,
                    now_ms,
                    priority: next_repository_priority(&transaction, owner)?,
                    resolver_indexed: false,
                    display_name: None,
                },
            )?;
            insert_event(
                &transaction,
                owner,
                "repository",
                &id.to_string(),
                Revision::INITIAL,
                "repository.registered",
                now_ms,
            )?;
            id
        };
        records.push(load_repository_summary(&transaction, owner, id)?);
    }
    transaction.commit().map_err(sql_failure)?;
    Ok(records)
}

pub(super) fn register_repository(
    connection: &mut Connection,
    access: &RepositoryAccess,
    registration: app::RepositoryRegistration,
    credential: Option<&str>,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<SyncWrite<RepositoryRecord>, M3Error> {
    register_repository_projected(
        connection,
        access.scope(),
        registration,
        credential,
        key,
        now_ms,
    )
}

fn register_repository_projected<T: From<RepositoryRecord> + Serialize + DeserializeOwned>(
    connection: &mut Connection,
    scope: RepositoryWriteScope<'_>,
    registration: app::RepositoryRegistration,
    credential: Option<&str>,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<SyncWrite<T>, M3Error> {
    let (method, fingerprint) = scope.identity(REPOSITORY_REGISTER, || {
        registration_fingerprint(&registration.requested_source, credential.is_some())
    })?;
    let mut observation = registration.observation;
    let owner = scope.owner;
    let transaction = begin(connection)?;
    scope.require(&transaction)?;
    if let Some(mut response) =
        existing_response::<SyncWrite<T>>(&transaction, scope.caller(), method, key, &fingerprint)?
    {
        response.replayed = true;
        transaction.commit().map_err(sql_failure)?;
        return Ok(response);
    }
    let duplicate = transaction
        .query_row(
            "SELECT 1 FROM repositories WHERE source_identity_key=?1",
            params![observation.source_identity_key],
            |_| Ok(()),
        )
        .optional()
        .map_err(sql_failure)?
        .is_some();
    if duplicate {
        return Err(M3Error::new(M3ErrorCode::RepositoryAlreadyRegistered));
    }
    let id = RepositoryId::new();
    insert_repository(
        &transaction,
        owner,
        id,
        &observation,
        Revision::INITIAL,
        RepositoryInsertOptions {
            registered_at_ms: now_ms,
            now_ms,
            priority: next_repository_priority(&transaction, owner)?,
            resolver_indexed: true,
            display_name: None,
        },
    )?;
    if let Some(reference) = credential {
        transaction
            .execute(
                "UPDATE repositories SET credential_reference=?1 WHERE repository_id=?2",
                params![reference, id.to_string()],
            )
            .map_err(sql_failure)?;
        finish_credential_cleanup(&transaction, reference)?;
    }
    // The write result is repository metadata. Package versions are read separately by page.
    observation.packages.clear();
    let record = RepositoryRecord {
        repository_id: id,
        display_name: None,
        observation,
        revision: Revision::INITIAL,
        registered_at_ms: now_ms,
        resolver_indexed: true,
        credentials_configured: credential.is_some(),
    };
    insert_event(
        &transaction,
        owner,
        "repository",
        &id.to_string(),
        Revision::INITIAL,
        "repository.registered",
        now_ms,
    )?;
    committed(
        &transaction,
        scope.caller(),
        now_ms,
        CommittedStep::succeeded(
            REPOSITORY_REGISTER,
            "registered",
            Some(repository_target(id, &record.observation)),
        ),
    )
    .map_err(sql_failure)?;
    let response = SyncWrite {
        value: record.into(),
        replayed: false,
    };
    save_response(
        &transaction,
        scope.caller(),
        method,
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(sql_failure)?;
    Ok(response)
}

pub(super) fn get_repository(
    connection: &Connection,
    owner: &PrincipalId,
    id: RepositoryId,
) -> Result<RepositoryRecord, M3Error> {
    load_repository(connection, owner, id)
}

pub(super) fn credential_reference(
    connection: &Connection,
    owner: &PrincipalId,
    id: RepositoryId,
) -> Result<Option<String>, M3Error> {
    connection.query_row(
        "SELECT credential_reference FROM repositories WHERE owner_principal_id=?1 AND repository_id=?2",
        params![owner.as_str(), id.to_string()],
        |row| row.get(0),
    ).optional().map_err(sql_failure)?.ok_or_else(|| M3Error::new(M3ErrorCode::RepositoryNotRegistered))
}

pub(super) fn queue_credential_cleanup(
    connection: &Connection,
    reference: &str,
) -> Result<(), M3Error> {
    connection
        .execute(
            "INSERT OR IGNORE INTO repository_credential_cleanup(credential_reference) VALUES(?1)",
            [reference],
        )
        .map_err(sql_failure)?;
    Ok(())
}

pub(super) fn finish_credential_cleanup(
    connection: &Connection,
    reference: &str,
) -> Result<(), M3Error> {
    connection
        .execute(
            "DELETE FROM repository_credential_cleanup WHERE credential_reference=?1",
            [reference],
        )
        .map_err(sql_failure)?;
    Ok(())
}

pub(super) fn pending_credential_cleanup(connection: &Connection) -> Result<Vec<String>, M3Error> {
    let mut statement = connection.prepare(
        "SELECT c.credential_reference FROM repository_credential_cleanup c
         WHERE NOT EXISTS (SELECT 1 FROM repositories r WHERE r.credential_reference=c.credential_reference)
         ORDER BY c.credential_reference LIMIT 4096",
    ).map_err(sql_failure)?;
    statement
        .query_map([], |row| row.get(0))
        .map_err(sql_failure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_failure)
}

pub(super) fn set_repository_display_name(
    connection: &mut Connection,
    access: &RepositoryAccess,
    id: RepositoryId,
    expected: Revision,
    display_name: Option<String>,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<SyncWrite<RepositoryRecord>, M3Error> {
    let fingerprint = json(&serde_json::json!({
        "repositoryId": id.to_string(), "expectedRevision": expected.get(),
        "displayName": display_name, "version": 1
    }))?;
    let transaction = begin(connection)?;
    let scope = access.scope();
    scope.require(&transaction)?;
    let owner = scope.owner;
    if let Some(mut response) = existing_response::<SyncWrite<RepositoryRecord>>(
        &transaction,
        scope.caller(),
        REPOSITORY_SET_DISPLAY_NAME,
        key,
        &fingerprint,
    )? {
        response.replayed = true;
        transaction.commit().map_err(sql_failure)?;
        return Ok(response);
    }
    let mut current = load_repository_summary(&transaction, owner, id)?;
    if current.revision != expected {
        return Err(M3Error::new(M3ErrorCode::RevisionConflict));
    }
    let changed = current.display_name != display_name;
    if changed {
        let next = expected.checked_next().ok_or_else(internal)?;
        transaction
            .execute(
                "UPDATE repositories SET display_name=?1, revision=?2, updated_at_ms=?3
             WHERE owner_principal_id=?4 AND repository_id=?5",
                params![
                    display_name,
                    integer(next.get())?,
                    integer(now_ms)?,
                    owner.as_str(),
                    id.to_string()
                ],
            )
            .map_err(sql_failure)?;
        current.revision = next;
        current.display_name = display_name;
        insert_event(
            &transaction,
            owner,
            "repository",
            &id.to_string(),
            next,
            "repository.display-name-changed",
            now_ms,
        )?;
    }
    committed(
        &transaction,
        scope.caller(),
        now_ms,
        CommittedStep::succeeded(
            REPOSITORY_SET_DISPLAY_NAME,
            if changed { "updated" } else { "no_changes" },
            Some(repository_target(id, &current.observation)),
        ),
    )
    .map_err(sql_failure)?;
    let response = SyncWrite {
        value: current,
        replayed: false,
    };
    save_response(
        &transaction,
        scope.caller(),
        REPOSITORY_SET_DISPLAY_NAME,
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(sql_failure)?;
    Ok(response)
}

pub(super) fn set_credential_reference(
    connection: &mut Connection,
    access: &RepositoryAccess,
    id: RepositoryId,
    expected: Revision,
    reference: Option<&str>,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<SyncWrite<RepositoryRecord>, M3Error> {
    let fingerprint = credential_fingerprint(id, expected, reference.is_some())?;
    let transaction = begin(connection)?;
    let scope = access.scope();
    scope.require(&transaction)?;
    let owner = scope.owner;
    if let Some(mut response) = existing_response::<SyncWrite<RepositoryRecord>>(
        &transaction,
        scope.caller(),
        REPOSITORY_SET_CREDENTIALS,
        key,
        &fingerprint,
    )? {
        response.replayed = true;
        transaction.commit().map_err(sql_failure)?;
        return Ok(response);
    }
    let mut current = load_repository_summary(&transaction, owner, id)?;
    if current.revision != expected {
        return Err(M3Error::new(M3ErrorCode::RevisionConflict));
    }
    if !alcomd_application::repository_supports_authentication(&current.observation.source) {
        return Err(M3Error::new(M3ErrorCode::RepositorySourceInvalid));
    }
    let next = expected.checked_next().ok_or_else(internal)?;
    if let Some(old) = credential_reference(&transaction, owner, id)? {
        queue_credential_cleanup(&transaction, &old)?;
    }
    transaction.execute(
        "UPDATE repositories SET credential_reference=?1, revision=?2, etag=NULL, last_modified=NULL
         WHERE owner_principal_id=?3 AND repository_id=?4",
        params![reference, integer(next.get())?, owner.as_str(), id.to_string()],
    ).map_err(sql_failure)?;
    if let Some(reference) = reference {
        finish_credential_cleanup(&transaction, reference)?;
    }
    current.revision = next;
    current.credentials_configured = reference.is_some();
    current.observation.validators = RepositoryValidators::default();
    insert_event(
        &transaction,
        owner,
        "repository",
        &id.to_string(),
        next,
        "repository.refreshed",
        now_ms,
    )?;
    committed(
        &transaction,
        scope.caller(),
        now_ms,
        CommittedStep::succeeded(
            REPOSITORY_SET_CREDENTIALS,
            "updated",
            Some(repository_target(id, &current.observation)),
        ),
    )
    .map_err(sql_failure)?;
    let response = SyncWrite {
        value: current,
        replayed: false,
    };
    save_response(
        &transaction,
        scope.caller(),
        REPOSITORY_SET_CREDENTIALS,
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(sql_failure)?;
    Ok(response)
}

pub(super) fn ordered_repository_query(select: &str) -> String {
    format!(
        "WITH repository_order AS (
            SELECT repository_id, ROW_NUMBER() OVER (ORDER BY
                CASE repository_id
                    WHEN '{VRCHAT_OFFICIAL_REPOSITORY_ID}' THEN 0
                    WHEN '{VRCHAT_CURATED_REPOSITORY_ID}' THEN 1
                    ELSE 2
                END, priority, repository_id) AS list_rank
            FROM repositories WHERE owner_principal_id=?1
        ) {select}"
    )
}

struct OrderedRepository {
    id: String,
    registered_at_ms: i64,
    order_key: i64,
}

fn repository_order(
    connection: &Connection,
    owner: &PrincipalId,
) -> Result<Vec<OrderedRepository>, M3Error> {
    let mut statement = connection
        .prepare(&ordered_repository_query(
            "SELECT r.repository_id, r.registered_at_ms, r.priority
         FROM repository_order o JOIN repositories r USING(repository_id) ORDER BY o.list_rank",
        ))
        .map_err(sql_failure)?;
    statement
        .query_map([owner.as_str()], |row| {
            Ok(OrderedRepository {
                id: row.get(0)?,
                registered_at_ms: row.get(1)?,
                order_key: row.get(2)?,
            })
        })
        .map_err(sql_failure)?
        .collect::<Result<_, _>>()
        .map_err(sql_failure)
}

pub(super) fn list_repositories(
    connection: &Connection,
    owner: &PrincipalId,
    cursor: Option<app::RepositoryCursor>,
    limit: u32,
) -> Result<RepositoryPage, M3Error> {
    let order = repository_order(connection, owner)?;
    let order_fingerprint = app::repository_order_fingerprint(order.iter().map(|entry| &entry.id));
    let conflict = || M3Error::new(M3ErrorCode::RevisionConflict);
    let start = match cursor {
        Some(cursor) => {
            if cursor.order_fingerprint != order_fingerprint {
                return Err(conflict());
            }
            let id = cursor.id.to_string();
            let registered_at_ms = integer(cursor.registered_at_ms)?;
            order
                .iter()
                .position(|entry| entry.id == id && entry.registered_at_ms == registered_at_ms)
                .ok_or_else(conflict)?
                + 1
        }
        None => 0,
    };
    let end = (start + limit as usize).min(order.len());
    let mut repositories = Vec::with_capacity(end - start);
    for entry in &order[start..end] {
        let id = RepositoryId::parse(&entry.id).map_err(|_| internal())?;
        repositories.push(load_repository_summary(connection, owner, id)?);
    }
    let next_cursor = if end < order.len() {
        repositories.last().map(|record| app::RepositoryCursor {
            registered_at_ms: record.registered_at_ms,
            id: record.repository_id,
            order_fingerprint: order_fingerprint.clone(),
        })
    } else {
        None
    };
    Ok(RepositoryPage {
        repositories,
        next_cursor,
        order_fingerprint,
    })
}

pub(super) fn move_repository(
    connection: &mut Connection,
    access: &RepositoryAccess,
    movement: app::RepositoryMove,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<app::RepositoryMoveResult, M3Error> {
    let id = movement.id.to_string();
    let before = movement.before.map(|value| value.to_string());
    let request = json(&serde_json::json!([
        id,
        before,
        movement.expected_order_fingerprint
    ]))?;
    let transaction = begin(connection)?;
    let scope = access.scope();
    scope.require(&transaction)?;
    let owner = scope.owner;
    if let Some(mut result) = existing_response::<app::RepositoryMoveResult>(
        &transaction,
        scope.caller(),
        REPOSITORY_MOVE,
        key,
        &request,
    )? {
        result.replayed = true;
        transaction.commit().map_err(sql_failure)?;
        return Ok(result);
    }
    let mut order = repository_order(&transaction, owner)?;
    let fingerprint = app::repository_order_fingerprint(order.iter().map(|entry| &entry.id));
    if fingerprint != movement.expected_order_fingerprint {
        return Err(M3Error::new(M3ErrorCode::RevisionConflict));
    }
    let missing = || M3Error::new(M3ErrorCode::RepositoryNotRegistered);
    let from = order
        .iter()
        .position(|entry| entry.id == id)
        .ok_or_else(missing)?;
    let target = match before {
        Some(before) => order
            .iter()
            .position(|entry| entry.id == before)
            .ok_or_else(missing)?,
        None => order.len(),
    };
    let to = if target > from { target - 1 } else { target };
    let changed = from != to;
    if changed {
        let entry = order.remove(from);
        order.insert(to, entry);
        for (index, entry) in order.into_iter().enumerate() {
            let order_key = index as u64 + 1;
            if integer(order_key)? == entry.order_key {
                continue;
            }
            let id = RepositoryId::parse(&entry.id).map_err(|_| internal())?;
            let current = load_repository_summary(&transaction, owner, id)?;
            write_repository_metadata(
                &transaction,
                owner,
                &current,
                current.display_name.as_deref(),
                order_key,
                "repository.order-changed",
                now_ms,
            )?;
        }
    }
    let result = app::RepositoryMoveResult {
        changed,
        replayed: false,
    };
    committed(
        &transaction,
        scope.caller(),
        now_ms,
        CommittedStep::succeeded(
            REPOSITORY_MOVE,
            if changed { "updated" } else { "no_changes" },
            Some(app::LogTarget {
                kind: "repository".into(),
                id: Some(id),
                name: None,
                path: None,
            }),
        ),
    )
    .map_err(sql_failure)?;
    save_response(
        &transaction,
        scope.caller(),
        REPOSITORY_MOVE,
        key,
        &request,
        &result,
        now_ms,
    )?;
    transaction.commit().map_err(sql_failure)?;
    Ok(result)
}

pub(super) fn list_repository_packages(
    connection: &Connection,
    owner: &PrincipalId,
    id: RepositoryId,
    cursor: Option<PackageCursor>,
    limit: u32,
) -> Result<PackagePage, M3Error> {
    load_repository_summary(connection, owner, id)?;
    let mut statement = connection
        .prepare(
            "SELECT package_id, version_text, display_name, description, yanked, unity_text,
                    semantic_version, author_name, author_email, artifact_url, zip_sha256,
                    unity_release_text, dependencies_json, manifest_fingerprint,
                    legacy_metadata_present, resolver_ready, documentation_url, changelog_url, download_headers_present, presentation_json
         FROM repository_package_versions WHERE repository_id=?1 AND
         (?2 IS NULL OR package_id > ?2 OR (package_id = ?2 AND version_text > ?3))
         ORDER BY package_id ASC, version_text ASC LIMIT ?4",
        )
        .map_err(sql_failure)?;
    let cursor_package = cursor.as_ref().map(|value| value.package_id.as_str());
    let cursor_version = cursor.as_ref().map(|value| value.version.as_str());
    let rows = statement
        .query_map(
            params![
                id.to_string(),
                cursor_package,
                cursor_version,
                i64::from(limit) + 1
            ],
            |row| {
                Ok(RepositoryPackageVersion {
                    presentation: row
                        .get::<_, Option<String>>(19)?
                        .map(|v| serde_json::from_str(&v))
                        .transpose()
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    package_id: row.get(0)?,
                    version: row.get(1)?,
                    display_name: row.get(2)?,
                    description: row.get(3)?,
                    yanked: row.get::<_, i64>(4)? != 0,
                    unity: row.get(5)?,
                    resolver: resolver_metadata(row, 6)?,
                    links: repository_package_links(row.get(16)?, row.get(17)?),
                })
            },
        )
        .map_err(sql_failure)?;
    let mut packages = Vec::new();
    for row in rows {
        packages.push(row.map_err(sql_failure)?);
    }
    let has_more = packages.len() > limit as usize;
    if has_more {
        packages.pop();
    }
    let next_cursor = has_more
        .then(|| {
            packages.last().map(|value| PackageCursor {
                package_id: value.package_id.clone(),
                version: value.version.clone(),
            })
        })
        .flatten();
    Ok(PackagePage {
        packages,
        next_cursor,
    })
}

fn repository_package_links(
    documentation: Option<String>,
    changelog: Option<String>,
) -> Option<alcomd_application::RepositoryPackageLinks> {
    (documentation.is_some() || changelog.is_some()).then_some(
        alcomd_application::RepositoryPackageLinks {
            documentation,
            changelog,
        },
    )
}

struct RepositoryArchivePin {
    package_id: String,
    version: String,
    manifest_fingerprint: Vec<u8>,
    artifact_url: String,
    digest: Vec<u8>,
}

fn load_repository_archive_pins(
    transaction: &Transaction<'_>,
    id: RepositoryId,
) -> Result<Vec<RepositoryArchivePin>, M3Error> {
    let mut select = transaction
        .prepare(
            "SELECT package_id, version_text, manifest_fingerprint, artifact_url, local_zip_sha256
             FROM repository_package_versions WHERE repository_id=?1 AND local_zip_sha256 IS NOT NULL",
        )
        .map_err(sql_failure)?;
    select
        .query_map([id.to_string()], |row| {
            Ok(RepositoryArchivePin {
                package_id: row.get(0)?,
                version: row.get(1)?,
                manifest_fingerprint: row.get(2)?,
                artifact_url: row.get(3)?,
                digest: row.get(4)?,
            })
        })
        .map_err(sql_failure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(sql_failure)
}

fn restore_repository_archive_pins(
    transaction: &Transaction<'_>,
    id: RepositoryId,
    pins: Vec<RepositoryArchivePin>,
) -> Result<(), M3Error> {
    if pins.is_empty() {
        return Ok(());
    }
    let mut update = transaction
        .prepare(
            "UPDATE repository_package_versions SET local_zip_sha256=?1
             WHERE repository_id=?2 AND package_id=?3 AND version_text=?4
             AND manifest_fingerprint=?5 AND artifact_url=?6
             AND zip_sha256 IS NULL AND resolver_ready=1",
        )
        .map_err(sql_failure)?;
    let id = id.to_string();
    for pin in pins {
        update
            .execute(params![
                pin.digest,
                id,
                pin.package_id,
                pin.version,
                pin.manifest_fingerprint,
                pin.artifact_url
            ])
            .map_err(sql_failure)?;
    }
    Ok(())
}

pub(super) fn refresh_repository(
    connection: &mut Connection,
    access: &RepositoryAccess,
    id: RepositoryId,
    expected: Revision,
    observation: RepositoryObservation,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<SyncWrite<app::RepositoryRefreshReceipt>, M3Error> {
    let scope = access.scope();
    let (method, fingerprint) =
        scope.identity(REPOSITORY_REFRESH, || aggregate_fingerprint(id, expected))?;
    let owner = scope.owner;
    let transaction = begin(connection)?;
    scope.require(&transaction)?;
    if let Some(mut response) = existing_response::<SyncWrite<app::RepositoryRefreshReceipt>>(
        &transaction,
        scope.caller(),
        method,
        key,
        &fingerprint,
    )? {
        response.replayed = true;
        transaction.commit().map_err(sql_failure)?;
        return Ok(response);
    }
    let current = load_repository(&transaction, owner, id)?;
    if current.revision != expected {
        return Err(M3Error::new(M3ErrorCode::RevisionConflict));
    }
    if current.observation.source_identity_key != observation.source_identity_key {
        return Err(M3Error::new(M3ErrorCode::RepositorySourceInvalid));
    }
    let changed = !app::same_repository_evidence(&current.observation, &observation);
    let observed = current.observation.refreshed_at_ms != observation.refreshed_at_ms
        || !current.resolver_indexed;
    let next = if changed {
        current.revision.checked_next().ok_or_else(internal)?
    } else {
        current.revision
    };
    let priority = transaction
        .query_row(
            "SELECT priority FROM repositories WHERE repository_id=?1",
            [id.to_string()],
            |row| row.get::<_, i64>(0),
        )
        .map_err(sql_failure)?;
    let pins = load_repository_archive_pins(&transaction, id)?;
    let credential = credential_reference(&transaction, owner, id)?;
    transaction
        .execute(
            "DELETE FROM repositories WHERE owner_principal_id=?1 AND repository_id=?2",
            params![owner.as_str(), id.to_string()],
        )
        .map_err(sql_failure)?;
    insert_repository(
        &transaction,
        owner,
        id,
        &observation,
        next,
        RepositoryInsertOptions {
            registered_at_ms: current.registered_at_ms,
            now_ms,
            priority: u64::try_from(priority).map_err(|_| internal())?,
            resolver_indexed: true,
            display_name: current.display_name.as_deref(),
        },
    )?;
    restore_repository_archive_pins(&transaction, id, pins)?;
    if let Some(reference) = credential.as_deref() {
        transaction
            .execute(
                "UPDATE repositories SET credential_reference=?1 WHERE repository_id=?2",
                params![reference, id.to_string()],
            )
            .map_err(sql_failure)?;
    }
    if changed {
        insert_event(
            &transaction,
            owner,
            "repository",
            &id.to_string(),
            next,
            "repository.refreshed",
            now_ms,
        )?;
    } else if observed {
        insert_event(
            &transaction,
            owner,
            "repository",
            &id.to_string(),
            next,
            "repository.observed",
            now_ms,
        )?;
    }
    let record = RepositoryRecord {
        repository_id: id,
        display_name: current.display_name,
        observation,
        revision: next,
        registered_at_ms: current.registered_at_ms,
        resolver_indexed: true,
        credentials_configured: credential.is_some(),
    };
    committed(
        &transaction,
        scope.caller(),
        now_ms,
        CommittedStep::succeeded(
            REPOSITORY_REFRESH,
            if changed { "updated" } else { "no_changes" },
            Some(repository_target(id, &record.observation)),
        ),
    )
    .map_err(sql_failure)?;
    let response = SyncWrite {
        value: record.into(),
        replayed: false,
    };
    save_response(
        &transaction,
        scope.caller(),
        method,
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(sql_failure)?;
    Ok(response)
}

pub(super) fn update_repository_validators(
    connection: &mut Connection,
    access: &RepositoryAccess,
    id: RepositoryId,
    expected: Revision,
    validators: RepositoryValidators,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<SyncWrite<app::RepositoryRefreshReceipt>, M3Error> {
    let scope = access.scope();
    let (method, fingerprint) =
        scope.identity(REPOSITORY_REFRESH, || aggregate_fingerprint(id, expected))?;
    let owner = scope.owner;
    let transaction = begin(connection)?;
    scope.require(&transaction)?;
    if let Some(mut response) = existing_response::<SyncWrite<app::RepositoryRefreshReceipt>>(
        &transaction,
        scope.caller(),
        method,
        key,
        &fingerprint,
    )? {
        response.replayed = true;
        transaction.commit().map_err(sql_failure)?;
        return Ok(response);
    }
    let mut current = load_repository_summary(&transaction, owner, id)?;
    if current.revision != expected {
        return Err(M3Error::new(M3ErrorCode::RevisionConflict));
    }
    if !current.resolver_indexed {
        return Err(M3Error::new(M3ErrorCode::RepositoryUnavailable));
    }
    let observed = current.observation.refreshed_at_ms != now_ms;
    current.observation.validators = validators.clone();
    current.observation.refreshed_at_ms = now_ms;
    transaction.execute("UPDATE repositories SET etag=?1, last_modified=?2, refreshed_at_ms=?3, updated_at_ms=?3 WHERE owner_principal_id=?4 AND repository_id=?5",
        params![validators.etag, validators.last_modified, integer(now_ms)?, owner.as_str(), id.to_string()]).map_err(sql_failure)?;
    if observed {
        insert_event(
            &transaction,
            owner,
            "repository",
            &id.to_string(),
            current.revision,
            "repository.observed",
            now_ms,
        )?;
    }
    let target = repository_target(id, &current.observation);
    let response = SyncWrite {
        value: current.into(),
        replayed: false,
    };
    committed(
        &transaction,
        scope.caller(),
        now_ms,
        CommittedStep::succeeded(REPOSITORY_REFRESH, "no_changes", Some(target)),
    )
    .map_err(sql_failure)?;
    save_response(
        &transaction,
        scope.caller(),
        method,
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(sql_failure)?;
    Ok(response)
}

pub(super) fn unregister_repository(
    connection: &mut Connection,
    access: &RepositoryAccess,
    id: RepositoryId,
    expected: Revision,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<UnregisterResult<RepositoryId>, M3Error> {
    unregister_repository_scoped(connection, access.scope(), id, expected, key, now_ms)
}

fn unregister_repository_scoped(
    connection: &mut Connection,
    scope: RepositoryWriteScope<'_>,
    id: RepositoryId,
    expected: Revision,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<UnregisterResult<RepositoryId>, M3Error> {
    let (method, fingerprint) = scope.identity(REPOSITORY_UNREGISTER, || {
        aggregate_fingerprint(id, expected)
    })?;
    let owner = scope.owner;
    let transaction = begin(connection)?;
    scope.require(&transaction)?;
    if let Some(mut response) = existing_response::<UnregisterResult<RepositoryId>>(
        &transaction,
        scope.caller(),
        method,
        key,
        &fingerprint,
    )? {
        response.replayed = true;
        transaction.commit().map_err(sql_failure)?;
        return Ok(response);
    }
    let current = load_repository_summary(&transaction, owner, id)?;
    if current.revision != expected {
        return Err(M3Error::new(M3ErrorCode::RevisionConflict));
    }
    let next = current.revision.checked_next().ok_or_else(internal)?;
    if let Some(reference) = credential_reference(&transaction, owner, id)? {
        queue_credential_cleanup(&transaction, &reference)?;
    }
    transaction
        .execute(
            "DELETE FROM repositories WHERE owner_principal_id=?1 AND repository_id=?2",
            params![owner.as_str(), id.to_string()],
        )
        .map_err(sql_failure)?;
    insert_event(
        &transaction,
        owner,
        "repository",
        &id.to_string(),
        next,
        "repository.unregistered",
        now_ms,
    )?;
    committed(
        &transaction,
        scope.caller(),
        now_ms,
        CommittedStep::succeeded(
            REPOSITORY_UNREGISTER,
            "unregistered",
            Some(repository_target(id, &current.observation)),
        ),
    )
    .map_err(sql_failure)?;
    let response = UnregisterResult {
        id,
        revision: next,
        replayed: false,
    };
    save_response(
        &transaction,
        scope.caller(),
        method,
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(sql_failure)?;
    Ok(response)
}

pub(super) fn apply_repository_list_metadata(
    connection: &mut Connection,
    access: &RepositoryAccess,
    expected_order: &str,
    changes: Vec<alcomd_application::RepositoryListMetadata>,
    now_ms: u64,
) -> Result<(), M3Error> {
    let transaction = begin(connection)?;
    let scope = access.scope();
    scope.require(&transaction)?;
    let owner = scope.owner;
    let order = repository_order(&transaction, owner)?;
    if app::repository_order_fingerprint(order.iter().map(|entry| &entry.id)) != expected_order {
        return Err(M3Error::new(M3ErrorCode::RevisionConflict));
    }
    for change in changes {
        let id = RepositoryId::parse(&change.repository_id).map_err(|_| internal())?;
        let current = load_repository_summary(&transaction, owner, id)?;
        let priority=transaction.query_row("SELECT priority FROM repositories WHERE owner_principal_id=?1 AND repository_id=?2",params![owner.as_str(),id.to_string()],|row|row.get::<_,i64>(0)).map_err(sql_failure)?;
        let priority = u64::try_from(priority).map_err(|_| internal())?;
        if current.display_name == change.display_name && priority == change.priority {
            continue;
        }
        if current.display_name != change.expected_display_name
            || priority != change.expected_priority
        {
            return Err(M3Error::new(M3ErrorCode::RevisionConflict));
        }
        write_repository_metadata(
            &transaction,
            owner,
            &current,
            change.display_name.as_deref(),
            change.priority,
            "repository.configuration-imported",
            now_ms,
        )?;
        committed(
            &transaction,
            scope.caller(),
            now_ms,
            CommittedStep::succeeded(
                "repositories.importListFile",
                "restore_repository_configuration",
                Some(repository_target(id, &current.observation)),
            ),
        )
        .map_err(sql_failure)?;
    }
    transaction.commit().map_err(sql_failure)
}

fn write_repository_metadata(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    current: &RepositoryRecord,
    name: Option<&str>,
    order_key: u64,
    event: &str,
    now_ms: u64,
) -> Result<(), M3Error> {
    let next = current.revision.checked_next().ok_or_else(internal)?;
    let id = current.repository_id.to_string();
    transaction
        .execute(
            "UPDATE repositories SET display_name=?1, priority=?2, revision=?3, updated_at_ms=?4
         WHERE owner_principal_id=?5 AND repository_id=?6",
            params![
                name,
                integer(order_key)?,
                integer(next.get())?,
                integer(now_ms)?,
                owner.as_str(),
                id
            ],
        )
        .map_err(sql_failure)?;
    insert_event(transaction, owner, "repository", &id, next, event, now_ms)
}
