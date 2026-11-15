//! Transactional authority and effect receipts for extension MCP invocations.
use alcomd_application::{
    MCP_TOOLS_PROVIDE, ToolAuthentication, ToolError as E, ToolInvocation, ToolOperation,
    ToolProvider, ToolStoreSnapshot,
};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde_json::Value;
use std::cell::RefCell;
thread_local! {static INVOCATION:RefCell<Option<ToolInvocation>>=const{RefCell::new(None)};}
pub(super) struct ContextGuard(Option<ToolInvocation>);
impl ContextGuard {
    pub(super) fn install(value: Option<ToolInvocation>) -> Self {
        Self(INVOCATION.with(|v| v.replace(value)))
    }
}
impl Drop for ContextGuard {
    fn drop(&mut self) {
        INVOCATION.with(|v| {
            v.replace(self.0.take());
        });
    }
}
fn current() -> Option<ToolInvocation> {
    INVOCATION.with(|v| v.borrow().clone())
}
fn unavailable<T>(_: T) -> E {
    E::Unavailable
}
pub(super) fn authenticate(
    db: &Connection,
    auth: &ToolAuthentication,
) -> Result<alcomd_application::McpClient, E> {
    let now = super::m8_oauth::wall_time().map_err(unavailable)?;
    match auth {
        ToolAuthentication::OAuth(guard) => {
            super::m8_oauth::authenticate(db, guard, now).map_err(|_| E::PermissionDenied)
        }
        ToolAuthentication::Credential(hash) => {
            let id:String=db.query_row("SELECT c.client_id FROM mcp_clients c JOIN mcp_client_credentials k USING(client_id) WHERE k.token_hash=?1 AND k.revoked_at_ms IS NULL AND c.revoked_at_ms IS NULL",[hash.as_slice()],|r|r.get(0)).optional().map_err(unavailable)?.ok_or(E::PermissionDenied)?;
            super::m8_mcp::client(db, &id).map_err(|_| E::PermissionDenied)
        }
    }
}
pub(super) fn snapshot(
    db: &mut Connection,
    auth: &ToolAuthentication,
) -> Result<ToolStoreSnapshot, E> {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(unavailable)?;
    let client = authenticate(&tx, auth)?;
    let mut query=tx.prepare("SELECT e.extension_id,e.principal_id,e.live_package_locator FROM extensions e JOIN extension_grants g USING(extension_id) WHERE e.desired_state='enabled' AND e.quarantine_state='clear' AND g.permission_name='mcp.tools.provide' AND g.resource_kind='Global' AND g.resource_id='*' AND g.state='granted' ORDER BY e.extension_id").map_err(unavailable)?;
    let ids = query
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })
        .map_err(unavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(unavailable)?;
    drop(query);
    let mut providers = Vec::new();
    for (id, owner, locator) in ids {
        let owner = alcomd_application::PrincipalId::parse(owner).map_err(unavailable)?;
        let record = super::m6::get_extension(&tx, &owner, &id).map_err(unavailable)?;
        let mut query=tx.prepare("SELECT permission_name FROM extension_grants WHERE extension_id=?1 AND state='granted' AND resource_kind='Global' AND resource_id='*' ORDER BY permission_name").map_err(unavailable)?;
        let granted = query
            .query_map([&id], |r| r.get(0))
            .map_err(unavailable)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(unavailable)?;
        providers.push(ToolProvider {
            record,
            owner,
            locator,
            granted,
        });
    }
    tx.commit().map_err(unavailable)?;
    Ok(ToolStoreSnapshot { client, providers })
}
pub(super) fn check(
    db: &Connection,
    authority: &ToolInvocation,
    op: Option<ToolOperation>,
) -> Result<alcomd_application::AccessContext, E> {
    let now = super::m8_oauth::wall_time().map_err(unavailable)?;
    if !authority.is_current(now) {
        return Err(E::Interrupted);
    }
    if op.is_some_and(|op| !authority.allows(op)) {
        return Err(E::PermissionDenied);
    }
    // Renewal changes expiry, not instance identity. Resolve the current Core-held
    // expiry while retaining every immutable lease binding and revocation check.
    let mut lease = authority.lease.clone().ok_or(E::PermissionDenied)?;
    let expiry:i64=db.query_row("SELECT lease_expires_at_ms FROM extension_instances WHERE extension_id=?1 AND instance_id=?2",params![lease.extension_id,lease.instance_id],|r|r.get(0)).map_err(|_|E::PermissionDenied)?;
    lease.expires_at_ms = u64::try_from(expiry).map_err(|_| E::PermissionDenied)?;
    super::m6::validate_lease(db, &lease, now).map_err(|_| E::PermissionDenied)?;
    let valid:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM extensions WHERE extension_id=?1 AND package_digest=?2 AND publisher_fingerprint=?3 AND desired_state='enabled' AND quarantine_state='clear')",params![authority.origin.extension_id,hex_bytes(&authority.origin.package_digest)?,authority.origin.publisher_fingerprint],|r|r.get(0)).map_err(unavailable)?;
    if !valid {
        return Err(E::ProviderUnavailable);
    }
    let permissions = authority.declaration.permissions();
    for permission in
        std::iter::once(MCP_TOOLS_PROVIDE).chain(permissions.iter().map(String::as_str))
    {
        let granted:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM extension_grants WHERE extension_id=?1 AND permission_name=?2 AND resource_kind='Global' AND resource_id='*' AND state='granted')",params![authority.origin.extension_id,permission],|r|r.get(0)).map_err(unavailable)?;
        if !granted {
            return Err(E::PermissionDenied);
        }
    }
    let client = authenticate(db, &authority.authentication)?;
    if client.principal_id != authority.principal {
        return Err(E::PermissionDenied);
    }
    let access = authority.authentication.access(client)?;
    alcomd_application::require_tool_permissions(&access, &permissions)?;
    Ok(access)
}
fn hex_bytes(value: &str) -> Result<Vec<u8>, E> {
    if value.len() != 64 {
        return Err(E::Unavailable);
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|b| {
            std::str::from_utf8(b)
                .ok()
                .and_then(|s| u8::from_str_radix(s, 16).ok())
                .ok_or(E::Unavailable)
        })
        .collect()
}
pub(super) fn guard(db: &Connection, op: ToolOperation) -> Result<Option<ToolInvocation>, E> {
    let Some(authority) = current() else {
        return Ok(None);
    };
    check(db, &authority, Some(op))?;
    if !op.effect().writes() {
        return Ok(Some(authority));
    }
    let (state,effect):(String,Option<String>)=db.query_row("SELECT state,core_effect_json FROM extension_mcp_calls WHERE call_id=?1 AND principal_id=?2",params![authority.call_id,authority.principal.as_str()],|r|Ok((r.get(0)?,r.get(1)?))).map_err(unavailable)?;
    if state != "running" || effect.is_some() {
        return Err(E::IdempotencyConflict);
    }
    Ok(Some(authority))
}
pub(super) fn effect(
    tx: &Transaction<'_>,
    guard: Option<&ToolInvocation>,
    op: ToolOperation,
    value: Value,
) -> Result<(), E> {
    let Some(authority) = guard else {
        return Ok(());
    };
    if !authority.allows(op) || !op.effect().writes() {
        return Err(E::PermissionDenied);
    }
    let mut value = value;
    value["source"] = serde_json::to_value(&authority.origin).map_err(unavailable)?;
    let value = serde_json::to_string(&value).map_err(unavailable)?;
    if value.len() > 65536 {
        return Err(E::ResourceLimit);
    }
    let now = super::m8_oauth::wall_time().map_err(unavailable)?;
    if tx.execute("UPDATE extension_mcp_calls SET core_effect_json=?1,updated_at_ms=max(updated_at_ms,?2) WHERE call_id=?3 AND principal_id=?4 AND state='running' AND core_effect_json IS NULL",params![value,i64::try_from(now).map_err(unavailable)?,authority.call_id,authority.principal.as_str()]).map_err(unavailable)?!=1{return Err(E::IdempotencyConflict);}
    Ok(())
}
#[derive(Clone)]
pub(super) struct CallAdmission {
    pub call_id: String,
    pub replay: Option<Value>,
    pub state: String,
}
fn lookup(
    db: &Connection,
    access: &alcomd_application::AccessContext,
    name: &str,
    key: &str,
    fingerprint: &str,
) -> Result<Option<CallAdmission>, E> {
    let existing=db.query_row("SELECT call_id,request_fingerprint,core_effect_json,state,permissions_json FROM extension_mcp_calls WHERE principal_id=?1 AND tool_name=?2 AND idempotency_key=?3",params![access.principal().as_str(),name,key],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?))).optional().map_err(unavailable)?;
    if let Some((id, fp, result, state, required)) = existing {
        if fp != fingerprint {
            return Err(E::IdempotencyConflict);
        }
        let required: Vec<String> = serde_json::from_str(&required).map_err(unavailable)?;
        alcomd_application::require_tool_permissions(access, &required)?;
        return Ok(Some(CallAdmission {
            call_id: id,
            replay: result
                .map(|s| serde_json::from_str(&s))
                .transpose()
                .map_err(unavailable)?,
            state,
        }));
    }
    Ok(None)
}
pub(super) fn begin(
    db: &mut Connection,
    auth: &ToolAuthentication,
    request: &alcomd_application::ToolWriteAdmission,
) -> Result<CallAdmission, E> {
    let alcomd_application::ToolWriteAdmission {
        origin,
        name,
        key,
        fingerprint,
        permissions,
        effect,
    } = request;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unavailable)?;
    let client = authenticate(&tx, auth)?;
    let access = auth.access(client)?;
    alcomd_application::require_tool_permissions(&access, permissions)?;
    if let Some(saved) = lookup(&tx, &access, name, key, fingerprint)? {
        return Ok(saved);
    }
    let now = super::m8_oauth::wall_time().map_err(unavailable)?;
    let id = alcomd_application::OperationId::new().to_string();
    let kind = serde_json::to_value(effect).map_err(unavailable)?;
    tx.execute("INSERT INTO extension_mcp_calls VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'running',NULL,?11,?11)",params![id,access.principal().as_str(),name,key,fingerprint,origin.extension_id,origin.publisher_fingerprint,origin.package_digest,serde_json::to_string(permissions).map_err(unavailable)?,kind.as_str().ok_or(E::Unavailable)?,i64::try_from(now).map_err(unavailable)?]).map_err(unavailable)?;
    tx.commit().map_err(unavailable)?;
    Ok(CallAdmission {
        call_id: id,
        replay: None,
        state: "new".into(),
    })
}
impl super::StateStoreHandle {
    pub async fn tool_replay(
        &self,
        auth: ToolAuthentication,
        name: String,
        key: String,
        fingerprint: String,
    ) -> Result<Option<alcomd_application::ToolCallResult>, E> {
        self.request_worker(
            move |db| {
                let tx = db
                    .transaction_with_behavior(TransactionBehavior::Deferred)
                    .map_err(unavailable)?;
                let access = auth.access(authenticate(&tx, &auth)?)?;
                let value = lookup(&tx, &access, &name, &key, &fingerprint)?;
                tx.commit().map_err(unavailable)?;
                Ok(value.map(|saved| {
                    let error = if saved.state == "running" {
                        Some(E::InProgress)
                    } else if saved.replay.is_none() {
                        Some(E::Interrupted)
                    } else {
                        None
                    };
                    alcomd_application::ToolCallResult {
                        data: Value::Null,
                        core_effect: saved.replay,
                        replayed: true,
                        error,
                    }
                }))
            },
            || E::Unavailable,
        )
        .await
    }

    pub async fn tool_snapshot(&self, auth: ToolAuthentication) -> Result<ToolStoreSnapshot, E> {
        self.request_worker(move |db| snapshot(db, &auth), || E::Unavailable)
            .await
    }
    pub async fn tool_check(
        &self,
        authority: ToolInvocation,
        op: Option<ToolOperation>,
    ) -> Result<alcomd_application::AccessContext, E> {
        self.request_worker(move |db| check(db, &authority, op), || E::Unavailable)
            .await
    }
    pub async fn tool_begin(
        &self,
        auth: ToolAuthentication,
        request: alcomd_application::ToolWriteAdmission,
    ) -> Result<(String, Option<Value>, String), E> {
        self.request_worker(
            move |db| begin(db, &auth, &request).map(|v| (v.call_id, v.replay, v.state)),
            || E::Unavailable,
        )
        .await
    }
    pub async fn tool_finish(
        &self,
        call_id: String,
        principal: alcomd_application::PrincipalId,
        interrupted: bool,
    ) -> Result<Option<Value>, E> {
        self.request_worker(move|db|{
            let tx=db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(unavailable)?;
            let state=if interrupted{"interrupted"}else{"finished"};
            tx.execute("UPDATE extension_mcp_calls SET state=?1,updated_at_ms=max(updated_at_ms,?2) WHERE call_id=?3 AND principal_id=?4",params![state,i64::try_from(super::m8_oauth::wall_time().map_err(unavailable)?).map_err(unavailable)?,call_id,principal.as_str()]).map_err(unavailable)?;
            let value:Option<String>=tx.query_row("SELECT core_effect_json FROM extension_mcp_calls WHERE call_id=?1 AND principal_id=?2",params![call_id,principal.as_str()],|r|r.get(0)).map_err(unavailable)?;
            tx.commit().map_err(unavailable)?;value.map(|s|serde_json::from_str(&s).map_err(unavailable)).transpose()
        },||E::Unavailable).await
    }
    pub async fn tool_recover_startup(&self) -> Result<(), E> {
        self.request_worker(
            |db| {
                db.execute(
                    "UPDATE extension_mcp_calls SET state='interrupted' WHERE state='running'",
                    [],
                )
                .map_err(unavailable)?;
                Ok(())
            },
            || E::Unavailable,
        )
        .await
    }
}
