use crate::{
    AccessContext, IdempotencyKey, OperationId, Permission, PrincipalId, ProjectId, Revision,
};
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "method", rename_all_fields = "camelCase")]
pub enum DesktopAction {
    #[serde(rename = "desktop.focusProjectUnity")]
    Focus {
        project_id: ProjectId,
        expected_revision: Revision,
    },
    #[serde(rename = "desktop.openProjectDirectory")]
    Directory { project_id: ProjectId },
    #[serde(rename = "desktop.openUnityHubDownload")]
    Download,
}

impl DesktopAction {
    pub fn method(&self) -> &'static str {
        match self {
            Self::Focus { .. } => "desktop.focusProjectUnity",
            Self::Directory { .. } => "desktop.openProjectDirectory",
            Self::Download => "desktop.openUnityHubDownload",
        }
    }
    pub fn project_id(&self) -> Option<ProjectId> {
        match self {
            Self::Focus { project_id, .. } | Self::Directory { project_id } => Some(*project_id),
            Self::Download => None,
        }
    }
    pub fn permissions(&self) -> &'static [Permission] {
        match self {
            Self::Focus { .. } => &[Permission::ProjectsRead, Permission::UnityRead],
            Self::Directory { .. } => &[Permission::ProjectsRead],
            Self::Download => &[],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesktopError {
    PermissionDenied,
    ProjectNotFound,
    ResourceRevisionConflict,
    InvalidRequest,
    IdempotencyConflict,
    DesktopUnavailable,
    DesktopResultUnknown,
    ProjectDirectoryMissing,
    ProjectDirectoryNotDirectory,
    ProjectDirectoryOpenFailed,
    UnityHubDownloadOpenFailed,
    UnityWindowNotFound,
    UnityWindowFocusRejected,
    UnityWindowFocusFailed,
    StoreUnavailable,
}

#[derive(Clone, Debug)]
pub struct DesktopRequest {
    pub owner: PrincipalId,
    pub action: DesktopAction,
    pub key: IdempotencyKey,
}

pub enum DesktopAcceptance {
    Accepted(OperationId),
    Replay(Result<(), DesktopError>),
}

#[derive(Clone)]
pub struct DesktopProject {
    pub revision: Revision,
    pub root: PathBuf,
}

pub trait DesktopStore: Send + Sync {
    fn desktop_replay(
        &self,
        request: DesktopRequest,
    ) -> impl Future<Output = Result<Option<Result<(), DesktopError>>, DesktopError>> + Send;
    fn desktop_accept(
        &self,
        request: DesktopRequest,
        project: Option<DesktopProject>,
        now: u64,
    ) -> impl Future<Output = Result<DesktopAcceptance, DesktopError>> + Send;
    fn desktop_finish(
        &self,
        request: DesktopRequest,
        id: OperationId,
        result: Result<(), DesktopError>,
        now: u64,
    ) -> impl Future<Output = Result<(), DesktopError>> + Send;
}

pub trait DesktopEnvironment: Send + Sync {
    fn prepare(
        &self,
        access: &AccessContext,
        action: &DesktopAction,
    ) -> impl Future<Output = Result<Option<DesktopProject>, DesktopError>> + Send;
    fn perform(
        &self,
        action: &DesktopAction,
        root: Option<&Path>,
    ) -> impl Future<Output = Result<(), DesktopError>> + Send;
}

pub async fn run_desktop_action(
    access: &AccessContext,
    store: &impl DesktopStore,
    environment: &impl DesktopEnvironment,
    action: DesktopAction,
    key: String,
) -> Result<(), DesktopError> {
    if !access.is_interactive_client() {
        return Err(DesktopError::PermissionDenied);
    }
    for permission in action.permissions() {
        access
            .require(*permission)
            .map_err(|_| DesktopError::PermissionDenied)?;
    }
    let request = DesktopRequest {
        owner: access.principal().clone(),
        action,
        key: IdempotencyKey::parse(key).map_err(|_| DesktopError::InvalidRequest)?,
    };
    if let Some(replay) = store.desktop_replay(request.clone()).await? {
        return replay;
    }
    let project = environment.prepare(access, &request.action).await?;
    let now = || super::unix_time_ms().map_err(|_| DesktopError::StoreUnavailable);
    match store
        .desktop_accept(request.clone(), project.clone(), now()?)
        .await?
    {
        DesktopAcceptance::Replay(result) => result,
        DesktopAcceptance::Accepted(id) => {
            let result = environment
                .perform(&request.action, project.as_ref().map(|p| p.root.as_path()))
                .await;
            store
                .desktop_finish(request, id, result, now()?)
                .await
                .map_err(|_| DesktopError::DesktopResultUnknown)?;
            result
        }
    }
}
