BEGIN IMMEDIATE;

-- Project inspection time is removed from the current model and durable snapshots.
-- Keep the approved plan fingerprints and all independent writer/install evidence.
DROP TRIGGER project_copy_plans_immutable;
DROP TRIGGER project_delete_plans_immutable;
DROP TRIGGER project_copy_filesystem_journal_no_update;

ALTER TABLE projects DROP COLUMN observed_at_ms;

UPDATE projects
SET snapshot_json = json_remove(snapshot_json, '$.observed_at_ms')
WHERE json_type(snapshot_json, '$.observed_at_ms') IS NOT NULL;

UPDATE idempotency_records
SET response_json = json_remove(response_json, '$.value.observation.observed_at_ms')
WHERE method IN ('projects.register', 'projects.refresh', 'projects.setFavorite')
  AND json_type(response_json, '$.value.observation.observed_at_ms') IS NOT NULL;

UPDATE project_copy_plans
SET source_snapshot_json = json_remove(source_snapshot_json, '$.observation.observed_at_ms')
WHERE json_type(source_snapshot_json, '$.observation.observed_at_ms') IS NOT NULL;

UPDATE idempotency_records
SET response_json = json_remove(response_json, '$.plan.sourceProject.observation.observed_at_ms')
WHERE method = 'projects.planCopy'
  AND json_type(response_json, '$.plan.sourceProject.observation.observed_at_ms') IS NOT NULL;

UPDATE project_delete_plans
SET project_snapshot_json = json_remove(project_snapshot_json, '$.observation.observed_at_ms')
WHERE json_type(project_snapshot_json, '$.observation.observed_at_ms') IS NOT NULL;

UPDATE idempotency_records
SET response_json = json_remove(response_json, '$.plan.project.observation.observed_at_ms')
WHERE method IN ('projects.planDeleteDirectory', 'projects.planUnityMigration')
  AND json_type(response_json, '$.plan.project.observation.observed_at_ms') IS NOT NULL;

-- Older Unity migration authority hashes included the removed project metadata.
-- Never re-sign them: unapplied plans must be reviewed again under the new model.
-- Already-applied operations retain their original hash and recovery evidence.
UPDATE project_unity_migration_plans
SET project_snapshot_json = json_remove(project_snapshot_json, '$.observation.observed_at_ms')
WHERE json_type(project_snapshot_json, '$.observation.observed_at_ms') IS NOT NULL;

UPDATE project_unity_migration_plans
SET expires_at_ms = 0
WHERE state = 'unapplied';

UPDATE idempotency_records
SET response_json = json_set(response_json, '$.plan.expiresAtMs', 0)
WHERE method = 'projects.planUnityMigration'
  AND json_extract(response_json, '$.plan.planId') IN (
      SELECT plan_id FROM project_unity_migration_plans WHERE state = 'unapplied'
  );

UPDATE project_copy_filesystem_journal
SET evidence_json = json_remove(evidence_json, '$.published.observation.observed_at_ms')
WHERE json_type(evidence_json, '$.published.observation.observed_at_ms') IS NOT NULL;

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

CREATE TRIGGER project_copy_filesystem_journal_no_update
BEFORE UPDATE ON project_copy_filesystem_journal
BEGIN
    SELECT RAISE(ABORT, 'project copy journal is append-only');
END;

PRAGMA user_version = 16;
COMMIT;
