use alcomd_application::{
    IdempotencyKey, M3Error, M3ErrorCode, PackageCursor, PackagePage, PrincipalId, ProjectId,
    ProjectObservation, ProjectPage, ProjectRecord, RegistryCursor, RepositoryId,
    RepositoryObservation, RepositoryPackageVersion, RepositoryPage, RepositoryRecord,
    RepositorySource, RepositoryValidators, Revision, SyncWrite, UnregisterResult,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::Serialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;

const PROJECT_REGISTER: &str = "projects.register";
const PROJECT_REFRESH: &str = "projects.refresh";
const PROJECT_UNREGISTER: &str = "projects.unregister";
const REPOSITORY_REGISTER: &str = "repositories.register";
const REPOSITORY_REFRESH: &str = "repositories.refresh";
const REPOSITORY_UNREGISTER: &str = "repositories.unregister";

pub(super) fn unavailable() -> M3Error {
    M3Error::new(M3ErrorCode::StoreUnavailable)
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
        .map_err(|_| unavailable())
}

fn existing_response<T: DeserializeOwned>(
    transaction: &Transaction<'_>,
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
        .map_err(|_| unavailable())?;
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
        .map_err(|_| unavailable())?;
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
        .map_err(|_| unavailable())?;
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

fn semantic_project(mut observation: ProjectObservation) -> ProjectObservation {
    observation.observed_at_ms = 0;
    observation
}

fn project_type(value: alcomd_application::ProjectType) -> &'static str {
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
            "SELECT snapshot_json, revision, registered_at_ms, observed_at_ms
             FROM projects WHERE owner_principal_id=?1 AND project_id=?2",
            params![owner.as_str(), id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )
        .optional()
        .map_err(|_| unavailable())?
        .ok_or_else(|| M3Error::new(M3ErrorCode::ProjectNotRegistered))?;
    let mut observation: ProjectObservation = parse_json(&row.0)?;
    observation.observed_at_ms = u64::try_from(row.3).map_err(|_| internal())?;
    Ok(ProjectRecord {
        project_id: id,
        observation,
        revision: revision(row.1)?,
        registered_at_ms: u64::try_from(row.2).map_err(|_| internal())?,
    })
}

pub(super) fn register_project(
    connection: &mut Connection,
    owner: &PrincipalId,
    observation: ProjectObservation,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<SyncWrite<ProjectRecord>, M3Error> {
    let fingerprint = project_fingerprint(&observation)?;
    let transaction = begin(connection)?;
    if let Some(mut response) = existing_response::<SyncWrite<ProjectRecord>>(
        &transaction,
        owner,
        PROJECT_REGISTER,
        key,
        &fingerprint,
    )? {
        response.replayed = true;
        transaction.commit().map_err(|_| unavailable())?;
        return Ok(response);
    }
    let duplicate = transaction
        .query_row(
            "SELECT 1 FROM projects WHERE path_identity_key=?1",
            params![observation.path_identity_key],
            |_| Ok(()),
        )
        .optional()
        .map_err(|_| unavailable())?
        .is_some();
    if duplicate {
        return Err(M3Error::new(M3ErrorCode::ProjectAlreadyRegistered));
    }
    let id = ProjectId::new();
    let semantic = semantic_project(observation.clone());
    transaction
        .execute(
            "INSERT INTO projects (
                project_id, owner_principal_id, root_path, path_identity_key, project_type,
                unity_version, unity_revision, snapshot_json, revision, registered_at_ms,
                observed_at_ms, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?10, ?9)",
            params![
                id.to_string(),
                owner.as_str(),
                observation.root_path,
                observation.path_identity_key,
                project_type(observation.project_type),
                observation.unity_version,
                observation.unity_revision,
                json(&semantic)?,
                integer(now_ms)?,
                integer(observation.observed_at_ms)?
            ],
        )
        .map_err(|_| unavailable())?;
    let record = ProjectRecord {
        project_id: id,
        observation,
        revision: Revision::INITIAL,
        registered_at_ms: now_ms,
    };
    insert_event(
        &transaction,
        owner,
        "project",
        &id.to_string(),
        Revision::INITIAL,
        "project.registered",
        now_ms,
    )?;
    let response = SyncWrite {
        value: record,
        replayed: false,
    };
    save_response(
        &transaction,
        owner,
        PROJECT_REGISTER,
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(|_| unavailable())?;
    Ok(response)
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
        .map_err(|_| unavailable())?;
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
        .map_err(|_| unavailable())?;
    let mut projects = Vec::new();
    for row in rows {
        let id = ProjectId::parse(&row.map_err(|_| unavailable())?).map_err(|_| internal())?;
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
            })
        })
        .flatten();
    Ok(ProjectPage {
        projects,
        next_cursor,
    })
}

pub(super) fn refresh_project(
    connection: &mut Connection,
    owner: &PrincipalId,
    id: ProjectId,
    expected: Revision,
    observation: ProjectObservation,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<SyncWrite<ProjectRecord>, M3Error> {
    let fingerprint = aggregate_fingerprint(id, expected)?;
    let transaction = begin(connection)?;
    if let Some(mut response) = existing_response::<SyncWrite<ProjectRecord>>(
        &transaction,
        owner,
        PROJECT_REFRESH,
        key,
        &fingerprint,
    )? {
        response.replayed = true;
        transaction.commit().map_err(|_| unavailable())?;
        return Ok(response);
    }
    let current = load_project(&transaction, owner, id)?;
    if current.revision != expected {
        return Err(M3Error::new(M3ErrorCode::RevisionConflict));
    }
    if current.observation.path_identity_key != observation.path_identity_key {
        return Err(M3Error::new(M3ErrorCode::ProjectNotFound));
    }
    let changed =
        semantic_project(current.observation.clone()) != semantic_project(observation.clone());
    let next_revision = if changed {
        current.revision.checked_next().ok_or_else(internal)?
    } else {
        current.revision
    };
    transaction
        .execute(
            "UPDATE projects SET root_path=?1, project_type=?2, unity_version=?3,
                unity_revision=?4, snapshot_json=?5, revision=?6, observed_at_ms=?7,
                updated_at_ms=?8 WHERE owner_principal_id=?9 AND project_id=?10",
            params![
                observation.root_path,
                project_type(observation.project_type),
                observation.unity_version,
                observation.unity_revision,
                json(&semantic_project(observation.clone()))?,
                integer(next_revision.get())?,
                integer(observation.observed_at_ms)?,
                integer(now_ms)?,
                owner.as_str(),
                id.to_string()
            ],
        )
        .map_err(|_| unavailable())?;
    if changed {
        insert_event(
            &transaction,
            owner,
            "project",
            &id.to_string(),
            next_revision,
            "project.refreshed",
            now_ms,
        )?;
    }
    let record = ProjectRecord {
        project_id: id,
        observation,
        revision: next_revision,
        registered_at_ms: current.registered_at_ms,
    };
    let response = SyncWrite {
        value: record,
        replayed: false,
    };
    save_response(
        &transaction,
        owner,
        PROJECT_REFRESH,
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(|_| unavailable())?;
    Ok(response)
}

pub(super) fn unregister_project(
    connection: &mut Connection,
    owner: &PrincipalId,
    id: ProjectId,
    expected: Revision,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<UnregisterResult<ProjectId>, M3Error> {
    let fingerprint = aggregate_fingerprint(id, expected)?;
    let transaction = begin(connection)?;
    if let Some(mut response) = existing_response::<UnregisterResult<ProjectId>>(
        &transaction,
        owner,
        PROJECT_UNREGISTER,
        key,
        &fingerprint,
    )? {
        response.replayed = true;
        transaction.commit().map_err(|_| unavailable())?;
        return Ok(response);
    }
    let current = load_project(&transaction, owner, id)?;
    if current.revision != expected {
        return Err(M3Error::new(M3ErrorCode::RevisionConflict));
    }
    let next = current.revision.checked_next().ok_or_else(internal)?;
    transaction
        .execute(
            "DELETE FROM projects WHERE owner_principal_id=?1 AND project_id=?2",
            params![owner.as_str(), id.to_string()],
        )
        .map_err(|_| unavailable())?;
    insert_event(
        &transaction,
        owner,
        "project",
        &id.to_string(),
        next,
        "project.unregistered",
        now_ms,
    )?;
    let response = UnregisterResult {
        id,
        revision: next,
        replayed: false,
    };
    save_response(
        &transaction,
        owner,
        PROJECT_UNREGISTER,
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(|_| unavailable())?;
    Ok(response)
}

fn repository_fingerprint(observation: &RepositoryObservation) -> Result<String, M3Error> {
    json(&serde_json::json!({"source": observation.source, "version": 1}))
}

fn semantic_repository(mut observation: RepositoryObservation) -> RepositoryObservation {
    observation.validators = RepositoryValidators::default();
    observation.refreshed_at_ms = 0;
    observation
}

fn source_parts(source: &RepositorySource) -> (&'static str, &str) {
    match source {
        RepositorySource::Local { path } => ("local", path),
        RepositorySource::Remote { url } => ("remote", url),
    }
}

fn load_packages(
    connection: &Connection,
    id: RepositoryId,
) -> Result<Vec<RepositoryPackageVersion>, M3Error> {
    let mut statement = connection
        .prepare(
            "SELECT package_id, version_text, display_name, description, yanked, unity_text
         FROM repository_package_versions WHERE repository_id=?1
         ORDER BY package_id ASC, version_text ASC",
        )
        .map_err(|_| unavailable())?;
    let rows = statement
        .query_map(params![id.to_string()], |row| {
            Ok(RepositoryPackageVersion {
                package_id: row.get(0)?,
                version: row.get(1)?,
                display_name: row.get(2)?,
                description: row.get(3)?,
                yanked: row.get::<_, i64>(4)? != 0,
                unity: row.get(5)?,
            })
        })
        .map_err(|_| unavailable())?;
    let mut packages = Vec::new();
    for row in rows {
        packages.push(row.map_err(|_| unavailable())?);
    }
    Ok(packages)
}

fn load_repository(
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
    );
    let row: Row = connection.query_row(
        "SELECT source_kind, source_locator, source_identity_key, declared_id, name, declared_url,
                etag, last_modified, issues_json, revision, registered_at_ms, refreshed_at_ms
         FROM repositories WHERE owner_principal_id=?1 AND repository_id=?2",
        params![owner.as_str(), id.to_string()],
        |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?,row.get(9)?,row.get(10)?,row.get(11)?)),
    ).optional().map_err(|_| unavailable())?.ok_or_else(|| M3Error::new(M3ErrorCode::RepositoryNotRegistered))?;
    let source = match row.0.as_str() {
        "local" => RepositorySource::Local { path: row.1 },
        "remote" => RepositorySource::Remote { url: row.1 },
        _ => return Err(internal()),
    };
    Ok(RepositoryRecord {
        repository_id: id,
        observation: RepositoryObservation {
            source,
            source_identity_key: row.2,
            declared_id: row.3,
            name: row.4,
            declared_url: row.5,
            issues: parse_json(&row.8)?,
            packages: load_packages(connection, id)?,
            validators: RepositoryValidators {
                etag: row.6,
                last_modified: row.7,
            },
            refreshed_at_ms: u64::try_from(row.11).map_err(|_| internal())?,
        },
        revision: revision(row.9)?,
        registered_at_ms: u64::try_from(row.10).map_err(|_| internal())?,
    })
}

fn replace_packages(
    transaction: &Transaction<'_>,
    id: RepositoryId,
    packages: &[RepositoryPackageVersion],
) -> Result<(), M3Error> {
    transaction
        .execute(
            "DELETE FROM repository_package_versions WHERE repository_id=?1",
            params![id.to_string()],
        )
        .map_err(|_| unavailable())?;
    for package in packages {
        transaction.execute(
            "INSERT INTO repository_package_versions (
                repository_id, package_id, version_text, display_name, description, yanked, unity_text
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![id.to_string(), package.package_id, package.version, package.display_name, package.description, i64::from(package.yanked), package.unity],
        ).map_err(|_| unavailable())?;
    }
    Ok(())
}

fn insert_repository(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    id: RepositoryId,
    observation: &RepositoryObservation,
    revision: Revision,
    registered_at_ms: u64,
    now_ms: u64,
) -> Result<(), M3Error> {
    let (kind, locator) = source_parts(&observation.source);
    transaction
        .execute(
            "INSERT INTO repositories (
            repository_id, owner_principal_id, source_kind, source_locator, source_identity_key,
            declared_id, name, declared_url, etag, last_modified, issues_json, revision,
            registered_at_ms, refreshed_at_ms, updated_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
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
                integer(registered_at_ms)?,
                integer(observation.refreshed_at_ms)?,
                integer(now_ms)?
            ],
        )
        .map_err(|_| unavailable())?;
    replace_packages(transaction, id, &observation.packages)
}

pub(super) fn register_repository(
    connection: &mut Connection,
    owner: &PrincipalId,
    observation: RepositoryObservation,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<SyncWrite<RepositoryRecord>, M3Error> {
    let fingerprint = repository_fingerprint(&observation)?;
    let transaction = begin(connection)?;
    if let Some(mut response) = existing_response::<SyncWrite<RepositoryRecord>>(
        &transaction,
        owner,
        REPOSITORY_REGISTER,
        key,
        &fingerprint,
    )? {
        response.replayed = true;
        transaction.commit().map_err(|_| unavailable())?;
        return Ok(response);
    }
    let duplicate = transaction
        .query_row(
            "SELECT 1 FROM repositories WHERE source_identity_key=?1",
            params![observation.source_identity_key],
            |_| Ok(()),
        )
        .optional()
        .map_err(|_| unavailable())?
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
        now_ms,
        now_ms,
    )?;
    let record = RepositoryRecord {
        repository_id: id,
        observation,
        revision: Revision::INITIAL,
        registered_at_ms: now_ms,
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
    let response = SyncWrite {
        value: record,
        replayed: false,
    };
    save_response(
        &transaction,
        owner,
        REPOSITORY_REGISTER,
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(|_| unavailable())?;
    Ok(response)
}

pub(super) fn get_repository(
    connection: &Connection,
    owner: &PrincipalId,
    id: RepositoryId,
) -> Result<RepositoryRecord, M3Error> {
    load_repository(connection, owner, id)
}

pub(super) fn list_repositories(
    connection: &Connection,
    owner: &PrincipalId,
    cursor: Option<RegistryCursor<RepositoryId>>,
    limit: u32,
) -> Result<RepositoryPage, M3Error> {
    let mut statement = connection
        .prepare(
            "SELECT repository_id FROM repositories WHERE owner_principal_id=?1 AND
         (?2 IS NULL OR registered_at_ms < ?2 OR (registered_at_ms = ?2 AND repository_id < ?3))
         ORDER BY registered_at_ms DESC, repository_id DESC LIMIT ?4",
        )
        .map_err(|_| unavailable())?;
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
        .map_err(|_| unavailable())?;
    let mut repositories = Vec::new();
    for row in rows {
        let id = RepositoryId::parse(&row.map_err(|_| unavailable())?).map_err(|_| internal())?;
        repositories.push(load_repository(connection, owner, id)?);
    }
    let has_more = repositories.len() > limit as usize;
    if has_more {
        repositories.pop();
    }
    let next_cursor = has_more
        .then(|| {
            repositories.last().map(|record| RegistryCursor {
                registered_at_ms: record.registered_at_ms,
                id: record.repository_id,
            })
        })
        .flatten();
    Ok(RepositoryPage {
        repositories,
        next_cursor,
    })
}

pub(super) fn list_repository_packages(
    connection: &Connection,
    owner: &PrincipalId,
    id: RepositoryId,
    cursor: Option<PackageCursor>,
    limit: u32,
) -> Result<PackagePage, M3Error> {
    let _ = load_repository(connection, owner, id)?;
    let mut statement = connection
        .prepare(
            "SELECT package_id, version_text, display_name, description, yanked, unity_text
         FROM repository_package_versions WHERE repository_id=?1 AND
         (?2 IS NULL OR package_id > ?2 OR (package_id = ?2 AND version_text > ?3))
         ORDER BY package_id ASC, version_text ASC LIMIT ?4",
        )
        .map_err(|_| unavailable())?;
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
                    package_id: row.get(0)?,
                    version: row.get(1)?,
                    display_name: row.get(2)?,
                    description: row.get(3)?,
                    yanked: row.get::<_, i64>(4)? != 0,
                    unity: row.get(5)?,
                })
            },
        )
        .map_err(|_| unavailable())?;
    let mut packages = Vec::new();
    for row in rows {
        packages.push(row.map_err(|_| unavailable())?);
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

pub(super) fn refresh_repository(
    connection: &mut Connection,
    owner: &PrincipalId,
    id: RepositoryId,
    expected: Revision,
    observation: RepositoryObservation,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<SyncWrite<RepositoryRecord>, M3Error> {
    let fingerprint = aggregate_fingerprint(id, expected)?;
    let transaction = begin(connection)?;
    if let Some(mut response) = existing_response::<SyncWrite<RepositoryRecord>>(
        &transaction,
        owner,
        REPOSITORY_REFRESH,
        key,
        &fingerprint,
    )? {
        response.replayed = true;
        transaction.commit().map_err(|_| unavailable())?;
        return Ok(response);
    }
    let current = load_repository(&transaction, owner, id)?;
    if current.revision != expected {
        return Err(M3Error::new(M3ErrorCode::RevisionConflict));
    }
    if current.observation.source_identity_key != observation.source_identity_key {
        return Err(M3Error::new(M3ErrorCode::RepositorySourceInvalid));
    }
    let changed = semantic_repository(current.observation.clone())
        != semantic_repository(observation.clone());
    let next = if changed {
        current.revision.checked_next().ok_or_else(internal)?
    } else {
        current.revision
    };
    transaction
        .execute(
            "DELETE FROM repositories WHERE owner_principal_id=?1 AND repository_id=?2",
            params![owner.as_str(), id.to_string()],
        )
        .map_err(|_| unavailable())?;
    insert_repository(
        &transaction,
        owner,
        id,
        &observation,
        next,
        current.registered_at_ms,
        now_ms,
    )?;
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
    }
    let record = RepositoryRecord {
        repository_id: id,
        observation,
        revision: next,
        registered_at_ms: current.registered_at_ms,
    };
    let response = SyncWrite {
        value: record,
        replayed: false,
    };
    save_response(
        &transaction,
        owner,
        REPOSITORY_REFRESH,
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(|_| unavailable())?;
    Ok(response)
}

pub(super) fn update_repository_validators(
    connection: &mut Connection,
    owner: &PrincipalId,
    id: RepositoryId,
    expected: Revision,
    validators: RepositoryValidators,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<SyncWrite<RepositoryRecord>, M3Error> {
    let fingerprint = aggregate_fingerprint(id, expected)?;
    let transaction = begin(connection)?;
    if let Some(mut response) = existing_response::<SyncWrite<RepositoryRecord>>(
        &transaction,
        owner,
        REPOSITORY_REFRESH,
        key,
        &fingerprint,
    )? {
        response.replayed = true;
        transaction.commit().map_err(|_| unavailable())?;
        return Ok(response);
    }
    let mut current = load_repository(&transaction, owner, id)?;
    if current.revision != expected {
        return Err(M3Error::new(M3ErrorCode::RevisionConflict));
    }
    current.observation.validators = validators.clone();
    current.observation.refreshed_at_ms = now_ms;
    transaction.execute("UPDATE repositories SET etag=?1, last_modified=?2, refreshed_at_ms=?3, updated_at_ms=?3 WHERE owner_principal_id=?4 AND repository_id=?5",
        params![validators.etag, validators.last_modified, integer(now_ms)?, owner.as_str(), id.to_string()]).map_err(|_| unavailable())?;
    let response = SyncWrite {
        value: current,
        replayed: false,
    };
    save_response(
        &transaction,
        owner,
        REPOSITORY_REFRESH,
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(|_| unavailable())?;
    Ok(response)
}

pub(super) fn unregister_repository(
    connection: &mut Connection,
    owner: &PrincipalId,
    id: RepositoryId,
    expected: Revision,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<UnregisterResult<RepositoryId>, M3Error> {
    let fingerprint = aggregate_fingerprint(id, expected)?;
    let transaction = begin(connection)?;
    if let Some(mut response) = existing_response::<UnregisterResult<RepositoryId>>(
        &transaction,
        owner,
        REPOSITORY_UNREGISTER,
        key,
        &fingerprint,
    )? {
        response.replayed = true;
        transaction.commit().map_err(|_| unavailable())?;
        return Ok(response);
    }
    let current = load_repository(&transaction, owner, id)?;
    if current.revision != expected {
        return Err(M3Error::new(M3ErrorCode::RevisionConflict));
    }
    let next = current.revision.checked_next().ok_or_else(internal)?;
    transaction
        .execute(
            "DELETE FROM repositories WHERE owner_principal_id=?1 AND repository_id=?2",
            params![owner.as_str(), id.to_string()],
        )
        .map_err(|_| unavailable())?;
    insert_event(
        &transaction,
        owner,
        "repository",
        &id.to_string(),
        next,
        "repository.unregistered",
        now_ms,
    )?;
    let response = UnregisterResult {
        id,
        revision: next,
        replayed: false,
    };
    save_response(
        &transaction,
        owner,
        REPOSITORY_UNREGISTER,
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(|_| unavailable())?;
    Ok(response)
}
