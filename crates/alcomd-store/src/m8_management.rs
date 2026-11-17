use crate::StateStoreHandle;
use alcomd_application::{
    ExtensionDesiredState, ExtensionInstanceLease, ExtensionInvocationKind,
    ExtensionQuarantineState, ExtensionUiInvocationAuthority, MCP_MANAGEMENT_BYTES,
    ManagementClientPage, ManagementClientSummary, ManagementError, ManagementErrorCode as E,
    ManagementGrantPage, ManagementPage, ManagementProject, ManagementRequest as Q,
    ManagementResponse as R, McpIdentityError, McpIdentityWrite, McpManagementStore, Permission,
    PrincipalId, management_client_fingerprint,
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
    if let Q::Grants {
        expected_revision, ..
    }
    | Q::ChangeGrants {
        expected_revision, ..
    }
    | Q::Revoke {
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
            let mut query = tx.prepare("SELECT c.client_id,c.display_name,c.revision,c.created_at_ms,c.revoked_at_ms,(SELECT count(*) FROM mcp_client_grants g WHERE g.client_id=c.client_id AND g.grant_revision=c.grant_revision) FROM mcp_clients c ORDER BY c.client_id").map_err(unavailable)?;
            let items = query
                .query_map([], |row| {
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
                        grant_count: row.get(5)?,
                    })
                })
                .map_err(unavailable)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(unavailable)?;
            let can_manage = authority.is_current()
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
        Q::Grants {
            client_id,
            expected_revision,
            cursor,
            limit,
        } => {
            let revision: Result<(u64, i64), _> = tx.query_row(
                "SELECT revision,grant_revision FROM mcp_clients WHERE client_id=?1",
                [client_id.to_string()],
                |r| Ok((row_u64(r, 0)?, r.get(1)?)),
            );
            let (revision, snapshot) =
                revision.map_err(|_| ManagementError::from(E::InvalidArgument))?;
            if revision != expected_revision {
                return Err(E::RevisionConflict.into());
            }
            let items = crate::m8_mcp::grant_snapshot(&tx, &client_id.to_string(), snapshot)
                .map_err(identity)?;
            let result = page(
                &items,
                cursor,
                limit.unwrap_or(64),
                128,
                &serde_json::json!(["grants", binding, client_id, revision, snapshot]),
            )?;
            R::Grants(ManagementGrantPage {
                revision,
                items: result.items,
                next_cursor: result.next_cursor,
            })
        }
        Q::Projects { cursor, limit } => {
            let mut query = tx.prepare("SELECT project_id FROM projects WHERE owner_principal_id=?1 ORDER BY project_id").map_err(unavailable)?;
            let ids = query
                .query_map([PrincipalId::LOCAL_OWNER], |r| r.get::<_, String>(0))
                .map_err(unavailable)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(unavailable)?;
            let mut items = Vec::new();
            for id in ids {
                let summary = crate::m6::project_summary(&tx, lease, &id, now)
                    .map_err(|_| ManagementError::from(E::ScopeDenied))?;
                items.push(ManagementProject {
                    project_id: id,
                    name: summary.display_name,
                });
            }
            R::Projects(page(
                &items,
                cursor,
                limit.unwrap_or(32),
                64,
                &serde_json::json!(["projects", binding]),
            )?)
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
                McpIdentityWrite::Create {
                    name: display_name,
                    grants: vec![],
                },
                now,
            )?
        }
        Q::ChangeGrants {
            client_id,
            expected_revision,
            add,
            remove,
            idempotency_key,
        } => {
            alcomd_application::validate_mcp_grant_delta(&add, &remove).map_err(identity)?;
            write(
                &tx,
                lease,
                idempotency_key,
                McpIdentityWrite::ChangeGrants {
                    id: client_id,
                    expected: expected_revision,
                    add,
                    remove,
                },
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
        Q::Projects { .. } => Permission::ProjectsRead,
        request if request.is_write() => Permission::McpClientsManage,
        _ => Permission::McpClientsRead,
    };
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
    // MCP clients cannot possess administration/UI authority. Never reinterpret them as owner.
    if authority.client_access.is_mcp_client() {
        return Err(E::PermissionDenied.into());
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
    use alcomd_application::{McpGrant, McpIdentityWrite as W, PrincipalId};

    fn fixture() -> Connection {
        let db = crate::sqlite::mcp_state32_fixture();
        db.execute_batch(include_str!("../migrations/0033_mcp_clients.sql"))
            .expect("33");
        db.execute_batch(include_str!("../migrations/0034_mcp_grant_snapshots.sql"))
            .expect("34");
        db
    }
    fn global(permission: &str) -> McpGrant {
        McpGrant {
            permission: permission.into(),
            resource_kind: "global".into(),
            resource_id: "*".into(),
        }
    }
    fn command(
        db: &mut Connection,
        actor: PrincipalId,
        key: &str,
        command: W,
    ) -> Result<alcomd_application::McpClient, McpIdentityError> {
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .expect("transaction");
        let result = crate::m8_mcp::write_on(&tx, actor, key.into(), command, 10)?.client;
        tx.commit().expect("commit");
        Ok(result)
    }
    #[test]
    fn delta_acceptance_is_atomic_and_original_input_replays_without_restoring_old_grants() {
        let mut db = fixture();
        let actor = PrincipalId::parse("extension:dev.alcomd.manager").expect("actor");
        let client = command(
            &mut db,
            actor.clone(),
            "create",
            W::Create {
                name: "Client".into(),
                grants: vec![],
            },
        )
        .expect("create");
        let delta = || W::ChangeGrants {
            id: client.client_id,
            expected: 1,
            add: vec![global("state.check"), global("operations.read")],
            remove: vec![],
        };
        let first = command(&mut db, actor.clone(), "patch", delta()).expect("patch");
        assert_eq!(first.revision, 2);
        assert_eq!(first.grants.len(), 2);
        let changed = command(
            &mut db,
            actor.clone(),
            "later",
            W::ChangeGrants {
                id: client.client_id,
                expected: 2,
                add: vec![],
                remove: vec![global("state.check")],
            },
        )
        .expect("remove");
        assert_eq!(changed.revision, 3);
        let replay =
            command(&mut db, actor.clone(), "patch", delta()).expect("exact original replay");
        assert_eq!(replay, first);
        let current: i64 = db
            .query_row("SELECT revision FROM mcp_clients", [], |r| r.get(0))
            .expect("revision");
        assert_eq!(current, 3);
        let snapshot: i64 = db
            .query_row("SELECT grant_revision FROM mcp_clients", [], |r| r.get(0))
            .expect("snapshot");
        assert_eq!(
            crate::m8_mcp::grant_snapshot(&db, &client.client_id.to_string(), snapshot)
                .expect("current"),
            changed.grants
        );
        assert_eq!(
            command(
                &mut db,
                actor.clone(),
                "patch",
                W::ChangeGrants {
                    id: client.client_id,
                    expected: 1,
                    add: vec![global("events.read")],
                    remove: vec![]
                }
            )
            .expect_err("same key changed input"),
            McpIdentityError::IdempotencyConflict
        );
        let other = PrincipalId::parse("extension:dev.alcomd.other").expect("actor");
        assert_eq!(
            command(&mut db, other, "patch", delta())
                .expect_err("other actor cannot replay original"),
            McpIdentityError::RevisionConflict
        );
        assert_eq!(
            command(
                &mut db,
                actor,
                "bad",
                W::ChangeGrants {
                    id: client.client_id,
                    expected: 3,
                    add: vec![McpGrant {
                        permission: "projects.read".into(),
                        resource_kind: "project".into(),
                        resource_id: "00000000-0000-4000-8000-000000000001".into()
                    }],
                    remove: vec![global("operations.read")]
                }
            )
            .expect_err("missing project rolls back removal too"),
            McpIdentityError::InvalidInput
        );
        assert_eq!(
            crate::m8_mcp::grant_snapshot(&db, &client.client_id.to_string(), snapshot)
                .expect("still current"),
            changed.grants
        );
    }
    #[test]
    fn old_project_grants_migrate_out_of_current_authority_without_rewriting_history() {
        let mut db = fixture();
        let owner = PrincipalId::local_owner();
        let client = command(
            &mut db,
            owner.clone(),
            "create",
            W::Create {
                name: "Orphans".into(),
                grants: vec![],
            },
        )
        .expect("create");
        let ids = [
            "00000000-0000-4000-8000-000000000001",
            "00000000-0000-4000-8000-000000000002",
        ];
        for id in ids {
            db.execute(
                "INSERT INTO mcp_client_grants VALUES (?1,1,'projects.read','project',?2)",
                params![client.client_id.to_string(), id],
            )
            .expect("historical orphan snapshot");
        }
        let removed = McpGrant {
            permission: "projects.read".into(),
            resource_kind: "project".into(),
            resource_id: ids[0].into(),
        };
        for migration in [
            include_str!("../migrations/0035_extension_mcp_permissions.sql"),
            include_str!("../migrations/0036_mcp_oauth.sql"),
            include_str!("../migrations/0037_capability_permission_switches.sql"),
        ] {
            db.execute_batch(migration).expect("approved migration");
        }
        let current =
            crate::m8_mcp::client(&db, &client.client_id.to_string()).expect("migrated current");
        assert_eq!(current.revision, 2);
        assert!(current.grants.is_empty(), "no implicit global read");
        assert_eq!(
            command(
                &mut db,
                owner.clone(),
                "old-remove",
                W::ChangeGrants {
                    id: client.client_id,
                    expected: 2,
                    add: vec![],
                    remove: vec![removed]
                }
            )
            .expect_err("old scope requests are no longer supported"),
            McpIdentityError::InvalidInput
        );
        let result = command(
            &mut db,
            owner,
            "allow",
            W::ChangeGrants {
                id: client.client_id,
                expected: 2,
                add: vec![global("projects.read")],
                remove: vec![],
            },
        )
        .expect("explicit ability grant");
        assert_eq!(result.revision, 3);
        assert_eq!(result.grants, vec![global("projects.read")]);
        assert_eq!(
            crate::m8_mcp::grant_snapshot(&db, &client.client_id.to_string(), 1)
                .expect_err("unsupported historical data is not an authorization result"),
            McpIdentityError::Unavailable
        );
        let mut query = db.prepare("SELECT resource_id FROM mcp_client_grants WHERE client_id=?1 AND grant_revision=1 ORDER BY resource_id").expect("raw evidence");
        let history = query
            .query_map([client.client_id.to_string()], |row| {
                row.get::<_, String>(0)
            })
            .expect("raw rows")
            .collect::<Result<Vec<_>, _>>()
            .expect("evidence");
        assert_eq!(history, ids);
    }
    #[test]
    fn current_capabilities_page_completely_without_project_authorization() {
        let mut db = fixture();
        let items = [
            "projects.read",
            "packages.manage",
            "state.check",
            "operations.read",
            "operations.cancel",
            "events.read",
            "repositories.read",
            "packages.read",
        ]
        .map(global)
        .to_vec();
        let current = command(
            &mut db,
            PrincipalId::local_owner(),
            "create-global",
            W::Create {
                name: "Capabilities".into(),
                grants: items.clone(),
            },
        )
        .expect("create");
        let snapshot = crate::m8_mcp::grant_snapshot(&db, &current.client_id.to_string(), 1)
            .expect("supported snapshot");
        let binding = serde_json::json!(["grants", current.client_id, 1]);
        let mut cursor = None;
        let mut seen = Vec::new();
        loop {
            let next = page(&snapshot, cursor, 1, 128, &binding).expect("capability page");
            assert_eq!(next.items.len(), 1);
            seen.extend(next.items);
            cursor = next.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        assert_eq!(seen, snapshot);
        assert_eq!(seen.len(), 8);
        assert!(
            seen.iter()
                .all(|g| g.resource_kind == "global" && g.resource_id == "*")
        );
    }

    #[test]
    fn unsupported_4096_rows_are_rejected_while_evidence_and_page_bounds_remain() {
        let mut db = fixture();
        let client = command(
            &mut db,
            PrincipalId::local_owner(),
            "create",
            W::Create {
                name: "Large".into(),
                grants: vec![],
            },
        )
        .expect("client");
        for n in 0..4096 {
            db.execute(
                "INSERT INTO mcp_client_grants VALUES (?1,1,'projects.read','project',?2)",
                params![
                    client.client_id.to_string(),
                    format!("{n:08x}-0000-4000-8000-000000000001")
                ],
            )
            .expect("grant fixture");
        }
        let raw_ids = |db: &Connection| {
            let mut query = db.prepare("SELECT resource_id FROM mcp_client_grants WHERE client_id=?1 AND grant_revision=1 ORDER BY resource_id").expect("raw evidence");
            query
                .query_map([client.client_id.to_string()], |r| r.get::<_, String>(0))
                .expect("rows")
                .collect::<Result<Vec<_>, _>>()
                .expect("evidence")
        };
        assert_eq!(
            crate::m8_mcp::grant_snapshot(&db, &client.client_id.to_string(), 1)
                .expect_err("unsupported configuration"),
            McpIdentityError::Unavailable
        );
        let items = raw_ids(&db);
        assert_eq!(items.len(), 4096);
        // Exercise the generic bounded page helper with raw evidence IDs, not
        // permissions: no runtime caller can read these rows as supported grants.
        let binding = serde_json::json!([
            "grants",
            client.client_id,
            1,
            "current-lease-and-two-sided-scope"
        ]);
        let first = page(&items, None, 128, 128, &binding).expect("first");
        let old = first.next_cursor.clone();
        let mut seen = first.items;
        let mut cursor = old.clone();
        let mut pages = 1;
        while cursor.is_some() {
            let next = page(&items, cursor, 128, 128, &binding).expect("page");
            assert!(serde_json::to_vec(&next).expect("bytes").len() < MCP_MANAGEMENT_BYTES);
            seen.extend(next.items);
            cursor = next.next_cursor;
            pages += 1;
        }
        assert_eq!(pages, 32);
        assert_eq!(seen, items);
        assert_eq!(
            command(
                &mut db,
                PrincipalId::local_owner(),
                "over-capacity",
                W::ChangeGrants {
                    id: client.client_id,
                    expected: 1,
                    add: vec![global("state.check")],
                    remove: vec![]
                }
            )
            .expect_err("unrecognized current data cannot be repaired by a grant write"),
            McpIdentityError::Unavailable
        );
        assert_eq!(
            command(
                &mut db,
                PrincipalId::local_owner(),
                "replace-one",
                W::ChangeGrants {
                    id: client.client_id,
                    expected: 1,
                    add: vec![global("state.check")],
                    remove: vec![McpGrant {
                        permission: "projects.read".into(),
                        resource_kind: "project".into(),
                        resource_id: items[0].clone()
                    }]
                }
            )
            .expect_err("old project grant format is rejected"),
            McpIdentityError::InvalidInput
        );
        assert_eq!(raw_ids(&db), items);
        for migration in [
            include_str!("../migrations/0035_extension_mcp_permissions.sql"),
            include_str!("../migrations/0036_mcp_oauth.sql"),
            include_str!("../migrations/0037_capability_permission_switches.sql"),
        ] {
            db.execute_batch(migration).expect("approved migration");
        }
        assert!(
            crate::m8_mcp::client(&db, &client.client_id.to_string())
                .expect("current")
                .grants
                .is_empty()
        );
        let changed = command(
            &mut db,
            PrincipalId::local_owner(),
            "explicit-ability",
            W::ChangeGrants {
                id: client.client_id,
                expected: 2,
                add: vec![global("state.check")],
                remove: vec![],
            },
        )
        .expect("one explicit capability");
        assert_eq!(changed.revision, 3);
        assert_eq!(changed.grants, vec![global("state.check")]);
        assert_eq!(
            page(&["changed".to_owned()], old.clone(), 128, 128, &binding)
                .expect_err("actual data changed")
                .code,
            E::RevisionConflict
        );
        for changed in [
            serde_json::json!([
                "grants",
                client.client_id,
                2,
                "current-lease-and-two-sided-scope"
            ]),
            serde_json::json!([
                "grants",
                client.client_id,
                1,
                "revoked-or-different-principal"
            ]),
        ] {
            assert_eq!(
                page(&items, old.clone(), 128, 128, &changed)
                    .expect_err("must restart")
                    .code,
                E::RevisionConflict
            );
        }
        assert_eq!(
            page(&items, None, 129, 128, &binding)
                .expect_err("limit")
                .code,
            E::InvalidArgument
        );
    }
}
