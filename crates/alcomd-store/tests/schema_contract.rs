use rusqlite::{Connection, params};

const MIGRATION_V1: &str = include_str!("../migrations/0001_state.sql");
const MIGRATION_V2: &str = include_str!("../migrations/0002_projects_repositories.sql");
const MIGRATION_V3: &str = include_str!("../migrations/0003_package_transactions.sql");
const MIGRATION_V4: &str = include_str!("../migrations/0004_local_workflows.sql");

#[test]
fn bundled_sqlite_version_is_frozen() {
    assert_eq!(rusqlite::version(), "3.53.2");
}

#[test]
fn migration_v1_is_single_transaction_and_sets_version_last() {
    let begin = MIGRATION_V1.find("BEGIN IMMEDIATE;").expect("begin");
    let version = MIGRATION_V1
        .find("PRAGMA user_version = 1;")
        .expect("user_version");
    let commit = MIGRATION_V1.rfind("COMMIT;").expect("commit");
    assert_eq!(begin, 0);
    assert!(version < commit);
    assert!(!MIGRATION_V1[version + 1..commit].contains("CREATE "));

    let connection = Connection::open_in_memory().expect("open SQLite");
    connection
        .execute_batch("PRAGMA foreign_keys=ON;")
        .expect("enable foreign keys");
    connection
        .execute_batch(MIGRATION_V1)
        .expect("apply migration");
    assert_eq!(user_version(&connection), 1);
    assert_eq!(pragma_i64(&connection, "PRAGMA foreign_keys"), 1);
}

#[test]
fn failed_migration_rolls_back_every_table_and_user_version() {
    let connection = Connection::open_in_memory().expect("open database");
    let failing = MIGRATION_V1.replace(
        "PRAGMA user_version = 1;",
        "THIS IS NOT VALID SQL;\nPRAGMA user_version = 1;",
    );
    assert!(connection.execute_batch(&failing).is_err());
    connection
        .execute_batch("ROLLBACK;")
        .expect("rollback failed migration");
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("read user version");
    let tables: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_schema
             WHERE type='table' AND name IN (
                'operations', 'operation_journal', 'events', 'idempotency_records'
             )",
            [],
            |row| row.get(0),
        )
        .expect("count contract tables");
    assert_eq!(version, 0);
    assert_eq!(tables, 0);
}

#[test]
fn schema_v1_has_only_the_approved_strict_tables_and_indexes() {
    let connection = migrated_connection();
    let mut statement = connection
        .prepare(
            "SELECT name, sql FROM sqlite_schema \
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .expect("prepare schema query");
    let tables = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .expect("query tables")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect tables");
    assert_eq!(
        tables
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        [
            "events",
            "idempotency_records",
            "operation_journal",
            "operations"
        ]
    );
    assert!(tables.iter().all(|(_, sql)| sql.ends_with("STRICT")));

    for index in [
        "events_principal_sequence",
        "operations_owner_page",
        "operations_recovery",
    ] {
        let count: i64 = connection
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE type='index' AND name=?1",
                [index],
                |row| row.get(0),
            )
            .expect("query index");
        assert_eq!(count, 1, "missing index {index}");
    }
}

#[test]
fn schema_v1_rejects_invalid_operation_and_finite_states() {
    let connection = migrated_connection();
    insert_operation(&connection, "queued", 1).expect("valid operation");

    assert!(insert_operation(&connection, "unknown", 1).is_err());
    assert!(insert_operation(&connection, "running", 0).is_err());

    let operation_id = "00000000-0000-4000-8000-000000000001";
    assert!(
        connection
            .execute(
                "INSERT INTO operation_journal \
                 (operation_id, step, kind, state, payload_json, updated_at_ms) \
                 VALUES (?1, 1, 'integrity-check', 'unknown', '{}', 1)",
                [operation_id],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "INSERT INTO idempotency_records \
                 (principal_id, method, idempotency_key, request_fingerprint, state, \
                  operation_id, response_json, created_at_ms) \
                 VALUES ('builtin:local-owner', 'state.check', 'key', '{}', 'expired', \
                         ?1, NULL, 1)",
                [operation_id],
            )
            .is_err()
    );
}

#[test]
fn schema_v1_has_no_expiry_or_future_business_tables() {
    assert!(!MIGRATION_V1.contains("expires_at"));
    for forbidden in [
        "projects",
        "packages",
        "repositories",
        "settings",
        "extensions",
    ] {
        assert!(!MIGRATION_V1.contains(&format!("CREATE TABLE {forbidden}")));
    }
}

#[test]
fn migration_v2_is_atomic_and_adds_only_the_three_m3_tables() {
    let connection = migrated_v2_connection();
    assert_eq!(user_version(&connection), 2);
    let tables = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type='table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .expect("prepare tables")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query tables")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect tables");
    assert_eq!(
        tables,
        [
            "events",
            "idempotency_records",
            "operation_journal",
            "operations",
            "projects",
            "repositories",
            "repository_package_versions",
        ]
    );
    assert!(MIGRATION_V2.starts_with("BEGIN IMMEDIATE;"));
    assert!(MIGRATION_V2.ends_with("COMMIT;\n"));
    for forbidden in [
        "CREATE TABLE packages",
        "CREATE TABLE dependencies",
        "CREATE TABLE credentials",
        "CREATE TABLE plans",
    ] {
        assert!(!MIGRATION_V2.contains(forbidden));
    }
}

#[test]
fn migration_v2_preserves_events_indexes_and_autoincrement() {
    let connection = migrated_connection();
    insert_operation(&connection, "queued", 1).expect("valid operation");
    connection
        .execute(
            "INSERT INTO events (
                sequence, event_id, kind, aggregate_kind, aggregate_id, aggregate_revision,
                principal_id, occurred_at_ms, payload_json
             ) VALUES (7, '00000000-0000-4000-8000-000000000107', 'operation.queued',
                       'operation', '00000000-0000-4000-8000-000000000001', 1,
                       'builtin:local-owner', 1, '{}')",
            [],
        )
        .expect("insert event with sequence gap");
    connection
        .execute_batch(MIGRATION_V2)
        .expect("apply migration v2");
    assert_eq!(user_version(&connection), 2);
    assert_eq!(
        connection
            .query_row("SELECT sequence FROM events", [], |row| row
                .get::<_, i64>(0))
            .expect("preserved event"),
        7
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT seq FROM sqlite_sequence WHERE name='events'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("event sequence"),
        7
    );
    for index in ["events_principal_sequence", "events_aggregate_sequence"] {
        assert_eq!(
            connection
                .query_row(
                    "SELECT count(*) FROM sqlite_schema WHERE type='index' AND name=?1",
                    [index],
                    |row| row.get::<_, i64>(0),
                )
                .expect("event index"),
            1
        );
    }
    connection
        .execute(
            "INSERT INTO events (
                event_id, kind, aggregate_kind, aggregate_id, aggregate_revision,
                principal_id, occurred_at_ms, payload_json
             ) VALUES ('00000000-0000-4000-8000-000000000108', 'project.registered',
                       'project', '00000000-0000-4000-8000-000000000201', 1,
                       'builtin:local-owner', 2, '{}')",
            [],
        )
        .expect("insert M3 event");
    assert_eq!(
        connection
            .query_row("SELECT max(sequence) FROM events", [], |row| row
                .get::<_, i64>(0))
            .expect("next event sequence"),
        8
    );
}

#[test]
fn migration_v2_allows_completed_sync_idempotency_without_operation() {
    let connection = migrated_v2_connection();
    connection
        .execute(
            "INSERT INTO idempotency_records (
                principal_id, method, idempotency_key, request_fingerprint, state,
                operation_id, response_json, created_at_ms
             ) VALUES ('builtin:local-owner', 'projects.register', 'project-once', '{}',
                       'completed', NULL, '{\"projectId\":\"00000000-0000-4000-8000-000000000201\"}', 1)",
            [],
        )
        .expect("completed synchronous idempotency");
    assert!(
        connection
            .execute(
                "INSERT INTO idempotency_records (
                    principal_id, method, idempotency_key, request_fingerprint, state,
                    operation_id, response_json, created_at_ms
                 ) VALUES ('builtin:local-owner', 'projects.refresh', 'pending-without-operation',
                           '{}', 'pending', NULL, NULL, 1)",
                [],
            )
            .is_err()
    );
}

#[test]
fn failed_migration_v2_restores_complete_v1() {
    let connection = migrated_connection();
    let failing = MIGRATION_V2.replace(
        "PRAGMA user_version = 2;",
        "THIS IS NOT VALID SQL;\nPRAGMA user_version = 2;",
    );
    assert!(connection.execute_batch(&failing).is_err());
    connection
        .execute_batch("ROLLBACK;")
        .expect("rollback failed migration");
    assert_eq!(user_version(&connection), 1);
    let business_tables: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_schema
             WHERE type='table' AND name IN ('projects','repositories','repository_package_versions')",
            [],
            |row| row.get(0),
        )
        .expect("count M3 tables");
    assert_eq!(business_tables, 0);
    connection
        .execute(
            "INSERT INTO events (
                event_id, kind, aggregate_kind, aggregate_id, aggregate_revision,
                principal_id, occurred_at_ms, payload_json
             ) VALUES ('00000000-0000-4000-8000-000000000109', 'project.registered',
                       'project', '00000000-0000-4000-8000-000000000201', 1,
                       'builtin:local-owner', 2, '{}')",
            [],
        )
        .expect_err("v1 must still reject project aggregate");
}

#[test]
fn migration_v3_is_atomic_and_adds_only_the_m4_contract_tables() {
    let connection = migrated_v3_connection();
    assert_eq!(user_version(&connection), 3);
    let tables = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type='table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .expect("prepare tables")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query tables")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect tables");
    assert_eq!(
        tables,
        [
            "events",
            "idempotency_records",
            "operation_journal",
            "operations",
            "package_filesystem_journal",
            "package_plans",
            "projects",
            "repositories",
            "repository_package_versions",
        ]
    );
    assert!(MIGRATION_V3.starts_with("BEGIN IMMEDIATE;"));
    assert!(MIGRATION_V3.ends_with("COMMIT;\n"));
    assert!(!MIGRATION_V3.contains("plan_expired"));
    assert!(!MIGRATION_V3.contains("Packages/manifest.json"));
}

#[test]
fn migration_v3_preserves_v2_rows_and_assigns_deterministic_priority() {
    let connection = migrated_v2_connection();
    insert_project(&connection);
    for (repository_id, registered_at_ms) in [
        ("00000000-0000-4000-8000-000000000302", 20_i64),
        ("00000000-0000-4000-8000-000000000301", 10_i64),
        ("00000000-0000-4000-8000-000000000303", 20_i64),
    ] {
        connection
            .execute(
                "INSERT INTO repositories (
                    repository_id, owner_principal_id, source_kind, source_locator,
                    source_identity_key, issues_json, revision, registered_at_ms,
                    refreshed_at_ms, updated_at_ms
                 ) VALUES (?1, 'builtin:local-owner', 'remote', ?1, CAST(?1 AS BLOB),
                           '[]', 1, ?2, 1, 1)",
                params![repository_id, registered_at_ms],
            )
            .expect("insert repository");
    }
    connection
        .execute(
            "INSERT INTO repository_package_versions (
                repository_id, package_id, version_text, display_name, description,
                yanked, unity_text
             ) VALUES ('00000000-0000-4000-8000-000000000301',
                       'com.example.base', '1.2.3', 'Example Base', NULL, 0, '2022.3')",
            [],
        )
        .expect("insert raw package version");
    insert_operation(&connection, "queued", 1).expect("insert v2 operation");

    connection
        .execute_batch(MIGRATION_V3)
        .expect("apply migration v3");

    let priorities = connection
        .prepare("SELECT repository_id, priority FROM repositories ORDER BY priority")
        .expect("prepare priorities")
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .expect("query priorities")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect priorities");
    assert_eq!(
        priorities,
        [
            ("00000000-0000-4000-8000-000000000301".to_owned(), 1),
            ("00000000-0000-4000-8000-000000000302".to_owned(), 2),
            ("00000000-0000-4000-8000-000000000303".to_owned(), 3),
        ]
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT resolver_ready FROM repository_package_versions",
                [],
                |row| row.get::<_, i64>(0),
            )
            .expect("resolver marker"),
        0
    );
    assert!(
        connection
            .execute(
                "UPDATE repository_package_versions SET resolver_ready=1",
                [],
            )
            .is_err()
    );
    connection
        .execute(
            "UPDATE repository_package_versions SET
                semantic_version='1.2.3', author_name='Fixture Author',
                author_email='fixture@example.invalid',
                artifact_url='https://fixtures.invalid/base.zip',
                zip_sha256='0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
                manifest_fingerprint=zeroblob(32), resolver_ready=1",
            [],
        )
        .expect("complete resolver-ready metadata");
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM operations", [], |row| row
                .get::<_, i64>(0))
            .expect("preserved operation"),
        1
    );
    let foreign_key_errors: i64 = connection
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .expect("foreign key check");
    assert_eq!(foreign_key_errors, 0);
}

#[test]
fn schema_v3_freezes_immutable_plans_and_filesystem_phases() {
    let connection = migrated_v3_connection();
    insert_project(&connection);
    let change_set = r#"{"formatVersion":1,"mutations":[],"dependencyEdges":[]}"#;
    connection
        .execute(
            "INSERT INTO package_plans (
                plan_id, owner_principal_id, project_id, action, state, project_revision,
                project_snapshot_fingerprint, change_set_fingerprint, change_set_json,
                source_set_json, created_at_ms
             ) VALUES (
                '00000000-0000-4000-8000-000000000401', 'builtin:local-owner',
                '00000000-0000-4000-8000-000000000201', 'install', 'unapplied', 1,
                zeroblob(32), zeroblob(32), ?1, '[]', 1
             )",
            [change_set],
        )
        .expect("insert package plan");
    assert!(
        connection
            .execute(
                "UPDATE package_plans SET action='remove'
                 WHERE plan_id='00000000-0000-4000-8000-000000000401'",
                [],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "DELETE FROM package_plans
                 WHERE plan_id='00000000-0000-4000-8000-000000000401'",
                [],
            )
            .is_err()
    );

    connection
        .execute(
            "INSERT INTO operations (
                operation_id, kind, state, revision, owner_principal_id, request_json,
                cancel_requested, created_at_ms, updated_at_ms
             ) VALUES ('00000000-0000-4000-8000-000000000402', 'packages.apply', 'queued',
                       1, 'builtin:local-owner', '{}', 0, 1, 1)",
            [],
        )
        .expect("insert packages.apply operation");
    connection
        .execute(
            "UPDATE package_plans SET state='applied',
                    apply_operation_id='00000000-0000-4000-8000-000000000402'
             WHERE plan_id='00000000-0000-4000-8000-000000000401'",
            [],
        )
        .expect("bind operation once");
    assert!(
        connection
            .execute(
                "UPDATE package_plans SET apply_operation_id=NULL
                 WHERE plan_id='00000000-0000-4000-8000-000000000401'",
                [],
            )
            .is_err()
    );

    for (step, phase) in [
        (1_i64, "accepted"),
        (2, "archive_ready"),
        (3, "extracted"),
        (4, "prepared"),
        (5, "packages_replaced"),
        (6, "vpm_manifest_committed"),
        (7, "filesystem_committed"),
        (8, "state_committed"),
    ] {
        connection
            .execute(
                "INSERT INTO package_filesystem_journal (
                    operation_id, step, plan_id, project_id, phase, state,
                    project_identity_key, change_set_fingerprint, evidence_json, updated_at_ms
                 ) VALUES ('00000000-0000-4000-8000-000000000402', ?1,
                           '00000000-0000-4000-8000-000000000401',
                           '00000000-0000-4000-8000-000000000201', ?2, 'completed',
                           x'01', zeroblob(32), '{}', 1)",
                params![step, phase],
            )
            .expect("insert frozen filesystem phase");
    }
    connection
        .execute(
            "INSERT INTO package_filesystem_journal (
                operation_id, step, plan_id, project_id, phase, state,
                project_identity_key, change_set_fingerprint, evidence_json, updated_at_ms
             ) VALUES ('00000000-0000-4000-8000-000000000402', 9,
                       '00000000-0000-4000-8000-000000000401',
                       '00000000-0000-4000-8000-000000000201', 'archive_ready', 'completed',
                       x'01', zeroblob(32), '{\"attempt\":2}', 2)",
            [],
        )
        .expect("a restarted attempt may repeat an append-only phase");
    assert!(
        connection
            .execute(
                "INSERT INTO package_filesystem_journal (
                    operation_id, step, plan_id, project_id, phase, state,
                    project_identity_key, change_set_fingerprint, evidence_json, updated_at_ms
                 ) VALUES ('00000000-0000-4000-8000-000000000402', 10,
                           '00000000-0000-4000-8000-000000000401',
                           '00000000-0000-4000-8000-000000000201', 'guessed', 'completed',
                           x'01', zeroblob(32), '{}', 1)",
                [],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "UPDATE package_filesystem_journal SET evidence_json='{}'
                 WHERE operation_id='00000000-0000-4000-8000-000000000402' AND step=1",
                [],
            )
            .is_err()
    );
}

#[test]
fn failed_migration_v3_restores_complete_v2() {
    let connection = migrated_v2_connection();
    let failing = MIGRATION_V3.replace(
        "PRAGMA user_version = 3;",
        "THIS IS NOT VALID SQL;\nPRAGMA user_version = 3;",
    );
    assert!(connection.execute_batch(&failing).is_err());
    connection
        .execute_batch("ROLLBACK;")
        .expect("rollback failed migration");
    assert_eq!(user_version(&connection), 2);
    let m4_tables: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_schema
             WHERE type='table' AND name IN ('package_plans','package_filesystem_journal')",
            [],
            |row| row.get(0),
        )
        .expect("count M4 tables");
    assert_eq!(m4_tables, 0);
    assert!(
        connection
            .execute(
                "INSERT INTO operations (
                    operation_id, kind, state, revision, owner_principal_id, request_json,
                    cancel_requested, created_at_ms, updated_at_ms
                 ) VALUES ('00000000-0000-4000-8000-000000000403', 'packages.apply', 'queued',
                           1, 'builtin:local-owner', '{}', 0, 1, 1)",
                [],
            )
            .is_err()
    );
}

#[test]
fn migration_v4_is_atomic_and_adds_only_the_approved_m5_registry_tables() {
    let connection = migrated_v4_connection();
    assert_eq!(user_version(&connection), 4);
    let tables = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type='table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .expect("prepare tables")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query tables")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect tables");
    assert_eq!(
        tables,
        [
            "backups",
            "events",
            "idempotency_records",
            "operation_journal",
            "operations",
            "package_filesystem_journal",
            "package_plans",
            "project_editor_preferences",
            "projects",
            "repositories",
            "repository_package_versions",
            "templates",
            "unity_installations",
        ]
    );
    assert!(MIGRATION_V4.starts_with("BEGIN IMMEDIATE;"));
    assert!(MIGRATION_V4.ends_with("COMMIT;\n"));
    assert!(!MIGRATION_V4.contains("CREATE TABLE settings"));
    assert!(!MIGRATION_V4.contains("process_history"));
    assert!(!MIGRATION_V4.contains("workflow"));
}

#[test]
fn migration_v4_preserves_m2_m3_m4_state_and_event_sequence() {
    let connection = migrated_v3_connection();
    insert_project(&connection);
    connection
        .execute(
            "INSERT INTO events (
                event_id, kind, aggregate_kind, aggregate_id, aggregate_revision,
                principal_id, occurred_at_ms, payload_json
             ) VALUES ('00000000-0000-4000-8000-000000000501', 'project.registered',
                       'project', '00000000-0000-4000-8000-000000000201', 1,
                       'builtin:local-owner', 1, '{}')",
            [],
        )
        .expect("insert M3 event");
    connection
        .execute(
            "INSERT INTO operations (
                operation_id, kind, state, revision, owner_principal_id, request_json,
                cancel_requested, created_at_ms, updated_at_ms
             ) VALUES ('00000000-0000-4000-8000-000000000502', 'packages.apply', 'queued',
                       1, 'builtin:local-owner', '{}', 0, 1, 1)",
            [],
        )
        .expect("insert M4 operation");
    connection
        .execute(
            "INSERT INTO package_plans (
                plan_id, owner_principal_id, project_id, action, state, project_revision,
                project_snapshot_fingerprint, change_set_fingerprint, change_set_json,
                source_set_json, apply_operation_id, created_at_ms
             ) VALUES ('00000000-0000-4000-8000-000000000503', 'builtin:local-owner',
                       '00000000-0000-4000-8000-000000000201', 'install', 'applied', 1,
                       zeroblob(32), zeroblob(32),
                       '{\"formatVersion\":1,\"mutations\":[],\"dependencyEdges\":[]}', '[]',
                       '00000000-0000-4000-8000-000000000502', 1)",
            [],
        )
        .expect("insert M4 package plan");
    connection
        .execute(
            "INSERT INTO package_filesystem_journal (
                operation_id, step, plan_id, project_id, phase, state,
                project_identity_key, change_set_fingerprint, evidence_json, updated_at_ms
             ) VALUES ('00000000-0000-4000-8000-000000000502', 1,
                       '00000000-0000-4000-8000-000000000503',
                       '00000000-0000-4000-8000-000000000201', 'accepted', 'completed',
                       x'01', zeroblob(32), '{}', 1)",
            [],
        )
        .expect("insert M4 filesystem evidence");
    connection
        .execute(
            "INSERT INTO idempotency_records (
                principal_id, method, idempotency_key, request_fingerprint, state,
                operation_id, response_json, created_at_ms
             ) VALUES ('builtin:local-owner', 'packages.applyPlan', 'm5-preserve', '{}',
                       'completed', '00000000-0000-4000-8000-000000000502', '{}', 1)",
            [],
        )
        .expect("insert idempotency row");

    connection
        .execute_batch(MIGRATION_V4)
        .expect("apply migration v4");

    for table in [
        "projects",
        "operations",
        "package_plans",
        "package_filesystem_journal",
        "idempotency_records",
        "events",
    ] {
        let count: i64 = connection
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count preserved row");
        assert_eq!(count, 1, "{table}");
    }
    connection
        .execute(
            "INSERT INTO events (
                event_id, kind, aggregate_kind, aggregate_id, aggregate_revision,
                principal_id, occurred_at_ms, payload_json
             ) VALUES ('00000000-0000-4000-8000-000000000504', 'unity.installation.registered',
                       'unity-installation', '00000000-0000-4000-8000-000000000505', 1,
                       'builtin:local-owner', 2, '{}')",
            [],
        )
        .expect("new M5 aggregate kind is accepted");
    let sequences = connection
        .prepare("SELECT sequence FROM events ORDER BY sequence")
        .expect("prepare sequences")
        .query_map([], |row| row.get::<_, i64>(0))
        .expect("query sequences")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect sequences");
    assert_eq!(sequences, [1, 2]);
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("foreign key check"),
        0
    );
}

#[test]
fn schema_v4_freezes_unity_identity_argv_and_immutable_backup_metadata_shape() {
    let connection = migrated_v4_connection();
    insert_project(&connection);
    connection
        .execute(
            "INSERT INTO unity_installations (
                installation_id, owner_principal_id, executable_path,
                filesystem_identity_key, unity_version, architecture, source_kind,
                revision, observed_at_ms, updated_at_ms
             ) VALUES ('00000000-0000-4000-8000-000000000510', 'builtin:local-owner',
                       'X:/Unity/Editor/Unity.exe', x'010203', '2022.3.22f1', 'x86_64',
                       'manual', 1, 1, 1)",
            [],
        )
        .expect("insert Unity installation");
    assert!(
        connection
            .execute(
                "INSERT INTO unity_installations (
                    installation_id, owner_principal_id, executable_path,
                    filesystem_identity_key, unity_version, architecture, source_kind,
                    revision, observed_at_ms, updated_at_ms
                 ) VALUES ('00000000-0000-4000-8000-000000000511', 'builtin:local-owner',
                           'X:/Alias/Unity.exe', x'010203', '2022.3.22f1', 'x86_64',
                           'manual', 1, 1, 1)",
                [],
            )
            .is_err()
    );
    connection
        .execute(
            "INSERT INTO project_editor_preferences (
                project_id, installation_id, arguments_json, revision, updated_at_ms
             ) VALUES ('00000000-0000-4000-8000-000000000201',
                       '00000000-0000-4000-8000-000000000510', '[\"-batchmode\"]', 1, 1)",
            [],
        )
        .expect("insert argv preference");
    assert!(
        connection
            .execute(
                "DELETE FROM unity_installations
                 WHERE installation_id='00000000-0000-4000-8000-000000000510'",
                [],
            )
            .is_err()
    );
    assert!(
        connection
            .execute(
                "UPDATE project_editor_preferences SET arguments_json='{}'",
                [],
            )
            .is_err()
    );
    connection
        .execute(
            "INSERT INTO templates (
                template_id, owner_principal_id, source_kind, template_version,
                manifest_json, payload_locator, payload_sha256, favorite, revision,
                created_at_ms, updated_at_ms
             ) VALUES ('00000000-0000-4000-8000-000000000512', 'builtin:local-owner',
                       'builtin', '1', '{}', 'objects/template.fixture', zeroblob(32),
                       0, 1, 1, 1)",
            [],
        )
        .expect("insert structural template contract");
    connection
        .execute(
            "INSERT INTO backups (
                backup_id, owner_principal_id, source_project_id, archive_locator,
                file_identity_key, archive_sha256, byte_size, format_version,
                created_at_ms, compression_mode, exclude_vpm_packages
             ) VALUES ('00000000-0000-4000-8000-000000000513', 'builtin:local-owner',
                       '00000000-0000-4000-8000-000000000999', 'backups/fixture.zip',
                       x'01', zeroblob(32), 0, 1, 1, 'fast', 1)",
            [],
        )
        .expect("historical source ProjectId is not a live foreign key");
}

#[test]
fn failed_migration_v4_restores_complete_v3() {
    let connection = migrated_v3_connection();
    insert_project(&connection);
    let failing = MIGRATION_V4.replace(
        "PRAGMA user_version = 4;",
        "THIS IS NOT VALID SQL;\nPRAGMA user_version = 4;",
    );
    assert!(connection.execute_batch(&failing).is_err());
    connection
        .execute_batch("ROLLBACK;")
        .expect("rollback failed migration");
    assert_eq!(user_version(&connection), 3);
    let m5_tables: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_schema
             WHERE type='table' AND name IN (
                'unity_installations','project_editor_preferences','templates','backups'
             )",
            [],
            |row| row.get(0),
        )
        .expect("count M5 tables");
    assert_eq!(m5_tables, 0);
    connection
        .execute(
            "INSERT INTO events (
                event_id, kind, aggregate_kind, aggregate_id, aggregate_revision,
                principal_id, occurred_at_ms, payload_json
             ) VALUES ('00000000-0000-4000-8000-000000000520', 'unity.installation.registered',
                       'unity-installation', '00000000-0000-4000-8000-000000000521', 1,
                       'builtin:local-owner', 1, '{}')",
            [],
        )
        .expect_err("v3 event contract must remain intact after rollback");
}

fn migrated_connection() -> Connection {
    let connection = Connection::open_in_memory().expect("open SQLite");
    connection
        .execute_batch("PRAGMA foreign_keys=ON;")
        .expect("enable foreign keys");
    connection
        .execute_batch(MIGRATION_V1)
        .expect("apply migration");
    connection
}

fn migrated_v2_connection() -> Connection {
    let connection = migrated_connection();
    connection
        .execute_batch(MIGRATION_V2)
        .expect("apply migration v2");
    connection
}

fn migrated_v3_connection() -> Connection {
    let connection = migrated_v2_connection();
    connection
        .execute_batch(MIGRATION_V3)
        .expect("apply migration v3");
    connection
}

fn migrated_v4_connection() -> Connection {
    let connection = migrated_v3_connection();
    connection
        .execute_batch(MIGRATION_V4)
        .expect("apply migration v4");
    connection
}

fn insert_project(connection: &Connection) {
    connection
        .execute(
            "INSERT INTO projects (
                project_id, owner_principal_id, root_path, path_identity_key, project_type,
                unity_version, snapshot_json, revision, registered_at_ms, observed_at_ms,
                updated_at_ms
             ) VALUES ('00000000-0000-4000-8000-000000000201', 'builtin:local-owner',
                       'X:/fixture', x'01', 'unknown', '2022.3.22f1', '{}', 1, 1, 1, 1)",
            [],
        )
        .expect("insert project");
}

fn insert_operation(
    connection: &Connection,
    state: &str,
    revision: i64,
) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO operations \
         (operation_id, kind, state, revision, owner_principal_id, request_json, \
          cancel_requested, created_at_ms, updated_at_ms) \
         VALUES (?1, 'state.check', ?2, ?3, 'builtin:local-owner', '{}', 0, 1, 1)",
        params![
            if state == "queued" {
                "00000000-0000-4000-8000-000000000001"
            } else {
                "00000000-0000-4000-8000-000000000002"
            },
            state,
            revision
        ],
    )
}

fn user_version(connection: &Connection) -> i64 {
    pragma_i64(connection, "PRAGMA user_version")
}

fn pragma_i64(connection: &Connection, pragma: &str) -> i64 {
    connection
        .query_row(pragma, [], |row| row.get(0))
        .expect("read pragma")
}
