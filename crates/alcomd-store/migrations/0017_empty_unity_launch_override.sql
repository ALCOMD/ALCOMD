BEGIN IMMEDIATE;

CREATE TABLE project_unity_launch_config_v17 (
    project_id TEXT PRIMARY KEY
        REFERENCES projects(project_id) ON DELETE CASCADE,
    arguments_json TEXT NOT NULL
        CHECK (
            json_valid(arguments_json)
            AND json_type(arguments_json) = 'array'
            AND json_array_length(arguments_json) BETWEEN 0 AND 64
            AND length(arguments_json) <= 65536
        ),
    revision INTEGER NOT NULL
        CHECK (revision BETWEEN 1 AND 9223372036854775807),
    updated_at_ms INTEGER NOT NULL
        CHECK (updated_at_ms BETWEEN 0 AND 9223372036854775807)
) STRICT;

INSERT INTO project_unity_launch_config_v17 (
    project_id, arguments_json, revision, updated_at_ms
)
SELECT project_id, arguments_json, revision, updated_at_ms
FROM project_unity_launch_config;

DROP TABLE project_unity_launch_config;
ALTER TABLE project_unity_launch_config_v17 RENAME TO project_unity_launch_config;

PRAGMA user_version = 17;

COMMIT;
