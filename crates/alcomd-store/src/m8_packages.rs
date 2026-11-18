use alcomd_application::{
    CandidateCatalogEntry, CandidateQueryError, ConfigPackageSettings, PackageSourceSelector,
    Permission, PrincipalId, ProjectId, ProjectRecord, ToolAuthentication,
};

pub struct DelegatedPackageCatalog {
    pub authorization_revision: u64,
    pub project: Option<ProjectRecord>,
    pub sources: Vec<DelegatedPackageSource>,
    pub entries: Vec<CandidateCatalogEntry>,
    pub complete: bool,
}

pub struct DelegatedPackageSource {
    pub source: PackageSourceSelector,
    pub display_name: Option<String>,
    pub revision: i64,
    pub availability: DelegatedSourceAvailability,
}

#[derive(Clone, Copy)]
pub enum DelegatedSourceAvailability {
    Available,
    Hidden,
    RefreshRequired,
}

impl super::StateStoreHandle {
    pub async fn delegated_package_catalog(
        &self,
        authentication: ToolAuthentication,
        project: Option<ProjectId>,
        package_id: Option<String>,
        sources_only: bool,
        settings: ConfigPackageSettings,
    ) -> Result<DelegatedPackageCatalog, CandidateQueryError> {
        self.request_worker(
            move |db| {
                let tx = db
                    .transaction()
                    .map_err(|_| CandidateQueryError::Unavailable)?;
                let client = super::m8_tools::authenticate(&tx, &authentication)
                    .map_err(|_| CandidateQueryError::PermissionDenied)?;
                let authorization_revision = client.revision;
                let access = authentication
                    .access(client)
                    .map_err(|_| CandidateQueryError::PermissionDenied)?;
                for permission in [Permission::RepositoriesRead, Permission::PackagesRead] {
                    access
                        .require(permission)
                        .map_err(|_| CandidateQueryError::PermissionDenied)?;
                }
                let owner = PrincipalId::local_owner();
                let project = project
                    .map(|id| {
                        access
                            .require(Permission::ProjectsRead)
                            .map_err(|_| CandidateQueryError::PermissionDenied)?;
                        super::m3::get_project(&tx, &owner, id).map_err(|e| match e.code() {
                            alcomd_application::M3ErrorCode::ProjectNotRegistered => {
                                CandidateQueryError::ProjectNotRegistered
                            }
                            _ => CandidateQueryError::Unavailable,
                        })
                    })
                    .transpose()?;
                let sources = sources(&tx, &owner, &settings)?;
                let complete = !super::m4::repository_catalog_needs_refresh(&tx, &owner, &settings)
                    .map_err(|_| CandidateQueryError::Unavailable)?;
                let entries = if sources_only {
                    Vec::new()
                } else {
                    super::m7_candidates::catalog_entries(
                        &tx,
                        &owner,
                        package_id.map(|id| vec![id]),
                        &settings,
                    )?
                };
                Ok(DelegatedPackageCatalog {
                    authorization_revision,
                    project,
                    entries,
                    sources,
                    complete,
                })
            },
            || CandidateQueryError::Unavailable,
        )
        .await
    }
}

fn sources(
    tx: &rusqlite::Connection,
    owner: &PrincipalId,
    settings: &ConfigPackageSettings,
) -> Result<Vec<DelegatedPackageSource>, CandidateQueryError> {
    use DelegatedSourceAvailability as A;
    let unavailable = |_| CandidateQueryError::Unavailable;
    let mut query = tx
        .prepare(
            "SELECT repository_id, coalesce(display_name,name), resolver_indexed,revision
         FROM repositories WHERE owner_principal_id=?1 ORDER BY repository_id LIMIT 4097",
        )
        .map_err(unavailable)?;
    let rows = query
        .query_map([owner.as_str()], |row| {
            let repository_id: String = row.get(0)?;
            let availability = if settings.hidden_repository_ids.contains(&repository_id) {
                A::Hidden
            } else if !row.get::<_, bool>(2)? {
                A::RefreshRequired
            } else {
                A::Available
            };
            Ok(DelegatedPackageSource {
                source: PackageSourceSelector::Repository { repository_id },
                display_name: row.get(1)?,
                revision: row.get(3)?,
                availability,
            })
        })
        .map_err(unavailable)?;
    let mut sources = rows.collect::<Result<Vec<_>, _>>().map_err(unavailable)?;
    let mut query = tx
        .prepare(
            "SELECT user_package_id, json_extract(manifest_json,'$.displayName'),revision
         FROM user_package_sources WHERE owner_principal_id=?1 ORDER BY user_package_id LIMIT 4097",
        )
        .map_err(unavailable)?;
    let rows = query
        .query_map([owner.as_str()], |row| {
            let id: String = row.get(0)?;
            Ok(DelegatedPackageSource {
                source: PackageSourceSelector::UserPackage {
                    user_package_id: alcomd_application::UserPackageId::parse(&id)
                        .map_err(|_| rusqlite::Error::InvalidQuery)?,
                },
                display_name: row.get(1)?,
                revision: row.get(2)?,
                availability: if settings.hide_local_user_packages {
                    A::Hidden
                } else {
                    A::Available
                },
            })
        })
        .map_err(unavailable)?;
    sources.extend(rows.collect::<Result<Vec<_>, _>>().map_err(unavailable)?);
    if sources.len() > 4096 {
        return Err(CandidateQueryError::LimitExceeded);
    }
    Ok(sources)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    #[test]
    fn delegated_catalog_uses_the_same_registered_sources_and_visibility() {
        let path = std::env::temp_dir().join(format!(
            "alcomd-delegated-catalog-{}.db",
            uuid::Uuid::new_v4()
        ));
        let db = crate::sqlite::initialize_connection(&path).expect("isolated catalog database");
        let owner = PrincipalId::local_owner();
        let public = "33333333-3333-4333-8333-333333333333";
        let private = "44444444-4444-4444-8444-444444444444";
        let local = "66666666-6666-4666-8666-666666666666";
        let credential = "55555555-5555-4555-8555-555555555555";
        let protected_url = "https://example.invalid/private.json?key=fixture";
        for (id, kind, locator, credential) in [
            (
                public,
                "remote",
                "https://example.invalid/public.json",
                None,
            ),
            (private, "remote", protected_url, Some(credential)),
            (local, "local", "fixture-local.json", None),
        ] {
            db.execute(
                "INSERT INTO repositories(repository_id,owner_principal_id,source_kind,source_locator,source_identity_key,credential_reference,issues_json,revision,registered_at_ms,refreshed_at_ms,updated_at_ms,priority,resolver_indexed)
                 VALUES (?1,?2,?3,?4,CAST(?4 AS BLOB),?5,'[]',1,1,1,1,1,1)",
                params![id, owner.as_str(), kind, locator, credential],
            ).expect("isolated index source");
        }
        for (id, version, artifact, headers) in [
            (
                public,
                "1.0.0",
                "https://example.invalid/artifact-1.zip",
                false,
            ),
            (
                public,
                "2.0.0",
                "https://example.invalid/artifact-2.zip",
                false,
            ),
            (
                public,
                "3.0.0",
                "https://example.invalid/artifact-3.zip",
                true,
            ),
            (
                public,
                "4.0.0",
                "https://example.invalid/artifact-4.zip?private=fixture",
                false,
            ),
            (
                private,
                "5.0.0",
                "https://example.invalid/artifact-5.zip?source=fixture",
                true,
            ),
            (
                local,
                "6.0.0",
                "https://example.invalid/artifact-6.zip",
                true,
            ),
        ] {
            db.execute(
                "INSERT INTO repository_package_versions(repository_id,package_id,version_text,semantic_version,artifact_url,manifest_fingerprint,dependencies_json,resolver_ready,download_headers_present)
                 VALUES (?1,'com.example.fixture',?2,?2,?3,zeroblob(32),'{}',1,?4)",
                params![id, version, artifact, headers],
            ).expect("isolated index version");
        }
        let mut settings = alcomd_application::ConfigSnapshot::default()
            .settings
            .packages;
        let observed_sources =
            sources(&db, &owner, &settings).expect("safely projected source catalog");
        for repository_id in [private, local] {
            assert!(observed_sources.iter().any(|row| row.source
                == PackageSourceSelector::Repository {
                    repository_id: repository_id.into()
                }
                && matches!(row.availability, DelegatedSourceAvailability::Available)));
        }
        let catalog = super::super::m4::resolver_catalog(&db, &owner, true, &settings)
            .expect("same Core resolver");
        let mut expected = catalog
            .entries
            .into_iter()
            .map(|row| row.version)
            .collect::<Vec<_>>();
        assert_eq!(
            expected,
            ["1.0.0", "2.0.0", "3.0.0", "4.0.0", "5.0.0", "6.0.0"]
        );
        let versions = |settings: &ConfigPackageSettings| {
            let mut versions = super::super::m7_candidates::catalog_entries(
                &db,
                &owner,
                Some(vec!["com.example.fixture".into()]),
                settings,
            )
            .expect("same candidate evidence")
            .into_iter()
            .map(|row| row.version)
            .collect::<Vec<_>>();
            versions.sort();
            versions
        };
        assert_eq!(versions(&settings), expected);
        settings.hidden_repository_ids.push(private.into());
        expected.retain(|version| version != "5.0.0");
        assert_eq!(versions(&settings), expected);

        let pin = alcomd_application::PackageSourcePin::Repository(
            alcomd_application::RepositoryPackageSourcePin {
                repository_id: private.into(),
                repository_revision: 1,
                source_identity: format!("remote:{protected_url}"),
                manifest_fingerprint: [0; 32],
                package_id: "com.example.fixture".into(),
                version: "5.0.0".into(),
                artifact_url: "https://example.invalid/artifact-5.zip?source=fixture".into(),
                credential_reference: Some(credential.into()),
                download_headers: Some(alcomd_application::PackageDownloadHeaders {
                    local_path: None,
                }),
                archive_sha256: None,
            },
        );
        super::super::m4::validate_delegated_pins(&db, std::slice::from_ref(&pin))
            .expect("exact registered authority");
        db.execute(
            "UPDATE repositories SET credential_reference=NULL WHERE repository_id=?1",
            [private],
        )
        .expect("simulate credential drift");
        assert_eq!(
            super::super::m4::validate_delegated_pins(&db, &[pin])
                .expect_err("stale credential reference")
                .code(),
            alcomd_application::M4ErrorCode::PlanStale
        );
        drop(db);
        std::fs::remove_file(path).expect("cleanup isolated catalog database");
    }
}
