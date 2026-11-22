use crate::{StateStoreHandle, m8_mcp, m8_oauth};
use alcomd_application::*;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

const CREATE: &str = "mcp.clients.requests.create";
const DECIDE: &str = "mcp.clients.requests.decide";

fn unavailable<T>(_: T) -> OAuthError {
    OAuthError::Unavailable
}
fn sql_time(value: u64) -> Result<i64, OAuthError> {
    i64::try_from(value).map_err(|_| OAuthError::InvalidInput)
}
fn identity(error: McpIdentityError) -> OAuthError {
    match error {
        McpIdentityError::InvalidInput => OAuthError::InvalidInput,
        McpIdentityError::PermissionDenied => OAuthError::Denied,
        McpIdentityError::RevisionConflict => OAuthError::RevisionConflict,
        McpIdentityError::CredentialResponseUnavailable => OAuthError::ResponseUnavailable,
        McpIdentityError::IdempotencyConflict => OAuthError::IdempotencyConflict,
        McpIdentityError::LimitExceeded => OAuthError::LimitExceeded,
        McpIdentityError::Unavailable => OAuthError::Unavailable,
    }
}

fn read(db: &Connection, id: McpClientId, now: u64) -> Result<ClientManagementReview, OAuthError> {
    let (requester, action, status, expires, fingerprint): (String, String, String, i64, String) =
        db.query_row("SELECT requester,action_json,status,expires_at_ms,fingerprint FROM mcp_client_requests WHERE request_id=?1",
            [id.to_string()], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))
            .optional().map_err(unavailable)?.ok_or(OAuthError::InvalidInput)?;
    let expires = u64::try_from(expires).map_err(unavailable)?;
    let status = match status.as_str() {
        "pending" if now >= expires => ClientRequestStatus::Expired,
        "pending" => ClientRequestStatus::Pending,
        "approved" => ClientRequestStatus::Approved,
        "denied" => ClientRequestStatus::Denied,
        _ => return Err(OAuthError::Unavailable),
    };
    Ok(ClientManagementReview {
        request_id: id,
        requester: PrincipalId::parse(requester).map_err(unavailable)?,
        action: serde_json::from_str(&action).map_err(unavailable)?,
        status,
        expires_at_ms: expires,
        fingerprint,
    })
}

fn receipt(
    db: &Connection,
    actor: &PrincipalId,
    method: &str,
    key: &str,
    fingerprint: &str,
) -> Result<Option<McpClientId>, OAuthError> {
    let record: Option<(String, String)> = db
        .query_row(
            "SELECT request_fingerprint,response_json FROM idempotency_records
         WHERE principal_id=?1 AND method=?2 AND idempotency_key=?3",
            params![actor.as_str(), method, key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(unavailable)?;
    record
        .map(|(saved, id)| {
            let saved: String = serde_json::from_str(&saved).map_err(unavailable)?;
            if saved != fingerprint {
                return Err(OAuthError::IdempotencyConflict);
            }
            serde_json::from_str(&id).map_err(unavailable)
        })
        .transpose()
}

fn save_receipt(
    tx: &Transaction<'_>,
    actor: &PrincipalId,
    method: &str,
    key: &str,
    fp: &str,
    id: McpClientId,
    now: u64,
) -> Result<(), OAuthError> {
    tx.execute("INSERT INTO idempotency_records(principal_id,method,idempotency_key,request_fingerprint,state,response_json,created_at_ms)
        VALUES (?1,?2,?3,?4,'completed',?5,?6)", params![actor.as_str(),method,key,
            serde_json::to_string(fp).map_err(unavailable)?,serde_json::to_string(&id).map_err(unavailable)?,sql_time(now)?])
        .map_err(unavailable)?;
    Ok(())
}

fn apply(
    tx: &Transaction<'_>,
    owner: &PrincipalId,
    action: &ClientManagementAction,
    now: u64,
    secret: String,
) -> Result<(Option<String>, Option<String>), OAuthError> {
    let key = McpClientId::new().to_string();
    let mut credential = None;
    let write = match action {
        ClientManagementAction::CreateClient { display_name } => McpIdentityWrite::Create {
            name: display_name.clone(),
        },
        ClientManagementAction::IssueCredential {
            client_id,
            expected_revision,
        } => {
            let hash = mcp_credential_hash(&secret);
            credential = Some(secret);
            McpIdentityWrite::Issue {
                id: *client_id,
                expected: *expected_revision,
                hash,
            }
        }
        ClientManagementAction::RevokeClient {
            client_id,
            expected_revision,
        } => McpIdentityWrite::Revoke {
            id: *client_id,
            expected: *expected_revision,
        },
        ClientManagementAction::SetOAuthRegistration { confidential, .. } => {
            let input = action.registration(key).ok_or(OAuthError::InvalidInput)?;
            let secret = confidential.then_some(secret);
            m8_oauth::set_on(
                tx,
                owner,
                input,
                McpClientId::new().to_string(),
                secret.as_deref().map(mcp_credential_hash),
                now,
            )?;
            return Ok((None, secret));
        }
        ClientManagementAction::RevokeOAuthRegistration {
            oauth_client_id,
            expected_revision,
        } => {
            m8_oauth::revoke_registration_on(
                tx,
                owner,
                OAuthRegistrationRevokeParams {
                    oauth_client_id: oauth_client_id.to_string(),
                    expected_revision: *expected_revision,
                    idempotency_key: key,
                },
                now,
            )?;
            return Ok((None, None));
        }
    };
    m8_mcp::write_on(tx, owner.clone(), key, write, now).map_err(identity)?;
    Ok((credential, None))
}

impl ClientRequestStore for StateStoreHandle {
    async fn create_request(
        &self,
        requester: PrincipalId,
        action: ClientManagementAction,
        key: String,
        now: u64,
    ) -> Result<ClientManagementReview, OAuthError> {
        let actor = m8_mcp::AdministrativeActor::from(requester);
        self.request_worker(move |db| {
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(unavailable)?;
            actor.verify(&tx).map_err(identity)?;
            let fp = oauth_fingerprint(&action)?;
            if let Some(id) = receipt(&tx,&actor.principal,CREATE,&key,&fp)? { return read(&tx,id,now); }
            let outstanding: i64 = tx.query_row("SELECT count(*) FROM mcp_client_requests
                WHERE requester=?1 AND status='pending' AND expires_at_ms>?2",params![actor.principal.as_str(),sql_time(now)?],|r|r.get(0)).map_err(unavailable)?;
            if outstanding >= 32 { return Err(OAuthError::LimitExceeded); }
            let expires = now.checked_add(600_000).ok_or(OAuthError::InvalidInput)?;
            let id = McpClientId::new();
            let fingerprint = oauth_fingerprint(&(id, &actor.principal, &action, now, expires))?;
            tx.execute("INSERT INTO mcp_client_requests(request_id,requester,action_json,fingerprint,created_at_ms,expires_at_ms,status)
                VALUES (?1,?2,?3,?4,?5,?6,'pending')",params![id.to_string(),actor.principal.as_str(),serde_json::to_string(&action).map_err(unavailable)?,fingerprint,sql_time(now)?,sql_time(expires)?]).map_err(unavailable)?;
            save_receipt(&tx,&actor.principal,CREATE,&key,&fp,id,now)?;
            let result = read(&tx,id,now)?;
            tx.commit().map_err(unavailable)?;
            Ok(result)
        }, || OAuthError::Unavailable).await
    }

    async fn get_request(
        &self,
        actor: PrincipalId,
        id: McpClientId,
        now: u64,
    ) -> Result<ClientManagementReview, OAuthError> {
        let actor = m8_mcp::AdministrativeActor::from(actor);
        self.request_worker(
            move |db| {
                let tx = db.transaction().map_err(unavailable)?;
                actor.verify(&tx).map_err(identity)?;
                let record = read(&tx, id, now)?;
                if actor
                    .scope()
                    .is_some_and(|scope| scope != record.requester.as_str())
                {
                    return Err(OAuthError::Denied);
                }
                Ok(record)
            },
            || OAuthError::Unavailable,
        )
        .await
    }

    async fn list_requests(
        &self,
        actor: PrincipalId,
        input: OAuthPageParams,
        now: u64,
    ) -> Result<ClientRequestPage, OAuthError> {
        let actor = m8_mcp::AdministrativeActor::from(actor);
        self.request_worker(move |db| {
            let tx = db.transaction().map_err(unavailable)?;
            actor.verify(&tx).map_err(identity)?;
            let clause = "FROM mcp_client_requests WHERE ?1 IS NULL OR requester=?1";
            let (count,max_row,approved,denied,expired): (i64,i64,i64,i64,i64) = tx.query_row(
                &format!("SELECT count(*),coalesce(max(rowid),0),count(*) FILTER(WHERE status='approved'),
                    count(*) FILTER(WHERE status='denied'),count(*) FILTER(WHERE status='pending' AND expires_at_ms<=?2) {clause}"),
                params![actor.scope(),sql_time(now)?],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).map_err(unavailable)?;
            let fp = oauth_fingerprint(&(&actor.principal,count,max_row,approved,denied,expired))?;
            let count = usize::try_from(count).map_err(unavailable)?;
            let (start,limit) = oauth_page_window(&input.cursor,&fp,count,input.limit)?;
            let mut stmt = tx.prepare(&format!("SELECT request_id {clause} ORDER BY created_at_ms DESC,request_id LIMIT ?2 OFFSET ?3")).map_err(unavailable)?;
            let ids = stmt.query_map(params![actor.scope(),i64::try_from(limit).map_err(unavailable)?,i64::try_from(start).map_err(unavailable)?], |r|r.get::<_,String>(0)).map_err(unavailable)?;
            let mut items = Vec::new();
            let mut bytes = 1024;
            for id in ids {
                let record = read(&tx,McpClientId::parse(&id.map_err(unavailable)?).map_err(unavailable)?,now)?;
                bytes += serde_json::to_vec(&record).map_err(unavailable)?.len() + 1;
                if bytes > 256*1024 { break; }
                items.push(record);
            }
            let end = start + items.len();
            Ok(ClientRequestPage { items, next_cursor: (end < count).then(|| format!("{fp}:{end}")) })
        }, || OAuthError::Unavailable).await
    }

    async fn decide_request(
        &self,
        owner: PrincipalId,
        input: ClientRequestDecisionInput,
        secret: String,
        now: u64,
    ) -> Result<ClientManagementDecision, OAuthError> {
        let actor = m8_mcp::AdministrativeActor::from(owner);
        self.request_worker(move |db| {
            let tx = db.transaction_with_behavior(TransactionBehavior::Immediate).map_err(unavailable)?;
            actor.verify_owner(&tx).map_err(identity)?;
            let id = input.request_id;
            let mut record = read(&tx,id,now)?;
            if record.fingerprint != input.expected_fingerprint { return Err(OAuthError::RevisionConflict); }
            if let Some(client_id) = record.requester.as_str().strip_prefix("mcp:")
                && m8_mcp::client(&tx, client_id).map_err(identity)?.revoked_at_ms.is_some()
            {
                return Err(OAuthError::Denied);
            }
            let fp = oauth_fingerprint(&(id,input.approve,&record.fingerprint))?;
            if receipt(&tx,&actor.principal,DECIDE,&input.idempotency_key,&fp)?.is_some() {
                return Ok(ClientManagementDecision {request:record,credential:None,client_secret:None});
            }
            if record.status == ClientRequestStatus::Expired {return Err(OAuthError::Expired);}
            if record.status != ClientRequestStatus::Pending {return Err(OAuthError::RevisionConflict);}
            let (status,credential,client_secret) = if input.approve {
                let (credential,client_secret) = apply(&tx,&actor.principal,&record.action,now,secret)?;
                record.status = ClientRequestStatus::Approved;
                ("approved",credential,client_secret)
            } else {
                record.status = ClientRequestStatus::Denied;
                ("denied",None,None)
            };
            tx.execute("UPDATE mcp_client_requests SET status=?2,decided_by=?3,decided_at_ms=?4 WHERE request_id=?1",
                params![id.to_string(),status,actor.principal.as_str(),sql_time(now)?]).map_err(unavailable)?;
            save_receipt(&tx,&actor.principal,DECIDE,&input.idempotency_key,&fp,id,now)?;
            tx.commit().map_err(unavailable)?;
            Ok(ClientManagementDecision { request:record,credential,client_secret })
        }, || OAuthError::Unavailable).await
    }
}
