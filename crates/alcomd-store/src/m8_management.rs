use crate::StateStoreHandle;
use alcomd_application::{
    ExtensionDesiredState, ExtensionInstanceLease, ExtensionInvocationKind,
    ExtensionQuarantineState, ExtensionUiInvocationAuthority, MCP_MANAGEMENT_BYTES,
    ManagementClientPage, ManagementClientSummary, ManagementError, ManagementErrorCode as E,
    ManagementPage, ManagementRequest as Q, ManagementResponse as R, McpIdentityError,
    McpIdentityWrite, McpManagementStore, Permission, management_client_fingerprint,
};
use rusqlite::{Connection, TransactionBehavior, params};
use serde::Serialize;

fn unavailable<T>(_: T) -> ManagementError {
    E::Unavailable.into()
}
fn row_u64(row: &rusqlite::Row<'_>, index: usize) -> rusqlite::Result<u64> {
    u64::try_from(row.get::<_, i64>(index)?).map_err(|_| rusqlite::Error::InvalidQuery)
}
fn identity(error: McpIdentityError) -> ManagementError {
    match error {
        McpIdentityError::InvalidInput => E::InvalidArgument,
        McpIdentityError::PermissionDenied => E::InvalidArgument,
        McpIdentityError::RevisionConflict => E::RevisionConflict,
        McpIdentityError::IdempotencyConflict => E::IdempotencyConflict,
        McpIdentityError::LimitExceeded => E::ResourceLimit,
        McpIdentityError::Unavailable | McpIdentityError::CredentialResponseUnavailable => {
            E::Unavailable
        }
    }
    .into()
}

impl McpManagementStore for StateStoreHandle {
    async fn mcp_management(
        &self,
        lease: ExtensionInstanceLease,
        authority: ExtensionUiInvocationAuthority,
        request: Q,
        now_ms: u64,
    ) -> Result<R, ManagementError> {
        let queued_at = std::time::Instant::now();
        self.request_worker(
            move |db| {
                // Include queue delay: an unexpired call at dispatch may expire before acceptance.
                let elapsed = u64::try_from(queued_at.elapsed().as_millis()).unwrap_or(u64::MAX);
                execute(
                    db,
                    &lease,
                    &authority,
                    request,
                    now_ms.saturating_add(elapsed),
                )
            },
            || E::Unavailable.into(),
        )
        .await
    }
}

fn execute(
    db: &mut Connection,
    lease: &ExtensionInstanceLease,
    authority: &ExtensionUiInvocationAuthority,
    request: Q,
    now: u64,
) -> Result<R, ManagementError> {
    if serde_json::to_vec(&request).map_err(unavailable)?.len() > MCP_MANAGEMENT_BYTES {
        return Err(E::ResourceLimit.into());
    }
    let is_write = request.is_write();
    let tx = db
        .transaction_with_behavior(if is_write {
            TransactionBehavior::Immediate
        } else {
            TransactionBehavior::Deferred
        })
        .map_err(unavailable)?;
    authorize(&tx, lease, authority, &request, now)?;
    if let Q::Revoke {
        expected_revision, ..
    } = &request
        && (*expected_revision == 0 || *expected_revision > i64::MAX as u64)
    {
        return Err(E::InvalidArgument.into());
    }
    let binding = serde_json::json!([
        lease.extension_id,
        lease.instance_id,
        lease.lease_id,
        lease.grant_revision.get(),
        lease.lifecycle_generation.get(),
        lease.daemon_epoch,
        management_client_fingerprint(authority)
    ]);
    let response = match request {
        Q::List { cursor, limit } => {
            let mut query = tx.prepare("SELECT c.client_id,c.display_name,c.revision,c.created_at_ms,c.revoked_at_ms FROM mcp_clients c WHERE ?1 IS NULL OR c.principal_id=?1 ORDER BY c.client_id").map_err(unavailable)?;
            let scope = authority
                .client_access
                .is_mcp_client()
                .then(|| authority.client_access.principal().as_str());
            let items = query
                .query_map([scope], |row| {
                    let id: String = row.get(0)?;
                    Ok(ManagementClientSummary {
                        client_id: alcomd_application::McpClientId::parse(&id)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                        display_name: row.get(1)?,
                        revision: row_u64(row, 2)?,
                        created_at_ms: row_u64(row, 3)?,
                        revoked_at_ms: row
                            .get::<_, Option<i64>>(4)?
                            .map(u64::try_from)
                            .transpose()
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    })
                })
                .map_err(unavailable)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(unavailable)?;
            let can_manage = authority.client_access.is_local_owner()
                && authority.is_current()
                && authority
                    .client_access
                    .require(Permission::McpClientsManage)
                    .is_ok()
                && management_granted(&tx, lease, Permission::McpClientsManage)?;
            let result = page(
                &items,
                cursor,
                limit.unwrap_or(32),
                64,
                &serde_json::json!(["list", binding, can_manage]),
            )?;
            R::Clients(ManagementClientPage {
                items: result.items,
                next_cursor: result.next_cursor,
                can_manage,
            })
        }
        Q::Create {
            display_name,
            idempotency_key,
        } => {
            if display_name.trim().is_empty()
                || display_name.chars().count() > 128
                || display_name.chars().any(char::is_control)
            {
                return Err(E::InvalidArgument.into());
            }
            write(
                &tx,
                lease,
                idempotency_key,
                McpIdentityWrite::Create { name: display_name },
                now,
            )?
        }
        Q::Revoke {
            client_id,
            expected_revision,
            idempotency_key,
        } => write(
            &tx,
            lease,
            idempotency_key,
            McpIdentityWrite::Revoke {
                id: client_id,
                expected: expected_revision,
            },
            now,
        )?,
    };
    if serde_json::to_vec(&response).map_err(unavailable)?.len() > MCP_MANAGEMENT_BYTES {
        return Err(E::ResourceLimit.into());
    }
    tx.commit().map_err(|_| {
        ManagementError::from(if is_write {
            E::WriteOutcomeUnknown
        } else {
            E::Unavailable
        })
    })?;
    Ok(response)
}

fn write(
    tx: &rusqlite::Transaction<'_>,
    lease: &ExtensionInstanceLease,
    key: String,
    command: McpIdentityWrite,
    now: u64,
) -> Result<R, ManagementError> {
    alcomd_application::IdempotencyKey::parse(key.clone())
        .map_err(|_| ManagementError::from(E::InvalidArgument))?;
    let result = crate::m8_mcp::write_on(tx, lease.principal_id.clone(), key, command, now)
        .map_err(identity)?;
    Ok(R::Client(ManagementClientSummary::from(&result.client)))
}

fn authorize(
    db: &Connection,
    lease: &ExtensionInstanceLease,
    authority: &ExtensionUiInvocationAuthority,
    request: &Q,
    now: u64,
) -> Result<(), ManagementError> {
    let permission = match request {
        request if request.is_write() => Permission::McpClientsManage,
        _ => Permission::McpClientsRead,
    };
    if request.is_write() && !authority.client_access.is_local_owner() {
        return Err(E::PermissionDenied.into());
    }
    authorize_permission(
        db,
        lease,
        authority,
        Some(permission),
        request.is_write(),
        now,
    )
}

fn authorize_permission(
    db: &Connection,
    lease: &ExtensionInstanceLease,
    authority: &ExtensionUiInvocationAuthority,
    permission: Option<Permission>,
    is_write: bool,
    now: u64,
) -> Result<(), ManagementError> {
    if now >= lease.expires_at_ms {
        return Err(E::LeaseExpired.into());
    }
    if !authority.is_current() || now >= authority.deadline_ms {
        return Err(E::Cancelled.into());
    }
    if !matches!(
        authority.kind,
        ExtensionInvocationKind::InteractiveUiRender | ExtensionInvocationKind::InteractiveUiAction
    ) || (is_write && authority.kind != ExtensionInvocationKind::InteractiveUiAction)
    {
        return Err(E::PermissionDenied.into());
    }
    authority
        .client_access
        .require_extension_ui_scope(&lease.extension_id)
        .map_err(|_| ManagementError::from(E::ScopeDenied))?;
    if let Some(permission) = permission {
        authority
            .client_access
            .require(permission)
            .map_err(|_| ManagementError::from(E::PermissionDenied))?;
    }
    crate::m6::validate_lease(db, lease, now)
        .map_err(|_| ManagementError::from(E::LeaseRevoked))?;
    let extension =
        crate::m6::get_extension(db, authority.client_access.principal(), &lease.extension_id)
            .map_err(|_| ManagementError::from(E::ScopeDenied))?;
    if extension.desired_state != ExtensionDesiredState::Enabled
        || extension.quarantine_state != ExtensionQuarantineState::Clear
        || extension.grant_revision != lease.grant_revision
        || extension.lifecycle_generation != lease.lifecycle_generation
    {
        return Err(E::LeaseRevoked.into());
    }
    if let Some(permission) = permission
        && !management_granted(db, lease, permission)?
    {
        return Err(E::PermissionDenied.into());
    }
    Ok(())
}

fn management_granted(
    db: &Connection,
    lease: &ExtensionInstanceLease,
    permission: Permission,
) -> Result<bool, ManagementError> {
    db.query_row("SELECT EXISTS(SELECT 1 FROM extension_grants WHERE extension_id=?1 AND permission_name=?2 AND resource_kind='Global' AND resource_id='*' AND state='granted')",params![lease.extension_id, permission.as_str()],|r|r.get(0)).map_err(unavailable)
}

fn page<T: Clone + Serialize>(
    items: &[T],
    cursor: Option<String>,
    limit: u32,
    max: u32,
    binding: &serde_json::Value,
) -> Result<ManagementPage<T>, ManagementError> {
    if !(1..=max).contains(&limit) {
        return Err(E::InvalidArgument.into());
    }
    let fingerprint: String = alcomd_application::mcp_credential_hash(
        &serde_json::to_string(&(binding, items)).map_err(unavailable)?,
    )
    .iter()
    .map(|b| format!("{b:02x}"))
    .collect();
    let start = match cursor {
        None => 0,
        Some(cursor) => {
            if cursor.len() > 512 || !cursor.is_ascii() {
                return Err(E::InvalidArgument.into());
            }
            let (old, offset) = cursor
                .split_once(':')
                .ok_or_else(|| ManagementError::from(E::InvalidArgument))?;
            if old != fingerprint {
                return Err(E::RevisionConflict.into());
            }
            offset
                .parse::<usize>()
                .map_err(|_| ManagementError::from(E::InvalidArgument))?
        }
    };
    if start > items.len() {
        return Err(E::InvalidArgument.into());
    }
    let mut result = Vec::new();
    let mut bytes = 1024;
    for item in items.iter().skip(start).take(limit as usize) {
        let size = serde_json::to_vec(item).map_err(unavailable)?.len() + 1;
        if bytes + size > MCP_MANAGEMENT_BYTES - 1024 {
            break;
        }
        bytes += size;
        result.push(item.clone());
    }
    let end = start + result.len();
    if end == start && end < items.len() {
        return Err(E::ResourceLimit.into());
    }
    Ok(ManagementPage {
        items: result,
        next_cursor: (end < items.len()).then(|| format!("{fingerprint}:{end}")),
    })
}

impl alcomd_application::McpRuntimeAuthorityStore for StateStoreHandle {
    async fn mcp_runtime_access(
        &self,
        lease: ExtensionInstanceLease,
        authority: ExtensionUiInvocationAuthority,
        now_ms: u64,
    ) -> Result<alcomd_application::RuntimeReadAccess, ManagementError> {
        let queued_at = std::time::Instant::now();
        self.request_worker(
            move |db| {
                let now = now_ms.saturating_add(
                    u64::try_from(queued_at.elapsed().as_millis()).unwrap_or(u64::MAX),
                );
                let tx = db
                    .transaction_with_behavior(TransactionBehavior::Deferred)
                    .map_err(unavailable)?;
                authorize_permission(&tx, &lease, &authority, None, false, now)?;
                let client =
                    alcomd_application::RuntimeReadAccess::from_client(&authority.client_access);
                let access = alcomd_application::RuntimeReadAccess {
                    connections: client.connections
                        && management_granted(&tx, &lease, Permission::McpConnectionsRead)?,
                    requests: client.requests
                        && management_granted(&tx, &lease, Permission::McpRequestsRead)?,
                };
                if !access.connections && !access.requests {
                    return Err(E::PermissionDenied.into());
                }
                tx.commit().map_err(unavailable)?;
                Ok(access)
            },
            || E::Unavailable.into(),
        )
        .await
    }
}

impl alcomd_application::McpTransportAuthorityStore for StateStoreHandle {
    async fn mcp_transport_access(
        &self,
        lease: ExtensionInstanceLease,
        authority: ExtensionUiInvocationAuthority,
        now_ms: u64,
    ) -> Result<alcomd_application::TransportReadAccess, ManagementError> {
        let queued_at = std::time::Instant::now();
        self.request_worker(
            move |db| {
                let now = now_ms.saturating_add(
                    u64::try_from(queued_at.elapsed().as_millis()).unwrap_or(u64::MAX),
                );
                let tx = db
                    .transaction_with_behavior(TransactionBehavior::Deferred)
                    .map_err(unavailable)?;
                authorize_permission(&tx, &lease, &authority, None, false, now)?;
                let client =
                    alcomd_application::TransportReadAccess::from_client(&authority.client_access);
                let access = alcomd_application::TransportReadAccess {
                    connections: client.connections
                        && management_granted(&tx, &lease, Permission::McpConnectionsRead)?,
                    streams: client.streams
                        && management_granted(&tx, &lease, Permission::McpSubscriptionStreamsRead)?,
                    requests: client.requests
                        && management_granted(&tx, &lease, Permission::McpRequestsRead)?,
                };
                if !access.connections && !access.requests && !access.streams {
                    return Err(E::PermissionDenied.into());
                }
                tx.commit().map_err(unavailable)?;
                Ok(access)
            },
            || E::Unavailable.into(),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn management_create_and_revoke_keep_replay_bound_to_the_actual_actor() {
        let mut db = crate::sqlite::current_schema_fixture();
        let actor = alcomd_application::PrincipalId::parse("extension:dev.alcomd.manager")
            .expect("extension principal");
        let other = alcomd_application::PrincipalId::local_owner();
        let write = |db: &mut Connection, actor, key: &str, command| {
            let tx = db
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .expect("tx");
            let result = crate::m8_mcp::write_on(&tx, actor, key.into(), command, 10)?;
            tx.commit().expect("commit");
            Ok::<_, McpIdentityError>(result.client)
        };
        let create = || McpIdentityWrite::Create {
            name: "Client".into(),
        };
        let first = write(&mut db, actor.clone(), "create", create()).expect("create");
        let replayed = write(&mut db, actor.clone(), "create", create()).expect("replay");
        let separate = write(&mut db, other, "create", create()).expect("separate actor");
        assert_eq!(first, replayed);
        assert_ne!(first.client_id, separate.client_id);
        let revoke = || McpIdentityWrite::Revoke {
            id: first.client_id,
            expected: first.revision,
        };
        let revoked = write(&mut db, actor.clone(), "revoke", revoke()).expect("revoke");
        assert!(revoked.revoked_at_ms.is_some());
        assert_eq!(
            write(&mut db, actor, "revoke", revoke()).expect("replay revoke"),
            revoked
        );
    }
}
