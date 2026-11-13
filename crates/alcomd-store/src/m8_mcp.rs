use crate::StateStoreHandle;
use alcomd_application::McpClientId;
use alcomd_application::{
    McpClient, McpGrant, McpIdentityError as E, McpIdentityStore, McpIdentityWrite as W,
    McpIdentityWriteResult as R, PrincipalId, mcp_credential_hash,
};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use uuid::Uuid;

fn unavailable<T>(_: T) -> E {
    E::Unavailable
}
fn client(db: &Connection, id: &str) -> Result<McpClient, E> {
    let mut record=db.query_row("SELECT client_id,principal_id,display_name,revision,created_at_ms,revoked_at_ms FROM mcp_clients WHERE client_id=?1",[id],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,i64>(3)?,r.get::<_,i64>(4)?,r.get::<_,Option<i64>>(5)?))).optional().map_err(unavailable)?.ok_or(E::PermissionDenied)?;
    let revision = db
        .query_row(
            "SELECT grant_revision FROM mcp_clients WHERE client_id=?1",
            [id],
            |r| r.get::<_, i64>(0),
        )
        .map_err(unavailable)?;
    let grants = grant_snapshot(db, id, revision)?;
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
        grants,
    })
}
fn grant_snapshot(db: &Connection, id: &str, revision: i64) -> Result<Vec<McpGrant>, E> {
    let mut q=db.prepare("SELECT permission,resource_kind,resource_id FROM mcp_client_grants WHERE client_id=?1 AND grant_revision=?2 ORDER BY permission,resource_kind,resource_id").map_err(unavailable)?;
    q.query_map(params![id, revision], |r| {
        Ok(McpGrant {
            permission: r.get(0)?,
            resource_kind: r.get(1)?,
            resource_id: r.get(2)?,
        })
    })
    .map_err(unavailable)?
    .collect::<Result<Vec<_>, _>>()
    .map_err(unavailable)
}
fn grants(db: &Connection, id: &str, revision: i64, items: Vec<McpGrant>) -> Result<(), E> {
    for g in items {
        if g.resource_kind == "project" {
            let exists: bool = db
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM projects WHERE project_id=?1)",
                    [&g.resource_id],
                    |r| r.get(0),
                )
                .map_err(unavailable)?;
            if !exists {
                return Err(E::InvalidInput);
            }
        }
        db.execute("INSERT INTO mcp_client_grants(client_id,grant_revision,permission,resource_kind,resource_id) VALUES (?1,?2,?3,?4,?5)",params![id,revision,g.permission,g.resource_kind,g.resource_id]).map_err(unavailable)?;
    }
    db.execute(
        "UPDATE mcp_clients SET grant_revision=?2 WHERE client_id=?1",
        params![id, revision],
    )
    .map_err(unavailable)?;
    Ok(())
}
fn write(
    db: &mut Connection,
    actor: PrincipalId,
    key: String,
    command: W,
    now: u64,
) -> Result<R, E> {
    let now = i64::try_from(now).map_err(|_| E::InvalidInput)?;
    let (method, input) = match &command {
        W::Create { name, grants } => ("mcp.clients.create", serde_json::json!([name, grants])),
        W::SetGrants {
            id,
            expected,
            grants,
        } => (
            "mcp.clients.setGrants",
            serde_json::json!([id, expected, grants]),
        ),
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
    let tx = db
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(unavailable)?;
    let replay=tx.query_row("SELECT request_fingerprint,response_json FROM idempotency_records WHERE principal_id=?1 AND method=?2 AND idempotency_key=?3",params![actor.as_str(),method,key],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional().map_err(unavailable)?;
    if let Some((old, response)) = replay {
        if old != fingerprint {
            return Err(E::IdempotencyConflict);
        }
        if matches!(command, W::Issue { .. }) {
            return Err(E::CredentialResponseUnavailable);
        }
        let mut saved: serde_json::Value = serde_json::from_str(&response).map_err(unavailable)?;
        let client_id = saved["client"]["clientId"].as_str().ok_or(E::Unavailable)?;
        let revision = saved["grantRevision"].as_i64().ok_or(E::Unavailable)?;
        let snapshot = grant_snapshot(&tx, client_id, revision)?;
        saved["client"]["grants"] = serde_json::to_value(snapshot).map_err(unavailable)?;
        return Ok(R {
            client: serde_json::from_value(saved["client"].clone()).map_err(unavailable)?,
            credential_id: None,
        });
    }
    let mut credential_id = None;
    let id = match command {
        W::Create {
            name,
            grants: items,
        } => {
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
            grants(&tx, &id, 1, items)?;
            id
        }
        command => {
            let (id, expected) = match &command {
                W::SetGrants { id, expected, .. }
                | W::Issue { id, expected, .. }
                | W::Revoke { id, expected } => (id.to_string(), *expected),
                W::Create { .. } => return Err(E::InvalidInput),
            };
            let current = client(&tx, &id)?;
            if current.revoked_at_ms.is_some()
                || current.revision != expected
                || expected >= i64::MAX as u64
            {
                return Err(E::RevisionConflict);
            }
            match command {
                W::SetGrants { grants: items, .. } => grants(
                    &tx,
                    &id,
                    i64::try_from(expected + 1).map_err(unavailable)?,
                    items,
                )?,
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
        client: client(&tx, &id)?,
        credential_id,
    };
    let grant_revision = tx
        .query_row(
            "SELECT grant_revision FROM mcp_clients WHERE client_id=?1",
            [&id],
            |r| r.get::<_, i64>(0),
        )
        .map_err(unavailable)?;
    let mut metadata = serde_json::to_value(&result.client).map_err(unavailable)?;
    metadata
        .as_object_mut()
        .ok_or(E::Unavailable)?
        .remove("grants");
    let response=serde_json::json!({"client":metadata,"grantRevision":grant_revision,"credentialId":result.credential_id}).to_string();
    // Record only the request digest and non-secret result, including for issuance.
    tx.execute("INSERT INTO idempotency_records(principal_id,method,idempotency_key,request_fingerprint,state,response_json,created_at_ms) VALUES (?1,?2,?3,?4,'completed',?5,?6)",params![actor.as_str(),method,key,fingerprint,response,now]).map_err(unavailable)?;
    tx.commit().map_err(unavailable)?;
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
    // Authorization is a join against current grants, never an owner substitution.
    let owner=db.query_row("SELECT p.owner_principal_id FROM projects p JOIN mcp_client_grants g ON g.resource_id=p.project_id AND g.resource_kind='project' AND g.permission='projects.read' JOIN mcp_clients c ON c.client_id=g.client_id AND g.grant_revision=c.grant_revision WHERE c.principal_id=?1 AND c.revoked_at_ms IS NULL AND p.project_id=?2",params![principal.as_str(),id.to_string()],|r|r.get::<_,String>(0)).optional().map_err(|_|M3Error::new(M3ErrorCode::StoreUnavailable))?.ok_or_else(||M3Error::new(M3ErrorCode::PermissionDenied))?;
    let owner = PrincipalId::parse(owner).map_err(|_| M3Error::new(M3ErrorCode::Internal))?;
    crate::m3::get_project(db, &owner, id)
}
pub(super) fn projects(
    db: &Connection,
    principal: &PrincipalId,
    cursor: Option<RegistryCursor<ProjectId>>,
    limit: u32,
) -> Result<ProjectPage, M3Error> {
    let mut q=db.prepare("SELECT p.project_id FROM projects p WHERE EXISTS (SELECT 1 FROM mcp_clients c JOIN mcp_client_grants g USING(client_id) WHERE c.principal_id=?1 AND c.revoked_at_ms IS NULL AND g.grant_revision=c.grant_revision AND g.permission='projects.read' AND g.resource_kind='project' AND g.resource_id=p.project_id) AND (?2 IS NULL OR p.registered_at_ms<?2 OR (p.registered_at_ms=?2 AND p.project_id<?3)) ORDER BY p.registered_at_ms DESC,p.project_id DESC LIMIT ?4").map_err(|_|M3Error::new(M3ErrorCode::StoreUnavailable))?;
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
                i64::from(limit) + 1
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
    let next_cursor = if more {
        records.last().map(|r| RegistryCursor {
            registered_at_ms: r.registered_at_ms,
            id: r.project_id,
        })
    } else {
        None
    };
    Ok(ProjectPage {
        projects: records,
        next_cursor,
        directory_states: Default::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alcomd_application::{IdempotencyKey, ManifestState, ProjectObservation, ProjectType};

    #[test]
    fn accepted_removal_preserves_resource_owner_after_revocation_and_cannot_replay_without_grants()
    {
        use alcomd_application::{
            PackageApplyCompletion, PackageChangeSet, PackagePlanDraft, PlanAction, Revision,
        };
        let mut db = crate::sqlite::mcp_state32_fixture();
        db.execute_batch(include_str!("../migrations/0033_mcp_clients.sql"))
            .expect("State33");
        db.execute_batch(include_str!("../migrations/0034_mcp_grant_snapshots.sql"))
            .expect("State34");
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
        let mut grants = vec![];
        for permission in ["projects.read", "packages.manage"] {
            grants.push(McpGrant {
                permission: permission.into(),
                resource_kind: "project".into(),
                resource_id: project.project_id.to_string(),
            });
        }
        for permission in ["repositories.read", "packages.read", "operations.read"] {
            grants.push(McpGrant {
                permission: permission.into(),
                resource_kind: "global".into(),
                resource_id: "*".into(),
            });
        }
        let client = write(
            &mut db,
            owner.clone(),
            "create".into(),
            W::Create {
                name: "Scoped writer".into(),
                grants,
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
        .expect("same grant replay");
        assert_eq!(accepted.operation_id, replay.operation_id);
        assert!(replay.replayed);
        let mut nonremoval = draft();
        nonremoval.action = PlanAction::Install;
        let install = crate::m4::create_package_plan(&mut db, &client.principal_id, nonremoval, 5)
            .expect("frozen other action fixture");
        assert!(
            matches!(crate::m4::accept_package_plan(&mut db,&client.principal_id,install.plan_id,project.revision,&IdempotencyKey::parse("no-install").expect("key"),6),Err(e) if e.code()==alcomd_application::M4ErrorCode::PermissionDenied)
        );
        write(
            &mut db,
            owner.clone(),
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
                .expect("accepted target, independent of revoked grants")
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

    #[test]
    fn large_grant_results_replay_exactly_without_reauthorizing_historical_scopes() {
        let mut db = crate::sqlite::mcp_state32_fixture();
        db.execute_batch(include_str!("../migrations/0033_mcp_clients.sql"))
            .expect("State33");
        db.execute_batch(include_str!("../migrations/0034_mcp_grant_snapshots.sql"))
            .expect("State34");
        let owner = PrincipalId::local_owner();
        let mut items = Vec::new();
        for n in 0_u32..1024 {
            let observation = ProjectObservation {
                root_path: format!("C:/owned-fixture/{n}"),
                path_identity_key: n.to_le_bytes().repeat(6),
                project_type: ProjectType::Avatars,
                unity_version: "2022.3.22f1".into(),
                unity_revision: None,
                vpm_manifest: ManifestState::Valid,
                upm_manifest: ManifestState::Missing,
                direct_dependencies: vec![],
                locked_dependencies: vec![],
                issues: vec![],
            };
            let project = crate::m3::register_project(
                &mut db,
                &owner,
                observation,
                &IdempotencyKey::parse(format!("seed-{n}")).expect("key"),
                1,
            )
            .expect("isolated registry fixture")
            .value;
            items.push(McpGrant {
                permission: "projects.read".into(),
                resource_kind: "project".into(),
                resource_id: project.project_id.to_string(),
            });
        }
        let original = write(
            &mut db,
            owner.clone(),
            "create".into(),
            W::Create {
                name: "Large ACL".into(),
                grants: items.clone(),
            },
            2,
        )
        .expect("large client")
        .client;
        assert!(serde_json::to_vec(&original).expect("full result").len() > 65536);
        let response_len:i64=db.query_row("SELECT length(response_json) FROM idempotency_records WHERE method='mcp.clients.create'",[],|r|r.get(0)).expect("compact record");
        assert!(response_len < 65536);
        write(
            &mut db,
            owner.clone(),
            "empty".into(),
            W::SetGrants {
                id: original.client_id,
                expected: original.revision,
                grants: vec![],
            },
            3,
        )
        .expect("remove all current grants");
        let replay = write(
            &mut db,
            owner,
            "create".into(),
            W::Create {
                name: "Large ACL".into(),
                grants: items.clone(),
            },
            4,
        )
        .expect("original exact response")
        .client;
        assert_eq!(replay, original);
        let current = client(&db, &original.client_id.to_string()).expect("current authority");
        assert!(current.grants.is_empty());
        assert!(
            alcomd_application::mcp_access(&current)
                .expect("current identity")
                .require(alcomd_application::Permission::ProjectsRead)
                .is_err()
        );
        let denied = project(
            &db,
            &current.principal_id,
            ProjectId::parse(&items[0].resource_id).expect("project ID"),
        );
        assert!(matches!(denied,Err(ref error) if error.code()==M3ErrorCode::PermissionDenied));
        let retained: i64 = db
            .query_row("SELECT count(*) FROM mcp_client_grants", [], |r| r.get(0))
            .expect("history retained");
        assert_eq!(retained, 1024);
    }
}
