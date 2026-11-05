BEGIN IMMEDIATE;

DROP TRIGGER IF EXISTS activities_created;
DROP TRIGGER IF EXISTS activities_changed;
DROP TRIGGER IF EXISTS operation_activity_created;
DROP TRIGGER IF EXISTS operation_activity_required;
DROP TRIGGER IF EXISTS operation_activity_changed;
DROP TABLE IF EXISTS activities;

-- Only the withdrawn history projection is removed. Business events and their
-- identities remain unchanged; deleted history sequences are never reused.
CREATE TEMP TABLE m20_sequence AS SELECT seq FROM sqlite_sequence WHERE name='events';
DELETE FROM events WHERE aggregate_kind='activity' AND kind='activity.changed';
DROP INDEX events_principal_sequence;
DROP INDEX events_aggregate_sequence;
ALTER TABLE events RENAME TO events_before_activity_removal;
CREATE TABLE events (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT
        CHECK (sequence BETWEEN 1 AND 9223372036854775807),
    event_id TEXT NOT NULL UNIQUE CHECK (length(event_id) = 36),
    kind TEXT NOT NULL CHECK (length(kind) BETWEEN 1 AND 128),
    aggregate_kind TEXT NOT NULL CHECK (aggregate_kind IN (
        'operation', 'project', 'repository', 'unity-installation',
        'project-editor-preference', 'project-unity-launch-config',
        'template', 'user-package'
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
INSERT INTO events SELECT * FROM events_before_activity_removal ORDER BY sequence;
DROP TABLE events_before_activity_removal;
INSERT INTO sqlite_sequence(name,seq)
    SELECT 'events',0 WHERE NOT EXISTS(SELECT 1 FROM sqlite_sequence WHERE name='events');
UPDATE sqlite_sequence SET seq=max(seq, coalesce((SELECT seq FROM m20_sequence),0)) WHERE name='events';
DROP TABLE m20_sequence;
CREATE INDEX events_principal_sequence ON events(principal_id,sequence);
CREATE INDEX events_aggregate_sequence ON events(aggregate_kind,aggregate_id,sequence);
PRAGMA user_version=20;
COMMIT;
