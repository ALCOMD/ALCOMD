//! Bounded catalog and plan projections for the authenticated adapter.
use alcomd_application as app;
use alcomd_protocol as rpc;
use alcomd_store::{DelegatedPackageCatalog, DelegatedSourceAvailability};
use serde::Serialize;
use serde_json::json;
use std::collections::{BTreeMap, HashMap};

use super::{Applications, ConnectionState, DispatchAction, error_action, success_action};

const MAX_DATA_BYTES: usize = 3 * 1024 * 1024;
const MAX_SAFE: u64 = 9_007_199_254_740_991;

pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    apps: &Applications,
    access: &app::AccessContext,
    raw_bytes: usize,
) -> DispatchAction {
    if let Some(action) = super::require_capability(
        &request.id,
        state,
        rpc::CAPABILITY_MCP_PACKAGES_CATALOG_PLAN_V1,
    ) {
        return action;
    }
    if raw_bytes > 64 * 1024 {
        return super::invalid(request.id);
    }
    let Ok(authentication) = super::m8_tools_rpc::auth(state, access) else {
        return error_action(Some(request.id), rpc::RpcError::permission_denied(), false);
    };
    let id = request.id;
    if request.method == rpc::METHOD_MCP_PACKAGES_QUERY {
        let Ok(input) = serde_json::from_value::<rpc::McpPackageQuery>(request.params) else {
            return super::invalid(id);
        };
        return match query(apps, authentication, input).await {
            Ok(page) => success_action(id, page, None),
            Err(error) => super::m7_candidates_rpc::failure(id, error),
        };
    }
    let Ok(input) = serde_json::from_value::<rpc::McpPackagePlanRequest>(request.params) else {
        return super::invalid(id);
    };
    match plan(apps, access, authentication, input).await {
        Ok(plan) => success_action(id, plan, None),
        Err(error) => super::m4_rpc::m4_error(id, error),
    }
}

type QueryError = app::CandidateQueryError;

fn valid_revision(revision: u64) -> bool {
    (1..=MAX_SAFE).contains(&revision)
}

fn source_is_valid(source: &rpc::PackageSourceSelector) -> bool {
    match source {
        rpc::PackageSourceSelector::Repository { repository_id } => {
            app::RepositoryId::parse(repository_id).is_ok()
        }
        rpc::PackageSourceSelector::UserPackage { user_package_id } => {
            app::UserPackageId::parse(user_package_id).is_ok()
        }
    }
}

fn source_selector(source: &app::PackageSourceSelector) -> rpc::PackageSourceSelector {
    match source {
        app::PackageSourceSelector::Repository { repository_id } => {
            rpc::PackageSourceSelector::Repository {
                repository_id: repository_id.clone(),
            }
        }
        app::PackageSourceSelector::UserPackage { user_package_id } => {
            rpc::PackageSourceSelector::UserPackage {
                user_package_id: user_package_id.to_string(),
            }
        }
    }
}

fn validate_query(input: &rpc::McpPackageQuery) -> Result<Option<app::ProjectId>, QueryError> {
    let invalid = QueryError::InvalidInput;
    if input.limit.is_some_and(|limit| !(1..=200).contains(&limit)) {
        return Err(invalid);
    }
    match &input.view {
        rpc::McpPackageView::Sources if input.project.is_some() => return Err(invalid),
        rpc::McpPackageView::Summary {
            query,
            repository_id,
        } => {
            if query
                .as_ref()
                .is_some_and(|query| query.len() > 256 || query.chars().any(char::is_control))
                || repository_id
                    .as_ref()
                    .is_some_and(|id| app::RepositoryId::parse(id).is_err())
            {
                return Err(invalid);
            }
        }
        rpc::McpPackageView::Versions {
            package_id,
            version,
            source,
        } => {
            if !app::valid_presentation_package_id(package_id)
                || version
                    .as_deref()
                    .is_some_and(|v| v.len() > 256 || alcomd_vpm::classify_prerelease(v).is_none())
                || source.as_ref().is_some_and(|s| !source_is_valid(s))
            {
                return Err(invalid);
            }
        }
        rpc::McpPackageView::Sources => {}
    }
    input
        .project
        .as_ref()
        .map(|project| {
            if !valid_revision(project.expected_revision) {
                return Err(invalid);
            }
            app::ProjectId::parse(&project.project_id).map_err(|_| invalid)
        })
        .transpose()
}

async fn query(
    apps: &Applications,
    authentication: app::ToolAuthentication,
    input: rpc::McpPackageQuery,
) -> Result<rpc::McpPackagePage, QueryError> {
    let project = validate_query(&input)?;
    let package_id = match &input.view {
        rpc::McpPackageView::Versions { package_id, .. } => Some(package_id.clone()),
        _ => None,
    };
    let settings = apps
        .m4
        .package_source_settings()
        .await
        .map_err(|_| QueryError::Unavailable)?;
    let mut catalog = apps
        .store
        .delegated_package_catalog(
            authentication.clone(),
            project,
            package_id,
            matches!(input.view, rpc::McpPackageView::Sources),
            settings.clone(),
        )
        .await?;
    if let (Some(expected), Some(project)) = (&input.project, &catalog.project)
        && project.content_revision.get() != expected.expected_revision
    {
        return Err(QueryError::RevisionConflict);
    }
    let project_binding = catalog
        .project
        .as_ref()
        .map(|project| {
            serde_json::to_vec(&project.observation)
                .map(|bytes| app::tool_cursor_mac(&apps.tools_cursor_key, &bytes))
        })
        .transpose()
        .map_err(|_| QueryError::Unavailable)?;
    let source_revisions = catalog
        .sources
        .iter()
        .map(|source| (&source.source, source.revision))
        .collect::<Vec<_>>();
    let binding = serde_json::to_vec(&json!([
        "mcp.packages.query.v1",
        authentication.binding(),
        catalog.authorization_revision,
        input.view,
        input.project,
        project_binding,
        catalog.complete,
        source_revisions,
        settings.show_prerelease,
        settings.prefer_prerelease,
        settings.hidden_repository_ids,
        settings.hidden_repository_packages,
        settings.hide_local_user_packages,
    ]))
    .map_err(|_| QueryError::Unavailable)?;
    let catalog_complete = catalog.complete;
    let page = CatalogPage {
        key: &apps.tools_cursor_key,
        binding,
        cursor: input.cursor.as_deref(),
        limit: input.limit.unwrap_or(100) as usize,
    };
    let (items, snapshot, next_cursor) = match input.view {
        rpc::McpPackageView::Sources => {
            let rows = catalog
                .sources
                .into_iter()
                .map(|source| rpc::McpPackageSource {
                    source: source_selector(&source.source),
                    display_name: source.display_name,
                    availability: match source.availability {
                        DelegatedSourceAvailability::Available => {
                            rpc::McpPackageSourceAvailability::Available
                        }
                        DelegatedSourceAvailability::Hidden => {
                            rpc::McpPackageSourceAvailability::Hidden
                        }
                        DelegatedSourceAvailability::RefreshRequired => {
                            rpc::McpPackageSourceAvailability::RefreshRequired
                        }
                    },
                })
                .collect();
            let (rows, snapshot, next) = page.read(rows)?;
            (rpc::McpPackageItems::Sources(rows), snapshot, next)
        }
        rpc::McpPackageView::Summary {
            query,
            repository_id,
        } => {
            if let Some(id) = repository_id {
                catalog.entries.retain(|row| matches!(&row.source, app::PackageSourceSelector::Repository { repository_id } if repository_id == &id));
            }
            let rows = summary(catalog, settings, query.as_deref());
            let (rows, snapshot, next) = page.read(rows)?;
            (rpc::McpPackageItems::Summary(rows), snapshot, next)
        }
        rpc::McpPackageView::Versions {
            package_id,
            version,
            source,
        } => {
            let rows = versions(
                catalog,
                settings,
                &package_id,
                version.as_deref(),
                source.as_ref(),
            );
            let (rows, snapshot, next) = page.read(rows)?;
            (rpc::McpPackageItems::Versions(rows), snapshot, next)
        }
    };
    Ok(rpc::McpPackagePage {
        items,
        snapshot,
        catalog_complete,
        next_cursor,
    })
}

struct CatalogPage<'a> {
    key: &'a [u8; 32],
    binding: Vec<u8>,
    cursor: Option<&'a str>,
    limit: usize,
}

impl CatalogPage<'_> {
    fn read<T: Serialize>(
        &self,
        rows: Vec<T>,
    ) -> Result<(Vec<T>, String, Option<String>), QueryError> {
        let mut sizes = Vec::with_capacity(rows.len());
        let mut evidence = self.binding.clone();
        for row in &rows {
            let bytes = serde_json::to_vec(row).map_err(|_| QueryError::Unavailable)?;
            sizes.push(bytes.len());
            evidence.extend(app::tool_cursor_mac(self.key, &bytes).bytes());
        }
        let snapshot = app::tool_cursor_mac(self.key, &evidence);
        let offset = match self.cursor {
            None => 0,
            Some(cursor) => {
                let (offset, _) = cursor.split_once('.').ok_or(QueryError::RevisionConflict)?;
                let offset = offset
                    .parse::<usize>()
                    .map_err(|_| QueryError::RevisionConflict)?;
                if offset >= rows.len() || self.cursor_for(offset, &snapshot) != cursor {
                    return Err(QueryError::RevisionConflict);
                }
                offset
            }
        };
        let mut bytes = 4096;
        let mut count = 0;
        for size in sizes.into_iter().skip(offset).take(self.limit) {
            bytes += size + 1;
            if bytes > MAX_DATA_BYTES {
                if count == 0 {
                    return Err(QueryError::LimitExceeded);
                }
                break;
            }
            count += 1;
        }
        let after = offset + count;
        let next = (after < rows.len()).then(|| self.cursor_for(after, &snapshot));
        Ok((
            rows.into_iter().skip(offset).take(count).collect(),
            snapshot,
            next,
        ))
    }

    fn cursor_for(&self, offset: usize, snapshot: &str) -> String {
        let mac = app::tool_cursor_mac(
            self.key,
            format!("mcp.packages.cursor.v1:{offset}:{snapshot}").as_bytes(),
        );
        format!("{offset}.{mac}")
    }
}

fn candidate_input(
    catalog: &mut DelegatedPackageCatalog,
    settings: app::ConfigPackageSettings,
) -> Option<app::CandidateEvidenceInput> {
    Some(app::CandidateEvidenceInput {
        installed_presentations: Default::default(),
        source_metadata: HashMap::new(),
        project: catalog.project.take()?.observation,
        entries: std::mem::take(&mut catalog.entries),
        settings,
    })
}

fn compatibility(value: app::CandidateEvidence) -> Option<rpc::McpPackageCompatibility> {
    serde_json::to_value(value).ok().and_then(|value| {
        serde_json::from_value(json!({
        "unity": value["unity"], "eligibility": value["eligibility"], "reasons": value["reasons"],
    })).ok()
    })
}

fn summary(
    mut catalog: DelegatedPackageCatalog,
    settings: app::ConfigPackageSettings,
    query: Option<&str>,
) -> Vec<rpc::McpPackageSummary> {
    let candidate = candidate_input(&mut catalog, settings);
    let project_rows = candidate
        .as_ref()
        .map(|input| alcomd_vpm::evaluate_candidate_summary(input, None, &[]))
        .unwrap_or_default()
        .into_iter()
        .map(|row| (row.package_id.clone(), row))
        .collect::<BTreeMap<_, _>>();
    let mut groups: BTreeMap<&str, Vec<&app::CandidateCatalogEntry>> = BTreeMap::new();
    for entry in candidate
        .as_ref()
        .map_or(catalog.entries.as_slice(), |input| input.entries.as_slice())
    {
        groups.entry(&entry.package_id).or_default().push(entry);
    }
    let query = query.unwrap_or_default().trim().to_lowercase();
    groups
        .into_iter()
        .filter_map(|(id, rows)| {
            let representative = rows
                .iter()
                .copied()
                .filter(|row| !row.yanked)
                .max_by(|a, b| alcomd_vpm::compare_catalog_versions(&a.version, &b.version))?;
            if !query.is_empty()
                && !id.to_lowercase().contains(&query)
                && !rows.iter().any(|row| {
                    row.display_name
                        .as_ref()
                        .is_some_and(|name| name.to_lowercase().contains(&query))
                        || row.presentation.as_ref().is_some_and(|metadata| {
                            metadata
                                .keywords
                                .iter()
                                .chain(&metadata.aliases)
                                .any(|term| term.to_lowercase().contains(&query))
                        })
                })
            {
                return None;
            }
            let mut sources = Vec::new();
            for row in &rows {
                let source = source_selector(&row.source);
                if !sources.contains(&source) {
                    sources.push(source);
                }
            }
            let existing = project_rows.get(id);
            let locked_version = existing.and_then(|row| match &row.installed {
                app::CandidateInstalled::Locked { version } => Some(version.clone()),
                _ => None,
            });
            let (version, compatibility) = match existing.map(|row| &row.recommended) {
                Some(app::CandidateChoice::Candidate { candidate }) => {
                    (candidate.version.clone(), compatibility(candidate.clone()))
                }
                Some(choice) => (representative.version.clone(), Some(blocked(choice))),
                None => (representative.version.clone(), None),
            };
            Some(rpc::McpPackageSummary {
                package_id: id.into(),
                display_name: representative.display_name.clone(),
                version,
                locked_version,
                sources,
                compatibility,
            })
        })
        .collect()
}

fn blocked(choice: &app::CandidateChoice) -> rpc::McpPackageCompatibility {
    let reasons = match choice {
        app::CandidateChoice::None { reasons } | app::CandidateChoice::Unknown { reasons } => {
            reasons.clone()
        }
        app::CandidateChoice::Ambiguous { reason } => vec![*reason],
        app::CandidateChoice::Candidate { .. } => Vec::new(),
    };
    rpc::McpPackageCompatibility {
        unity: rpc::CandidateUnity::Unknown,
        eligibility: if matches!(choice, app::CandidateChoice::Unknown { .. }) {
            rpc::CandidateEligibility::Unknown
        } else {
            rpc::CandidateEligibility::Blocked
        },
        reasons: serde_json::to_value(reasons)
            .ok()
            .and_then(|value| serde_json::from_value(value).ok())
            .unwrap_or_default(),
    }
}

fn versions(
    mut catalog: DelegatedPackageCatalog,
    settings: app::ConfigPackageSettings,
    package_id: &str,
    version: Option<&str>,
    source: Option<&rpc::PackageSourceSelector>,
) -> Vec<rpc::McpPackageVersion> {
    catalog
        .entries
        .retain(|row| source.is_none_or(|source| source == &source_selector(&row.source)));
    let candidate = candidate_input(&mut catalog, settings);
    let evidence = candidate
        .as_ref()
        .map(|input| alcomd_vpm::evaluate_candidate_versions(input, package_id, None))
        .unwrap_or_default();
    let entries = candidate.map_or(catalog.entries, |input| input.entries);
    let mut rows = entries
        .into_iter()
        .filter(|row| version.is_none_or(|version| version == row.version))
        .map(|row| {
            let compatibility = evidence
                .iter()
                .find(|item| item.source == row.source && item.version == row.version)
                .cloned()
                .and_then(compatibility);
            let metadata = row.presentation.unwrap_or_default();
            rpc::McpPackageVersion {
                package_id: row.package_id,
                version: row.version,
                source: source_selector(&row.source),
                display_name: row.display_name,
                description: row.description,
                unity: row.unity,
                unity_release: row.unity_release,
                documentation_url: row
                    .links
                    .documentation
                    .filter(|url| app::mcp_package_documentation_url(url)),
                changelog_url: row
                    .links
                    .changelog
                    .filter(|url| app::mcp_package_documentation_url(url)),
                aliases: metadata.aliases,
                keywords: metadata.keywords,
                legacy_packages: metadata.legacy_packages,
                dependencies: row.dependencies,
                yanked: row.yanked || metadata.yanked,
                compatibility,
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| alcomd_vpm::compare_catalog_versions(&b.version, &a.version));
    rows
}

fn plan_request(
    input: rpc::McpPackagePlanRequest,
) -> Result<app::PackagePlanRequest, app::M4Error> {
    let invalid = || app::M4Error::new(app::M4ErrorCode::InvalidInput);
    let package_id = || input.package_id.clone().ok_or_else(invalid);
    let project_id = input.project.project_id;
    let expected_revision = input.project.expected_revision;
    if input
        .package_id
        .as_deref()
        .is_some_and(|id| !app::valid_presentation_package_id(id))
        || !valid_revision(expected_revision)
        || input
            .source
            .as_ref()
            .is_some_and(|source| !source_is_valid(source))
    {
        return Err(invalid());
    }
    let is_upgrade = matches!(&input.intent, rpc::McpPackageIntent::Upgrade { .. });
    let valid_range = |value: &str| {
        !value.is_empty() && value.len() <= 1024 && !value.chars().any(char::is_control)
    };
    let valid_version =
        |value: &str| value.len() <= 256 && alcomd_vpm::classify_prerelease(value).is_some();
    let no_target = || input.package_id.is_none() && input.source.is_none();
    let (action, params) = match input.intent {
        rpc::McpPackageIntent::Install {
            version_selector,
            include_prerelease,
            allow_unity_incompatible,
        }
        | rpc::McpPackageIntent::Upgrade {
            version_selector,
            include_prerelease,
            allow_unity_incompatible,
        } => {
            if allow_unity_incompatible
                && !matches!(
                    &version_selector,
                    Some(rpc::McpPackageVersionSelector::Exact { .. })
                )
            {
                return Err(invalid());
            }
            let version_range = match version_selector {
                None | Some(rpc::McpPackageVersionSelector::LatestCompatible) => None,
                Some(rpc::McpPackageVersionSelector::Exact { version })
                    if valid_version(&version) =>
                {
                    Some(format!("={version}"))
                }
                Some(rpc::McpPackageVersionSelector::Range { range }) if valid_range(&range) => {
                    Some(range)
                }
                _ => return Err(invalid()),
            };
            (
                if is_upgrade {
                    app::PlanAction::Upgrade
                } else {
                    app::PlanAction::Install
                },
                serde_json::to_value(rpc::PackagePlanInstallParams {
                    project_id,
                    expected_revision,
                    package_id: package_id()?,
                    version_range,
                    repository_id: None,
                    source: input.source,
                    include_prerelease,
                    allow_unity_incompatible,
                }),
            )
        }
        rpc::McpPackageIntent::Downgrade {
            version,
            allow_unity_incompatible,
        } if valid_version(&version) => (
            app::PlanAction::Downgrade,
            serde_json::to_value(rpc::PackagePlanDowngradeParams {
                project_id,
                expected_revision,
                package_id: package_id()?,
                version,
                repository_id: None,
                source: input.source,
                allow_unity_incompatible,
            }),
        ),
        rpc::McpPackageIntent::Remove if input.source.is_none() => (
            app::PlanAction::Remove,
            serde_json::to_value(rpc::PackagePlanRemoveParams {
                project_id,
                expected_revision,
                package_id: package_id()?,
            }),
        ),
        rpc::McpPackageIntent::Resolve { include_prerelease } if no_target() => (
            app::PlanAction::Resolve,
            serde_json::to_value(rpc::PackagePlanResolveParams {
                project_id,
                expected_revision,
                include_prerelease,
            }),
        ),
        rpc::McpPackageIntent::Reinstall {
            selection,
            mut sources,
        } => {
            let selection = match selection {
                Some(selection) if no_target() => selection,
                None if sources.is_empty() => {
                    let package_id = package_id()?;
                    sources.extend(input.source.map(|source| rpc::PackageReinstallSource {
                        package_id: package_id.clone(),
                        source,
                    }));
                    rpc::PackageReinstallSelection::Packages {
                        package_ids: vec![package_id],
                    }
                }
                _ => return Err(invalid()),
            };
            if matches!(&selection, rpc::PackageReinstallSelection::Packages { package_ids }
                if package_ids.is_empty() || package_ids.len() > 256 || package_ids.iter().any(|id| !app::valid_presentation_package_id(id)))
                || sources.len() > 256
                || sources.iter().any(|item| {
                    !app::valid_presentation_package_id(&item.package_id)
                        || !source_is_valid(&item.source)
                })
            {
                return Err(invalid());
            }
            (
                app::PlanAction::Reinstall,
                serde_json::to_value(rpc::PackagePlanReinstallParams {
                    project_id,
                    expected_revision,
                    selection,
                    sources,
                }),
            )
        }
        rpc::McpPackageIntent::Bulk { intents }
            if no_target() && (1..=256).contains(&intents.len()) =>
        {
            for intent in &intents {
                let (id, range, source) = match intent {
                    rpc::PackageBulkIntent::Install {
                        package_id,
                        version_range,
                        source,
                        ..
                    }
                    | rpc::PackageBulkIntent::Upgrade {
                        package_id,
                        version_range,
                        source,
                        ..
                    } => (package_id, version_range.as_deref(), source.as_ref()),
                    rpc::PackageBulkIntent::Reinstall { package_id, source } => {
                        (package_id, None, source.as_ref())
                    }
                    rpc::PackageBulkIntent::Remove { package_id } => (package_id, None, None),
                };
                if !app::valid_presentation_package_id(id)
                    || range.is_some_and(|range| !valid_range(range))
                    || source.is_some_and(|source| !source_is_valid(source))
                {
                    return Err(invalid());
                }
            }
            (
                app::PlanAction::Bulk,
                serde_json::to_value(rpc::PackagePlanBulkParams {
                    project_id,
                    expected_revision,
                    intents,
                }),
            )
        }
        _ => return Err(invalid()),
    };
    super::m4_rpc::plan_request(action, params.map_err(|_| invalid())?).map_err(|_| invalid())
}

async fn plan(
    apps: &Applications,
    access: &app::AccessContext,
    authentication: app::ToolAuthentication,
    input: rpc::McpPackagePlanRequest,
) -> Result<rpc::McpPackagePlan, app::M4Error> {
    let settings = apps.m4.package_source_settings().await?;
    let request = plan_request(input)?;
    let sources = apps
        .store
        .delegated_package_catalog(
            authentication.clone(),
            Some(request.project_id),
            None,
            true,
            settings,
        )
        .await
        .map_err(|error| {
            app::M4Error::new(match error {
                QueryError::PermissionDenied => app::M4ErrorCode::PermissionDenied,
                QueryError::ProjectNotRegistered => app::M4ErrorCode::ProjectNotRegistered,
                _ => app::M4ErrorCode::Internal,
            })
        })?
        .sources;
    let plan = apps
        .m4
        .plan_authenticated(access, request, Some(authentication))
        .await?;
    let mut plan = super::m4_rpc::plan_to_rpc(plan);
    let selected = plan
        .change_set
        .mutations
        .iter_mut()
        .filter_map(|mutation| {
            let source = match mutation.source.take()? {
                rpc::PackageSourcePin::Repository(source) => {
                    rpc::PackageSourceSelector::Repository {
                        repository_id: source.repository_id,
                    }
                }
                rpc::PackageSourcePin::UserPackage(source) => {
                    rpc::PackageSourceSelector::UserPackage {
                        user_package_id: source.user_package_id,
                    }
                }
            };
            let display_name = sources
                .iter()
                .find(|item| source_selector(&item.source) == source)
                .and_then(|item| item.display_name.clone());
            Some(rpc::McpPackagePlanSource {
                package_id: mutation.package_id.clone(),
                source,
                display_name,
            })
        })
        .collect();
    let plan = rpc::McpPackagePlan {
        plan,
        sources: selected,
    };
    if serde_json::to_vec(&plan).map_or(true, |bytes| bytes.len() > MAX_DATA_BYTES) {
        return Err(app::M4Error::new(app::M4ErrorCode::PlanTooLarge));
    }
    Ok(plan)
}
