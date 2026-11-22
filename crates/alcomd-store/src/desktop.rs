use crate::{StateStoreHandle, m3, m8_workflows, sqlite};
use alcomd_application::{
    DesktopAcceptance, DesktopAction, DesktopError, DesktopProject, DesktopRequest, DesktopStore,
    M3ErrorCode, OperationId, PrincipalId, Revision, workflow_resource_owner,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

type Effect = Result<(), DesktopError>;

fn unavailable<T>(_: T) -> DesktopError {
    DesktopError::StoreUnavailable
}

pub(super) fn is_desktop(kind: &str) -> bool {
    matches!(
        kind,
        "desktop.focusProjectUnity"
            | "desktop.openProjectDirectory"
            | "desktop.openUnityHubDownload"
    )
}

fn check_actor(tx: &Connection, request: &DesktopRequest) -> Result<(), DesktopError> {
    if request.owner != PrincipalId::local_owner() && !request.owner.as_str().starts_with("mcp:") {
        return Err(DesktopError::PermissionDenied);
    }
    m8_workflows::require(tx, &request.owner, request.action.permissions())
        .map_err(|_| DesktopError::PermissionDenied)
}

fn replay(tx: &Connection, request: &DesktopRequest) -> Result<Option<Effect>, DesktopError> {
    check_actor(tx, request)?;
    let stored: Option<(String, Option<String>)> = tx.query_row(
        "SELECT request_fingerprint,response_json FROM idempotency_records WHERE principal_id=?1 AND method=?2 AND idempotency_key=?3",
        params![request.owner.as_str(),request.action.method(),request.key.as_str()], |row| Ok((row.get(0)?,row.get(1)?))
    ).optional().map_err(unavailable)?;
    let Some((fingerprint, response)) = stored else {
        return Ok(None);
    };
    if fingerprint != serde_json::to_string(&request.action).map_err(unavailable)? {
        return Err(DesktopError::IdempotencyConflict);
    }
    response.map_or(Ok(Some(Err(DesktopError::DesktopResultUnknown))), |json| {
        serde_json::from_str(&json).map(Some).map_err(unavailable)
    })
}

impl DesktopStore for StateStoreHandle {
    async fn desktop_replay(
        &self,
        request: DesktopRequest,
    ) -> Result<Option<Effect>, DesktopError> {
        self.request_worker(
            move |db| {
                let tx = db.transaction().map_err(unavailable)?;
                replay(&tx, &request)
            },
            || DesktopError::StoreUnavailable,
        )
        .await
    }

    async fn desktop_accept(
        &self,
        request: DesktopRequest,
        prepared: Option<DesktopProject>,
        now: u64,
    ) -> Result<DesktopAcceptance, DesktopError> {
        self.request_worker(move |db| {
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(unavailable)?;
            if let Some(result) = replay(&tx, &request)? {
                return Ok(DesktopAcceptance::Replay(result));
            }
            if let Some(id) = request.action.project_id() {
                let current = m3::get_project(&tx, &workflow_resource_owner(&request.owner), id).map_err(|e| match e.code() {
                    M3ErrorCode::ProjectNotFound => DesktopError::ProjectNotFound,
                    _ => DesktopError::StoreUnavailable,
                })?;
                if prepared.as_ref().is_none_or(|p| p.revision != current.revision)
                    || matches!(request.action, DesktopAction::Focus{expected_revision,..} if expected_revision != current.revision) {
                    return Err(DesktopError::ResourceRevisionConflict);
                }
            }
            let now = i64::try_from(now).map_err(unavailable)?;
            let id = OperationId::new();
            let fp = serde_json::to_string(&request.action).map_err(unavailable)?;
            tx.execute("INSERT INTO operations(operation_id,kind,state,revision,owner_principal_id,request_json,created_at_ms,updated_at_ms,started_at_ms)
                VALUES (?1,?2,'running',1,?3,?4,?5,?5,?5)", params![id.to_string(),request.action.method(),request.owner.as_str(),fp,now]).map_err(unavailable)?;
            tx.execute("INSERT INTO idempotency_records(principal_id,method,idempotency_key,request_fingerprint,state,operation_id,created_at_ms)
                VALUES (?1,?2,?3,?4,'pending',?5,?6)", params![request.owner.as_str(),request.action.method(),request.key.as_str(),fp,id.to_string(),now]).map_err(unavailable)?;
            sqlite::insert_event(&tx,&request.owner,id,Revision::INITIAL,"operation.created",r#"{"state":"running"}"#,now).map_err(unavailable)?;
            tx.commit().map_err(unavailable)?;
            Ok(DesktopAcceptance::Accepted(id))
        }, || DesktopError::StoreUnavailable).await
    }

    async fn desktop_finish(
        &self,
        request: DesktopRequest,
        id: OperationId,
        result: Effect,
        now: u64,
    ) -> Result<(), DesktopError> {
        self.request_worker(
            move |db| {
                let tx = db
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(unavailable)?;
                complete(
                    &tx,
                    &request.owner,
                    id,
                    result,
                    i64::try_from(now).map_err(unavailable)?,
                )?;
                tx.commit().map_err(unavailable)
            },
            || DesktopError::StoreUnavailable,
        )
        .await
    }
}

fn complete(
    tx: &Transaction<'_>,
    owner: &PrincipalId,
    id: OperationId,
    result: Effect,
    now: i64,
) -> Result<(), DesktopError> {
    let state = if result.is_ok() {
        "succeeded"
    } else {
        "failed"
    };
    let receipt = serde_json::to_string(&result).map_err(unavailable)?;
    let error = result
        .err()
        .map(|e| serde_json::to_value(e).map_err(unavailable))
        .transpose()?;
    let code = error.as_ref().and_then(serde_json::Value::as_str);
    let count = tx.execute("UPDATE operations SET state=?1, revision=2, error_code=?2, updated_at_ms=?3, completed_at_ms=?3,
        result_json=CASE WHEN ?2 IS NULL THEN json_object('outcome', CASE kind WHEN 'desktop.focusProjectUnity' THEN 'focused' ELSE 'requested' END) ELSE NULL END
        WHERE operation_id=?4 AND owner_principal_id=?5 AND state='running' AND revision=1",
        params![state,code,now,id.to_string(),owner.as_str()]).map_err(unavailable)?;
    if count != 1 {
        return Err(DesktopError::StoreUnavailable);
    }
    let count = tx.execute("UPDATE idempotency_records SET state='completed', response_json=?1 WHERE operation_id=?2 AND principal_id=?3 AND state='pending'",
        params![receipt,id.to_string(),owner.as_str()]).map_err(unavailable)?;
    if count != 1 {
        return Err(DesktopError::StoreUnavailable);
    }
    let revision = Revision::INITIAL
        .checked_next()
        .ok_or(DesktopError::StoreUnavailable)?;
    sqlite::insert_event(
        tx,
        owner,
        id,
        revision,
        "operation.state_changed",
        &serde_json::json!({"state":state}).to_string(),
        now,
    )
    .map_err(unavailable)
}

pub(super) fn recover(db: &mut Connection, id: OperationId, now: i64) -> Result<(), DesktopError> {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unavailable)?;
    let owner: String = tx
        .query_row(
            "SELECT owner_principal_id FROM operations WHERE operation_id=?1",
            [id.to_string()],
            |r| r.get(0),
        )
        .map_err(unavailable)?;
    complete(
        &tx,
        &PrincipalId::parse(owner).map_err(unavailable)?,
        id,
        Err(DesktopError::DesktopResultUnknown),
        now,
    )?;
    tx.commit().map_err(unavailable)
}
