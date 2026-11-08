//! Transport-neutral, read-only candidate evidence and its normalized inputs.

use serde::{Deserialize, Serialize};

use crate::{ConfigPackageSettings, PackageSourceSelector, ProjectObservation};

/// A registered observation, including entries not ready for the resolver.
#[derive(Clone, Debug)]
pub struct CandidateCatalogEntry {
    pub description: Option<String>,
    pub presentation: Option<crate::PackageDisplayMetadata>,
    pub dependencies: std::collections::BTreeMap<String, String>,
    pub package_id: String,
    pub version: String,
    pub display_name: Option<String>,
    pub links: CandidateLinks,
    pub source: PackageSourceSelector,
    pub source_revision: u64,
    pub priority: u64,
    pub yanked: bool,
    pub unity: Option<String>,
    pub unity_release: Option<String>,
    pub metadata_ready: bool,
    pub legacy_metadata_present: bool,
}

#[derive(Clone, Debug)]
pub struct CandidateEvidenceInput {
    pub installed_presentations: crate::ProjectPackagePresentations,
    pub source_metadata: std::collections::HashMap<String, crate::CandidateSourceMetadata>,
    pub project: ProjectObservation,
    pub entries: Vec<CandidateCatalogEntry>,
    pub settings: ConfigPackageSettings,
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
    pub source: PackageSourceSelector,
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

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateProvider {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub builtin: Option<crate::BuiltinRepository>,
    pub source_kind: crate::CandidateFilterSource,
    pub display_order: u32,
    pub source: PackageSourceSelector,
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
    pub presentation_state: crate::PresentationState,
    #[serde(skip)]
    pub search_terms: Vec<String>,
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

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
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
    pub source: PackageSourceSelector,
}
