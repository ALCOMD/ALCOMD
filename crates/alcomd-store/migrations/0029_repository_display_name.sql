BEGIN IMMEDIATE;

ALTER TABLE repositories ADD COLUMN display_name TEXT
    CHECK (display_name IS NULL OR length(display_name) BETWEEN 1 AND 128);

PRAGMA user_version = 29;
COMMIT;
