BEGIN IMMEDIATE;
CREATE TABLE extension_grants_v45 (
 extension_id TEXT NOT NULL REFERENCES extensions(extension_id) ON DELETE CASCADE,
 permission_name TEXT NOT NULL CHECK(permission_name IN ('background.run','projects.read','mcp.clients.read','mcp.clients.manage','mcp.tools.provide','repositories.read','packages.read','packages.manage','operations.read','operations.cancel','mcp.connections.read','mcp.requests.read','mcp.subscription-streams.read','unity.sessions.read','local-ipc.connect')),
 resource_kind TEXT NOT NULL CHECK(resource_kind IN ('Extension','Global','LocalIpcTarget')),
 resource_id TEXT NOT NULL CHECK(length(resource_id) BETWEEN 1 AND 255),
 state TEXT NOT NULL CHECK(state IN ('granted','revoked')),
 grant_revision INTEGER NOT NULL CHECK(grant_revision BETWEEN 1 AND 9223372036854775807),
 updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms BETWEEN 0 AND 9223372036854775807),
 PRIMARY KEY(extension_id,permission_name,resource_kind,resource_id),
 CHECK((permission_name='background.run' AND resource_kind='Extension' AND resource_id=extension_id)
 OR (permission_name IN ('projects.read','mcp.clients.read','mcp.clients.manage','mcp.tools.provide','repositories.read','packages.read','packages.manage','operations.read','operations.cancel','mcp.connections.read','mcp.requests.read','mcp.subscription-streams.read','unity.sessions.read') AND resource_kind='Global' AND resource_id='*')
 OR (permission_name='local-ipc.connect' AND resource_kind='LocalIpcTarget' AND length(resource_id)=64 AND resource_id NOT GLOB '*[^0-9a-f]*'))
) STRICT;
INSERT INTO extension_grants_v45 SELECT * FROM extension_grants;
DROP TABLE extension_grants;
ALTER TABLE extension_grants_v45 RENAME TO extension_grants;
PRAGMA user_version=45;
COMMIT;
