use serde::{Deserialize, Serialize};

pub const CAPABILITY_DESKTOP_ACTIONS_V1: &str = "desktop.actions.v1";
pub const METHOD_DESKTOP_FOCUS_PROJECT_UNITY: &str = "desktop.focusProjectUnity";
pub const METHOD_DESKTOP_OPEN_PROJECT_DIRECTORY: &str = "desktop.openProjectDirectory";
pub const METHOD_DESKTOP_OPEN_UNITY_HUB_DOWNLOAD: &str = "desktop.openUnityHubDownload";

impl crate::RpcError {
    pub fn desktop(code: &str) -> Self {
        Self::simple(code, "The requested desktop action could not be completed.")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopFocusProjectUnityParams {
    pub project_id: String,
    pub expected_revision: u64,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopOpenProjectDirectoryParams {
    pub project_id: String,
    pub idempotency_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopOpenUnityHubDownloadParams {
    pub idempotency_key: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DesktopActionOutcome {
    Focused,
    Requested,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesktopActionResult {
    pub outcome: DesktopActionOutcome,
}
