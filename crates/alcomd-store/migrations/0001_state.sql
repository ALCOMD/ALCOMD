BEGIN IMMEDIATE;

CREATE TABLE operations (
    operation_id TEXT PRIMARY KEY
        CHECK (length(operation_id) = 36),
    kind TEXT NOT NULL
        CHECK (kind = 'state.check'),
    state TEXT NOT NULL
        CHECK (state IN (
            'queued',
            'planning',
            'waiting_for_input',
            'running',
            'cancelling',
            'succeeded',
            'failed',
            'cancelled',
            'interrupted',
            'recovering'
        )),
    revision INTEGER NOT NULL
        CHECK (revision BETWEEN 1 AND 9223372036854775807),
    owner_principal_id TEXT NOT NULL
        CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    request_json TEXT NOT NULL
        CHECK (json_valid(request_json) AND length(request_json) <= 65536),
    result_json TEXT
        CHECK (result_json IS NULL OR (json_valid(result_json) AND length(result_json) <= 65536)),
    error_code TEXT
        CHECK (error_code IS NULL OR length(error_code) BETWEEN 1 AND 128),
    diagnostic_id TEXT
        CHECK (diagnostic_id IS NULL OR length(diagnostic_id) = 36),
    cancel_requested INTEGER NOT NULL DEFAULT 0
        CHECK (cancel_requested IN (0, 1)),
    created_at_ms INTEGER NOT NULL
        CHECK (created_at_ms BETWEEN 0 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL
        CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    started_at_ms INTEGER
        CHECK (started_at_ms IS NULL OR started_at_ms BETWEEN 0 AND 9223372036854775807),
    completed_at_ms INTEGER
        CHECK (completed_at_ms IS NULL OR completed_at_ms BETWEEN 0 AND 9223372036854775807)
) STRICT;

CREATE INDEX operations_owner_page
    ON operations(owner_principal_id, created_at_ms DESC, operation_id DESC);
CREATE INDEX operations_recovery
    ON operations(state, created_at_ms ASC, operation_id ASC);

CREATE TABLE operation_journal (
    operation_id TEXT NOT NULL
        REFERENCES operations(operation_id) ON DELETE CASCADE,
    step INTEGER NOT NULL
        CHECK (step BETWEEN 1 AND 9223372036854775807),
    kind TEXT NOT NULL
        CHECK (length(kind) BETWEEN 1 AND 128),
    state TEXT NOT NULL
        CHECK (state IN ('prepared', 'applied')),
    payload_json TEXT NOT NULL
        CHECK (json_valid(payload_json) AND length(payload_json) <= 65536),
    updated_at_ms INTEGER NOT NULL
        CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    PRIMARY KEY (operation_id, step)
) STRICT;

CREATE TABLE events (
    sequence INTEGER PRIMARY KEY AUTOINCREMENT
        CHECK (sequence BETWEEN 1 AND 9223372036854775807),
    event_id TEXT NOT NULL UNIQUE
        CHECK (length(event_id) = 36),
    kind TEXT NOT NULL
        CHECK (length(kind) BETWEEN 1 AND 128),
    aggregate_kind TEXT NOT NULL
        CHECK (aggregate_kind = 'operation'),
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

CREATE INDEX events_principal_sequence
    ON events(principal_id, sequence ASC);
CREATE INDEX events_aggregate_sequence
    ON events(aggregate_kind, aggregate_id, sequence ASC);

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
    operation_id TEXT NOT NULL
        REFERENCES operations(operation_id),
    response_json TEXT
        CHECK (response_json IS NULL OR (json_valid(response_json) AND length(response_json) <= 65536)),
    created_at_ms INTEGER NOT NULL
        CHECK (created_at_ms BETWEEN 0 AND 9223372036854775807),
    PRIMARY KEY (principal_id, method, idempotency_key),
    CHECK (
        (state = 'pending' AND response_json IS NULL)
        OR (state = 'completed' AND response_json IS NOT NULL)
    )
) STRICT;

PRAGMA user_version = 1;

COMMIT;
