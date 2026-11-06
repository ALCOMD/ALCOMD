BEGIN IMMEDIATE;
ALTER TABLE repositories ADD COLUMN credential_reference TEXT
    CHECK (credential_reference IS NULL OR
           (source_kind = 'remote' AND length(credential_reference) = 36
            AND credential_reference NOT GLOB '*[^0-9a-f-]*'));
CREATE UNIQUE INDEX repositories_credential_reference
    ON repositories(credential_reference) WHERE credential_reference IS NOT NULL;
CREATE TABLE repository_credential_cleanup (
    credential_reference TEXT PRIMARY KEY
        CHECK (length(credential_reference) = 36 AND credential_reference NOT GLOB '*[^0-9a-f-]*')
) STRICT;
PRAGMA user_version = 25;
COMMIT;
