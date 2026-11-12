BEGIN IMMEDIATE;

CREATE TABLE mcp_clients (
    client_id TEXT PRIMARY KEY CHECK (length(client_id) = 36),
    principal_id TEXT NOT NULL UNIQUE CHECK (principal_id = 'mcp:' || client_id),
    display_name TEXT NOT NULL CHECK (length(display_name) BETWEEN 1 AND 128),
    revision INTEGER NOT NULL CHECK (revision BETWEEN 1 AND 9223372036854775807),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
    revoked_at_ms INTEGER CHECK (revoked_at_ms >= created_at_ms)
) STRICT;

CREATE TRIGGER mcp_clients_capacity BEFORE INSERT ON mcp_clients
WHEN (SELECT count(*) FROM mcp_clients) >= 64
BEGIN SELECT RAISE(ABORT, 'MCP client capacity exceeded'); END;

CREATE TABLE mcp_client_grants (
    client_id TEXT NOT NULL REFERENCES mcp_clients(client_id),
    permission TEXT NOT NULL CHECK (length(permission) BETWEEN 1 AND 128),
    resource_kind TEXT NOT NULL CHECK (length(resource_kind) BETWEEN 1 AND 64),
    resource_id TEXT NOT NULL CHECK (length(resource_id) BETWEEN 1 AND 128),
    PRIMARY KEY (client_id, permission, resource_kind, resource_id)
) STRICT;

CREATE TRIGGER mcp_client_grants_capacity BEFORE INSERT ON mcp_client_grants
WHEN (SELECT count(*) FROM mcp_client_grants WHERE client_id = NEW.client_id) >= 4096
    OR (NOT EXISTS (SELECT 1 FROM mcp_client_grants WHERE client_id = NEW.client_id AND permission = NEW.permission)
        AND (SELECT count(DISTINCT permission) FROM mcp_client_grants WHERE client_id = NEW.client_id) >= 64)
BEGIN SELECT RAISE(ABORT, 'MCP grant capacity exceeded'); END;

CREATE TABLE mcp_client_credentials (
    credential_id TEXT PRIMARY KEY CHECK (length(credential_id) = 36),
    client_id TEXT NOT NULL REFERENCES mcp_clients(client_id),
    token_hash BLOB NOT NULL UNIQUE CHECK (length(token_hash) = 32),
    created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
    revoked_at_ms INTEGER CHECK (revoked_at_ms >= created_at_ms)
) STRICT;

CREATE UNIQUE INDEX mcp_client_one_active_credential ON mcp_client_credentials(client_id)
    WHERE revoked_at_ms IS NULL;

PRAGMA user_version = 33;
COMMIT;
