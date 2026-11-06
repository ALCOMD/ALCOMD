BEGIN IMMEDIATE;
ALTER TABLE repository_package_versions ADD COLUMN download_headers_present INTEGER NOT NULL DEFAULT 0
    CHECK (download_headers_present IN (0, 1) AND (download_headers_present = 0 OR resolver_ready = 1));
-- A previous index cannot tell whether a manifest declared download headers.
UPDATE repositories SET resolver_indexed = 0, etag = NULL, last_modified = NULL
WHERE resolver_indexed = 1 AND EXISTS (
    SELECT 1 FROM repository_package_versions p WHERE p.repository_id = repositories.repository_id
);
PRAGMA user_version = 26;
COMMIT;
