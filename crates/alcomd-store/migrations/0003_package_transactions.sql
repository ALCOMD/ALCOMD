BEGIN IMMEDIATE;

DROP INDEX operations_owner_page;
DROP INDEX operations_recovery;
ALTER TABLE operation_journal RENAME TO operation_journal_v2;
ALTER TABLE idempotency_records RENAME TO idempotency_records_v2;
ALTER TABLE operations RENAME TO operations_v2;

CREATE TABLE operations (
    operation_id TEXT PRIMARY KEY
        CHECK (length(operation_id) = 36),
    kind TEXT NOT NULL
        CHECK (kind IN ('state.check', 'packages.apply')),
    state TEXT NOT NULL
        CHECK (state IN (
            'queued', 'planning', 'waiting_for_input', 'running', 'cancelling',
            'succeeded', 'failed', 'cancelled', 'interrupted', 'recovering'
        )),
    revision INTEGER NOT NULL
        CHECK (revision BETWEEN 1 AND 9223372036854775807),
    owner_principal_id TEXT NOT NULL
        CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    request_json TEXT NOT NULL
        CHECK (json_valid(request_json) AND length(request_json) <= 65536),
    result_json TEXT
        CHECK (result_json IS NULL OR (json_valid(result_json) AND length(result_json) <= 65536)),
    error_code TEXT
        CHECK (error_code IS NULL OR length(error_code) BETWEEN 1 AND 128),
    diagnostic_id TEXT
        CHECK (diagnostic_id IS NULL OR length(diagnostic_id) = 36),
    cancel_requested INTEGER NOT NULL DEFAULT 0
        CHECK (cancel_requested IN (0, 1)),
    created_at_ms INTEGER NOT NULL
        CHECK (created_at_ms BETWEEN 0 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL
        CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    started_at_ms INTEGER
        CHECK (started_at_ms IS NULL OR started_at_ms BETWEEN 0 AND 9223372036854775807),
    completed_at_ms INTEGER
        CHECK (completed_at_ms IS NULL OR completed_at_ms BETWEEN 0 AND 9223372036854775807)
) STRICT;

INSERT INTO operations
SELECT * FROM operations_v2;

CREATE INDEX operations_owner_page
    ON operations(owner_principal_id, created_at_ms DESC, operation_id DESC);
CREATE INDEX operations_recovery
    ON operations(state, created_at_ms ASC, operation_id ASC);

CREATE TABLE operation_journal (
    operation_id TEXT NOT NULL
        REFERENCES operations(operation_id) ON DELETE CASCADE,
    step INTEGER NOT NULL
        CHECK (step BETWEEN 1 AND 9223372036854775807),
    kind TEXT NOT NULL
        CHECK (length(kind) BETWEEN 1 AND 128),
    state TEXT NOT NULL
        CHECK (state IN ('prepared', 'applied')),
    payload_json TEXT NOT NULL
        CHECK (json_valid(payload_json) AND length(payload_json) <= 65536),
    updated_at_ms INTEGER NOT NULL
        CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    PRIMARY KEY (operation_id, step)
) STRICT;

INSERT INTO operation_journal
SELECT * FROM operation_journal_v2;

CREATE TABLE idempotency_records (
    principal_id TEXT NOT NULL
        CHECK (length(principal_id) BETWEEN 1 AND 128),
    method TEXT NOT NULL
        CHECK (length(method) BETWEEN 3 AND 128),
    idempotency_key TEXT NOT NULL
        CHECK (length(idempotency_key) BETWEEN 1 AND 128),
    request_fingerprint TEXT NOT NULL
        CHECK (json_valid(request_fingerprint) AND length(request_fingerprint) <= 4096),
    state TEXT NOT NULL
        CHECK (state IN ('pending', 'completed')),
    operation_id TEXT
        REFERENCES operations(operation_id),
    response_json TEXT
        CHECK (response_json IS NULL OR (json_valid(response_json) AND length(response_json) <= 65536)),
    created_at_ms INTEGER NOT NULL
        CHECK (created_at_ms BETWEEN 0 AND 9223372036854775807),
    PRIMARY KEY (principal_id, method, idempotency_key),
    CHECK (
        (state = 'pending' AND operation_id IS NOT NULL AND response_json IS NULL)
        OR (state = 'completed' AND response_json IS NOT NULL)
    )
) STRICT;

INSERT INTO idempotency_records
SELECT * FROM idempotency_records_v2;

DROP TABLE operation_journal_v2;
DROP TABLE idempotency_records_v2;
DROP TABLE operations_v2;

ALTER TABLE repositories ADD COLUMN priority INTEGER NOT NULL DEFAULT 1
    CHECK (priority BETWEEN 1 AND 9223372036854775807);

WITH ordered AS (
    SELECT repository_id,
           row_number() OVER (
               PARTITION BY owner_principal_id
               ORDER BY registered_at_ms ASC, repository_id ASC
           ) AS assigned_priority
    FROM repositories
)
UPDATE repositories
SET priority = (
    SELECT assigned_priority
    FROM ordered
    WHERE ordered.repository_id = repositories.repository_id
);

ALTER TABLE repository_package_versions ADD COLUMN semantic_version TEXT
    CHECK (semantic_version IS NULL OR length(semantic_version) BETWEEN 1 AND 1024);
ALTER TABLE repository_package_versions ADD COLUMN author_name TEXT
    CHECK (author_name IS NULL OR length(author_name) BETWEEN 1 AND 4096);
ALTER TABLE repository_package_versions ADD COLUMN author_email TEXT
    CHECK (author_email IS NULL OR length(author_email) BETWEEN 1 AND 4096);
ALTER TABLE repository_package_versions ADD COLUMN artifact_url TEXT
    CHECK (artifact_url IS NULL OR length(artifact_url) BETWEEN 1 AND 32768);
ALTER TABLE repository_package_versions ADD COLUMN zip_sha256 TEXT
    CHECK (zip_sha256 IS NULL OR (
        length(zip_sha256) = 64
        AND zip_sha256 = lower(zip_sha256)
        AND zip_sha256 NOT GLOB '*[^0-9a-f]*'
    ));
ALTER TABLE repository_package_versions ADD COLUMN unity_release_text TEXT
    CHECK (unity_release_text IS NULL OR length(unity_release_text) BETWEEN 1 AND 1024);
ALTER TABLE repository_package_versions ADD COLUMN dependencies_json TEXT NOT NULL DEFAULT '{}'
    CHECK (json_valid(dependencies_json) AND json_type(dependencies_json) = 'object'
           AND length(dependencies_json) <= 1048576);
ALTER TABLE repository_package_versions ADD COLUMN manifest_fingerprint BLOB
    CHECK (manifest_fingerprint IS NULL OR length(manifest_fingerprint) = 32);
ALTER TABLE repository_package_versions ADD COLUMN legacy_metadata_present INTEGER NOT NULL DEFAULT 0
    CHECK (legacy_metadata_present IN (0, 1));
ALTER TABLE repository_package_versions ADD COLUMN resolver_ready INTEGER NOT NULL DEFAULT 0
    CHECK (resolver_ready IN (0, 1));

CREATE TRIGGER repository_package_versions_resolver_ready_insert
BEFORE INSERT ON repository_package_versions
WHEN NEW.resolver_ready = 1 AND (
    NEW.semantic_version IS NULL
    OR NEW.display_name IS NULL
    OR NEW.author_name IS NULL
    OR NEW.author_email IS NULL
    OR NEW.artifact_url IS NULL
    OR NEW.zip_sha256 IS NULL
    OR NEW.manifest_fingerprint IS NULL
)
BEGIN
    SELECT RAISE(ABORT, 'resolver-ready package metadata is incomplete');
END;

CREATE TRIGGER repository_package_versions_resolver_ready_update
BEFORE UPDATE ON repository_package_versions
WHEN NEW.resolver_ready = 1 AND (
    NEW.semantic_version IS NULL
    OR NEW.display_name IS NULL
    OR NEW.author_name IS NULL
    OR NEW.author_email IS NULL
    OR NEW.artifact_url IS NULL
    OR NEW.zip_sha256 IS NULL
    OR NEW.manifest_fingerprint IS NULL
)
BEGIN
    SELECT RAISE(ABORT, 'resolver-ready package metadata is incomplete');
END;

CREATE TABLE package_plans (
    plan_id TEXT PRIMARY KEY
        CHECK (length(plan_id) = 36),
    owner_principal_id TEXT NOT NULL
        CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    project_id TEXT NOT NULL
        REFERENCES projects(project_id),
    action TEXT NOT NULL
        CHECK (action IN ('install', 'remove', 'upgrade', 'downgrade', 'resolve')),
    state TEXT NOT NULL
        CHECK (state IN ('unapplied', 'applied')),
    project_revision INTEGER NOT NULL
        CHECK (project_revision BETWEEN 1 AND 9223372036854775807),
    project_snapshot_fingerprint BLOB NOT NULL
        CHECK (length(project_snapshot_fingerprint) = 32),
    change_set_fingerprint BLOB NOT NULL
        CHECK (length(change_set_fingerprint) = 32),
    change_set_json TEXT NOT NULL
        CHECK (
            json_valid(change_set_json)
            AND json_type(change_set_json) = 'object'
            AND length(change_set_json) <= 4194304
            AND json_array_length(change_set_json, '$.mutations') <= 1024
            AND json_array_length(change_set_json, '$.dependencyEdges') <= 4096
        ),
    source_set_json TEXT NOT NULL
        CHECK (json_valid(source_set_json) AND json_type(source_set_json) = 'array'
               AND length(source_set_json) <= 4194304),
    apply_operation_id TEXT UNIQUE
        REFERENCES operations(operation_id),
    created_at_ms INTEGER NOT NULL
        CHECK (created_at_ms BETWEEN 0 AND 9223372036854775807),
    CHECK (
        (state = 'unapplied' AND apply_operation_id IS NULL)
        OR (state = 'applied' AND apply_operation_id IS NOT NULL)
    )
) STRICT;

CREATE INDEX package_plans_owner_created
    ON package_plans(owner_principal_id, created_at_ms DESC, plan_id DESC);
CREATE INDEX package_plans_project_created
    ON package_plans(project_id, created_at_ms DESC, plan_id DESC);

CREATE TRIGGER package_plans_immutable
BEFORE UPDATE ON package_plans
WHEN NOT (
    OLD.state = 'unapplied'
    AND NEW.state = 'applied'
    AND OLD.apply_operation_id IS NULL
    AND NEW.apply_operation_id IS NOT NULL
    AND OLD.plan_id = NEW.plan_id
    AND OLD.owner_principal_id = NEW.owner_principal_id
    AND OLD.project_id = NEW.project_id
    AND OLD.action = NEW.action
    AND OLD.project_revision = NEW.project_revision
    AND OLD.project_snapshot_fingerprint = NEW.project_snapshot_fingerprint
    AND OLD.change_set_fingerprint = NEW.change_set_fingerprint
    AND OLD.change_set_json = NEW.change_set_json
    AND OLD.source_set_json = NEW.source_set_json
    AND OLD.created_at_ms = NEW.created_at_ms
    AND (SELECT kind FROM operations WHERE operation_id = NEW.apply_operation_id) = 'packages.apply'
)
BEGIN
    SELECT RAISE(ABORT, 'package plan is immutable');
END;

CREATE TRIGGER package_plans_no_delete
BEFORE DELETE ON package_plans
BEGIN
    SELECT RAISE(ABORT, 'package plans are durable');
END;

CREATE TABLE package_filesystem_journal (
    operation_id TEXT NOT NULL
        REFERENCES operations(operation_id) ON DELETE CASCADE,
    step INTEGER NOT NULL
        CHECK (step BETWEEN 1 AND 9223372036854775807),
    plan_id TEXT NOT NULL
        REFERENCES package_plans(plan_id),
    project_id TEXT NOT NULL
        REFERENCES projects(project_id),
    phase TEXT NOT NULL
        CHECK (phase IN (
            'accepted', 'archive_ready', 'extracted', 'prepared',
            'packages_replaced', 'vpm_manifest_committed', 'filesystem_committed',
            'state_committed', 'rolling_back', 'rolled_back', 'recovery_required'
        )),
    state TEXT NOT NULL
        CHECK (state IN ('intent', 'completed')),
    project_identity_key BLOB NOT NULL
        CHECK (length(project_identity_key) BETWEEN 1 AND 128),
    change_set_fingerprint BLOB NOT NULL
        CHECK (length(change_set_fingerprint) = 32),
    evidence_json TEXT NOT NULL
        CHECK (json_valid(evidence_json) AND json_type(evidence_json) = 'object'
               AND length(evidence_json) <= 4194304),
    updated_at_ms INTEGER NOT NULL
        CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    PRIMARY KEY (operation_id, step)
) STRICT;

CREATE INDEX package_filesystem_journal_recovery
    ON package_filesystem_journal(operation_id, step DESC);

CREATE TRIGGER package_filesystem_journal_validate_insert
BEFORE INSERT ON package_filesystem_journal
WHEN (SELECT kind FROM operations WHERE operation_id = NEW.operation_id) != 'packages.apply'
     OR (SELECT apply_operation_id FROM package_plans WHERE plan_id = NEW.plan_id) != NEW.operation_id
BEGIN
    SELECT RAISE(ABORT, 'filesystem journal ownership mismatch');
END;

CREATE TRIGGER package_filesystem_journal_no_update
BEFORE UPDATE ON package_filesystem_journal
BEGIN
    SELECT RAISE(ABORT, 'filesystem journal is append-only');
END;

CREATE TRIGGER package_filesystem_journal_no_delete
BEFORE DELETE ON package_filesystem_journal
BEGIN
    SELECT RAISE(ABORT, 'filesystem journal is durable');
END;

PRAGMA user_version = 3;

COMMIT;
