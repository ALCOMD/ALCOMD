BEGIN IMMEDIATE;

CREATE TEMP TABLE m10_operations AS SELECT * FROM operations;
CREATE TEMP TABLE m10_operation_journal AS SELECT * FROM operation_journal;
CREATE TEMP TABLE m10_package_filesystem_journal AS SELECT * FROM package_filesystem_journal;
CREATE TEMP TABLE m10_backup_restore_filesystem_journal AS
    SELECT * FROM backup_restore_filesystem_journal;

DROP TRIGGER package_filesystem_journal_no_delete;
DROP TRIGGER backup_restore_filesystem_journal_no_delete;

PRAGMA defer_foreign_keys = ON;
DROP TABLE operations;

CREATE TABLE operations (
    operation_id TEXT PRIMARY KEY CHECK (length(operation_id) = 36),
    kind TEXT NOT NULL CHECK (kind IN (
        'state.check', 'packages.apply',
        'templates.import', 'templates.derive', 'templates.create-project',
        'backups.create', 'backups.restore',
        'extensions.install', 'extensions.uninstall',
        'projects.copy'
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

INSERT INTO operations SELECT * FROM m10_operations;

CREATE INDEX operations_owner_page
    ON operations(owner_principal_id, created_at_ms DESC, operation_id DESC);
CREATE INDEX operations_recovery
    ON operations(state, created_at_ms ASC, operation_id ASC);

INSERT INTO operation_journal SELECT * FROM m10_operation_journal;
INSERT INTO package_filesystem_journal SELECT * FROM m10_package_filesystem_journal;
INSERT INTO backup_restore_filesystem_journal
    SELECT * FROM m10_backup_restore_filesystem_journal;

DROP TABLE m10_operation_journal;
DROP TABLE m10_package_filesystem_journal;
DROP TABLE m10_backup_restore_filesystem_journal;
DROP TABLE m10_operations;

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

CREATE TABLE project_copy_plans (
    plan_id TEXT PRIMARY KEY CHECK (length(plan_id) = 36),
    owner_principal_id TEXT NOT NULL CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    state TEXT NOT NULL CHECK (state IN ('unapplied', 'applied')),
    source_project_id TEXT NOT NULL REFERENCES projects(project_id),
    source_revision INTEGER NOT NULL CHECK (source_revision BETWEEN 1 AND 9223372036854775807),
    source_root_path TEXT NOT NULL CHECK (length(source_root_path) BETWEEN 1 AND 32768),
    source_root_identity BLOB NOT NULL CHECK (length(source_root_identity) BETWEEN 1 AND 128),
    source_snapshot_json TEXT NOT NULL CHECK (
        json_valid(source_snapshot_json) AND json_type(source_snapshot_json) = 'object'
        AND length(CAST(source_snapshot_json AS BLOB)) <= 4194304
    ),
    target_parent_path TEXT NOT NULL CHECK (length(target_parent_path) BETWEEN 1 AND 32768),
    target_parent_identity BLOB NOT NULL CHECK (length(target_parent_identity) BETWEEN 1 AND 128),
    target_parent_identity_sha256 BLOB NOT NULL CHECK (length(target_parent_identity_sha256) = 32),
    target_leaf TEXT NOT NULL CHECK (
        length(target_leaf) BETWEEN 1 AND 255
        AND instr(target_leaf, '/') = 0
        AND instr(target_leaf, char(92)) = 0
        AND instr(target_leaf, char(0)) = 0
    ),
    target_project_id TEXT NOT NULL UNIQUE CHECK (length(target_project_id) = 36),
    profile_version INTEGER NOT NULL CHECK (profile_version = 1),
    writer_evidence_json TEXT NOT NULL CHECK (
        json_valid(writer_evidence_json) AND json_type(writer_evidence_json) = 'object'
        AND length(CAST(writer_evidence_json AS BLOB)) <= 65536
    ),
    plan_fingerprint BLOB NOT NULL CHECK (length(plan_fingerprint) = 32),
    plan_json TEXT NOT NULL CHECK (
        json_valid(plan_json) AND json_type(plan_json) = 'object'
        AND length(CAST(plan_json AS BLOB)) <= 4194304
    ),
    plan_idempotency_key TEXT NOT NULL CHECK (length(plan_idempotency_key) BETWEEN 1 AND 128),
    apply_operation_id TEXT UNIQUE REFERENCES operations(operation_id),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN 0 AND 9223372036854775807),
    expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms = created_at_ms + 900000),
    CHECK (
        (state = 'unapplied' AND apply_operation_id IS NULL)
        OR (state = 'applied' AND apply_operation_id IS NOT NULL)
    )
) STRICT;

CREATE INDEX project_copy_plans_owner_created
    ON project_copy_plans(owner_principal_id, created_at_ms DESC, plan_id DESC);

CREATE TRIGGER project_copy_plans_immutable
BEFORE UPDATE ON project_copy_plans
WHEN NOT (
    OLD.state = 'unapplied' AND NEW.state = 'applied'
    AND OLD.apply_operation_id IS NULL AND NEW.apply_operation_id IS NOT NULL
    AND OLD.plan_id = NEW.plan_id
    AND OLD.owner_principal_id = NEW.owner_principal_id
    AND OLD.source_project_id = NEW.source_project_id
    AND OLD.source_revision = NEW.source_revision
    AND OLD.source_root_path = NEW.source_root_path
    AND OLD.source_root_identity = NEW.source_root_identity
    AND OLD.source_snapshot_json = NEW.source_snapshot_json
    AND OLD.target_parent_path = NEW.target_parent_path
    AND OLD.target_parent_identity = NEW.target_parent_identity
    AND OLD.target_parent_identity_sha256 = NEW.target_parent_identity_sha256
    AND OLD.target_leaf = NEW.target_leaf
    AND OLD.target_project_id = NEW.target_project_id
    AND OLD.profile_version = NEW.profile_version
    AND OLD.writer_evidence_json = NEW.writer_evidence_json
    AND OLD.plan_fingerprint = NEW.plan_fingerprint
    AND OLD.plan_json = NEW.plan_json
    AND OLD.plan_idempotency_key = NEW.plan_idempotency_key
    AND OLD.created_at_ms = NEW.created_at_ms
    AND OLD.expires_at_ms = NEW.expires_at_ms
    AND (SELECT kind FROM operations WHERE operation_id = NEW.apply_operation_id) = 'projects.copy'
)
BEGIN
    SELECT RAISE(ABORT, 'project copy plan is immutable');
END;

CREATE TRIGGER project_copy_plans_no_delete
BEFORE DELETE ON project_copy_plans
BEGIN
    SELECT RAISE(ABORT, 'project copy plans are durable');
END;

CREATE TABLE project_copy_filesystem_journal (
    operation_id TEXT NOT NULL REFERENCES operations(operation_id) ON DELETE CASCADE,
    step INTEGER NOT NULL CHECK (step BETWEEN 1 AND 9223372036854775807),
    plan_id TEXT NOT NULL REFERENCES project_copy_plans(plan_id),
    source_project_id TEXT NOT NULL REFERENCES projects(project_id),
    target_project_id TEXT NOT NULL CHECK (length(target_project_id) = 36),
    phase TEXT NOT NULL CHECK (phase IN (
        'accepted', 'inventory_ready', 'staging', 'staging_complete', 'publish_intent',
        'target_published', 'project_registry_commit_intent', 'state_committed',
        'cleanup_complete', 'recovery_required'
    )),
    state TEXT NOT NULL CHECK (state IN ('intent', 'completed')),
    source_identity BLOB NOT NULL CHECK (length(source_identity) BETWEEN 1 AND 128),
    target_parent_identity BLOB NOT NULL CHECK (length(target_parent_identity) BETWEEN 1 AND 128),
    target_identity BLOB CHECK (target_identity IS NULL OR length(target_identity) BETWEEN 1 AND 128),
    inventory_locator TEXT CHECK (inventory_locator IS NULL OR length(inventory_locator) BETWEEN 1 AND 32768),
    inventory_sha256 BLOB CHECK (inventory_sha256 IS NULL OR length(inventory_sha256) = 32),
    inventory_byte_length INTEGER CHECK (inventory_byte_length IS NULL OR inventory_byte_length >= 1),
    owner_marker TEXT CHECK (owner_marker IS NULL OR length(owner_marker) BETWEEN 1 AND 32768),
    evidence_json TEXT NOT NULL CHECK (
        json_valid(evidence_json) AND json_type(evidence_json) = 'object'
        AND length(CAST(evidence_json AS BLOB)) <= 4194304
    ),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    PRIMARY KEY (operation_id, step)
) STRICT;

CREATE INDEX project_copy_filesystem_journal_recovery
    ON project_copy_filesystem_journal(operation_id, step DESC);

CREATE TRIGGER project_copy_filesystem_journal_validate_insert
BEFORE INSERT ON project_copy_filesystem_journal
WHEN (SELECT kind FROM operations WHERE operation_id = NEW.operation_id) IS NOT 'projects.copy'
     OR (SELECT apply_operation_id FROM project_copy_plans WHERE plan_id = NEW.plan_id)
        IS NOT NEW.operation_id
BEGIN
    SELECT RAISE(ABORT, 'project copy journal ownership mismatch');
END;

CREATE TRIGGER project_copy_filesystem_journal_no_update
BEFORE UPDATE ON project_copy_filesystem_journal
BEGIN
    SELECT RAISE(ABORT, 'project copy journal is append-only');
END;

CREATE TRIGGER project_copy_filesystem_journal_no_delete
BEFORE DELETE ON project_copy_filesystem_journal
BEGIN
    SELECT RAISE(ABORT, 'project copy journal is durable');
END;

PRAGMA user_version = 10;

COMMIT;
