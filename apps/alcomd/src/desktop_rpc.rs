use crate::{
    Applications, ConnectionState, DispatchAction, error_action, require_capability, success_action,
};
use alcomd_application as app;
use alcomd_platform::desktop as os;
use alcomd_protocol as rpc;
use os::DesktopDriver;
use std::path::Path;

impl app::DesktopEnvironment for Applications {
    async fn prepare(
        &self,
        access: &app::AccessContext,
        action: &app::DesktopAction,
    ) -> Result<Option<app::DesktopProject>, app::DesktopError> {
        let Some(id) = action.project_id() else {
            return Ok(None);
        };
        let project = self
            .m3
            .get_project(access, id)
            .await
            .map_err(|e| match e.code() {
                app::M3ErrorCode::ProjectNotFound => app::DesktopError::ProjectNotFound,
                app::M3ErrorCode::PermissionDenied => app::DesktopError::PermissionDenied,
                _ => app::DesktopError::StoreUnavailable,
            })?
            .project;
        if let app::DesktopAction::Focus {
            expected_revision, ..
        } = action
        {
            if *expected_revision != project.revision {
                return Err(app::DesktopError::ResourceRevisionConflict);
            }
            if !cfg!(windows) {
                return Err(app::DesktopError::DesktopUnavailable);
            }
            let observation =
                self.m5
                    .writer_state(access, id)
                    .await
                    .map_err(|e| match e.code() {
                        app::M5UnityErrorCode::PermissionDenied => {
                            app::DesktopError::PermissionDenied
                        }
                        app::M5UnityErrorCode::ProjectNotRegistered => {
                            app::DesktopError::ProjectNotFound
                        }
                        _ => app::DesktopError::StoreUnavailable,
                    })?;
            if !matches!(
                observation.state,
                app::UnityWriterStateKind::RunningConfirmed
                    | app::UnityWriterStateKind::RunningSuspected
            ) {
                return Err(app::DesktopError::UnityWindowNotFound);
            }
        }
        let root = tokio::task::spawn_blocking(move || {
            let root = os::registered_root(Path::new(&project.observation.root_path))?;
            if alcomd_platform::file_identity_key(&root)
                .map_err(|_| os::DesktopError::DirectoryOpenFailed)?
                != project.observation.path_identity_key
            {
                return Err(os::DesktopError::DirectoryMissing);
            }
            Ok(root)
        })
        .await
        .map_err(|_| app::DesktopError::ProjectDirectoryOpenFailed)?
        .map_err(platform_error)?;
        Ok(Some(app::DesktopProject {
            revision: project.revision,
            root,
        }))
    }

    async fn perform(
        &self,
        action: &app::DesktopAction,
        root: Option<&Path>,
    ) -> Result<(), app::DesktopError> {
        let action = action.clone();
        let root = root.map(Path::to_owned);
        #[cfg(feature = "test-kill-gates")]
        let driver: std::sync::Arc<dyn DesktopDriver> = self.desktop_driver.get().map_or_else(
            || std::sync::Arc::new(os::SystemDesktop) as std::sync::Arc<dyn DesktopDriver>,
            |observed| observed.clone(),
        );
        #[cfg(not(feature = "test-kill-gates"))]
        let driver = os::SystemDesktop;
        tokio::task::spawn_blocking(move || match action {
            app::DesktopAction::Focus { .. } => {
                driver.focus_unity(root.as_deref().ok_or(os::DesktopError::DirectoryMissing)?)
            }
            app::DesktopAction::Directory { .. } => {
                driver.open_directory(root.as_deref().ok_or(os::DesktopError::DirectoryMissing)?)
            }
            app::DesktopAction::Download => driver.open_unity_download(),
        })
        .await
        .map_err(|_| app::DesktopError::DesktopResultUnknown)?
        .map_err(platform_error)
    }
}

fn platform_error(e: os::DesktopError) -> app::DesktopError {
    match e {
        os::DesktopError::Unavailable => app::DesktopError::DesktopUnavailable,
        os::DesktopError::DirectoryMissing => app::DesktopError::ProjectDirectoryMissing,
        os::DesktopError::NotDirectory => app::DesktopError::ProjectDirectoryNotDirectory,
        os::DesktopError::DirectoryOpenFailed => app::DesktopError::ProjectDirectoryOpenFailed,
        os::DesktopError::DownloadOpenFailed => app::DesktopError::UnityHubDownloadOpenFailed,
        os::DesktopError::WindowNotFound => app::DesktopError::UnityWindowNotFound,
        os::DesktopError::FocusRejected => app::DesktopError::UnityWindowFocusRejected,
        os::DesktopError::FocusFailed => app::DesktopError::UnityWindowFocusFailed,
    }
}

fn parse(method: &str, value: serde_json::Value) -> Option<(app::DesktopAction, String)> {
    match method {
        rpc::METHOD_DESKTOP_FOCUS_PROJECT_UNITY => {
            let p: rpc::DesktopFocusProjectUnityParams = serde_json::from_value(value).ok()?;
            Some((
                app::DesktopAction::Focus {
                    project_id: app::ProjectId::parse(&p.project_id).ok()?,
                    expected_revision: app::Revision::new(p.expected_revision)?,
                },
                p.idempotency_key,
            ))
        }
        rpc::METHOD_DESKTOP_OPEN_PROJECT_DIRECTORY => {
            let p: rpc::DesktopOpenProjectDirectoryParams = serde_json::from_value(value).ok()?;
            Some((
                app::DesktopAction::Directory {
                    project_id: app::ProjectId::parse(&p.project_id).ok()?,
                },
                p.idempotency_key,
            ))
        }
        rpc::METHOD_DESKTOP_OPEN_UNITY_HUB_DOWNLOAD => {
            let p: rpc::DesktopOpenUnityHubDownloadParams = serde_json::from_value(value).ok()?;
            Some((app::DesktopAction::Download, p.idempotency_key))
        }
        _ => None,
    }
}

pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    apps: &Applications,
    access: &app::AccessContext,
) -> DispatchAction {
    if let Some(error) = require_capability(&request.id, state, rpc::CAPABILITY_DESKTOP_ACTIONS_V1)
    {
        return error;
    }
    if !matches!(
        request.method.as_str(),
        rpc::METHOD_DESKTOP_FOCUS_PROJECT_UNITY
            | rpc::METHOD_DESKTOP_OPEN_PROJECT_DIRECTORY
            | rpc::METHOD_DESKTOP_OPEN_UNITY_HUB_DOWNLOAD
    ) {
        return error_action(Some(request.id), rpc::RpcError::method_not_found(), false);
    }
    let Some((action, key)) = parse(&request.method, request.params) else {
        return error_action(Some(request.id), rpc::RpcError::invalid_request(), false);
    };
    let outcome = match action {
        app::DesktopAction::Focus { .. } => rpc::DesktopActionOutcome::Focused,
        _ => rpc::DesktopActionOutcome::Requested,
    };
    match app::run_desktop_action(access, &apps.store, apps, action, key).await {
        Ok(()) => success_action(request.id, rpc::DesktopActionResult { outcome }, None),
        Err(error) => {
            let code = serde_json::to_value(error).ok();
            error_action(
                Some(request.id),
                rpc::RpcError::desktop(
                    code.as_ref()
                        .and_then(|v| v.as_str())
                        .unwrap_or("store_unavailable"),
                ),
                false,
            )
        }
    }
}
