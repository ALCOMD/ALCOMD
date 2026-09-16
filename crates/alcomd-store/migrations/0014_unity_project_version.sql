BEGIN IMMEDIATE;

-- Rebuild the Operation kind constraint while preserving every durable child row.
CREATE TEMP TABLE m14_operations AS SELECT * FROM operations;
CREATE TEMP TABLE m14_operation_journal AS SELECT * FROM operation_journal;
CREATE TEMP TABLE m14_package_filesystem_journal AS SELECT * FROM package_filesystem_journal;
CREATE TEMP TABLE m14_backup_restore_filesystem_journal AS
    SELECT * FROM backup_restore_filesystem_journal;
CREATE TEMP TABLE m14_extension_filesystem_journal AS SELECT * FROM extension_filesystem_journal;
CREATE TEMP TABLE m14_project_copy_filesystem_journal AS
    SELECT * FROM project_copy_filesystem_journal;
CREATE TEMP TABLE m14_project_delete_filesystem_journal AS
    SELECT * FROM project_delete_filesystem_journal;

DROP TRIGGER package_filesystem_journal_no_delete;
DROP TRIGGER backup_restore_filesystem_journal_no_delete;
DROP TRIGGER extension_filesystem_journal_no_delete;
DROP TRIGGER project_copy_filesystem_journal_no_delete;
DROP TRIGGER project_delete_filesystem_journal_no_delete;

PRAGMA defer_foreign_keys = ON;
DROP TABLE operations;

CREATE TABLE operations (
    operation_id TEXT PRIMARY KEY CHECK (length(operation_id) = 36),
    kind TEXT NOT NULL CHECK (kind IN (
        'state.check', 'packages.apply',
        'templates.import', 'templates.derive', 'templates.create-project',
        'backups.create', 'backups.restore',
        'extensions.install', 'extensions.uninstall',
        'projects.copy', 'projects.delete-directory', 'projects.unity-migration'
    )),
    state TEXT NOT NULL CHECK (state IN (
        'queued', 'planning', 'waiting_for_input', 'running', 'cancelling',
        'succeeded', 'failed', 'cancelled', 'interrupted', 'recovering'
    )),
    revision INTEGER NOT NULL CHECK (revision BETWEEN 1 AND 9223372036854775807),
    owner_principal_id TEXT NOT NULL CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    request_json TEXT NOT NULL CHECK (json_valid(request_json) AND length(request_json) <= 65536),
    result_json TEXT CHECK (
        result_json IS NULL OR (json_valid(result_json) AND length(result_json) <= 65536)
    ),
    error_code TEXT CHECK (error_code IS NULL OR length(error_code) BETWEEN 1 AND 128),
    diagnostic_id TEXT CHECK (diagnostic_id IS NULL OR length(diagnostic_id) = 36),
    cancel_requested INTEGER NOT NULL DEFAULT 0 CHECK (cancel_requested IN (0, 1)),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN 0 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    started_at_ms INTEGER CHECK (
        started_at_ms IS NULL OR started_at_ms BETWEEN 0 AND 9223372036854775807
    ),
    completed_at_ms INTEGER CHECK (
        completed_at_ms IS NULL OR completed_at_ms BETWEEN 0 AND 9223372036854775807
    )
) STRICT;

INSERT INTO operations SELECT * FROM m14_operations;

CREATE INDEX operations_owner_page
    ON operations(owner_principal_id, created_at_ms DESC, operation_id DESC);
CREATE INDEX operations_recovery
    ON operations(state, created_at_ms ASC, operation_id ASC);

INSERT INTO operation_journal SELECT * FROM m14_operation_journal;
INSERT INTO package_filesystem_journal SELECT * FROM m14_package_filesystem_journal;
INSERT INTO backup_restore_filesystem_journal
    SELECT * FROM m14_backup_restore_filesystem_journal;
INSERT INTO extension_filesystem_journal SELECT * FROM m14_extension_filesystem_journal;
INSERT INTO project_copy_filesystem_journal
    SELECT * FROM m14_project_copy_filesystem_journal;
INSERT INTO project_delete_filesystem_journal
    SELECT * FROM m14_project_delete_filesystem_journal;

DROP TABLE m14_operation_journal;
DROP TABLE m14_package_filesystem_journal;
DROP TABLE m14_backup_restore_filesystem_journal;
DROP TABLE m14_extension_filesystem_journal;
DROP TABLE m14_project_copy_filesystem_journal;
DROP TABLE m14_project_delete_filesystem_journal;
DROP TABLE m14_operations;

CREATE TRIGGER package_filesystem_journal_no_delete
BEFORE DELETE ON package_filesystem_journal
BEGIN
    SELECT RAISE(ABORT, 'filesystem journal is durable');
END;

CREATE TRIGGER backup_restore_filesystem_journal_no_delete
BEFORE DELETE ON backup_restore_filesystem_journal
BEGIN
    SELECT RAISE(ABORT, 'backup restore journal is durable');
END;

CREATE TRIGGER extension_filesystem_journal_no_delete
BEFORE DELETE ON extension_filesystem_journal
BEGIN
    SELECT RAISE(ABORT, 'extension filesystem journal is durable');
END;

CREATE TRIGGER project_copy_filesystem_journal_no_delete
BEFORE DELETE ON project_copy_filesystem_journal
BEGIN
    SELECT RAISE(ABORT, 'project copy journal is durable');
END;

CREATE TRIGGER project_delete_filesystem_journal_no_delete
BEFORE DELETE ON project_delete_filesystem_journal
BEGIN
    SELECT RAISE(ABORT, 'project delete journal is durable');
END;

CREATE TEMP TABLE m7_unity_launch_config_guard (
    value INTEGER NOT NULL CHECK (value = 1)
);

INSERT INTO m7_unity_launch_config_guard (value)
SELECT CASE WHEN EXISTS (
    SELECT 1
    FROM project_editor_preferences AS preference
    WHERE NOT json_valid(preference.arguments_json)
       OR json_type(preference.arguments_json) <> 'array'
       OR json_array_length(preference.arguments_json) > 64
       OR length(preference.arguments_json) > 65536
       OR EXISTS (
            SELECT 1
            FROM json_each(preference.arguments_json) AS argument
            WHERE argument.type <> 'text'
               OR length(CAST(argument.value AS BLOB)) > 4096
       )
) THEN 0 ELSE 1 END;

DROP TABLE m7_unity_launch_config_guard;

CREATE TABLE project_unity_launch_config (
    project_id TEXT PRIMARY KEY
        REFERENCES projects(project_id) ON DELETE CASCADE,
    arguments_json TEXT NOT NULL
        CHECK (
            json_valid(arguments_json)
            AND json_type(arguments_json) = 'array'
            AND json_array_length(arguments_json) BETWEEN 1 AND 64
            AND length(arguments_json) <= 65536
        ),
    revision INTEGER NOT NULL
        CHECK (revision BETWEEN 1 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL
        CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807)
) STRICT;

INSERT INTO project_unity_launch_config (
    project_id, arguments_json, revision, updated_at_ms
)
SELECT project_id, arguments_json, revision, updated_at_ms
FROM project_editor_preferences
WHERE json_array_length(arguments_json) > 0;

CREATE TABLE project_unity_migration_plans (
    plan_id TEXT PRIMARY KEY CHECK (length(plan_id) = 36),
    owner_principal_id TEXT NOT NULL
        CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    project_id TEXT NOT NULL
        REFERENCES projects(project_id) ON DELETE RESTRICT,
    project_revision INTEGER NOT NULL
        CHECK (project_revision BETWEEN 1 AND 9223372036854775807),
    source_unity_version TEXT NOT NULL
        CHECK (length(source_unity_version) BETWEEN 8 AND 64),
    source_revision_metadata TEXT
        CHECK (source_revision_metadata IS NULL OR length(source_revision_metadata) BETWEEN 1 AND 128),
    project_root_identity BLOB NOT NULL
        CHECK (length(project_root_identity) BETWEEN 1 AND 128),
    project_snapshot_json TEXT NOT NULL
        CHECK (
            json_valid(project_snapshot_json)
            AND json_type(project_snapshot_json) = 'object'
            AND length(project_snapshot_json) BETWEEN 2 AND 65536
        ),
    project_version_marker_sha256 BLOB NOT NULL
        CHECK (length(project_version_marker_sha256) = 32),
    target_unity_version TEXT NOT NULL
        CHECK (length(target_unity_version) BETWEEN 8 AND 64),
    target_revision_metadata TEXT
        CHECK (target_revision_metadata IS NULL OR length(target_revision_metadata) BETWEEN 1 AND 128),
    target_installation_id TEXT NOT NULL
        REFERENCES unity_installations(installation_id) ON DELETE RESTRICT,
    target_installation_revision INTEGER NOT NULL
        CHECK (target_installation_revision BETWEEN 1 AND 9223372036854775807),
    target_installation_identity BLOB NOT NULL
        CHECK (length(target_installation_identity) BETWEEN 1 AND 128),
    target_installation_snapshot_json TEXT NOT NULL
        CHECK (
            json_valid(target_installation_snapshot_json)
            AND json_type(target_installation_snapshot_json) = 'object'
            AND length(target_installation_snapshot_json) BETWEEN 2 AND 65536
        ),
    writer_evidence_revision INTEGER NOT NULL
        CHECK (writer_evidence_revision BETWEEN 1 AND 9223372036854775807),
    writer_evidence_json TEXT NOT NULL
        CHECK (
            json_valid(writer_evidence_json)
            AND json_type(writer_evidence_json) = 'object'
            AND length(writer_evidence_json) BETWEEN 2 AND 65536
        ),
    classification TEXT NOT NULL
        CHECK (classification IN (
            'patch_or_minor_upgrade', 'major_upgrade',
            'patch_or_minor_downgrade', 'major_downgrade', 'china_variant_change'
        )),
    preparation_profile TEXT
        CHECK (preparation_profile IS NULL OR preparation_profile = 'vrchat-2019-to-2022-v1'),
    plan_fingerprint TEXT NOT NULL CHECK (length(plan_fingerprint) BETWEEN 1 AND 65536),
    request_fingerprint TEXT NOT NULL CHECK (length(request_fingerprint) BETWEEN 1 AND 65536),
    plan_idempotency_key TEXT NOT NULL CHECK (length(plan_idempotency_key) BETWEEN 1 AND 128),
    created_at_ms INTEGER NOT NULL
        CHECK (created_at_ms BETWEEN 0 AND 9223372036854775807),
    expires_at_ms INTEGER NOT NULL
        CHECK (expires_at_ms BETWEEN 0 AND 9223372036854775807),
    state TEXT NOT NULL CHECK (state IN ('unapplied', 'applied')),
    operation_id TEXT UNIQUE
        REFERENCES operations(operation_id) ON DELETE RESTRICT,
    CHECK (
        (state = 'unapplied' AND operation_id IS NULL)
        OR (state = 'applied' AND operation_id IS NOT NULL)
    )
) STRICT;

CREATE INDEX project_unity_migration_plans_owner_project
    ON project_unity_migration_plans(owner_principal_id, project_id, created_at_ms DESC);

CREATE TABLE project_unity_migration_journal (
    operation_id TEXT NOT NULL
        REFERENCES operations(operation_id) ON DELETE CASCADE,
    step INTEGER NOT NULL CHECK (step BETWEEN 1 AND 9223372036854775807),
    phase TEXT NOT NULL CHECK (phase IN (
        'accepted', 'preflight_complete', 'preparation_intent', 'preparation_complete',
        'launch_intent', 'unity_started', 'unity_exited', 'project_reobserved',
        'state_committed', 'cleanup_complete', 'recovery_required'
    )),
    evidence_json TEXT NOT NULL
        CHECK (
            json_valid(evidence_json)
            AND json_type(evidence_json) = 'object'
            AND length(evidence_json) <= 65536
        ),
    updated_at_ms INTEGER NOT NULL
        CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    PRIMARY KEY (operation_id, step)
) STRICT;

CREATE TRIGGER project_unity_migration_journal_no_update
BEFORE UPDATE ON project_unity_migration_journal
BEGIN
    SELECT RAISE(ABORT, 'project unity migration journal is append-only');
END;

DELETE FROM idempotency_records
WHERE method IN ('unity.projectEditor.set', 'unity.projectEditor.clear');

DROP INDEX project_editor_preferences_installation;
DROP TABLE project_editor_preferences;

CREATE TEMP TABLE m14_events_sequence_backup (
    sequence_value INTEGER NOT NULL
);

INSERT INTO m14_events_sequence_backup (sequence_value)
SELECT coalesce((SELECT seq FROM sqlite_sequence WHERE name = 'events'), 0);

DROP INDEX events_principal_sequence;
DROP INDEX events_aggregate_sequence;
ALTER TABLE events RENAME TO events_v13;

CREATE TABLE events (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT
        CHECK (sequence BETWEEN 1 AND 9223372036854775807),
    event_id TEXT NOT NULL UNIQUE CHECK (length(event_id) = 36),
    kind TEXT NOT NULL CHECK (length(kind) BETWEEN 1 AND 128),
    aggregate_kind TEXT NOT NULL CHECK (aggregate_kind IN (
        'operation', 'project', 'repository', 'unity-installation',
        'project-editor-preference', 'project-unity-launch-config',
        'template', 'user-package'
    )),
    aggregate_id TEXT NOT NULL CHECK (length(aggregate_id) = 36),
    aggregate_revision INTEGER NOT NULL
        CHECK (aggregate_revision BETWEEN 1 AND 9223372036854775807),
    principal_id TEXT NOT NULL CHECK (length(principal_id) BETWEEN 1 AND 128),
    occurred_at_ms INTEGER NOT NULL
        CHECK (occurred_at_ms BETWEEN 0 AND 9223372036854775807),
    payload_json TEXT NOT NULL
        CHECK (json_valid(payload_json) AND length(payload_json) <= 65536)
) STRICT;

INSERT INTO events (
    sequence, event_id, kind, aggregate_kind, aggregate_id, aggregate_revision,
    principal_id, occurred_at_ms, payload_json
)
SELECT sequence, event_id, kind, aggregate_kind, aggregate_id, aggregate_revision,
       principal_id, occurred_at_ms, payload_json
FROM events_v13 ORDER BY sequence;

DROP TABLE events_v13;

UPDATE sqlite_sequence
SET seq = (SELECT sequence_value FROM m14_events_sequence_backup)
WHERE name = 'events' AND EXISTS (SELECT 1 FROM m14_events_sequence_backup);

DROP TABLE m14_events_sequence_backup;

CREATE INDEX events_principal_sequence
    ON events(principal_id, sequence ASC);
CREATE INDEX events_aggregate_sequence
    ON events(aggregate_kind, aggregate_id, sequence ASC);

PRAGMA user_version = 14;

COMMIT;
