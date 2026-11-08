BEGIN IMMEDIATE;

CREATE TABLE project_presentation_observations (
    project_id TEXT PRIMARY KEY REFERENCES projects(project_id) ON DELETE CASCADE,
    revision INTEGER NOT NULL CHECK (revision > 0),
    project_revision INTEGER NOT NULL CHECK (project_revision > 0),
    state TEXT NOT NULL CHECK (state IN ('complete','partial','unobserved')),
    fingerprint TEXT NOT NULL CHECK (length(fingerprint) = 64)
);
CREATE TABLE project_package_presentations (
    project_id TEXT NOT NULL REFERENCES projects(project_id) ON DELETE CASCADE,
    package_id TEXT NOT NULL,
    presentation_json TEXT NOT NULL CHECK (json_valid(presentation_json)),
    PRIMARY KEY (project_id,package_id)
);
ALTER TABLE repository_package_versions ADD COLUMN presentation_json TEXT
    CHECK (presentation_json IS NULL OR json_valid(presentation_json));

PRAGMA user_version = 30;
COMMIT;
