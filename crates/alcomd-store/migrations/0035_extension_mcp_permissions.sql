BEGIN IMMEDIATE;

CREATE TABLE extension_grants_v35 (
    extension_id TEXT NOT NULL REFERENCES extensions(extension_id) ON DELETE CASCADE,
    permission_name TEXT NOT NULL CHECK (permission_name IN ('background.run', 'projects.read', 'mcp.clients.read', 'mcp.clients.manage')),
    resource_kind TEXT NOT NULL CHECK (resource_kind IN ('Extension', 'Project', 'Global')),
    resource_id TEXT NOT NULL CHECK (length(resource_id) BETWEEN 1 AND 255),
    state TEXT NOT NULL CHECK (state IN ('granted', 'revoked')),
    grant_revision INTEGER NOT NULL CHECK (grant_revision BETWEEN 1 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    PRIMARY KEY (extension_id, permission_name, resource_kind, resource_id),
    CHECK (
        (permission_name = 'background.run' AND resource_kind = 'Extension' AND resource_id = extension_id)
        OR (permission_name = 'projects.read' AND resource_kind = 'Project' AND length(resource_id) = 36)
        OR (permission_name IN ('mcp.clients.read', 'mcp.clients.manage') AND resource_kind = 'Global' AND resource_id = '*')
    )
) STRICT;

INSERT INTO extension_grants_v35
    (extension_id, permission_name, resource_kind, resource_id, state, grant_revision, updated_at_ms)
SELECT extension_id, permission_name, resource_kind, resource_id, state, grant_revision, updated_at_ms
FROM extension_grants;
DROP TABLE extension_grants;
ALTER TABLE extension_grants_v35 RENAME TO extension_grants;

PRAGMA user_version=35;
COMMIT;
