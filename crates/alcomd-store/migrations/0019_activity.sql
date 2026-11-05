BEGIN IMMEDIATE;

-- The original Events stream remains authoritative and keeps its sequence.
CREATE TEMP TABLE m19_sequence AS SELECT seq FROM sqlite_sequence WHERE name='events';
DROP INDEX events_principal_sequence;
DROP INDEX events_aggregate_sequence;
ALTER TABLE events RENAME TO events_v18;
CREATE TABLE events (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT
        CHECK (sequence BETWEEN 1 AND 9223372036854775807),
    event_id TEXT NOT NULL UNIQUE CHECK (length(event_id) = 36),
    kind TEXT NOT NULL CHECK (length(kind) BETWEEN 1 AND 128),
    aggregate_kind TEXT NOT NULL CHECK (aggregate_kind IN (
        'operation', 'project', 'repository', 'unity-installation',
        'project-editor-preference', 'project-unity-launch-config',
        'template', 'user-package', 'activity'
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
INSERT INTO events SELECT * FROM events_v18 ORDER BY sequence;
DROP TABLE events_v18;
UPDATE sqlite_sequence SET seq=max(seq, coalesce((SELECT seq FROM m19_sequence),0)) WHERE name='events';
DROP TABLE m19_sequence;
CREATE INDEX events_principal_sequence ON events(principal_id, sequence);
CREATE INDEX events_aggregate_sequence ON events(aggregate_kind,aggregate_id,sequence);

-- No backfill: this model begins with new execution in the unpublished product.
CREATE TABLE activities (
    activity_id TEXT PRIMARY KEY CHECK(length(activity_id)=36),
    owner_principal_id TEXT NOT NULL CHECK(length(owner_principal_id) BETWEEN 1 AND 128),
    revision INTEGER NOT NULL CHECK(revision BETWEEN 1 AND 9223372036854775807),
    action TEXT NOT NULL CHECK(length(action) BETWEEN 3 AND 128),
    status TEXT NOT NULL CHECK(status IN ('queued','planning','waiting_for_input','running','cancelling','succeeded','failed','cancelled','interrupted','recovering')),
    started_at_ms INTEGER NOT NULL CHECK(started_at_ms>=0),
    finished_at_ms INTEGER CHECK(finished_at_ms IS NULL OR finished_at_ms>=started_at_ms),
    updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms>=0),
    subject_json TEXT NOT NULL CHECK(json_valid(subject_json) AND json_type(subject_json)='object' AND length(CAST(subject_json AS BLOB))<=4096),
    parameters_json TEXT NOT NULL CHECK(json_valid(parameters_json) AND json_type(parameters_json)='object' AND length(CAST(parameters_json AS BLOB))<=8192),
    result_json TEXT CHECK(result_json IS NULL OR (json_valid(result_json) AND json_type(result_json)='object' AND length(CAST(result_json AS BLOB))<=1024)),
    operation_id TEXT UNIQUE CHECK(operation_id IS NULL OR length(operation_id)=36),
    diagnostic_id TEXT CHECK(diagnostic_id IS NULL OR length(diagnostic_id)=36)
) STRICT;
CREATE INDEX activities_owner_page ON activities(owner_principal_id,started_at_ms DESC,activity_id DESC);

CREATE TRIGGER activities_created AFTER INSERT ON activities BEGIN
    INSERT INTO events(event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,principal_id,occurred_at_ms,payload_json)
    VALUES (lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(6))),'activity.changed','activity',NEW.activity_id,NEW.revision,NEW.owner_principal_id,NEW.updated_at_ms,'{}');
END;
CREATE TRIGGER activities_changed AFTER UPDATE OF revision ON activities WHEN NEW.revision<>OLD.revision BEGIN
    INSERT INTO events(event_id,kind,aggregate_kind,aggregate_id,aggregate_revision,principal_id,occurred_at_ms,payload_json)
    VALUES (lower(hex(randomblob(4)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(2)) || '-' || hex(randomblob(6))),'activity.changed','activity',NEW.activity_id,NEW.revision,NEW.owner_principal_id,NEW.updated_at_ms,'{}');
END;

-- A task and its user history are always changed in the same transaction,
-- including every existing recovery, cancellation and terminal path.
CREATE TRIGGER operation_activity_created AFTER INSERT ON operations BEGIN
    INSERT INTO activities(activity_id,owner_principal_id,revision,action,status,started_at_ms,finished_at_ms,updated_at_ms,subject_json,parameters_json,result_json,operation_id,diagnostic_id)
    VALUES(NEW.operation_id,NEW.owner_principal_id,1,NEW.kind,NEW.state,NEW.created_at_ms,
        CASE WHEN NEW.state IN ('succeeded','failed','cancelled') THEN max(NEW.created_at_ms,coalesce(NEW.completed_at_ms,NEW.updated_at_ms)) END,
        NEW.updated_at_ms,json_object('kind',CASE WHEN instr(NEW.kind,'.')>0 THEN substr(NEW.kind,1,instr(NEW.kind,'.')-1) ELSE NEW.kind END),'{}',
        CASE WHEN NEW.state IN ('succeeded','failed','cancelled') THEN json_object('code',coalesce(NEW.error_code,NEW.kind||'.'||NEW.state)) END,
        NEW.operation_id,NEW.diagnostic_id);
END;
CREATE TRIGGER operation_activity_required BEFORE UPDATE OF state,error_code,diagnostic_id,result_json ON operations
WHEN (NEW.state IS NOT OLD.state OR NEW.error_code IS NOT OLD.error_code OR NEW.diagnostic_id IS NOT OLD.diagnostic_id OR NEW.result_json IS NOT OLD.result_json)
 AND NOT EXISTS(SELECT 1 FROM activities WHERE operation_id=NEW.operation_id AND owner_principal_id=NEW.owner_principal_id)
BEGIN SELECT RAISE(ABORT,'task activity is missing'); END;
CREATE TRIGGER operation_activity_changed AFTER UPDATE OF state,error_code,diagnostic_id,result_json ON operations
WHEN NEW.state IS NOT OLD.state OR NEW.error_code IS NOT OLD.error_code OR NEW.diagnostic_id IS NOT OLD.diagnostic_id OR NEW.result_json IS NOT OLD.result_json
BEGIN
    UPDATE activities SET revision=revision+1,status=NEW.state,updated_at_ms=NEW.updated_at_ms,
        finished_at_ms=CASE WHEN NEW.state IN ('succeeded','failed','cancelled') THEN max(NEW.created_at_ms,coalesce(NEW.completed_at_ms,NEW.updated_at_ms)) END,
        result_json=CASE WHEN NEW.state IN ('succeeded','failed','cancelled') THEN json_object('code',coalesce(NEW.error_code,NEW.kind||'.'||NEW.state)) END,
        diagnostic_id=NEW.diagnostic_id
    WHERE operation_id=NEW.operation_id AND owner_principal_id=NEW.owner_principal_id;
END;
PRAGMA user_version=19;
COMMIT;
