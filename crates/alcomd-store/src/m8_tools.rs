//! Transactional authority and effect receipts for extension MCP invocations.
use alcomd_application::{
    ToolAuthentication, ToolError as E, ToolInvocation, ToolOperation, ToolProvider,
    ToolStoreSnapshot,
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
        ToolAuthentication::Native(_) => Err(E::PermissionDenied),
        ToolAuthentication::OAuth(guard) => {
            super::m8_oauth::authenticate(db, guard, now).map_err(|_| E::PermissionDenied)
        }
        ToolAuthentication::Credential(hash) => {
            let id:String=db.query_row("SELECT c.client_id FROM mcp_clients c JOIN mcp_client_credentials k USING(client_id) WHERE k.token_hash=?1 AND k.revoked_at_ms IS NULL AND c.revoked_at_ms IS NULL",[hash.as_slice()],|r|r.get(0)).optional().map_err(unavailable)?.ok_or(E::PermissionDenied)?;
            super::m8_mcp::client(db, &id).map_err(|_| E::PermissionDenied)
        }
    }
}
fn entry_access(
    db: &Connection,
    auth: &ToolAuthentication,
) -> Result<alcomd_application::AccessContext, E> {
    match auth {
        ToolAuthentication::Native(access)
            if access.principal() == &alcomd_application::PrincipalId::local_owner() =>
        {
            Ok(access.clone())
        }
        _ => auth.access(authenticate(db, auth)?),
    }
}
pub(super) fn snapshot(
    db: &mut Connection,
    auth: &ToolAuthentication,
    kind: alcomd_application::ExtensionEntryKind,
) -> Result<ToolStoreSnapshot, E> {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Deferred)
        .map_err(unavailable)?;
    let access = entry_access(&tx, auth)?;
    let mut query=tx.prepare("SELECT e.extension_id,e.principal_id,e.live_package_locator FROM extensions e WHERE e.desired_state='enabled' AND e.quarantine_state='clear' AND EXISTS(SELECT 1 FROM extension_plans p WHERE p.extension_id=e.extension_id AND p.action='install' AND p.state='applied' AND p.package_digest=e.package_digest AND json_array_length(p.requested_interfaces_json, ?1)>0) ORDER BY e.extension_id").map_err(unavailable)?;
    let ids = query
        .query_map(
            [match kind {
                alcomd_application::ExtensionEntryKind::Mcp => "$.mcp_tools",
                alcomd_application::ExtensionEntryKind::Cli => "$.cli_commands",
            }],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            },
        )
        .map_err(unavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(unavailable)?;
    drop(query);
    let mut providers = Vec::new();
    for (id, owner, locator) in ids {
        let owner = alcomd_application::PrincipalId::parse(owner).map_err(unavailable)?;
        let record = super::m6::get_extension(&tx, &owner, &id).map_err(unavailable)?;
        providers.push(ToolProvider {
            record,
            owner,
            locator,
        });
    }
    tx.commit().map_err(unavailable)?;
    Ok(ToolStoreSnapshot {
        principal: access.principal().clone(),
        providers,
    })
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
    let access = entry_access(db, &authority.authentication)?;
    if access.principal() != &authority.principal {
        return Err(E::PermissionDenied);
    }
    let permissions = authority.declaration.permissions();
    verify_provider(db, &authority.origin, &permissions)?;
    alcomd_application::require_tool_permissions(&access, &permissions)?;
    Ok(access)
}
fn verify_provider(
    db: &Connection,
    origin: &alcomd_application::ToolOrigin,
    permissions: &[String],
) -> Result<(), E> {
    let valid:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM extensions WHERE extension_id=?1 AND package_digest=?2 AND publisher_fingerprint=?3 AND desired_state='enabled' AND quarantine_state='clear')",params![origin.extension_id,hex_bytes(&origin.package_digest)?,origin.publisher_fingerprint],|r|r.get(0)).map_err(unavailable)?;
    if !valid {
        return Err(E::ProviderUnavailable);
    }
    for permission in permissions {
        let granted:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM extension_grants WHERE extension_id=?1 AND permission_name=?2 AND resource_kind='Global' AND resource_id='*' AND state='granted')",params![origin.extension_id,permission],|r|r.get(0)).map_err(unavailable)?;
        if !granted {
            return Err(E::PermissionDenied);
        }
    }
    Ok(())
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
    let (state,effect):(String,Option<String>)=db.query_row("SELECT state,core_effect_json FROM extension_entry_calls WHERE call_id=?1 AND principal_id=?2",params![authority.call_id,authority.principal.as_str()],|r|Ok((r.get(0)?,r.get(1)?))).map_err(unavailable)?;
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
    if tx.execute("UPDATE extension_entry_calls SET core_effect_json=?1,updated_at_ms=max(updated_at_ms,?2) WHERE call_id=?3 AND principal_id=?4 AND state='running' AND core_effect_json IS NULL",params![value,i64::try_from(now).map_err(unavailable)?,authority.call_id,authority.principal.as_str()]).map_err(unavailable)?!=1{return Err(E::IdempotencyConflict);}
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
    request: &alcomd_application::ToolWriteAdmission,
) -> Result<Option<CallAdmission>, E> {
    let alcomd_application::ToolWriteAdmission {
        origin,
        kind,
        name,
        key,
        fingerprint,
        ..
    } = request;
    let existing=db.query_row("SELECT call_id,request_fingerprint,core_effect_json,state,permissions_json FROM extension_entry_calls WHERE principal_id=?1 AND extension_id=?2 AND entry_kind=?3 AND entry_name=?4 AND idempotency_key=?5",params![access.principal().as_str(),origin.extension_id,kind.as_str(),name,key],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?,r.get::<_,String>(3)?,r.get::<_,String>(4)?))).optional().map_err(unavailable)?;
    if let Some((id, fp, result, state, required)) = existing {
        if fp != *fingerprint {
            return Err(E::IdempotencyConflict);
        }
        let required: Vec<String> = serde_json::from_str(&required).map_err(unavailable)?;
        verify_provider(db, origin, &required)?;
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
        kind,
        name,
        key,
        fingerprint,
        permissions,
        effect,
    } = request;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unavailable)?;
    let access = entry_access(&tx, auth)?;
    verify_provider(&tx, origin, permissions)?;
    alcomd_application::require_tool_permissions(&access, permissions)?;
    if let Some(saved) = lookup(&tx, &access, request)? {
        return Ok(saved);
    }
    let now = super::m8_oauth::wall_time().map_err(unavailable)?;
    let id = alcomd_application::OperationId::new().to_string();
    let effect_kind = serde_json::to_value(effect).map_err(unavailable)?;
    tx.execute("INSERT INTO extension_entry_calls VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,'running',NULL,?12,?12)",params![id,access.principal().as_str(),kind.as_str(),name,key,fingerprint,origin.extension_id,origin.publisher_fingerprint,origin.package_digest,serde_json::to_string(permissions).map_err(unavailable)?,effect_kind.as_str().ok_or(E::Unavailable)?,i64::try_from(now).map_err(unavailable)?]).map_err(unavailable)?;
    tx.commit().map_err(unavailable)?;
    Ok(CallAdmission {
        call_id: id,
        replay: None,
        state: "new".into(),
    })
}
impl super::StateStoreHandle {
    pub async fn tool_snapshot(
        &self,
        auth: ToolAuthentication,
        kind: alcomd_application::ExtensionEntryKind,
    ) -> Result<ToolStoreSnapshot, E> {
        self.request_worker(move |db| snapshot(db, &auth, kind), || E::Unavailable)
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
            tx.execute("UPDATE extension_entry_calls SET state=?1,updated_at_ms=max(updated_at_ms,?2) WHERE call_id=?3 AND principal_id=?4",params![state,i64::try_from(super::m8_oauth::wall_time().map_err(unavailable)?).map_err(unavailable)?,call_id,principal.as_str()]).map_err(unavailable)?;
            let value:Option<String>=tx.query_row("SELECT core_effect_json FROM extension_entry_calls WHERE call_id=?1 AND principal_id=?2",params![call_id,principal.as_str()],|r|r.get(0)).map_err(unavailable)?;
            tx.commit().map_err(unavailable)?;value.map(|s|serde_json::from_str(&s).map_err(unavailable)).transpose()
        },||E::Unavailable).await
    }
    pub async fn tool_recover_startup(&self) -> Result<(), E> {
        self.request_worker(
            |db| {
                db.execute(
                    "UPDATE extension_entry_calls SET state='interrupted' WHERE state='running'",
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
