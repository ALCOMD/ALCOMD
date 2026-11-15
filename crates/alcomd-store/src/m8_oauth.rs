//! SQLite authority for OAuth registrations, token ceilings and atomic rotation.
use crate::StateStoreHandle;
use alcomd_application::*;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

fn unavailable<T>(_: T) -> OAuthError {
    OAuthError::Unavailable
}
pub(super) fn wall_time() -> Result<u64, OAuthError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|d| u64::try_from(d.as_millis()).ok())
        .ok_or(OAuthError::Unavailable)
}
fn uint(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    let n = row.get::<_, i64>(index)?;
    u64::try_from(n).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, n))
}
fn integer(n: u64) -> Result<i64, OAuthError> {
    i64::try_from(n).map_err(|_| OAuthError::InvalidInput)
}
fn generation(db: &Connection, id: &str) -> Result<String, OAuthError> {
    Ok(db.query_row("SELECT credential_id FROM mcp_client_credentials WHERE client_id=?1 AND revoked_at_ms IS NULL",[id],|r|r.get(0)).optional().map_err(unavailable)?.unwrap_or_default())
}
fn client(db: &Connection, id: &str) -> Result<McpClient, OAuthError> {
    super::m8_mcp::client(db, id).map_err(|_| OAuthError::InvalidClient)
}
fn registration(db: &Connection, id: &str) -> Result<OAuthRegistrationState, OAuthError> {
    let (record,secret_hash)=db.query_row("SELECT oauth_client_id,client_id,issuer,resource,redirect_uris_json,confidential,allow_refresh,revision,created_at_ms,revoked_at_ms,secret_hash FROM mcp_oauth_registrations WHERE oauth_client_id=?1",[id],|r|{
        let json:String=r.get(4)?;
        let uris=serde_json::from_str(&json).map_err(|e|rusqlite::Error::FromSqlConversionFailure(4,rusqlite::types::Type::Text,Box::new(e)))?;
        Ok((OAuthRegistration{oauth_client_id:r.get(0)?,client_id:r.get(1)?,issuer:r.get(2)?,resource:r.get(3)?,redirect_uris:uris,confidential:r.get(5)?,allow_refresh:r.get(6)?,revision:uint(r,7)?,created_at_ms:uint(r,8)?,revoked_at_ms:r.get::<_,Option<i64>>(9)?.map(|v|u64::try_from(v).map_err(|_|rusqlite::Error::IntegralValueOutOfRange(9,v))).transpose()?},r.get::<_,Option<Vec<u8>>>(10)?))
    }).optional().map_err(unavailable)?.ok_or(OAuthError::InvalidClient)?;
    let grant_revision = db
        .query_row(
            "SELECT grant_revision FROM mcp_clients WHERE client_id=?1",
            [&record.client_id],
            |r| uint(r, 0),
        )
        .map_err(unavailable)?;
    Ok(OAuthRegistrationState {
        credential_generation: generation(db, &record.client_id)?,
        client: client(db, &record.client_id)?,
        registration: record,
        secret_hash: secret_hash
            .map(|v| v.try_into().map_err(|_| OAuthError::Unavailable))
            .transpose()?,
        grant_revision,
    })
}
fn active(current: &OAuthRegistrationState) -> Result<(), OAuthError> {
    if current.registration.revoked_at_ms.is_some() || current.client.revoked_at_ms.is_some() {
        Err(OAuthError::InvalidClient)
    } else {
        Ok(())
    }
}
fn metadata(row: &rusqlite::Row<'_>) -> rusqlite::Result<OAuthRegistration> {
    let json: String = row.get(4)?;
    let redirect_uris = serde_json::from_str(&json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(OAuthRegistration {
        oauth_client_id: row.get(0)?,
        client_id: row.get(1)?,
        issuer: row.get(2)?,
        resource: row.get(3)?,
        redirect_uris,
        confidential: row.get(5)?,
        allow_refresh: row.get(6)?,
        revision: uint(row, 7)?,
        created_at_ms: uint(row, 8)?,
        revoked_at_ms: row
            .get::<_, Option<i64>>(9)?
            .map(|v| u64::try_from(v).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(9, v)))
            .transpose()?,
    })
}
fn registration_page(
    db: &mut Connection,
    input: OAuthPageParams,
) -> Result<OAuthRegistrationPage, OAuthError> {
    let tx = db.transaction().map_err(unavailable)?;
    let select = "SELECT oauth_client_id,client_id,issuer,resource,redirect_uris_json,confidential,allow_refresh,revision,created_at_ms,revoked_at_ms FROM mcp_oauth_registrations ORDER BY oauth_client_id";
    let (fp, count) = {
        let mut q = tx.prepare(select).map_err(unavailable)?;
        let rows = q.query_map([], metadata).map_err(unavailable)?;
        oauth_registration_fingerprint(rows.map(|r| r.map_err(unavailable)))?
    };
    let (start, limit) = oauth_page_window(&input.cursor, &fp, count, input.limit)?;
    let mut items = Vec::new();
    let mut bytes = 1024usize;
    {
        let mut q = tx
            .prepare(&format!("{select} LIMIT ?1 OFFSET ?2"))
            .map_err(unavailable)?;
        let rows = q
            .query_map(
                params![
                    i64::try_from(limit).map_err(unavailable)?,
                    i64::try_from(start).map_err(unavailable)?
                ],
                metadata,
            )
            .map_err(unavailable)?;
        for row in rows {
            let row = row.map_err(unavailable)?;
            let size = serde_json::to_vec(&row).map_err(unavailable)?.len() + 1;
            if bytes + size > 65536 {
                if items.is_empty() {
                    return Err(OAuthError::LimitExceeded);
                }
                break;
            }
            bytes += size;
            items.push(row);
        }
    }
    let end = start + items.len();
    tx.commit().map_err(unavailable)?;
    Ok(OAuthRegistrationPage {
        items,
        next_cursor: (end < count).then(|| format!("{fp}:{end}")),
    })
}
fn replay(
    db: &Connection,
    method: &str,
    key: &str,
    fp: &str,
) -> Result<Option<OAuthRegistration>, OAuthError> {
    let row=db.query_row("SELECT request_fingerprint,response_json FROM idempotency_records WHERE principal_id='builtin:local-owner' AND method=?1 AND idempotency_key=?2",params![method,key],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional().map_err(unavailable)?;
    let fp = serde_json::to_string(fp).map_err(unavailable)?;
    row.map(|(saved, json)| {
        if saved != fp {
            return Err(OAuthError::IdempotencyConflict);
        }
        serde_json::from_str(&json).map_err(unavailable)
    })
    .transpose()
}
fn remember(
    db: &Connection,
    method: &str,
    key: &str,
    fp: &str,
    record: &OAuthRegistration,
    now: u64,
) -> Result<(), OAuthError> {
    db.execute("INSERT INTO idempotency_records(principal_id,method,idempotency_key,request_fingerprint,state,response_json,created_at_ms) VALUES ('builtin:local-owner',?1,?2,?3,'completed',?4,?5)",params![method,key,serde_json::to_string(fp).map_err(unavailable)?,serde_json::to_string(record).map_err(unavailable)?,integer(now)?]).map_err(unavailable)?;
    Ok(())
}
fn set(
    db: &mut Connection,
    input: OAuthRegistrationSetParams,
    id: String,
    secret_hash: Option<[u8; 32]>,
    now: u64,
) -> Result<OAuthRegistration, OAuthError> {
    oauth_validate_registration(&input)?;
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unavailable)?;
    let fp = oauth_fingerprint(&input)?;
    if let Some(record) = replay(
        &tx,
        "mcp.oauth.registrations.set",
        &input.idempotency_key,
        &fp,
    )? {
        return if record.confidential {
            Err(OAuthError::ResponseUnavailable)
        } else {
            Ok(record)
        };
    }
    let core = client(&tx, &input.client_id)?;
    if core.revoked_at_ms.is_some() {
        return Err(OAuthError::Denied);
    }
    if input.confidential != secret_hash.is_some() {
        return Err(OAuthError::InvalidInput);
    }
    let existing:Option<String>=tx.query_row("SELECT oauth_client_id FROM mcp_oauth_registrations WHERE client_id=?1 AND revoked_at_ms IS NULL",[&input.client_id],|r|r.get(0)).optional().map_err(unavailable)?;
    let id = if let Some(id) = existing {
        let current = registration(&tx, &id)?;
        if current.registration.revision != input.expected_revision {
            return Err(OAuthError::RevisionConflict);
        }
        tx.execute("UPDATE mcp_oauth_registrations SET issuer=?2,resource=?3,redirect_uris_json=?4,confidential=?5,secret_hash=?6,allow_refresh=?7,revision=revision+1 WHERE oauth_client_id=?1",params![id,input.issuer,input.resource,serde_json::to_string(&input.redirect_uris).map_err(unavailable)?,input.confidential,secret_hash.as_ref().map(|v|v.as_slice()),input.allow_refresh]).map_err(unavailable)?;
        tx.execute("UPDATE mcp_oauth_tokens SET revoked_at_ms=max(created_at_ms,?2) WHERE oauth_client_id=?1 AND revoked_at_ms IS NULL",params![id,integer(now)?]).map_err(unavailable)?;
        id
    } else {
        if input.expected_revision != 0 {
            return Err(OAuthError::RevisionConflict);
        }
        tx.execute("INSERT INTO mcp_oauth_registrations(oauth_client_id,client_id,issuer,resource,redirect_uris_json,confidential,secret_hash,allow_refresh,revision,created_at_ms) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,1,?9)",params![id,input.client_id,input.issuer,input.resource,serde_json::to_string(&input.redirect_uris).map_err(unavailable)?,input.confidential,secret_hash.as_ref().map(|v|v.as_slice()),input.allow_refresh,integer(now)?]).map_err(unavailable)?;
        id
    };
    let result = registration(&tx, &id)?.registration;
    remember(
        &tx,
        "mcp.oauth.registrations.set",
        &input.idempotency_key,
        &fp,
        &result,
        now,
    )?;
    tx.commit().map_err(unavailable)?;
    Ok(result)
}
fn revoke_registration(
    db: &mut Connection,
    input: OAuthRegistrationRevokeParams,
    now: u64,
) -> Result<OAuthRegistration, OAuthError> {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unavailable)?;
    let fp = oauth_fingerprint(&input)?;
    if let Some(record) = replay(
        &tx,
        "mcp.oauth.registrations.revoke",
        &input.idempotency_key,
        &fp,
    )? {
        return Ok(record);
    }
    let current = registration(&tx, &input.oauth_client_id)?;
    if current.registration.revision != input.expected_revision
        || current.registration.revoked_at_ms.is_some()
    {
        return Err(OAuthError::RevisionConflict);
    }
    tx.execute("UPDATE mcp_oauth_registrations SET revoked_at_ms=max(created_at_ms,?2),revision=revision+1 WHERE oauth_client_id=?1",params![input.oauth_client_id,integer(now)?]).map_err(unavailable)?;
    tx.execute("UPDATE mcp_oauth_tokens SET revoked_at_ms=max(created_at_ms,?2) WHERE oauth_client_id=?1 AND revoked_at_ms IS NULL",params![input.oauth_client_id,integer(now)?]).map_err(unavailable)?;
    let result = registration(&tx, &input.oauth_client_id)?.registration;
    remember(
        &tx,
        "mcp.oauth.registrations.revoke",
        &input.idempotency_key,
        &fp,
        &result,
        now,
    )?;
    tx.commit().map_err(unavailable)?;
    Ok(result)
}

struct Token {
    kind: String,
    id: String,
    reg_revision: u64,
    family: String,
    generation: String,
    grant_revision: u64,
    grants: Vec<McpGrant>,
    issuer: String,
    resource: String,
    expires: u64,
    absolute: u64,
    consumed: bool,
    revoked: bool,
}
fn token(db: &Connection, hash: &[u8; 32]) -> Result<Token, OAuthError> {
    let t=db.query_row("SELECT kind,oauth_client_id,registration_revision,family_id,credential_generation,grant_revision,grants_json,issuer,resource,expires_at_ms,family_expires_at_ms,consumed_at_ms,revoked_at_ms FROM mcp_oauth_tokens WHERE token_hash=?1",[hash.as_slice()],|r|{
        let json:String=r.get(6)?;
        let grants=serde_json::from_str(&json).map_err(|e|rusqlite::Error::FromSqlConversionFailure(6,rusqlite::types::Type::Text,Box::new(e)))?;
        Ok(Token{kind:r.get(0)?,id:r.get(1)?,reg_revision:uint(r,2)?,family:r.get(3)?,generation:r.get(4)?,grant_revision:uint(r,5)?,grants,issuer:r.get(7)?,resource:r.get(8)?,expires:uint(r,9)?,absolute:uint(r,10)?,consumed:r.get::<_,Option<i64>>(11)?.is_some(),revoked:r.get::<_,Option<i64>>(12)?.is_some()})
    }).optional().map_err(unavailable)?.ok_or(OAuthError::InvalidGrant)?;
    validate_mcp_grants(&t.grants).map_err(|_| OAuthError::InvalidGrant)?;
    Ok(t)
}
fn validate_token(
    db: &Connection,
    t: &Token,
    now: u64,
) -> Result<OAuthRegistrationState, OAuthError> {
    let current = registration(db, &t.id)?;
    active(&current)?;
    if t.revoked
        // A consumed refresh remains replay evidence through family expiry, even
        // after its own idle deadline. Its caller must revoke the whole family.
        || (t.expires <= now && !(t.kind == "refresh" && t.consumed))
        || t.absolute <= now
        || t.grant_revision > current.grant_revision
        || t.reg_revision != current.registration.revision
        || t.generation != current.credential_generation
        || t.issuer != current.registration.issuer
        || t.resource != current.registration.resource
    {
        return Err(OAuthError::InvalidGrant);
    }
    Ok(current)
}
pub(super) fn authenticate(
    db: &Connection,
    guard: &OAuthAccessGuard,
    now: u64,
) -> Result<McpClient, OAuthError> {
    let t = token(db, &guard.token_hash)?;
    if t.kind != "access" || t.resource != guard.resource {
        return Err(OAuthError::InvalidGrant);
    }
    let current = validate_token(db, &t, now)?;
    let mut client = current.client;
    client.grants.retain(|g| t.grants.contains(g));
    Ok(client)
}
pub(super) fn project_cursor_binding(
    db: &Connection,
    guard: &OAuthAccessGuard,
    now: u64,
    cursor: &RegistryCursor<ProjectId>,
) -> Result<String, OAuthError> {
    let t = token(db, &guard.token_hash)?;
    if t.kind != "access" || t.resource != guard.resource {
        return Err(OAuthError::InvalidGrant);
    }
    let current = validate_token(db, &t, now)?;
    oauth_project_cursor_binding(guard, current.grant_revision, &t.grants, cursor)
}
fn revoke_family(db: &Connection, family: &str, now: u64) -> Result<(), OAuthError> {
    db.execute("UPDATE mcp_oauth_tokens SET revoked_at_ms=max(created_at_ms,?2) WHERE family_id=?1 AND revoked_at_ms IS NULL",params![family,integer(now)?]).map_err(unavailable)?;
    Ok(())
}
fn refresh(
    db: &mut Connection,
    hash: &[u8; 32],
    id: &str,
    now: u64,
) -> Result<OAuthTokenState, OAuthError> {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unavailable)?;
    let t = token(&tx, hash)?;
    if t.kind != "refresh" || t.id != id {
        return Err(OAuthError::InvalidGrant);
    }
    let current = validate_token(&tx, &t, now)?;
    if t.consumed {
        revoke_family(&tx, &t.family, now)?;
        tx.commit().map_err(unavailable)?;
        return Err(OAuthError::InvalidGrant);
    }
    let result = OAuthTokenState {
        registration: current,
        grants: t.grants,
        family_id: t.family,
        family_expires_at_ms: t.absolute,
        consumed: false,
    };
    tx.commit().map_err(unavailable)?;
    Ok(result)
}
fn issue(db: &mut Connection, issue: OAuthIssue, now: u64) -> Result<(), OAuthError> {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unavailable)?;
    let current = registration(&tx, &issue.registration.registration.oauth_client_id)?;
    active(&current)?;
    if current.registration != issue.registration.registration
        || current.client.revision != issue.registration.client.revision
        || current.credential_generation != issue.registration.credential_generation
    {
        return Err(OAuthError::RevisionConflict);
    }
    validate_mcp_grants(&issue.grants).map_err(|_| OAuthError::InvalidInput)?;
    if issue
        .grants
        .iter()
        .any(|g| !current.client.grants.contains(g))
        || issue.family_expires_at_ms <= now
    {
        return Err(OAuthError::Denied);
    }
    if let Some(parent) = &issue.parent_refresh {
        let t = token(&tx, parent)?;
        validate_token(&tx, &t, now)?;
        if t.kind != "refresh"
            || t.id != current.registration.oauth_client_id
            || t.family != issue.family_id
            || t.absolute != issue.family_expires_at_ms
        {
            return Err(OAuthError::InvalidGrant);
        }
        if t.consumed {
            revoke_family(&tx, &t.family, now)?;
            tx.commit().map_err(unavailable)?;
            return Err(OAuthError::InvalidGrant);
        }
        if issue.grants.iter().any(|g| !t.grants.contains(g)) {
            return Err(OAuthError::Denied);
        }
        tx.execute("UPDATE mcp_oauth_tokens SET consumed_at_ms=?2 WHERE token_hash=?1 AND consumed_at_ms IS NULL",params![parent.as_slice(),integer(now)?]).map_err(unavailable)?;
    }
    tx.execute(
        "DELETE FROM mcp_oauth_tokens WHERE family_expires_at_ms<=?1",
        [integer(now)?],
    )
    .map_err(unavailable)?;
    let access_count:i64=tx.query_row("SELECT count(*) FROM mcp_oauth_tokens WHERE oauth_client_id=?1 AND kind='access' AND expires_at_ms>?2 AND revoked_at_ms IS NULL",params![current.registration.oauth_client_id,integer(now)?],|r|r.get(0)).map_err(unavailable)?;
    let families:i64=tx.query_row("SELECT count(DISTINCT family_id) FROM mcp_oauth_tokens WHERE oauth_client_id=?1 AND expires_at_ms>?2 AND revoked_at_ms IS NULL",params![current.registration.oauth_client_id,integer(now)?],|r|r.get(0)).map_err(unavailable)?;
    if access_count >= 64 || (issue.parent_refresh.is_none() && families >= 16) {
        return Err(OAuthError::LimitExceeded);
    }
    let grants = serde_json::to_string(&issue.grants).map_err(unavailable)?;
    if grants.len() > 1_048_576 {
        return Err(OAuthError::LimitExceeded);
    }
    for (kind, digest, lifetime) in [
        ("access", Some(issue.access_hash), OAUTH_ACCESS_MS),
        ("refresh", issue.refresh_hash, OAUTH_REFRESH_IDLE_MS),
    ] {
        if let Some(digest) = digest {
            let source_code = if kind == "access" {
                issue.source_code_hash.as_ref().map(|v| v.as_slice())
            } else {
                None
            };
            let expires = now.saturating_add(lifetime).min(issue.family_expires_at_ms);
            tx.execute("INSERT INTO mcp_oauth_tokens(token_hash,kind,oauth_client_id,registration_revision,family_id,parent_hash,source_code_hash,credential_generation,grant_revision,grants_json,issuer,resource,created_at_ms,expires_at_ms,family_expires_at_ms) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",params![digest.as_slice(),kind,current.registration.oauth_client_id,integer(current.registration.revision)?,issue.family_id,issue.parent_refresh.as_ref().map(|v|v.as_slice()),source_code,current.credential_generation,integer(current.grant_revision)?,grants,current.registration.issuer,current.registration.resource,integer(now)?,integer(expires)?,integer(issue.family_expires_at_ms)?]).map_err(|e| if matches!(e,rusqlite::Error::SqliteFailure(ref error,_) if error.code==rusqlite::ErrorCode::ConstraintViolation) { OAuthError::InvalidGrant } else { OAuthError::Unavailable })?;
        }
    }
    tx.commit().map_err(unavailable)?;
    Ok(())
}

impl OAuthStore for StateStoreHandle {
    async fn oauth_registration(&self, id: String) -> Result<OAuthRegistrationState, OAuthError> {
        self.request_worker(move |db| registration(db, &id), || OAuthError::Unavailable)
            .await
    }
    async fn oauth_registrations(
        &self,
        input: OAuthPageParams,
    ) -> Result<OAuthRegistrationPage, OAuthError> {
        self.request_worker(
            move |db| registration_page(db, input),
            || OAuthError::Unavailable,
        )
        .await
    }
    async fn oauth_set_registration(
        &self,
        input: OAuthRegistrationSetParams,
        id: String,
        secret_hash: Option<[u8; 32]>,
        now: u64,
    ) -> Result<OAuthRegistration, OAuthError> {
        self.request_worker(
            move |db| set(db, input, id, secret_hash, now),
            || OAuthError::Unavailable,
        )
        .await
    }
    async fn oauth_revoke_registration(
        &self,
        input: OAuthRegistrationRevokeParams,
        now: u64,
    ) -> Result<OAuthRegistration, OAuthError> {
        self.request_worker(
            move |db| revoke_registration(db, input, now),
            || OAuthError::Unavailable,
        )
        .await
    }
    async fn oauth_issue(&self, input: OAuthIssue, now: u64) -> Result<(), OAuthError> {
        self.request_worker(move |db| issue(db, input, now), || OAuthError::Unavailable)
            .await
    }
    async fn oauth_refresh(
        &self,
        hash: [u8; 32],
        id: String,
        now: u64,
    ) -> Result<OAuthTokenState, OAuthError> {
        self.request_worker(
            move |db| refresh(db, &hash, &id, now),
            || OAuthError::Unavailable,
        )
        .await
    }
    async fn oauth_revoke_token(
        &self,
        hash: [u8; 32],
        id: String,
        now: u64,
    ) -> Result<(), OAuthError> {
        self.request_worker(
            move |db| match token(db, &hash) {
                Ok(t) if t.id == id => revoke_family(db, &t.family, now),
                Ok(_) | Err(OAuthError::InvalidGrant) => Ok(()),
                Err(error) => Err(error),
            },
            || OAuthError::Unavailable,
        )
        .await
    }
    async fn oauth_authenticate(
        &self,
        guard: OAuthAccessGuard,
        now: u64,
    ) -> Result<OAuthAuthenticated, OAuthError> {
        self.request_worker(
            move |db| {
                let client = authenticate(db, &guard, now)?;
                let t = token(db, &guard.token_hash)?;
                let mut scopes: Vec<_> = t.grants.into_iter().map(|g| g.permission).collect();
                scopes.sort();
                scopes.dedup();
                Ok(OAuthAuthenticated { client, scopes })
            },
            || OAuthError::Unavailable,
        )
        .await
    }
}

#[cfg(test)]
#[path = "m8_oauth_tests.rs"]
mod tests;
