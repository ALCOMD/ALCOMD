use alcomd_application::{
    CandidateCatalogEntry, CandidateFilterSource, CandidateLinks, CandidateQueryError as Error,
    CandidateSourceMetadata, CandidateStoreAuthority, CandidateStoreSnapshot, M3ErrorCode,
    MAX_CANDIDATE_RECORDS, MAX_CANDIDATE_SOURCES, PackageSourceSelector, PrincipalId,
    ProjectAuthorityFingerprint, ProjectId, ProjectRecord, UserPackageId,
};
use rusqlite::{Connection, params};

pub(super) fn snapshot(
    connection: &mut Connection,
    owner: &PrincipalId,
    project_id: ProjectId,
    ids: Option<Vec<String>>,
    settings: &alcomd_application::ConfigPackageSettings,
) -> Result<CandidateStoreSnapshot, Error> {
    // DEFERRED read transaction gives every registry read one SQLite snapshot. No writes.
    let transaction = connection.transaction().map_err(|_| Error::Unavailable)?;
    let (project, authority) = read_authority(&transaction, owner, project_id, settings)?;
    let hidden =
        serde_json::to_string(&settings.hidden_repository_ids).map_err(|_| Error::Unavailable)?;
    let ids_json = ids
        .map(|v| serde_json::to_string(&v))
        .transpose()
        .map_err(|_| Error::Unavailable)?;
    let mut statement = transaction.prepare(
        "SELECT p.package_id,p.version_text,r.repository_id,r.revision,r.priority,p.yanked,p.unity_text,
                (r.resolver_indexed AND p.resolver_ready),p.legacy_metadata_present,p.unity_release_text,
                p.display_name,p.documentation_url,p.changelog_url,p.description,p.presentation_json,p.dependencies_json
         FROM repository_package_versions p JOIN repositories r ON r.repository_id=p.repository_id
         WHERE r.owner_principal_id=?1 AND (r.resolver_indexed=0 OR p.resolver_ready=1)
         AND r.repository_id NOT IN (SELECT value FROM json_each(?3))
         AND (?4 OR p.semantic_version IS NULL OR instr(substr(p.semantic_version,1,instr(p.semantic_version||'+','+')-1),'-')=0)
         AND (?2 IS NULL OR p.package_id IN (SELECT value FROM json_each(?2)))
         ORDER BY r.repository_id,p.package_id,p.version_text LIMIT 100001"
    ).map_err(|_| Error::Unavailable)?;
    let mut entries = statement
        .query_map(
            params![owner.as_str(), ids_json, hidden, settings.show_prerelease],
            |row| {
                Ok(CandidateCatalogEntry {
                    description: row.get(13)?,
                    presentation: row
                        .get::<_, Option<String>>(14)?
                        .map(|v| serde_json::from_str(&v))
                        .transpose()
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    dependencies: serde_json::from_str(&row.get::<_, String>(15)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    package_id: row.get(0)?,
                    version: row.get(1)?,
                    display_name: row.get(10)?,
                    links: CandidateLinks {
                        documentation: row.get(11)?,
                        changelog: row.get(12)?,
                    },
                    source: PackageSourceSelector::Repository {
                        repository_id: row.get(2)?,
                    },
                    source_revision: u64::try_from(row.get::<_, i64>(3)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    priority: u64::try_from(row.get::<_, i64>(4)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    yanked: row.get(5)?,
                    unity: row.get(6)?,
                    unity_release: row.get(9)?,
                    metadata_ready: row.get(7)?,
                    legacy_metadata_present: row.get(8)?,
                })
            },
        )
        .map_err(|_| Error::Unavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| Error::Unavailable)?;
    if entries.len() > MAX_CANDIDATE_RECORDS {
        return Err(Error::LimitExceeded);
    }
    let remaining = MAX_CANDIDATE_RECORDS - entries.len() + 1;
    let mut statement = transaction.prepare(
        "SELECT package_id,version,user_package_id,revision,manifest_json FROM user_package_sources
         WHERE owner_principal_id=?1 AND (?2 IS NULL OR package_id IN (SELECT value FROM json_each(?2))) AND ?4=0 AND (?5 OR instr(substr(version,1,instr(version||'+','+')-1),'-')=0) ORDER BY user_package_id LIMIT ?3"
    ).map_err(|_| Error::Unavailable)?;
    let user_entries = statement
        .query_map(
            params![
                owner.as_str(),
                ids_json,
                remaining as i64,
                settings.hide_local_user_packages,
                settings.show_prerelease
            ],
            |row| {
                let id: String = row.get(2)?;
                let manifest: String = row.get(4)?;
                let manifest =
                    serde_json::from_str(&manifest).map_err(|_| rusqlite::Error::InvalidQuery)?;
                let metadata = crate::m4::user_manifest_metadata(&manifest)?;
                let field = |key| {
                    manifest
                        .get(key)
                        .and_then(serde_json::Value::as_str)
                        .filter(|value| {
                            !value.is_empty()
                                && value.len() <= if key == "description" { 65_536 } else { 4_096 }
                        })
                        .map(str::to_owned)
                };
                Ok(CandidateCatalogEntry {
                    description: field("description"),
                    presentation: alcomd_application::normalize_package_display_metadata(&manifest),
                    dependencies: manifest
                        .get("vpmDependencies")
                        .map(|v| serde_json::from_value(v.clone()))
                        .transpose()
                        .map_err(|_| rusqlite::Error::InvalidQuery)?
                        .unwrap_or_default(),
                    package_id: row.get(0)?,
                    version: row.get(1)?,
                    display_name: field("displayName"),
                    links: CandidateLinks {
                        documentation: field("documentationUrl"),
                        changelog: field("changelogUrl"),
                    },
                    source: PackageSourceSelector::UserPackage {
                        user_package_id: UserPackageId::parse(&id)
                            .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    },
                    source_revision: u64::try_from(row.get::<_, i64>(3)?)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                    priority: 0,
                    yanked: manifest
                        .as_object()
                        .and_then(alcomd_application::package_yanked)
                        .ok_or(rusqlite::Error::InvalidQuery)?,
                    unity: metadata.unity,
                    unity_release: metadata.unity_release,
                    metadata_ready: true,
                    legacy_metadata_present: metadata.legacy_present,
                })
            },
        )
        .map_err(|_| Error::Unavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| Error::Unavailable)?;
    entries.extend(user_entries);
    if entries.len() > MAX_CANDIDATE_RECORDS {
        return Err(Error::LimitExceeded);
    }
    if entries
        .iter()
        .any(|e| e.package_id.len() > 128 || e.version.len() > 256)
    {
        return Err(Error::LimitExceeded);
    }
    Ok(CandidateStoreSnapshot {
        project,
        entries,
        fingerprint_records: authority.fingerprint_records,
        catalog_complete: authority.catalog_complete,
        source_metadata: source_metadata(&transaction, owner, settings)?,
    })
}

fn source_metadata(
    transaction: &Connection,
    owner: &PrincipalId,
    settings: &alcomd_application::ConfigPackageSettings,
) -> Result<std::collections::HashMap<String, CandidateSourceMetadata>, Error> {
    let hidden =
        serde_json::to_string(&settings.hidden_repository_ids).map_err(|_| Error::Unavailable)?;
    let mut result = std::collections::HashMap::new();
    let mut statement = transaction.prepare(
        "SELECT repository_id,source_kind,display_name,coalesce(name,declared_id,source_locator),coalesce(display_name,name,declared_id,source_locator)
         FROM repositories WHERE owner_principal_id=?1
         AND repository_id NOT IN (SELECT value FROM json_each(?4))
         ORDER BY CASE repository_id WHEN ?2 THEN 0 WHEN ?3 THEN 1 ELSE 2 END,priority,repository_id"
    ).map_err(|_| Error::Unavailable)?;
    let repositories = statement
        .query_map(
            params![
                owner.as_str(),
                alcomd_application::VRCHAT_OFFICIAL_REPOSITORY_ID,
                alcomd_application::VRCHAT_CURATED_REPOSITORY_ID,
                hidden
            ],
            |row| {
                let id: String = row.get(0)?;
                let kind = match row.get::<_, String>(1)?.as_str() {
                    "local" => CandidateFilterSource::Local,
                    "remote" => CandidateFilterSource::Remote,
                    _ => return Err(rusqlite::Error::InvalidQuery),
                };
                let builtin = alcomd_application::RepositoryId::parse(&id)
                    .ok()
                    .and_then(alcomd_application::builtin_repository);
                Ok((
                    id,
                    CandidateSourceMetadata {
                        kind,
                        display_name: row.get(2)?,
                        name: row.get(3)?,
                        builtin,
                        search_name: row.get::<_, String>(4)?.to_lowercase(),
                        display_order: 0,
                    },
                ))
            },
        )
        .map_err(|_| Error::Unavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| Error::Unavailable)?;
    for (index, (id, mut metadata)) in repositories.into_iter().enumerate() {
        metadata.display_order = match metadata.builtin {
            Some(alcomd_application::BuiltinRepository::Official) => 0,
            Some(alcomd_application::BuiltinRepository::Curated) => 1,
            None => 3 + index as u32,
        };
        result.insert(format!("repository:{id}"), metadata);
    }
    let mut statement = transaction.prepare(
        "SELECT user_package_id,json_extract(manifest_json,'$.displayName') FROM user_package_sources WHERE owner_principal_id=?1 AND ?2=0 ORDER BY user_package_id"
    ).map_err(|_| Error::Unavailable)?;
    let users = statement
        .query_map(
            params![owner.as_str(), settings.hide_local_user_packages],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )
        .map_err(|_| Error::Unavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| Error::Unavailable)?;
    for (id, name) in users {
        result.insert(
            format!("user_package:{id}"),
            CandidateSourceMetadata {
                kind: CandidateFilterSource::UserPackage,
                display_name: None,
                search_name: name.as_deref().unwrap_or("").to_lowercase(),
                name,
                builtin: None,
                display_order: 2,
            },
        );
    }
    Ok(result)
}

/// Revalidate a cached summary without loading any package version rows.
pub(super) fn authority(
    connection: &mut Connection,
    owner: &PrincipalId,
    project_id: ProjectId,
    settings: &alcomd_application::ConfigPackageSettings,
) -> Result<CandidateStoreAuthority, Error> {
    let transaction = connection.transaction().map_err(|_| Error::Unavailable)?;
    let (_, authority) = read_authority(&transaction, owner, project_id, settings)?;
    Ok(authority)
}

fn read_authority(
    transaction: &Connection,
    owner: &PrincipalId,
    project_id: ProjectId,
    settings: &alcomd_application::ConfigPackageSettings,
) -> Result<(ProjectRecord, CandidateStoreAuthority), Error> {
    let project =
        crate::m3::get_project(transaction, owner, project_id).map_err(|e| match e.code() {
            M3ErrorCode::ProjectNotRegistered => Error::ProjectNotRegistered,
            M3ErrorCode::ProjectManifestInvalid => Error::ProjectManifestInvalid,
            _ => Error::Unavailable,
        })?;
    let mut fingerprint_records = vec![
        "project-candidates-snapshot-v3".to_owned(),
        serde_json::to_string(&ProjectAuthorityFingerprint::from(&project))
            .map_err(|_| Error::Unavailable)?,
    ];
    let mut statement = transaction.prepare(
        "SELECT json_array('repository',repository_id,revision,priority,hex(source_identity_key),resolver_indexed) FROM repositories WHERE owner_principal_id=?1 AND repository_id NOT IN (SELECT value FROM json_each(?2))
         UNION ALL SELECT json_array('user_package',user_package_id,revision,0,hex(source_identity_key),hex(manifest_fingerprint),archive_sha256) FROM user_package_sources WHERE owner_principal_id=?1 AND ?3=0
         ORDER BY 1 LIMIT 4097"
    ).map_err(|_| Error::Unavailable)?;
    let hidden =
        serde_json::to_string(&settings.hidden_repository_ids).map_err(|_| Error::Unavailable)?;
    let inventory = statement
        .query_map(
            params![owner.as_str(), hidden, settings.hide_local_user_packages],
            |row| row.get::<_, String>(0),
        )
        .map_err(|_| Error::Unavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| Error::Unavailable)?;
    if inventory.len() > MAX_CANDIDATE_SOURCES {
        return Err(Error::LimitExceeded);
    }
    fingerprint_records.extend(inventory);
    let incomplete = crate::m4::repository_catalog_needs_refresh(transaction, owner, settings)
        .map_err(|_| Error::Unavailable)?;
    fingerprint_records.push(incomplete.to_string());
    let authority = CandidateStoreAuthority {
        project: project.observation.clone(),
        project_revision: project.revision,
        fingerprint_records,
        catalog_complete: !incomplete,
    };
    Ok((project, authority))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alcomd_application::{
        DependencyIdentity, IdempotencyKey, ManifestState, ProjectObservation, ProjectRecord,
        ProjectType, ReadIssue, Revision,
    };

    fn authority_project() -> ProjectRecord {
        ProjectRecord {
            project_id: ProjectId::parse("00000000-0000-4000-8000-000000000101")
                .expect("project ID"),
            observation: ProjectObservation {
                root_path: "C:\\Projects\\日本語".into(),
                path_identity_key: vec![0, 255],
                project_type: ProjectType::Avatars,
                unity_version: "2022.3.22f1".into(),
                unity_revision: Some("abcdef".into()),
                vpm_manifest: ManifestState::Valid,
                upm_manifest: ManifestState::Missing,
                direct_dependencies: vec![DependencyIdentity {
                    package_id: "com.example.direct".into(),
                    value: "1.0.0".into(),
                }],
                locked_dependencies: vec![DependencyIdentity {
                    package_id: "com.example.locked".into(),
                    value: "2.0.0".into(),
                }],
                issues: vec![ReadIssue {
                    code: "bounded-issue".into(),
                    component: "manifest".into(),
                    item: "com.example.direct".into(),
                    line: Some(2),
                    column: None,
                }],
            },
            revision: Revision::INITIAL,
            registered_at_ms: 11,
            last_modified_at_ms: Some(99),
            favorite: true,
        }
    }

    #[test]
    fn project_authority_projection_preserves_frozen_bytes_and_existing_field_policy() {
        // Frozen pre-projection bytes, including struct field order and non-ASCII escaping.
        const FROZEN: &str = r#"{"project_id":"00000000-0000-4000-8000-000000000101","observation":{"root_path":"C:\\Projects\\日本語","path_identity_key":[0,255],"project_type":"avatars","unity_version":"2022.3.22f1","unity_revision":"abcdef","vpm_manifest":"valid","upm_manifest":"missing","direct_dependencies":[{"package_id":"com.example.direct","value":"1.0.0"}],"locked_dependencies":[{"package_id":"com.example.locked","value":"2.0.0"}],"issues":[{"code":"bounded-issue","component":"manifest","item":"com.example.direct","line":2,"column":null}]},"revision":1,"registered_at_ms":11,"favorite":true}"#;
        let project = authority_project();
        let encode = |project: &ProjectRecord| {
            serde_json::to_string(&ProjectAuthorityFingerprint::from(project))
                .expect("serialize project authority")
        };
        assert_eq!(encode(&project).as_bytes(), FROZEN.as_bytes());
        let mut previous_representation = project.clone();
        previous_representation.last_modified_at_ms = None;
        assert_eq!(
            serde_json::to_string(&previous_representation).expect("serialize previous authority"),
            FROZEN
        );
        for activity in [None, Some(0), Some(1_000)] {
            let mut changed = project.clone();
            changed.last_modified_at_ms = activity;
            assert_eq!(encode(&changed), FROZEN);
        }
        type Mutation = (&'static str, fn(&mut ProjectRecord));
        let mutations: [Mutation; 14] = [
            ("project ID", |p| p.project_id = ProjectId::new()),
            ("revision", |p| {
                p.revision = p.revision.checked_next().expect("next revision")
            }),
            ("registration", |p| p.registered_at_ms += 1),
            ("favorite", |p| p.favorite = !p.favorite),
            ("root", |p| p.observation.root_path.push('x')),
            ("identity", |p| p.observation.path_identity_key.push(1)),
            ("project type", |p| {
                p.observation.project_type = ProjectType::Worlds
            }),
            ("Unity version", |p| p.observation.unity_version.push('x')),
            ("Unity revision", |p| p.observation.unity_revision = None),
            ("VPM state", |p| {
                p.observation.vpm_manifest = ManifestState::Missing
            }),
            ("UPM state", |p| {
                p.observation.upm_manifest = ManifestState::Valid
            }),
            ("direct dependencies", |p| {
                p.observation.direct_dependencies.clear()
            }),
            ("locked dependencies", |p| {
                p.observation.locked_dependencies.clear()
            }),
            ("issues", |p| p.observation.issues.clear()),
        ];
        for (name, mutate) in mutations {
            let mut changed = project.clone();
            mutate(&mut changed);
            assert_ne!(encode(&changed), FROZEN, "{name} must remain in authority");
        }
    }

    struct Fixture {
        connection: Connection,
        path: std::path::PathBuf,
        project: ProjectId,
    }
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "alcomd-candidate-store-{}.db",
                uuid::Uuid::new_v4()
            ));
            let mut connection = crate::sqlite::initialize_connection(&path)
                .expect("candidate fixture or query must succeed");
            let observation = ProjectObservation {
                root_path: "unopened-project".into(),
                path_identity_key: vec![1],
                project_type: ProjectType::Unknown,
                unity_version: "2022.3.1f1".into(),
                unity_revision: None,
                vpm_manifest: ManifestState::Valid,
                upm_manifest: ManifestState::Missing,
                direct_dependencies: vec![],
                locked_dependencies: vec![],
                issues: vec![],
            };
            let record = crate::m3::register_project(
                &mut connection,
                &PrincipalId::local_owner(),
                observation,
                &IdempotencyKey::parse("candidate-test-register")
                    .expect("candidate fixture or query must succeed"),
                1,
            )
            .expect("candidate fixture or query must succeed");
            Self {
                connection,
                path,
                project: record.value.project_id,
            }
        }
        fn repository(&self) {
            self.connection.execute("INSERT INTO repositories (repository_id,owner_principal_id,source_kind,source_locator,source_identity_key,issues_json,revision,registered_at_ms,refreshed_at_ms,updated_at_ms,priority) VALUES ('22222222-2222-4222-8222-222222222222',?1,'local','unopened-repository',x'02','[]',1,1,1,1,1)", [PrincipalId::local_owner().as_str()]).expect("candidate fixture or query must succeed");
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    #[test]
    fn retired_development_copy_is_removed_without_changing_project_authority_or_history() {
        let f = Fixture::new();
        f.repository();
        let before: String = f
            .connection
            .query_row("SELECT snapshot_json FROM projects", [], |row| row.get(0))
            .expect("project snapshot");
        let events: i64 = f
            .connection
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .expect("history");
        f.connection.execute_batch("CREATE TABLE project_presentation_observations(project_id TEXT PRIMARY KEY, revision INTEGER, project_revision INTEGER, state TEXT, fingerprint TEXT); CREATE TABLE project_package_presentations(project_id TEXT,package_id TEXT,presentation_json TEXT); PRAGMA user_version=30;").expect("old development copy");
        f.connection
            .execute(
                "INSERT INTO project_package_presentations VALUES(?1,'com.example.obsolete','{}')",
                [f.project.to_string()],
            )
            .expect("obsolete cache");
        f.connection
            .execute_batch(include_str!(
                "../migrations/0031_remove_project_presentations.sql"
            ))
            .expect("approved removal");
        assert_eq!(f.connection.query_row::<i64,_,_>("SELECT COUNT(*) FROM sqlite_schema WHERE name IN ('project_package_presentations','project_presentation_observations')",[],|row|row.get(0)).expect("absence"),0);
        assert_eq!(
            f.connection
                .query_row::<String, _, _>("SELECT snapshot_json FROM projects", [], |row| row
                    .get(0))
                .expect("project after removal"),
            before
        );
        assert_eq!(
            f.connection
                .query_row::<i64, _, _>("SELECT COUNT(*) FROM events", [], |row| row.get(0))
                .expect("history after removal"),
            events
        );
        assert_eq!(
            f.connection
                .query_row::<i64, _, _>("PRAGMA user_version", [], |row| row.get(0))
                .expect("format"),
            31
        );
        assert_eq!(
            f.connection
                .query_row::<i64, _, _>("SELECT COUNT(*) FROM repositories", [], |row| row.get(0))
                .expect("repository retained"),
            1
        );
    }
    #[test]
    fn project_activity_time_is_returned_without_changing_candidate_evidence() {
        let mut f = Fixture::new();
        let before = snapshot(
            &mut f.connection,
            &PrincipalId::local_owner(),
            f.project,
            None,
            &alcomd_application::ConfigSnapshot::default()
                .settings
                .packages,
        )
        .expect("read original candidate authority");
        assert_eq!(before.project.last_modified_at_ms, Some(1));
        let mut previous_project_authority = before.project.clone();
        previous_project_authority.last_modified_at_ms = None;
        assert_eq!(
            before.fingerprint_records[1],
            serde_json::to_string(&previous_project_authority)
                .expect("serialize established candidate record")
        );
        f.connection
            .execute(
                "UPDATE projects SET last_modified_at_ms=20 WHERE project_id=?1",
                [f.project.to_string()],
            )
            .expect("record project activity without changing its contents");
        let changes = f.connection.total_changes();
        let after = snapshot(
            &mut f.connection,
            &PrincipalId::local_owner(),
            f.project,
            None,
            &alcomd_application::ConfigSnapshot::default()
                .settings
                .packages,
        )
        .expect("read candidate authority after project activity");
        assert_eq!(after.project.last_modified_at_ms, Some(20));
        assert_eq!(after.project.revision, before.project.revision);
        assert_eq!(after.project.observation, before.project.observation);
        assert_eq!(after.fingerprint_records, before.fingerprint_records);
        assert_eq!(changes, f.connection.total_changes());
    }

    #[test]
    fn registered_user_snapshot_is_read_without_source_files_or_effects() {
        let mut f = Fixture::new();
        f.connection.execute("INSERT INTO user_package_sources VALUES ('33333333-3333-4333-8333-333333333333',?1,'source-does-not-exist',x'03','com.example.user','1.0.0','{\"unity\":\"2022.3\",\"unityRelease\":\"22f1\",\"legacyFolders\":{\"Assets/Old\":\"\"}}',zeroblob(32),zeroblob(32),?2,1,1,1)", params![PrincipalId::local_owner().as_str(), "a".repeat(64)]).expect("candidate fixture or query must succeed");
        let changes = f.connection.total_changes();
        let result = snapshot(
            &mut f.connection,
            &PrincipalId::local_owner(),
            f.project,
            None,
            &alcomd_application::ConfigSnapshot::default()
                .settings
                .packages,
        )
        .expect("candidate fixture or query must succeed");
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].unity.as_deref(), Some("2022.3"));
        assert_eq!(result.entries[0].unity_release.as_deref(), Some("22f1"));
        assert!(result.entries[0].legacy_metadata_present);
        let resolver = crate::m4::resolver_catalog(
            &f.connection,
            &PrincipalId::local_owner(),
            true,
            &alcomd_application::ConfigSnapshot::default()
                .settings
                .packages,
        )
        .expect("resolver sees the same registered metadata");
        assert_eq!(resolver.entries[0].unity_release.as_deref(), Some("22f1"));
        assert!(resolver.entries[0].legacy_metadata_present);
        assert!(result.entries[0].metadata_ready);
        assert_eq!(changes, f.connection.total_changes());
        assert!(matches!(
            snapshot(
                &mut f.connection,
                &PrincipalId::parse("another-principal")
                    .expect("candidate fixture or query must succeed"),
                f.project,
                None,
                &alcomd_application::ConfigSnapshot::default()
                    .settings
                    .packages
            ),
            Err(Error::ProjectNotRegistered)
        ));
    }

    #[test]
    fn invalid_stored_user_metadata_is_not_silently_treated_as_absent() {
        for manifest in [
            serde_json::json!({"unity":"2022.3","unityRelease":22}),
            serde_json::json!({"unityRelease":null}),
            serde_json::json!({"legacyFolders":[]}),
            serde_json::json!({"legacyFiles":{"Assets/Old":"invalid-guid"}}),
            serde_json::json!({"legacyPackages":{}}),
        ] {
            let mut f = Fixture::new();
            f.connection.execute("INSERT INTO user_package_sources VALUES ('33333333-3333-4333-8333-333333333333',?1,'source-does-not-exist',x'03','com.example.user','1.0.0',?2,zeroblob(32),zeroblob(32),?3,1,1,1)", params![PrincipalId::local_owner().as_str(), serde_json::to_string(&manifest).expect("manifest"), "a".repeat(64)]).expect("historical registered input");
            assert!(
                snapshot(
                    &mut f.connection,
                    &PrincipalId::local_owner(),
                    f.project,
                    None,
                    &alcomd_application::ConfigSnapshot::default()
                        .settings
                        .packages
                )
                .is_err(),
                "candidate {manifest}"
            );
            assert!(
                crate::m4::resolver_catalog(
                    &f.connection,
                    &PrincipalId::local_owner(),
                    true,
                    &alcomd_application::ConfigSnapshot::default()
                        .settings
                        .packages
                )
                .is_err(),
                "resolver {manifest}"
            );
        }
    }
    #[test]
    fn raw_unready_evidence_and_global_readiness_are_not_silently_omitted() {
        let mut f = Fixture::new();
        f.repository();
        f.connection.execute("INSERT INTO repository_package_versions(repository_id,package_id,version_text) VALUES ('22222222-2222-4222-8222-222222222222','com.example.package','1.0.0')", []).expect("candidate fixture or query must succeed");
        let all = snapshot(
            &mut f.connection,
            &PrincipalId::local_owner(),
            f.project,
            None,
            &alcomd_application::ConfigSnapshot::default()
                .settings
                .packages,
        )
        .expect("candidate fixture or query must succeed");
        assert!(!all.catalog_complete);
        assert!(!all.entries[0].metadata_ready);
        let empty = snapshot(
            &mut f.connection,
            &PrincipalId::local_owner(),
            f.project,
            Some(vec!["com.example.absent".into()]),
            &alcomd_application::ConfigSnapshot::default()
                .settings
                .packages,
        )
        .expect("candidate fixture or query must succeed");
        assert!(empty.entries.is_empty());
        assert!(!empty.catalog_complete);
        assert_eq!(all.fingerprint_records, empty.fingerprint_records);
        let current = authority(
            &mut f.connection,
            &PrincipalId::local_owner(),
            f.project,
            &alcomd_application::ConfigSnapshot::default()
                .settings
                .packages,
        )
        .expect("authority-only read");
        assert_eq!(current.project_revision, all.project.revision);
        assert_eq!(current.catalog_complete, all.catalog_complete);
        assert_eq!(current.fingerprint_records, all.fingerprint_records);
    }
    #[test]
    fn real_source_and_record_work_caps_fail_without_partial_results() {
        let mut f = Fixture::new();
        f.repository();
        f.connection.execute("WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<100001) INSERT INTO repository_package_versions(repository_id,package_id,version_text) SELECT '22222222-2222-4222-8222-222222222222','com.example.package',printf('1.0.%d',i) FROM n", []).expect("candidate fixture or query must succeed");
        let changes = f.connection.total_changes();
        assert!(matches!(
            snapshot(
                &mut f.connection,
                &PrincipalId::local_owner(),
                f.project,
                None,
                &alcomd_application::ConfigSnapshot::default()
                    .settings
                    .packages
            ),
            Err(Error::LimitExceeded)
        ));
        assert_eq!(changes, f.connection.total_changes());
        // A scoped relevant query does not drain the unrelated version catalog.
        assert!(
            snapshot(
                &mut f.connection,
                &PrincipalId::local_owner(),
                f.project,
                Some(vec!["com.example.absent".into()]),
                &alcomd_application::ConfigSnapshot::default()
                    .settings
                    .packages
            )
            .expect("candidate fixture or query must succeed")
            .entries
            .is_empty()
        );
        f.connection.execute("WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<4096) INSERT INTO repositories(repository_id,owner_principal_id,source_kind,source_locator,source_identity_key,issues_json,revision,registered_at_ms,refreshed_at_ms,updated_at_ms,priority) SELECT printf('%08d-2222-4222-8222-222222222222',i),?1,'local','unopened',CAST(printf('identity-%d',i) AS BLOB),'[]',1,1,1,1,1 FROM n", [PrincipalId::local_owner().as_str()]).expect("candidate fixture or query must succeed");
        assert!(matches!(
            snapshot(
                &mut f.connection,
                &PrincipalId::local_owner(),
                f.project,
                Some(vec!["com.example.absent".into()]),
                &alcomd_application::ConfigSnapshot::default()
                    .settings
                    .packages
            ),
            Err(Error::LimitExceeded)
        ));
    }
}
