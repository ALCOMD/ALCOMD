BEGIN IMMEDIATE;

ALTER TABLE projects ADD COLUMN favorite INTEGER NOT NULL DEFAULT 0
    CHECK (favorite IN (0, 1));

DROP INDEX project_editor_preferences_installation;
ALTER TABLE project_editor_preferences RENAME TO project_editor_preferences_v10;

CREATE TABLE project_editor_preferences (
    project_id TEXT PRIMARY KEY
        REFERENCES projects(project_id) ON DELETE CASCADE,
    selection_mode TEXT NOT NULL
        CHECK (selection_mode IN ('automatic', 'explicit')),
    installation_id TEXT
        REFERENCES unity_installations(installation_id) ON DELETE RESTRICT,
    arguments_json TEXT NOT NULL
        CHECK (
            json_valid(arguments_json)
            AND json_type(arguments_json) = 'array'
            AND json_array_length(arguments_json) <= 64
            AND length(arguments_json) <= 65536
        ),
    revision INTEGER NOT NULL
        CHECK (revision BETWEEN 1 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL
        CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807),
    CHECK (
        (selection_mode = 'automatic' AND installation_id IS NULL)
        OR (selection_mode = 'explicit' AND installation_id IS NOT NULL)
    )
) STRICT;

INSERT INTO project_editor_preferences (
    project_id, selection_mode, installation_id, arguments_json, revision, updated_at_ms
)
SELECT project_id, 'explicit', installation_id, arguments_json, revision, updated_at_ms
FROM project_editor_preferences_v10;

DROP TABLE project_editor_preferences_v10;

CREATE INDEX project_editor_preferences_installation
    ON project_editor_preferences(installation_id, project_id);

PRAGMA user_version = 11;

COMMIT;
