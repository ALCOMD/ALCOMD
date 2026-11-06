-- The Store owns the immediate transaction and verifies foreign keys before commit.
DELETE FROM events WHERE aggregate_kind = 'project-template';
DELETE FROM operation_journal
WHERE kind = 'projects.create.declaration'
  AND operation_id IN (
      SELECT operation_id FROM operations
      WHERE kind = 'projects.create' AND state IN ('succeeded', 'failed', 'cancelled')
  );

DROP TABLE project_templates;
CREATE TABLE project_templates (
    owner_principal_id TEXT NOT NULL CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    template_id TEXT NOT NULL CHECK (length(CAST(template_id AS BLOB)) BETWEEN 1 AND 128),
    aggregate_id TEXT NOT NULL UNIQUE CHECK (length(aggregate_id) = 36),
    declaration_json TEXT NOT NULL CHECK (
        length(CAST(declaration_json AS BLOB)) <= 24576
        AND json_valid(declaration_json)
        AND json_type(declaration_json) IS 'object'
        AND json_type(declaration_json, '$.id') IS 'text'
        AND json_extract(declaration_json, '$.id') = template_id
        AND json_type(declaration_json, '$.name') IS 'text'
        AND json_type(declaration_json, '$.archiveSha256') IS 'text'
        AND length(json_extract(declaration_json, '$.archiveSha256')) = 64
        AND json_extract(declaration_json, '$.archiveSha256') NOT GLOB '*[^0-9a-f]*'
        AND json_type(declaration_json, '$.dependencies') IS 'array'
        AND json_type(declaration_json, '$.lockedVersions') IS 'object'
        AND (json_type(declaration_json, '$.editorVersion') IS NULL
             OR json_type(declaration_json, '$.editorVersion') IN ('text', 'null'))
    ),
    revision INTEGER NOT NULL CHECK (revision BETWEEN 1 AND 9007199254740991),
    locked INTEGER NOT NULL CHECK (locked IN (0, 1)),
    PRIMARY KEY (owner_principal_id, template_id)
) STRICT;

CREATE TEMP TABLE m28_operations AS
SELECT rowid AS original_rowid, * FROM operations;
DROP TABLE operations;
CREATE TABLE operations (
    operation_id TEXT PRIMARY KEY CHECK (length(operation_id) = 36),
    kind TEXT NOT NULL CHECK (kind IN (
        'state.check', 'packages.apply',
        'backups.create', 'backups.restore',
        'extensions.install', 'extensions.uninstall',
        'projects.create', 'projects.copy', 'projects.delete-directory', 'projects.unity-migration',
        'projectTemplates.save', 'projectTemplates.export'
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
) SELECT * FROM m28_operations;
DROP TABLE m28_operations;

CREATE INDEX operations_owner_page
    ON operations(owner_principal_id, created_at_ms DESC, operation_id DESC);
CREATE INDEX operations_recovery
    ON operations(state, created_at_ms ASC, operation_id ASC);

PRAGMA user_version = 28;
