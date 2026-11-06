BEGIN IMMEDIATE;

ALTER TABLE repositories ADD COLUMN resolver_indexed INTEGER NOT NULL DEFAULT 0
    CHECK (resolver_indexed IN (0, 1));

ALTER TABLE repository_package_versions ADD COLUMN local_zip_sha256 BLOB
    CHECK (local_zip_sha256 IS NULL OR (
        length(local_zip_sha256) = 32 AND resolver_ready = 1 AND zip_sha256 IS NULL
        AND substr(artifact_url, 1, 8) = 'https://'
    ));

DROP TRIGGER repository_package_versions_resolver_ready_insert;
DROP TRIGGER repository_package_versions_resolver_ready_update;

CREATE TRIGGER repository_package_versions_resolver_ready_insert
BEFORE INSERT ON repository_package_versions
WHEN NEW.resolver_ready = 1 AND (
    NEW.semantic_version IS NULL
    OR NEW.display_name IS NULL
    OR NEW.author_name IS NULL
    OR NEW.artifact_url IS NULL
    OR NEW.manifest_fingerprint IS NULL
    OR (NEW.zip_sha256 IS NULL AND substr(NEW.artifact_url, 1, 8) <> 'https://')
)
BEGIN
    SELECT RAISE(ABORT, 'resolver-ready package metadata is incomplete');
END;

CREATE TRIGGER repository_package_versions_resolver_ready_update
BEFORE UPDATE ON repository_package_versions
WHEN NEW.resolver_ready = 1 AND (
    NEW.semantic_version IS NULL
    OR NEW.display_name IS NULL
    OR NEW.author_name IS NULL
    OR NEW.artifact_url IS NULL
    OR NEW.manifest_fingerprint IS NULL
    OR (NEW.zip_sha256 IS NULL AND substr(NEW.artifact_url, 1, 8) <> 'https://')
)
BEGIN
    SELECT RAISE(ABORT, 'resolver-ready package metadata is incomplete');
END;

PRAGMA user_version = 24;
COMMIT;
