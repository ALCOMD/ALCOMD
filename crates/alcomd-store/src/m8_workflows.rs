use alcomd_application::{
    Permission, PlanId, PrincipalId, ToolAuthentication, ToolError, WorkflowPlanPath,
};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use std::cell::RefCell;

thread_local! {
    static AUTHENTICATION: RefCell<Option<ToolAuthentication>> = const { RefCell::new(None) };
    static PATH: RefCell<Option<WorkflowPlanPath>> = const { RefCell::new(None) };
}

pub(super) struct ContextGuard(Option<ToolAuthentication>, Option<WorkflowPlanPath>);

impl ContextGuard {
    pub(super) fn install(
        authentication: Option<ToolAuthentication>,
        path: Option<WorkflowPlanPath>,
    ) -> Self {
        Self(
            AUTHENTICATION.with(|slot| slot.replace(authentication)),
            PATH.with(|slot| slot.replace(path)),
        )
    }
}

impl Drop for ContextGuard {
    fn drop(&mut self) {
        AUTHENTICATION.with(|slot| slot.replace(self.0.take()));
        PATH.with(|slot| slot.replace(self.1.take()));
    }
}

pub(super) fn install_plan_proof(
    tx: &Transaction<'_>,
    caller: &PrincipalId,
    plan_id: PlanId,
    now: u64,
) -> Result<(), ToolError> {
    if !caller.as_str().starts_with("mcp:") {
        return Ok(());
    }
    let proof = PATH
        .with(|slot| slot.borrow().clone())
        .ok_or(ToolError::PermissionDenied)?;
    let json = serde_json::to_string(&proof).map_err(|_| ToolError::Unavailable)?;
    tx.execute(
        "INSERT INTO idempotency_records (
            principal_id,method,idempotency_key,request_fingerprint,state,response_json,created_at_ms
        ) VALUES (?1,'mcp.projectPaths.plan',?2,'1','completed',?3,?4)",
        params![caller.as_str(), plan_id.to_string(), json, i64::try_from(now).map_err(|_| ToolError::Unavailable)?],
    )
    .map_err(|_| ToolError::Unavailable)?;
    Ok(())
}

pub(super) fn plan_proof(
    db: &Connection,
    caller: &PrincipalId,
    plan_id: PlanId,
) -> Result<WorkflowPlanPath, ToolError> {
    let value: String = db
        .query_row(
            "SELECT response_json FROM idempotency_records
         WHERE principal_id=?1 AND method='mcp.projectPaths.plan' AND idempotency_key=?2",
            params![caller.as_str(), plan_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|_| ToolError::Unavailable)?
        .ok_or(ToolError::PermissionDenied)?;
    serde_json::from_str(&value).map_err(|_| ToolError::Unavailable)
}

pub(super) fn check_plan_proof(
    db: &Connection,
    caller: &PrincipalId,
    plan_id: PlanId,
) -> Result<(), ToolError> {
    if caller.as_str().starts_with("mcp:") {
        let expected = plan_proof(db, caller, plan_id)?;
        if PATH.with(|slot| slot.borrow().as_ref() != Some(&expected)) {
            return Err(ToolError::PermissionDenied);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_proof_persists_only_binding_and_cannot_be_replaced_on_apply() {
        let root = std::env::temp_dir().join(format!("alcomd-workflow-proof-{}", PlanId::new()));
        std::fs::create_dir(&root).expect("isolated directory");
        let path = root.join("state.db");
        let mut db = crate::sqlite::initialize_connection(&path).expect("isolated state");
        let owner = PrincipalId::parse("mcp:00000000-0000-4000-8000-000000000001").expect("caller");
        let other =
            PrincipalId::parse("mcp:00000000-0000-4000-8000-000000000002").expect("other caller");
        let plan_id = PlanId::new();
        let proof = WorkflowPlanPath::Approved {
            digest: "ab".repeat(32),
        };
        let tx = db.transaction().expect("plan transaction");
        {
            let _context = ContextGuard::install(None, Some(proof.clone()));
            install_plan_proof(&tx, &owner, plan_id, 17).expect("persist immutable proof");
        }
        tx.commit().expect("commit plan transaction");
        assert_eq!(
            plan_proof(&db, &owner, plan_id).expect("owned proof"),
            proof
        );
        assert!(plan_proof(&db, &other, plan_id).is_err());
        {
            let _context = ContextGuard::install(None, Some(WorkflowPlanPath::Default));
            assert!(check_plan_proof(&db, &owner, plan_id).is_err());
        }
        let _context = ContextGuard::install(None, Some(proof));
        assert!(check_plan_proof(&db, &owner, plan_id).is_ok());
        assert!(check_plan_proof(&db, &owner, PlanId::new()).is_err());
        drop(db);
        std::fs::remove_dir_all(root).expect("remove isolated fixture");
    }
}

pub(super) fn require(
    db: &Connection,
    caller: &PrincipalId,
    permissions: &[Permission],
) -> Result<(), ToolError> {
    if !caller.as_str().starts_with("mcp:") {
        return Ok(());
    }
    let authentication = AUTHENTICATION
        .with(|slot| slot.borrow().clone())
        .ok_or(ToolError::PermissionDenied)?;
    let client = super::m8_tools::authenticate(db, &authentication)?;
    if &client.principal_id != caller {
        return Err(ToolError::PermissionDenied);
    }
    let access = authentication.access(client)?;
    for permission in permissions {
        access
            .require(*permission)
            .map_err(|_| ToolError::PermissionDenied)?;
    }
    Ok(())
}
