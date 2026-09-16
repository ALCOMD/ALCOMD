use alcomd_application::{
    CandidateCatalogEntry, CandidateQueryError as Error, CandidateStoreSnapshot, M3ErrorCode,
    MAX_CANDIDATE_RECORDS, MAX_CANDIDATE_SOURCES, PackageSourceSelector, PrincipalId, ProjectId,
    UserPackageId,
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
        serde_json::to_string(&project).map_err(|_| Error::Unavailable)?,
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
    use alcomd_application::{IdempotencyKey, ManifestState, ProjectObservation, ProjectType};

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
                observed_at_ms: 1,
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
