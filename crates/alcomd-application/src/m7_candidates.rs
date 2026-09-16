//! Bounded, side-effect-free candidate query orchestration.
use crate::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const MAX_CANDIDATE_RECORDS: usize = 100_000;
pub const MAX_CANDIDATE_SOURCES: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectCandidateRequest {
    pub project_id: String,
    pub expected_revision: u64,
    pub view: CandidateView,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_snapshot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<CandidateCursor>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CandidateView {
    Summary {
        #[serde(rename = "packageIds", skip_serializing_if = "Option::is_none")]
        package_ids: Option<Vec<String>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        sources: Option<Vec<CandidateSourceOverride>>,
    },
    Versions {
        #[serde(rename = "packageId")]
        package_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<PackageSourceSelector>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateCursor {
    pub snapshot_fingerprint: String,
    pub query_fingerprint: String,
    pub offset: usize,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateSnapshot {
    pub fingerprint: String,
    pub project_revision: u64,
    pub config_revision: u64,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "view", rename_all = "snake_case")]
pub enum CandidatePageItems {
    Summary {
        items: Vec<CandidateSummary>,
    },
    Versions {
        #[serde(rename = "packageId")]
        package_id: String,
        items: Vec<CandidateEvidence>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCandidatePage {
    pub project_id: String,
    pub snapshot: CandidateSnapshot,
    pub show_prerelease: bool,
    pub catalog_complete: bool,
    #[serde(flatten)]
    pub page: CandidatePageItems,
    pub next_cursor: Option<CandidateCursor>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CandidateQueryError {
    InvalidInput,
    PermissionDenied,
    ProjectNotRegistered,
    ProjectManifestInvalid,
    RevisionConflict,
    EvidenceStale,
    LimitExceeded,
    Unavailable,
}

pub struct CandidateStoreSnapshot {
    pub project: ProjectRecord,
    pub entries: Vec<CandidateCatalogEntry>,
    pub fingerprint_records: Vec<String>,
    pub catalog_complete: bool,
}
pub trait CandidateStore: Clone + Send + Sync + 'static {
    fn candidate_snapshot(
        &self,
        owner: PrincipalId,
        project: ProjectId,
        package_ids: Option<Vec<String>>,
    ) -> impl std::future::Future<Output = Result<CandidateStoreSnapshot, CandidateQueryError>> + Send;
}
pub trait CandidateEvidenceEngine: Clone + Send + Sync + 'static {
    fn summarize(
        &self,
        input: &CandidateEvidenceInput,
        package_ids: Option<&[String]>,
        sources: &[CandidateSourceOverride],
    ) -> Vec<CandidateSummary>;
    fn versions(
        &self,
        input: &CandidateEvidenceInput,
        package_id: &str,
        source: Option<&PackageSourceSelector>,
    ) -> Vec<CandidateEvidence>;
    fn fingerprint(&self, records: &[String]) -> String;
    fn encoded_request_size(&self, request: &ProjectCandidateRequest) -> Option<usize>;
    fn encoded_page_size(&self, page: &ProjectCandidatePage) -> Option<usize>;
}

#[derive(Clone)]
pub struct ProjectCandidateApplication<S, E> {
    store: S,
    engine: E,
    config: M7OfficialApplication<S>,
}
impl<S: CandidateStore + OfficialGuiStore, E: CandidateEvidenceEngine>
    ProjectCandidateApplication<S, E>
{
    pub fn new(store: S, engine: E, config: M7OfficialApplication<S>) -> Self {
        Self {
            store,
            engine,
            config,
        }
    }
    pub async fn query(
        &self,
        access: &AccessContext,
        request: ProjectCandidateRequest,
    ) -> Result<ProjectCandidatePage, CandidateQueryError> {
        for permission in [
            Permission::ProjectsRead,
            Permission::RepositoriesRead,
            Permission::PackagesRead,
            Permission::SettingsRead,
        ] {
            access
                .require(permission)
                .map_err(|_| CandidateQueryError::PermissionDenied)?;
        }
        access
            .require_project_read_scope(&request.project_id)
            .map_err(|_| CandidateQueryError::PermissionDenied)?;
        let project_id =
            ProjectId::parse(&request.project_id).map_err(|_| CandidateQueryError::InvalidInput)?;
        if self
            .engine
            .encoded_request_size(&request)
            .is_none_or(|n| n > 64 * 1024)
        {
            return Err(CandidateQueryError::LimitExceeded);
        }
        let query_records = validate_request(&request)?;
        let query_fingerprint = self.engine.fingerprint(&query_records);
        if request
            .cursor
            .as_ref()
            .is_some_and(|c| c.query_fingerprint != query_fingerprint)
        {
            return Err(CandidateQueryError::InvalidInput);
        }
        let config = self
            .config
            .candidate_settings()
            .await
            .map_err(|_| CandidateQueryError::Unavailable)?;
        let ids = match &request.view {
            CandidateView::Summary { package_ids, .. } => package_ids.clone(),
            CandidateView::Versions { package_id, .. } => Some(vec![package_id.clone()]),
        };
        let mut snapshot = self
            .store
            .candidate_snapshot(access.principal().clone(), project_id, ids)
            .await?;
        if snapshot.project.revision.get() != request.expected_revision {
            return Err(
                if request.cursor.is_some() || request.expected_snapshot.is_some() {
                    CandidateQueryError::EvidenceStale
                } else {
                    CandidateQueryError::RevisionConflict
                },
            );
        }
        snapshot.fingerprint_records.extend([
            access.principal().as_str().to_owned(),
            config.revision.to_string(),
            config.settings.packages.show_prerelease.to_string(),
            config
                .settings
                .packages
                .hide_local_user_packages
                .to_string(),
        ]);
        let mut hidden = config.settings.packages.hidden_repository_ids.clone();
        hidden.sort();
        snapshot.fingerprint_records.extend(hidden);
        let fingerprint = self.engine.fingerprint(&snapshot.fingerprint_records);
        if request
            .expected_snapshot
            .as_ref()
            .is_some_and(|v| v != &fingerprint)
            || request
                .cursor
                .as_ref()
                .is_some_and(|c| c.snapshot_fingerprint != fingerprint)
        {
            return Err(CandidateQueryError::EvidenceStale);
        }
        let input = CandidateEvidenceInput {
            project: snapshot.project.observation,
            entries: snapshot.entries,
            settings: config.settings.packages.clone(),
        };
        let offset = request.cursor.as_ref().map_or(0, |c| c.offset);
        let limit = request.limit.unwrap_or(64) as usize;
        let (page, total) = match &request.view {
            CandidateView::Summary {
                package_ids,
                sources,
            } => {
                let items = self.engine.summarize(
                    &input,
                    package_ids.as_deref(),
                    sources.as_deref().unwrap_or_default(),
                );
                // M3 observations intentionally allow wider raw identities than this
                // read DTO. Reject an unrepresentable result without changing M3 data.
                if items.iter().any(|item| {
                    item.package_id.len() > 128
                        || matches!(&item.installed, CandidateInstalled::Locked { version } if version.len() > 256)
                }) {
                    return Err(CandidateQueryError::LimitExceeded);
                }
                let total = items.len();
                (
                    CandidatePageItems::Summary {
                        items: items.into_iter().skip(offset).take(limit).collect(),
                    },
                    total,
                )
            }
            CandidateView::Versions { package_id, source } => {
                let items = self.engine.versions(&input, package_id, source.as_ref());
                let total = items.len();
                (
                    CandidatePageItems::Versions {
                        package_id: package_id.clone(),
                        items: items.into_iter().skip(offset).take(limit).collect(),
                    },
                    total,
                )
            }
        };
        if total > MAX_CANDIDATE_RECORDS {
            return Err(CandidateQueryError::LimitExceeded);
        }
        if offset > total || (offset == total && offset != 0) {
            return Err(CandidateQueryError::InvalidInput);
        }
        if self
            .config
            .candidate_settings()
            .await
            .map_err(|_| CandidateQueryError::Unavailable)?
            != config
        {
            return Err(CandidateQueryError::EvidenceStale);
        }
        let next_cursor = (offset + limit < total).then(|| CandidateCursor {
            snapshot_fingerprint: fingerprint.clone(),
            query_fingerprint,
            offset: offset + limit,
        });
        let page = ProjectCandidatePage {
            project_id: request.project_id,
            snapshot: CandidateSnapshot {
                fingerprint,
                project_revision: request.expected_revision,
                config_revision: config.revision,
            },
            show_prerelease: config.settings.packages.show_prerelease,
            catalog_complete: snapshot.catalog_complete,
            page,
            next_cursor,
        };
        if self
            .engine
            .encoded_page_size(&page)
            .is_none_or(|n| n > 1024 * 1024)
        {
            return Err(CandidateQueryError::LimitExceeded);
        }
        Ok(page)
    }
}

fn source_key(source: &PackageSourceSelector) -> Result<String, CandidateQueryError> {
    Ok(match source {
        PackageSourceSelector::Repository { repository_id } => {
            RepositoryId::parse(repository_id).map_err(|_| CandidateQueryError::InvalidInput)?;
            format!("repository:{repository_id}")
        }
        PackageSourceSelector::UserPackage { user_package_id } => {
            format!("user_package:{user_package_id}")
        }
    })
}
fn package_valid(id: &str) -> bool {
    id.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}
fn digest_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn validate_request(request: &ProjectCandidateRequest) -> Result<Vec<String>, CandidateQueryError> {
    let invalid = CandidateQueryError::InvalidInput;
    if request.expected_revision == 0
        || request.expected_revision > i64::MAX as u64
        || request.limit.is_some_and(|v| !(1..=128).contains(&v))
        || request
            .expected_snapshot
            .as_deref()
            .is_some_and(|v| !digest_valid(v))
    {
        return Err(invalid);
    }
    if let Some(cursor) = &request.cursor
        && (cursor.offset > MAX_CANDIDATE_RECORDS
            || !digest_valid(&cursor.snapshot_fingerprint)
            || !digest_valid(&cursor.query_fingerprint)
            || request
                .expected_snapshot
                .as_ref()
                .is_some_and(|v| v != &cursor.snapshot_fingerprint))
    {
        return Err(invalid);
    }
    let mut records = vec![
        "project-candidates-query-v1".to_owned(),
        request.project_id.clone(),
    ];
    match &request.view {
        CandidateView::Summary {
            package_ids,
            sources,
        } => {
            records.push("summary".to_owned());
            if let Some(ids) = package_ids {
                if ids.is_empty() || ids.len() > 256 || ids.iter().any(|id| !package_valid(id)) {
                    return Err(invalid);
                }
                let ids_sorted = ids.iter().collect::<BTreeSet<_>>();
                if ids_sorted.len() != ids.len() {
                    return Err(invalid);
                }
                records.push("explicit".to_owned());
                records.extend(ids_sorted.into_iter().cloned());
            } else {
                records.push("all".to_owned());
            }
            records.push("sources".to_owned());
            let mut overrides = BTreeSet::new();
            let mut override_ids = BTreeSet::new();
            if let Some(sources) = sources {
                if sources.len() > 256 {
                    return Err(invalid);
                }
                for item in sources {
                    if !package_valid(&item.package_id)
                        || !override_ids.insert(&item.package_id)
                        || package_ids
                            .as_ref()
                            .is_some_and(|ids| !ids.contains(&item.package_id))
                    {
                        return Err(invalid);
                    }
                    overrides.insert((item.package_id.clone(), source_key(&item.source)?));
                }
            }
            for (id, source) in overrides {
                records.extend([id, source]);
            }
        }
        CandidateView::Versions { package_id, source } => {
            if !package_valid(package_id) {
                return Err(invalid);
            }
            records.extend([
                "versions".to_owned(),
                package_id.clone(),
                source
                    .as_ref()
                    .map(source_key)
                    .transpose()?
                    .unwrap_or_default(),
            ]);
        }
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> ProjectCandidateRequest {
        ProjectCandidateRequest {
            project_id: "11111111-1111-4111-8111-111111111111".into(),
            expected_revision: 1,
            view: CandidateView::Summary {
                package_ids: Some(vec!["com.example.b".into(), "com.example.a".into()]),
                sources: None,
            },
            limit: None,
            expected_snapshot: None,
            cursor: None,
        }
    }
    #[test]
    fn canonical_query_ignores_order_and_page_size_but_binds_view_and_sources() {
        let a = request();
        let mut b = a.clone();
        b.limit = Some(128);
        if let CandidateView::Summary {
            package_ids: Some(ids),
            ..
        } = &mut b.view
        {
            ids.reverse();
        }
        assert_eq!(
            validate_request(&a).expect("candidate fixture or query must succeed"),
            validate_request(&b).expect("candidate fixture or query must succeed")
        );
        b.view = CandidateView::Versions {
            package_id: "com.example.a".into(),
            source: None,
        };
        assert_ne!(
            validate_request(&a).expect("candidate fixture or query must succeed"),
            validate_request(&b).expect("candidate fixture or query must succeed")
        );
    }
    #[test]
    fn rejects_duplicate_out_of_subset_and_cursor_mismatches() {
        let mut r = request();
        r.view = CandidateView::Summary {
            package_ids: Some(vec!["com.example.a".into(); 2]),
            sources: None,
        };
        assert_eq!(validate_request(&r), Err(CandidateQueryError::InvalidInput));
        r = request();
        if let CandidateView::Summary { sources, .. } = &mut r.view {
            *sources = Some(vec![CandidateSourceOverride {
                package_id: "com.example.other".into(),
                source: PackageSourceSelector::Repository {
                    repository_id: "22222222-2222-4222-8222-222222222222".into(),
                },
            }]);
        }
        assert_eq!(validate_request(&r), Err(CandidateQueryError::InvalidInput));
        r = request();
        r.expected_snapshot = Some("a".repeat(64));
        r.cursor = Some(CandidateCursor {
            snapshot_fingerprint: "b".repeat(64),
            query_fingerprint: "c".repeat(64),
            offset: 1,
        });
        assert_eq!(validate_request(&r), Err(CandidateQueryError::InvalidInput));
        r = request();
        r.limit = Some(129);
        assert_eq!(validate_request(&r), Err(CandidateQueryError::InvalidInput));
        r = request();
        r.view = CandidateView::Summary {
            package_ids: Some((0..257).map(|i| format!("com.example.{i}")).collect()),
            sources: None,
        };
        assert_eq!(validate_request(&r), Err(CandidateQueryError::InvalidInput));
    }
    #[derive(Clone)]
    struct NeverRead;
    impl CandidateStore for NeverRead {
        async fn candidate_snapshot(
            &self,
            _: PrincipalId,
            _: ProjectId,
            _: Option<Vec<String>>,
        ) -> Result<CandidateStoreSnapshot, CandidateQueryError> {
            panic!("unauthorized query reached store")
        }
    }
    impl OfficialGuiStore for NeverRead {
        async fn list_official_activity(
            &self,
            _: PrincipalId,
            _: Option<OfficialActivityCursor>,
            _: u32,
        ) -> Result<OfficialActivityPage, StoreError> {
            panic!()
        }
        async fn list_official_diagnostics(
            &self,
            _: PrincipalId,
            _: Option<OfficialDiagnosticCursor>,
            _: u32,
        ) -> Result<OfficialDiagnosticPage, StoreError> {
            panic!()
        }
    }
    impl CandidateEvidenceEngine for NeverRead {
        fn summarize(
            &self,
            _: &CandidateEvidenceInput,
            _: Option<&[String]>,
            _: &[CandidateSourceOverride],
        ) -> Vec<CandidateSummary> {
            panic!()
        }
        fn versions(
            &self,
            _: &CandidateEvidenceInput,
            _: &str,
            _: Option<&PackageSourceSelector>,
        ) -> Vec<CandidateEvidence> {
            panic!()
        }
        fn fingerprint(&self, _: &[String]) -> String {
            panic!()
        }
        fn encoded_request_size(&self, _: &ProjectCandidateRequest) -> Option<usize> {
            panic!()
        }
        fn encoded_page_size(&self, _: &ProjectCandidatePage) -> Option<usize> {
            panic!()
        }
    }
    #[tokio::test]
    async fn all_four_permissions_and_exact_project_scope_are_required_before_reads() {
        let application = ProjectCandidateApplication::new(
            NeverRead,
            NeverRead,
            M7OfficialApplication::new(NeverRead, "must-not-open".into()),
        );
        let permissions = [
            Permission::ProjectsRead,
            Permission::RepositoriesRead,
            Permission::PackagesRead,
            Permission::SettingsRead,
        ];
        for omitted in permissions {
            let access = AccessContext::new(
                PrincipalId::local_owner(),
                permissions.into_iter().filter(|p| *p != omitted),
            )
            .with_project_read_scopes([request().project_id]);
            assert_eq!(
                application
                    .query(&access, request())
                    .await
                    .expect_err("query must be denied"),
                CandidateQueryError::PermissionDenied
            );
        }
        let access = AccessContext::new(PrincipalId::local_owner(), permissions)
            .with_project_read_scopes(["22222222-2222-4222-8222-222222222222".into()]);
        assert_eq!(
            application
                .query(&access, request())
                .await
                .expect_err("query must be denied"),
            CandidateQueryError::PermissionDenied
        );
    }

    #[derive(Clone)]
    struct QueryIdentityEngine {
        original_records: Vec<String>,
    }

    impl CandidateEvidenceEngine for QueryIdentityEngine {
        fn summarize(
            &self,
            _: &CandidateEvidenceInput,
            _: Option<&[String]>,
            _: &[CandidateSourceOverride],
        ) -> Vec<CandidateSummary> {
            panic!("changed query must be rejected before candidate computation")
        }
        fn versions(
            &self,
            _: &CandidateEvidenceInput,
            _: &str,
            _: Option<&PackageSourceSelector>,
        ) -> Vec<CandidateEvidence> {
            panic!("changed query must be rejected before candidate computation")
        }
        fn fingerprint(&self, records: &[String]) -> String {
            if records == self.original_records {
                "a".repeat(64)
            } else {
                "b".repeat(64)
            }
        }
        fn encoded_request_size(&self, _: &ProjectCandidateRequest) -> Option<usize> {
            Some(1024)
        }
        fn encoded_page_size(&self, _: &ProjectCandidatePage) -> Option<usize> {
            panic!("changed query cannot produce a page")
        }
    }

    #[tokio::test]
    async fn changed_cursor_ids_source_and_view_are_rejected_before_snapshot_reads() {
        let baseline = request();
        let engine = QueryIdentityEngine {
            original_records: validate_request(&baseline).expect("valid baseline query"),
        };
        let application = ProjectCandidateApplication::new(
            NeverRead,
            engine,
            M7OfficialApplication::new(NeverRead, "must-not-open".into()),
        );
        let mut continued = baseline.clone();
        continued.cursor = Some(CandidateCursor {
            snapshot_fingerprint: "c".repeat(64),
            query_fingerprint: "a".repeat(64),
            offset: 1,
        });
        let mut changed_ids = continued.clone();
        changed_ids.view = CandidateView::Summary {
            package_ids: Some(vec!["com.example.c".into()]),
            sources: None,
        };
        let mut changed_source = continued.clone();
        if let CandidateView::Summary { sources, .. } = &mut changed_source.view {
            *sources = Some(vec![CandidateSourceOverride {
                package_id: "com.example.a".into(),
                source: PackageSourceSelector::Repository {
                    repository_id: "22222222-2222-4222-8222-222222222222".into(),
                },
            }]);
        }
        let mut changed_view = continued;
        changed_view.view = CandidateView::Versions {
            package_id: "com.example.a".into(),
            source: None,
        };
        for changed in [changed_ids, changed_source, changed_view] {
            assert_eq!(
                application
                    .query(&AccessContext::local_owner(), changed)
                    .await
                    .expect_err("cursor cannot continue a different query"),
                CandidateQueryError::InvalidInput
            );
        }
    }
}
