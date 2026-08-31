use alcomd_application::{
    IdempotencyKey, M5UnityError, M5UnityErrorCode, PrincipalId, ProjectId, ProjectRecord,
    ProjectUnityLaunchConfig, Revision, UnityArchitecture, UnityInstallationCursor,
    UnityInstallationId, UnityInstallationObservation, UnityInstallationPage,
    UnityInstallationRecord, UnityLaunchId, UnityLaunchRecord, UnityLaunchState, UnitySourceKind,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::Serialize;
use uuid::Uuid;

#[derive(serde::Deserialize, Serialize)]
struct LaunchConfigMutationRecord {
    config: ProjectUnityLaunchConfig,
    changed: bool,
}

pub(super) fn register_installation(
    connection: &mut Connection,
    owner: &PrincipalId,
    observation: UnityInstallationObservation,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<(UnityInstallationRecord, bool), M5UnityError> {
    let transaction = transaction(connection)?;
    let fingerprint = installation_fingerprint(&observation);
    if let Some(record) = replay::<UnityInstallationRecord>(
        &transaction,
        owner,
        "unity.installations.register",
        key,
        &fingerprint,
    )? {
        transaction.commit().map_err(failure)?;
        return Ok((record, true));
    }
    let existing =
        load_installation_by_identity(&transaction, owner, &observation.filesystem_identity)?;
    let record = if let Some(record) = existing {
        record
    } else {
        let id = UnityInstallationId::new();
        transaction
            .execute(
                "INSERT INTO unity_installations (
                    installation_id, owner_principal_id, executable_path,
                    filesystem_identity_key, unity_version, architecture, source_kind,
                    revision, observed_at_ms, updated_at_ms
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7,1,?8,?9)",
                params![
                    id.to_string(),
                    owner.as_str(),
                    observation.executable_path,
                    observation.filesystem_identity,
                    observation.unity_version,
                    architecture_name(observation.architecture),
                    source_name(observation.source_kind),
                    integer(observation.observed_at_ms)?,
                    integer(now_ms)?,
                ],
            )
            .map_err(failure)?;
        insert_event(
            &transaction,
            owner,
            "unity.installation.registered",
            "unity-installation",
            &id.to_string(),
            Revision::INITIAL,
            now_ms,
        )?;
        load_installation(&transaction, owner, id)?
    };
    save_response(
        &transaction,
        owner,
        "unity.installations.register",
        key,
        &fingerprint,
        &record,
        now_ms,
    )?;
    transaction.commit().map_err(failure)?;
    Ok((record, false))
}

pub(super) fn get_installation(
    connection: &Connection,
    owner: &PrincipalId,
    id: UnityInstallationId,
) -> Result<UnityInstallationRecord, M5UnityError> {
    load_installation(connection, owner, id)
}

pub(super) fn list_installations(
    connection: &Connection,
    owner: &PrincipalId,
    cursor: Option<UnityInstallationCursor>,
    limit: u32,
) -> Result<UnityInstallationPage, M5UnityError> {
    let (cursor_time, cursor_id) = cursor.map_or((i64::MAX, "~".to_owned()), |cursor| {
        (
            i64::try_from(cursor.updated_at_ms).unwrap_or(i64::MAX),
            cursor.installation_id.to_string(),
        )
    });
    let mut statement = connection
        .prepare(
            "SELECT installation_id FROM unity_installations
             WHERE owner_principal_id=?1
               AND (updated_at_ms<?2 OR (updated_at_ms=?2 AND installation_id<?3))
             ORDER BY updated_at_ms DESC, installation_id DESC LIMIT ?4",
        )
        .map_err(failure)?;
    let ids = statement
        .query_map(
            params![owner.as_str(), cursor_time, cursor_id, i64::from(limit) + 1],
            |row| row.get::<_, String>(0),
        )
        .map_err(failure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(failure)?;
    let mut installations = ids
        .into_iter()
        .map(|id| {
            UnityInstallationId::parse(&id)
                .map_err(|_| corrupt())
                .and_then(|id| load_installation(connection, owner, id))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let has_more = installations.len() > limit as usize;
    if has_more {
        installations.pop();
    }
    let next_cursor = has_more.then(|| {
        let last = installations.last().expect("non-empty page with extra row");
        UnityInstallationCursor {
            updated_at_ms: last.updated_at_ms,
            installation_id: last.installation_id,
        }
    });
    Ok(UnityInstallationPage {
        installations,
        next_cursor,
    })
}

pub(super) fn remove_installation(
    connection: &mut Connection,
    owner: &PrincipalId,
    id: UnityInstallationId,
    expected: Revision,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<(bool, bool), M5UnityError> {
    let transaction = transaction(connection)?;
    let fingerprint = format!(
        r#"{{"expectedRevision":{},"installationId":"{}","version":1}}"#,
        expected.get(),
        id
    );
    if let Some(value) = replay::<bool>(
        &transaction,
        owner,
        "unity.installations.remove",
        key,
        &fingerprint,
    )? {
        transaction.commit().map_err(failure)?;
        return Ok((value, true));
    }
    let record = load_installation(&transaction, owner, id)?;
    if record.revision != expected {
        return Err(error(M5UnityErrorCode::RevisionConflict));
    }
    transaction
        .execute(
            "DELETE FROM unity_installations WHERE installation_id=?1 AND owner_principal_id=?2",
            params![id.to_string(), owner.as_str()],
        )
        .map_err(|_| error(M5UnityErrorCode::InstallationInUse))?;
    let next = expected.checked_next().ok_or_else(corrupt)?;
    insert_event(
        &transaction,
        owner,
        "unity.installation.removed",
        "unity-installation",
        &id.to_string(),
        next,
        now_ms,
    )?;
    save_response(
        &transaction,
        owner,
        "unity.installations.remove",
        key,
        &fingerprint,
        &true,
        now_ms,
    )?;
    transaction.commit().map_err(failure)?;
    Ok((true, false))
}

pub(super) fn synchronize_installations(
    connection: &mut Connection,
    owner: &PrincipalId,
    observations: Vec<UnityInstallationObservation>,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<(UnityInstallationPage, bool), M5UnityError> {
    let transaction = transaction(connection)?;
    let fingerprint = discovery_fingerprint(&observations);
    if let Some(page) = replay::<UnityInstallationPage>(
        &transaction,
        owner,
        "unity.installations.refresh",
        key,
        &fingerprint,
    )? {
        transaction.commit().map_err(failure)?;
        return Ok((page, true));
    }
    for observation in observations {
        if load_installation_by_identity(&transaction, owner, &observation.filesystem_identity)?
            .is_none()
        {
            let id = UnityInstallationId::new();
            transaction
                .execute(
                    "INSERT INTO unity_installations (
                        installation_id, owner_principal_id, executable_path,
                        filesystem_identity_key, unity_version, architecture, source_kind,
                        revision, observed_at_ms, updated_at_ms
                     ) VALUES (?1,?2,?3,?4,?5,?6,?7,1,?8,?9)",
                    params![
                        id.to_string(),
                        owner.as_str(),
                        observation.executable_path,
                        observation.filesystem_identity,
                        observation.unity_version,
                        architecture_name(observation.architecture),
                        source_name(observation.source_kind),
                        integer(observation.observed_at_ms)?,
                        integer(now_ms)?,
                    ],
                )
                .map_err(failure)?;
            insert_event(
                &transaction,
                owner,
                "unity.installation.discovered",
                "unity-installation",
                &id.to_string(),
                Revision::INITIAL,
                now_ms,
            )?;
        }
    }
    let page = list_installations(&transaction, owner, None, 1_000)?;
    save_response(
        &transaction,
        owner,
        "unity.installations.refresh",
        key,
        &fingerprint,
        &page,
        now_ms,
    )?;
    transaction.commit().map_err(failure)?;
    Ok((page, false))
}

pub(super) fn get_project_launch_config(
    connection: &Connection,
    owner: &PrincipalId,
    project_id: ProjectId,
) -> Result<ProjectUnityLaunchConfig, M5UnityError> {
    let registered = connection
        .query_row(
            "SELECT 1 FROM projects WHERE project_id=?1 AND owner_principal_id=?2",
            params![project_id.to_string(), owner.as_str()],
            |_| Ok(()),
        )
        .optional()
        .map_err(failure)?
        .is_some();
    if !registered {
        return Err(error(M5UnityErrorCode::ProjectNotRegistered));
    }
    let row = connection
        .query_row(
            "SELECT arguments_json,revision,updated_at_ms
             FROM project_unity_launch_config WHERE project_id=?1",
            params![project_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()
        .map_err(failure)?;
    let Some((arguments, revision, updated_at_ms)) = row else {
        return Ok(ProjectUnityLaunchConfig {
            project_id,
            arguments: Vec::new(),
            revision: None,
            updated_at_ms: 0,
        });
    };
    Ok(ProjectUnityLaunchConfig {
        project_id,
        arguments: serde_json::from_str(&arguments).map_err(|_| corrupt())?,
        revision: Some(parse_revision(revision)?),
        updated_at_ms: parse_u64(updated_at_ms)?,
    })
}

pub(super) fn set_project_launch_config(
    connection: &mut Connection,
    owner: &PrincipalId,
    project_id: ProjectId,
    arguments: Vec<String>,
    expected: Option<Revision>,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<(ProjectUnityLaunchConfig, bool, bool), M5UnityError> {
    let transaction = transaction(connection)?;
    let arguments_json = serde_json::to_string(&arguments).map_err(|_| corrupt())?;
    let fingerprint = format!(
        r#"{{"arguments":{},"expectedRevision":{},"projectId":"{}","version":1}}"#,
        arguments_json,
        expected.map_or(0, Revision::get),
        project_id
    );
    if let Some(record) = replay::<LaunchConfigMutationRecord>(
        &transaction,
        owner,
        "unity.projectLaunchConfig.set",
        key,
        &fingerprint,
    )? {
        transaction.commit().map_err(failure)?;
        return Ok((record.config, record.changed, true));
    }
    let current = get_project_launch_config(&transaction, owner, project_id)?;
    match (current.revision, expected) {
        (None, None) => {}
        (Some(current), Some(expected)) if current == expected => {}
        _ => return Err(error(M5UnityErrorCode::RevisionConflict)),
    }
    if current.arguments == arguments {
        let response = LaunchConfigMutationRecord {
            config: current,
            changed: false,
        };
        save_response(
            &transaction,
            owner,
            "unity.projectLaunchConfig.set",
            key,
            &fingerprint,
            &response,
            now_ms,
        )?;
        transaction.commit().map_err(failure)?;
        return Ok((response.config, false, false));
    }
    let revision = current
        .revision
        .map_or(Some(Revision::INITIAL), Revision::checked_next)
        .ok_or_else(corrupt)?;
    transaction
        .execute(
            "INSERT INTO project_unity_launch_config (
                project_id,arguments_json,revision,updated_at_ms
             ) VALUES (?1,?2,?3,?4)
             ON CONFLICT(project_id) DO UPDATE SET
                arguments_json=excluded.arguments_json,
                revision=excluded.revision,
                updated_at_ms=excluded.updated_at_ms",
            params![
                project_id.to_string(),
                arguments_json,
                i64::try_from(revision.get()).map_err(|_| corrupt())?,
                integer(now_ms)?
            ],
        )
        .map_err(failure)?;
    insert_event(
        &transaction,
        owner,
        "unity.project_launch_config_changed",
        "project-unity-launch-config",
        &project_id.to_string(),
        revision,
        now_ms,
    )?;
    let record = get_project_launch_config(&transaction, owner, project_id)?;
    let response = LaunchConfigMutationRecord {
        config: record,
        changed: true,
    };
    save_response(
        &transaction,
        owner,
        "unity.projectLaunchConfig.set",
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(failure)?;
    Ok((response.config, true, false))
}

pub(super) fn clear_project_launch_config(
    connection: &mut Connection,
    owner: &PrincipalId,
    project_id: ProjectId,
    expected: Option<Revision>,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<(ProjectUnityLaunchConfig, bool, bool), M5UnityError> {
    let transaction = transaction(connection)?;
    let fingerprint = format!(
        r#"{{"expectedRevision":{},"projectId":"{}","version":1}}"#,
        expected.map_or(0, Revision::get),
        project_id
    );
    if let Some(record) = replay::<LaunchConfigMutationRecord>(
        &transaction,
        owner,
        "unity.projectLaunchConfig.clear",
        key,
        &fingerprint,
    )? {
        transaction.commit().map_err(failure)?;
        return Ok((record.config, record.changed, true));
    }
    let current = get_project_launch_config(&transaction, owner, project_id)?;
    match (current.revision, expected) {
        (None, None) => {}
        (Some(revision), Some(expected)) if revision == expected => {}
        _ => return Err(error(M5UnityErrorCode::RevisionConflict)),
    }
    let (record, changed) = match current.revision {
        Some(revision) => {
            let next = revision.checked_next().ok_or_else(corrupt)?;
            transaction
                .execute(
                    "DELETE FROM project_unity_launch_config WHERE project_id=?1",
                    params![project_id.to_string()],
                )
                .map_err(failure)?;
            insert_event(
                &transaction,
                owner,
                "unity.project_launch_config_changed",
                "project-unity-launch-config",
                &project_id.to_string(),
                next,
                now_ms,
            )?;
            (
                ProjectUnityLaunchConfig {
                    project_id,
                    arguments: Vec::new(),
                    revision: Some(next),
                    updated_at_ms: now_ms,
                },
                true,
            )
        }
        None => (current, false),
    };
    let response = LaunchConfigMutationRecord {
        config: record,
        changed,
    };
    save_response(
        &transaction,
        owner,
        "unity.projectLaunchConfig.clear",
        key,
        &fingerprint,
        &response,
        now_ms,
    )?;
    transaction.commit().map_err(failure)?;
    Ok((response.config, response.changed, false))
}

pub(super) fn accept_launch(
    connection: &mut Connection,
    owner: &PrincipalId,
    project: ProjectRecord,
    config: ProjectUnityLaunchConfig,
    installation_id: UnityInstallationId,
    key: &IdempotencyKey,
    now_ms: u64,
) -> Result<(UnityLaunchRecord, bool), M5UnityError> {
    let transaction = transaction(connection)?;
    let fingerprint = launch_fingerprint(&project, &config, installation_id);
    if let Some(record) =
        replay::<UnityLaunchRecord>(&transaction, owner, "unity.launch", key, &fingerprint)?
    {
        transaction.commit().map_err(failure)?;
        return Ok((record, true));
    }
    let record = UnityLaunchRecord {
        launch_id: UnityLaunchId::new(),
        project_id: project.project_id,
        installation_id,
        state: UnityLaunchState::Opening,
        spawn_accepted: false,
        created_at_ms: now_ms,
    };
    save_response(
        &transaction,
        owner,
        "unity.launch",
        key,
        &fingerprint,
        &record,
        now_ms,
    )?;
    transaction.commit().map_err(failure)?;
    Ok((record, false))
}

pub(super) fn replay_launch(
    connection: &Connection,
    owner: &PrincipalId,
    project: &ProjectRecord,
    config: &ProjectUnityLaunchConfig,
    installation_id: UnityInstallationId,
    key: &IdempotencyKey,
) -> Result<Option<UnityLaunchRecord>, M5UnityError> {
    let fingerprint = launch_fingerprint(project, config, installation_id);
    let existing = connection
        .query_row(
            "SELECT request_fingerprint,response_json FROM idempotency_records
             WHERE principal_id=?1 AND method='unity.launch' AND idempotency_key=?2",
            params![owner.as_str(), key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(failure)?;
    match existing {
        Some((stored, _)) if stored != fingerprint => {
            Err(error(M5UnityErrorCode::IdempotencyConflict))
        }
        Some((_, response)) => serde_json::from_str(&response)
            .map(Some)
            .map_err(|_| corrupt()),
        None => Ok(None),
    }
}

fn launch_fingerprint(
    project: &ProjectRecord,
    config: &ProjectUnityLaunchConfig,
    installation_id: UnityInstallationId,
) -> String {
    format!(
        r#"{{"installationId":"{}","launchConfigRevision":{},"projectId":"{}","projectRevision":{},"version":3}}"#,
        installation_id,
        config.revision.map_or(0, Revision::get),
        project.project_id,
        project.revision.get()
    )
}

pub(super) fn get_launch(
    connection: &Connection,
    owner: &PrincipalId,
    launch_id: UnityLaunchId,
) -> Result<UnityLaunchRecord, M5UnityError> {
    connection
        .query_row(
            "SELECT response_json FROM idempotency_records
             WHERE principal_id=?1 AND method='unity.launch'
               AND json_extract(response_json,'$.launch_id')=?2",
            params![owner.as_str(), launch_id.to_string()],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(failure)?
        .ok_or_else(|| error(M5UnityErrorCode::LaunchNotFound))
        .and_then(|json| serde_json::from_str(&json).map_err(|_| corrupt()))
}

pub(super) fn set_launch_state(
    connection: &mut Connection,
    owner: &PrincipalId,
    launch_id: UnityLaunchId,
    state: UnityLaunchState,
    spawn_accepted: bool,
) -> Result<UnityLaunchRecord, M5UnityError> {
    let transaction = transaction(connection)?;
    let mut record = get_launch(&transaction, owner, launch_id)?;
    record.state = state;
    record.spawn_accepted = spawn_accepted;
    let response = serde_json::to_string(&record).map_err(|_| corrupt())?;
    transaction
        .execute(
            "UPDATE idempotency_records SET response_json=?1
             WHERE principal_id=?2 AND method='unity.launch'
               AND json_extract(response_json,'$.launch_id')=?3",
            params![response, owner.as_str(), launch_id.to_string()],
        )
        .map_err(failure)?;
    transaction.commit().map_err(failure)?;
    Ok(record)
}

fn load_installation(
    connection: &Connection,
    owner: &PrincipalId,
    id: UnityInstallationId,
) -> Result<UnityInstallationRecord, M5UnityError> {
    connection
        .query_row(
            "SELECT executable_path,filesystem_identity_key,unity_version,architecture,
                    source_kind,revision,observed_at_ms,updated_at_ms
             FROM unity_installations WHERE installation_id=?1 AND owner_principal_id=?2",
            params![id.to_string(), owner.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                ))
            },
        )
        .optional()
        .map_err(failure)?
        .ok_or_else(|| error(M5UnityErrorCode::InstallationNotFound))
        .and_then(|row| {
            Ok(UnityInstallationRecord {
                installation_id: id,
                observation: UnityInstallationObservation {
                    executable_path: row.0,
                    filesystem_identity: row.1,
                    unity_version: row.2,
                    architecture: parse_architecture(&row.3)?,
                    source_kind: parse_source(&row.4)?,
                    observed_at_ms: parse_u64(row.6)?,
                },
                revision: parse_revision(row.5)?,
                updated_at_ms: parse_u64(row.7)?,
            })
        })
}

fn load_installation_by_identity(
    connection: &Connection,
    owner: &PrincipalId,
    identity: &[u8],
) -> Result<Option<UnityInstallationRecord>, M5UnityError> {
    let id = connection
        .query_row(
            "SELECT installation_id FROM unity_installations
             WHERE owner_principal_id=?1 AND filesystem_identity_key=?2",
            params![owner.as_str(), identity],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(failure)?;
    id.map(|value| {
        UnityInstallationId::parse(&value)
            .map_err(|_| corrupt())
            .and_then(|id| load_installation(connection, owner, id))
    })
    .transpose()
}

fn replay<T: serde::de::DeserializeOwned>(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    method: &str,
    key: &IdempotencyKey,
    fingerprint: &str,
) -> Result<Option<T>, M5UnityError> {
    let existing = transaction
        .query_row(
            "SELECT request_fingerprint,response_json FROM idempotency_records
             WHERE principal_id=?1 AND method=?2 AND idempotency_key=?3",
            params![owner.as_str(), method, key.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(failure)?;
    match existing {
        Some((existing, _)) if existing != fingerprint => {
            Err(error(M5UnityErrorCode::IdempotencyConflict))
        }
        Some((_, response)) => serde_json::from_str(&response)
            .map(Some)
            .map_err(|_| corrupt()),
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
) -> Result<(), M5UnityError> {
    let response = serde_json::to_string(response).map_err(|_| corrupt())?;
    transaction
        .execute(
            "INSERT INTO idempotency_records (
                principal_id,method,idempotency_key,request_fingerprint,state,
                operation_id,response_json,created_at_ms
             ) VALUES (?1,?2,?3,?4,'completed',NULL,?5,?6)",
            params![
                owner.as_str(),
                method,
                key.as_str(),
                fingerprint,
                response,
                integer(now_ms)?
            ],
        )
        .map_err(failure)?;
    Ok(())
}

fn insert_event(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    kind: &str,
    aggregate_kind: &str,
    aggregate_id: &str,
    revision: Revision,
    now_ms: u64,
) -> Result<(), M5UnityError> {
    transaction
        .execute(
            "INSERT INTO events (
                event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,
                principal_id,occurred_at_ms,payload_json
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,'{}')",
            params![
                Uuid::new_v4().to_string(),
                kind,
                aggregate_kind,
                aggregate_id,
                i64::try_from(revision.get()).map_err(|_| corrupt())?,
                owner.as_str(),
                integer(now_ms)?
            ],
        )
        .map_err(failure)?;
    Ok(())
}

fn installation_fingerprint(observation: &UnityInstallationObservation) -> String {
    format!(
        r#"{{"identity":"{}","source":"{}","version":1}}"#,
        hex(&observation.filesystem_identity),
        source_name(observation.source_kind)
    )
}

fn discovery_fingerprint(observations: &[UnityInstallationObservation]) -> String {
    let mut identities = observations
        .iter()
        .map(|value| hex(&value.filesystem_identity))
        .collect::<Vec<_>>();
    identities.sort();
    format!(
        r#"{{"identities":{},"version":1}}"#,
        serde_json::json!(identities)
    )
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(char::from(DIGITS[usize::from(byte >> 4)]));
        value.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    value
}

fn architecture_name(value: UnityArchitecture) -> &'static str {
    match value {
        UnityArchitecture::X86_64 => "x86_64",
        UnityArchitecture::Arm64 => "arm64",
        UnityArchitecture::Universal => "universal",
        UnityArchitecture::Unknown => "unknown",
    }
}

fn parse_architecture(value: &str) -> Result<UnityArchitecture, M5UnityError> {
    match value {
        "x86_64" => Ok(UnityArchitecture::X86_64),
        "arm64" => Ok(UnityArchitecture::Arm64),
        "universal" => Ok(UnityArchitecture::Universal),
        "unknown" => Ok(UnityArchitecture::Unknown),
        _ => Err(corrupt()),
    }
}

fn source_name(value: UnitySourceKind) -> &'static str {
    match value {
        UnitySourceKind::Manual => "manual",
        UnitySourceKind::HubConfig => "hub_config",
        UnitySourceKind::KnownInstallRoot => "known_install_root",
        UnitySourceKind::UnityCliHint => "unity_cli_hint",
    }
}

fn parse_source(value: &str) -> Result<UnitySourceKind, M5UnityError> {
    match value {
        "manual" => Ok(UnitySourceKind::Manual),
        "hub_config" => Ok(UnitySourceKind::HubConfig),
        "known_install_root" => Ok(UnitySourceKind::KnownInstallRoot),
        "unity_cli_hint" => Ok(UnitySourceKind::UnityCliHint),
        _ => Err(corrupt()),
    }
}

fn parse_revision(value: i64) -> Result<Revision, M5UnityError> {
    u64::try_from(value)
        .ok()
        .and_then(Revision::new)
        .ok_or_else(corrupt)
}

fn parse_u64(value: i64) -> Result<u64, M5UnityError> {
    u64::try_from(value).map_err(|_| corrupt())
}

fn integer(value: u64) -> Result<i64, M5UnityError> {
    i64::try_from(value).map_err(|_| corrupt())
}

fn transaction(connection: &mut Connection) -> Result<Transaction<'_>, M5UnityError> {
    connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(failure)
}

fn failure(_: rusqlite::Error) -> M5UnityError {
    unavailable()
}

pub(super) fn unavailable() -> M5UnityError {
    error(M5UnityErrorCode::StoreUnavailable)
}

fn corrupt() -> M5UnityError {
    error(M5UnityErrorCode::Internal)
}

fn error(code: M5UnityErrorCode) -> M5UnityError {
    M5UnityError::new(code)
}
