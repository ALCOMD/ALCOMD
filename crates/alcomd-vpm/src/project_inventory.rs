use std::{collections::BTreeMap, path::Path};

use alcomd_application::{
    CandidateQueryError as Error, InstalledEvidence as State, M3ReadAdapter, ManifestState,
    ProjectInventory, ProjectInventoryEntry, ProjectInventoryReader, ProjectObservation,
    ProjectResolutionEvidence as Resolution, valid_presentation_package_id,
};

use crate::{ProjectDiscoveryMode, VpmReader, range::ProjectDependencyRange};

impl ProjectInventoryReader for VpmReader {
    async fn read_project_inventory(
        &self,
        registered: &ProjectObservation,
    ) -> Result<ProjectInventory, Error> {
        let observed = self
            .inspect_project(
                registered.root_path.clone(),
                ProjectDiscoveryMode::ExactRoot,
            )
            .await
            .map_err(|error| match error.code() {
                alcomd_application::M3ErrorCode::ProjectManifestInvalid => {
                    Error::ProjectManifestInvalid
                }
                _ => Error::Unavailable,
            })?;
        if observed.path_identity_key != registered.path_identity_key {
            return Err(Error::EvidenceStale);
        }
        let mut inventory = BTreeMap::<String, (Option<String>, Option<String>)>::new();
        for dependency in observed.direct_dependencies {
            inventory.entry(dependency.package_id).or_default().0 = Some(dependency.value);
        }
        for dependency in observed.locked_dependencies {
            inventory.entry(dependency.package_id).or_default().1 = Some(dependency.value);
        }
        if inventory.len() > alcomd_application::MAX_PRESENTATION_PACKAGES {
            return Err(Error::LimitExceeded);
        }
        let root = Path::new(&observed.root_path);
        let mut remaining = alcomd_application::MAX_PRESENTATION_BYTES;
        let mut entries = Vec::with_capacity(inventory.len());
        let mut unresolved = false;
        let mut uncertain = false;
        for (package_id, (declared_range, locked_version)) in inventory {
            let (state, observed_version) =
                if valid_presentation_package_id(&package_id) && remaining > 0 {
                    observe(root, &package_id, locked_version.as_deref(), &mut remaining).await
                } else {
                    (State::Unknown, None)
                };
            unresolved |= matches!(state, State::Missing | State::VersionMismatch)
                || (declared_range.is_some() && locked_version.is_none());
            uncertain |= state == State::Unknown;
            if let (Some(range), Some(version)) = (&declared_range, &locked_version) {
                match (
                    ProjectDependencyRange::parse(range),
                    semver::Version::parse(version),
                ) {
                    (Ok(range), Ok(version)) => unresolved |= !range.matches(&version),
                    _ => uncertain = true,
                }
            }
            entries.push(ProjectInventoryEntry {
                package_id,
                declared_range,
                locked_version,
                observed_version,
                state,
            });
        }
        let (resolution, reason) = if unresolved {
            (Resolution::Needed, "project_packages_out_of_sync")
        } else if observed.vpm_manifest == ManifestState::Missing {
            (Resolution::Unknown, "vpm_manifest_missing")
        } else if uncertain {
            (Resolution::Unknown, "package_evidence_incomplete")
        } else {
            (
                Resolution::NoIssueObserved,
                "declared_and_locked_packages_match",
            )
        };
        Ok(ProjectInventory {
            entries,
            resolution,
            reason: reason.into(),
        })
    }
}

async fn observe(
    root: &Path,
    id: &str,
    locked: Option<&str>,
    remaining: &mut usize,
) -> (State, Option<String>) {
    let limit = crate::PROJECT_MANIFEST_LIMIT.min(*remaining);
    let bytes = match crate::read_component(root, &["Packages", id, "package.json"], limit).await {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return (State::Missing, None),
        Err(_) => {
            *remaining -= limit;
            return (State::Unknown, None);
        }
    };
    *remaining -= bytes.len();
    let version = crate::parse_bounded_json(
        &bytes,
        alcomd_application::M3ErrorCode::ProjectManifestInvalid,
    )
    .ok()
    .and_then(|manifest| {
        let version = manifest["version"].as_str()?;
        (manifest["name"].as_str() == Some(id)
            && semver::Version::parse(version).is_ok()
            && version.len() <= 256)
            .then(|| version.to_owned())
    });
    let state = match (version.as_deref(), locked) {
        (Some(actual), Some(expected)) if actual == expected => State::Matched,
        (Some(_), Some(_)) => State::VersionMismatch,
        _ => State::Unknown,
    };
    (state, version)
}
