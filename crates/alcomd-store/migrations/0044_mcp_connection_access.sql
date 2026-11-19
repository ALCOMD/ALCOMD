BEGIN IMMEDIATE;
DROP TABLE mcp_client_grants;
ALTER TABLE mcp_clients DROP COLUMN grant_revision;
ALTER TABLE mcp_oauth_tokens DROP COLUMN grant_revision;
ALTER TABLE mcp_oauth_tokens DROP COLUMN grants_json;
PRAGMA user_version = 44;
COMMIT;
