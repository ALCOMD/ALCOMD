BEGIN IMMEDIATE;

ALTER TABLE extensions
ADD COLUMN ui_protocol TEXT CHECK (ui_protocol IS NULL OR ui_protocol = 'portable-v1');

DROP TRIGGER extension_plans_immutable;

ALTER TABLE extension_plans
ADD COLUMN ui_protocol TEXT CHECK (ui_protocol IS NULL OR ui_protocol = 'portable-v1');

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
    AND OLD.ui_protocol IS NEW.ui_protocol
    AND OLD.created_at_ms = NEW.created_at_ms
    AND (SELECT kind FROM operations WHERE operation_id = NEW.apply_operation_id) =
        CASE NEW.action WHEN 'install' THEN 'extensions.install' ELSE 'extensions.uninstall' END
)
BEGIN
    SELECT RAISE(ABORT, 'extension plan is immutable');
END;

PRAGMA user_version = 9;

COMMIT;
