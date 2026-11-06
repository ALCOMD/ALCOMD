use std::collections::BTreeSet;

use alcomd_application::{
    LogTarget, PrincipalId, ProjectCreateError, ProjectCreateErrorCode, ProjectTemplateDeclaration,
    ProjectTemplatePage, ProjectTemplateRecord, ProjectTemplateRef,
};
use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};
use uuid::Uuid;

use super::operation_logs::{self, CommittedStep};
use super::project_create::{begin, db_error, integer, internal};

const MAX_DECLARATION_BYTES: usize = 24 * 1024;

pub(super) fn archive_references(
    connection: &Connection,
) -> Result<BTreeSet<String>, ProjectCreateError> {
    let mut statement = connection
        .prepare("SELECT DISTINCT json_extract(declaration_json,'$.archiveSha256') FROM project_templates")
        .map_err(db_error)?;
    let mut referenced = statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(db_error)?
        .collect::<Result<BTreeSet<_>, _>>()
        .map_err(db_error)?;
    super::project_create::add_archive_references(connection, &mut referenced)?;
    super::project_template_jobs::add_archive_references(connection, &mut referenced)?;
    Ok(referenced)
}

pub(super) fn list(
    connection: &Connection,
    owner: &PrincipalId,
    limit: usize,
    cursor: Option<String>,
) -> Result<ProjectTemplatePage, ProjectCreateError> {
    if limit == 0 || limit > 100 || cursor.as_ref().is_some_and(|value| value.len() > 128) {
        return Err(ProjectCreateError::new(
            ProjectCreateErrorCode::InvalidInput,
        ));
    }
    let sql_limit = i64::try_from(limit + 1).map_err(|_| internal())?;
    let mut statement = connection
        .prepare(
            "SELECT declaration_json, revision, locked, aggregate_id
             FROM project_templates
             WHERE owner_principal_id=?1 AND template_id>?2
             ORDER BY template_id LIMIT ?3",
        )
        .map_err(db_error)?;
    let mut templates = statement
        .query_map(
            params![owner.as_str(), cursor.as_deref().unwrap_or(""), sql_limit],
            decode,
        )
        .map_err(db_error)?
        .map(|row| row.map(|(record, _)| record))
        .collect::<Result<Vec<_>, _>>()
        .map_err(db_error)?;
    let next_cursor = if templates.len() > limit {
        templates.truncate(limit);
        templates.last().map(|record| record.declaration.id.clone())
    } else {
        None
    };
    Ok(ProjectTemplatePage {
        templates,
        next_cursor,
    })
}

pub(super) fn frozen(
    connection: &Connection,
    owner: &PrincipalId,
    reference: &ProjectTemplateRef,
) -> Result<ProjectTemplateDeclaration, ProjectCreateError> {
    match get(connection, owner, &reference.id)? {
        Some((record, _)) if record.revision == reference.revision => Ok(record.declaration),
        _ => Err(stale()),
    }
}

#[cfg(test)]
pub(super) fn save(
    connection: &mut Connection,
    owner: &PrincipalId,
    declaration: ProjectTemplateDeclaration,
    expected_revision: Option<u64>,
    now_ms: u64,
) -> Result<ProjectTemplateRecord, ProjectCreateError> {
    let transaction = begin(connection)?;
    let saved = save_in_transaction(&transaction, owner, declaration, expected_revision, now_ms)?;
    transaction.commit().map_err(db_error)?;
    Ok(saved)
}

pub(super) fn save_in_transaction(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    declaration: ProjectTemplateDeclaration,
    expected_revision: Option<u64>,
    now_ms: u64,
) -> Result<ProjectTemplateRecord, ProjectCreateError> {
    let existing = get(transaction, owner, &declaration.id)?;
    let encoded = encode(&declaration)?;
    let aggregate_id = if let Some(expected) = expected_revision {
        let (current, aggregate_id) = editable(existing, expected)?;
        if current.declaration == declaration {
            return Ok(current);
        }
        aggregate_id
    } else {
        if let Some((current, _)) = existing {
            if current.declaration == declaration {
                return Ok(current);
            }
            return Err(ProjectCreateError::new(
                ProjectCreateErrorCode::TemplateConflict,
            ));
        }
        Uuid::new_v4().to_string()
    };
    let revision = allocate_revision(transaction)?;
    if expected_revision.is_some() {
        transaction
            .execute(
                "UPDATE project_templates SET declaration_json=?1, revision=?2
                 WHERE owner_principal_id=?3 AND template_id=?4",
                params![encoded, integer(revision)?, owner.as_str(), declaration.id],
            )
            .map_err(db_error)?;
    } else {
        insert(
            transaction,
            owner,
            &declaration,
            &encoded,
            &aggregate_id,
            revision,
            false,
        )?;
    }
    changed(
        transaction,
        owner,
        &aggregate_id,
        revision,
        &declaration.id,
        now_ms,
    )?;
    log_change(
        transaction,
        owner,
        &declaration,
        "projectTemplates.save",
        "saved",
        now_ms,
    )?;
    Ok(ProjectTemplateRecord {
        declaration,
        revision,
        locked: false,
    })
}

pub(super) fn remove(
    connection: &mut Connection,
    owner: &PrincipalId,
    id: &str,
    expected_revision: u64,
    now_ms: u64,
) -> Result<u64, ProjectCreateError> {
    let transaction = begin(connection)?;
    let (record, aggregate_id) = editable(get(&transaction, owner, id)?, expected_revision)?;
    let revision = allocate_revision(&transaction)?;
    transaction
        .execute(
            "DELETE FROM project_templates WHERE owner_principal_id=?1 AND template_id=?2",
            params![owner.as_str(), id],
        )
        .map_err(db_error)?;
    changed(&transaction, owner, &aggregate_id, revision, id, now_ms)?;
    log_change(
        &transaction,
        owner,
        &record.declaration,
        "projectTemplates.remove",
        "removed",
        now_ms,
    )?;
    transaction.commit().map_err(db_error)?;
    Ok(revision)
}

pub(super) fn seed(
    connection: &mut Connection,
    owner: &PrincipalId,
    declarations: Vec<ProjectTemplateDeclaration>,
) -> Result<(), ProjectCreateError> {
    let transaction = begin(connection)?;
    for declaration in declarations {
        if let Some((existing, _)) = get(&transaction, owner, &declaration.id)? {
            if existing.locked && existing.declaration == declaration {
                continue;
            }
            return Err(ProjectCreateError::new(
                ProjectCreateErrorCode::TemplateConflict,
            ));
        }
        let encoded = encode(&declaration)?;
        let revision = allocate_revision(&transaction)?;
        insert(
            &transaction,
            owner,
            &declaration,
            &encoded,
            &Uuid::new_v4().to_string(),
            revision,
            true,
        )?;
    }
    transaction.commit().map_err(db_error)
}

fn get(
    connection: &Connection,
    owner: &PrincipalId,
    id: &str,
) -> Result<Option<(ProjectTemplateRecord, String)>, ProjectCreateError> {
    connection
        .query_row(
            "SELECT declaration_json, revision, locked, aggregate_id
             FROM project_templates WHERE owner_principal_id=?1 AND template_id=?2",
            params![owner.as_str(), id],
            decode,
        )
        .optional()
        .map_err(db_error)
}

fn decode(row: &Row<'_>) -> rusqlite::Result<(ProjectTemplateRecord, String)> {
    let value: String = row.get(0)?;
    let declaration = serde_json::from_str(&value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })?;
    let revision =
        u64::try_from(row.get::<_, i64>(1)?).map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok((
        ProjectTemplateRecord {
            declaration,
            revision,
            locked: row.get(2)?,
        },
        row.get(3)?,
    ))
}

fn editable(
    existing: Option<(ProjectTemplateRecord, String)>,
    expected_revision: u64,
) -> Result<(ProjectTemplateRecord, String), ProjectCreateError> {
    let (record, aggregate_id) = existing.ok_or_else(stale)?;
    if record.revision != expected_revision {
        return Err(stale());
    }
    if record.locked {
        return Err(ProjectCreateError::new(
            ProjectCreateErrorCode::TemplateLocked,
        ));
    }
    Ok((record, aggregate_id))
}

fn stale() -> ProjectCreateError {
    ProjectCreateError::new(ProjectCreateErrorCode::TemplateStale)
}

fn encode(declaration: &ProjectTemplateDeclaration) -> Result<String, ProjectCreateError> {
    let encoded =
        serde_json::to_string(declaration).map_err(|error| internal().with_source(error))?;
    if encoded.len() > MAX_DECLARATION_BYTES {
        return Err(ProjectCreateError::new(
            ProjectCreateErrorCode::InvalidInput,
        ));
    }
    Ok(encoded)
}

fn allocate_revision(transaction: &Transaction<'_>) -> Result<u64, ProjectCreateError> {
    let revision = transaction
        .query_row(
            "UPDATE project_template_revision SET last_revision=last_revision+1
             WHERE singleton=1 RETURNING last_revision",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(db_error)?;
    u64::try_from(revision).map_err(|_| internal())
}

fn insert(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    declaration: &ProjectTemplateDeclaration,
    encoded: &str,
    aggregate_id: &str,
    revision: u64,
    locked: bool,
) -> Result<(), ProjectCreateError> {
    transaction
        .execute(
            "INSERT INTO project_templates (
                owner_principal_id, template_id, aggregate_id, declaration_json, revision, locked
             ) VALUES (?1,?2,?3,?4,?5,?6)",
            params![
                owner.as_str(),
                declaration.id,
                aggregate_id,
                encoded,
                integer(revision)?,
                locked
            ],
        )
        .map_err(db_error)?;
    Ok(())
}

fn changed(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    aggregate_id: &str,
    revision: u64,
    id: &str,
    now_ms: u64,
) -> Result<(), ProjectCreateError> {
    transaction
        .execute(
            "INSERT INTO events (
                event_id, kind, aggregate_kind, aggregate_id, aggregate_revision,
                principal_id, occurred_at_ms, payload_json
             ) VALUES (?1,'project-template.changed','project-template',?2,?3,?4,?5,?6)",
            params![
                Uuid::new_v4().to_string(),
                aggregate_id,
                integer(revision)?,
                owner.as_str(),
                integer(now_ms)?,
                serde_json::json!({ "id": id }).to_string()
            ],
        )
        .map_err(db_error)?;
    Ok(())
}

fn log_change(
    transaction: &Transaction<'_>,
    owner: &PrincipalId,
    declaration: &ProjectTemplateDeclaration,
    action: &str,
    step: &str,
    now_ms: u64,
) -> Result<(), ProjectCreateError> {
    let target = LogTarget {
        kind: "project-template".to_owned(),
        id: Some(declaration.id.clone()),
        name: Some(declaration.name.clone()),
        path: None,
    };
    operation_logs::committed(
        transaction,
        owner,
        now_ms,
        CommittedStep::succeeded(action, step, Some(target)),
    )
    .map_err(db_error)
}
