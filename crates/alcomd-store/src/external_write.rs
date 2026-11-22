use crate::{StateStoreHandle, m8_workflows, sqlite};
use alcomd_application::{
    ExternalAdmission, ExternalError, ExternalReceipt, ExternalRequest, ExternalWriteStore,
    OperationId, PrincipalId, Revision,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

fn unavailable<T>(_: T) -> ExternalError {
    ExternalError::Unavailable
}
const JOURNAL: &str = r#"{"journal":true}"#;

fn load_receipt(
    tx: &Connection,
    id: &str,
    json: Option<String>,
) -> Result<ExternalReceipt, ExternalError> {
    let Some(mut json) = json else {
        return Ok(ExternalReceipt::OutcomeUnknown);
    };
    if json == JOURNAL {
        json.clear();
        let mut rows=tx.prepare("SELECT payload_json FROM operation_journal WHERE operation_id=?1 AND kind='external.result' AND state='applied' ORDER BY step").map_err(unavailable)?;
        for chunk in rows
            .query_map([id], |r| r.get::<_, String>(0))
            .map_err(unavailable)?
        {
            json.push_str(
                &serde_json::from_str::<String>(&chunk.map_err(unavailable)?)
                    .map_err(unavailable)?,
            );
        }
    }
    serde_json::from_str(&json).map_err(unavailable)
}

impl ExternalWriteStore for StateStoreHandle {
    async fn external_admit(
        &self,
        request: ExternalRequest,
        reserve: bool,
        now: u64,
    ) -> Result<ExternalAdmission, ExternalError> {
        self.request_worker(move|db|{
            let tx=db.transaction_with_behavior(if reserve {TransactionBehavior::Immediate} else {TransactionBehavior::Deferred}).map_err(unavailable)?;
            if request.caller != PrincipalId::local_owner() && !request.caller.as_str().starts_with("mcp:") {return Err(ExternalError::Denied)}
            m8_workflows::require(&tx,&request.caller,&request.permissions).map_err(|_|ExternalError::Denied)?;
            let fp=serde_json::to_string(&request.fingerprint).map_err(unavailable)?;
            let existing:Option<(String,String,Option<String>)>=tx.query_row("SELECT request_fingerprint,operation_id,response_json FROM idempotency_records WHERE principal_id=?1 AND method=?2 AND idempotency_key=?3",
                params![request.caller.as_str(),request.method.name(),request.key.as_str()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(unavailable)?;
            if let Some((fingerprint,id,payload))=existing {
                if fingerprint!=fp {return Err(ExternalError::Conflict);}
                return Ok(ExternalAdmission::Replayed(load_receipt(&tx,&id,payload)?));
            }
            if !reserve {return Ok(ExternalAdmission::Ready);}
            let now=i64::try_from(now).map_err(unavailable)?;
            let id=OperationId::new();
            tx.execute("INSERT INTO operations(operation_id,kind,state,revision,owner_principal_id,request_json,created_at_ms,updated_at_ms,started_at_ms) VALUES (?1,?2,'running',1,?3,json_object('fingerprint',?4),?5,?5,?5)",
                params![id.to_string(),request.method.name(),request.caller.as_str(),request.fingerprint,now]).map_err(unavailable)?;
            tx.execute("INSERT INTO idempotency_records(principal_id,method,idempotency_key,request_fingerprint,state,operation_id,created_at_ms) VALUES (?1,?2,?3,?4,'pending',?5,?6)",
                params![request.caller.as_str(),request.method.name(),request.key.as_str(),fp,id.to_string(),now]).map_err(unavailable)?;
            sqlite::insert_event(&tx,&request.caller,id,Revision::INITIAL,"operation.created",r#"{"state":"running"}"#,now).map_err(unavailable)?;
            tx.commit().map_err(unavailable)?;
            Ok(ExternalAdmission::Accepted(id))
        },||ExternalError::Unavailable).await
    }
    async fn external_finish(
        &self,
        request: ExternalRequest,
        id: OperationId,
        result: ExternalReceipt,
        now: u64,
    ) -> Result<(), ExternalError> {
        self.request_worker(
            move |db| {
                let tx = db
                    .transaction_with_behavior(TransactionBehavior::Immediate)
                    .map_err(unavailable)?;
                complete(
                    &tx,
                    &request.caller,
                    id,
                    result,
                    i64::try_from(now).map_err(unavailable)?,
                )?;
                tx.commit().map_err(unavailable)
            },
            || ExternalError::Unavailable,
        )
        .await
    }
}

fn complete(
    tx: &Transaction<'_>,
    owner: &PrincipalId,
    id: OperationId,
    result: ExternalReceipt,
    now: i64,
) -> Result<(), ExternalError> {
    let (state, code) = match &result {
        ExternalReceipt::Result(_) => ("succeeded", None),
        ExternalReceipt::Error(e) => ("failed", e.get("code").and_then(|v| v.as_str())),
        ExternalReceipt::OutcomeUnknown => ("failed", Some("external_write_outcome_unknown")),
    };
    let response = serde_json::to_string(&result).map_err(unavailable)?;
    let mut remaining = response.as_str();
    let mut step = 0i64;
    // Settings snapshots may exceed the bounded single-receipt row. Journal chunks are
    // transactionally committed with the receipt and never interpreted as work to resume.
    while response.len() > 65536 && !remaining.is_empty() {
        let end = remaining.floor_char_boundary(remaining.len().min(8192));
        let (part, rest) = remaining.split_at(end);
        step += 1;
        tx.execute("INSERT INTO operation_journal(operation_id,step,kind,state,payload_json,updated_at_ms) VALUES (?1,?2,'external.result','applied',?3,?4)",
            params![id.to_string(),step,serde_json::to_string(part).map_err(unavailable)?,now]).map_err(unavailable)?;
        remaining = rest;
    }
    let stored = if step == 0 {
        response.as_str()
    } else {
        JOURNAL
    };
    let count=tx.execute("UPDATE operations SET state=?1,revision=2,error_code=?2,updated_at_ms=?3,completed_at_ms=?3,result_json=CASE WHEN ?2 IS NULL THEN '{}' ELSE NULL END WHERE operation_id=?4 AND owner_principal_id=?5 AND state='running' AND revision=1",
        params![state,code,now,id.to_string(),owner.as_str()]).map_err(unavailable)?;
    if count != 1 {
        return Err(ExternalError::Unavailable);
    }
    if tx.execute("UPDATE idempotency_records SET state='completed',response_json=?1 WHERE operation_id=?2 AND principal_id=?3 AND state='pending'",params![stored,id.to_string(),owner.as_str()]).map_err(unavailable)? !=1 {return Err(ExternalError::Unavailable);}
    sqlite::insert_event(
        tx,
        owner,
        id,
        Revision::INITIAL
            .checked_next()
            .ok_or(ExternalError::Unavailable)?,
        "operation.state_changed",
        &serde_json::json!({"state":state}).to_string(),
        now,
    )
    .map_err(unavailable)
}

pub(super) fn recover(db: &mut Connection, id: OperationId, now: i64) -> Result<(), ExternalError> {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unavailable)?;
    let caller: String = tx
        .query_row(
            "SELECT owner_principal_id FROM operations WHERE operation_id=?1",
            [id.to_string()],
            |r| r.get(0),
        )
        .map_err(unavailable)?;
    complete(
        &tx,
        &PrincipalId::parse(caller).map_err(unavailable)?,
        id,
        ExternalReceipt::OutcomeUnknown,
        now,
    )?;
    tx.commit().map_err(unavailable)
}
