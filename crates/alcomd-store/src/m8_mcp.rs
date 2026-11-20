use crate::StateStoreHandle;
use alcomd_application::McpClientId;
use alcomd_application::{
    McpClient, McpIdentityError as E, McpIdentityStore, McpIdentityWrite as W,
    McpIdentityWriteResult as R, PrincipalId, mcp_credential_hash,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use uuid::Uuid;

fn unavailable<T>(_: T) -> E {
    E::Unavailable
}
pub(super) fn client(db: &Connection, id: &str) -> Result<McpClient, E> {
    let mut record=db.query_row("SELECT client_id,principal_id,display_name,revision,created_at_ms,revoked_at_ms FROM mcp_clients WHERE client_id=?1",[id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,i64>(3)?,r.get::<_,i64>(4)?,r.get::<_,Option<i64>>(5)?))).optional().map_err(unavailable)?.ok_or(E::PermissionDenied)?;
    Ok(McpClient {
        client_id: McpClientId::parse(&record.0).map_err(unavailable)?,
        principal_id: PrincipalId::parse(std::mem::take(&mut record.1)).map_err(unavailable)?,
        display_name: record.2,
        revision: u64::try_from(record.3).map_err(unavailable)?,
        created_at_ms: u64::try_from(record.4).map_err(unavailable)?,
        revoked_at_ms: record
            .5
            .map(u64::try_from)
            .transpose()
            .map_err(unavailable)?,
    })
}
pub(super) struct AdministrativeActor {
    pub(super) principal: PrincipalId,
    authentication: Option<alcomd_application::ToolAuthentication>,
}

impl From<PrincipalId> for AdministrativeActor {
    fn from(principal: PrincipalId) -> Self {
        Self {
            principal,
            authentication: alcomd_application::workflow_authentication(),
        }
    }
}

impl AdministrativeActor {
    pub(super) fn verify(&self, db: &Connection) -> Result<(), E> {
        if let Some(authentication) = &self.authentication {
            let client = super::m8_tools::authenticate(db, authentication)
                .map_err(|_| E::PermissionDenied)?;
            if client.principal_id == self.principal {
                return Ok(());
            }
        } else if self.principal == PrincipalId::local_owner() {
            return Ok(());
        }
        Err(E::PermissionDenied)
    }
}

fn write(
    db: &mut Connection,
    actor: AdministrativeActor,
    key: String,
    command: W,
    now: u64,
) -> Result<R, E> {
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unavailable)?;
    actor.verify(&tx)?;
    let result = write_on(&tx, actor.principal, key, command, now)?;
    tx.commit().map_err(unavailable)?;
    Ok(result)
}

pub(super) fn write_on(
    tx: &rusqlite::Transaction<'_>,
    actor: PrincipalId,
    key: String,
    command: W,
    now: u64,
) -> Result<R, E> {
    let now = i64::try_from(now).map_err(|_| E::InvalidInput)?;
    let (method, input) = match &command {
        W::Create { name } => ("mcp.clients.create", serde_json::json!([name])),
        W::Issue { id, expected, .. } => (
            "mcp.clients.issueCredential",
            serde_json::json!([id, expected]),
        ),
        W::Revoke { id, expected } => ("mcp.clients.revoke", serde_json::json!([id, expected])),
    };
    let digest = mcp_credential_hash(&input.to_string());
    let fingerprint = serde_json::to_string(
        &digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
    )
    .map_err(unavailable)?;
    let replay=tx.query_row("SELECT request_fingerprint,response_json FROM idempotency_records WHERE principal_id=?1 AND method=?2 AND idempotency_key=?3",params![actor.as_str(),method,key],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional().map_err(unavailable)?;
    if let Some((old, response)) = replay {
        if old != fingerprint {
            return Err(E::IdempotencyConflict);
        }
        if matches!(command, W::Issue { .. }) {
            return Err(E::CredentialResponseUnavailable);
        }
        return serde_json::from_str(&response).map_err(unavailable);
    }
    let mut credential_id = None;
    let id = match command {
        W::Create { name } => {
            let count: u32 = tx
                .query_row("SELECT count(*) FROM mcp_clients", [], |r| r.get(0))
                .map_err(unavailable)?;
            if count >= 64 {
                return Err(E::LimitExceeded);
            }
            let id = McpClientId::new().to_string();
            tx.execute(
                "INSERT INTO mcp_clients(client_id,principal_id,display_name,revision,created_at_ms,revoked_at_ms) VALUES (?1,?2,?3,1,?4,NULL)",
                params![id, format!("mcp:{id}"), name, now],
            )
            .map_err(unavailable)?;
            id
        }
        command => {
            let (id, expected) = match &command {
                W::Issue { id, expected, .. } | W::Revoke { id, expected } => {
                    (id.to_string(), *expected)
                }
                W::Create { .. } => return Err(E::InvalidInput),
            };
            let current = client(tx, &id)?;
            if current.revoked_at_ms.is_some()
                || current.revision != expected
                || expected >= i64::MAX as u64
            {
                return Err(E::RevisionConflict);
            }
            match command {
                W::Issue { hash, .. } => {
                    tx.execute("UPDATE mcp_client_credentials SET revoked_at_ms=max(created_at_ms,?2) WHERE client_id=?1 AND revoked_at_ms IS NULL",params![id,now]).map_err(unavailable)?;
                    let cid = Uuid::new_v4().to_string();
                    tx.execute(
                        "INSERT INTO mcp_client_credentials VALUES (?1,?2,?3,?4,NULL)",
                        params![cid, id, hash.as_slice(), now],
                    )
                    .map_err(unavailable)?;
                    credential_id = Some(cid);
                }
                W::Revoke { .. } => {
                    tx.execute("UPDATE mcp_clients SET revoked_at_ms=max(created_at_ms,?2) WHERE client_id=?1",params![id,now]).map_err(unavailable)?;
                    tx.execute("UPDATE mcp_client_credentials SET revoked_at_ms=max(created_at_ms,?2) WHERE client_id=?1 AND revoked_at_ms IS NULL",params![id,now]).map_err(unavailable)?;
                }
                W::Create { .. } => return Err(E::InvalidInput),
            }
            tx.execute(
                "UPDATE mcp_clients SET revision=revision+1 WHERE client_id=?1",
                [&id],
            )
            .map_err(unavailable)?;
            id
        }
    };
    let result = R {
        client: client(tx, &id)?,
        credential_id,
    };
    let response = serde_json::to_string(&result).map_err(unavailable)?;
    // Record only the request digest and non-secret result, including for issuance.
    tx.execute("INSERT INTO idempotency_records(principal_id,method,idempotency_key,request_fingerprint,state,response_json,created_at_ms) VALUES (?1,?2,?3,?4,'completed',?5,?6)",params![actor.as_str(),method,key,fingerprint,response,now]).map_err(unavailable)?;
    Ok(result)
}

impl McpIdentityStore for StateStoreHandle {
    async fn mcp_clients(&self) -> Result<Vec<McpClient>, E> {
        self.request_worker(
            |db| {
                let mut q = db
                    .prepare("SELECT client_id FROM mcp_clients ORDER BY client_id")
                    .map_err(unavailable)?;
                let ids = q
                    .query_map([], |r| r.get::<_, String>(0))
                    .map_err(unavailable)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(unavailable)?;
                ids.iter().map(|id| client(db, id)).collect()
            },
            || E::Unavailable,
        )
        .await
    }
    async fn mcp_authenticate(&self, hash: [u8; 32]) -> Result<McpClient, E> {
        self.request_worker(move|db|{
        let id=db.query_row("SELECT c.client_id FROM mcp_clients c JOIN mcp_client_credentials k USING(client_id) WHERE k.token_hash=?1 AND k.revoked_at_ms IS NULL AND c.revoked_at_ms IS NULL",[hash.as_slice()],|r|r.get::<_,String>(0)).optional().map_err(unavailable)?.ok_or(E::PermissionDenied)?;
        client(db,&id)
    },||E::Unavailable).await
    }
    async fn mcp_write(
        &self,
        actor: PrincipalId,
        key: String,
        command: W,
        now: u64,
    ) -> Result<R, E> {
        let actor = AdministrativeActor::from(actor);
        self.request_worker(
            move |db| write(db, actor, key, command, now),
            || E::Unavailable,
        )
        .await
    }
}

use alcomd_application::{
    M3Error, M3ErrorCode, ProjectId, ProjectPage, ProjectRecord, RegistryCursor,
};
pub(super) fn project(
    db: &Connection,
    principal: &PrincipalId,
    id: ProjectId,
) -> Result<ProjectRecord, M3Error> {
    // Read the resource using its recorded owner after checking the MCP identity.
    let owner=db.query_row("SELECT p.owner_principal_id FROM projects p JOIN mcp_clients c ON c.principal_id=?1 WHERE c.principal_id=?1 AND c.revoked_at_ms IS NULL AND p.project_id=?2",params![principal.as_str(),id.to_string()],|r|r.get::<_,String>(0)).optional().map_err(|_|M3Error::new(M3ErrorCode::StoreUnavailable))?.ok_or_else(||M3Error::new(M3ErrorCode::PermissionDenied))?;
    let owner = PrincipalId::parse(owner).map_err(|_| M3Error::new(M3ErrorCode::Internal))?;
    crate::m3::get_project(db, &owner, id)
}
pub(super) fn projects(
    db: &Connection,
    principal: &PrincipalId,
    cursor: Option<RegistryCursor<ProjectId>>,
    limit: u32,
) -> Result<ProjectPage, M3Error> {
    authorized_projects(db, principal, cursor, limit, None)
}
pub(super) fn authorized_projects(
    db: &Connection,
    principal: &PrincipalId,
    cursor: Option<RegistryCursor<ProjectId>>,
    limit: u32,
    guard: Option<&alcomd_application::OAuthAccessGuard>,
) -> Result<ProjectPage, M3Error> {
    if guard.is_some() {
        // Scope, snapshot binding and page contents must see the same read transaction.
        let tx = db
            .unchecked_transaction()
            .map_err(|_| M3Error::new(M3ErrorCode::StoreUnavailable))?;
        let page = authorized_projects_on(&tx, principal, cursor, limit, guard)?;
        tx.commit()
            .map_err(|_| M3Error::new(M3ErrorCode::StoreUnavailable))?;
        Ok(page)
    } else {
        authorized_projects_on(db, principal, cursor, limit, None)
    }
}
fn authorized_projects_on(
    db: &Connection,
    principal: &PrincipalId,
    cursor: Option<RegistryCursor<ProjectId>>,
    limit: u32,
    guard: Option<&alcomd_application::OAuthAccessGuard>,
) -> Result<ProjectPage, M3Error> {
    validate_project_cursor(db, principal, cursor.as_ref(), guard)?;
    let mut q=db.prepare("SELECT p.project_id FROM projects p WHERE EXISTS (SELECT 1 FROM mcp_clients c WHERE c.principal_id=?1 AND c.revoked_at_ms IS NULL) AND (?2 IS NULL OR p.registered_at_ms<?2 OR (p.registered_at_ms=?2 AND p.project_id<?3)) ORDER BY p.registered_at_ms DESC,p.project_id DESC LIMIT ?4").map_err(|_|M3Error::new(M3ErrorCode::StoreUnavailable))?;
    let cursor_time = cursor
        .as_ref()
        .map(|c| i64::try_from(c.registered_at_ms))
        .transpose()
        .map_err(|_| M3Error::new(M3ErrorCode::InvalidInput))?;
    let ids = q
        .query_map(
            params![
                principal.as_str(),
                cursor_time,
                cursor.as_ref().map(|c| c.id.to_string()),
                i64::from(limit) + 1,
            ],
            |r| r.get::<_, String>(0),
        )
        .map_err(|_| M3Error::new(M3ErrorCode::StoreUnavailable))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| M3Error::new(M3ErrorCode::StoreUnavailable))?;
    let more = ids.len() > limit as usize;
    let mut records = Vec::new();
    for id in ids.into_iter().take(limit as usize) {
        records.push(project(
            db,
            principal,
            ProjectId::parse(&id).map_err(|_| M3Error::new(M3ErrorCode::Internal))?,
        )?);
    }
    let mut next_cursor = if more {
        records.last().map(|r| RegistryCursor {
            registered_at_ms: r.registered_at_ms,
            id: r.project_id,
            authorization_binding: None,
        })
    } else {
        None
    };
    bind_project_cursor(db, guard, next_cursor.as_mut())?;
    Ok(ProjectPage {
        projects: records,
        next_cursor,
        directory_states: Default::default(),
    })
}

pub(super) fn validate_project_cursor(
    db: &Connection,
    principal: &PrincipalId,
    cursor: Option<&RegistryCursor<ProjectId>>,
    guard: Option<&alcomd_application::OAuthAccessGuard>,
) -> Result<(), M3Error> {
    if let Some(guard) = guard {
        let now = super::m8_oauth::wall_time()
            .map_err(|_| M3Error::new(M3ErrorCode::StoreUnavailable))?;
        let limited = super::m8_oauth::authenticate(db, guard, now)
            .map_err(|_| M3Error::new(M3ErrorCode::PermissionDenied))?;
        if &limited.principal_id != principal {
            return Err(M3Error::new(M3ErrorCode::PermissionDenied));
        }
        if let Some(c) = cursor {
            let expected = super::m8_oauth::project_cursor_binding(db, guard, now, c)
                .map_err(|_| M3Error::new(M3ErrorCode::RevisionConflict))?;
            let matches = c.authorization_binding.as_deref().is_some_and(|actual| {
                actual.len() == expected.len()
                    && actual
                        .bytes()
                        .zip(expected.bytes())
                        .fold(0u8, |difference, (a, b)| difference | (a ^ b))
                        == 0
            });
            if !matches {
                return Err(M3Error::new(M3ErrorCode::RevisionConflict));
            }
        }
    }
    Ok(())
}

pub(super) fn bind_project_cursor(
    db: &Connection,
    guard: Option<&alcomd_application::OAuthAccessGuard>,
    next_cursor: Option<&mut RegistryCursor<ProjectId>>,
) -> Result<(), M3Error> {
    if let (Some(guard), Some(next)) = (guard, next_cursor) {
        let now = super::m8_oauth::wall_time()
            .map_err(|_| M3Error::new(M3ErrorCode::StoreUnavailable))?;
        next.authorization_binding = Some(
            super::m8_oauth::project_cursor_binding(db, guard, now, next)
                .map_err(|_| M3Error::new(M3ErrorCode::RevisionConflict))?,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alcomd_application::{IdempotencyKey, ManifestState, ProjectObservation, ProjectType};

    #[test]
    fn revocation_blocks_replay_and_preserves_the_accepted_removal_and_existing_owner() {
        use alcomd_application::{
            PackageApplyCompletion, PackageChangeSet, PackagePlanDraft, PlanAction, Revision,
        };
        let mut db = crate::sqlite::current_schema_fixture();
        let owner = PrincipalId::local_owner();
        let observation = ProjectObservation {
            root_path: "C:/owned-registry-fixture".into(),
            path_identity_key: vec![7; 24],
            project_type: ProjectType::Avatars,
            unity_version: "2022.3.22f1".into(),
            unity_revision: None,
            vpm_manifest: ManifestState::Valid,
            upm_manifest: ManifestState::Valid,
            direct_dependencies: vec![],
            locked_dependencies: vec![],
            issues: vec![],
        };
        let project = crate::m3::register_project(
            &mut db,
            &owner,
            observation.clone(),
            &IdempotencyKey::parse("seed").expect("key"),
            1,
        )
        .expect("register without filesystem access")
        .value;
        let client = write(
            &mut db,
            owner.clone().into(),
            "create".into(),
            W::Create {
                name: "Scoped writer".into(),
            },
            2,
        )
        .expect("client")
        .client;
        let draft = || PackagePlanDraft {
            project_id: project.project_id,
            action: PlanAction::Remove,
            project_revision: project.revision,
            project_snapshot_fingerprint: [1; 32],
            change_set_fingerprint: [2; 32],
            source_set: vec![],
            change_set: PackageChangeSet {
                format_version: 1,
                mutations: vec![],
                dependency_edges: vec![],
                removal_conflicts: vec![],
                absent_removal_packages: vec![],
                legacy_reinstall_packages: vec![],
                legacy_removals: vec![],
                legacy_packages: vec![],
                vpm_manifest_sha256: [3; 32],
                unity_compatibility_overrides: vec![],
            },
        };
        let plan = crate::m4::create_package_plan(&mut db, &client.principal_id, draft(), 3)
            .expect("actual caller plan");
        let key = IdempotencyKey::parse("apply").expect("key");
        let accepted = crate::m4::accept_package_plan(
            &mut db,
            &client.principal_id,
            plan.plan_id,
            project.revision,
            &key,
            4,
        )
        .expect("authorized acceptance");
        let replay = crate::m4::accept_package_plan(
            &mut db,
            &client.principal_id,
            plan.plan_id,
            project.revision,
            &key,
            5,
        )
        .expect("same-key replay");
        assert_eq!(accepted.operation_id, replay.operation_id);
        assert!(replay.replayed);
        let mut nonremoval = draft();
        nonremoval.action = PlanAction::Install;
        let install = crate::m4::create_package_plan(&mut db, &client.principal_id, nonremoval, 5);
        assert!(
            matches!(install,Err(e) if e.code()==alcomd_application::M4ErrorCode::PermissionDenied)
        );
        write(
            &mut db,
            owner.clone().into(),
            "revoke".into(),
            W::Revoke {
                id: client.client_id,
                expected: client.revision,
            },
            6,
        )
        .expect("revoke before task runs");
        assert!(
            matches!(crate::m4::accept_package_plan(&mut db,&client.principal_id,plan.plan_id,project.revision,&key,7),Err(e) if e.code()==alcomd_application::M4ErrorCode::PermissionDenied)
        );
        assert_eq!(
            crate::m4::operation_project(&db, accepted.operation_id)
                .expect("accepted target, independent of revoked client")
                .project_id,
            project.project_id
        );
        crate::m4::begin_package_apply(&mut db, accepted.operation_id, 8)
            .expect("run accepted task");
        crate::m4::complete_package_apply(
            &mut db,
            accepted.operation_id,
            PackageApplyCompletion {
                project_observation: observation,
                result_json: "{}".into(),
            },
            9,
        )
        .expect("accepted target commits without owner rewrite");
        let saved = crate::m3::get_project(&db, &owner, project.project_id)
            .expect("original owner still reads");
        assert_eq!(
            saved.revision,
            Revision::new(project.revision.get() + 1).expect("revision")
        );
        let resource_owner: String = db
            .query_row(
                "SELECT owner_principal_id FROM projects WHERE project_id=?1",
                [project.project_id.to_string()],
                |r| r.get(0),
            )
            .expect("owner");
        assert_eq!(resource_owner, owner.as_str());
        let task_owner: String = db
            .query_row(
                "SELECT owner_principal_id FROM operations WHERE operation_id=?1",
                [accepted.operation_id.to_string()],
                |r| r.get(0),
            )
            .expect("task owner");
        assert_eq!(task_owner, client.principal_id.as_str());
        let event_owner: String = db
            .query_row(
                "SELECT principal_id FROM events WHERE kind='project.packages_changed'",
                [],
                |r| r.get(0),
            )
            .expect("resource event audience");
        assert_eq!(event_owner, owner.as_str());
        assert!(db.execute("UPDATE package_filesystem_journal SET project_identity_key=x'ff' WHERE operation_id=?1 AND step=1",[accepted.operation_id.to_string()]).is_err(),"accepted journal evidence remains immutable");
        db.execute(
            "UPDATE projects SET path_identity_key=x'ff' WHERE project_id=?1",
            [project.project_id.to_string()],
        )
        .expect("model a changed registered target identity");
        assert!(crate::m4::operation_project(&db, accepted.operation_id).is_err());
    }
}
