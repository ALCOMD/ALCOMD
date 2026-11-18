use alcomd_application::{
    CandidateCatalogEntry, CandidateQueryError, ConfigPackageSettings, PackageSourceSelector,
    Permission, PrincipalId, ProjectId, ProjectRecord, ToolAuthentication, delegated_package_url,
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
    AuthenticationRequired,
    Unsupported,
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
                let complete = !sources.iter().any(|s| {
                    matches!(s.availability, DelegatedSourceAvailability::RefreshRequired)
                });
                let entries = if sources_only {
                    Vec::new()
                } else {
                    entries(&tx, &owner, &settings, package_id)?
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
            "SELECT repository_id, coalesce(display_name,name), source_kind, source_locator,
                credential_reference IS NOT NULL, resolver_indexed,revision
         FROM repositories WHERE owner_principal_id=?1 ORDER BY repository_id LIMIT 4097",
        )
        .map_err(unavailable)?;
    let rows = query
        .query_map([owner.as_str()], |row| {
            let repository_id: String = row.get(0)?;
            let locator: String = row.get(3)?;
            let availability = if settings.hidden_repository_ids.contains(&repository_id) {
                A::Hidden
            } else if row.get::<_, bool>(4)? {
                A::AuthenticationRequired
            } else if row.get::<_, String>(2)? != "remote" || !delegated_package_url(&locator) {
                A::Unsupported
            } else if !row.get::<_, bool>(5)? {
                A::RefreshRequired
            } else {
                A::Available
            };
            Ok(DelegatedPackageSource {
                source: PackageSourceSelector::Repository { repository_id },
                display_name: row.get(1)?,
                revision: row.get(6)?,
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

fn entries(
    tx: &rusqlite::Connection,
    owner: &PrincipalId,
    settings: &ConfigPackageSettings,
    package_id: Option<String>,
) -> Result<Vec<CandidateCatalogEntry>, CandidateQueryError> {
    use alcomd_application::ResolverCatalogSource as S;
    let unavailable = |_| CandidateQueryError::Unavailable;
    let catalog = resolver_catalog(tx, settings.clone()).map_err(unavailable)?;
    let allowed = catalog
        .entries
        .into_iter()
        .map(|row| {
            let source = match row.source {
                S::Repository { repository_id, .. } => (false, repository_id),
                S::UserPackage {
                    user_package_id, ..
                } => (true, user_package_id.to_string()),
            };
            (source, row.package_id, row.version)
        })
        .collect::<std::collections::HashSet<_>>();
    let mut entries =
        super::m7_candidates::catalog_entries(tx, owner, package_id.map(|id| vec![id]), settings)?;
    entries.retain(|row| {
        row.metadata_ready
            && allowed.contains(&(
                source_id(&row.source),
                row.package_id.clone(),
                row.version.clone(),
            ))
    });
    Ok(entries)
}

pub(super) fn resolver_catalog(
    db: &rusqlite::Connection,
    mut settings: ConfigPackageSettings,
) -> Result<alcomd_application::ResolverCatalog, alcomd_application::M4Error> {
    use alcomd_application::{M4Error, M4ErrorCode};
    let failure = |error| M4Error::new(M4ErrorCode::StoreUnavailable).with_source(error);
    let owner = PrincipalId::local_owner();
    let mut query = db
        .prepare(
            "SELECT repository_id,source_kind,source_locator,credential_reference IS NOT NULL
         FROM repositories WHERE owner_principal_id=?1 ORDER BY repository_id LIMIT 4097",
        )
        .map_err(failure)?;
    let rows = query
        .query_map([owner.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, bool>(3)?,
            ))
        })
        .map_err(failure)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(failure)?;
    if rows.len() > 4096 {
        return Err(M4Error::new(M4ErrorCode::PlanTooLarge));
    }
    settings
        .hidden_repository_ids
        .extend(
            rows.into_iter()
                .filter_map(|(id, kind, locator, authenticated)| {
                    (authenticated || kind != "remote" || !delegated_package_url(&locator))
                        .then_some(id)
                }),
        );
    let mut catalog = super::m4::resolver_catalog(db, &owner, true, &settings)?;
    catalog.restrict_to_delegated_sources();
    Ok(catalog)
}

fn source_id(source: &PackageSourceSelector) -> (bool, String) {
    match source {
        PackageSourceSelector::Repository { repository_id } => (false, repository_id.clone()),
        PackageSourceSelector::UserPackage { user_package_id } => {
            (true, user_package_id.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;

    #[test]
    fn delegated_catalog_excludes_authentication_and_keeps_exact_anonymous_versions() {
        let path = std::env::temp_dir().join(format!(
            "alcomd-delegated-catalog-{}.db",
            uuid::Uuid::new_v4()
        ));
        let db = crate::sqlite::initialize_connection(&path).expect("isolated catalog database");
        let owner = PrincipalId::local_owner();
        let public = "33333333-3333-4333-8333-333333333333";
        let private = "44444444-4444-4444-8444-444444444444";
        for (id, locator, credential, indexed) in [
            (public, "https://example.invalid/public.json", None, true),
            (
                private,
                "https://example.invalid/private.json",
                Some("55555555-5555-4555-8555-555555555555"),
                false,
            ),
        ] {
            db.execute(
                "INSERT INTO repositories(repository_id,owner_principal_id,source_kind,source_locator,source_identity_key,credential_reference,issues_json,revision,registered_at_ms,refreshed_at_ms,updated_at_ms,priority,resolver_indexed)
                 VALUES (?1,?2,'remote',?3,CAST(?3 AS BLOB),?4,'[]',1,1,1,1,1,?5)",
                params![id, owner.as_str(), locator, credential, indexed],
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
                "https://example.invalid/artifact-5.zip",
                false,
            ),
        ] {
            db.execute(
                "INSERT INTO repository_package_versions(repository_id,package_id,version_text,semantic_version,artifact_url,manifest_fingerprint,dependencies_json,resolver_ready,download_headers_present)
                 VALUES (?1,'com.example.fixture',?2,?2,?3,zeroblob(32),'{}',1,?4)",
                params![id, version, artifact, headers],
            ).expect("isolated index version");
        }
        let settings = alcomd_application::ConfigSnapshot::default()
            .settings
            .packages;
        let observed_sources =
            sources(&db, &owner, &settings).expect("safely projected source catalog");
        assert!(
            observed_sources
                .iter()
                .any(|row| source_id(&row.source).1 == private
                    && matches!(
                        row.availability,
                        DelegatedSourceAvailability::AuthenticationRequired
                    ))
        );
        let catalog = resolver_catalog(&db, settings.clone()).expect("delegated resolver index");
        assert!(
            catalog.complete,
            "unindexed excluded source does not block anonymous sources"
        );
        let results = entries(&db, &owner, &settings, Some("com.example.fixture".into()))
            .expect("delegated display index");
        assert_eq!(
            catalog
                .entries
                .iter()
                .map(|row| row.version.as_str())
                .collect::<Vec<_>>(),
            ["1.0.0", "2.0.0"]
        );
        assert_eq!(
            results
                .iter()
                .map(|row| row.version.as_str())
                .collect::<Vec<_>>(),
            ["1.0.0", "2.0.0"]
        );
        drop(db);
        std::fs::remove_file(path).expect("cleanup isolated catalog database");
    }
}
