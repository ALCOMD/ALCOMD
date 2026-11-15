BEGIN IMMEDIATE;

-- A previous per-project approval must never silently become approval for all
-- existing and future projects. Revoke its leases before dropping those rows.
UPDATE extension_instances SET lease_cancelled=1
WHERE extension_id IN (
    SELECT extension_id FROM extension_grants WHERE permission_name='projects.read'
);
UPDATE extensions SET grant_revision=grant_revision+1, revision=revision+1
WHERE extension_id IN (
    SELECT extension_id FROM extension_grants WHERE permission_name='projects.read'
);

CREATE TABLE extension_grants_v37 (
    extension_id TEXT NOT NULL REFERENCES extensions(extension_id) ON DELETE CASCADE,
    permission_name TEXT NOT NULL CHECK (permission_name IN ('background.run', 'projects.read', 'mcp.clients.read', 'mcp.clients.manage')),
    resource_kind TEXT NOT NULL CHECK (resource_kind IN ('Extension', 'Global')),
    resource_id TEXT NOT NULL CHECK (length(resource_id) BETWEEN 1 AND 255),
    state TEXT NOT NULL CHECK (state IN ('granted', 'revoked')),
    grant_revision INTEGER NOT NULL CHECK (grant_revision BETWEEN 1 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    PRIMARY KEY (extension_id, permission_name, resource_kind, resource_id),
    CHECK (
        (permission_name='background.run' AND resource_kind='Extension' AND resource_id=extension_id)
        OR (permission_name IN ('projects.read','mcp.clients.read','mcp.clients.manage') AND resource_kind='Global' AND resource_id='*')
    )
) STRICT;
INSERT INTO extension_grants_v37
    (extension_id,permission_name,resource_kind,resource_id,state,grant_revision,updated_at_ms)
SELECT extension_id,permission_name,resource_kind,resource_id,state,grant_revision,updated_at_ms
FROM extension_grants WHERE permission_name<>'projects.read';
DROP TABLE extension_grants;
ALTER TABLE extension_grants_v37 RENAME TO extension_grants;

-- Preserve immutable historical snapshots. Only the current authority moves to
-- a new snapshot without per-project approvals; no global approval is created.
CREATE TEMP TABLE mcp_project_grants_v37 (
    client_id TEXT PRIMARY KEY,
    next_revision INTEGER NOT NULL
);
INSERT INTO mcp_project_grants_v37
SELECT c.client_id,c.revision+1 FROM mcp_clients c
WHERE EXISTS (
    SELECT 1 FROM mcp_client_grants g
    WHERE g.client_id=c.client_id AND g.grant_revision=c.grant_revision
      AND g.permission IN ('projects.read','packages.manage') AND g.resource_kind='project'
);
INSERT INTO mcp_client_grants(client_id,grant_revision,permission,resource_kind,resource_id)
SELECT g.client_id,a.next_revision,g.permission,g.resource_kind,g.resource_id
FROM mcp_project_grants_v37 a JOIN mcp_clients c USING(client_id)
JOIN mcp_client_grants g ON g.client_id=c.client_id AND g.grant_revision=c.grant_revision
WHERE NOT (g.permission IN ('projects.read','packages.manage') AND g.resource_kind='project');

UPDATE mcp_oauth_tokens SET revoked_at_ms=max(created_at_ms,CAST(strftime('%s','now') AS INTEGER)*1000)
WHERE revoked_at_ms IS NULL AND family_id IN (
    SELECT t.family_id FROM mcp_oauth_tokens t JOIN mcp_oauth_registrations r USING(oauth_client_id)
    WHERE r.client_id IN (SELECT client_id FROM mcp_project_grants_v37)
       OR EXISTS (SELECT 1 FROM json_each(t.grants_json)
                  WHERE json_extract(value,'$.resourceKind')='project')
);
UPDATE mcp_clients SET
    revision=(SELECT next_revision FROM mcp_project_grants_v37 a WHERE a.client_id=mcp_clients.client_id),
    grant_revision=(SELECT next_revision FROM mcp_project_grants_v37 a WHERE a.client_id=mcp_clients.client_id)
WHERE client_id IN (SELECT client_id FROM mcp_project_grants_v37);
DROP TABLE mcp_project_grants_v37;

PRAGMA user_version=37;
COMMIT;
