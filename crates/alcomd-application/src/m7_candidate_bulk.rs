use super::{
    CandidateChoice, CandidateClassification, CandidateEligibility, CandidateInstalled,
    CandidateRelation, CandidateSourceOverride, CandidateSummary, CandidateUpdate,
    PackageSourceSelector,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const MAX_TARGETS: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum CandidateBulkIntent {
    Install {
        package_id: String,
        version_range: String,
        source: PackageSourceSelector,
        include_prerelease: bool,
    },
    Upgrade {
        package_id: String,
        version_range: String,
        source: PackageSourceSelector,
        include_prerelease: bool,
    },
    Remove {
        package_id: String,
    },
    Reinstall {
        package_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<PackageSourceSelector>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateBulkExclusion {
    pub package_id: String,
    pub update: CandidateUpdate,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateBulkAction {
    pub target_count: usize,
    pub intents: Vec<CandidateBulkIntent>,
    pub exclusions: Vec<CandidateBulkExclusion>,
    pub omitted_exclusions: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateBulkActions {
    pub update_all: CandidateBulkAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_all_stable: Option<CandidateBulkAction>,
    pub install_update: CandidateBulkAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install_update_stable: Option<CandidateBulkAction>,
    pub remove: Vec<CandidateBulkIntent>,
    pub reinstall: Vec<CandidateBulkIntent>,
}

fn update_choice(row: &CandidateSummary, stable: bool) -> (&CandidateChoice, &CandidateUpdate) {
    if stable {
        (&row.project_latest_stable, &row.stable_update)
    } else {
        (&row.project_latest, &row.update)
    }
}

fn intent(row: &CandidateSummary, stable: bool) -> Option<CandidateBulkIntent> {
    let (choice, update) = update_choice(row, stable);
    let candidate = match (&row.installed, choice, update) {
        (CandidateInstalled::Locked { .. }, _, CandidateUpdate::Target { candidate })
        | (CandidateInstalled::Absent, CandidateChoice::Candidate { candidate }, _) => candidate,
        _ => return None,
    };
    if candidate.eligibility != CandidateEligibility::Eligible
        || candidate.relation == CandidateRelation::Unknown
    {
        return None;
    }
    let package_id = row.package_id.clone();
    let version_range = format!("={}", candidate.version);
    let source = candidate.source.clone();
    let include_prerelease = candidate.classification == CandidateClassification::Prerelease;
    Some(if matches!(row.installed, CandidateInstalled::Absent) {
        CandidateBulkIntent::Install {
            package_id,
            version_range,
            source,
            include_prerelease,
        }
    } else {
        CandidateBulkIntent::Upgrade {
            package_id,
            version_range,
            source,
            include_prerelease,
        }
    })
}

fn update_all(
    items: &[CandidateSummary],
    stable: bool,
    catalog_complete: bool,
) -> CandidateBulkAction {
    let mut action = CandidateBulkAction::default();
    let mut blocked = !catalog_complete;
    for row in items
        .iter()
        .filter(|row| !matches!(row.installed, CandidateInstalled::Absent))
    {
        let (choice, update) = update_choice(row, stable);
        blocked |= matches!(row.installed, CandidateInstalled::Unknown { .. })
            || matches!(
                choice,
                CandidateChoice::Unknown { .. } | CandidateChoice::Ambiguous { .. }
            );
        if let Some(intent) = intent(row, stable) {
            action.target_count += 1;
            if action.target_count <= MAX_TARGETS {
                action.intents.push(intent);
            }
        } else if !matches!(update, CandidateUpdate::Target { .. }) {
            if action.exclusions.len() < MAX_TARGETS {
                action.exclusions.push(CandidateBulkExclusion {
                    package_id: row.package_id.clone(),
                    update: update.clone(),
                });
            } else {
                action.omitted_exclusions += 1;
            }
        }
    }
    if blocked || action.target_count > MAX_TARGETS {
        action.intents.clear();
    }
    action
}

pub(super) fn bulk_actions(
    items: &[CandidateSummary],
    selected_ids: &[String],
    source_overrides: &[CandidateSourceOverride],
    catalog_complete: bool,
) -> CandidateBulkActions {
    let update_all = update_all(items, false, catalog_complete);
    let stable = self::update_all(items, true, catalog_complete);
    let update_all_stable = (update_all.target_count != stable.target_count
        || update_all.intents != stable.intents)
        .then_some(stable);
    let ids: HashSet<&str> = selected_ids.iter().map(String::as_str).collect();
    let selected: Vec<_> = items
        .iter()
        .filter(|row| ids.contains(row.package_id.as_str()))
        .collect();
    let selection_valid =
        !selected.is_empty() && selected.len() == ids.len() && ids.len() <= MAX_TARGETS;
    let valid = catalog_complete && selection_valid;
    let install_update = |stable| {
        let intents: Vec<_> = selected
            .iter()
            .filter_map(|row| intent(row, stable))
            .collect();
        CandidateBulkAction {
            target_count: intents.len(),
            intents: if valid && intents.len() == selected.len() {
                intents
            } else {
                Vec::new()
            },
            ..Default::default()
        }
    };
    let latest = install_update(false);
    let stable = install_update(true);
    let install_update_stable = (latest.target_count != stable.target_count
        || latest.intents != stable.intents)
        .then_some(stable);
    let remove = if selection_valid
        && selected
            .iter()
            .all(|row| matches!(row.installed, CandidateInstalled::Locked { .. }))
    {
        selected
            .iter()
            .map(|row| CandidateBulkIntent::Remove {
                package_id: row.package_id.clone(),
            })
            .collect()
    } else {
        Vec::new()
    };
    let reinstall = if valid
        && selected
            .iter()
            .all(|row| matches!(row.installed, CandidateInstalled::Locked { .. }))
    {
        let sources: HashMap<_, _> = source_overrides
            .iter()
            .map(|item| (item.package_id.as_str(), &item.source))
            .collect();
        selected
            .iter()
            .map(|row| CandidateBulkIntent::Reinstall {
                package_id: row.package_id.clone(),
                source: sources
                    .get(row.package_id.as_str())
                    .map(|source| (*source).clone()),
            })
            .collect()
    } else {
        Vec::new()
    };
    CandidateBulkActions {
        update_all,
        update_all_stable,
        install_update: latest,
        install_update_stable,
        remove,
        reinstall,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removal_eligibility_uses_installed_state_without_direct_or_catalog_requirements() {
        let empty = CandidateChoice::None {
            reasons: Vec::new(),
        };
        let mut row = CandidateSummary {
            package_id: "com.example.transitive".into(),
            requested_range: None,
            description: None,
            installed_yanked: None,
            presentation_state: crate::PresentationState::Complete,
            search_terms: Vec::new(),
            display_name: None,
            links: None,
            providers: Vec::new(),
            direct: false,
            installed: CandidateInstalled::Locked {
                version: "1.0.0".into(),
            },
            latest: empty.clone(),
            latest_stable: empty.clone(),
            project_latest: empty.clone(),
            project_latest_stable: empty,
            update: CandidateUpdate::NoUpdate {
                reason: crate::CandidateNoUpdateReason::NotInstalled,
            },
            stable_update: CandidateUpdate::NoUpdate {
                reason: crate::CandidateNoUpdateReason::NotInstalled,
            },
        };
        let selected = vec![row.package_id.clone()];
        let actions = bulk_actions(&[row.clone()], &selected, &[], false);
        assert_eq!(
            actions.remove,
            vec![CandidateBulkIntent::Remove {
                package_id: row.package_id.clone()
            }]
        );
        assert!(actions.install_update.intents.is_empty());
        assert!(actions.reinstall.is_empty());
        row.installed = CandidateInstalled::Absent;
        row.direct = true;
        assert!(
            bulk_actions(&[row], &selected, &[], true).remove.is_empty(),
            "a declaration is not an installed package"
        );
        assert!(
            bulk_actions(&[], &selected, &[], false).remove.is_empty(),
            "unknown selection is not actionable"
        );
    }
}
