BEGIN IMMEDIATE;

-- Rebuild the Operation kind constraint while preserving every durable child row.
CREATE TEMP TABLE m13_operations AS SELECT * FROM operations;
CREATE TEMP TABLE m13_operation_journal AS SELECT * FROM operation_journal;
CREATE TEMP TABLE m13_package_filesystem_journal AS SELECT * FROM package_filesystem_journal;
CREATE TEMP TABLE m13_backup_restore_filesystem_journal AS
    SELECT * FROM backup_restore_filesystem_journal;
CREATE TEMP TABLE m13_extension_filesystem_journal AS SELECT * FROM extension_filesystem_journal;
CREATE TEMP TABLE m13_project_copy_filesystem_journal AS
    SELECT * FROM project_copy_filesystem_journal;

DROP TRIGGER package_filesystem_journal_no_delete;
DROP TRIGGER backup_restore_filesystem_journal_no_delete;
DROP TRIGGER extension_filesystem_journal_no_delete;
DROP TRIGGER project_copy_filesystem_journal_no_delete;

PRAGMA defer_foreign_keys = ON;
DROP TABLE operations;

CREATE TABLE operations (
    operation_id TEXT PRIMARY KEY CHECK (length(operation_id) = 36),
    kind TEXT NOT NULL CHECK (kind IN (
        'state.check', 'packages.apply',
        'templates.import', 'templates.derive', 'templates.create-project',
        'backups.create', 'backups.restore',
        'extensions.install', 'extensions.uninstall',
        'projects.copy', 'projects.delete-directory'
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

INSERT INTO operations SELECT * FROM m13_operations;

CREATE INDEX operations_owner_page
    ON operations(owner_principal_id, created_at_ms DESC, operation_id DESC);
CREATE INDEX operations_recovery
    ON operations(state, created_at_ms ASC, operation_id ASC);

INSERT INTO operation_journal SELECT * FROM m13_operation_journal;
INSERT INTO package_filesystem_journal SELECT * FROM m13_package_filesystem_journal;
INSERT INTO backup_restore_filesystem_journal
    SELECT * FROM m13_backup_restore_filesystem_journal;
INSERT INTO extension_filesystem_journal SELECT * FROM m13_extension_filesystem_journal;
INSERT INTO project_copy_filesystem_journal
    SELECT * FROM m13_project_copy_filesystem_journal;

DROP TABLE m13_operation_journal;
DROP TABLE m13_package_filesystem_journal;
DROP TABLE m13_backup_restore_filesystem_journal;
DROP TABLE m13_extension_filesystem_journal;
DROP TABLE m13_project_copy_filesystem_journal;
DROP TABLE m13_operations;

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

-- Durable Package authority retains its ProjectId after the registry row disappears.
DROP TRIGGER package_filesystem_journal_validate_insert;
DROP TRIGGER package_filesystem_journal_no_update;
DROP TRIGGER package_filesystem_journal_no_delete;
DROP INDEX package_filesystem_journal_recovery;
ALTER TABLE package_filesystem_journal RENAME TO package_filesystem_journal_v12;

DROP TRIGGER package_plans_immutable;
DROP TRIGGER package_plans_no_delete;
DROP INDEX package_plans_owner_created;
DROP INDEX package_plans_project_created;
ALTER TABLE package_plans RENAME TO package_plans_v12;

CREATE TABLE package_plans (
    plan_id TEXT PRIMARY KEY CHECK (length(plan_id) = 36),
    owner_principal_id TEXT NOT NULL CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    project_id TEXT NOT NULL CHECK (
        length(project_id) = 36 AND project_id = lower(project_id)
        AND substr(project_id, 9, 1) = '-' AND substr(project_id, 14, 1) = '-'
        AND substr(project_id, 19, 1) = '-' AND substr(project_id, 24, 1) = '-'
        AND replace(project_id, '-', '') NOT GLOB '*[^0-9a-f]*'
    ),
    action TEXT NOT NULL CHECK (action IN (
        'install', 'remove', 'upgrade', 'downgrade', 'resolve', 'reinstall', 'bulk'
    )),
    state TEXT NOT NULL CHECK (state IN ('unapplied', 'applied')),
    project_revision INTEGER NOT NULL CHECK (project_revision BETWEEN 1 AND 9223372036854775807),
    project_snapshot_fingerprint BLOB NOT NULL CHECK (length(project_snapshot_fingerprint) = 32),
    change_set_fingerprint BLOB NOT NULL CHECK (length(change_set_fingerprint) = 32),
    change_set_json TEXT NOT NULL CHECK (
        json_valid(change_set_json)
        AND json_type(change_set_json) = 'object'
        AND length(change_set_json) <= 4194304
        AND json_array_length(change_set_json, '$.mutations') <= 1024
        AND json_array_length(change_set_json, '$.dependencyEdges') <= 4096
    ),
    source_set_json TEXT NOT NULL CHECK (
        json_valid(source_set_json) AND json_type(source_set_json) = 'array'
        AND length(source_set_json) <= 4194304
    ),
    apply_operation_id TEXT UNIQUE REFERENCES operations(operation_id),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN 0 AND 9223372036854775807),
    CHECK (
        (state = 'unapplied' AND apply_operation_id IS NULL)
        OR (state = 'applied' AND apply_operation_id IS NOT NULL)
    )
) STRICT;

INSERT INTO package_plans SELECT * FROM package_plans_v12;

CREATE INDEX package_plans_owner_created
    ON package_plans(owner_principal_id, created_at_ms DESC, plan_id DESC);
CREATE INDEX package_plans_project_created
    ON package_plans(project_id, created_at_ms DESC, plan_id DESC);

CREATE TRIGGER package_plans_immutable
BEFORE UPDATE ON package_plans
WHEN NOT (
    OLD.state = 'unapplied' AND NEW.state = 'applied'
    AND OLD.apply_operation_id IS NULL AND NEW.apply_operation_id IS NOT NULL
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
    operation_id TEXT NOT NULL REFERENCES operations(operation_id) ON DELETE CASCADE,
    step INTEGER NOT NULL CHECK (step BETWEEN 1 AND 9223372036854775807),
    plan_id TEXT NOT NULL REFERENCES package_plans(plan_id),
    project_id TEXT NOT NULL CHECK (
        length(project_id) = 36 AND project_id = lower(project_id)
        AND substr(project_id, 9, 1) = '-' AND substr(project_id, 14, 1) = '-'
        AND substr(project_id, 19, 1) = '-' AND substr(project_id, 24, 1) = '-'
        AND replace(project_id, '-', '') NOT GLOB '*[^0-9a-f]*'
    ),
    phase TEXT NOT NULL CHECK (phase IN (
        'accepted', 'archive_ready', 'extracted', 'prepared',
        'packages_replaced', 'vpm_manifest_committed', 'filesystem_committed',
        'state_committed', 'rolling_back', 'rolled_back', 'recovery_required'
    )),
    state TEXT NOT NULL CHECK (state IN ('intent', 'completed')),
    project_identity_key BLOB NOT NULL CHECK (length(project_identity_key) BETWEEN 1 AND 128),
    change_set_fingerprint BLOB NOT NULL CHECK (length(change_set_fingerprint) = 32),
    evidence_json TEXT NOT NULL CHECK (
        json_valid(evidence_json) AND json_type(evidence_json) = 'object'
        AND length(evidence_json) <= 4194304
    ),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    PRIMARY KEY (operation_id, step)
) STRICT;

INSERT INTO package_filesystem_journal SELECT * FROM package_filesystem_journal_v12;

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

DROP TABLE package_filesystem_journal_v12;
DROP TABLE package_plans_v12;

-- Durable Project Copy authority also retains its source ProjectId.
DROP TRIGGER project_copy_filesystem_journal_validate_insert;
DROP TRIGGER project_copy_filesystem_journal_no_update;
DROP TRIGGER project_copy_filesystem_journal_no_delete;
DROP INDEX project_copy_filesystem_journal_recovery;
ALTER TABLE project_copy_filesystem_journal RENAME TO project_copy_filesystem_journal_v12;

DROP TRIGGER project_copy_plans_immutable;
DROP TRIGGER project_copy_plans_no_delete;
DROP INDEX project_copy_plans_owner_created;
ALTER TABLE project_copy_plans RENAME TO project_copy_plans_v12;

CREATE TABLE project_copy_plans (
    plan_id TEXT PRIMARY KEY CHECK (length(plan_id) = 36),
    owner_principal_id TEXT NOT NULL CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    state TEXT NOT NULL CHECK (state IN ('unapplied', 'applied')),
    source_project_id TEXT NOT NULL CHECK (
        length(source_project_id) = 36 AND source_project_id = lower(source_project_id)
        AND substr(source_project_id, 9, 1) = '-'
        AND substr(source_project_id, 14, 1) = '-'
        AND substr(source_project_id, 19, 1) = '-'
        AND substr(source_project_id, 24, 1) = '-'
        AND replace(source_project_id, '-', '') NOT GLOB '*[^0-9a-f]*'
    ),
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

INSERT INTO project_copy_plans SELECT * FROM project_copy_plans_v12;

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
    source_project_id TEXT NOT NULL CHECK (
        length(source_project_id) = 36 AND source_project_id = lower(source_project_id)
        AND substr(source_project_id, 9, 1) = '-'
        AND substr(source_project_id, 14, 1) = '-'
        AND substr(source_project_id, 19, 1) = '-'
        AND substr(source_project_id, 24, 1) = '-'
        AND replace(source_project_id, '-', '') NOT GLOB '*[^0-9a-f]*'
    ),
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

INSERT INTO project_copy_filesystem_journal SELECT * FROM project_copy_filesystem_journal_v12;

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

DROP TABLE project_copy_filesystem_journal_v12;
DROP TABLE project_copy_plans_v12;

CREATE TABLE project_delete_plans (
    plan_id TEXT PRIMARY KEY CHECK (length(plan_id) = 36),
    owner_principal_id TEXT NOT NULL CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    state TEXT NOT NULL CHECK (state IN ('unapplied', 'applied')),
    project_id TEXT NOT NULL CHECK (
        length(project_id) = 36 AND project_id = lower(project_id)
        AND substr(project_id, 9, 1) = '-' AND substr(project_id, 14, 1) = '-'
        AND substr(project_id, 19, 1) = '-' AND substr(project_id, 24, 1) = '-'
        AND replace(project_id, '-', '') NOT GLOB '*[^0-9a-f]*'
    ),
    project_revision INTEGER NOT NULL CHECK (project_revision BETWEEN 1 AND 9223372036854775807),
    root_path TEXT NOT NULL CHECK (length(CAST(root_path AS BLOB)) BETWEEN 1 AND 32768),
    root_identity BLOB NOT NULL CHECK (length(root_identity) BETWEEN 1 AND 128),
    parent_path TEXT NOT NULL CHECK (length(CAST(parent_path AS BLOB)) BETWEEN 1 AND 32768),
    parent_identity BLOB NOT NULL CHECK (length(parent_identity) BETWEEN 1 AND 128),
    parent_identity_sha256 BLOB NOT NULL CHECK (length(parent_identity_sha256) = 32),
    normalized_leaf TEXT NOT NULL CHECK (
        length(normalized_leaf) BETWEEN 1 AND 255
        AND instr(normalized_leaf, '/') = 0
        AND instr(normalized_leaf, char(92)) = 0
        AND instr(normalized_leaf, char(0)) = 0
    ),
    project_snapshot_json TEXT NOT NULL CHECK (
        json_valid(project_snapshot_json) AND json_type(project_snapshot_json) = 'object'
        AND length(CAST(project_snapshot_json AS BLOB)) <= 4194304
    ),
    marker_fingerprint BLOB NOT NULL CHECK (length(marker_fingerprint) = 32),
    writer_evidence_json TEXT NOT NULL CHECK (
        json_valid(writer_evidence_json) AND json_type(writer_evidence_json) = 'object'
        AND length(CAST(writer_evidence_json AS BLOB)) <= 65536
    ),
    deletion_mode TEXT NOT NULL CHECK (deletion_mode = 'sibling-quarantine-permanent-v1'),
    safety_profile_id TEXT NOT NULL CHECK (safety_profile_id = 'alcomd-project-delete'),
    safety_profile_version INTEGER NOT NULL CHECK (safety_profile_version = 1),
    protected_root_profile_version INTEGER NOT NULL CHECK (protected_root_profile_version = 1),
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

CREATE INDEX project_delete_plans_owner_created
    ON project_delete_plans(owner_principal_id, created_at_ms DESC, plan_id DESC);

CREATE TRIGGER project_delete_plans_immutable
BEFORE UPDATE ON project_delete_plans
WHEN NOT (
    OLD.state = 'unapplied' AND NEW.state = 'applied'
    AND OLD.apply_operation_id IS NULL AND NEW.apply_operation_id IS NOT NULL
    AND OLD.plan_id = NEW.plan_id
    AND OLD.owner_principal_id = NEW.owner_principal_id
    AND OLD.project_id = NEW.project_id
    AND OLD.project_revision = NEW.project_revision
    AND OLD.root_path = NEW.root_path
    AND OLD.root_identity = NEW.root_identity
    AND OLD.parent_path = NEW.parent_path
    AND OLD.parent_identity = NEW.parent_identity
    AND OLD.parent_identity_sha256 = NEW.parent_identity_sha256
    AND OLD.normalized_leaf = NEW.normalized_leaf
    AND OLD.project_snapshot_json = NEW.project_snapshot_json
    AND OLD.marker_fingerprint = NEW.marker_fingerprint
    AND OLD.writer_evidence_json = NEW.writer_evidence_json
    AND OLD.deletion_mode = NEW.deletion_mode
    AND OLD.safety_profile_id = NEW.safety_profile_id
    AND OLD.safety_profile_version = NEW.safety_profile_version
    AND OLD.protected_root_profile_version = NEW.protected_root_profile_version
    AND OLD.plan_fingerprint = NEW.plan_fingerprint
    AND OLD.plan_json = NEW.plan_json
    AND OLD.plan_idempotency_key = NEW.plan_idempotency_key
    AND OLD.created_at_ms = NEW.created_at_ms
    AND OLD.expires_at_ms = NEW.expires_at_ms
    AND (SELECT kind FROM operations WHERE operation_id = NEW.apply_operation_id)
        = 'projects.delete-directory'
)
BEGIN
    SELECT RAISE(ABORT, 'project delete plan is immutable');
END;

CREATE TRIGGER project_delete_plans_no_delete
BEFORE DELETE ON project_delete_plans
BEGIN
    SELECT RAISE(ABORT, 'project delete plans are durable');
END;

CREATE TABLE project_delete_filesystem_journal (
    operation_id TEXT NOT NULL REFERENCES operations(operation_id) ON DELETE CASCADE,
    step INTEGER NOT NULL CHECK (step BETWEEN 1 AND 9223372036854775807),
    plan_id TEXT NOT NULL REFERENCES project_delete_plans(plan_id),
    project_id TEXT NOT NULL CHECK (
        length(project_id) = 36 AND project_id = lower(project_id)
        AND substr(project_id, 9, 1) = '-' AND substr(project_id, 14, 1) = '-'
        AND substr(project_id, 19, 1) = '-' AND substr(project_id, 24, 1) = '-'
        AND replace(project_id, '-', '') NOT GLOB '*[^0-9a-f]*'
    ),
    phase TEXT NOT NULL CHECK (phase IN (
        'accepted', 'preflight_complete', 'quarantine_intent', 'root_quarantined',
        'registry_commit_intent', 'state_committed', 'deleting',
        'cleanup_complete', 'recovery_required'
    )),
    state TEXT NOT NULL CHECK (state IN ('intent', 'completed')),
    root_identity BLOB NOT NULL CHECK (length(root_identity) BETWEEN 1 AND 128),
    parent_identity BLOB NOT NULL CHECK (length(parent_identity) BETWEEN 1 AND 128),
    quarantine_identity BLOB CHECK (
        quarantine_identity IS NULL OR length(quarantine_identity) BETWEEN 1 AND 128
    ),
    payload_identity BLOB CHECK (
        payload_identity IS NULL OR length(payload_identity) BETWEEN 1 AND 128
    ),
    quarantine_locator TEXT NOT NULL CHECK (
        length(CAST(quarantine_locator AS BLOB)) BETWEEN 1 AND 32768
    ),
    owner_marker TEXT NOT NULL CHECK (
        length(CAST(owner_marker AS BLOB)) BETWEEN 1 AND 65536
    ),
    attempt_count INTEGER NOT NULL CHECK (attempt_count BETWEEN 0 AND 9223372036854775807),
    entries_processed INTEGER NOT NULL CHECK (
        entries_processed BETWEEN 0 AND 9223372036854775807
    ),
    safe_reason TEXT CHECK (safe_reason IS NULL OR length(safe_reason) BETWEEN 1 AND 128),
    evidence_json TEXT NOT NULL CHECK (
        json_valid(evidence_json) AND json_type(evidence_json) = 'object'
        AND length(CAST(evidence_json AS BLOB)) <= 65536
    ),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    PRIMARY KEY (operation_id, step)
) STRICT;

CREATE INDEX project_delete_filesystem_journal_recovery
    ON project_delete_filesystem_journal(operation_id, step DESC);

CREATE TRIGGER project_delete_filesystem_journal_validate_insert
BEFORE INSERT ON project_delete_filesystem_journal
WHEN (SELECT kind FROM operations WHERE operation_id = NEW.operation_id)
        IS NOT 'projects.delete-directory'
     OR (SELECT apply_operation_id FROM project_delete_plans WHERE plan_id = NEW.plan_id)
        IS NOT NEW.operation_id
BEGIN
    SELECT RAISE(ABORT, 'project delete journal ownership mismatch');
END;

CREATE TRIGGER project_delete_filesystem_journal_no_update
BEFORE UPDATE ON project_delete_filesystem_journal
BEGIN
    SELECT RAISE(ABORT, 'project delete journal is append-only');
END;

CREATE TRIGGER project_delete_filesystem_journal_no_delete
BEFORE DELETE ON project_delete_filesystem_journal
BEGIN
    SELECT RAISE(ABORT, 'project delete journal is durable');
END;

PRAGMA user_version = 13;

COMMIT;
