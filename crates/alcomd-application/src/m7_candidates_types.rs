//! Transport-neutral, read-only candidate evidence and its normalized inputs.

use serde::{Deserialize, Serialize};

use crate::{ConfigPackageSettings, PackageSourceSelector, ProjectObservation};

/// A registered observation, including entries not ready for the resolver.
#[derive(Clone, Debug)]
pub struct CandidateCatalogEntry {
    pub package_id: String,
    pub version: String,
    pub source: PackageSourceSelector,
    pub source_revision: u64,
    pub priority: u64,
    pub yanked: bool,
    pub unity: Option<String>,
    pub metadata_ready: bool,
    pub legacy_metadata_present: bool,
}

#[derive(Clone, Debug)]
pub struct CandidateEvidenceInput {
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
    pub source: PackageSourceSelector,
    pub source_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateSummary {
    pub package_id: String,
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
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CandidateSourceOverride {
    pub package_id: String,
    pub source: PackageSourceSelector,
}
