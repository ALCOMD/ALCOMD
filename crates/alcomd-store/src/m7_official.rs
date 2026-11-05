use alcomd_application::{
    NewFailedActivity, OfficialActivityCursor, OfficialActivityItem, OfficialActivityPage,
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

    async fn get_official_activity(
        &self,
        owner: PrincipalId,
        activity_id: String,
    ) -> Result<OfficialActivityItem, StoreError> {
        self.request_worker(
            move |connection| super::activity::get(connection, &owner, &activity_id),
            sqlite::unavailable,
        )
        .await
    }

    async fn record_failed_activity(
        &self,
        owner: PrincipalId,
        value: NewFailedActivity,
    ) -> Result<OfficialActivityItem, StoreError> {
        self.request_worker(
            move |connection| super::activity::insert_failed(connection, &owner, &value),
            sqlite::unavailable,
        )
        .await
    }
}

pub(super) fn list_activity(
    connection: &Connection,
    owner: &PrincipalId,
    cursor: Option<&OfficialActivityCursor>,
    limit: u32,
) -> Result<OfficialActivityPage, StoreError> {
    if limit == 0 || limit > 200 {
        return Err(sqlite::unavailable());
    }
    let time = cursor
        .map(|v| i64::try_from(v.started_at_ms))
        .transpose()
        .map_err(|error| sqlite::unavailable().with_source(error))?;
    let id = cursor.map(|v| v.activity_id.as_str());
    let sql = format!(
        "SELECT {} FROM activities WHERE owner_principal_id=?1 AND (?2 IS NULL OR started_at_ms<?2 OR (started_at_ms=?2 AND activity_id<?3)) ORDER BY started_at_ms DESC,activity_id DESC LIMIT ?4",
        super::activity::COLUMNS
    );
    let mut statement = connection
        .prepare(&sql)
        .map_err(|error| sqlite::unavailable().with_source(error))?;
    let mut items = statement
        .query_map(
            params![owner.as_str(), time, id, i64::from(limit) + 1],
            super::activity::decode,
        )
        .map_err(|error| sqlite::unavailable().with_source(error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| sqlite::unavailable().with_source(error))?;
    let more = items.len() > limit as usize;
    if more {
        items.pop();
    }
    let next_cursor = if more {
        items.last().map(|item| OfficialActivityCursor {
            started_at_ms: item.started_at_ms,
            activity_id: item.activity_id.clone(),
        })
    } else {
        None
    };
    Ok(OfficialActivityPage { items, next_cursor })
}
