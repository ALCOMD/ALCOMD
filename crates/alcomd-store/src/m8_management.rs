use crate::StateStoreHandle;
use alcomd_application::{
    ExtensionDesiredState, ExtensionInstanceLease, ExtensionInvocationKind,
    ExtensionQuarantineState, ExtensionUiInvocationAuthority, MCP_MANAGEMENT_BYTES,
    ManagementClientPage, ManagementClientSummary, ManagementError, ManagementErrorCode as E,
    ManagementGrantPage, ManagementPage, ManagementProject, ManagementRequest as Q,
    ManagementResponse as R, McpIdentityError, McpIdentityWrite, McpManagementStore, Permission,
    management_client_fingerprint,
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
            let mut query = tx.prepare("SELECT p.project_id FROM projects p JOIN extension_grants g ON g.resource_id=p.project_id WHERE g.extension_id=?1 AND g.permission_name='projects.read' AND g.resource_kind='Project' AND g.state='granted' ORDER BY p.project_id").map_err(unavailable)?;
            let ids = query
                .query_map([&lease.extension_id], |r| r.get::<_, String>(0))
                .map_err(unavailable)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(unavailable)?;
            let mut items = Vec::new();
            for id in ids {
                if authority
                    .client_access
                    .require_project_read_scope(&id)
                    .is_ok()
                {
                    let summary = crate::m6::project_summary(&tx, lease, &id, now)
                        .map_err(|_| ManagementError::from(E::ScopeDenied))?;
                    items.push(ManagementProject {
                        project_id: id,
                        name: summary.display_name,
                    });
                }
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
            for grant in &add {
                if grant.resource_kind == "project" {
                    authority
                        .client_access
                        .require_project_read_scope(&grant.resource_id)
                        .map_err(|_| ManagementError::from(E::ScopeDenied))?;
                    crate::m6::project_summary(&tx, lease, &grant.resource_id, now)
                        .map_err(|_| ManagementError::from(E::ScopeDenied))?;
                }
            }
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
    if now >= lease.expires_at_ms {
        return Err(E::LeaseExpired.into());
    }
    if !authority.is_current() || now >= authority.deadline_ms {
        return Err(E::Cancelled.into());
    }
    if !matches!(
        authority.kind,
        ExtensionInvocationKind::InteractiveUiRender | ExtensionInvocationKind::InteractiveUiAction
    ) || (request.is_write() && authority.kind != ExtensionInvocationKind::InteractiveUiAction)
    {
        return Err(E::PermissionDenied.into());
    }
    authority
        .client_access
        .require_extension_ui_scope(&lease.extension_id)
        .map_err(|_| ManagementError::from(E::ScopeDenied))?;
    let permission = match request {
        Q::Projects { .. } => Permission::ProjectsRead,
        r if r.is_write() => Permission::McpClientsManage,
        _ => Permission::McpClientsRead,
    };
    authority
        .client_access
        .require(permission)
        .map_err(|_| ManagementError::from(E::PermissionDenied))?;
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
    if permission != Permission::ProjectsRead && !management_granted(db, lease, permission)? {
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
    fn delta_can_remove_orphan_grants_and_preserve_other_orphans() {
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
        let result = command(
            &mut db,
            owner,
            "remove",
            W::ChangeGrants {
                id: client.client_id,
                expected: 1,
                add: vec![],
                remove: vec![removed],
            },
        )
        .expect("orphan removal");
        assert_eq!(result.revision, 2);
        assert_eq!(result.grants.len(), 1);
        assert_eq!(result.grants[0].resource_id, ids[1]);
    }
    #[test]
    fn all_4096_grants_page_completely_with_revision_and_authority_binding() {
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
        let items = crate::m8_mcp::grant_snapshot(&db, &client.client_id.to_string(), 1)
            .expect("actual full snapshot");
        assert_eq!(items.len(), 4096);
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
            .expect_err("final cap remains enforced"),
            McpIdentityError::LimitExceeded
        );
        let changed = command(
            &mut db,
            PrincipalId::local_owner(),
            "replace-one",
            W::ChangeGrants {
                id: client.client_id,
                expected: 1,
                add: vec![global("state.check")],
                remove: vec![items[0].clone()],
            },
        )
        .expect("one atomic replacement at capacity");
        assert_eq!(changed.revision, 2);
        assert_eq!(changed.grants.len(), 4096);
        assert_eq!(
            page(&changed.grants, old.clone(), 128, 128, &binding)
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
