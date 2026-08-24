BEGIN IMMEDIATE;

CREATE TEMP TABLE m8_operations AS SELECT * FROM operations;
CREATE TEMP TABLE m8_operation_journal AS SELECT * FROM operation_journal;
CREATE TEMP TABLE m8_package_filesystem_journal AS SELECT * FROM package_filesystem_journal;
CREATE TEMP TABLE m8_backup_restore_filesystem_journal AS
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
        'extensions.install', 'extensions.uninstall'
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

INSERT INTO operations SELECT * FROM m8_operations;

CREATE INDEX operations_owner_page
    ON operations(owner_principal_id, created_at_ms DESC, operation_id DESC);
CREATE INDEX operations_recovery
    ON operations(state, created_at_ms ASC, operation_id ASC);

INSERT INTO operation_journal SELECT * FROM m8_operation_journal;
INSERT INTO package_filesystem_journal SELECT * FROM m8_package_filesystem_journal;
INSERT INTO backup_restore_filesystem_journal
    SELECT * FROM m8_backup_restore_filesystem_journal;

DROP TABLE m8_operation_journal;
DROP TABLE m8_package_filesystem_journal;
DROP TABLE m8_backup_restore_filesystem_journal;
DROP TABLE m8_operations;

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

CREATE TABLE extensions (
    extension_id TEXT PRIMARY KEY CHECK (length(extension_id) BETWEEN 3 AND 255),
    version TEXT NOT NULL CHECK (length(version) BETWEEN 1 AND 128),
    api_major INTEGER NOT NULL CHECK (api_major = 1),
    package_digest BLOB NOT NULL CHECK (length(package_digest) = 32),
    manifest_digest BLOB NOT NULL CHECK (length(manifest_digest) = 32),
    component_digest BLOB NOT NULL CHECK (length(component_digest) = 32),
    publisher_fingerprint TEXT NOT NULL CHECK (length(publisher_fingerprint) = 79),
    trust_decision TEXT NOT NULL CHECK (trust_decision IN ('official', 'user_approved_for_extension')),
    principal_id TEXT NOT NULL CHECK (length(principal_id) BETWEEN 1 AND 128),
    live_package_locator TEXT NOT NULL CHECK (length(live_package_locator) BETWEEN 1 AND 32768),
    desired_state TEXT NOT NULL CHECK (desired_state IN ('installed_disabled', 'enabled', 'uninstalling')),
    quarantine_state TEXT NOT NULL CHECK (quarantine_state IN ('clear', 'quarantined')),
    grant_revision INTEGER NOT NULL CHECK (grant_revision BETWEEN 1 AND 9223372036854775807),
    lifecycle_generation INTEGER NOT NULL CHECK (lifecycle_generation BETWEEN 1 AND 9223372036854775807),
    revision INTEGER NOT NULL CHECK (revision BETWEEN 1 AND 9223372036854775807),
    installed_at_ms INTEGER NOT NULL CHECK (installed_at_ms BETWEEN 0 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807)
) STRICT;

CREATE TABLE extension_grants (
    extension_id TEXT NOT NULL REFERENCES extensions(extension_id) ON DELETE CASCADE,
    permission_name TEXT NOT NULL CHECK (permission_name IN ('background.run', 'projects.read')),
    resource_kind TEXT NOT NULL CHECK (resource_kind IN ('Extension', 'Project')),
    resource_id TEXT NOT NULL CHECK (length(resource_id) BETWEEN 3 AND 255),
    state TEXT NOT NULL CHECK (state IN ('granted', 'revoked')),
    grant_revision INTEGER NOT NULL CHECK (grant_revision BETWEEN 1 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    PRIMARY KEY (extension_id, permission_name, resource_kind, resource_id),
    CHECK (
        (permission_name = 'background.run' AND resource_kind = 'Extension' AND resource_id = extension_id)
        OR (permission_name = 'projects.read' AND resource_kind = 'Project' AND length(resource_id) = 36)
    )
) STRICT;

CREATE TABLE extension_instances (
    extension_id TEXT PRIMARY KEY REFERENCES extensions(extension_id) ON DELETE CASCADE,
    instance_id TEXT NOT NULL UNIQUE CHECK (length(instance_id) = 36),
    principal_id TEXT NOT NULL CHECK (length(principal_id) BETWEEN 1 AND 128),
    bound_grant_revision INTEGER NOT NULL CHECK (bound_grant_revision BETWEEN 1 AND 9223372036854775807),
    lifecycle_generation INTEGER NOT NULL CHECK (lifecycle_generation BETWEEN 1 AND 9223372036854775807),
    daemon_epoch TEXT NOT NULL CHECK (length(daemon_epoch) = 36),
    runtime_state TEXT NOT NULL CHECK (runtime_state IN ('stopped', 'starting', 'running', 'stopping', 'crashed')),
    lease_expires_at_ms INTEGER NOT NULL CHECK (lease_expires_at_ms BETWEEN 0 AND 9223372036854775807),
    lease_cancelled INTEGER NOT NULL CHECK (lease_cancelled IN (0, 1)),
    started_at_ms INTEGER CHECK (started_at_ms IS NULL OR started_at_ms BETWEEN 0 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807)
) STRICT;

CREATE TABLE extension_crashes (
    extension_id TEXT NOT NULL REFERENCES extensions(extension_id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence BETWEEN 1 AND 16),
    occurred_at_ms INTEGER NOT NULL CHECK (occurred_at_ms BETWEEN 0 AND 9223372036854775807),
    reason_code TEXT NOT NULL CHECK (length(reason_code) BETWEEN 1 AND 64),
    PRIMARY KEY (extension_id, sequence)
) STRICT;

CREATE TABLE extension_plans (
    plan_id TEXT PRIMARY KEY CHECK (length(plan_id) = 36),
    owner_principal_id TEXT NOT NULL CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    action TEXT NOT NULL CHECK (action IN ('install', 'uninstall')),
    state TEXT NOT NULL CHECK (state IN ('unapplied', 'applied')),
    extension_id TEXT NOT NULL CHECK (length(extension_id) BETWEEN 3 AND 255),
    version TEXT NOT NULL CHECK (length(version) BETWEEN 1 AND 128),
    api_major INTEGER NOT NULL CHECK (api_major = 1),
    profile_version INTEGER NOT NULL CHECK (profile_version = 1),
    expected_revision INTEGER CHECK (expected_revision IS NULL OR expected_revision BETWEEN 1 AND 9223372036854775807),
    source_kind TEXT NOT NULL CHECK (source_kind IN ('not_applicable', 'local_owner_selected', 'first_party_packaged')),
    source_locator TEXT CHECK (source_locator IS NULL OR length(source_locator) BETWEEN 1 AND 32768),
    source_identity BLOB CHECK (source_identity IS NULL OR length(source_identity) BETWEEN 1 AND 128),
    package_digest BLOB NOT NULL CHECK (length(package_digest) = 32),
    manifest_digest BLOB NOT NULL CHECK (length(manifest_digest) = 32),
    component_digest BLOB NOT NULL CHECK (length(component_digest) = 32),
    publisher_fingerprint TEXT NOT NULL CHECK (length(publisher_fingerprint) = 79),
    trust_decision TEXT NOT NULL CHECK (trust_decision IN ('official', 'user_approved_for_extension')),
    requested_permissions_json TEXT NOT NULL CHECK (json_valid(requested_permissions_json)),
    requested_interfaces_json TEXT NOT NULL CHECK (json_valid(requested_interfaces_json)),
    data_disposition TEXT NOT NULL CHECK (data_disposition IN ('not_applicable', 'retain_data', 'delete_data')),
    plan_fingerprint BLOB NOT NULL CHECK (length(plan_fingerprint) = 32),
    apply_operation_id TEXT UNIQUE REFERENCES operations(operation_id),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN 0 AND 9223372036854775807),
    CHECK (
        (state = 'unapplied' AND apply_operation_id IS NULL)
        OR (state = 'applied' AND apply_operation_id IS NOT NULL)
    )
) STRICT;

CREATE INDEX extension_plans_owner_created
    ON extension_plans(owner_principal_id, created_at_ms DESC, plan_id DESC);

CREATE TRIGGER extension_plans_immutable
BEFORE UPDATE ON extension_plans
WHEN NOT (
    OLD.state = 'unapplied' AND NEW.state = 'applied'
    AND OLD.apply_operation_id IS NULL AND NEW.apply_operation_id IS NOT NULL
    AND OLD.plan_id = NEW.plan_id AND OLD.owner_principal_id = NEW.owner_principal_id
    AND OLD.action = NEW.action AND OLD.extension_id = NEW.extension_id
    AND OLD.version = NEW.version AND OLD.api_major = NEW.api_major
    AND OLD.profile_version = NEW.profile_version
    AND OLD.expected_revision IS NEW.expected_revision AND OLD.source_kind = NEW.source_kind
    AND OLD.source_locator IS NEW.source_locator AND OLD.source_identity IS NEW.source_identity
    AND OLD.package_digest = NEW.package_digest AND OLD.manifest_digest = NEW.manifest_digest
    AND OLD.component_digest = NEW.component_digest
    AND OLD.publisher_fingerprint = NEW.publisher_fingerprint
    AND OLD.trust_decision = NEW.trust_decision
    AND OLD.requested_permissions_json = NEW.requested_permissions_json
    AND OLD.requested_interfaces_json = NEW.requested_interfaces_json
    AND OLD.data_disposition = NEW.data_disposition AND OLD.plan_fingerprint = NEW.plan_fingerprint
    AND OLD.created_at_ms = NEW.created_at_ms
    AND (SELECT kind FROM operations WHERE operation_id = NEW.apply_operation_id) =
        CASE NEW.action WHEN 'install' THEN 'extensions.install' ELSE 'extensions.uninstall' END
)
BEGIN
    SELECT RAISE(ABORT, 'extension plan is immutable');
END;

CREATE TRIGGER extension_plans_no_delete
BEFORE DELETE ON extension_plans
BEGIN
    SELECT RAISE(ABORT, 'extension plans are durable');
END;

CREATE TABLE extension_filesystem_journal (
    operation_id TEXT NOT NULL REFERENCES operations(operation_id) ON DELETE CASCADE,
    step INTEGER NOT NULL CHECK (step BETWEEN 1 AND 9223372036854775807),
    plan_id TEXT NOT NULL REFERENCES extension_plans(plan_id),
    extension_id TEXT NOT NULL CHECK (length(extension_id) BETWEEN 3 AND 255),
    action TEXT NOT NULL CHECK (action IN ('install', 'uninstall')),
    phase TEXT NOT NULL CHECK (phase IN (
        'accepted', 'source_verified', 'archive_verified', 'staging_complete',
        'publish_intent', 'package_published', 'grants_revoked', 'lease_revoked',
        'host_stopped', 'package_backup_intent', 'package_moved_to_backup',
        'data_delete_intent', 'data_deleted', 'state_commit_intent',
        'state_committed', 'cleanup_complete'
    )),
    state TEXT NOT NULL CHECK (state IN ('intent', 'completed')),
    evidence_json TEXT NOT NULL CHECK (
        json_valid(evidence_json) AND json_type(evidence_json) = 'object'
        AND length(CAST(evidence_json AS BLOB)) <= 1048576
    ),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    PRIMARY KEY (operation_id, step)
) STRICT;

CREATE INDEX extension_filesystem_journal_recovery
    ON extension_filesystem_journal(operation_id, step DESC);

CREATE TRIGGER extension_filesystem_journal_validate_insert
BEFORE INSERT ON extension_filesystem_journal
WHEN (SELECT apply_operation_id FROM extension_plans WHERE plan_id = NEW.plan_id) IS NOT NEW.operation_id
     OR (SELECT extension_id FROM extension_plans WHERE plan_id = NEW.plan_id) IS NOT NEW.extension_id
     OR (SELECT action FROM extension_plans WHERE plan_id = NEW.plan_id) IS NOT NEW.action
BEGIN
    SELECT RAISE(ABORT, 'extension filesystem journal ownership mismatch');
END;

CREATE TRIGGER extension_filesystem_journal_no_update
BEFORE UPDATE ON extension_filesystem_journal
BEGIN
    SELECT RAISE(ABORT, 'extension filesystem journal is append-only');
END;

CREATE TRIGGER extension_filesystem_journal_no_delete
BEFORE DELETE ON extension_filesystem_journal
BEGIN
    SELECT RAISE(ABORT, 'extension filesystem journal is durable');
END;

CREATE TABLE extension_data_namespaces (
    extension_id TEXT NOT NULL CHECK (length(extension_id) BETWEEN 3 AND 255),
    publisher_fingerprint TEXT NOT NULL CHECK (length(publisher_fingerprint) = 79),
    revision INTEGER NOT NULL CHECK (revision BETWEEN 1 AND 9223372036854775807),
    key_count INTEGER NOT NULL CHECK (key_count BETWEEN 0 AND 1024),
    total_value_bytes INTEGER NOT NULL CHECK (total_value_bytes BETWEEN 0 AND 4194304),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    PRIMARY KEY (extension_id, publisher_fingerprint)
) STRICT;

CREATE TABLE extension_data_items (
    extension_id TEXT NOT NULL,
    publisher_fingerprint TEXT NOT NULL,
    key TEXT NOT NULL CHECK (
        length(CAST(key AS BLOB)) BETWEEN 1 AND 128
        AND key = lower(key) AND instr(key, char(0)) = 0
        AND instr(key, '//') = 0 AND instr(key, '/../') = 0
    ),
    value BLOB NOT NULL CHECK (length(value) <= 65536),
    key_revision INTEGER NOT NULL CHECK (key_revision BETWEEN 1 AND 9223372036854775807),
    PRIMARY KEY (extension_id, publisher_fingerprint, key),
    FOREIGN KEY (extension_id, publisher_fingerprint)
        REFERENCES extension_data_namespaces(extension_id, publisher_fingerprint) ON DELETE CASCADE
) STRICT;

PRAGMA user_version = 8;

COMMIT;
