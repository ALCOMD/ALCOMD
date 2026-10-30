use alcomd_application::{
    CandidateCatalogEntry, CandidateQueryError as Error, CandidateStoreSnapshot, M3ErrorCode,
    MAX_CANDIDATE_RECORDS, MAX_CANDIDATE_SOURCES, PackageSourceSelector, PrincipalId,
    ProjectAuthorityFingerprint, ProjectId, UserPackageId,
};
use rusqlite::{Connection, params};

pub(super) fn snapshot(
    connection: &mut Connection,
    owner: &PrincipalId,
    project_id: ProjectId,
    ids: Option<Vec<String>>,
) -> Result<CandidateStoreSnapshot, Error> {
    // DEFERRED read transaction gives every registry read one SQLite snapshot. No writes.
    let transaction = connection.transaction().map_err(|_| Error::Unavailable)?;
    let project =
        crate::m3::get_project(&transaction, owner, project_id).map_err(|e| match e.code() {
            M3ErrorCode::ProjectNotRegistered => Error::ProjectNotRegistered,
            M3ErrorCode::ProjectManifestInvalid => Error::ProjectManifestInvalid,
            _ => Error::Unavailable,
        })?;
    let mut fingerprint_records = vec![
        "project-candidates-snapshot-v1".to_owned(),
        serde_json::to_string(&ProjectAuthorityFingerprint::from(&project))
            .map_err(|_| Error::Unavailable)?,
    ];
    let mut statement = transaction.prepare(
        "SELECT json_array('repository',repository_id,revision,priority,hex(source_identity_key)) FROM repositories WHERE owner_principal_id=?1
         UNION ALL SELECT json_array('user_package',user_package_id,revision,0,hex(source_identity_key),hex(manifest_fingerprint),archive_sha256) FROM user_package_sources WHERE owner_principal_id=?1
         ORDER BY 1 LIMIT 4097"
    ).map_err(|_| Error::Unavailable)?;
    let inventory = statement
        .query_map([owner.as_str()], |row| row.get::<_, String>(0))
        .map_err(|_| Error::Unavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| Error::Unavailable)?;
    if inventory.len() > MAX_CANDIDATE_SOURCES {
        return Err(Error::LimitExceeded);
    }
    fingerprint_records.extend(inventory);
    let incomplete: bool = transaction.query_row("SELECT EXISTS(SELECT 1 FROM repository_package_versions p JOIN repositories r ON r.repository_id=p.repository_id WHERE r.owner_principal_id=?1 AND p.resolver_ready=0)", [owner.as_str()], |row| row.get(0)).map_err(|_| Error::Unavailable)?;
    fingerprint_records.push(incomplete.to_string());
    let ids_json = ids
        .map(|v| serde_json::to_string(&v))
        .transpose()
        .map_err(|_| Error::Unavailable)?;
    let mut statement = transaction.prepare(
        "SELECT p.package_id,p.version_text,r.repository_id,r.revision,r.priority,p.yanked,p.unity_text,p.resolver_ready,p.legacy_metadata_present
         FROM repository_package_versions p JOIN repositories r ON r.repository_id=p.repository_id
         WHERE r.owner_principal_id=?1 AND (?2 IS NULL OR p.package_id IN (SELECT value FROM json_each(?2)))
         ORDER BY r.repository_id,p.package_id,p.version_text LIMIT 100001"
    ).map_err(|_| Error::Unavailable)?;
    let mut entries = statement
        .query_map(params![owner.as_str(), ids_json], |row| {
            Ok(CandidateCatalogEntry {
                package_id: row.get(0)?,
                version: row.get(1)?,
                source: PackageSourceSelector::Repository {
                    repository_id: row.get(2)?,
                },
                source_revision: u64::try_from(row.get::<_, i64>(3)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                priority: u64::try_from(row.get::<_, i64>(4)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                yanked: row.get(5)?,
                unity: row.get(6)?,
                metadata_ready: row.get(7)?,
                legacy_metadata_present: row.get(8)?,
            })
        })
        .map_err(|_| Error::Unavailable)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| Error::Unavailable)?;
    if entries.len() > MAX_CANDIDATE_RECORDS {
        return Err(Error::LimitExceeded);
    }
    let remaining = MAX_CANDIDATE_RECORDS - entries.len() + 1;
    let mut statement = transaction.prepare(
        "SELECT package_id,version,user_package_id,revision,json_extract(manifest_json,'$.unity') FROM user_package_sources
         WHERE owner_principal_id=?1 AND (?2 IS NULL OR package_id IN (SELECT value FROM json_each(?2))) ORDER BY user_package_id LIMIT ?3"
    ).map_err(|_| Error::Unavailable)?;
    let user_entries = statement
        .query_map(params![owner.as_str(), ids_json, remaining as i64], |row| {
            let id: String = row.get(2)?;
            Ok(CandidateCatalogEntry {
                package_id: row.get(0)?,
                version: row.get(1)?,
                source: PackageSourceSelector::UserPackage {
                    user_package_id: UserPackageId::parse(&id)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                },
                source_revision: u64::try_from(row.get::<_, i64>(3)?)
                    .map_err(|_| rusqlite::Error::InvalidQuery)?,
                priority: 0,
                yanked: false,
                unity: row.get(4)?,
                metadata_ready: true,
                legacy_metadata_present: false,
            })
        })
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
        fingerprint_records,
        catalog_complete: !incomplete,
    })
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
    fn project_activity_time_is_returned_without_changing_candidate_evidence() {
        let mut f = Fixture::new();
        let before = snapshot(
            &mut f.connection,
            &PrincipalId::local_owner(),
            f.project,
            None,
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
        f.connection.execute("INSERT INTO user_package_sources VALUES ('33333333-3333-4333-8333-333333333333',?1,'source-does-not-exist',x'03','com.example.user','1.0.0','{\"unity\":\"2022.3\"}',zeroblob(32),zeroblob(32),?2,1,1,1)", params![PrincipalId::local_owner().as_str(), "a".repeat(64)]).expect("candidate fixture or query must succeed");
        let changes = f.connection.total_changes();
        let result = snapshot(
            &mut f.connection,
            &PrincipalId::local_owner(),
            f.project,
            None,
        )
        .expect("candidate fixture or query must succeed");
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].unity.as_deref(), Some("2022.3"));
        assert!(result.entries[0].metadata_ready);
        assert_eq!(changes, f.connection.total_changes());
        assert!(matches!(
            snapshot(
                &mut f.connection,
                &PrincipalId::parse("another-principal")
                    .expect("candidate fixture or query must succeed"),
                f.project,
                None
            ),
            Err(Error::ProjectNotRegistered)
        ));
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
        )
        .expect("candidate fixture or query must succeed");
        assert!(!all.catalog_complete);
        assert!(!all.entries[0].metadata_ready);
        let empty = snapshot(
            &mut f.connection,
            &PrincipalId::local_owner(),
            f.project,
            Some(vec!["com.example.absent".into()]),
        )
        .expect("candidate fixture or query must succeed");
        assert!(empty.entries.is_empty());
        assert!(!empty.catalog_complete);
        assert_eq!(all.fingerprint_records, empty.fingerprint_records);
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
                None
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
                Some(vec!["com.example.absent".into()])
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
                Some(vec!["com.example.absent".into()])
            ),
            Err(Error::LimitExceeded)
        ));
    }
}
