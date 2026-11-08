//! Pure candidate evidence over registered observations. This never runs the resolver.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use alcomd_application::{
    CandidateCatalogEntry, CandidateChoice, CandidateClassification, CandidateEligibility,
    CandidateEvidence, CandidateEvidenceInput, CandidateInstalled, CandidateLinks,
    CandidateNoUpdateReason, CandidateProvider, CandidateReason, CandidateRelation,
    CandidateSourceOverride, CandidateSummary, CandidateUnity, CandidateUpdate,
    PackageSourceSelector,
};
use semver::{BuildMetadata, Version};
use sha2::{Digest, Sha256};

use crate::range::compare_precedence;
use crate::resolver::source_group_is_ambiguous;
use crate::unity_version::{UnityVersion, meets_minimum, package_minimum};

#[derive(Clone, Copy, Debug, Default)]
pub struct ProjectCandidateEngine;

impl alcomd_application::CandidateEvidenceEngine for ProjectCandidateEngine {
    fn encoded_request_size(
        &self,
        request: &alcomd_application::ProjectCandidateRequest,
    ) -> Option<usize> {
        serde_json::to_vec(request).ok().map(|bytes| bytes.len())
    }

    fn encoded_page_size(&self, page: &alcomd_application::ProjectCandidatePage) -> Option<usize> {
        serde_json::to_vec(page).ok().map(|bytes| bytes.len())
    }

    fn summarize(
        &self,
        input: &CandidateEvidenceInput,
        package_ids: Option<&[String]>,
        sources: &[CandidateSourceOverride],
    ) -> Vec<CandidateSummary> {
        evaluate_candidate_summary(input, package_ids, sources)
    }

    fn versions(
        &self,
        input: &CandidateEvidenceInput,
        package_id: &str,
        source: Option<&PackageSourceSelector>,
    ) -> Vec<CandidateEvidence> {
        evaluate_candidate_versions(input, package_id, source)
    }

    fn fingerprint(&self, records: &[String]) -> String {
        let mut hash = Sha256::new();
        for record in records {
            hash.update((record.len() as u64).to_be_bytes());
            hash.update(record.as_bytes());
        }
        hash.finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}

struct Observed<'a> {
    entry: &'a CandidateCatalogEntry,
    version: Option<Version>,
    evidence: CandidateEvidence,
}

/// Computes complete summaries before the application slices a response page.
pub fn evaluate_candidate_summary(
    input: &CandidateEvidenceInput,
    package_ids: Option<&[String]>,
    sources: &[CandidateSourceOverride],
) -> Vec<CandidateSummary> {
    let mut catalog: BTreeMap<&str, Vec<&CandidateCatalogEntry>> = BTreeMap::new();
    for entry in &input.entries {
        if entry_visible(input, entry) {
            catalog.entry(&entry.package_id).or_default().push(entry);
        }
    }
    let presentations: BTreeMap<_, _> = input
        .installed_presentations
        .packages
        .iter()
        .map(|item| (item.package_id.as_str(), item))
        .collect();
    let direct: BTreeSet<&str> = input
        .project
        .direct_dependencies
        .iter()
        .map(|item| item.package_id.as_str())
        .collect();
    let locked: BTreeMap<&str, &str> = input
        .project
        .locked_dependencies
        .iter()
        .map(|item| (item.package_id.as_str(), item.value.as_str()))
        .collect();
    let overrides: BTreeMap<&str, &PackageSourceSelector> = sources
        .iter()
        .map(|item| (item.package_id.as_str(), &item.source))
        .collect();
    let ids: BTreeSet<&str> = match package_ids {
        Some(ids) => ids.iter().map(String::as_str).collect(),
        None => catalog
            .keys()
            .copied()
            .chain(direct.iter().copied())
            .chain(locked.keys().copied())
            .collect(),
    };
    let mut summaries: Vec<_> = ids
        .into_iter()
        .map(|package_id| {
            let source = overrides.get(package_id).copied();
            // Provider membership describes the whole visible package set, not the
            // chosen winner or a temporary explicit source override.
            let providers: BTreeMap<String, CandidateProvider> = catalog
                .get(package_id)
                .into_iter()
                .flatten()
                .filter_map(|entry| {
                    let metadata = input.source_metadata.get(&source_key(&entry.source))?;
                    Some((
                        source_key(&entry.source),
                        CandidateProvider {
                            display_name: metadata.display_name.clone(),
                            name: metadata.name.clone(),
                            builtin: metadata.builtin,
                            source_kind: metadata.kind,
                            display_order: metadata.display_order,
                            source: entry.source.clone(),
                            source_revision: entry.source_revision,
                        },
                    ))
                })
                .collect();
            let installed = installed_from_lock(locked.get(package_id).copied());
            let rows = observed_entries(
                input,
                catalog.get(package_id).into_iter().flatten().copied(),
                source,
                &installed,
            );
            let latest = choose(
                &rows,
                false,
                false,
                source.is_some(),
                input.settings.show_prerelease,
            );
            let latest_stable = choose(
                &rows,
                true,
                false,
                source.is_some(),
                input.settings.show_prerelease,
            );
            let project_latest = choose(
                &rows,
                false,
                true,
                source.is_some(),
                input.settings.show_prerelease,
            );
            let project_latest_stable = choose(
                &rows,
                true,
                true,
                source.is_some(),
                input.settings.show_prerelease,
            );
            let update = update_evidence(&project_latest, &installed);
            let stable_update = update_evidence(&project_latest_stable, &installed);
            let (mut display_name, mut links) = summary_presentation(
                &rows,
                locked.get(package_id).copied(),
                &project_latest,
                &latest,
            );
            let local = presentations.get(package_id).copied().filter(|item| {
                locked
                    .get(package_id)
                    .is_some_and(|version| *version == item.version)
            });
            let preferred = locked
                .get(package_id)
                .and_then(|version| rows.iter().find(|row| row.entry.version == *version))
                .or_else(|| {
                    [&project_latest, &latest]
                        .into_iter()
                        .find_map(|choice| match choice {
                            CandidateChoice::Candidate { candidate } => rows.iter().find(|row| {
                                row.entry.source == candidate.source
                                    && row.entry.version == candidate.version
                            }),
                            _ => None,
                        })
                });
            let mut description = preferred.and_then(|row| row.entry.description.clone());
            let mut search_terms = Vec::new();
            for row in &rows {
                if let Some(metadata) = &row.entry.presentation {
                    search_terms.extend(metadata.aliases.iter().chain(&metadata.keywords).cloned());
                }
            }
            let mut installed_yanked = locked.get(package_id).and_then(|version| {
                let exact: Vec<_> = rows
                    .iter()
                    .filter(|row| row.entry.version == *version)
                    .collect();
                if exact.is_empty() {
                    None
                } else if exact.iter().any(|row| yanked(row.entry)) {
                    Some(true)
                } else if exact.iter().all(|row| row.entry.presentation.is_some()) {
                    Some(false)
                } else {
                    None
                }
            });
            let presentation_state = if let Some(local) = local {
                display_name = local.display_name.clone().or(display_name);
                description = local.description.clone().or(description);
                links = (local.links.documentation.is_some() || local.links.changelog.is_some())
                    .then(|| local.links.clone());
                installed_yanked = Some(local.yanked || installed_yanked.unwrap_or(false));
                search_terms.extend(
                    local
                        .metadata
                        .aliases
                        .iter()
                        .chain(&local.metadata.keywords)
                        .cloned(),
                );
                alcomd_application::PresentationState::Complete
            } else if locked.contains_key(package_id) {
                if input.installed_presentations.state
                    == alcomd_application::PresentationState::Unobserved
                {
                    alcomd_application::PresentationState::Unobserved
                } else {
                    alcomd_application::PresentationState::Partial
                }
            } else if preferred.is_some_and(|row| row.entry.presentation.is_some()) {
                alcomd_application::PresentationState::Complete
            } else {
                alcomd_application::PresentationState::Unobserved
            };
            let mut providers: Vec<_> = providers.into_values().collect();
            providers
                .sort_by_key(|provider| (provider.display_order, source_key(&provider.source)));
            CandidateSummary {
                description,
                installed_yanked,
                presentation_state,
                search_terms,
                package_id: package_id.to_owned(),
                display_name,
                links,
                providers,
                direct: direct.contains(package_id),
                installed,
                latest,
                latest_stable,
                project_latest,
                project_latest_stable,
                update,
                stable_update,
            }
        })
        .collect();
    if package_ids.is_none() {
        summaries.retain(|item| {
            !matches!(item.installed, CandidateInstalled::Absent)
                || direct.contains(item.package_id.as_str())
                || catalog
                    .get(item.package_id.as_str())
                    .is_some_and(|entries| entries.iter().any(|entry| !yanked(entry)))
        });
    }
    prune_display_rows(input, &catalog, &mut summaries);
    summaries
}

fn yanked(entry: &CandidateCatalogEntry) -> bool {
    entry.yanked
        || entry
            .presentation
            .as_ref()
            .is_some_and(|metadata| metadata.yanked)
}

fn prune_display_rows(
    input: &CandidateEvidenceInput,
    catalog: &BTreeMap<&str, Vec<&CandidateCatalogEntry>>,
    rows: &mut Vec<CandidateSummary>,
) {
    let installed: BTreeMap<_, _> = input
        .project
        .locked_dependencies
        .iter()
        .map(|item| (item.package_id.as_str(), item.value.as_str()))
        .collect();
    let mut excluded = BTreeSet::<String>::new();
    const AVATARS: &str = "com.vrchat.avatars";
    const WORLDS: &str = "com.vrchat.worlds";
    match (
        installed.contains_key(AVATARS),
        installed.contains_key(WORLDS),
    ) {
        (true, false) => {
            excluded.insert(WORLDS.into());
        }
        (false, true) => {
            excluded.insert(AVATARS.into());
        }
        _ => (),
    }
    if !excluded.is_empty() {
        let mut dependents: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for row in rows.iter() {
            let CandidateChoice::Candidate { candidate } = &row.project_latest else {
                continue;
            };
            if let Some(entry) = catalog
                .get(row.package_id.as_str())
                .into_iter()
                .flatten()
                .find(|entry| {
                    entry.source == candidate.source && entry.version == candidate.version
                })
            {
                for dependency in entry.dependencies.keys() {
                    dependents
                        .entry(dependency)
                        .or_default()
                        .push(&row.package_id);
                }
            }
        }
        let mut pending: Vec<_> = excluded.iter().cloned().collect();
        while let Some(id) = pending.pop() {
            for dependent in dependents.get(id.as_str()).into_iter().flatten() {
                if excluded.insert((*dependent).to_owned()) {
                    pending.push((*dependent).to_owned());
                }
            }
        }
    }
    for local in &input.installed_presentations.packages {
        if installed
            .get(local.package_id.as_str())
            .is_some_and(|version| *version == local.version)
        {
            excluded.extend(local.metadata.legacy_packages.iter().cloned());
        }
    }
    rows.retain(|row| {
        installed.contains_key(row.package_id.as_str()) || !excluded.contains(&row.package_id)
    });
}

fn summary_presentation(
    rows: &[Observed<'_>],
    locked_version: Option<&str>,
    project_latest: &CandidateChoice,
    latest: &CandidateChoice,
) -> (Option<String>, Option<CandidateLinks>) {
    let installed_entry = locked_version.and_then(|version| {
        rows.iter()
            .filter(|row| row.entry.version == version)
            .min_by(|left, right| display_order(left, right))
    });
    let latest_entry = [project_latest, latest].into_iter().find_map(|choice| {
        let CandidateChoice::Candidate { candidate } = choice else {
            return None;
        };
        rows.iter().find(|row| {
            row.entry.source == candidate.source && row.entry.version == candidate.version
        })
    });
    let display_name = installed_entry
        .or(latest_entry)
        .and_then(|row| row.entry.display_name.clone())
        .or_else(|| {
            rows.iter()
                .filter(|row| row.entry.display_name.is_some())
                .min_by(|left, right| display_order(left, right))
                .and_then(|row| row.entry.display_name.clone())
        });
    let link_entry = if locked_version.is_some() {
        installed_entry.filter(|preferred| {
            rows.iter()
                .filter(|row| {
                    row.entry.version == preferred.entry.version
                        && priority(row) == priority(preferred)
                })
                .take(2)
                .count()
                == 1
        })
    } else {
        latest_entry
    };
    let links = link_entry.and_then(|row| {
        let links = &row.entry.links;
        let links = CandidateLinks {
            documentation: links
                .documentation
                .as_deref()
                .and_then(crate::normalize_package_link),
            changelog: links
                .changelog
                .as_deref()
                .and_then(crate::normalize_package_link),
        };
        (links.documentation.is_some() || links.changelog.is_some()).then_some(links)
    });
    (display_name, links)
}

/// Source/build ambiguity is determined on the complete set, never on one page.
pub fn evaluate_candidate_versions(
    input: &CandidateEvidenceInput,
    package_id: &str,
    source: Option<&PackageSourceSelector>,
) -> Vec<CandidateEvidence> {
    let installed = installed(input, package_id);
    let mut rows = observed(input, package_id, source, &installed);
    let mut counts = BTreeMap::new();
    for row in &rows {
        if resolver_direct_match(row)
            && let Some(key) = exact_group(row)
        {
            *counts.entry(key).or_insert(0_usize) += 1;
        }
    }
    for row in &mut rows {
        if row.evidence.eligibility == CandidateEligibility::Eligible
            && exact_group(row).is_some_and(|key| counts.get(&key).is_some_and(|count| *count > 1))
        {
            row.evidence.eligibility = CandidateEligibility::Blocked;
            row.evidence.reasons.push(CandidateReason::SourceAmbiguous);
        }
    }
    rows.sort_by(display_order);
    rows.into_iter()
        .filter(|row| !yanked(row.entry))
        .map(|row| row.evidence)
        .collect()
}

fn visible(input: &CandidateEvidenceInput, source: &PackageSourceSelector) -> bool {
    match source {
        PackageSourceSelector::Repository { repository_id } => {
            !input.settings.hidden_repository_ids.contains(repository_id)
        }
        PackageSourceSelector::UserPackage { .. } => !input.settings.hide_local_user_packages,
    }
}

fn entry_visible(input: &CandidateEvidenceInput, entry: &CandidateCatalogEntry) -> bool {
    visible(input, &entry.source)
        && (input.settings.show_prerelease
            || !Version::parse(&entry.version).is_ok_and(|version| !version.pre.is_empty()))
}

fn installed(input: &CandidateEvidenceInput, package_id: &str) -> CandidateInstalled {
    installed_from_lock(
        input
            .project
            .locked_dependencies
            .iter()
            .find(|item| item.package_id == package_id)
            .map(|item| item.value.as_str()),
    )
}

fn installed_from_lock(version: Option<&str>) -> CandidateInstalled {
    match version {
        None => CandidateInstalled::Absent,
        Some(version) if Version::parse(version).is_ok() => CandidateInstalled::Locked {
            version: version.to_owned(),
        },
        Some(_) => CandidateInstalled::Unknown {
            reason: CandidateReason::InstalledEvidenceUnknown,
        },
    }
}

fn observed<'a>(
    input: &'a CandidateEvidenceInput,
    package_id: &str,
    source: Option<&PackageSourceSelector>,
    installed: &CandidateInstalled,
) -> Vec<Observed<'a>> {
    observed_entries(
        input,
        input
            .entries
            .iter()
            .filter(|entry| entry.package_id == package_id && entry_visible(input, entry)),
        source,
        installed,
    )
}

fn observed_entries<'a>(
    input: &CandidateEvidenceInput,
    entries: impl Iterator<Item = &'a CandidateCatalogEntry>,
    source: Option<&PackageSourceSelector>,
    installed: &CandidateInstalled,
) -> Vec<Observed<'a>> {
    let project_unity = UnityVersion::parse_editor(&input.project.unity_version);
    entries
        .filter(|entry| source.is_none_or(|source| source == &entry.source))
        .map(|entry| {
            let version = Version::parse(&entry.version).ok();
            let classification = match &version {
                None => CandidateClassification::Unknown,
                Some(version) if version.pre.is_empty() => CandidateClassification::Stable,
                Some(_) => CandidateClassification::Prerelease,
            };
            let relation = match (&version, installed) {
                (Some(_), CandidateInstalled::Absent) => CandidateRelation::NotInstalled,
                (Some(version), CandidateInstalled::Locked { version: locked }) => {
                    match Version::parse(locked).map(|locked| compare_precedence(version, &locked))
                    {
                        Ok(Ordering::Greater) => CandidateRelation::Newer,
                        Ok(Ordering::Equal) => CandidateRelation::SamePrecedence,
                        Ok(Ordering::Less) => CandidateRelation::Older,
                        Err(_) => CandidateRelation::Unknown,
                    }
                }
                _ => CandidateRelation::Unknown,
            };
            let minimum = package_minimum(entry.unity.as_deref(), entry.unity_release.as_deref());
            let unity = match minimum {
                Ok(None) => CandidateUnity::NotRequired,
                Ok(Some(_)) if project_unity.is_none() => CandidateUnity::Unknown,
                Ok(minimum) if meets_minimum(project_unity, minimum) => CandidateUnity::Compatible,
                Ok(_) => CandidateUnity::Incompatible,
                Err(()) => CandidateUnity::Unknown,
            };
            let mut reasons = BTreeSet::new();
            if yanked(entry) {
                reasons.insert(CandidateReason::AllYanked);
            }
            if classification == CandidateClassification::Prerelease
                && !input.settings.show_prerelease
            {
                reasons.insert(CandidateReason::PrereleaseExcluded);
            }
            if unity == CandidateUnity::Incompatible {
                reasons.insert(CandidateReason::UnityIncompatible);
            }
            if unity == CandidateUnity::Unknown {
                reasons.insert(if minimum.is_err() {
                    CandidateReason::MetadataInvalid
                } else {
                    CandidateReason::ProjectUnityUnknown
                });
            }
            if !entry.metadata_ready {
                reasons.insert(CandidateReason::CatalogIncomplete);
            }
            if classification == CandidateClassification::Unknown {
                reasons.insert(CandidateReason::ClassificationUnknown);
                reasons.insert(CandidateReason::MetadataInvalid);
            }
            let eligibility = if reasons.is_empty() {
                CandidateEligibility::Eligible
            } else if !entry.metadata_ready
                || classification == CandidateClassification::Unknown
                || unity == CandidateUnity::Unknown
            {
                CandidateEligibility::Unknown
            } else {
                CandidateEligibility::Blocked
            };
            Observed {
                entry,
                version,
                evidence: CandidateEvidence {
                    version: entry.version.clone(),
                    source: entry.source.clone(),
                    source_revision: entry.source_revision,
                    classification,
                    relation,
                    unity,
                    eligibility,
                    reasons: reasons.into_iter().collect(),
                },
            }
        })
        .collect()
}

fn same_precedence(left: &Observed<'_>, right: &Observed<'_>) -> bool {
    match (&left.version, &right.version) {
        (Some(left), Some(right)) => compare_precedence(left, right) == Ordering::Equal,
        _ => false,
    }
}

fn exact_group(row: &Observed<'_>) -> Option<(String, Version)> {
    row.version.clone().map(|mut version| {
        version.build = BuildMetadata::EMPTY;
        (source_key(&row.entry.source), version)
    })
}

// Legacy metadata requires a target-specific read-only proof in Plan; its mere presence does not block a source.
// Include it when checking whether an exact precedence/source intent is expressible.
fn resolver_direct_match(row: &Observed<'_>) -> bool {
    row.entry.metadata_ready
        && row.version.is_some()
        && !yanked(row.entry)
        && matches!(
            row.evidence.unity,
            CandidateUnity::Compatible | CandidateUnity::NotRequired
        )
        && !row
            .evidence
            .reasons
            .contains(&CandidateReason::PrereleaseExcluded)
}

fn priority(row: &Observed<'_>) -> u64 {
    match row.entry.source {
        PackageSourceSelector::Repository { .. } => row.entry.priority,
        PackageSourceSelector::UserPackage { .. } => u64::MAX,
    }
}

fn source_key(source: &PackageSourceSelector) -> String {
    match source {
        PackageSourceSelector::Repository { repository_id } => {
            format!("repository:{repository_id}")
        }
        PackageSourceSelector::UserPackage { user_package_id } => {
            format!("user_package:{user_package_id}")
        }
    }
}

fn display_order(left: &Observed<'_>, right: &Observed<'_>) -> Ordering {
    match (&left.version, &right.version) {
        (Some(left), Some(right)) => compare_precedence(right, left),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
    .then_with(|| {
        if left.version.is_some() {
            priority(left).cmp(&priority(right))
        } else {
            Ordering::Equal
        }
    })
    .then_with(|| source_key(&left.entry.source).cmp(&source_key(&right.entry.source)))
    .then_with(|| {
        left.entry
            .version
            .as_bytes()
            .cmp(right.entry.version.as_bytes())
    })
}

fn choose(
    rows: &[Observed<'_>],
    stable: bool,
    project: bool,
    explicit: bool,
    show_prerelease: bool,
) -> CandidateChoice {
    if rows.is_empty() {
        return CandidateChoice::None {
            reasons: vec![if explicit {
                CandidateReason::SourceUnavailable
            } else {
                CandidateReason::NoVisibleCandidate
            }],
        };
    }
    let mut known = Vec::new();
    let mut uncertain = Vec::new();
    let mut blocked_reasons = BTreeSet::new();
    for row in rows {
        let mut reasons = BTreeSet::new();
        if yanked(row.entry) {
            reasons.insert(CandidateReason::AllYanked);
        }
        if row.evidence.classification == CandidateClassification::Prerelease
            && (stable || !show_prerelease)
        {
            reasons.insert(if stable {
                CandidateReason::StableUnavailable
            } else {
                CandidateReason::PrereleaseExcluded
            });
        }
        if project && row.evidence.unity == CandidateUnity::Incompatible {
            reasons.insert(CandidateReason::UnityIncompatible);
        }
        if !reasons.is_empty() {
            blocked_reasons.extend(reasons);
            continue;
        }
        if !row.entry.metadata_ready
            || row.version.is_none()
            || row
                .evidence
                .reasons
                .contains(&CandidateReason::MetadataInvalid)
            || (project && row.evidence.unity == CandidateUnity::Unknown)
        {
            uncertain.push(row);
        } else {
            known.push(row);
        }
    }
    known.sort_by(|left, right| display_order(left, right));
    let first = known.first().copied();
    let relevant_unknown: Vec<_> = uncertain
        .into_iter()
        .filter(|row| {
            first.is_none_or(|first| match (&row.version, &first.version) {
                (Some(version), Some(best)) => compare_precedence(version, best) != Ordering::Less,
                _ => true,
            })
        })
        .collect();
    if !relevant_unknown.is_empty() {
        let reasons: BTreeSet<_> = relevant_unknown
            .into_iter()
            .flat_map(|row| row.evidence.reasons.iter().copied())
            .collect();
        return CandidateChoice::Unknown {
            reasons: reasons.into_iter().collect(),
        };
    }
    let Some(first) = first else {
        if blocked_reasons.is_empty() {
            blocked_reasons.insert(if stable {
                CandidateReason::StableUnavailable
            } else {
                CandidateReason::NoVisibleCandidate
            });
        }
        return CandidateChoice::None {
            reasons: blocked_reasons.into_iter().collect(),
        };
    };
    let group: Vec<_> = known
        .into_iter()
        .take_while(|row| same_precedence(row, first))
        .collect();
    let has_repository = group
        .iter()
        .any(|row| matches!(row.entry.source, PackageSourceSelector::Repository { .. }));
    let has_user = group
        .iter()
        .any(|row| matches!(row.entry.source, PackageSourceSelector::UserPackage { .. }));
    let best_priority = group
        .iter()
        .map(|row| priority(row))
        .min()
        .expect("nonempty precedence group");
    let preferred: Vec<_> = group
        .into_iter()
        .filter(|row| priority(row) == best_priority)
        .collect();
    if source_group_is_ambiguous(explicit, has_repository, has_user, preferred.len()) {
        return CandidateChoice::Ambiguous {
            reason: CandidateReason::SourceAmbiguous,
        };
    }
    let selected = preferred[0];
    if rows
        .iter()
        .filter(|row| {
            resolver_direct_match(row)
                && row.entry.source == selected.entry.source
                && same_precedence(row, selected)
        })
        .count()
        > 1
    {
        return CandidateChoice::Ambiguous {
            reason: CandidateReason::SourceAmbiguous,
        };
    }
    CandidateChoice::Candidate {
        candidate: selected.evidence.clone(),
    }
}

fn update_evidence(choice: &CandidateChoice, installed: &CandidateInstalled) -> CandidateUpdate {
    match installed {
        CandidateInstalled::Absent => {
            return CandidateUpdate::NoUpdate {
                reason: CandidateNoUpdateReason::NotInstalled,
            };
        }
        CandidateInstalled::Unknown { .. } => {
            return CandidateUpdate::Unavailable {
                reasons: vec![CandidateReason::InstalledEvidenceUnknown],
            };
        }
        CandidateInstalled::Locked { .. } => {}
    }
    match choice {
        CandidateChoice::Candidate { candidate } => match candidate.relation {
            CandidateRelation::Newer if candidate.eligibility == CandidateEligibility::Eligible => {
                CandidateUpdate::Target {
                    candidate: candidate.clone(),
                }
            }
            CandidateRelation::SamePrecedence => CandidateUpdate::NoUpdate {
                reason: CandidateNoUpdateReason::SamePrecedence,
            },
            CandidateRelation::Older => CandidateUpdate::NoUpdate {
                reason: CandidateNoUpdateReason::InstalledNewer,
            },
            _ => CandidateUpdate::Unavailable {
                reasons: vec![CandidateReason::InstalledEvidenceUnknown],
            },
        },
        CandidateChoice::None { reasons } | CandidateChoice::Unknown { reasons } => {
            CandidateUpdate::Unavailable {
                reasons: reasons.clone(),
            }
        }
        CandidateChoice::Ambiguous { reason } => CandidateUpdate::Unavailable {
            reasons: vec![*reason],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alcomd_application::{
        CandidateEvidenceEngine, ConfigPackageSettings, DependencyIdentity, ManifestState,
        ProjectObservation, ProjectType, UserPackageId,
    };

    const PACKAGE: &str = "com.example.package";

    fn entry(version: &str) -> CandidateCatalogEntry {
        CandidateCatalogEntry {
            description: None,
            presentation: None,
            dependencies: Default::default(),
            display_name: None,
            links: CandidateLinks::default(),
            package_id: PACKAGE.to_owned(),
            version: version.to_owned(),
            source: PackageSourceSelector::Repository {
                repository_id: "repository-a".to_owned(),
            },
            source_revision: 7,
            priority: 1,
            yanked: false,
            unity: None,
            unity_release: None,
            metadata_ready: true,
            legacy_metadata_present: false,
        }
    }

    fn input(versions: &[&str], installed: Option<&str>) -> CandidateEvidenceInput {
        CandidateEvidenceInput {
            installed_presentations: Default::default(),
            source_metadata: [
                "repository:repository-a",
                "repository:repository-b",
                "user_package:aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
            ]
            .into_iter()
            .map(|key| {
                (
                    key.into(),
                    alcomd_application::CandidateSourceMetadata {
                        display_name: None,
                        name: Some(key.into()),
                        builtin: None,
                        display_order: 3,
                        kind: if key.starts_with("user_package:") {
                            alcomd_application::CandidateFilterSource::UserPackage
                        } else {
                            alcomd_application::CandidateFilterSource::Remote
                        },
                        search_name: key.into(),
                    },
                )
            })
            .collect(),
            project: ProjectObservation {
                package_presentations: Default::default(),
                root_path: String::new(),
                path_identity_key: vec![],
                project_type: ProjectType::VpmStarter,
                unity_version: "2022.3.22f1".to_owned(),
                unity_revision: None,
                vpm_manifest: ManifestState::Valid,
                upm_manifest: ManifestState::Valid,
                direct_dependencies: vec![DependencyIdentity {
                    package_id: PACKAGE.to_owned(),
                    value: "*".to_owned(),
                }],
                locked_dependencies: installed
                    .into_iter()
                    .map(|version| DependencyIdentity {
                        package_id: PACKAGE.to_owned(),
                        value: version.to_owned(),
                    })
                    .collect(),
                issues: vec![],
            },
            entries: versions.iter().map(|version| entry(version)).collect(),
            settings: ConfigPackageSettings {
                show_prerelease: false,
                hidden_repository_ids: vec![],
                hide_local_user_packages: false,
            },
        }
    }

    fn summary(input: &CandidateEvidenceInput) -> CandidateSummary {
        evaluate_candidate_summary(input, Some(&[PACKAGE.to_owned()]), &[]).remove(0)
    }

    #[test]
    fn presentation_uses_exact_locked_version_and_respects_hidden_sources() {
        let mut input = input(&["2.0.0", "1.0.0"], Some("1.0.0"));
        input.entries[0].source = PackageSourceSelector::Repository {
            repository_id: "repository-b".to_owned(),
        };
        input.entries[0].display_name = Some("New name".to_owned());
        input.entries[0].links.documentation = Some("https://example.invalid/new".to_owned());
        input.entries[1].display_name = Some("Installed name".to_owned());
        input.entries[1].links.documentation = Some("https://example.invalid/installed".to_owned());
        input.entries[1].links.changelog = Some("https://user@example.invalid/private".to_owned());

        let installed = summary(&input);
        assert_eq!(installed.display_name.as_deref(), Some("Installed name"));
        let links = installed.links.expect("validated installed links");
        assert_eq!(
            links.documentation.as_deref(),
            Some("https://example.invalid/installed")
        );
        assert!(links.changelog.is_none());

        input
            .settings
            .hidden_repository_ids
            .push("repository-a".to_owned());
        let hidden = summary(&input);
        assert_eq!(hidden.display_name.as_deref(), Some("New name"));
        assert!(hidden.links.is_none());
    }

    fn candidate(choice: &CandidateChoice) -> &CandidateEvidence {
        match choice {
            CandidateChoice::Candidate { candidate } => candidate,
            value => panic!("expected candidate, got {value:?}"),
        }
    }

    #[test]
    fn semver_order_and_upgrade_direction_are_core_authoritative() {
        let input = input(&["1.9.0", "1.10.0", "1.2.0"], Some("1.9.0"));
        let summary = summary(&input);
        assert_eq!(candidate(&summary.latest).version, "1.10.0");
        assert!(matches!(summary.update, CandidateUpdate::Target { .. }));
        assert_eq!(
            evaluate_candidate_versions(&input, PACKAGE, None)
                .iter()
                .map(|entry| entry.version.as_str())
                .collect::<Vec<_>>(),
            ["1.10.0", "1.9.0", "1.2.0"]
        );
    }

    #[test]
    fn build_only_is_same_precedence_not_update() {
        let summary = summary(&input(&["1.2.3+other"], Some("1.2.3+installed")));
        assert_eq!(
            candidate(&summary.latest).relation,
            CandidateRelation::SamePrecedence
        );
        assert_eq!(
            summary.update,
            CandidateUpdate::NoUpdate {
                reason: CandidateNoUpdateReason::SamePrecedence
            }
        );
    }

    #[test]
    fn same_source_build_ambiguity_is_computed_before_paging() {
        let mut input = input(&["2.0.0+a", "2.0.0+z"], Some("1.0.0"));
        for index in 1..130 {
            input.entries.push(entry(&format!("1.0.{index}")));
        }
        assert!(matches!(
            summary(&input).project_latest,
            CandidateChoice::Ambiguous { .. }
        ));
        let versions = evaluate_candidate_versions(&input, PACKAGE, None);
        for version in &versions[..2] {
            assert_eq!(version.eligibility, CandidateEligibility::Blocked);
            assert_eq!(version.reasons, [CandidateReason::SourceAmbiguous]);
        }
        assert_eq!(versions[0].version, "2.0.0+a");
    }

    #[test]
    fn legacy_build_alternative_still_prevents_exact_source_submission() {
        let mut input = input(&["2.0.0+a", "2.0.0+b"], Some("1.0.0"));
        input.entries[1].legacy_metadata_present = true;
        let versions = evaluate_candidate_versions(&input, PACKAGE, None);
        assert_eq!(versions[0].reasons, [CandidateReason::SourceAmbiguous]);
        assert_eq!(versions[1].reasons, [CandidateReason::SourceAmbiguous]);
        assert!(matches!(
            summary(&input).project_latest,
            CandidateChoice::Ambiguous { .. }
        ));
    }

    #[test]
    fn precedence_precedes_source_priority_and_equal_priority_is_ambiguous() {
        let mut input = input(&["1.0.0", "2.0.0"], Some("0.5.0"));
        input.entries[1].priority = 99;
        assert_eq!(candidate(&summary(&input).latest).version, "2.0.0");
        let mut second = entry("2.0.0");
        second.source = PackageSourceSelector::Repository {
            repository_id: "repository-b".to_owned(),
        };
        input.entries.push(second);
        assert_eq!(
            candidate(&summary(&input).latest).source,
            input.entries[2].source
        );
        input.entries[1].priority = 1;
        assert!(matches!(
            summary(&input).latest,
            CandidateChoice::Ambiguous { .. }
        ));
        let selected = CandidateSourceOverride {
            package_id: PACKAGE.to_owned(),
            source: input.entries[2].source.clone(),
        };
        let rows = evaluate_candidate_summary(&input, None, &[selected]);
        assert_eq!(candidate(&rows[0].latest).source, input.entries[2].source);
    }

    #[test]
    fn repository_user_tie_requires_source_selector() {
        let mut input = input(&["2.0.0", "2.0.0"], Some("1.0.0"));
        input.entries[1].source = PackageSourceSelector::UserPackage {
            user_package_id: UserPackageId::parse("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa")
                .expect("fixture user package ID is a valid UUID"),
        };
        assert!(matches!(
            summary(&input).latest,
            CandidateChoice::Ambiguous { .. }
        ));
        let rows = evaluate_candidate_summary(
            &input,
            None,
            &[CandidateSourceOverride {
                package_id: PACKAGE.to_owned(),
                source: input.entries[1].source.clone(),
            }],
        );
        assert_eq!(candidate(&rows[0].latest).source, input.entries[1].source);
        let versions = evaluate_candidate_versions(&input, PACKAGE, None);
        assert_eq!(versions.len(), 2);
        assert!(
            versions
                .iter()
                .all(|entry| entry.eligibility == CandidateEligibility::Eligible)
        );
        // Each individual option is submitted with its explicit source, unlike Latest.
    }

    #[test]
    fn stable_and_prerelease_policy_are_separate_sets() {
        let mut input = input(&["1.9.0", "2.0.0-beta.1"], Some("1.0.0"));
        input.settings.show_prerelease = true;
        let row = summary(&input);
        assert_eq!(candidate(&row.latest).version, "2.0.0-beta.1");
        assert_eq!(candidate(&row.latest_stable).version, "1.9.0");
        input.settings.show_prerelease = false;
        assert_eq!(candidate(&summary(&input).latest).version, "1.9.0");
        let versions = evaluate_candidate_versions(&input, PACKAGE, None);
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0].version, "1.9.0");
        assert!(versions[0].reasons.is_empty());
    }

    #[test]
    fn versions_omit_closed_prereleases_for_every_source_and_installed_version() {
        let mut input = input(&["2.0.0-beta.1", "1.9.0"], Some("2.0.0-beta.1"));
        let mut user_package = entry("3.0.0-beta.1");
        user_package.source = PackageSourceSelector::UserPackage {
            user_package_id: UserPackageId::parse("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa")
                .expect("fixture user package ID"),
        };
        input.entries.push(user_package);
        for source in [
            None,
            Some(&input.entries[0].source),
            Some(&input.entries[2].source),
        ] {
            let versions = evaluate_candidate_versions(&input, PACKAGE, source);
            assert!(
                versions
                    .iter()
                    .all(|row| row.classification != CandidateClassification::Prerelease)
            );
        }
        input.settings.show_prerelease = true;
        let versions = evaluate_candidate_versions(&input, PACKAGE, None);
        assert_eq!(versions.len(), 3);
        assert_eq!(versions[0].version, "3.0.0-beta.1");
        assert_eq!(versions[1].version, "2.0.0-beta.1");
    }

    #[test]
    fn closed_versions_never_introduce_packages_or_providers() {
        let mut input = input(&["1.9.0"], None);
        input.project.direct_dependencies.clear();
        let mut prerelease_provider = entry("2.0.0-beta.1");
        prerelease_provider.source = PackageSourceSelector::Repository {
            repository_id: "repository-b".to_owned(),
        };
        let mut prerelease_only = prerelease_provider.clone();
        prerelease_only.package_id = "com.example.prerelease-only".to_owned();
        input.entries.extend([prerelease_provider, prerelease_only]);
        let rows = evaluate_candidate_summary(&input, None, &[]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].package_id, PACKAGE);
        assert_eq!(rows[0].providers.len(), 1);
        assert_eq!(rows[0].providers[0].source, input.entries[0].source);

        input.settings.show_prerelease = true;
        let rows = evaluate_candidate_summary(&input, None, &[]);
        assert_eq!(rows.len(), 2);
        let package = rows
            .iter()
            .find(|row| row.package_id == PACKAGE)
            .expect("visible package");
        assert_eq!(package.providers.len(), 2);
    }

    #[test]
    fn installed_package_with_closed_sources_matches_a_missing_catalog() {
        let mut hidden = input(&["1.0.0", "2.0.0"], Some("1.0.0"));
        hidden
            .settings
            .hidden_repository_ids
            .push("repository-a".to_owned());
        let mut missing = hidden.clone();
        missing.entries.clear();
        let hidden_rows = evaluate_candidate_summary(&hidden, None, &[]);
        let missing_rows = evaluate_candidate_summary(&missing, None, &[]);
        assert_eq!(hidden_rows, missing_rows);
        assert_eq!(hidden_rows.len(), 1);
        assert!(matches!(
            hidden_rows[0].installed,
            CandidateInstalled::Locked { .. }
        ));
        assert!(hidden_rows[0].providers.is_empty());
        assert!(hidden_rows[0].display_name.is_none());
        assert!(hidden_rows[0].links.is_none());
        assert!(evaluate_candidate_versions(&hidden, PACKAGE, None).is_empty());

        hidden.entries.push(entry("3.0.0"));
        hidden.entries[2].source = PackageSourceSelector::Repository {
            repository_id: "repository-b".to_owned(),
        };
        let visible = summary(&hidden);
        assert_eq!(visible.providers.len(), 1);
        assert_eq!(visible.providers[0].source, hidden.entries[2].source);
        assert_eq!(candidate(&visible.latest).version, "3.0.0");
    }

    #[test]
    fn stable_older_than_installed_prerelease_is_not_stable_update() {
        let mut input = input(&["1.9.0", "2.0.0-beta.2"], Some("2.0.0-beta.1"));
        input.settings.show_prerelease = true;
        let row = summary(&input);
        assert!(matches!(row.update, CandidateUpdate::Target { .. }));
        assert_eq!(
            candidate(&row.project_latest_stable).relation,
            CandidateRelation::Older
        );
        assert_eq!(
            row.stable_update,
            CandidateUpdate::NoUpdate {
                reason: CandidateNoUpdateReason::InstalledNewer
            }
        );
    }

    #[test]
    fn unknown_classification_never_becomes_stable() {
        let input = input(&["garbage", "1.0.0"], Some("0.5.0"));
        let row = summary(&input);
        assert!(matches!(row.latest, CandidateChoice::Unknown { .. }));
        assert!(matches!(row.latest_stable, CandidateChoice::Unknown { .. }));
        let versions = evaluate_candidate_versions(&input, PACKAGE, None);
        assert_eq!(versions[1].classification, CandidateClassification::Unknown);
        assert_eq!(versions[1].eligibility, CandidateEligibility::Unknown);
    }

    #[test]
    fn unity_incompatible_latest_does_not_displace_compatible_target() {
        let mut input = input(&["1.9.0", "2.0.0"], Some("1.0.0"));
        input.entries[1].unity = Some("2023.1".to_owned());
        let row = summary(&input);
        assert_eq!(candidate(&row.latest).version, "2.0.0");
        assert_eq!(candidate(&row.latest).unity, CandidateUnity::Incompatible);
        assert_eq!(candidate(&row.project_latest).version, "1.9.0");
        assert!(matches!(row.update, CandidateUpdate::Target { .. }));
    }

    #[test]
    fn unknown_unity_blocks_only_potential_winners_with_minimum() {
        let mut input = input(&["1.9.0", "2.0.0"], Some("1.0.0"));
        input.project.unity_version = "unknown".to_owned();
        input.entries[1].unity = Some("2022.3".to_owned());
        assert!(matches!(
            summary(&input).project_latest,
            CandidateChoice::Unknown { .. }
        ));
        assert_eq!(candidate(&summary(&input).latest).version, "2.0.0");
        input.entries[0].version = "3.0.0".to_owned();
        let row = summary(&input);
        assert_eq!(candidate(&row.project_latest).version, "3.0.0");
        assert_eq!(
            candidate(&row.project_latest).unity,
            CandidateUnity::NotRequired
        );
    }

    #[test]
    fn malformed_unity_is_unknown_metadata_not_compatible() {
        let mut input = input(&["2.0.0"], Some("1.0.0"));
        input.entries[0].unity = Some("2022.3.4".to_owned());
        assert!(matches!(
            summary(&input).latest,
            CandidateChoice::Unknown { .. }
        ));
        let rows = evaluate_candidate_versions(&input, PACKAGE, None);
        assert_eq!(rows[0].reasons, [CandidateReason::MetadataInvalid]);
    }

    #[test]
    fn package_release_candidates_and_resolver_use_the_same_minimum() {
        use crate::{
            PackageCandidate, PackageSource, PackageSourceAuthority, ResolveRequest,
            resolve_packages,
        };
        let mut input = input(&["2.0.0"], Some("1.0.0"));
        input.entries[0].unity = Some("2022.3".to_owned());
        input.entries[0].unity_release = Some("22f1".to_owned());
        let package = PackageCandidate {
            package_id: PACKAGE.to_owned(),
            version: Version::new(2, 0, 0),
            yanked: false,
            unity_minimum: package_minimum(Some("2022.3"), Some("22f1")).expect("minimum"),
            legacy_metadata_present: false,
            dependencies: Vec::new(),
            source: PackageSource {
                authority: PackageSourceAuthority::Repository {
                    credential_reference: None,
                    download_headers_present: false,
                    repository_locator: None,
                    repository_id: "repository-a".to_owned(),
                    repository_revision: 1,
                    priority: 1,
                    artifact_url: "https://example.invalid/package.zip".to_owned(),
                },
                source_identity: "repository-a".to_owned(),
                manifest_fingerprint: [1; 32],
                archive_sha256: Some([2; 32]),
            },
        };
        for (actual, compatible) in [
            ("2022.3.21f10", false),
            ("2022.3.22b10", false),
            ("2022.3.22f1", true),
            ("2022.3.22f10", true),
            ("2022.3.22p1", true),
            ("2022.3.100f1", true),
        ] {
            input.project.unity_version = actual.to_owned();
            let rows = evaluate_candidate_versions(&input, PACKAGE, None);
            assert_eq!(
                rows[0].unity == CandidateUnity::Compatible,
                compatible,
                "candidate {actual}"
            );
            let request = ResolveRequest {
                package_id: PACKAGE.to_owned(),
                range: "*".to_owned(),
                source: None,
                include_prerelease: false,
                unity_version: UnityVersion::parse_editor(actual),
            };
            assert_eq!(
                resolve_packages(std::slice::from_ref(&package), &[request]).is_ok(),
                compatible,
                "resolver {actual}"
            );
        }
        input.entries[0].unity_release = Some("22z1".to_owned());
        let rows = evaluate_candidate_versions(&input, PACKAGE, None);
        assert_eq!(rows[0].unity, CandidateUnity::Unknown);
        assert!(rows[0].reasons.contains(&CandidateReason::MetadataInvalid));
        input.entries[0].unity = None;
        assert_eq!(
            evaluate_candidate_versions(&input, PACKAGE, None)[0].unity,
            CandidateUnity::NotRequired
        );
    }

    #[test]
    fn yanked_is_blocked_but_legacy_metadata_requires_plan_target_proof() {
        let mut input = input(&["2.0.0"], Some("1.0.0"));
        input.entries[0].yanked = true;
        assert_eq!(
            summary(&input).latest,
            CandidateChoice::None {
                reasons: vec![CandidateReason::AllYanked]
            }
        );
        assert!(evaluate_candidate_versions(&input, PACKAGE, None).is_empty());
        input.entries[0].yanked = false;
        input.entries[0].legacy_metadata_present = true;
        assert_eq!(candidate(&summary(&input).latest).version, "2.0.0");
        assert_eq!(
            evaluate_candidate_versions(&input, PACKAGE, None)[0].eligibility,
            CandidateEligibility::Eligible
        );
    }

    #[test]
    fn installed_unknown_absent_and_newer_are_not_upgrades() {
        let row = summary(&input(&["1.9.0"], Some("invalid")));
        assert_eq!(candidate(&row.latest).relation, CandidateRelation::Unknown);
        assert!(matches!(row.update, CandidateUpdate::Unavailable { .. }));
        let row = summary(&input(&["1.9.0"], None));
        assert!(row.direct);
        assert_eq!(row.installed, CandidateInstalled::Absent);
        assert_eq!(
            candidate(&row.latest).relation,
            CandidateRelation::NotInstalled
        );
        assert_eq!(
            row.update,
            CandidateUpdate::NoUpdate {
                reason: CandidateNoUpdateReason::NotInstalled
            }
        );
        let row = summary(&input(&["1.9.0"], Some("2.0.0")));
        assert_eq!(
            row.update,
            CandidateUpdate::NoUpdate {
                reason: CandidateNoUpdateReason::InstalledNewer
            }
        );
    }

    #[test]
    fn hidden_sources_and_stale_explicit_source_never_fall_back() {
        let mut input = input(&["2.0.0"], Some("1.0.0"));
        input.settings.hidden_repository_ids = vec!["repository-a".to_owned()];
        assert_eq!(
            summary(&input).latest,
            CandidateChoice::None {
                reasons: vec![CandidateReason::NoVisibleCandidate]
            }
        );
        let rows = evaluate_candidate_summary(
            &input,
            None,
            &[CandidateSourceOverride {
                package_id: PACKAGE.to_owned(),
                source: input.entries[0].source.clone(),
            }],
        );
        assert_eq!(
            rows[0].latest,
            CandidateChoice::None {
                reasons: vec![CandidateReason::SourceUnavailable]
            }
        );
        assert!(evaluate_candidate_versions(&input, PACKAGE, None).is_empty());
    }

    #[test]
    fn relevant_incomplete_is_unknown_but_unrelated_does_not_erase_evidence() {
        let mut input = input(&["2.0.0"], Some("1.0.0"));
        input.entries[0].metadata_ready = false;
        assert_eq!(
            summary(&input).latest,
            CandidateChoice::Unknown {
                reasons: vec![CandidateReason::CatalogIncomplete]
            }
        );
        input.entries[0].package_id = "com.other.package".to_owned();
        input.entries.push(entry("2.0.0"));
        assert_eq!(candidate(&summary(&input).latest).version, "2.0.0");
    }

    #[test]
    fn summary_enumeration_is_union_and_byte_sorted() {
        let mut input = input(&["1.0.0"], None);
        input.project.direct_dependencies[0].package_id = "com.z.package".to_owned();
        input.project.locked_dependencies.push(DependencyIdentity {
            package_id: "com.a.package".to_owned(),
            value: "1.0.0".to_owned(),
        });
        let rows = evaluate_candidate_summary(&input, None, &[]);
        assert_eq!(
            rows.iter()
                .map(|row| row.package_id.as_str())
                .collect::<Vec<_>>(),
            ["com.a.package", PACKAGE, "com.z.package"]
        );
    }

    #[test]
    fn providers_preserve_nonwinners_ambiguity_and_unfiltered_source_switching() {
        let mut input = input(&["2.0.0", "1.0.0", "1.0.1"], Some("0.5.0"));
        let other = PackageSourceSelector::Repository {
            repository_id: "repository-b".to_owned(),
        };
        input.entries[1].source = other.clone();
        input.entries[2].source = other.clone();
        input.entries[1].source_revision = 11;
        input.entries[2].source_revision = 11;
        let row = summary(&input);
        assert_eq!(row.providers.len(), 2);
        assert_eq!(row.providers[0].source, input.entries[0].source);
        assert_eq!(
            row.providers[1],
            CandidateProvider {
                display_name: None,
                name: Some("repository:repository-b".into()),
                builtin: None,
                source_kind: alcomd_application::CandidateFilterSource::Remote,
                display_order: 3,
                source: other.clone(),
                source_revision: 11
            }
        );
        let pinned = evaluate_candidate_summary(
            &input,
            None,
            &[CandidateSourceOverride {
                package_id: PACKAGE.to_owned(),
                source: other.clone(),
            }],
        );
        assert_eq!(pinned[0].providers, row.providers);
        assert_eq!(candidate(&pinned[0].latest).version, "1.0.1");
        input.entries[1].version = "2.0.0".to_owned();
        let ambiguous = summary(&input);
        assert!(matches!(
            ambiguous.latest,
            CandidateChoice::Ambiguous { .. }
        ));
        assert_eq!(ambiguous.providers, row.providers);
        input
            .settings
            .hidden_repository_ids
            .push("repository-b".to_owned());
        let hidden = summary(&input);
        assert_eq!(hidden.providers.len(), 1);
        let absent =
            evaluate_candidate_summary(&input, Some(&["com.example.absent".to_owned()]), &[]);
        assert!(absent[0].providers.is_empty());
        let unavailable = evaluate_candidate_summary(
            &input,
            None,
            &[CandidateSourceOverride {
                package_id: PACKAGE.to_owned(),
                source: PackageSourceSelector::Repository {
                    repository_id: "not-registered".to_owned(),
                },
            }],
        );
        assert_eq!(unavailable[0].providers, hidden.providers);
        assert!(matches!(
            unavailable[0].latest,
            CandidateChoice::None { .. }
        ));
    }

    #[test]
    fn fingerprint_has_unambiguous_record_boundaries() {
        let engine = ProjectCandidateEngine;
        assert_ne!(
            engine.fingerprint(&["ab".to_owned(), "c".to_owned()]),
            engine.fingerprint(&["a".to_owned(), "bc".to_owned()])
        );
        assert_eq!(
            engine.fingerprint(&["same".to_owned()]),
            engine.fingerprint(&["same".to_owned()])
        );
    }

    #[test]
    fn eligible_candidate_does_not_prove_dependency_graph_resolution() {
        use crate::{
            PackageCandidate, PackageDependency, PackageSource, PackageSourceAuthority,
            ResolveRequest, resolve_packages,
        };
        let row = summary(&input(&["2.0.0"], Some("1.0.0")));
        assert!(matches!(row.update, CandidateUpdate::Target { .. }));
        let make = |id: &str, version: &str, dependency: Option<&str>| PackageCandidate {
            package_id: id.to_owned(),
            version: Version::parse(version).expect("fixture dependency version is valid SemVer"),
            yanked: false,
            unity_minimum: None,
            legacy_metadata_present: false,
            dependencies: dependency
                .into_iter()
                .map(|range| PackageDependency {
                    package_id: "com.example.zdependency".to_owned(),
                    range: range.to_owned(),
                })
                .collect(),
            source: PackageSource {
                authority: PackageSourceAuthority::Repository {
                    credential_reference: None,
                    download_headers_present: false,
                    repository_locator: None,
                    repository_id: "repository-a".to_owned(),
                    repository_revision: 1,
                    priority: 1,
                    artifact_url: "https://example.invalid/package.zip".to_owned(),
                },
                source_identity: "repository-a".to_owned(),
                manifest_fingerprint: [1; 32],
                archive_sha256: Some([2; 32]),
            },
        };
        let catalog = [
            make(PACKAGE, "2.0.0", Some("=2.0.0")),
            make("com.example.other", "1.0.0", Some("=1.0.0")),
            make("com.example.zdependency", "1.0.0", None),
            make("com.example.zdependency", "2.0.0", None),
        ];
        let requests = [PACKAGE, "com.example.other"].map(|id| ResolveRequest {
            package_id: id.to_owned(),
            range: "*".to_owned(),
            source: None,
            include_prerelease: false,
            unity_version: Some(UnityVersion::major_minor(2022, 3)),
        });
        let result = resolve_packages(&catalog, &requests);
        assert!(
            matches!(result, Err(crate::ResolveError::DependencyConflict { .. })),
            "{result:?}"
        );
    }
    fn local_presentation(id: &str) -> alcomd_application::InstalledPackagePresentation {
        alcomd_application::InstalledPackagePresentation {
            package_id: id.into(),
            version: "1.0.0".into(),
            display_name: Some("Installed local title".into()),
            description: Some("Installed local description".into()),
            metadata: alcomd_application::PackageDisplayMetadata {
                keywords: vec!["needle".into()],
                ..Default::default()
            },
            links: CandidateLinks {
                documentation: Some("https://example.invalid/installed".into()),
                changelog: None,
            },
            yanked: true,
            manifest_sha256: "a".repeat(64),
        }
    }

    #[test]
    fn installed_presentation_survives_closed_or_missing_sources_without_revealing_them() {
        let mut input = input(&["2.0.0"], Some("1.0.0"));
        input.installed_presentations = alcomd_application::ProjectPackagePresentations {
            state: alcomd_application::PresentationState::Complete,
            fingerprint: "a".repeat(64),
            packages: vec![local_presentation(PACKAGE)],
        };
        input
            .settings
            .hidden_repository_ids
            .push("repository-a".into());
        let closed = summary(&input);
        assert!(closed.providers.is_empty());
        assert_eq!(
            closed.display_name.as_deref(),
            Some("Installed local title")
        );
        assert_eq!(
            closed.description.as_deref(),
            Some("Installed local description")
        );
        assert_eq!(closed.installed_yanked, Some(true));
        assert_eq!(closed.search_terms, ["needle"]);
        assert_eq!(
            closed.presentation_state,
            alcomd_application::PresentationState::Complete
        );
        input.entries.clear();
        assert_eq!(closed, summary(&input));
        input.installed_presentations.packages.clear();
        let partial = summary(&input);
        assert_eq!(
            partial.presentation_state,
            alcomd_application::PresentationState::Partial
        );
        assert!(matches!(
            partial.installed,
            CandidateInstalled::Locked { .. }
        ));
        assert_eq!(partial.installed_yanked, None);
    }

    #[test]
    fn yanked_versions_are_not_selectable_and_provider_facts_still_exist() {
        let mut input = input(&["2.0.0", "1.0.0"], Some("1.0.0"));
        input.entries[0].presentation = Some(alcomd_application::PackageDisplayMetadata {
            yanked: true,
            ..Default::default()
        });
        assert_eq!(
            evaluate_candidate_versions(&input, PACKAGE, None)
                .iter()
                .map(|item| item.version.as_str())
                .collect::<Vec<_>>(),
            ["1.0.0"]
        );
        assert_eq!(candidate(&summary(&input).project_latest).version, "1.0.0");
        input.entries[1].yanked = true;
        assert!(evaluate_candidate_versions(&input, PACKAGE, None).is_empty());
        assert_eq!(summary(&input).installed_yanked, Some(true));
        assert_eq!(summary(&input).providers.len(), 1);
        input.project.locked_dependencies.clear();
        input.project.direct_dependencies.clear();
        assert!(evaluate_candidate_summary(&input, None, &[]).is_empty());
    }

    #[test]
    fn sdk_and_legacy_pruning_uses_known_compatible_dependencies_and_preserves_installed_conflicts()
    {
        let mut input = input(&[], None);
        input.project.direct_dependencies.clear();
        input.project.locked_dependencies = vec![alcomd_application::DependencyIdentity {
            package_id: "com.vrchat.avatars".into(),
            value: "1.0.0".into(),
        }];
        for id in [
            "com.vrchat.avatars",
            "com.vrchat.worlds",
            "com.example.world-tools",
            "com.example.world-tools-child",
            "com.example.unknown",
            "com.example.modern",
            "com.example.legacy",
        ] {
            let mut item = entry("1.0.0");
            item.package_id = id.into();
            input.entries.push(item);
        }
        input.entries[2]
            .dependencies
            .insert("com.vrchat.worlds".into(), "*".into());
        input.entries[3]
            .dependencies
            .insert("com.example.world-tools".into(), "*".into());
        input.entries[4]
            .dependencies
            .insert("com.vrchat.worlds".into(), "*".into());
        input.entries[4].metadata_ready = false;
        input
            .project
            .locked_dependencies
            .push(alcomd_application::DependencyIdentity {
                package_id: "com.example.modern".into(),
                value: "1.0.0".into(),
            });
        let mut modern = local_presentation("com.example.modern");
        modern
            .metadata
            .legacy_packages
            .push("com.example.legacy".into());
        input.installed_presentations.packages.push(modern);
        let ids = |input: &CandidateEvidenceInput| {
            evaluate_candidate_summary(input, None, &[])
                .into_iter()
                .map(|item| item.package_id)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            ids(&input),
            [
                "com.example.modern",
                "com.example.unknown",
                "com.vrchat.avatars"
            ]
        );
        input
            .project
            .locked_dependencies
            .push(alcomd_application::DependencyIdentity {
                package_id: "com.example.legacy".into(),
                value: "1.0.0".into(),
            });
        assert!(ids(&input).iter().any(|id| id == "com.example.legacy"));
        input
            .project
            .locked_dependencies
            .push(alcomd_application::DependencyIdentity {
                package_id: "com.vrchat.worlds".into(),
                value: "1.0.0".into(),
            });
        let conflict = ids(&input);
        assert!(conflict.iter().any(|id| id == "com.vrchat.worlds"));
        assert!(
            conflict
                .iter()
                .any(|id| id == "com.example.world-tools-child")
        );
    }
}
