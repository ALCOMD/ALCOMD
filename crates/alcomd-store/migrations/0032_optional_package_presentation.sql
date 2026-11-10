BEGIN IMMEDIATE;

DROP TRIGGER repository_package_versions_resolver_ready_insert;
DROP TRIGGER repository_package_versions_resolver_ready_update;

CREATE TRIGGER repository_package_versions_resolver_ready_insert
BEFORE INSERT ON repository_package_versions
WHEN NEW.resolver_ready = 1 AND (
    NEW.semantic_version IS NULL
    OR NEW.artifact_url IS NULL
    OR NEW.manifest_fingerprint IS NULL
    OR (NEW.zip_sha256 IS NULL
        AND substr(NEW.artifact_url, 1, 7) <> 'http://'
        AND substr(NEW.artifact_url, 1, 8) <> 'https://')
)
BEGIN
    SELECT RAISE(ABORT, 'resolver-ready package metadata is incomplete');
END;

CREATE TRIGGER repository_package_versions_resolver_ready_update
BEFORE UPDATE ON repository_package_versions
WHEN NEW.resolver_ready = 1 AND (
    NEW.semantic_version IS NULL
    OR NEW.artifact_url IS NULL
    OR NEW.manifest_fingerprint IS NULL
    OR (NEW.zip_sha256 IS NULL
        AND substr(NEW.artifact_url, 1, 7) <> 'http://'
        AND substr(NEW.artifact_url, 1, 8) <> 'https://')
)
BEGIN
    SELECT RAISE(ABORT, 'resolver-ready package metadata is incomplete');
END;

-- Retry only repositories whose former parser rejected version metadata.
-- Existing rows, archive pins and configuration remain intact until normal refresh.
UPDATE repositories SET resolver_indexed = 0, etag = NULL, last_modified = NULL
WHERE EXISTS (
    SELECT 1 FROM json_each(repositories.issues_json) issue
    WHERE json_extract(issue.value, '$.code') = 'resolver_metadata_invalid'
);

PRAGMA user_version = 32;
COMMIT;
