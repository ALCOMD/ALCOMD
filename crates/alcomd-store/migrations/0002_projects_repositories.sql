BEGIN IMMEDIATE;

CREATE TEMP TABLE m3_events_sequence_backup (
    sequence_value INTEGER NOT NULL
) STRICT;

INSERT INTO m3_events_sequence_backup(sequence_value)
SELECT seq FROM sqlite_sequence WHERE name = 'events';

DROP INDEX events_principal_sequence;
DROP INDEX events_aggregate_sequence;
ALTER TABLE events RENAME TO events_v1;

CREATE TABLE events (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT
        CHECK (sequence BETWEEN 1 AND 9223372036854775807),
    event_id TEXT NOT NULL UNIQUE
        CHECK (length(event_id) = 36),
    kind TEXT NOT NULL
        CHECK (length(kind) BETWEEN 1 AND 128),
    aggregate_kind TEXT NOT NULL
        CHECK (aggregate_kind IN ('operation', 'project', 'repository')),
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
FROM events_v1
ORDER BY sequence;

DROP TABLE events_v1;

UPDATE sqlite_sequence
SET seq = (SELECT sequence_value FROM m3_events_sequence_backup)
WHERE name = 'events' AND EXISTS (SELECT 1 FROM m3_events_sequence_backup);

DROP TABLE m3_events_sequence_backup;

CREATE INDEX events_principal_sequence
    ON events(principal_id, sequence ASC);
CREATE INDEX events_aggregate_sequence
    ON events(aggregate_kind, aggregate_id, sequence ASC);

ALTER TABLE idempotency_records RENAME TO idempotency_records_v1;

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

INSERT INTO idempotency_records (
    principal_id, method, idempotency_key, request_fingerprint, state,
    operation_id, response_json, created_at_ms
)
SELECT principal_id, method, idempotency_key, request_fingerprint, state,
       operation_id, response_json, created_at_ms
FROM idempotency_records_v1;

DROP TABLE idempotency_records_v1;

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
    observed_at_ms INTEGER NOT NULL
        CHECK (observed_at_ms BETWEEN 0 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL
        CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807)
) STRICT;

CREATE INDEX projects_owner_page
    ON projects(owner_principal_id, registered_at_ms DESC, project_id DESC);

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
        CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807)
) STRICT;

CREATE INDEX repositories_owner_page
    ON repositories(owner_principal_id, registered_at_ms DESC, repository_id DESC);

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
    PRIMARY KEY (repository_id, package_id, version_text)
) STRICT;

CREATE INDEX repository_package_versions_page
    ON repository_package_versions(repository_id, package_id ASC, version_text ASC);

PRAGMA user_version = 2;

COMMIT;
