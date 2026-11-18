use serde::{Deserialize, Serialize};

use crate::{
    CandidateEligibility, CandidateReason, CandidateUnity, PackageBulkIntent, PackagePlan,
    PackageReinstallSelection, PackageReinstallSource, PackageSourceSelector,
};

pub const METHOD_MCP_PACKAGES_QUERY: &str = "mcp.packages.query";
pub const METHOD_MCP_PACKAGES_PLAN: &str = "mcp.packages.plan";
pub const CAPABILITY_MCP_PACKAGES_CATALOG_PLAN_V1: &str = "mcp.packages.catalog-plan.v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpPackageProject {
    pub project_id: String,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum McpPackageView {
    Sources,
    Summary {
        query: Option<String>,
        repository_id: Option<String>,
    },
    Versions {
        package_id: String,
        version: Option<String>,
        source: Option<PackageSourceSelector>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpPackageQuery {
    pub view: McpPackageView,
    pub project: Option<McpPackageProject>,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum McpPackageVersionSelector {
    LatestCompatible,
    Exact { version: String },
    Range { range: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum McpPackageIntent {
    Install {
        version_selector: McpPackageVersionSelector,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        include_prerelease: Option<bool>,
        #[serde(default, skip_serializing_if = "is_false")]
        allow_unity_incompatible: bool,
    },
    Upgrade {
        version_selector: McpPackageVersionSelector,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        include_prerelease: Option<bool>,
        #[serde(default, skip_serializing_if = "is_false")]
        allow_unity_incompatible: bool,
    },
    Downgrade {
        version: String,
        #[serde(default, skip_serializing_if = "is_false")]
        allow_unity_incompatible: bool,
    },
    Remove,
    Resolve {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        include_prerelease: Option<bool>,
    },
    Reinstall {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        selection: Option<PackageReinstallSelection>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        sources: Vec<PackageReinstallSource>,
    },
    Bulk {
        intents: Vec<PackageBulkIntent>,
    },
}

fn is_false(value: &bool) -> bool {
    !value
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpPackagePlanRequest {
    pub project: McpPackageProject,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<PackageSourceSelector>,
    pub intent: McpPackageIntent,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpPackageSource {
    pub source: PackageSourceSelector,
    pub display_name: Option<String>,
    pub availability: McpPackageSourceAvailability,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpPackageSourceAvailability {
    Available,
    Hidden,
    AuthenticationRequired,
    Unsupported,
    RefreshRequired,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpPackageCompatibility {
    pub unity: CandidateUnity,
    pub eligibility: CandidateEligibility,
    pub reasons: Vec<CandidateReason>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpPackageSummary {
    pub package_id: String,
    pub display_name: Option<String>,
    pub version: String,
    pub locked_version: Option<String>,
    pub sources: Vec<PackageSourceSelector>,
    pub compatibility: Option<McpPackageCompatibility>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpPackageVersion {
    pub package_id: String,
    pub version: String,
    pub source: PackageSourceSelector,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub unity: Option<String>,
    pub unity_release: Option<String>,
    pub documentation_url: Option<String>,
    pub changelog_url: Option<String>,
    pub aliases: Vec<String>,
    pub keywords: Vec<String>,
    pub legacy_packages: Vec<String>,
    pub dependencies: std::collections::BTreeMap<String, String>,
    pub yanked: bool,
    pub compatibility: Option<McpPackageCompatibility>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "items",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum McpPackageItems {
    Sources(Vec<McpPackageSource>),
    Summary(Vec<McpPackageSummary>),
    Versions(Vec<McpPackageVersion>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPackagePage {
    #[serde(flatten)]
    pub items: McpPackageItems,
    pub snapshot: String,
    pub catalog_complete: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpPackagePlanSource {
    pub package_id: String,
    pub source: PackageSourceSelector,
    pub display_name: Option<String>,
}

/// All mutation source pins are absent from the plan. Only safe source selectors appear here.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct McpPackagePlan {
    pub plan: PackagePlan,
    pub sources: Vec<McpPackagePlanSource>,
}
