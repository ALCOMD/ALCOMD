BEGIN IMMEDIATE;

CREATE TABLE project_templates (
    owner_principal_id TEXT NOT NULL CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    template_id TEXT NOT NULL CHECK (length(CAST(template_id AS BLOB)) BETWEEN 1 AND 128),
    aggregate_id TEXT NOT NULL UNIQUE CHECK (length(aggregate_id) = 36),
    declaration_json TEXT NOT NULL CHECK (
        length(CAST(declaration_json AS BLOB)) <= 24576
        AND json_valid(declaration_json)
        AND json_type(declaration_json) = 'object'
        AND json_type(declaration_json, '$.id') = 'text'
        AND json_extract(declaration_json, '$.id') = template_id
    ),
    revision INTEGER NOT NULL CHECK (revision BETWEEN 1 AND 9007199254740991),
    locked INTEGER NOT NULL CHECK (locked IN (0, 1)),
    PRIMARY KEY (owner_principal_id, template_id)
) STRICT;

CREATE TABLE project_template_revision (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    last_revision INTEGER NOT NULL CHECK (last_revision BETWEEN 0 AND 9007199254740991)
) STRICT;
INSERT INTO project_template_revision VALUES (1, 0);

CREATE TEMP TABLE m23_event_sequence AS SELECT seq FROM sqlite_sequence WHERE name='events';
DROP INDEX events_principal_sequence;
DROP INDEX events_aggregate_sequence;
ALTER TABLE events RENAME TO events_before_project_templates;

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

INSERT INTO events SELECT * FROM events_before_project_templates ORDER BY sequence;
DROP TABLE events_before_project_templates;
INSERT INTO sqlite_sequence(name,seq)
    SELECT 'events',0 WHERE NOT EXISTS(SELECT 1 FROM sqlite_sequence WHERE name='events');
UPDATE sqlite_sequence
SET seq=max(seq, coalesce((SELECT seq FROM m23_event_sequence),0))
WHERE name='events';
DROP TABLE m23_event_sequence;

CREATE INDEX events_principal_sequence ON events(principal_id,sequence);
CREATE INDEX events_aggregate_sequence ON events(aggregate_kind,aggregate_id,sequence);

PRAGMA user_version = 23;
COMMIT;
