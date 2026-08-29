use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use semver::Version;
use serde_json::Value;

use crate::range::{VpmRange, compare_precedence};
use alcomd_application::{PackageSourceSelector, ResolverCatalog, ResolverCatalogSource};

const MAX_REQUIREMENTS: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageSource {
    pub authority: PackageSourceAuthority,
    pub source_identity: String,
    pub manifest_fingerprint: [u8; 32],
    pub archive_sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PackageSourceAuthority {
    Repository {
        repository_id: String,
        repository_revision: u64,
        priority: u64,
        artifact_url: String,
    },
    UserPackage {
        user_package_id: alcomd_application::UserPackageId,
        source_revision: u64,
    },
}

impl PackageSource {
    fn priority(&self) -> u64 {
        match self.authority {
            PackageSourceAuthority::Repository { priority, .. } => priority,
            PackageSourceAuthority::UserPackage { .. } => u64::MAX,
        }
    }

    fn deterministic_key(&self) -> String {
        match &self.authority {
            PackageSourceAuthority::Repository { repository_id, .. } => {
                format!("repository:{repository_id}")
            }
            PackageSourceAuthority::UserPackage {
                user_package_id, ..
            } => format!("user-package:{user_package_id}"),
        }
    }

    fn matches_selector(&self, selector: &PackageSourceSelector) -> bool {
        match (&self.authority, selector) {
            (
                PackageSourceAuthority::Repository { repository_id, .. },
                PackageSourceSelector::Repository {
                    repository_id: selected,
                },
            ) => repository_id == selected,
            (
                PackageSourceAuthority::UserPackage {
                    user_package_id, ..
                },
                PackageSourceSelector::UserPackage {
                    user_package_id: selected,
                },
            ) => user_package_id == selected,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageDependency {
    pub package_id: String,
    pub range: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageCandidate {
    pub package_id: String,
    pub version: Version,
    pub yanked: bool,
    pub unity_minimum: Option<(u64, u64)>,
    pub legacy_metadata_present: bool,
    pub dependencies: Vec<PackageDependency>,
    pub source: PackageSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolveRequest {
    pub package_id: String,
    pub range: String,
    pub source: Option<PackageSourceSelector>,
    pub include_prerelease: bool,
    pub unity_version: Option<(u64, u64)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedPackage {
    pub package_id: String,
    pub version: Version,
    pub source: PackageSource,
    pub direct: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resolution {
    pub packages: Vec<ResolvedPackage>,
    pub dependency_edges: Vec<PackageDependencyEdge>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct PackageDependencyEdge {
    pub from_package_id: String,
    pub to_package_id: String,
    pub range: String,
    pub direct: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolveError {
    InvalidPackageId,
    InvalidVersion,
    InvalidRange,
    TooManyRequirements,
    PackageNotFound { package_id: String },
    DependencyMissing { package_id: String },
    DependencyConflict { package_id: String },
    UnityIncompatible { package_id: String },
    VersionYanked { package_id: String },
    SourceAmbiguous { package_id: String, version: String },
    LegacyCleanupRequired { package_id: String },
}

pub fn candidates_from_catalog(
    catalog: &ResolverCatalog,
) -> Result<Vec<PackageCandidate>, ResolveError> {
    let mut candidates = Vec::with_capacity(catalog.entries.len());
    for row in &catalog.entries {
        let version = Version::parse(&row.version).map_err(|_| ResolveError::InvalidVersion)?;
        let unity_minimum = row.unity.as_deref().map(parse_unity).transpose()?;
        let dependencies = serde_json::from_str::<Value>(&row.dependencies_json)
            .ok()
            .and_then(|value| value.as_object().cloned())
            .ok_or(ResolveError::InvalidRange)?
            .into_iter()
            .map(|(package_id, value)| {
                let range = value.as_str().ok_or(ResolveError::InvalidRange)?;
                validate_package_id(&package_id)?;
                VpmRange::parse(range).map_err(|_| ResolveError::InvalidRange)?;
                Ok(PackageDependency {
                    package_id,
                    range: range.to_owned(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let (source, manifest_fingerprint, archive_sha256) = match &row.source {
            ResolverCatalogSource::Repository {
                repository_id,
                repository_revision,
                repository_priority,
                source_identity,
                artifact_url,
                zip_sha256,
                manifest_fingerprint,
            } => (
                PackageSource {
                    authority: PackageSourceAuthority::Repository {
                        repository_id: repository_id.clone(),
                        repository_revision: *repository_revision,
                        priority: *repository_priority,
                        artifact_url: artifact_url.clone(),
                    },
                    source_identity: source_identity.clone(),
                    manifest_fingerprint: *manifest_fingerprint,
                    archive_sha256: parse_digest(zip_sha256)?,
                },
                *manifest_fingerprint,
                parse_digest(zip_sha256)?,
            ),
            ResolverCatalogSource::UserPackage {
                user_package_id,
                source_revision,
                source_identity,
                archive_sha256,
                manifest_fingerprint,
            } => (
                PackageSource {
                    authority: PackageSourceAuthority::UserPackage {
                        user_package_id: *user_package_id,
                        source_revision: *source_revision,
                    },
                    source_identity: source_identity.clone(),
                    manifest_fingerprint: *manifest_fingerprint,
                    archive_sha256: *archive_sha256,
                },
                *manifest_fingerprint,
                *archive_sha256,
            ),
        };
        debug_assert_eq!(source.manifest_fingerprint, manifest_fingerprint);
        debug_assert_eq!(source.archive_sha256, archive_sha256);
        candidates.push(PackageCandidate {
            package_id: row.package_id.clone(),
            version,
            yanked: row.yanked,
            unity_minimum,
            legacy_metadata_present: row.legacy_metadata_present,
            dependencies,
            source,
        });
    }
    candidates.sort_by(|left, right| {
        left.package_id
            .as_bytes()
            .cmp(right.package_id.as_bytes())
            .then_with(|| compare_precedence(&left.version, &right.version))
            .then_with(|| left.source.priority().cmp(&right.source.priority()))
            .then_with(|| {
                left.source
                    .deterministic_key()
                    .cmp(&right.source.deterministic_key())
            })
    });
    Ok(candidates)
}

impl fmt::Display for ResolveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPackageId => formatter.write_str("invalid package identifier"),
            Self::InvalidVersion => formatter.write_str("invalid package version"),
            Self::InvalidRange => formatter.write_str("invalid VPM version range"),
            Self::TooManyRequirements => formatter.write_str("dependency graph exceeds its limit"),
            Self::PackageNotFound { package_id } => {
                write!(
                    formatter,
                    "no compatible package candidate for {package_id}"
                )
            }
            Self::DependencyMissing { package_id } => {
                write!(formatter, "package dependency is missing: {package_id}")
            }
            Self::DependencyConflict { package_id } => {
                write!(
                    formatter,
                    "package dependency constraints conflict: {package_id}"
                )
            }
            Self::UnityIncompatible { package_id } => {
                write!(
                    formatter,
                    "package is incompatible with Unity: {package_id}"
                )
            }
            Self::VersionYanked { package_id } => {
                write!(formatter, "package version is yanked: {package_id}")
            }
            Self::SourceAmbiguous {
                package_id,
                version,
            } => write!(
                formatter,
                "package source is ambiguous for {package_id}@{version}"
            ),
            Self::LegacyCleanupRequired { package_id } => {
                write!(
                    formatter,
                    "package requires unsupported legacy cleanup: {package_id}"
                )
            }
        }
    }
}

impl std::error::Error for ResolveError {}

#[derive(Clone, Debug)]
struct Requirement {
    from_package_id: String,
    range_text: String,
    range: VpmRange,
    source: Option<PackageSourceSelector>,
    direct: bool,
}

struct Solved<'a> {
    selected: BTreeMap<String, &'a PackageCandidate>,
    requirements: BTreeMap<String, Vec<Requirement>>,
}

pub fn resolve_packages(
    catalog: &[PackageCandidate],
    requests: &[ResolveRequest],
) -> Result<Resolution, ResolveError> {
    if requests.is_empty() || requests.len() > MAX_REQUIREMENTS {
        return Err(ResolveError::TooManyRequirements);
    }

    let mut requirements = BTreeMap::<String, Vec<Requirement>>::new();
    let mut include_prerelease = false;
    let mut unity_version = None;
    for request in requests {
        validate_package_id(&request.package_id)?;
        include_prerelease |= request.include_prerelease;
        if let Some(request_unity) = request.unity_version {
            if unity_version.is_some_and(|existing| existing != request_unity) {
                return Err(ResolveError::InvalidRange);
            }
            unity_version = Some(request_unity);
        }
        let range = VpmRange::parse(&request.range).map_err(|_| ResolveError::InvalidRange)?;
        requirements
            .entry(request.package_id.clone())
            .or_default()
            .push(Requirement {
                from_package_id: request.package_id.clone(),
                range_text: range.canonical(),
                range,
                source: request.source.clone(),
                direct: true,
            });
    }

    let mut catalog_by_id = BTreeMap::<&str, Vec<&PackageCandidate>>::new();
    for candidate in catalog {
        validate_candidate(candidate)?;
        catalog_by_id
            .entry(candidate.package_id.as_str())
            .or_default()
            .push(candidate);
    }

    let solved = solve(
        &catalog_by_id,
        requirements.clone(),
        BTreeMap::new(),
        include_prerelease,
        unity_version,
    )?;
    let direct_ids = requests
        .iter()
        .map(|request| request.package_id.as_str())
        .collect::<BTreeSet<_>>();
    let packages = solved
        .selected
        .into_values()
        .map(|candidate| ResolvedPackage {
            package_id: candidate.package_id.clone(),
            version: candidate.version.clone(),
            source: candidate.source.clone(),
            direct: direct_ids.contains(candidate.package_id.as_str()),
        })
        .collect();
    let mut dependency_edges = solved
        .requirements
        .into_iter()
        .flat_map(|(to_package_id, requirements)| {
            requirements
                .into_iter()
                .map(move |requirement| PackageDependencyEdge {
                    from_package_id: requirement.from_package_id,
                    to_package_id: to_package_id.clone(),
                    range: requirement.range_text,
                    direct: requirement.direct,
                })
        })
        .collect::<Vec<_>>();
    dependency_edges.sort();
    dependency_edges.dedup();
    Ok(Resolution {
        packages,
        dependency_edges,
    })
}

fn solve<'a>(
    catalog: &BTreeMap<&str, Vec<&'a PackageCandidate>>,
    requirements: BTreeMap<String, Vec<Requirement>>,
    selected: BTreeMap<String, &'a PackageCandidate>,
    include_prerelease: bool,
    unity_version: Option<(u64, u64)>,
) -> Result<Solved<'a>, ResolveError> {
    for (package_id, package_requirements) in &requirements {
        if let Some(candidate) = selected.get(package_id)
            && !candidate_satisfies(
                candidate,
                package_requirements,
                include_prerelease,
                unity_version,
            )
        {
            return Err(ResolveError::PackageNotFound {
                package_id: package_id.clone(),
            });
        }
    }

    let Some((package_id, package_requirements)) = requirements
        .iter()
        .find(|(package_id, _)| !selected.contains_key(*package_id))
    else {
        return Ok(Solved {
            selected,
            requirements,
        });
    };

    let mut candidates = catalog
        .get(package_id.as_str())
        .into_iter()
        .flatten()
        .copied()
        .filter(|candidate| {
            candidate_satisfies(
                candidate,
                package_requirements,
                include_prerelease,
                unity_version,
            )
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err(classify_no_candidates(
            catalog,
            package_id,
            package_requirements,
            include_prerelease,
            unity_version,
        ));
    }
    candidates.sort_by(candidate_order);

    let mut last_error = None;
    let mut index = 0;
    while index < candidates.len() {
        let precedence_end = candidates[index..]
            .iter()
            .position(|candidate| {
                compare_precedence(&candidate.version, &candidates[index].version)
                    != Ordering::Equal
            })
            .map_or(candidates.len(), |offset| index + offset);
        let precedence_group = &candidates[index..precedence_end];
        let best_priority = precedence_group
            .iter()
            .map(|candidate| candidate.source.priority())
            .min()
            .expect("non-empty candidate group");
        let preferred = precedence_group
            .iter()
            .copied()
            .filter(|candidate| candidate.source.priority() == best_priority)
            .collect::<Vec<_>>();
        let has_repository = precedence_group.iter().any(|candidate| {
            matches!(
                candidate.source.authority,
                PackageSourceAuthority::Repository { .. }
            )
        });
        let has_user_package = precedence_group.iter().any(|candidate| {
            matches!(
                candidate.source.authority,
                PackageSourceAuthority::UserPackage { .. }
            )
        });
        let source_is_explicit = package_requirements
            .iter()
            .any(|requirement| requirement.source.is_some());
        if !source_is_explicit && has_repository && has_user_package {
            return Err(ResolveError::SourceAmbiguous {
                package_id: package_id.clone(),
                version: candidates[index].version.to_string(),
            });
        }
        if preferred.len() > 1 {
            return Err(ResolveError::SourceAmbiguous {
                package_id: package_id.clone(),
                version: candidates[index].version.to_string(),
            });
        }
        let candidate = preferred[0];
        if candidate.legacy_metadata_present {
            return Err(ResolveError::LegacyCleanupRequired {
                package_id: package_id.clone(),
            });
        }

        let mut next_requirements = requirements.clone();
        let mut too_many = false;
        for dependency in &candidate.dependencies {
            let ranges = next_requirements
                .entry(dependency.package_id.clone())
                .or_default();
            let range =
                VpmRange::parse(&dependency.range).map_err(|_| ResolveError::InvalidRange)?;
            ranges.push(Requirement {
                from_package_id: candidate.package_id.clone(),
                range_text: range.canonical(),
                range,
                source: None,
                direct: false,
            });
            if next_requirements.values().map(Vec::len).sum::<usize>() > MAX_REQUIREMENTS {
                too_many = true;
                break;
            }
        }
        if too_many {
            return Err(ResolveError::TooManyRequirements);
        }
        let mut next_selected = selected.clone();
        next_selected.insert(package_id.clone(), candidate);
        match solve(
            catalog,
            next_requirements,
            next_selected,
            include_prerelease,
            unity_version,
        ) {
            Ok(solution) => return Ok(solution),
            Err(error @ ResolveError::SourceAmbiguous { .. })
            | Err(error @ ResolveError::LegacyCleanupRequired { .. })
            | Err(error @ ResolveError::TooManyRequirements) => return Err(error),
            Err(error) => last_error = Some(error),
        }
        index = precedence_end;
    }

    Err(last_error.unwrap_or_else(|| ResolveError::PackageNotFound {
        package_id: package_id.clone(),
    }))
}

fn classify_no_candidates(
    catalog: &BTreeMap<&str, Vec<&PackageCandidate>>,
    package_id: &str,
    requirements: &[Requirement],
    include_prerelease: bool,
    unity_version: Option<(u64, u64)>,
) -> ResolveError {
    let Some(candidates) = catalog.get(package_id) else {
        return if requirements.iter().any(|requirement| !requirement.direct) {
            ResolveError::DependencyMissing {
                package_id: package_id.to_owned(),
            }
        } else {
            ResolveError::PackageNotFound {
                package_id: package_id.to_owned(),
            }
        };
    };
    let source_and_range_match = |candidate: &&PackageCandidate| {
        requirements.iter().all(|requirement| {
            requirement
                .source
                .as_ref()
                .is_none_or(|selector| candidate.source.matches_selector(selector))
                && requirement
                    .range
                    .matches(&candidate.version, include_prerelease)
        })
    };
    if candidates
        .iter()
        .copied()
        .filter(source_and_range_match)
        .any(|candidate| candidate.yanked)
    {
        return ResolveError::VersionYanked {
            package_id: package_id.to_owned(),
        };
    }
    if candidates
        .iter()
        .copied()
        .filter(source_and_range_match)
        .any(|candidate| {
            candidate
                .unity_minimum
                .is_some_and(|minimum| unity_version.is_none_or(|unity| unity < minimum))
        })
    {
        return ResolveError::UnityIncompatible {
            package_id: package_id.to_owned(),
        };
    }
    if requirements.len() > 1 || requirements.iter().any(|requirement| !requirement.direct) {
        ResolveError::DependencyConflict {
            package_id: package_id.to_owned(),
        }
    } else {
        ResolveError::PackageNotFound {
            package_id: package_id.to_owned(),
        }
    }
}

fn candidate_satisfies(
    candidate: &PackageCandidate,
    requirements: &[Requirement],
    include_prerelease: bool,
    unity_version: Option<(u64, u64)>,
) -> bool {
    !candidate.yanked
        && candidate
            .unity_minimum
            .is_none_or(|minimum| unity_version.is_some_and(|unity| unity >= minimum))
        && requirements.iter().all(|requirement| {
            requirement
                .source
                .as_ref()
                .is_none_or(|selector| candidate.source.matches_selector(selector))
                && requirement
                    .range
                    .matches(&candidate.version, include_prerelease)
        })
}

fn candidate_order(left: &&PackageCandidate, right: &&PackageCandidate) -> Ordering {
    compare_precedence(&right.version, &left.version)
        .then_with(|| left.source.priority().cmp(&right.source.priority()))
        .then_with(|| left.package_id.as_bytes().cmp(right.package_id.as_bytes()))
}

fn validate_candidate(candidate: &PackageCandidate) -> Result<(), ResolveError> {
    validate_package_id(&candidate.package_id)?;
    let authority_valid = match &candidate.source.authority {
        PackageSourceAuthority::Repository {
            repository_id,
            repository_revision,
            priority,
            artifact_url,
        } => {
            !repository_id.is_empty()
                && *repository_revision != 0
                && *priority != 0
                && !artifact_url.is_empty()
        }
        PackageSourceAuthority::UserPackage {
            source_revision, ..
        } => *source_revision != 0,
    };
    if !authority_valid
        || candidate.source.source_identity.is_empty()
        || candidate.dependencies.len() > MAX_REQUIREMENTS
    {
        return Err(ResolveError::InvalidPackageId);
    }
    for dependency in &candidate.dependencies {
        validate_package_id(&dependency.package_id)?;
        VpmRange::parse(&dependency.range).map_err(|_| ResolveError::InvalidRange)?;
    }
    Ok(())
}

fn validate_package_id(value: &str) -> Result<(), ResolveError> {
    if value.is_empty()
        || value.len() > 128
        || !value.as_bytes()[0].is_ascii_alphanumeric()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(ResolveError::InvalidPackageId);
    }
    Ok(())
}

fn parse_unity(value: &str) -> Result<(u64, u64), ResolveError> {
    let (major, minor) = value.split_once('.').ok_or(ResolveError::InvalidRange)?;
    if major.is_empty()
        || minor.is_empty()
        || minor.contains('.')
        || !major.bytes().all(|byte| byte.is_ascii_digit())
        || !minor.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(ResolveError::InvalidRange);
    }
    Ok((
        major.parse().map_err(|_| ResolveError::InvalidRange)?,
        minor.parse().map_err(|_| ResolveError::InvalidRange)?,
    ))
}

fn parse_digest(value: &str) -> Result<[u8; 32], ResolveError> {
    if value.len() != 64
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(ResolveError::InvalidRange);
    }
    let mut digest = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        digest[index] = (digest_nibble(pair[0]) << 4) | digest_nibble(pair[1]);
    }
    Ok(digest)
}

fn digest_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        package_id: &str,
        version: &str,
        priority: u64,
        repository_id: &str,
        dependencies: &[(&str, &str)],
    ) -> PackageCandidate {
        PackageCandidate {
            package_id: package_id.to_owned(),
            version: Version::parse(version).expect("version"),
            yanked: false,
            unity_minimum: None,
            legacy_metadata_present: false,
            dependencies: dependencies
                .iter()
                .map(|(package_id, range)| PackageDependency {
                    package_id: (*package_id).to_owned(),
                    range: (*range).to_owned(),
                })
                .collect(),
            source: PackageSource {
                authority: PackageSourceAuthority::Repository {
                    repository_id: repository_id.to_owned(),
                    repository_revision: 1,
                    priority,
                    artifact_url: format!("https://example.invalid/{package_id}-{version}.zip"),
                },
                source_identity: format!("source:{repository_id}"),
                manifest_fingerprint: [priority as u8; 32],
                archive_sha256: [priority as u8; 32],
            },
        }
    }

    fn request(package_id: &str, range: &str) -> ResolveRequest {
        ResolveRequest {
            package_id: package_id.to_owned(),
            range: range.to_owned(),
            source: None,
            include_prerelease: false,
            unity_version: Some((2022, 3)),
        }
    }

    fn user_candidate(package_id: &str, version: &str) -> PackageCandidate {
        PackageCandidate {
            package_id: package_id.to_owned(),
            version: Version::parse(version).expect("version"),
            yanked: false,
            unity_minimum: None,
            legacy_metadata_present: false,
            dependencies: Vec::new(),
            source: PackageSource {
                authority: PackageSourceAuthority::UserPackage {
                    user_package_id: alcomd_application::UserPackageId::new(),
                    source_revision: 1,
                },
                source_identity: "user-package-source".to_owned(),
                manifest_fingerprint: [8; 32],
                archive_sha256: [9; 32],
            },
        }
    }

    #[test]
    fn direct_and_transitive_resolution_is_deterministic_across_input_order() {
        let catalog = vec![
            candidate(
                "com.example.root",
                "1.0.0",
                1,
                "repo-a",
                &[("com.example.dep", "^1.0.0")],
            ),
            candidate("com.example.dep", "1.1.0", 2, "repo-b", &[]),
            candidate("com.example.dep", "1.2.0", 1, "repo-a", &[]),
        ];
        let mut reversed = catalog.clone();
        reversed.reverse();
        let left = resolve_packages(&catalog, &[request("com.example.root", "1.0.0")])
            .expect("resolution");
        let right = resolve_packages(&reversed, &[request("com.example.root", "1.0.0")])
            .expect("resolution");
        assert_eq!(left, right);
        assert_eq!(
            left.packages
                .iter()
                .map(|package| (package.package_id.as_str(), package.version.to_string()))
                .collect::<Vec<_>>(),
            [
                ("com.example.dep", "1.2.0".to_owned()),
                ("com.example.root", "1.0.0".to_owned())
            ]
        );
    }

    #[test]
    fn same_precedence_and_priority_is_ambiguous_instead_of_using_iteration_order() {
        let catalog = [
            candidate("com.example.pkg", "1.0.0+left", 1, "repo-a", &[]),
            candidate("com.example.pkg", "1.0.0+right", 1, "repo-b", &[]),
        ];
        assert!(matches!(
            resolve_packages(&catalog, &[request("com.example.pkg", ">=1.0.0")]),
            Err(ResolveError::SourceAmbiguous { .. })
        ));
    }

    #[test]
    fn repository_and_user_package_require_explicit_source_without_hidden_priority() {
        let repository = candidate("com.example.pkg", "1.0.0", 1, "repo-a", &[]);
        let user = user_candidate("com.example.pkg", "1.0.0");
        let user_id = match &user.source.authority {
            PackageSourceAuthority::UserPackage {
                user_package_id, ..
            } => *user_package_id,
            PackageSourceAuthority::Repository { .. } => unreachable!(),
        };
        assert!(matches!(
            resolve_packages(
                &[repository.clone(), user.clone()],
                &[request("com.example.pkg", "=1.0.0")]
            ),
            Err(ResolveError::SourceAmbiguous { .. })
        ));

        let mut repository_request = request("com.example.pkg", "=1.0.0");
        repository_request.source = Some(PackageSourceSelector::Repository {
            repository_id: "repo-a".to_owned(),
        });
        let repository_resolution =
            resolve_packages(&[repository.clone(), user.clone()], &[repository_request])
                .expect("explicit repository source");
        assert!(matches!(
            repository_resolution.packages[0].source.authority,
            PackageSourceAuthority::Repository { .. }
        ));

        let mut user_request = request("com.example.pkg", "=1.0.0");
        user_request.source = Some(PackageSourceSelector::UserPackage {
            user_package_id: user_id,
        });
        let user_resolution =
            resolve_packages(&[repository, user], &[user_request]).expect("explicit user source");
        assert!(matches!(
            user_resolution.packages[0].source.authority,
            PackageSourceAuthority::UserPackage { .. }
        ));
    }

    #[test]
    fn build_metadata_never_becomes_a_hidden_version_tiebreak() {
        let catalog = [
            candidate("com.example.pkg", "1.0.0+z", 2, "repo-low", &[]),
            candidate("com.example.pkg", "1.0.0+a", 1, "repo-high", &[]),
        ];
        let resolution = resolve_packages(&catalog, &[request("com.example.pkg", ">=1.0.0")])
            .expect("resolution");
        assert!(matches!(
            &resolution.packages[0].source.authority,
            PackageSourceAuthority::Repository { repository_id, .. }
                if repository_id == "repo-high"
        ));
        assert_eq!(resolution.packages[0].version.to_string(), "1.0.0+a");
    }

    #[test]
    fn yanked_unity_and_source_pin_constraints_fail_closed() {
        let mut blocked = candidate("com.example.pkg", "1.0.0", 1, "repo-a", &[]);
        blocked.yanked = true;
        let mut incompatible = candidate("com.example.pkg", "2.0.0", 1, "repo-a", &[]);
        incompatible.unity_minimum = Some((2023, 1));
        let mut pinned = request("com.example.pkg", ">=1.0.0");
        pinned.source = Some(PackageSourceSelector::Repository {
            repository_id: "repo-b".to_owned(),
        });
        assert!(matches!(
            resolve_packages(&[blocked, incompatible], &[pinned]),
            Err(ResolveError::PackageNotFound { .. })
        ));
    }
}
