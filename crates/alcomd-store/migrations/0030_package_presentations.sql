BEGIN IMMEDIATE;
ALTER TABLE repository_package_versions ADD COLUMN presentation_json TEXT
    CHECK (presentation_json IS NULL OR json_valid(presentation_json));
PRAGMA user_version = 30;
COMMIT;
