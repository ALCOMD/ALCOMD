//! Durable user-action history. Callers operate inside the business transaction.
//! No raw RPC bodies, filesystem paths or journal payloads are copied here.

use alcomd_application::{
    NewFailedActivity, OfficialActivityItem, OfficialActivityResult, OfficialActivitySubject,
    PrincipalId, StoreError, valid_activity_parameters,
};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde_json::{Value, json};

pub(super) const COLUMNS: &str = "activity_id,revision,action,status,started_at_ms,finished_at_ms,subject_json,parameters_json,result_json,operation_id,diagnostic_id";

pub(super) fn decode(row: &Row<'_>) -> rusqlite::Result<OfficialActivityItem> {
    let subject: String = row.get(6)?;
    let parameters: String = row.get(7)?;
    let result: Option<String> = row.get(8)?;
    let parsed = |error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    };
    let action: String = row.get(2)?;
    let parameters = serde_json::from_str(&parameters).map_err(parsed)?;
    if !valid_activity_parameters(&action, &parameters) {
        return Err(rusqlite::Error::InvalidQuery);
    }
    Ok(OfficialActivityItem {
        activity_id: row.get(0)?,
        revision: u64::try_from(row.get::<_, i64>(1)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        action,
        status: row.get(3)?,
        started_at_ms: u64::try_from(row.get::<_, i64>(4)?)
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        finished_at_ms: row
            .get::<_, Option<i64>>(5)?
            .map(u64::try_from)
            .transpose()
            .map_err(|_| rusqlite::Error::InvalidQuery)?,
        subject: serde_json::from_str(&subject).map_err(parsed)?,
        parameters,
        result: result
            .map(|value| serde_json::from_str(&value).map_err(parsed))
            .transpose()?,
        operation_id: row.get(9)?,
        diagnostic_id: row.get(10)?,
    })
}

pub(super) fn get(
    connection: &Connection,
    owner: &PrincipalId,
    id: &str,
) -> Result<OfficialActivityItem, StoreError> {
    connection
        .query_row(
            &format!(
                "SELECT {COLUMNS} FROM activities WHERE owner_principal_id=?1 AND activity_id=?2"
            ),
            params![owner.as_str(), id],
            decode,
        )
        .optional()
        .map_err(|error| super::sqlite::unavailable().with_source(error))?
        .ok_or_else(|| StoreError::new(alcomd_application::StoreErrorKind::OperationNotFound))
}

pub(super) fn insert_failed(
    connection: &Connection,
    owner: &PrincipalId,
    value: &NewFailedActivity,
) -> Result<OfficialActivityItem, StoreError> {
    if !valid_activity_parameters(&value.action, &value.parameters) {
        return Err(super::sqlite::unavailable());
    }
    let transaction = connection
        .unchecked_transaction()
        .map_err(|error| super::sqlite::unavailable().with_source(error))?;
    insert(
        &transaction,
        &value.activity_id,
        owner.as_str(),
        &value.action,
        "failed",
        value.occurred_at_ms,
        &value.subject,
        &value.parameters,
        &value.error_code,
        value.diagnostic_id.as_deref(),
    )
    .map_err(|error| super::sqlite::unavailable().with_source(error))?;
    let result = get(&transaction, owner, &value.activity_id)?;
    transaction
        .commit()
        .map_err(|error| super::sqlite::unavailable().with_source(error))?;
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn insert(
    connection: &Connection,
    id: &str,
    owner: &str,
    action: &str,
    status: &str,
    now: u64,
    subject: &OfficialActivitySubject,
    parameters: &Value,
    code: &str,
    diagnostic_id: Option<&str>,
) -> rusqlite::Result<()> {
    if !valid_activity_parameters(action, parameters) {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let subject = serde_json::to_string(subject).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let parameters =
        serde_json::to_string(parameters).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let result = serde_json::to_string(&OfficialActivityResult {
        code: code.to_owned(),
    })
    .map_err(|_| rusqlite::Error::InvalidQuery)?;
    connection.execute("INSERT INTO activities(activity_id,owner_principal_id,revision,action,status,started_at_ms,finished_at_ms,updated_at_ms,subject_json,parameters_json,result_json,diagnostic_id) VALUES(?1,?2,1,?3,?4,?5,?5,?5,?6,?7,?8,?9)",params![id,owner,action,status,i64::try_from(now).map_err(|_|rusqlite::Error::InvalidQuery)?,subject,parameters,result,diagnostic_id])?;
    Ok(())
}

/// A synchronous command result is written in its existing transaction. Removal
/// calls this before deletion, so the target name is a real pre-delete snapshot.
pub(super) fn resource(
    connection: &Connection,
    owner: &PrincipalId,
    action: &str,
    kind: &str,
    id: &str,
    now: u64,
) -> rusqlite::Result<()> {
    let subject = subject(connection, owner.as_str(), kind, id)?;
    let parameters = if action == "project.favorite" {
        json!({"favorite":connection.query_row("SELECT favorite FROM projects WHERE project_id=?1 AND owner_principal_id=?2",params![id,owner.as_str()],|r| r.get::<_,bool>(0))?})
    } else if action == "template.favorite" {
        json!({"favorite":connection.query_row("SELECT favorite FROM templates WHERE template_id=?1 AND owner_principal_id=?2",params![id,owner.as_str()],|r| r.get::<_,bool>(0))?})
    } else {
        json!({})
    };
    let id = uuid::Uuid::new_v4().to_string();
    let code = if action == "unity.launch" {
        "unity.launch.request_submitted".to_owned()
    } else {
        format!("{action}.succeeded")
    };
    insert(
        connection,
        &id,
        owner.as_str(),
        action,
        "succeeded",
        now,
        &subject,
        &parameters,
        &code,
        None,
    )
}

/// Frozen mapping of committed synchronous actions, not an Events read model.
pub(super) fn resource_event(
    connection: &Connection,
    owner: &PrincipalId,
    event: &str,
    kind: &str,
    id: &str,
    now: u64,
) -> rusqlite::Result<()> {
    let action = match event {
        "project.registered" => "project.add",
        "project.refreshed" => "project.refresh",
        "project.favorite_changed" => "project.favorite",
        "repository.registered" => "repository.add",
        "repository.refreshed" => "repository.fetch",
        "unity.installation.registered" => "unity.installation.add",
        "unity.project_launch_config_changed" => "unity.launch-config.change",
        "template.favorite_changed" => "template.favorite",
        "user_package.enrolled" => "user-package.add",
        "user_package.refreshed" => "user-package.refresh",
        _ => return Ok(()),
    };
    resource(connection, owner, action, kind, id, now)
}

pub(super) fn permission_changed(
    connection: &Connection,
    owner: &PrincipalId,
    extension_id: &str,
    permission: &str,
    granted: bool,
    now: u64,
) -> rusqlite::Result<()> {
    let target = subject(connection, owner.as_str(), "extension", extension_id)?;
    insert(
        connection,
        &uuid::Uuid::new_v4().to_string(),
        owner.as_str(),
        "extensions.permission.change",
        "succeeded",
        now,
        &target,
        &json!({"permission":permission,"granted":granted}),
        "extensions.permission.changed",
        None,
    )
}

pub(super) fn launch_created(
    connection: &Connection,
    owner: &PrincipalId,
    launch_id: &str,
    project_id: &str,
    now: u64,
) -> rusqlite::Result<()> {
    let target = subject(connection, owner.as_str(), "project", project_id)?;
    connection.execute("INSERT INTO activities(activity_id,owner_principal_id,revision,action,status,started_at_ms,updated_at_ms,subject_json,parameters_json) VALUES(?1,?2,1,'unity.launch','running',?3,?3,?4,'{}')",params![launch_id,owner.as_str(),i64::try_from(now).map_err(|_|rusqlite::Error::InvalidQuery)?,serde_json::to_string(&target).map_err(|_|rusqlite::Error::InvalidQuery)?])?;
    Ok(())
}

pub(super) fn launch_updated(
    connection: &Connection,
    owner: &PrincipalId,
    launch_id: &str,
    created_at_ms: u64,
    failed: bool,
    accepted: bool,
    diagnostic_id: Option<&str>,
) -> rusqlite::Result<()> {
    if !failed && !accepted {
        return Ok(());
    }
    let status = if failed { "failed" } else { "succeeded" };
    let code = if failed {
        "unity_launch_failed"
    } else {
        "unity.launch.request_submitted"
    };
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .and_then(|v| u64::try_from(v.as_millis()).ok())
        .unwrap_or(created_at_ms)
        .max(created_at_ms);
    connection.execute("UPDATE activities SET revision=revision+1,status=?1,finished_at_ms=?2,updated_at_ms=?2,result_json=json_object('code',?3),diagnostic_id=coalesce(?6,diagnostic_id) WHERE activity_id=?4 AND owner_principal_id=?5 AND (status<>?1 OR (?6 IS NOT NULL AND diagnostic_id IS NOT ?6))",params![status,i64::try_from(now).map_err(|_|rusqlite::Error::InvalidQuery)?,code,launch_id,owner.as_str(),diagnostic_id])?;
    Ok(())
}

fn subject(
    connection: &Connection,
    owner: &str,
    kind: &str,
    id: &str,
) -> rusqlite::Result<OfficialActivitySubject> {
    let name=match kind {
        "project" | "project-unity-launch-config" => connection.query_row("SELECT root_path FROM projects WHERE owner_principal_id=?1 AND project_id=?2",params![owner,id],|row|row.get::<_,String>(0)).optional()?.map(|path|leaf(&path)),
        "repository"=>connection.query_row("SELECT name FROM repositories WHERE owner_principal_id=?1 AND repository_id=?2",params![owner,id],|row|row.get::<_,Option<String>>(0)).optional()?.flatten(),
        "unity-installation"=>connection.query_row("SELECT unity_version FROM unity_installations WHERE owner_principal_id=?1 AND installation_id=?2",params![owner,id],|row|row.get::<_,String>(0)).optional()?,
        "template"=>connection.query_row("SELECT json_extract(manifest_json,'$.name') FROM templates WHERE owner_principal_id=?1 AND template_id=?2",params![owner,id],|row|row.get::<_,Option<String>>(0)).optional()?.flatten(),
        "user-package"=>connection.query_row("SELECT CASE WHEN json_type(manifest_json,'$.displayName')='text' THEN json_extract(manifest_json,'$.displayName') ELSE package_id END FROM user_package_sources WHERE owner_principal_id=?1 AND user_package_id=?2",params![owner,id],|row|row.get::<_,Option<String>>(0)).optional()?.flatten(),
        "extension"=>Some(id.to_owned()),
        _=>None,
    };
    Ok(OfficialActivitySubject {
        kind: if kind == "project-unity-launch-config" {
            "project".to_owned()
        } else {
            kind.to_owned()
        },
        id: Some(id.to_owned()),
        name: name.map(|v| v.chars().filter(|c| !c.is_control()).take(512).collect()),
    })
}
fn leaf(path: &str) -> String {
    path.trim_end_matches(['/', char::from(92)])
        .rsplit(['/', char::from(92)])
        .next()
        .unwrap_or(path)
        .to_owned()
}

/// Invoked immediately after the task INSERT and before accepting its Apply.
/// The immutable plan is consulted once; later target changes cannot alter this.
pub(super) fn snapshot_operation(
    connection: &Connection,
    operation_id: &str,
) -> rusqlite::Result<()> {
    let (kind, owner, request): (String, String, String) = connection.query_row(
        "SELECT kind,owner_principal_id,request_json FROM operations WHERE operation_id=?1",
        [operation_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let request: Value =
        serde_json::from_str(&request).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let plan = request.get("planId").and_then(Value::as_str);
    let mut action = kind.clone();
    let mut parameters = json!({});
    let mut target = None;
    if let Some(plan) = plan {
        parameters["planId"] = json!(plan);
        match kind.as_str(){
            "packages.apply"=>{let (id,subaction,changes):(String,String,String)=connection.query_row("SELECT project_id,action,change_set_json FROM package_plans WHERE plan_id=?1 AND owner_principal_id=?2",params![plan,owner],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?; action=format!("packages.{subaction}");target=Some(("project",id)); let changes:Value=serde_json::from_str(&changes).map_err(|_|rusqlite::Error::InvalidQuery)?;parameters["changeCount"]=json!(changes.get("mutations").and_then(Value::as_array).map_or(0,Vec::len));}
            "projects.copy"=>target=Some(("project",connection.query_row("SELECT source_project_id FROM project_copy_plans WHERE plan_id=?1 AND owner_principal_id=?2",params![plan,owner],|r|r.get(0))?)),
            "projects.delete-directory"=>target=Some(("project",connection.query_row("SELECT project_id FROM project_delete_plans WHERE plan_id=?1 AND owner_principal_id=?2",params![plan,owner],|r|r.get(0))?)),
            "projects.unity-migration"=>{let(id,version):(String,String)=connection.query_row("SELECT project_id,target_unity_version FROM project_unity_migration_plans WHERE plan_id=?1 AND owner_principal_id=?2",params![plan,owner],|r|Ok((r.get(0)?,r.get(1)?)))?;target=Some(("project",id));parameters["unityVersion"]=json!(version);}
            "extensions.install"|"extensions.uninstall"=>target=Some(("extension",connection.query_row("SELECT extension_id FROM extension_plans WHERE plan_id=?1 AND owner_principal_id=?2",params![plan,owner],|r|r.get(0))?)),
            "templates.import"|"templates.derive"|"templates.create-project"=>{let raw:String=connection.query_row("SELECT plan_json FROM template_plans WHERE plan_id=?1 AND owner_principal_id=?2",params![plan,owner],|r|r.get(0))?; let raw:Value=serde_json::from_str(&raw).map_err(|_|rusqlite::Error::InvalidQuery)?;if let Some(id)=raw.get("sourceProjectId").and_then(Value::as_str){target=Some(("project",id.to_owned()));}else if let Some(id)=raw.get("templateId").and_then(Value::as_str){target=Some(("template",id.to_owned()));}}
            "backups.restore"=>{let raw:String=connection.query_row("SELECT plan_json FROM backup_restore_plans WHERE plan_id=?1 AND owner_principal_id=?2",params![plan,owner],|r|r.get(0))?;let raw:Value=serde_json::from_str(&raw).map_err(|_|rusqlite::Error::InvalidQuery)?;if let Some(id)=raw.get("backupId").and_then(Value::as_str){target=Some(("backup",id.to_owned()));}}
            _=>{}
        }
    }
    if kind == "backups.create" {
        if let Some(id) = request.get("projectId").and_then(Value::as_str) {
            target = Some(("project", id.to_owned()));
        }
        for (from, to) in [
            ("compressionMode", "compression"),
            ("excludeVpmPackages", "excludeVpmPackages"),
            ("includeLibrary", "includeLibrary"),
        ] {
            if let Some(v) = request.get(from) {
                parameters[to] = v.clone();
            }
        }
    }
    if !valid_activity_parameters(&action, &parameters) {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let subject = match target {
        Some((kind, id)) => subject(connection, &owner, kind, &id)?,
        None => OfficialActivitySubject {
            kind: kind.split('.').next().unwrap_or("operation").to_owned(),
            id: None,
            name: None,
        },
    };
    connection.execute("UPDATE activities SET action=?1,subject_json=?2,parameters_json=?3 WHERE operation_id=?4 AND revision=1",params![action,serde_json::to_string(&subject).map_err(|_|rusqlite::Error::InvalidQuery)?,serde_json::to_string(&parameters).map_err(|_|rusqlite::Error::InvalidQuery)?,operation_id])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alcomd_application::{OfficialActivityCursor, OperationId};
    struct Directory(std::path::PathBuf);
    impl Directory {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("alcomd-activity-{}", OperationId::new()));
            std::fs::create_dir(&p).expect("isolated directory");
            Self(p)
        }
        fn database(&self) -> std::path::PathBuf {
            self.0.join("state.db")
        }
        fn connection(&self) -> Connection {
            crate::sqlite::initialize_connection(&self.database()).expect("fresh State19")
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn task(c: &Connection, owner: &PrincipalId, now: i64) -> String {
        let id = OperationId::new().to_string();
        c.execute("INSERT INTO operations(operation_id,kind,state,revision,owner_principal_id,request_json,created_at_ms,updated_at_ms) VALUES(?1,'state.check','queued',1,?2,'{}',?3,?3)",params![id,owner.as_str(),now]).expect("create task");
        id
    }
    fn activity_count(c: &Connection) -> i64 {
        c.query_row("SELECT count(*) FROM activities", [], |r| r.get(0))
            .expect("valid isolated activity fixture")
    }
    #[test]
    fn new_schema_has_no_backfill_and_task_lifecycle_is_atomic() {
        let d = Directory::new();
        let c = d.connection();
        let owner = PrincipalId::local_owner();
        assert_eq!(
            c.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
                .expect("valid isolated activity fixture"),
            19
        );
        assert_eq!(activity_count(&c), 0);
        let id = task(&c, &owner, 10);
        let a = get(&c, &owner, &id).expect("valid isolated activity fixture");
        assert_eq!(a.status, "queued");
        assert_eq!(a.revision, 1);
        for (state, time) in [
            ("planning", 11),
            ("waiting_for_input", 12),
            ("running", 13),
            ("cancelling", 14),
            ("interrupted", 15),
            ("recovering", 16),
            ("succeeded", 17),
        ] {
            c.execute("UPDATE operations SET state=?1,revision=revision+1,updated_at_ms=?2 WHERE operation_id=?3",params![state,time,id]).expect("valid isolated activity fixture");
            assert_eq!(
                get(&c, &owner, &id)
                    .expect("valid isolated activity fixture")
                    .status,
                state
            );
        }
        let a = get(&c, &owner, &id).expect("valid isolated activity fixture");
        assert_eq!(a.revision, 8);
        assert_eq!(a.finished_at_ms, Some(17));
        assert_eq!(activity_count(&c), 1);
        let events:i64=c.query_row("SELECT count(*) FROM events WHERE kind='activity.changed' AND aggregate_kind='activity' AND aggregate_id=?1",[id],|r|r.get(0)).expect("valid isolated activity fixture");
        assert_eq!(events, 8);
    }
    #[test]
    fn repeated_status_and_progress_do_not_change_activity_revision() {
        let d = Directory::new();
        let c = d.connection();
        let owner = PrincipalId::local_owner();
        let id = task(&c, &owner, 10);
        c.execute(
            "UPDATE operations SET revision=revision+1,updated_at_ms=11 WHERE operation_id=?1",
            [&id],
        )
        .expect("valid isolated activity fixture");
        c.execute("UPDATE operations SET state='queued',revision=revision+1,updated_at_ms=12 WHERE operation_id=?1",[&id]).expect("valid isolated activity fixture");
        assert_eq!(
            get(&c, &owner, &id)
                .expect("valid isolated activity fixture")
                .revision,
            1
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM events", [], |r| r.get::<_, i64>(0))
                .expect("valid isolated activity fixture"),
            1
        );
    }
    #[test]
    fn failed_activity_event_rolls_back_task_creation_and_terminal_state() {
        let d = Directory::new();
        let c = d.connection();
        let owner = PrincipalId::local_owner();
        let id = task(&c, &owner, 10);
        c.execute_batch("CREATE TEMP TRIGGER fail_history BEFORE INSERT ON events WHEN NEW.kind='activity.changed' BEGIN SELECT RAISE(ABORT,'injected history failure'); END;").expect("valid isolated activity fixture");
        assert!(c.execute("UPDATE operations SET state='failed',revision=2,error_code='project_inaccessible',updated_at_ms=20 WHERE operation_id=?1",[&id]).is_err());
        assert_eq!(
            get(&c, &owner, &id)
                .expect("valid isolated activity fixture")
                .status,
            "queued"
        );
        assert_eq!(
            c.query_row(
                "SELECT state FROM operations WHERE operation_id=?1",
                [&id],
                |r| r.get::<_, String>(0)
            )
            .expect("valid isolated activity fixture"),
            "queued"
        );
        let next = OperationId::new().to_string();
        assert!(c.execute("INSERT INTO operations(operation_id,kind,state,revision,owner_principal_id,request_json,created_at_ms,updated_at_ms) VALUES(?1,'state.check','queued',1,?2,'{}',30,30)",params![next,owner.as_str()]).is_err());
        assert_eq!(activity_count(&c), 1);
    }
    #[test]
    fn terminal_transaction_rollback_and_restart_preserve_single_history() {
        let d = Directory::new();
        let mut c = d.connection();
        let owner = PrincipalId::local_owner();
        let id = task(&c, &owner, 10);
        {
            let tx = c.transaction().expect("valid isolated activity fixture");
            tx.execute("UPDATE operations SET state='succeeded',revision=2,updated_at_ms=20 WHERE operation_id=?1",[&id]).expect("valid isolated activity fixture");
        }
        assert_eq!(
            get(&c, &owner, &id)
                .expect("valid isolated activity fixture")
                .status,
            "queued"
        );
        {
            let tx = c.transaction().expect("valid isolated activity fixture");
            tx.execute("UPDATE operations SET state='succeeded',revision=2,updated_at_ms=20 WHERE operation_id=?1",[&id]).expect("valid isolated activity fixture");
            tx.commit().expect("valid isolated activity fixture");
        }
        drop(c);
        let c = d.connection();
        assert_eq!(activity_count(&c), 1);
        assert_eq!(
            get(&c, &owner, &id)
                .expect("valid isolated activity fixture")
                .status,
            "succeeded"
        );
    }
    #[test]
    fn owner_and_fixed_start_cursor_survive_status_updates() {
        let d = Directory::new();
        let c = d.connection();
        let owner = PrincipalId::local_owner();
        let other = PrincipalId::parse("test:other").expect("valid isolated activity fixture");
        let first = task(&c, &owner, 10);
        let second = task(&c, &owner, 20);
        let hidden = task(&c, &other, 30);
        let page = crate::m7_official::list_activity(&c, &owner, None, 1)
            .expect("valid isolated activity fixture");
        assert_eq!(page.items[0].activity_id, second);
        c.execute("UPDATE operations SET state='running',revision=2,updated_at_ms=99 WHERE operation_id=?1",[&first]).expect("valid isolated activity fixture");
        let cursor = OfficialActivityCursor {
            started_at_ms: 20,
            activity_id: second,
        };
        let page = crate::m7_official::list_activity(&c, &owner, Some(&cursor), 1)
            .expect("valid isolated activity fixture");
        assert_eq!(page.items[0].activity_id, first);
        assert!(get(&c, &owner, &hidden).is_err());
        assert_eq!(
            get(&c, &owner, &first)
                .expect("valid isolated activity fixture")
                .started_at_ms,
            10
        );
    }
    #[test]
    fn precheck_failure_is_independent_but_not_a_business_mutation() {
        let d = Directory::new();
        let c = d.connection();
        let owner = PrincipalId::local_owner();
        let failure = NewFailedActivity {
            activity_id: OperationId::new().to_string(),
            action: "project.add".into(),
            subject: OfficialActivitySubject {
                kind: "project".into(),
                id: None,
                name: Some("Example".into()),
            },
            parameters: json!({}),
            diagnostic_id: Some(OperationId::new().to_string()),
            occurred_at_ms: 10,
            error_code: "project_manifest_invalid".into(),
        };
        let a = insert_failed(&c, &owner, &failure).expect("valid isolated activity fixture");
        assert_eq!(a.status, "failed");
        assert_eq!(a.diagnostic_id, failure.diagnostic_id);
        assert_eq!(
            c.query_row("SELECT count(*) FROM operations", [], |r| r
                .get::<_, i64>(0))
                .expect("valid isolated activity fixture"),
            0
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM projects", [], |r| r.get::<_, i64>(0))
                .expect("valid isolated activity fixture"),
            0
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM idempotency_records", [], |r| r
                .get::<_, i64>(0))
                .expect("valid isolated activity fixture"),
            0
        );
        assert!(insert_failed(&c, &owner, &failure).is_err());
        let mut retry = failure.clone();
        retry.activity_id = OperationId::new().to_string();
        insert_failed(&c, &owner, &retry).expect("valid isolated activity fixture");
        assert_eq!(activity_count(&c), 2);
        let mut invalid = retry;
        invalid.activity_id = OperationId::new().to_string();
        invalid.parameters = json!({"argv":["secret"],"path":"private"});
        assert!(insert_failed(&c, &owner, &invalid).is_err());
        invalid.parameters = json!({"favorite": true});
        assert!(
            insert_failed(&c, &owner, &invalid).is_err(),
            "a favorite field cannot attach to project.add"
        );
        assert_eq!(activity_count(&c), 2);
        invalid.action = "project.favorite".into();
        insert_failed(&c, &owner, &invalid).expect("favorite field belongs to its own action");
        assert_eq!(activity_count(&c), 3);
    }
    #[test]
    fn target_snapshot_is_not_rewritten_when_the_target_disappears() {
        let d = Directory::new();
        let mut c = d.connection();
        let owner = PrincipalId::local_owner();
        let key = |value| {
            alcomd_application::IdempotencyKey::parse(value)
                .expect("valid isolated activity fixture")
        };
        let project = crate::m3::register_project(
            &mut c,
            &owner,
            crate::events_sources_tests::project_observation(),
            &key("add"),
            1,
        )
        .expect("valid isolated activity fixture")
        .value;
        let before = crate::m7_official::list_activity(&c, &owner, None, 100)
            .expect("valid isolated activity fixture");
        assert_eq!(before.items.len(), 1);
        assert_eq!(before.items[0].subject.name.as_deref(), Some("Project"));
        crate::m3::unregister_project(
            &mut c,
            &owner,
            project.project_id,
            project.revision,
            &key("remove"),
            2,
        )
        .expect("valid isolated activity fixture");
        assert_eq!(
            c.query_row("SELECT count(*) FROM projects", [], |r| r.get::<_, i64>(0))
                .expect("valid isolated activity fixture"),
            0
        );
        assert_eq!(
            get(&c, &owner, &before.items[0].activity_id).expect("valid isolated activity fixture"),
            before.items[0]
        );
        let after = crate::m7_official::list_activity(&c, &owner, None, 100)
            .expect("valid isolated activity fixture");
        assert_eq!(after.items.len(), 2);
        assert_eq!(after.items[0].action, "project.remove");
        assert_eq!(after.items[0].subject, before.items[0].subject);
    }
    #[test]
    fn activity_failure_rolls_back_synchronous_business_and_idempotency() {
        let d = Directory::new();
        let mut c = d.connection();
        let owner = PrincipalId::local_owner();
        let key = |value| {
            alcomd_application::IdempotencyKey::parse(value)
                .expect("valid isolated activity fixture")
        };
        let project = crate::m3::register_project(
            &mut c,
            &owner,
            crate::events_sources_tests::project_observation(),
            &key("add"),
            1,
        )
        .expect("valid isolated activity fixture")
        .value;
        let before: i64 = c
            .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
            .expect("valid isolated activity fixture");
        c.execute_batch("CREATE TEMP TRIGGER fail_activity BEFORE INSERT ON events WHEN NEW.kind='activity.changed' BEGIN SELECT RAISE(ABORT,'injected history failure'); END;").expect("valid isolated activity fixture");
        assert!(
            crate::m3::set_project_favorite(
                &mut c,
                &owner,
                project.project_id,
                true,
                project.revision,
                &key("favorite"),
                2
            )
            .is_err()
        );
        assert_eq!(
            c.query_row("SELECT favorite FROM projects", [], |r| r.get::<_, i64>(0))
                .expect("valid isolated activity fixture"),
            0
        );
        assert_eq!(
            c.query_row("SELECT count(*) FROM events", [], |r| r.get::<_, i64>(0))
                .expect("valid isolated activity fixture"),
            before
        );
        assert_eq!(
            c.query_row(
                "SELECT count(*) FROM idempotency_records WHERE idempotency_key='favorite'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .expect("valid isolated activity fixture"),
            0
        );
        c.execute_batch("DROP TRIGGER fail_activity;")
            .expect("valid isolated activity fixture");
        let result = crate::m3::set_project_favorite(
            &mut c,
            &owner,
            project.project_id,
            true,
            project.revision,
            &key("favorite"),
            2,
        )
        .expect("valid isolated activity fixture");
        assert!(!result.replayed);
        let replay = crate::m3::set_project_favorite(
            &mut c,
            &owner,
            project.project_id,
            true,
            project.revision,
            &key("favorite"),
            3,
        )
        .expect("valid isolated activity fixture");
        assert!(replay.replayed);
        assert_eq!(activity_count(&c), 2);
    }
}
