BEGIN IMMEDIATE;

ALTER TABLE mcp_clients ADD COLUMN grant_revision INTEGER NOT NULL DEFAULT 1
    CHECK (grant_revision BETWEEN 1 AND 9223372036854775807);

DROP TRIGGER mcp_client_grants_capacity;
ALTER TABLE mcp_client_grants RENAME TO mcp_client_grants_v33;
CREATE TABLE mcp_client_grants (
    client_id TEXT NOT NULL REFERENCES mcp_clients(client_id),
    grant_revision INTEGER NOT NULL CHECK (grant_revision BETWEEN 1 AND 9223372036854775807),
    permission TEXT NOT NULL CHECK (length(permission) BETWEEN 1 AND 128),
    resource_kind TEXT NOT NULL CHECK (length(resource_kind) BETWEEN 1 AND 64),
    resource_id TEXT NOT NULL CHECK (length(resource_id) BETWEEN 1 AND 128),
    PRIMARY KEY (client_id, grant_revision, permission, resource_kind, resource_id)
) STRICT;
INSERT INTO mcp_client_grants(client_id,grant_revision,permission,resource_kind,resource_id)
SELECT client_id,1,permission,resource_kind,resource_id FROM mcp_client_grants_v33;
DROP TABLE mcp_client_grants_v33;

CREATE TRIGGER mcp_client_grants_capacity BEFORE INSERT ON mcp_client_grants
WHEN (SELECT count(*) FROM mcp_client_grants WHERE client_id=NEW.client_id AND grant_revision=NEW.grant_revision) >= 4096
    OR (NOT EXISTS (SELECT 1 FROM mcp_client_grants WHERE client_id=NEW.client_id AND grant_revision=NEW.grant_revision AND permission=NEW.permission)
        AND (SELECT count(DISTINCT permission) FROM mcp_client_grants WHERE client_id=NEW.client_id AND grant_revision=NEW.grant_revision) >= 64)
BEGIN SELECT RAISE(ABORT, 'MCP grant snapshot capacity exceeded'); END;

CREATE TRIGGER mcp_client_grant_snapshot_immutable_update BEFORE UPDATE ON mcp_client_grants
BEGIN SELECT RAISE(ABORT, 'MCP grant snapshots are immutable'); END;
CREATE TRIGGER mcp_client_grant_snapshot_immutable_delete BEFORE DELETE ON mcp_client_grants
BEGIN SELECT RAISE(ABORT, 'MCP grant snapshots are immutable'); END;

PRAGMA user_version=34;
COMMIT;
