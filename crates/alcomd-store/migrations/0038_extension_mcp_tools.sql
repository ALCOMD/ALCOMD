BEGIN IMMEDIATE;
CREATE TABLE extension_grants_v38 (
 extension_id TEXT NOT NULL REFERENCES extensions(extension_id) ON DELETE CASCADE,
 permission_name TEXT NOT NULL CHECK(permission_name IN ('background.run','projects.read','mcp.clients.read','mcp.clients.manage','mcp.tools.provide','repositories.read','packages.read','packages.manage','operations.read','operations.cancel')),
 resource_kind TEXT NOT NULL CHECK(resource_kind IN ('Extension','Global')),
 resource_id TEXT NOT NULL CHECK(length(resource_id) BETWEEN 1 AND 255),
 state TEXT NOT NULL CHECK(state IN ('granted','revoked')),
 grant_revision INTEGER NOT NULL CHECK(grant_revision BETWEEN 1 AND 9223372036854775807),
 updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms BETWEEN 0 AND 9223372036854775807),
 PRIMARY KEY(extension_id,permission_name,resource_kind,resource_id),
 CHECK((permission_name='background.run' AND resource_kind='Extension' AND resource_id=extension_id)
 OR (permission_name IN ('projects.read','mcp.clients.read','mcp.clients.manage','mcp.tools.provide','repositories.read','packages.read','packages.manage','operations.read','operations.cancel') AND resource_kind='Global' AND resource_id='*'))
) STRICT;
INSERT INTO extension_grants_v38 SELECT * FROM extension_grants;
DROP TABLE extension_grants;
ALTER TABLE extension_grants_v38 RENAME TO extension_grants;
CREATE TABLE extension_mcp_calls (
 call_id TEXT PRIMARY KEY CHECK(length(call_id)=36),
 principal_id TEXT NOT NULL CHECK(length(principal_id) BETWEEN 5 AND 128 AND principal_id LIKE 'mcp:%'),
 tool_name TEXT NOT NULL CHECK(length(tool_name) BETWEEN 1 AND 128),
 idempotency_key TEXT NOT NULL CHECK(length(idempotency_key) BETWEEN 1 AND 128),
 request_fingerprint TEXT NOT NULL CHECK(length(request_fingerprint)=64),
 extension_id TEXT NOT NULL CHECK(length(extension_id) BETWEEN 3 AND 255),
 publisher_fingerprint TEXT NOT NULL CHECK(length(publisher_fingerprint) BETWEEN 1 AND 128),
 package_digest TEXT NOT NULL CHECK(length(package_digest)=64),
 permissions_json TEXT NOT NULL CHECK(json_valid(permissions_json) AND length(permissions_json)<=4096),
 effect_kind TEXT NOT NULL CHECK(effect_kind IN ('plan','apply','cancel')),
 state TEXT NOT NULL CHECK(state IN ('running','finished','interrupted')),
 core_effect_json TEXT CHECK(core_effect_json IS NULL OR (json_valid(core_effect_json) AND length(core_effect_json)<=65536)),
 created_at_ms INTEGER NOT NULL CHECK(created_at_ms>=0),
 updated_at_ms INTEGER NOT NULL CHECK(updated_at_ms>=created_at_ms),
 UNIQUE(principal_id,tool_name,idempotency_key)
) STRICT;
PRAGMA user_version=38;
COMMIT;
