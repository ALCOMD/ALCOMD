BEGIN IMMEDIATE;

DROP TRIGGER package_filesystem_journal_validate_insert;
DROP TRIGGER package_filesystem_journal_no_update;
DROP TRIGGER package_filesystem_journal_no_delete;
DROP INDEX package_filesystem_journal_recovery;
ALTER TABLE package_filesystem_journal RENAME TO package_filesystem_journal_v11;

DROP TRIGGER package_plans_immutable;
DROP TRIGGER package_plans_no_delete;
DROP INDEX package_plans_owner_created;
DROP INDEX package_plans_project_created;
ALTER TABLE package_plans RENAME TO package_plans_v11;

CREATE TABLE package_plans (
    plan_id TEXT PRIMARY KEY CHECK (length(plan_id) = 36),
    owner_principal_id TEXT NOT NULL CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    project_id TEXT NOT NULL REFERENCES projects(project_id),
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

INSERT INTO package_plans SELECT * FROM package_plans_v11;

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
    project_id TEXT NOT NULL REFERENCES projects(project_id),
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

INSERT INTO package_filesystem_journal SELECT * FROM package_filesystem_journal_v11;

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

DROP TABLE package_filesystem_journal_v11;
DROP TABLE package_plans_v11;

ALTER TABLE repository_package_versions ADD COLUMN documentation_url TEXT
    CHECK (documentation_url IS NULL OR length(CAST(documentation_url AS BLOB)) BETWEEN 1 AND 2048);
ALTER TABLE repository_package_versions ADD COLUMN changelog_url TEXT
    CHECK (changelog_url IS NULL OR length(CAST(changelog_url AS BLOB)) BETWEEN 1 AND 2048);

CREATE TABLE user_package_sources (
    user_package_id TEXT PRIMARY KEY CHECK (length(user_package_id) = 36),
    owner_principal_id TEXT NOT NULL CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    source_root_path TEXT NOT NULL CHECK (length(CAST(source_root_path AS BLOB)) BETWEEN 1 AND 32768),
    source_identity_key BLOB NOT NULL CHECK (length(source_identity_key) BETWEEN 1 AND 128),
    package_id TEXT NOT NULL CHECK (length(package_id) BETWEEN 1 AND 128),
    version TEXT NOT NULL CHECK (length(version) BETWEEN 1 AND 1024),
    manifest_json TEXT NOT NULL CHECK (
        json_valid(manifest_json) AND json_type(manifest_json) = 'object'
        AND length(CAST(manifest_json AS BLOB)) <= 1048576
    ),
    manifest_fingerprint BLOB NOT NULL CHECK (length(manifest_fingerprint) = 32),
    content_fingerprint BLOB NOT NULL CHECK (length(content_fingerprint) = 32),
    archive_sha256 TEXT NOT NULL CHECK (
        length(archive_sha256) = 64 AND archive_sha256 = lower(archive_sha256)
        AND archive_sha256 NOT GLOB '*[^0-9a-f]*'
    ),
    revision INTEGER NOT NULL CHECK (revision BETWEEN 1 AND 9223372036854775807),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN 0 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    UNIQUE (owner_principal_id, source_identity_key),
    UNIQUE (owner_principal_id, package_id)
) STRICT;

CREATE INDEX user_package_sources_owner_page
    ON user_package_sources(owner_principal_id, updated_at_ms DESC, user_package_id DESC);

PRAGMA user_version = 12;

COMMIT;
