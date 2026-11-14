BEGIN IMMEDIATE;

CREATE TABLE mcp_oauth_registrations (
    oauth_client_id TEXT PRIMARY KEY NOT NULL CHECK(length(oauth_client_id)=36),
    client_id TEXT NOT NULL REFERENCES mcp_clients(client_id),
    issuer TEXT NOT NULL CHECK(length(issuer) BETWEEN 1 AND 2048),
    resource TEXT NOT NULL CHECK(length(resource) BETWEEN 1 AND 2048),
    redirect_uris_json TEXT NOT NULL CHECK(json_valid(redirect_uris_json) AND length(redirect_uris_json)<=32768),
    confidential INTEGER NOT NULL CHECK(confidential IN (0,1)),
    secret_hash BLOB CHECK(secret_hash IS NULL OR length(secret_hash)=32),
    allow_refresh INTEGER NOT NULL CHECK(allow_refresh IN (0,1)),
    revision INTEGER NOT NULL CHECK(revision BETWEEN 1 AND 9223372036854775807),
    created_at_ms INTEGER NOT NULL CHECK(created_at_ms>=0),
    revoked_at_ms INTEGER CHECK(revoked_at_ms IS NULL OR revoked_at_ms>=created_at_ms),
    CHECK((confidential=1 AND secret_hash IS NOT NULL) OR (confidential=0 AND secret_hash IS NULL))
) STRICT;
CREATE UNIQUE INDEX mcp_oauth_active_client ON mcp_oauth_registrations(client_id) WHERE revoked_at_ms IS NULL;

CREATE TABLE mcp_oauth_tokens (
    token_hash BLOB PRIMARY KEY NOT NULL CHECK(length(token_hash)=32),
    kind TEXT NOT NULL CHECK(kind IN ('access','refresh')),
    oauth_client_id TEXT NOT NULL REFERENCES mcp_oauth_registrations(oauth_client_id),
    registration_revision INTEGER NOT NULL CHECK(registration_revision>=1),
    family_id TEXT NOT NULL CHECK(length(family_id)=36),
    parent_hash BLOB CHECK(parent_hash IS NULL OR length(parent_hash)=32),
    source_code_hash BLOB CHECK(source_code_hash IS NULL OR length(source_code_hash)=32),
    credential_generation TEXT NOT NULL CHECK(length(credential_generation) IN (0,36)),
    grant_revision INTEGER NOT NULL CHECK(grant_revision>=1),
    grants_json TEXT NOT NULL CHECK(json_valid(grants_json) AND length(grants_json)<=1048576),
    issuer TEXT NOT NULL CHECK(length(issuer) BETWEEN 1 AND 2048),
    resource TEXT NOT NULL CHECK(length(resource) BETWEEN 1 AND 2048),
    created_at_ms INTEGER NOT NULL CHECK(created_at_ms>=0),
    expires_at_ms INTEGER NOT NULL CHECK(expires_at_ms>created_at_ms),
    family_expires_at_ms INTEGER NOT NULL CHECK(family_expires_at_ms>=expires_at_ms),
    consumed_at_ms INTEGER CHECK(consumed_at_ms IS NULL OR consumed_at_ms>=created_at_ms),
    revoked_at_ms INTEGER CHECK(revoked_at_ms IS NULL OR revoked_at_ms>=created_at_ms),
    CHECK(kind='refresh' OR consumed_at_ms IS NULL)
) STRICT;
CREATE UNIQUE INDEX mcp_oauth_code_once ON mcp_oauth_tokens(source_code_hash) WHERE source_code_hash IS NOT NULL;
CREATE INDEX mcp_oauth_family ON mcp_oauth_tokens(family_id);
CREATE INDEX mcp_oauth_registration_tokens ON mcp_oauth_tokens(oauth_client_id,expires_at_ms);

PRAGMA user_version=36;
COMMIT;
