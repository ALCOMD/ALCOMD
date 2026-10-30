BEGIN IMMEDIATE;

-- Existing observation/database times do not prove a project modification.
ALTER TABLE projects ADD COLUMN last_modified_at_ms INTEGER
    CHECK (last_modified_at_ms IS NULL OR last_modified_at_ms BETWEEN 0 AND 9223372036854775807);

PRAGMA user_version = 15;
COMMIT;
