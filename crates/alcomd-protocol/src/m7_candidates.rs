//! Approved M7 project candidate read DTOs.
use serde::{Deserialize, Serialize};

/// Source identity in the candidate query's frozen snake_case wire format.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "snake_case",
    deny_unknown_fields
)]
pub enum CandidateSource {
    Repository { repository_id: String },
    UserPackage { user_package_id: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateClassification {
    Stable,
    Prerelease,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateRelation {
    NotInstalled,
    Newer,
    SamePrecedence,
    Older,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateUnity {
    NotRequired,
    Compatible,
    Incompatible,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateEligibility {
    Eligible,
    Blocked,
    Unknown,
}

/// Declaration order is the frozen order of independently applicable reasons.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateReason {
    NoVisibleCandidate,
    SourceUnavailable,
    AllYanked,
    PrereleaseExcluded,
    StableUnavailable,
    UnityIncompatible,
    ProjectUnityUnknown,
    CatalogIncomplete,
    MetadataInvalid,
    ClassificationUnknown,
    InstalledEvidenceUnknown,
    LegacyMetadata,
    SourceAmbiguous,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CandidateInstalled {
    Absent,
    Locked { version: String },
    Unknown { reason: CandidateReason },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateEvidence {
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub manual_unity_override_allowed: bool,
    pub version: String,
    pub source: CandidateSource,
    pub source_revision: u64,
    pub classification: CandidateClassification,
    pub relation: CandidateRelation,
    pub unity: CandidateUnity,
    pub eligibility: CandidateEligibility,
    pub reasons: Vec<CandidateReason>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CandidateChoice {
    Candidate { candidate: CandidateEvidence },
    None { reasons: Vec<CandidateReason> },
    Unknown { reasons: Vec<CandidateReason> },
    Ambiguous { reason: CandidateReason },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateNoUpdateReason {
    SamePrecedence,
    InstalledNewer,
    NotInstalled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CandidateUpdate {
    Target { candidate: CandidateEvidence },
    NoUpdate { reason: CandidateNoUpdateReason },
    Unavailable { reasons: Vec<CandidateReason> },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationState {
    Complete,
    Partial,
    Unobserved,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateProvider {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub builtin: Option<crate::BuiltinRepository>,
    pub source_kind: CandidateFilterSource,
    pub display_order: u32,
    pub source: CandidateSource,
    pub source_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_range: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_yanked: Option<bool>,
    pub presentation_state: PresentationState,
    pub package_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<CandidateLinks>,
    pub providers: Vec<CandidateProvider>,
    pub direct: bool,
    pub installed: CandidateInstalled,
    pub latest: CandidateChoice,
    pub latest_stable: CandidateChoice,
    pub project_latest: CandidateChoice,
    pub project_latest_stable: CandidateChoice,
    pub update: CandidateUpdate,
    pub stable_update: CandidateUpdate,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateLinks {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changelog: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateSourceOverride {
    pub package_id: String,
    pub source: CandidateSource,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectCandidateRequest {
    pub project_id: String,
    pub expected_revision: u64,
    pub view: CandidateView,
    #[serde(
        skip_serializing_if = "Option::is_none",
        default,
        deserialize_with = "nonnull_optional"
    )]
    pub limit: Option<u32>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        default,
        deserialize_with = "nonnull_optional"
    )]
    pub expected_snapshot: Option<String>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        default,
        deserialize_with = "nonnull_optional"
    )]
    pub cursor: Option<CandidateCursor>,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum CandidateView {
    Summary {
        #[serde(
            rename = "packageIds",
            skip_serializing_if = "Option::is_none",
            default,
            deserialize_with = "nonnull_optional"
        )]
        package_ids: Option<Vec<String>>,
        #[serde(
            skip_serializing_if = "Option::is_none",
            default,
            deserialize_with = "nonnull_optional"
        )]
        sources: Option<Vec<CandidateSourceOverride>>,
        #[serde(
            skip_serializing_if = "Option::is_none",
            default,
            deserialize_with = "nonnull_optional"
        )]
        filter: Option<CandidateFilter>,
        #[serde(
            rename = "bulkPackageIds",
            skip_serializing_if = "Option::is_none",
            default,
            deserialize_with = "nonnull_optional"
        )]
        bulk_package_ids: Option<Vec<String>>,
    },
    Versions {
        #[serde(rename = "packageId")]
        package_id: String,
        #[serde(
            skip_serializing_if = "Option::is_none",
            default,
            deserialize_with = "nonnull_optional"
        )]
        source: Option<CandidateSource>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateFilter {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "nonnull_optional"
    )]
    pub search: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "nonnull_optional"
    )]
    pub status: Option<CandidateFilterStatus>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "nonnull_optional"
    )]
    pub source: Option<CandidateFilterSource>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateFilterStatus {
    Installed,
    Available,
    Missing,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CandidateFilterSource {
    Local,
    Remote,
    UserPackage,
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
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum CandidateBulkIntent {
    Install {
        package_id: String,
        version_range: String,
        source: CandidateSource,
        include_prerelease: bool,
    },
    Upgrade {
        package_id: String,
        version_range: String,
        source: CandidateSource,
        include_prerelease: bool,
    },
    Remove {
        package_id: String,
    },
    Reinstall {
        package_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        source: Option<CandidateSource>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateBulkExclusion {
    pub package_id: String,
    pub update: CandidateUpdate,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateBulkAction {
    pub target_count: usize,
    pub intents: Vec<CandidateBulkIntent>,
    pub exclusions: Vec<CandidateBulkExclusion>,
    pub omitted_exclusions: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCandidatePage {
    pub presentation_complete: bool,
    pub project_id: String,
    pub snapshot: CandidateSnapshot,
    pub show_prerelease: bool,
    pub catalog_complete: bool,
    #[serde(flatten)]
    pub page: CandidatePageItems,
    pub next_cursor: Option<CandidateCursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bulk: Option<CandidateBulkActions>,
}

impl<'de> Deserialize<'de> for ProjectCandidatePage {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "view", rename_all = "snake_case", deny_unknown_fields)]
        enum ClosedPage {
            Summary {
                #[serde(rename = "projectId")]
                project_id: String,
                snapshot: CandidateSnapshot,
                #[serde(rename = "showPrerelease")]
                show_prerelease: bool,
                #[serde(rename = "catalogComplete")]
                catalog_complete: bool,
                #[serde(rename = "presentationComplete")]
                presentation_complete: bool,
                items: Vec<CandidateSummary>,
                #[serde(rename = "nextCursor", deserialize_with = "nullable_required")]
                next_cursor: Option<CandidateCursor>,
                #[serde(default, deserialize_with = "nonnull_optional")]
                bulk: Option<Box<CandidateBulkActions>>,
            },
            Versions {
                #[serde(rename = "projectId")]
                project_id: String,
                snapshot: CandidateSnapshot,
                #[serde(rename = "showPrerelease")]
                show_prerelease: bool,
                #[serde(rename = "catalogComplete")]
                catalog_complete: bool,
                #[serde(rename = "presentationComplete")]
                presentation_complete: bool,
                #[serde(rename = "packageId")]
                package_id: String,
                items: Vec<CandidateEvidence>,
                #[serde(rename = "nextCursor", deserialize_with = "nullable_required")]
                next_cursor: Option<CandidateCursor>,
            },
        }
        Ok(match ClosedPage::deserialize(deserializer)? {
            ClosedPage::Summary {
                project_id,
                snapshot,
                show_prerelease,
                catalog_complete,
                presentation_complete,
                items,
                next_cursor,
                bulk,
            } => Self {
                project_id,
                snapshot,
                show_prerelease,
                catalog_complete,
                presentation_complete,
                page: CandidatePageItems::Summary { items },
                next_cursor,
                bulk: bulk.map(|actions| *actions),
            },
            ClosedPage::Versions {
                project_id,
                snapshot,
                show_prerelease,
                catalog_complete,
                presentation_complete,
                package_id,
                items,
                next_cursor,
            } => Self {
                project_id,
                snapshot,
                show_prerelease,
                catalog_complete,
                presentation_complete,
                page: CandidatePageItems::Versions { package_id, items },
                next_cursor,
                bulk: None,
            },
        })
    }
}

fn nonnull_optional<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

fn nullable_required<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
