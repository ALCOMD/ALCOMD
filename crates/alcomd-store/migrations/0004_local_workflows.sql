BEGIN IMMEDIATE;

CREATE TEMP TABLE m5_events_sequence_backup (
    sequence_value INTEGER NOT NULL
) STRICT;

INSERT INTO m5_events_sequence_backup(sequence_value)
SELECT seq FROM sqlite_sequence WHERE name = 'events';

DROP INDEX events_principal_sequence;
DROP INDEX events_aggregate_sequence;
ALTER TABLE events RENAME TO events_v3;

CREATE TABLE events (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT
        CHECK (sequence BETWEEN 1 AND 9223372036854775807),
    event_id TEXT NOT NULL UNIQUE
        CHECK (length(event_id) = 36),
    kind TEXT NOT NULL
        CHECK (length(kind) BETWEEN 1 AND 128),
    aggregate_kind TEXT NOT NULL
        CHECK (aggregate_kind IN (
            'operation', 'project', 'repository',
            'unity-installation', 'project-editor-preference', 'template'
        )),
    aggregate_id TEXT NOT NULL
        CHECK (length(aggregate_id) = 36),
    aggregate_revision INTEGER NOT NULL
        CHECK (aggregate_revision BETWEEN 1 AND 9223372036854775807),
    principal_id TEXT NOT NULL
        CHECK (length(principal_id) BETWEEN 1 AND 128),
    occurred_at_ms INTEGER NOT NULL
        CHECK (occurred_at_ms BETWEEN 0 AND 9223372036854775807),
    payload_json TEXT NOT NULL
        CHECK (json_valid(payload_json) AND length(payload_json) <= 65536)
) STRICT;

INSERT INTO events (
    sequence, event_id, kind, aggregate_kind, aggregate_id, aggregate_revision,
    principal_id, occurred_at_ms, payload_json
)
SELECT sequence, event_id, kind, aggregate_kind, aggregate_id, aggregate_revision,
       principal_id, occurred_at_ms, payload_json
FROM events_v3
ORDER BY sequence;

DROP TABLE events_v3;

UPDATE sqlite_sequence
SET seq = (SELECT sequence_value FROM m5_events_sequence_backup)
WHERE name = 'events' AND EXISTS (SELECT 1 FROM m5_events_sequence_backup);

DROP TABLE m5_events_sequence_backup;

CREATE INDEX events_principal_sequence
    ON events(principal_id, sequence ASC);
CREATE INDEX events_aggregate_sequence
    ON events(aggregate_kind, aggregate_id, sequence ASC);

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

CREATE INDEX unity_installations_owner_page
    ON unity_installations(owner_principal_id, updated_at_ms DESC, installation_id DESC);

CREATE TABLE project_editor_preferences (
    project_id TEXT PRIMARY KEY
        REFERENCES projects(project_id) ON DELETE CASCADE,
    installation_id TEXT NOT NULL
        REFERENCES unity_installations(installation_id) ON DELETE RESTRICT,
    arguments_json TEXT NOT NULL
        CHECK (
            json_valid(arguments_json)
            AND json_type(arguments_json) = 'array'
            AND json_array_length(arguments_json) <= 64
            AND length(arguments_json) <= 65536
        ),
    revision INTEGER NOT NULL
        CHECK (revision BETWEEN 1 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL
        CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807)
) STRICT;

CREATE INDEX project_editor_preferences_installation
    ON project_editor_preferences(installation_id, project_id);

CREATE TABLE templates (
    template_id TEXT PRIMARY KEY
        CHECK (length(template_id) = 36),
    owner_principal_id TEXT NOT NULL
        CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    source_kind TEXT NOT NULL
        CHECK (source_kind IN ('builtin', 'user')),
    template_version TEXT NOT NULL
        CHECK (length(template_version) BETWEEN 1 AND 128),
    manifest_json TEXT NOT NULL
        CHECK (
            json_valid(manifest_json)
            AND json_type(manifest_json) = 'object'
            AND length(manifest_json) <= 1048576
        ),
    payload_locator TEXT NOT NULL UNIQUE
        CHECK (length(payload_locator) BETWEEN 1 AND 32768),
    payload_sha256 BLOB NOT NULL
        CHECK (length(payload_sha256) = 32),
    favorite INTEGER NOT NULL DEFAULT 0
        CHECK (favorite IN (0, 1)),
    revision INTEGER NOT NULL
        CHECK (revision BETWEEN 1 AND 9223372036854775807),
    created_at_ms INTEGER NOT NULL
        CHECK (created_at_ms BETWEEN 0 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL
        CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807)
) STRICT;

CREATE INDEX templates_owner_page
    ON templates(owner_principal_id, updated_at_ms DESC, template_id DESC);

CREATE TABLE backups (
    backup_id TEXT PRIMARY KEY
        CHECK (length(backup_id) = 36),
    owner_principal_id TEXT NOT NULL
        CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    source_project_id TEXT
        CHECK (source_project_id IS NULL OR length(source_project_id) = 36),
    archive_locator TEXT NOT NULL UNIQUE
        CHECK (length(archive_locator) BETWEEN 1 AND 32768),
    file_identity_key BLOB NOT NULL
        CHECK (length(file_identity_key) BETWEEN 1 AND 128),
    archive_sha256 BLOB NOT NULL
        CHECK (length(archive_sha256) = 32),
    byte_size INTEGER NOT NULL
        CHECK (byte_size BETWEEN 0 AND 9223372036854775807),
    format_version INTEGER NOT NULL
        CHECK (format_version BETWEEN 1 AND 2147483647),
    created_at_ms INTEGER NOT NULL
        CHECK (created_at_ms BETWEEN 0 AND 9223372036854775807),
    compression_mode TEXT NOT NULL
        CHECK (compression_mode IN ('store', 'fast', 'maximum')),
    exclude_vpm_packages INTEGER NOT NULL
        CHECK (exclude_vpm_packages IN (0, 1))
) STRICT;

CREATE INDEX backups_owner_page
    ON backups(owner_principal_id, created_at_ms DESC, backup_id DESC);

PRAGMA user_version = 4;

COMMIT;
