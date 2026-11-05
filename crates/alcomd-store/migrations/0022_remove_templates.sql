-- The Store owns the immediate transaction and suspends FK enforcement while
-- replacing the operations table. It checks all references before committing.
CREATE TEMP TABLE m22_legacy_operations (operation_id TEXT PRIMARY KEY);
INSERT INTO m22_legacy_operations
SELECT operation_id FROM operations
WHERE kind IN ('templates.import', 'templates.derive', 'templates.create-project');

DELETE FROM operation_log_entries
WHERE operation_id IN (SELECT operation_id FROM m22_legacy_operations)
   OR action GLOB 'templates.*';

DELETE FROM idempotency_records
WHERE operation_id IN (SELECT operation_id FROM m22_legacy_operations)
   OR method IN ('templates.applyPlan', 'templates.setFavorite', 'templates.remove');

DELETE FROM operation_journal
WHERE operation_id IN (SELECT operation_id FROM m22_legacy_operations);

DROP TABLE template_plans;
DROP TABLE templates;

CREATE TEMP TABLE m22_operations AS
SELECT rowid AS original_rowid, * FROM operations
WHERE operation_id NOT IN (SELECT operation_id FROM m22_legacy_operations);

DROP TABLE operations;
CREATE TABLE operations (
    operation_id TEXT PRIMARY KEY CHECK (length(operation_id) = 36),
    kind TEXT NOT NULL CHECK (kind IN (
        'state.check', 'packages.apply',
        'backups.create', 'backups.restore',
        'extensions.install', 'extensions.uninstall',
        'projects.create', 'projects.copy', 'projects.delete-directory', 'projects.unity-migration'
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

INSERT INTO operations (
    rowid, operation_id, kind, state, revision, owner_principal_id, request_json,
    result_json, error_code, diagnostic_id, cancel_requested, created_at_ms,
    updated_at_ms, started_at_ms, completed_at_ms
) SELECT * FROM m22_operations;
DROP TABLE m22_operations;

CREATE INDEX operations_owner_page
    ON operations(owner_principal_id, created_at_ms DESC, operation_id DESC);
CREATE INDEX operations_recovery
    ON operations(state, created_at_ms ASC, operation_id ASC);

CREATE TEMP TABLE m22_event_sequence AS SELECT seq FROM sqlite_sequence WHERE name='events';
DROP INDEX events_principal_sequence;
DROP INDEX events_aggregate_sequence;
ALTER TABLE events RENAME TO events_before_template_removal;

CREATE TABLE events (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT
        CHECK (sequence BETWEEN 1 AND 9223372036854775807),
    event_id TEXT NOT NULL UNIQUE CHECK (length(event_id) = 36),
    kind TEXT NOT NULL CHECK (length(kind) BETWEEN 1 AND 128),
    aggregate_kind TEXT NOT NULL CHECK (aggregate_kind IN (
        'operation', 'project', 'repository', 'unity-installation',
        'project-editor-preference', 'project-unity-launch-config', 'user-package'
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

INSERT INTO events SELECT * FROM events_before_template_removal
WHERE aggregate_kind <> 'template'
  AND (aggregate_kind <> 'operation'
       OR aggregate_id NOT IN (SELECT operation_id FROM m22_legacy_operations))
ORDER BY sequence;
DROP TABLE events_before_template_removal;
DROP TABLE m22_legacy_operations;

INSERT INTO sqlite_sequence(name,seq)
    SELECT 'events',0 WHERE NOT EXISTS(SELECT 1 FROM sqlite_sequence WHERE name='events');
UPDATE sqlite_sequence
SET seq=max(seq, coalesce((SELECT seq FROM m22_event_sequence),0))
WHERE name='events';
DROP TABLE m22_event_sequence;

CREATE INDEX events_principal_sequence ON events(principal_id,sequence);
CREATE INDEX events_aggregate_sequence ON events(aggregate_kind,aggregate_id,sequence);

PRAGMA user_version = 22;
