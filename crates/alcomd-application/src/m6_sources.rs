//! Internal source adapter contracts. These are not guest Host capabilities.
use crate::{ExtensionPackageEvidence, M6Error};
use serde::{Deserialize, Serialize};
use std::{future::Future, path::PathBuf, pin::Pin};
pub type ExtensionSourceFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, M6Error>> + Send + 'a>>;
pub struct ExtensionSourceBytes {
    pub url: String,
    pub bytes: Vec<u8>,
}
pub trait ExtensionSourceFetcher: Send + Sync {
    fn normalize_url(&self, url: &str) -> Result<String, M6Error>;
    fn index<'a>(&'a self, url: &'a str) -> ExtensionSourceFuture<'a, ExtensionSourceBytes>;
    fn archive<'a>(&'a self, url: &'a str, sha256: [u8; 32]) -> ExtensionSourceFuture<'a, PathBuf>;
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionRemoteSourceEvidence {
    pub url: String,
    pub version: String,
    pub source_fingerprint: String,
    pub artifact_url: String,
    pub artifact_sha256: String,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionInstallationSource {
    pub url: String,
    pub version: String,
}
impl ExtensionRemoteSourceEvidence {
    pub fn summary(&self) -> ExtensionInstallationSource {
        ExtensionInstallationSource {
            url: self.url.clone(),
            version: self.version.clone(),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionSourceVersion {
    pub version: String,
    pub api_major: u32,
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionSourcePreview {
    pub url: String,
    pub extension_id: String,
    pub display_name: String,
    pub publisher_fingerprint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub versions: Vec<ExtensionSourceVersion>,
    pub source_fingerprint: String,
}
pub struct ExtensionPreparedSource {
    pub evidence: ExtensionPackageEvidence,
    pub plan_fingerprint: [u8; 32],
}
