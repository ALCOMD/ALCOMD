BEGIN IMMEDIATE;

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
CREATE INDEX operation_logs_owner_sequence ON operation_log_entries(owner, sequence);
CREATE INDEX operation_logs_execution ON operation_log_entries(owner, execution_id, sequence);
CREATE INDEX operation_logs_operation ON operation_log_entries(owner, operation_id, sequence) WHERE operation_id IS NOT NULL;

PRAGMA user_version = 21;
COMMIT;
