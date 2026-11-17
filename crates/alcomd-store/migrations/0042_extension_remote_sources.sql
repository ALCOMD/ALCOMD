-- Foreign keys are disabled only on this migration connection for the SQLite
-- table rebuild; all relationships are checked before committing and re-enabled.
PRAGMA foreign_keys=OFF;
PRAGMA legacy_alter_table=ON;
BEGIN IMMEDIATE;
CREATE TABLE extension_plans_v42 (
    plan_id TEXT PRIMARY KEY CHECK (length(plan_id) = 36),
    owner_principal_id TEXT NOT NULL CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    action TEXT NOT NULL CHECK (action IN ('install', 'uninstall')),
    state TEXT NOT NULL CHECK (state IN ('unapplied', 'applied')),
    extension_id TEXT NOT NULL CHECK (length(extension_id) BETWEEN 3 AND 255),
    version TEXT NOT NULL CHECK (length(version) BETWEEN 1 AND 128),
    api_major INTEGER NOT NULL CHECK (api_major = 1),
    profile_version INTEGER NOT NULL CHECK (profile_version = 1),
    expected_revision INTEGER CHECK (expected_revision IS NULL OR expected_revision BETWEEN 1 AND 9223372036854775807),
    source_kind TEXT NOT NULL CHECK (source_kind IN ('not_applicable', 'local_owner_selected', 'first_party_packaged', 'remote_extension_source')),
    source_locator TEXT CHECK (source_locator IS NULL OR length(source_locator) BETWEEN 1 AND 32768),
    source_identity BLOB CHECK (source_identity IS NULL OR length(source_identity) BETWEEN 1 AND 128),
    package_digest BLOB NOT NULL CHECK (length(package_digest) = 32),
    manifest_digest BLOB NOT NULL CHECK (length(manifest_digest) = 32),
    component_digest BLOB NOT NULL CHECK (length(component_digest) = 32),
    publisher_fingerprint TEXT NOT NULL CHECK (length(publisher_fingerprint) = 79),
    requested_permissions_json TEXT NOT NULL CHECK (json_valid(requested_permissions_json)),
    requested_interfaces_json TEXT NOT NULL CHECK (json_valid(requested_interfaces_json)),
    data_disposition TEXT NOT NULL CHECK (data_disposition IN ('not_applicable', 'retain_data', 'delete_data')),
    plan_fingerprint BLOB NOT NULL CHECK (length(plan_fingerprint) = 32),
    apply_operation_id TEXT UNIQUE REFERENCES operations(operation_id),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN 0 AND 9223372036854775807),
    ui_protocol TEXT CHECK (ui_protocol IS NULL OR ui_protocol = 'portable-v1'),
    installation_terms_json TEXT CHECK(installation_terms_json IS NULL OR (json_valid(installation_terms_json) AND length(CAST(installation_terms_json AS BLOB)) <= 100000)),
    remote_source_json TEXT CHECK(remote_source_json IS NULL OR (json_valid(remote_source_json) AND length(CAST(remote_source_json AS BLOB)) <= 65536)),
    CHECK ((source_kind='remote_extension_source') = (remote_source_json IS NOT NULL)),
    CHECK (
        (state = 'unapplied' AND apply_operation_id IS NULL)
        OR (state = 'applied' AND apply_operation_id IS NOT NULL)
    )
) STRICT;
INSERT INTO extension_plans_v42 (plan_id,owner_principal_id,action,state,extension_id,version,api_major,profile_version,expected_revision,source_kind,source_locator,source_identity,package_digest,manifest_digest,component_digest,publisher_fingerprint,requested_permissions_json,requested_interfaces_json,data_disposition,plan_fingerprint,apply_operation_id,created_at_ms,ui_protocol,installation_terms_json) SELECT plan_id,owner_principal_id,action,state,extension_id,version,api_major,profile_version,expected_revision,source_kind,source_locator,source_identity,package_digest,manifest_digest,component_digest,publisher_fingerprint,requested_permissions_json,requested_interfaces_json,data_disposition,plan_fingerprint,apply_operation_id,created_at_ms,ui_protocol,installation_terms_json FROM extension_plans;
DROP TRIGGER extension_plans_immutable;
DROP TABLE extension_plans;
ALTER TABLE extension_plans_v42 RENAME TO extension_plans;
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
    AND OLD.remote_source_json IS NEW.remote_source_json
    AND OLD.installation_terms_json IS NEW.installation_terms_json
    AND OLD.requested_permissions_json = NEW.requested_permissions_json
    AND OLD.requested_interfaces_json = NEW.requested_interfaces_json
    AND OLD.data_disposition = NEW.data_disposition AND OLD.plan_fingerprint = NEW.plan_fingerprint
    AND OLD.ui_protocol IS NEW.ui_protocol
    AND OLD.created_at_ms = NEW.created_at_ms
    AND (SELECT kind FROM operations WHERE operation_id = NEW.apply_operation_id) =
        CASE NEW.action WHEN 'install' THEN 'extensions.install' ELSE 'extensions.uninstall' END
)
BEGIN
    SELECT RAISE(ABORT, 'extension plan is immutable');
END;
CREATE TEMP TABLE state42_integrity_check (violations INTEGER CHECK(violations=0));
INSERT INTO state42_integrity_check SELECT count(*) FROM pragma_foreign_key_check;
DROP TABLE state42_integrity_check;
PRAGMA user_version=42;
COMMIT;
PRAGMA legacy_alter_table=OFF;
PRAGMA foreign_keys=ON;
