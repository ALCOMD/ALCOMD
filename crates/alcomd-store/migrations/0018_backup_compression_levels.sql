-- sqlite.rs owns the transaction, temporarily disables foreign_keys outside it,
-- recreates every original explicit index/trigger, and checks all foreign keys
-- before committing. No child table or existing backup row is rewritten.

CREATE TABLE backups_v18 (
    backup_id TEXT PRIMARY KEY
        CHECK (length(backup_id) = 36),
    owner_principal_id TEXT NOT NULL
        CHECK (length(owner_principal_id) BETWEEN 1 AND 128),
    source_project_id TEXT
        CHECK (source_project_id IS NULL OR length(source_project_id) = 36),
    archive_locator TEXT NOT NULL UNIQUE
        CHECK (length(archive_locator) BETWEEN 1 AND 32768),
    file_identity_key BLOB NOT NULL
        CHECK (length(file_identity_key) BETWEEN 1 AND 128),
    archive_sha256 BLOB NOT NULL
        CHECK (length(archive_sha256) = 32),
    byte_size INTEGER NOT NULL
        CHECK (byte_size BETWEEN 0 AND 9223372036854775807),
    format_version INTEGER NOT NULL
        CHECK (format_version BETWEEN 1 AND 2147483647),
    created_at_ms INTEGER NOT NULL
        CHECK (created_at_ms BETWEEN 0 AND 9223372036854775807),
    compression_mode TEXT NOT NULL
        CHECK (compression_mode IN ('store', 'fast', 'level3', 'level5', 'level7', 'maximum')),
    exclude_vpm_packages INTEGER NOT NULL
        CHECK (exclude_vpm_packages IN (0, 1))
) STRICT;

INSERT INTO backups_v18 (
    rowid, backup_id, owner_principal_id, source_project_id, archive_locator,
    file_identity_key, archive_sha256, byte_size, format_version,
    created_at_ms, compression_mode, exclude_vpm_packages
)
SELECT rowid, backup_id, owner_principal_id, source_project_id, archive_locator,
       file_identity_key, archive_sha256, byte_size, format_version,
       created_at_ms, compression_mode, exclude_vpm_packages
FROM backups;

DROP TABLE backups;
ALTER TABLE backups_v18 RENAME TO backups;

PRAGMA user_version = 18;
