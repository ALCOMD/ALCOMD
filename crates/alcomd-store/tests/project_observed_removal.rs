use alcomd_application::{ProjectObservation, ProjectRecord, SyncWrite};
use rusqlite::{Connection, params};
use serde_json::{Value, json};

const MIGRATION_V16: &str = include_str!("../migrations/0016_remove_project_observed.sql");
const PROJECT_ID: &str = "00000000-0000-4000-8000-000000000201";
const COPY_PLAN_ID: &str = "00000000-0000-4000-8000-000000000221";
const COPY_OPERATION_ID: &str = "00000000-0000-4000-8000-000000000211";
const MIGRATION_OPERATION_ID: &str = "00000000-0000-4000-8000-000000000212";
const UNAPPLIED_MIGRATION_ID: &str = "00000000-0000-4000-8000-000000000234";
const APPLIED_MIGRATION_ID: &str = "00000000-0000-4000-8000-000000000235";

#[test]
fn state_15_to_16_removes_project_observation_from_every_durable_json_shape() {
    let connection = fixture();
    let triggers_before = trigger_snapshot(&connection);
    let authority_before = authority_snapshot(&connection);
    connection.execute_batch(MIGRATION_V16).expect("apply v16");

    assert_eq!(integer(&connection, "PRAGMA user_version"), 16);
    assert_eq!(
        integer(
            &connection,
            "SELECT count(*) FROM pragma_table_info('projects') WHERE name='observed_at_ms'"
        ),
        0
    );
    assert_eq!(
        integer(&connection, "SELECT last_modified_at_ms FROM projects"),
        404
    );
    assert_eq!(integer(&connection, "SELECT revision FROM projects"), 9);
    assert_eq!(
        integer(&connection, "SELECT registered_at_ms FROM projects"),
        101
    );
    assert_eq!(
        integer(&connection, "SELECT updated_at_ms FROM projects"),
        303
    );

    for (query, pointer) in [
        ("SELECT snapshot_json FROM projects", "/observed_at_ms"),
        (
            "SELECT source_snapshot_json FROM project_copy_plans",
            "/observation/observed_at_ms",
        ),
        (
            "SELECT project_snapshot_json FROM project_delete_plans",
            "/observation/observed_at_ms",
        ),
        (
            "SELECT project_snapshot_json FROM project_unity_migration_plans ORDER BY plan_id LIMIT 1",
            "/observation/observed_at_ms",
        ),
        (
            "SELECT project_snapshot_json FROM project_unity_migration_plans ORDER BY plan_id DESC LIMIT 1",
            "/observation/observed_at_ms",
        ),
        (
            "SELECT evidence_json FROM project_copy_filesystem_journal",
            "/published/observation/observed_at_ms",
        ),
    ] {
        assert_eq!(json_column(&connection, query).pointer(pointer), None);
    }

    for method in [
        "projects.register",
        "projects.refresh",
        "projects.setFavorite",
        "projects.planCopy",
        "projects.planDeleteDirectory",
        "projects.planUnityMigration",
    ] {
        let mut statement = connection
            .prepare("SELECT response_json FROM idempotency_records WHERE method=?1")
            .expect("prepare replay rows");
        let responses = statement
            .query_map([method], |row| row.get::<_, String>(0))
            .expect("query replays");
        for response in responses {
            let response: Value =
                serde_json::from_str(&response.expect("response")).expect("valid replay response");
            for pointer in [
                "/value/observation/observed_at_ms",
                "/plan/sourceProject/observation/observed_at_ms",
                "/plan/project/observation/observed_at_ms",
            ] {
                assert_eq!(response.pointer(pointer), None, "{method}: {pointer}");
            }
        }
    }

    // Removal is scoped to projects. Installation and writer/reobservation times are evidence.
    assert_eq!(
        integer(
            &connection,
            "SELECT observed_at_ms FROM unity_installations"
        ),
        707
    );
    assert_eq!(
        json_column(
            &connection,
            "SELECT target_installation_snapshot_json FROM project_unity_migration_plans LIMIT 1"
        )["observation"]["observed_at_ms"],
        707
    );
    assert_eq!(
        json_column(
            &connection,
            "SELECT evidence_json FROM project_unity_migration_journal"
        )["reobservedAtMs"],
        909
    );
    assert_eq!(
        json_column(
            &connection,
            "SELECT response_json FROM idempotency_records WHERE method='unity.launch'"
        )["observedAtMs"],
        111
    );
    assert_eq!(
        json_column(
            &connection,
            "SELECT response_json FROM idempotency_records WHERE method='projects.planCopy'"
        )["plan"]["writerEvidence"]["checked_at_ms"],
        809
    );
    assert_eq!(
        json_column(&connection, "SELECT snapshot_json FROM projects")["unity_version"],
        "2022.3.22f1"
    );

    // Old unapplied Unity plans cannot acquire a replacement approval hash silently.
    assert_eq!(
        integer(
            &connection,
            "SELECT expires_at_ms FROM project_unity_migration_plans WHERE state='unapplied'"
        ),
        0
    );
    assert_eq!(
        integer(
            &connection,
            "SELECT expires_at_ms FROM project_unity_migration_plans WHERE state='applied'"
        ),
        900001
    );
    assert_eq!(
        json_column(
            &connection,
            "SELECT response_json FROM idempotency_records WHERE idempotency_key='unity-unapplied'"
        )["plan"]["expiresAtMs"],
        0
    );
    assert_eq!(
        json_column(
            &connection,
            "SELECT response_json FROM idempotency_records WHERE idempotency_key='unity-applied'"
        )["plan"]["expiresAtMs"],
        900001
    );
    assert_eq!(authority_snapshot(&connection), authority_before);
    assert_eq!(trigger_snapshot(&connection), triggers_before);
    let observation: ProjectObservation = serde_json::from_value(json_column(
        &connection,
        "SELECT snapshot_json FROM projects",
    ))
    .expect("current project observation reads the migrated snapshot");
    assert_eq!(observation.root_path, "X:/fixture");
    let copied_source: ProjectRecord = serde_json::from_value(json_column(
        &connection,
        "SELECT source_snapshot_json FROM project_copy_plans",
    ))
    .expect("current project record reads the migrated copy source");
    assert_eq!(copied_source.last_modified_at_ms, Some(404));
    let replay: SyncWrite<ProjectRecord> = serde_json::from_value(json_column(
        &connection,
        "SELECT response_json FROM idempotency_records WHERE method='projects.register'",
    ))
    .expect("permanent project replay reads the current model");
    assert_eq!(replay.value, copied_source);

    for sql in [
        "UPDATE project_copy_plans SET source_snapshot_json='{}'",
        "UPDATE project_delete_plans SET project_snapshot_json='{}'",
        "UPDATE project_copy_filesystem_journal SET evidence_json='{}'",
        "DELETE FROM project_copy_plans",
        "DELETE FROM project_delete_plans",
        "DELETE FROM project_copy_filesystem_journal",
    ] {
        assert!(connection.execute(sql, []).is_err(), "guard remains: {sql}");
    }
    assert_eq!(
        integer(&connection, "SELECT count(*) FROM pragma_foreign_key_check"),
        0
    );
}

#[test]
fn failed_state_16_migration_rolls_back_column_json_expiry_and_trigger_changes() {
    let connection = fixture();
    connection
        .execute_batch(
            "CREATE TRIGGER fail_observed_removal BEFORE UPDATE ON project_copy_filesystem_journal
             BEGIN SELECT RAISE(ABORT, 'fixture migration failure'); END;",
        )
        .expect("inject failure after column removal");
    let triggers_before = trigger_snapshot(&connection);
    let authority_before = authority_snapshot(&connection);
    let project_before = json_column(&connection, "SELECT snapshot_json FROM projects");
    assert!(connection.execute_batch(MIGRATION_V16).is_err());
    assert!(!connection.is_autocommit());
    connection
        .execute_batch("ROLLBACK;")
        .expect("rollback migration");
    assert_eq!(integer(&connection, "PRAGMA user_version"), 15);
    assert_eq!(
        integer(&connection, "SELECT observed_at_ms FROM projects"),
        202
    );
    assert_eq!(
        json_column(&connection, "SELECT snapshot_json FROM projects"),
        project_before
    );
    assert_eq!(
        integer(
            &connection,
            "SELECT expires_at_ms FROM project_unity_migration_plans WHERE state='unapplied'"
        ),
        900001
    );
    assert_eq!(trigger_snapshot(&connection), triggers_before);
    assert_eq!(authority_snapshot(&connection), authority_before);
}

fn fixture() -> Connection {
    let connection = Connection::open_in_memory().expect("open memory fixture");
    connection
        .execute_batch("PRAGMA foreign_keys=ON;")
        .expect("foreign keys");
    for migration in [
        include_str!("../migrations/0001_state.sql"),
        include_str!("../migrations/0002_projects_repositories.sql"),
        include_str!("../migrations/0003_package_transactions.sql"),
        include_str!("../migrations/0004_local_workflows.sql"),
        include_str!("../migrations/0005_template_plans.sql"),
        include_str!("../migrations/0006_backup_create.sql"),
        include_str!("../migrations/0007_backup_restore.sql"),
        include_str!("../migrations/0008_extension_runtime.sql"),
        include_str!("../migrations/0009_portable_extension_ui.sql"),
        include_str!("../migrations/0010_project_copy.sql"),
        include_str!("../migrations/0011_project_preferences.sql"),
        include_str!("../migrations/0012_package_functional_closure.sql"),
        include_str!("../migrations/0013_project_directory_delete.sql"),
        include_str!("../migrations/0014_unity_project_version.sql"),
        include_str!("../migrations/0015_project_last_modified.sql"),
    ] {
        connection
            .execute_batch(migration)
            .expect("build authentic v15 schema");
    }
    let observation = json!({
        "root_path":"X:/fixture", "path_identity_key":[1], "project_type":"unknown",
        "unity_version":"2022.3.22f1", "unity_revision":null,
        "vpm_manifest":"missing", "upm_manifest":"missing",
        "direct_dependencies":[], "locked_dependencies":[], "issues":[], "observed_at_ms":202
    });
    let project = json!({
        "project_id":PROJECT_ID, "observation":observation, "revision":9,
        "registered_at_ms":101, "last_modified_at_ms":404, "favorite":false
    });
    let writer = json!({
        "project_id":PROJECT_ID,"state":"not_observed","evidence":[],"checked_at_ms":809
    });
    connection
        .execute(
            "INSERT INTO projects (project_id,owner_principal_id,root_path,path_identity_key,
             project_type,unity_version,snapshot_json,revision,registered_at_ms,observed_at_ms,
             updated_at_ms,last_modified_at_ms) VALUES (?1,'builtin:local-owner','X:/fixture',
             x'01','unknown','2022.3.22f1',?2,9,101,202,303,404)",
            params![PROJECT_ID, observation.to_string()],
        )
        .expect("legacy project");
    connection
        .execute_batch(
            "INSERT INTO unity_installations (installation_id,owner_principal_id,executable_path,
         filesystem_identity_key,unity_version,architecture,source_kind,revision,observed_at_ms,
         updated_at_ms) VALUES ('00000000-0000-4000-8000-000000000202','builtin:local-owner',
         'X:/Unity.exe',x'02','2022.3.22f1','x86_64','manual',1,707,708);
         INSERT INTO operations (operation_id,kind,state,revision,owner_principal_id,
         request_json,result_json,created_at_ms,updated_at_ms,completed_at_ms) VALUES
         ('00000000-0000-4000-8000-000000000211','projects.copy','succeeded',1,
          'builtin:local-owner','{}','{\"targetProjectId\":\"target\"}',1,2,2),
         ('00000000-0000-4000-8000-000000000212','projects.unity-migration','running',1,
          'builtin:local-owner','{}',NULL,1,2,NULL);",
        )
        .expect("independent installation and operations");
    connection
        .execute(
            "INSERT INTO project_copy_plans (plan_id,owner_principal_id,state,source_project_id,
         source_revision,source_root_path,source_root_identity,source_snapshot_json,
         target_parent_path,target_parent_identity,target_parent_identity_sha256,target_leaf,
         target_project_id,profile_version,writer_evidence_json,plan_fingerprint,plan_json,
         plan_idempotency_key,apply_operation_id,created_at_ms,expires_at_ms) VALUES
         (?1,'builtin:local-owner','applied',?2,9,'X:/fixture',x'01',?3,'X:/copies',x'03',
         zeroblob(32),'Copy','00000000-0000-4000-8000-000000000223',1,?4,zeroblob(32),
         '{\"writerEvidence\":{\"checked_at_ms\":809}}','copy-key',?5,1,900001)",
            params![
                COPY_PLAN_ID,
                PROJECT_ID,
                project.to_string(),
                writer.to_string(),
                COPY_OPERATION_ID
            ],
        )
        .expect("copy snapshot");
    connection.execute(
        "INSERT INTO project_delete_plans (plan_id,owner_principal_id,state,project_id,
         project_revision,root_path,root_identity,parent_path,parent_identity,parent_identity_sha256,
         normalized_leaf,project_snapshot_json,marker_fingerprint,writer_evidence_json,deletion_mode,
         safety_profile_id,safety_profile_version,protected_root_profile_version,plan_fingerprint,
         plan_json,plan_idempotency_key,created_at_ms,expires_at_ms) VALUES
         ('00000000-0000-4000-8000-000000000224','builtin:local-owner','unapplied',?1,9,
          'X:/fixture',x'01','X:/',x'04',zeroblob(32),'fixture',?2,zeroblob(32),?3,
          'sibling-quarantine-permanent-v1','alcomd-project-delete',1,1,zeroblob(32),
          '{\"writerEvidence\":{\"checked_at_ms\":809}}','delete-key',1,900001)",
        params![PROJECT_ID,project.to_string(),writer.to_string()],
    ).expect("delete snapshot");
    for (id, state, operation) in [
        (UNAPPLIED_MIGRATION_ID, "unapplied", None),
        (
            APPLIED_MIGRATION_ID,
            "applied",
            Some(MIGRATION_OPERATION_ID),
        ),
    ] {
        connection.execute(
            "INSERT INTO project_unity_migration_plans (plan_id,owner_principal_id,project_id,
             project_revision,source_unity_version,project_root_identity,project_snapshot_json,
             project_version_marker_sha256,target_unity_version,target_installation_id,
             target_installation_revision,target_installation_identity,target_installation_snapshot_json,
             writer_evidence_revision,writer_evidence_json,classification,plan_fingerprint,
             request_fingerprint,plan_idempotency_key,created_at_ms,expires_at_ms,state,operation_id)
             VALUES (?1,'builtin:local-owner',?2,9,'2022.3.22f1',x'01',?3,zeroblob(32),'2022.3.23f1',
             '00000000-0000-4000-8000-000000000202',1,x'02',
             '{\"observation\":{\"observed_at_ms\":707}}',809,?4,'patch_or_minor_upgrade',
             'original-approved-authority-hash','original-request-hash',?1,1,900001,?5,?6)",
            params![id,PROJECT_ID,project.to_string(),writer.to_string(),state,operation],
        ).expect("Unity plan snapshot");
    }
    connection.execute(
        "INSERT INTO project_copy_filesystem_journal (operation_id,step,plan_id,source_project_id,
         target_project_id,phase,state,source_identity,target_parent_identity,evidence_json,updated_at_ms)
         VALUES (?1,1,?2,?3,'00000000-0000-4000-8000-000000000223','target_published','completed',
         x'01',x'03',?4,2)",
        params![COPY_OPERATION_ID,COPY_PLAN_ID,PROJECT_ID,json!({"inventory":null,"published":{"observation":observation,"target_identity":[5]}}).to_string()],
    ).expect("published copy observation");
    connection.execute(
        "INSERT INTO project_unity_migration_journal (operation_id,step,phase,evidence_json,updated_at_ms)
         VALUES (?1,1,'project_reobserved','{\"reobservedAtMs\":909,\"writerInactiveCheckedAtMs\":910}',911)",
        [MIGRATION_OPERATION_ID],
    ).expect("independent recovery timestamps");
    for method in [
        "projects.register",
        "projects.refresh",
        "projects.setFavorite",
    ] {
        insert_replay(
            &connection,
            method,
            method,
            json!({"value":project,"replayed":false}),
        );
    }
    insert_replay(
        &connection,
        "projects.planCopy",
        "copy",
        json!({"plan":{"sourceProject":project,"writerEvidence":writer},"replayed":false}),
    );
    insert_replay(
        &connection,
        "projects.planDeleteDirectory",
        "delete",
        json!({"plan":{"project":project,"writerEvidence":writer},"replayed":false}),
    );
    for (key, id) in [
        ("unity-unapplied", UNAPPLIED_MIGRATION_ID),
        ("unity-applied", APPLIED_MIGRATION_ID),
    ] {
        insert_replay(
            &connection,
            "projects.planUnityMigration",
            key,
            json!({"kind":"planned","plan":{"planId":id,"project":project,"writerEvidence":writer,"expiresAtMs":900001,"planFingerprint":"original-approved-authority-hash"},"replayed":false}),
        );
    }
    insert_replay(
        &connection,
        "unity.launch",
        "unrelated",
        json!({"observedAtMs":111}),
    );
    connection
}

fn insert_replay(connection: &Connection, method: &str, key: &str, response: Value) {
    connection.execute(
        "INSERT INTO idempotency_records (principal_id,method,idempotency_key,request_fingerprint,
         state,response_json,created_at_ms) VALUES ('builtin:local-owner',?1,?2,'{}','completed',?3,1)",
        params![method,key,response.to_string()],
    ).expect("permanent replay");
}

fn integer(connection: &Connection, query: &str) -> i64 {
    connection
        .query_row(query, [], |row| row.get(0))
        .expect("integer query")
}

fn json_column(connection: &Connection, query: &str) -> Value {
    let text: String = connection
        .query_row(query, [], |row| row.get(0))
        .expect("JSON query");
    serde_json::from_str(&text).expect("valid JSON")
}

fn trigger_snapshot(connection: &Connection) -> Vec<(String, String)> {
    connection
        .prepare("SELECT name,sql FROM sqlite_schema WHERE type='trigger' ORDER BY name")
        .expect("prepare trigger snapshot")
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("query trigger snapshot")
        .collect::<Result<_, _>>()
        .expect("collect trigger snapshot")
}

fn authority_snapshot(connection: &Connection) -> Vec<String> {
    [
        "SELECT json_array(hex(plan_fingerprint),plan_json,writer_evidence_json) FROM project_copy_plans",
        "SELECT json_array(hex(plan_fingerprint),plan_json,writer_evidence_json) FROM project_delete_plans",
        "SELECT json_array(plan_fingerprint,request_fingerprint,writer_evidence_json,target_installation_snapshot_json,state,operation_id) FROM project_unity_migration_plans ORDER BY plan_id",
        "SELECT json_array(operation_id,state,result_json,updated_at_ms) FROM operations ORDER BY operation_id",
        "SELECT json_array(method,idempotency_key,request_fingerprint) FROM idempotency_records ORDER BY method,idempotency_key",
        "SELECT evidence_json FROM project_unity_migration_journal",
    ].into_iter().flat_map(|query| {
        connection.prepare(query).expect("prepare authority snapshot")
            .query_map([],|row|row.get::<_,String>(0)).expect("query authority snapshot")
            .collect::<Result<Vec<_>,_>>().expect("collect authority snapshot")
    }).collect()
}
