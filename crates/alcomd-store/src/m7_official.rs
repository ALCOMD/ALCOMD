use alcomd_application::{
    OfficialActivityCursor, OfficialActivityItem, OfficialActivityKind, OfficialActivityPage,
    OfficialDiagnosticCursor, OfficialDiagnosticItem, OfficialDiagnosticPage,
};
use rusqlite::{Connection, params};

use super::{PrincipalId, StateStoreHandle, StoreError, sqlite};

impl alcomd_application::OfficialGuiStore for StateStoreHandle {
    async fn list_official_activity(
        &self,
        owner: PrincipalId,
        cursor: Option<OfficialActivityCursor>,
        limit: u32,
    ) -> Result<OfficialActivityPage, StoreError> {
        self.request_worker(
            move |connection| list_activity(connection, &owner, cursor.as_ref(), limit),
            sqlite::unavailable,
        )
        .await
    }

    async fn list_official_diagnostics(
        &self,
        owner: PrincipalId,
        cursor: Option<OfficialDiagnosticCursor>,
        limit: u32,
    ) -> Result<OfficialDiagnosticPage, StoreError> {
        self.request_worker(
            move |connection| list_diagnostics(connection, &owner, cursor.as_ref(), limit),
            sqlite::unavailable,
        )
        .await
    }
}

fn list_activity(
    connection: &Connection,
    owner: &PrincipalId,
    cursor: Option<&OfficialActivityCursor>,
    limit: u32,
) -> Result<OfficialActivityPage, StoreError> {
    let cursor_time = cursor
        .map(|value| i64::try_from(value.occurred_at_ms))
        .transpose()
        .map_err(|_| sqlite::unavailable())?;
    let cursor_rank = cursor.map(|value| i64::from(value.source_rank));
    let cursor_id = cursor.map(|value| value.stable_id.as_str());
    let fetch_limit = i64::from(limit) + 1;
    let mut statement = connection
        .prepare(
            r#"SELECT occurred_at_ms, source_rank, stable_id, item_kind, summary_code,
                      operation_id, event_sequence, resource_kind, resource_id, state
               FROM (
                   SELECT e.occurred_at_ms, 1 AS source_rank,
                          printf('%020d', e.sequence) AS stable_id,
                          'event' AS item_kind, e.kind AS summary_code,
                          NULL AS operation_id, e.sequence AS event_sequence,
                          e.aggregate_kind AS resource_kind, e.aggregate_id AS resource_id,
                          NULL AS state
                   FROM events e WHERE e.principal_id = ?1
                   UNION ALL
                   SELECT o.updated_at_ms, 0 AS source_rank, o.operation_id AS stable_id,
                          'operation' AS item_kind,
                          'operation.' || o.kind || '.' || o.state AS summary_code,
                          o.operation_id, NULL AS event_sequence, NULL AS resource_kind,
                          NULL AS resource_id, o.state
                   FROM operations o WHERE o.owner_principal_id = ?1
               ) AS activity
               WHERE ?2 IS NULL
                  OR occurred_at_ms < ?2
                  OR (occurred_at_ms = ?2 AND source_rank < ?3)
                  OR (occurred_at_ms = ?2 AND source_rank = ?3 AND stable_id < ?4)
               ORDER BY occurred_at_ms DESC, source_rank DESC, stable_id DESC
               LIMIT ?5"#,
        )
        .map_err(|_| sqlite::unavailable())?;
    let rows = statement
        .query_map(
            params![
                owner.as_str(),
                cursor_time,
                cursor_rank,
                cursor_id,
                fetch_limit
            ],
            |row| {
                let item_kind: String = row.get(3)?;
                Ok((
                    OfficialActivityItem {
                        occurred_at_ms: to_u64(row.get(0)?)?,
                        kind: match item_kind.as_str() {
                            "operation" => OfficialActivityKind::Operation,
                            "event" => OfficialActivityKind::Event,
                            _ => return Err(rusqlite::Error::InvalidQuery),
                        },
                        summary_code: row.get(4)?,
                        operation_id: row.get(5)?,
                        event_sequence: optional_u64(row.get(6)?)?,
                        resource_kind: row.get(7)?,
                        resource_id: row.get(8)?,
                        state: row.get(9)?,
                    },
                    OfficialActivityCursor {
                        occurred_at_ms: to_u64(row.get(0)?)?,
                        source_rank: u8::try_from(row.get::<_, i64>(1)?)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        stable_id: row.get(2)?,
                    },
                ))
            },
        )
        .map_err(|_| sqlite::unavailable())?;

    let mut entries = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| sqlite::unavailable())?;
    let has_more = entries.len() > usize::try_from(limit).unwrap_or(usize::MAX);
    if has_more {
        entries.pop();
    }
    let next_cursor = has_more.then(|| {
        entries
            .last()
            .expect("page with overflow is non-empty")
            .1
            .clone()
    });
    Ok(OfficialActivityPage {
        items: entries.into_iter().map(|(item, _)| item).collect(),
        next_cursor,
    })
}

fn list_diagnostics(
    connection: &Connection,
    owner: &PrincipalId,
    cursor: Option<&OfficialDiagnosticCursor>,
    limit: u32,
) -> Result<OfficialDiagnosticPage, StoreError> {
    let cursor_time = cursor
        .map(|value| i64::try_from(value.occurred_at_ms))
        .transpose()
        .map_err(|_| sqlite::unavailable())?;
    let cursor_id = cursor.map(|value| value.operation_id.as_str());
    let fetch_limit = i64::from(limit) + 1;
    let mut statement = connection
        .prepare(
            r#"SELECT updated_at_ms, kind, error_code, diagnostic_id, operation_id
               FROM operations
               WHERE owner_principal_id = ?1 AND state = 'failed' AND error_code IS NOT NULL
                 AND (?2 IS NULL OR updated_at_ms < ?2
                      OR (updated_at_ms = ?2 AND operation_id < ?3))
               ORDER BY updated_at_ms DESC, operation_id DESC
               LIMIT ?4"#,
        )
        .map_err(|_| sqlite::unavailable())?;
    let rows = statement
        .query_map(
            params![owner.as_str(), cursor_time, cursor_id, fetch_limit],
            |row| {
                let kind: String = row.get(1)?;
                let operation_id: String = row.get(4)?;
                let occurred_at_ms = to_u64(row.get(0)?)?;
                Ok((
                    OfficialDiagnosticItem {
                        occurred_at_ms,
                        subsystem: kind.split('.').next().unwrap_or("operation").to_owned(),
                        code: row.get(2)?,
                        diagnostic_id: row.get(3)?,
                        operation_id: operation_id.clone(),
                    },
                    OfficialDiagnosticCursor {
                        occurred_at_ms,
                        operation_id,
                    },
                ))
            },
        )
        .map_err(|_| sqlite::unavailable())?;
    let mut entries = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| sqlite::unavailable())?;
    let has_more = entries.len() > usize::try_from(limit).unwrap_or(usize::MAX);
    if has_more {
        entries.pop();
    }
    let next_cursor = has_more.then(|| {
        entries
            .last()
            .expect("page with overflow is non-empty")
            .1
            .clone()
    });
    Ok(OfficialDiagnosticPage {
        items: entries.into_iter().map(|(item, _)| item).collect(),
        next_cursor,
    })
}

fn to_u64(value: i64) -> rusqlite::Result<u64> {
    u64::try_from(value).map_err(|_| rusqlite::Error::InvalidQuery)
}

fn optional_u64(value: Option<i64>) -> rusqlite::Result<Option<u64>> {
    value.map(to_u64).transpose()
}
