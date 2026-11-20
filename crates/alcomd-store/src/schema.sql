-- Initial State46 database schema. Executed atomically before WAL is enabled.

CREATE TABLE projects (
    project_id TEXT PRIMARY KEY
        CHECK (length(project_id) = 36),
    owner_principal_id TEXT NOT NULL
        CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    root_path TEXT NOT NULL
        CHECK (length(root_path) BETWEEN 1 AND 32768),
    path_identity_key BLOB NOT NULL UNIQUE
        CHECK (length(path_identity_key) BETWEEN 1 AND 128),
    project_type TEXT NOT NULL
        CHECK (project_type IN (
            'avatars', 'worlds', 'vpm-starter', 'upm-avatars', 'upm-worlds',
            'upm-starter', 'legacy-sdk2', 'legacy-worlds', 'legacy-avatars', 'unknown'
        )),
    unity_version TEXT NOT NULL
        CHECK (length(unity_version) BETWEEN 1 AND 128),
    unity_revision TEXT
        CHECK (unity_revision IS NULL OR length(unity_revision) BETWEEN 1 AND 128),
    snapshot_json TEXT NOT NULL
        CHECK (json_valid(snapshot_json) AND length(snapshot_json) <= 4194304),
    revision INTEGER NOT NULL
        CHECK (revision BETWEEN 1 AND 9223372036854775807),
    registered_at_ms INTEGER NOT NULL
        CHECK (registered_at_ms BETWEEN 0 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL
        CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    favorite INTEGER NOT NULL DEFAULT 0
        CHECK (favorite IN (0, 1)),
    last_modified_at_ms INTEGER
        CHECK (last_modified_at_ms IS NULL OR last_modified_at_ms BETWEEN 0 AND 9223372036854775807)
) STRICT;

CREATE TABLE repositories (
    repository_id TEXT PRIMARY KEY
        CHECK (length(repository_id) = 36),
    owner_principal_id TEXT NOT NULL
        CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    source_kind TEXT NOT NULL
        CHECK (source_kind IN ('local', 'remote')),
    source_locator TEXT NOT NULL
        CHECK (length(source_locator) BETWEEN 1 AND 32768),
    source_identity_key BLOB NOT NULL UNIQUE
        CHECK (length(source_identity_key) BETWEEN 1 AND 32768),
    declared_id TEXT
        CHECK (declared_id IS NULL OR length(declared_id) BETWEEN 1 AND 1024),
    name TEXT
        CHECK (name IS NULL OR length(name) BETWEEN 1 AND 4096),
    declared_url TEXT
        CHECK (declared_url IS NULL OR length(declared_url) BETWEEN 1 AND 32768),
    etag TEXT
        CHECK (etag IS NULL OR length(etag) BETWEEN 1 AND 4096),
    last_modified TEXT
        CHECK (last_modified IS NULL OR length(last_modified) BETWEEN 1 AND 4096),
    issues_json TEXT NOT NULL
        CHECK (json_valid(issues_json) AND length(issues_json) <= 1048576),
    revision INTEGER NOT NULL
        CHECK (revision BETWEEN 1 AND 9223372036854775807),
    registered_at_ms INTEGER NOT NULL
        CHECK (registered_at_ms BETWEEN 0 AND 9223372036854775807),
    refreshed_at_ms INTEGER NOT NULL
        CHECK (refreshed_at_ms BETWEEN 0 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL
        CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    priority INTEGER NOT NULL DEFAULT 1
        CHECK (priority BETWEEN 1 AND 9223372036854775807),
    resolver_indexed INTEGER NOT NULL DEFAULT 0
        CHECK (resolver_indexed IN (0, 1)),
    credential_reference TEXT
        CHECK (credential_reference IS NULL OR
           (source_kind = 'remote' AND length(credential_reference) = 36
            AND credential_reference NOT GLOB '*[^0-9a-f-]*')),
    display_name TEXT
        CHECK (display_name IS NULL OR length(display_name) BETWEEN 1 AND 128)
) STRICT;

CREATE TABLE repository_package_versions (
    repository_id TEXT NOT NULL
        REFERENCES repositories(repository_id) ON DELETE CASCADE,
    package_id TEXT NOT NULL
        CHECK (length(package_id) BETWEEN 1 AND 1024),
    version_text TEXT NOT NULL
        CHECK (length(version_text) BETWEEN 1 AND 1024),
    display_name TEXT
        CHECK (display_name IS NULL OR length(display_name) BETWEEN 1 AND 4096),
    description TEXT
        CHECK (description IS NULL OR length(description) BETWEEN 1 AND 65536),
    yanked INTEGER NOT NULL DEFAULT 0
        CHECK (yanked IN (0, 1)),
    unity_text TEXT
        CHECK (unity_text IS NULL OR length(unity_text) BETWEEN 1 AND 1024),
    semantic_version TEXT
        CHECK (semantic_version IS NULL OR length(semantic_version) BETWEEN 1 AND 1024),
    author_name TEXT
        CHECK (author_name IS NULL OR length(author_name) BETWEEN 1 AND 4096),
    author_email TEXT
        CHECK (author_email IS NULL OR length(author_email) BETWEEN 1 AND 4096),
    artifact_url TEXT
        CHECK (artifact_url IS NULL OR length(artifact_url) BETWEEN 1 AND 32768),
    zip_sha256 TEXT
        CHECK (zip_sha256 IS NULL OR (
        length(zip_sha256) = 64
        AND zip_sha256 = lower(zip_sha256)
        AND zip_sha256 NOT GLOB '*[^0-9a-f]*'
    )),
    unity_release_text TEXT
        CHECK (unity_release_text IS NULL OR length(unity_release_text) BETWEEN 1 AND 1024),
    dependencies_json TEXT NOT NULL DEFAULT '{}'
        CHECK (json_valid(dependencies_json) AND json_type(dependencies_json) = 'object'
           AND length(dependencies_json) <= 1048576),
    manifest_fingerprint BLOB
        CHECK (manifest_fingerprint IS NULL OR length(manifest_fingerprint) = 32),
    legacy_metadata_present INTEGER NOT NULL DEFAULT 0
        CHECK (legacy_metadata_present IN (0, 1)),
    resolver_ready INTEGER NOT NULL DEFAULT 0
        CHECK (resolver_ready IN (0, 1)),
    documentation_url TEXT
        CHECK (documentation_url IS NULL OR length(CAST(documentation_url AS BLOB)) BETWEEN 1 AND 2048),
    changelog_url TEXT
        CHECK (changelog_url IS NULL OR length(CAST(changelog_url AS BLOB)) BETWEEN 1 AND 2048),
    download_headers_present INTEGER NOT NULL DEFAULT 0
        CHECK (download_headers_present IN (0, 1) AND (download_headers_present = 0 OR resolver_ready = 1)),
    local_zip_sha256 BLOB
        CHECK (local_zip_sha256 IS NULL OR (
        length(local_zip_sha256) = 32 AND resolver_ready = 1 AND zip_sha256 IS NULL
        AND (substr(artifact_url, 1, 7) = 'http://' OR substr(artifact_url, 1, 8) = 'https://')
    )),
    presentation_json TEXT
        CHECK (presentation_json IS NULL OR json_valid(presentation_json)),
    PRIMARY KEY (repository_id, package_id, version_text)
) STRICT;

CREATE TABLE unity_installations (
    installation_id TEXT PRIMARY KEY
        CHECK (length(installation_id) = 36),
    owner_principal_id TEXT NOT NULL
        CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    executable_path TEXT NOT NULL
        CHECK (length(executable_path) BETWEEN 1 AND 32768),
    filesystem_identity_key BLOB NOT NULL UNIQUE
        CHECK (length(filesystem_identity_key) BETWEEN 1 AND 128),
    unity_version TEXT NOT NULL
        CHECK (length(unity_version) BETWEEN 1 AND 128),
    architecture TEXT NOT NULL
        CHECK (architecture IN ('x86_64', 'arm64', 'universal', 'unknown')),
    source_kind TEXT NOT NULL
        CHECK (source_kind IN ('manual', 'hub_config', 'known_install_root', 'unity_cli_hint')),
    revision INTEGER NOT NULL
        CHECK (revision BETWEEN 1 AND 9223372036854775807),
    observed_at_ms INTEGER NOT NULL
        CHECK (observed_at_ms BETWEEN 0 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL
        CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807)
) STRICT;

CREATE TABLE operation_journal (
    operation_id TEXT NOT NULL REFERENCES operations(operation_id) ON DELETE CASCADE,
    step INTEGER NOT NULL CHECK (step BETWEEN 1 AND 9223372036854775807),
    kind TEXT NOT NULL CHECK (length(kind) BETWEEN 1 AND 128),
    state TEXT NOT NULL CHECK (state IN ('prepared', 'applied')),
    payload_json TEXT NOT NULL CHECK (json_valid(payload_json) AND length(payload_json) <= 65536),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    PRIMARY KEY (operation_id, step)
) STRICT;

CREATE TABLE idempotency_records (
    principal_id TEXT NOT NULL CHECK (length(principal_id) BETWEEN 1 AND 128),
    method TEXT NOT NULL CHECK (length(method) BETWEEN 3 AND 128),
    idempotency_key TEXT NOT NULL CHECK (length(idempotency_key) BETWEEN 1 AND 128),
    request_fingerprint TEXT NOT NULL CHECK (
        json_valid(request_fingerprint) AND length(request_fingerprint) <= 4096
    ),
    state TEXT NOT NULL CHECK (state IN ('pending', 'completed')),
    operation_id TEXT REFERENCES operations(operation_id),
    response_json TEXT CHECK (
        response_json IS NULL OR (json_valid(response_json) AND length(response_json) <= 65536)
    ),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms BETWEEN 0 AND 9223372036854775807),
    PRIMARY KEY (principal_id, method, idempotency_key),
    CHECK (
        (state = 'pending' AND operation_id IS NOT NULL AND response_json IS NULL)
        OR (state = 'completed' AND response_json IS NOT NULL)
    )
) STRICT;

CREATE TABLE extensions (
    extension_id TEXT PRIMARY KEY CHECK (length(extension_id) BETWEEN 3 AND 255),
    version TEXT NOT NULL CHECK (length(version) BETWEEN 1 AND 128),
    api_major INTEGER NOT NULL CHECK (api_major = 1),
    package_digest BLOB NOT NULL CHECK (length(package_digest) = 32),
    manifest_digest BLOB NOT NULL CHECK (length(manifest_digest) = 32),
    component_digest BLOB NOT NULL CHECK (length(component_digest) = 32),
    publisher_fingerprint TEXT NOT NULL CHECK (length(publisher_fingerprint) = 79),
    principal_id TEXT NOT NULL CHECK (length(principal_id) BETWEEN 1 AND 128),
    live_package_locator TEXT NOT NULL CHECK (length(live_package_locator) BETWEEN 1 AND 32768),
    desired_state TEXT NOT NULL CHECK (desired_state IN ('installed_disabled', 'enabled', 'uninstalling')),
    quarantine_state TEXT NOT NULL CHECK (quarantine_state IN ('clear', 'quarantined')),
    grant_revision INTEGER NOT NULL CHECK (grant_revision BETWEEN 1 AND 9223372036854775807),
    lifecycle_generation INTEGER NOT NULL CHECK (lifecycle_generation BETWEEN 1 AND 9223372036854775807),
    revision INTEGER NOT NULL CHECK (revision BETWEEN 1 AND 9223372036854775807),
    installed_at_ms INTEGER NOT NULL CHECK (installed_at_ms BETWEEN 0 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    ui_protocol TEXT CHECK (ui_protocol IS NULL OR ui_protocol = 'portable-v1')
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

CREATE TABLE user_package_sources (
    user_package_id TEXT PRIMARY KEY CHECK (length(user_package_id) = 36),
    owner_principal_id TEXT NOT NULL CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    source_root_path TEXT NOT NULL CHECK (length(CAST(source_root_path AS BLOB)) BETWEEN 1 AND 32768),
    source_identity_key BLOB NOT NULL CHECK (length(source_identity_key) BETWEEN 1 AND 1024),
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

CREATE TABLE project_unity_migration_plans (
    plan_id TEXT PRIMARY KEY CHECK (length(plan_id) = 36),
    owner_principal_id TEXT NOT NULL
        CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    project_id TEXT NOT NULL
        REFERENCES projects(project_id) ON DELETE RESTRICT,
    project_revision INTEGER NOT NULL
        CHECK (project_revision BETWEEN 1 AND 9223372036854775807),
    source_unity_version TEXT NOT NULL
        CHECK (length(source_unity_version) BETWEEN 8 AND 64),
    source_revision_metadata TEXT
        CHECK (source_revision_metadata IS NULL OR length(source_revision_metadata) BETWEEN 1 AND 128),
    project_root_identity BLOB NOT NULL
        CHECK (length(project_root_identity) BETWEEN 1 AND 128),
    project_snapshot_json TEXT NOT NULL
        CHECK (
            json_valid(project_snapshot_json)
            AND json_type(project_snapshot_json) = 'object'
            AND length(project_snapshot_json) BETWEEN 2 AND 65536
        ),
    project_version_marker_sha256 BLOB NOT NULL
        CHECK (length(project_version_marker_sha256) = 32),
    target_unity_version TEXT NOT NULL
        CHECK (length(target_unity_version) BETWEEN 8 AND 64),
    target_revision_metadata TEXT
        CHECK (target_revision_metadata IS NULL OR length(target_revision_metadata) BETWEEN 1 AND 128),
    target_installation_id TEXT NOT NULL
        REFERENCES unity_installations(installation_id) ON DELETE RESTRICT,
    target_installation_revision INTEGER NOT NULL
        CHECK (target_installation_revision BETWEEN 1 AND 9223372036854775807),
    target_installation_identity BLOB NOT NULL
        CHECK (length(target_installation_identity) BETWEEN 1 AND 128),
    target_installation_snapshot_json TEXT NOT NULL
        CHECK (
            json_valid(target_installation_snapshot_json)
            AND json_type(target_installation_snapshot_json) = 'object'
            AND length(target_installation_snapshot_json) BETWEEN 2 AND 65536
        ),
    writer_evidence_revision INTEGER NOT NULL
        CHECK (writer_evidence_revision BETWEEN 1 AND 9223372036854775807),
    writer_evidence_json TEXT NOT NULL
        CHECK (
            json_valid(writer_evidence_json)
            AND json_type(writer_evidence_json) = 'object'
            AND length(writer_evidence_json) BETWEEN 2 AND 65536
        ),
    classification TEXT NOT NULL
        CHECK (classification IN (
            'patch_or_minor_upgrade', 'major_upgrade',
            'patch_or_minor_downgrade', 'major_downgrade', 'china_variant_change'
        )),
    preparation_profile TEXT
        CHECK (preparation_profile IS NULL OR preparation_profile = 'vrchat-2019-to-2022-v1'),
    plan_fingerprint TEXT NOT NULL CHECK (length(plan_fingerprint) BETWEEN 1 AND 65536),
    request_fingerprint TEXT NOT NULL CHECK (length(request_fingerprint) BETWEEN 1 AND 65536),
    plan_idempotency_key TEXT NOT NULL CHECK (length(plan_idempotency_key) BETWEEN 1 AND 128),
    created_at_ms INTEGER NOT NULL
        CHECK (created_at_ms BETWEEN 0 AND 9223372036854775807),
    expires_at_ms INTEGER NOT NULL
        CHECK (expires_at_ms BETWEEN 0 AND 9223372036854775807),
    state TEXT NOT NULL CHECK (state IN ('unapplied', 'applied')),
    operation_id TEXT UNIQUE
        REFERENCES operations(operation_id) ON DELETE RESTRICT,
    CHECK (
        (state = 'unapplied' AND operation_id IS NULL)
        OR (state = 'applied' AND operation_id IS NOT NULL)
    )
) STRICT;

CREATE TABLE project_unity_migration_journal (
    operation_id TEXT NOT NULL
        REFERENCES operations(operation_id) ON DELETE CASCADE,
    step INTEGER NOT NULL CHECK (step BETWEEN 1 AND 9223372036854775807),
    phase TEXT NOT NULL CHECK (phase IN (
        'accepted', 'preflight_complete', 'preparation_intent', 'preparation_complete',
        'launch_intent', 'unity_started', 'unity_exited', 'project_reobserved',
        'state_committed', 'cleanup_complete', 'recovery_required'
    )),
    evidence_json TEXT NOT NULL
        CHECK (
            json_valid(evidence_json)
            AND json_type(evidence_json) = 'object'
            AND length(evidence_json) <= 65536
        ),
    updated_at_ms INTEGER NOT NULL
        CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    PRIMARY KEY (operation_id, step)
) STRICT;

CREATE TABLE "project_unity_launch_config" (
    project_id TEXT PRIMARY KEY
        REFERENCES projects(project_id) ON DELETE CASCADE,
    arguments_json TEXT NOT NULL
        CHECK (
            json_valid(arguments_json)
            AND json_type(arguments_json) = 'array'
            AND json_array_length(arguments_json) BETWEEN 0 AND 64
            AND length(arguments_json) <= 65536
        ),
    revision INTEGER NOT NULL
        CHECK (revision BETWEEN 1 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL
        CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807)
) STRICT;

CREATE TABLE operation_log_entries (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT CHECK(sequence BETWEEN 1 AND 9007199254740991),
    entry_id TEXT NOT NULL UNIQUE CHECK(length(entry_id) = 36),
    owner TEXT NOT NULL,
    execution_id TEXT NOT NULL CHECK(length(execution_id) = 36),
    actor TEXT NOT NULL CHECK(length(CAST(actor AS BLOB)) BETWEEN 1 AND 64),
    operation_id TEXT,
    occurred_at_ms INTEGER NOT NULL CHECK(occurred_at_ms >= 0),
    action TEXT NOT NULL CHECK(length(action) BETWEEN 1 AND 128),
    step TEXT NOT NULL CHECK(length(step) BETWEEN 1 AND 128),
    outcome TEXT NOT NULL CHECK(outcome IN ('started', 'succeeded', 'failed', 'cancelled', 'waiting')),
    target_json TEXT NOT NULL CHECK(json_valid(target_json)),
    details TEXT CHECK(details IS NULL OR length(CAST(details AS BLOB)) <= 262144)
);

CREATE TABLE project_template_revision (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    last_revision INTEGER NOT NULL CHECK (last_revision BETWEEN 0 AND 9007199254740991)
) STRICT;

CREATE TABLE events (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT
        CHECK (sequence BETWEEN 1 AND 9223372036854775807),
    event_id TEXT NOT NULL UNIQUE CHECK (length(event_id) = 36),
    kind TEXT NOT NULL CHECK (length(kind) BETWEEN 1 AND 128),
    aggregate_kind TEXT NOT NULL CHECK (aggregate_kind IN (
        'operation', 'project', 'repository', 'unity-installation',
        'project-editor-preference', 'project-unity-launch-config',
        'user-package', 'project-template'
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

CREATE TABLE repository_credential_cleanup (
    credential_reference TEXT PRIMARY KEY
        CHECK (length(credential_reference) = 36 AND credential_reference NOT GLOB '*[^0-9a-f-]*')
) STRICT;

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

CREATE TABLE mcp_clients (
    client_id TEXT PRIMARY KEY CHECK (length(client_id) = 36),
    principal_id TEXT NOT NULL UNIQUE CHECK (principal_id = 'mcp:' || client_id),
    display_name TEXT NOT NULL CHECK (length(display_name) BETWEEN 1 AND 128),
    revision INTEGER NOT NULL CHECK (revision BETWEEN 1 AND 9223372036854775807),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
    revoked_at_ms INTEGER CHECK (revoked_at_ms >= created_at_ms)
) STRICT;

CREATE TABLE mcp_client_credentials (
    credential_id TEXT PRIMARY KEY CHECK (length(credential_id) = 36),
    client_id TEXT NOT NULL REFERENCES mcp_clients(client_id),
    token_hash BLOB NOT NULL UNIQUE CHECK (length(token_hash) = 32),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
    revoked_at_ms INTEGER CHECK (revoked_at_ms >= created_at_ms)
) STRICT;

CREATE TABLE mcp_oauth_registrations (
    oauth_client_id TEXT PRIMARY KEY NOT NULL CHECK(length(oauth_client_id)=36),
    client_id TEXT NOT NULL REFERENCES mcp_clients(client_id),
    issuer TEXT NOT NULL CHECK(length(issuer) BETWEEN 1 AND 2048),
    resource TEXT NOT NULL CHECK(length(resource) BETWEEN 1 AND 2048),
    redirect_uris_json TEXT NOT NULL CHECK(json_valid(redirect_uris_json) AND length(redirect_uris_json)<=32768),
    confidential INTEGER NOT NULL CHECK(confidential IN (0,1)),
    secret_hash BLOB CHECK(secret_hash IS NULL OR length(secret_hash)=32),
    allow_refresh INTEGER NOT NULL CHECK(allow_refresh IN (0,1)),
    revision INTEGER NOT NULL CHECK(revision BETWEEN 1 AND 9223372036854775807),
    created_at_ms INTEGER NOT NULL CHECK(created_at_ms>=0),
    revoked_at_ms INTEGER CHECK(revoked_at_ms IS NULL OR revoked_at_ms>=created_at_ms),
    CHECK((confidential=1 AND secret_hash IS NOT NULL) OR (confidential=0 AND secret_hash IS NULL))
) STRICT;

CREATE TABLE mcp_oauth_tokens (
    token_hash BLOB PRIMARY KEY NOT NULL CHECK(length(token_hash)=32),
    kind TEXT NOT NULL CHECK(kind IN ('access','refresh')),
    oauth_client_id TEXT NOT NULL REFERENCES mcp_oauth_registrations(oauth_client_id),
    registration_revision INTEGER NOT NULL CHECK(registration_revision>=1),
    family_id TEXT NOT NULL CHECK(length(family_id)=36),
    parent_hash BLOB CHECK(parent_hash IS NULL OR length(parent_hash)=32),
    source_code_hash BLOB CHECK(source_code_hash IS NULL OR length(source_code_hash)=32),
    credential_generation TEXT NOT NULL CHECK(length(credential_generation) IN (0,36)),
    issuer TEXT NOT NULL CHECK(length(issuer) BETWEEN 1 AND 2048),
    resource TEXT NOT NULL CHECK(length(resource) BETWEEN 1 AND 2048),
    created_at_ms INTEGER NOT NULL CHECK(created_at_ms>=0),
    expires_at_ms INTEGER NOT NULL CHECK(expires_at_ms>created_at_ms),
    family_expires_at_ms INTEGER NOT NULL CHECK(family_expires_at_ms>=expires_at_ms),
    consumed_at_ms INTEGER CHECK(consumed_at_ms IS NULL OR consumed_at_ms>=created_at_ms),
    revoked_at_ms INTEGER CHECK(revoked_at_ms IS NULL OR revoked_at_ms>=created_at_ms),
    CHECK(kind='refresh' OR consumed_at_ms IS NULL)
) STRICT;

CREATE TABLE extension_mcp_calls (
    call_id TEXT PRIMARY KEY CHECK(length(call_id)=36),
    principal_id TEXT NOT NULL CHECK(length(principal_id) BETWEEN 5 AND 128 AND principal_id LIKE 'mcp:%'),
    tool_name TEXT NOT NULL CHECK(length(tool_name) BETWEEN 1 AND 128),
    idempotency_key TEXT NOT NULL CHECK(length(idempotency_key) BETWEEN 1 AND 128),
    request_fingerprint TEXT NOT NULL CHECK(length(request_fingerprint)=64),
    extension_id TEXT NOT NULL CHECK(length(extension_id) BETWEEN 3 AND 255),
    publisher_fingerprint TEXT NOT NULL CHECK(length(publisher_fingerprint) BETWEEN 1 AND 128),
    package_digest TEXT NOT NULL CHECK(length(package_digest)=64),
    permissions_json TEXT NOT NULL CHECK(json_valid(permissions_json) AND length(permissions_json)<=4096),
    effect_kind TEXT NOT NULL CHECK(effect_kind IN ('plan','apply','cancel')),
    state TEXT NOT NULL CHECK(state IN ('running','finished','interrupted')),
    core_effect_json TEXT CHECK(core_effect_json IS NULL OR (json_valid(core_effect_json) AND length(core_effect_json)<=65536)),
    created_at_ms INTEGER NOT NULL CHECK(created_at_ms>=0),
    updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms>=created_at_ms),
    UNIQUE(principal_id,tool_name,idempotency_key)
) STRICT;

CREATE TABLE "extension_plans" (
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

CREATE TABLE "extension_grants" (
    extension_id TEXT NOT NULL REFERENCES extensions(extension_id) ON DELETE CASCADE,
    permission_name TEXT NOT NULL CHECK(permission_name IN ('background.run','projects.read','mcp.clients.read','mcp.clients.manage','mcp.tools.provide','repositories.read','packages.read','packages.manage','operations.read','operations.cancel','mcp.connections.read','mcp.requests.read','mcp.subscription-streams.read','unity.sessions.read','local-ipc.connect')),
    resource_kind TEXT NOT NULL CHECK(resource_kind IN ('Extension','Global','LocalIpcTarget')),
    resource_id TEXT NOT NULL CHECK(length(resource_id) BETWEEN 1 AND 255),
    state TEXT NOT NULL CHECK(state IN ('granted','revoked')),
    grant_revision INTEGER NOT NULL CHECK(grant_revision BETWEEN 1 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms BETWEEN 0 AND 9223372036854775807),
    PRIMARY KEY(extension_id,permission_name,resource_kind,resource_id),
    CHECK((permission_name='background.run' AND resource_kind='Extension' AND resource_id=extension_id)
 OR (permission_name IN ('projects.read','mcp.clients.read','mcp.clients.manage','mcp.tools.provide','repositories.read','packages.read','packages.manage','operations.read','operations.cancel','mcp.connections.read','mcp.requests.read','mcp.subscription-streams.read','unity.sessions.read') AND resource_kind='Global' AND resource_id='*')
 OR (permission_name='local-ipc.connect' AND resource_kind='LocalIpcTarget' AND length(resource_id)=64 AND resource_id NOT GLOB '*[^0-9a-f]*'))
) STRICT;

CREATE INDEX projects_owner_page
    ON projects(owner_principal_id, registered_at_ms DESC, project_id DESC);

CREATE INDEX repositories_owner_page
    ON repositories(owner_principal_id, registered_at_ms DESC, repository_id DESC);

CREATE INDEX repository_package_versions_page
    ON repository_package_versions(repository_id, package_id ASC, version_text ASC);

CREATE INDEX unity_installations_owner_page
    ON unity_installations(owner_principal_id, updated_at_ms DESC, installation_id DESC);

CREATE INDEX extension_filesystem_journal_recovery
    ON extension_filesystem_journal(operation_id, step DESC);

CREATE INDEX user_package_sources_owner_page
    ON user_package_sources(owner_principal_id, updated_at_ms DESC, user_package_id DESC);

CREATE INDEX package_plans_owner_created
    ON package_plans(owner_principal_id, created_at_ms DESC, plan_id DESC);

CREATE INDEX package_plans_project_created
    ON package_plans(project_id, created_at_ms DESC, plan_id DESC);

CREATE INDEX package_filesystem_journal_recovery
    ON package_filesystem_journal(operation_id, step DESC);

CREATE INDEX project_copy_plans_owner_created
    ON project_copy_plans(owner_principal_id, created_at_ms DESC, plan_id DESC);

CREATE INDEX project_copy_filesystem_journal_recovery
    ON project_copy_filesystem_journal(operation_id, step DESC);

CREATE INDEX project_delete_plans_owner_created
    ON project_delete_plans(owner_principal_id, created_at_ms DESC, plan_id DESC);

CREATE INDEX project_delete_filesystem_journal_recovery
    ON project_delete_filesystem_journal(operation_id, step DESC);

CREATE INDEX project_unity_migration_plans_owner_project
    ON project_unity_migration_plans(owner_principal_id, project_id, created_at_ms DESC);

CREATE INDEX operation_logs_owner_sequence ON operation_log_entries(owner, sequence);

CREATE INDEX operation_logs_execution ON operation_log_entries(owner, execution_id, sequence);

CREATE INDEX operation_logs_operation ON operation_log_entries(owner, operation_id, sequence) WHERE operation_id IS NOT NULL;

CREATE INDEX events_principal_sequence ON events(principal_id,sequence);

CREATE INDEX events_aggregate_sequence ON events(aggregate_kind,aggregate_id,sequence);

CREATE UNIQUE INDEX repositories_credential_reference
    ON repositories(credential_reference) WHERE credential_reference IS NOT NULL;

CREATE INDEX operations_owner_page
    ON operations(owner_principal_id, created_at_ms DESC, operation_id DESC);

CREATE INDEX operations_recovery
    ON operations(state, created_at_ms ASC, operation_id ASC);

CREATE UNIQUE INDEX mcp_client_one_active_credential ON mcp_client_credentials(client_id)
    WHERE revoked_at_ms IS NULL;

CREATE UNIQUE INDEX mcp_oauth_active_client ON mcp_oauth_registrations(client_id) WHERE revoked_at_ms IS NULL;

CREATE UNIQUE INDEX mcp_oauth_code_once ON mcp_oauth_tokens(source_code_hash) WHERE source_code_hash IS NOT NULL;

CREATE INDEX mcp_oauth_family ON mcp_oauth_tokens(family_id);

CREATE INDEX mcp_oauth_registration_tokens ON mcp_oauth_tokens(oauth_client_id,expires_at_ms);

CREATE INDEX extension_plans_owner_created
    ON extension_plans(owner_principal_id, created_at_ms DESC, plan_id DESC);

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

CREATE TRIGGER project_copy_plans_no_delete
BEFORE DELETE ON project_copy_plans
BEGIN
    SELECT RAISE(ABORT, 'project copy plans are durable');
END;

CREATE TRIGGER project_copy_filesystem_journal_validate_insert
BEFORE INSERT ON project_copy_filesystem_journal
WHEN (SELECT kind FROM operations WHERE operation_id = NEW.operation_id) IS NOT 'projects.copy'
     OR (SELECT apply_operation_id FROM project_copy_plans WHERE plan_id = NEW.plan_id)
        IS NOT NEW.operation_id
BEGIN
    SELECT RAISE(ABORT, 'project copy journal ownership mismatch');
END;

CREATE TRIGGER project_delete_plans_no_delete
BEFORE DELETE ON project_delete_plans
BEGIN
    SELECT RAISE(ABORT, 'project delete plans are durable');
END;

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

CREATE TRIGGER package_filesystem_journal_no_delete
BEFORE DELETE ON package_filesystem_journal
BEGIN
    SELECT RAISE(ABORT, 'filesystem journal is durable');
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

CREATE TRIGGER project_delete_filesystem_journal_no_delete
BEFORE DELETE ON project_delete_filesystem_journal
BEGIN
    SELECT RAISE(ABORT, 'project delete journal is durable');
END;

CREATE TRIGGER project_unity_migration_journal_no_update
BEFORE UPDATE ON project_unity_migration_journal
BEGIN
    SELECT RAISE(ABORT, 'project unity migration journal is append-only');
END;

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

CREATE TRIGGER repository_package_versions_resolver_ready_insert
BEFORE INSERT ON repository_package_versions
WHEN NEW.resolver_ready = 1 AND (
    NEW.semantic_version IS NULL
    OR NEW.artifact_url IS NULL
    OR NEW.manifest_fingerprint IS NULL
    OR (NEW.zip_sha256 IS NULL
        AND substr(NEW.artifact_url, 1, 7) <> 'http://'
        AND substr(NEW.artifact_url, 1, 8) <> 'https://')
)
BEGIN
    SELECT RAISE(ABORT, 'resolver-ready package metadata is incomplete');
END;

CREATE TRIGGER repository_package_versions_resolver_ready_update
BEFORE UPDATE ON repository_package_versions
WHEN NEW.resolver_ready = 1 AND (
    NEW.semantic_version IS NULL
    OR NEW.artifact_url IS NULL
    OR NEW.manifest_fingerprint IS NULL
    OR (NEW.zip_sha256 IS NULL
        AND substr(NEW.artifact_url, 1, 7) <> 'http://'
        AND substr(NEW.artifact_url, 1, 8) <> 'https://')
)
BEGIN
    SELECT RAISE(ABORT, 'resolver-ready package metadata is incomplete');
END;

CREATE TRIGGER mcp_clients_capacity BEFORE INSERT ON mcp_clients
WHEN (SELECT count(*) FROM mcp_clients) >= 64
BEGIN SELECT RAISE(ABORT, 'MCP client capacity exceeded'); END;

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

INSERT INTO project_template_revision (singleton, last_revision) VALUES (1, 0);
