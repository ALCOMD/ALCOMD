use alcomd_client::{AlcomdClient, ClientConfig, ClientError};
use alcomd_protocol::{
    ExtensionResult, ExtensionUiCloseParams, ExtensionUiCloseResult, ExtensionUiDispatchParams,
    ExtensionUiDispatchResult, ExtensionUiOpenParams, ExtensionUiOpenResult,
    ExtensionUiRefreshParams, ExtensionUiSnapshotResult, RpcError,
};
use tauri::async_runtime::Mutex;
use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;

mod events;
mod repository_links;
mod unity_windows;
mod window_state;

#[derive(Default)]
struct GuiClientState {
    client: Mutex<Option<AlcomdClient>>,
}

#[tauri::command]
async fn gui_system_status(
    state: State<'_, GuiClientState>,
) -> Result<alcomd_protocol::SystemStatusResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.system_status().await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_state_check(
    state: State<'_, GuiClientState>,
    idempotency_key: String,
) -> Result<alcomd_protocol::OperationAccepted, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.state_check(idempotency_key).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_logs_read(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::LogsReadParams,
) -> Result<alcomd_protocol::LogsReadResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.logs_read(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_operations_list(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::OperationsListParams,
) -> Result<alcomd_protocol::OperationsListResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => {
            client
                .operations_list(params.cursor, params.limit, params.active_only)
                .await
        }
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_operation_get(
    state: State<'_, GuiClientState>,
    operation_id: String,
) -> Result<alcomd_protocol::Operation, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.operation_get(operation_id).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_operation_cancel(
    state: State<'_, GuiClientState>,
    operation_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<alcomd_protocol::OperationWriteResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => {
            client
                .operation_cancel(operation_id, expected_revision, idempotency_key)
                .await
        }
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_settings_get(
    app: tauri::AppHandle,
    state: State<'_, GuiClientState>,
) -> Result<alcomd_protocol::SettingsGetResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.settings_get().await,
        None => return Err(daemon_unavailable()),
    };
    let settings = finish_call(&mut client, result)?;
    repository_links::accept_registration_preference(
        &app,
        settings.settings.integrations.handle_vcc_links,
    );
    Ok(settings)
}

#[tauri::command]
async fn gui_settings_update(
    app: tauri::AppHandle,
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::SettingsUpdateParams,
) -> Result<alcomd_protocol::SettingsGetResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.settings_update(params).await,
        None => return Err(daemon_unavailable()),
    };
    let settings = finish_call(&mut client, result)?;
    repository_links::accept_registration_preference(
        &app,
        settings.settings.integrations.handle_vcc_links,
    );
    Ok(settings)
}

#[tauri::command]
async fn gui_projects_inspect(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ProjectsInspectParams,
) -> Result<alcomd_protocol::ProjectResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => {
            client
                .project_inspect(params.path, params.discovery_mode)
                .await
        }
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_projects_list(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ProjectsListParams,
) -> Result<alcomd_protocol::ProjectsListResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.projects_list_with_view(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_project_get(
    state: State<'_, GuiClientState>,
    project_id: String,
) -> Result<alcomd_protocol::ProjectResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.project_get(project_id).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_open_project_directory(
    state: State<'_, GuiClientState>,
    project_id: String,
) -> Result<(), RpcError> {
    let project = {
        let mut client = state.client.lock().await;
        connect_if_needed(&mut client).await?;
        let result = match client.as_mut() {
            Some(client) => client.project_get(project_id).await,
            None => return Err(daemon_unavailable()),
        };
        finish_call(&mut client, result)?.project
    };

    let root = validated_registered_project_root(&project.root_path)?;
    let path = alcomd_platform::external_filesystem_path(&root);
    open::that(path.as_os_str()).map_err(|_| RpcError::extension("project_directory_open_failed"))
}

/// Closed UI adapter: no caller-supplied URL, executable path or arguments.
#[tauri::command]
async fn gui_open_unity_hub_download() -> Result<(), RpcError> {
    open::that("https://unity.com/download")
        .map_err(|_| RpcError::extension("unity_hub_download_open_failed"))
}

#[tauri::command]
async fn gui_unity_hub_open(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::UnityHubOpenParams,
) -> Result<alcomd_protocol::UnityHubOpenResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.unity_hub_open(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

/// Native selection only. Core validates and persists the returned path.
#[tauri::command]
async fn gui_select_unity_hub(
    app: tauri::AppHandle,
    current_path: Option<String>,
) -> Result<Option<String>, RpcError> {
    let mut picker = app.dialog().file();
    if let Some(path) = current_path {
        let path = std::path::PathBuf::from(path);
        #[cfg(target_os = "macos")]
        let path = if path.ends_with("Contents/MacOS/Unity Hub") {
            path.ancestors().nth(3).unwrap_or(&path).to_path_buf()
        } else {
            path
        };
        if let Some(parent) = path.parent() {
            picker =
                picker.set_directory(alcomd_platform::external_filesystem_path(parent).as_ref());
        }
        if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
            picker = picker.set_file_name(name);
        }
    }
    #[cfg(windows)]
    {
        picker = picker.add_filter("Executable", &["exe"]);
    }
    #[cfg(target_os = "macos")]
    {
        picker = picker.add_filter("Application", &["app"]);
    }
    picker
        .blocking_pick_file()
        .map(|selected| {
            selected
                .into_path()
                .ok()
                .and_then(|path| path.into_os_string().into_string().ok())
                .ok_or_else(|| RpcError::extension("file_selection_failed"))
        })
        .transpose()
}

#[tauri::command]
async fn gui_select_directory(app: tauri::AppHandle) -> Result<Option<String>, RpcError> {
    let selected = app.dialog().file().blocking_pick_folder();
    selected
        .map(|path| {
            let path = path
                .into_path()
                .map_err(|_| RpcError::extension("directory_selection_failed"))?;
            let canonical = std::fs::canonicalize(path)
                .map_err(|_| RpcError::extension("directory_selection_failed"))?;
            if !canonical.is_absolute()
                || !std::fs::metadata(&canonical)
                    .map(|metadata| metadata.is_dir())
                    .unwrap_or(false)
            {
                return Err(RpcError::extension("directory_selection_failed"));
            }
            canonical
                .into_os_string()
                .into_string()
                .map_err(|_| RpcError::extension("directory_selection_failed"))
        })
        .transpose()
}

#[tauri::command]
async fn gui_select_repository_list_file(
    app: tauri::AppHandle,
    mode: String,
    file_name: Option<String>,
) -> Result<Option<String>, RpcError> {
    select_local_file(
        app,
        &mode,
        file_name.as_deref(),
        "Repository list (*.txt)",
        "txt",
    )
}

#[tauri::command]
async fn gui_select_project_template_file(
    app: tauri::AppHandle,
    mode: String,
    file_name: Option<String>,
) -> Result<Option<String>, RpcError> {
    select_local_file(
        app,
        &mode,
        file_name.as_deref(),
        "ALCOMD template (*.alcomdtemplate)",
        "alcomdtemplate",
    )
}

fn select_local_file(
    app: tauri::AppHandle,
    mode: &str,
    file_name: Option<&str>,
    filter: &str,
    extension: &str,
) -> Result<Option<String>, RpcError> {
    // The existing save dialog cannot suppress its overwrite promise. Pick a parent
    // folder instead; the GUI supplies the reviewed new name and Core creates the file.
    let selected = match mode {
        "open" => app
            .dialog()
            .file()
            .add_filter(filter, &[extension])
            .blocking_pick_file(),
        "save" => app.dialog().file().blocking_pick_folder(),
        _ => return Err(RpcError::invalid_request()),
    };
    selected
        .map(|path| {
            path.into_path()
                .ok()
                .and_then(|path| {
                    if mode == "save" {
                        let name = file_name?;
                        if name.is_empty()
                            || name.chars().count() > 255
                            || name == "."
                            || name == ".."
                            || name.contains(['/', '\\', ':'])
                        {
                            return None;
                        }
                        path.join(name).into_os_string().into_string().ok()
                    } else {
                        path.into_os_string().into_string().ok()
                    }
                })
                .ok_or_else(|| RpcError::extension("file_selection_failed"))
        })
        .transpose()
}

#[tauri::command]
async fn gui_repository_list_inspect_file(
    state: State<'_, GuiClientState>,
    path: String,
) -> Result<alcomd_protocol::RepositoryListInspection, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.repository_list_inspect_file(path).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_repository_list_import_file(
    params: alcomd_protocol::RepositoryListImportParams,
) -> Result<alcomd_protocol::RepositoryListImportResult, RpcError> {
    let mut client = None;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.repository_list_import_file(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_repository_list_cancel_registration(
    params: alcomd_protocol::RepositoryListCancelParams,
) -> Result<alcomd_protocol::RepositoryListCancelResult, RpcError> {
    let mut client = None;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.repository_list_cancel_registration(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_repository_list_preview_export(
    state: State<'_, GuiClientState>,
) -> Result<alcomd_protocol::RepositoryListPreview, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.repository_list_preview_export().await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_repository_list_export_file(
    state: State<'_, GuiClientState>,
    target_path: String,
    expected_fingerprint: String,
) -> Result<alcomd_protocol::RepositoryListExportResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => {
            client
                .repository_list_export_file(target_path, expected_fingerprint)
                .await
        }
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_project_plan_copy(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ProjectsPlanCopyParams,
) -> Result<alcomd_protocol::ProjectsPlanCopyResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.project_plan_copy(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_project_apply_copy(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ProjectsApplyCopyParams,
) -> Result<alcomd_protocol::ProjectsApplyCopyResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.project_apply_copy(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_project_plan_delete_directory(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ProjectsPlanDeleteDirectoryParams,
) -> Result<alcomd_protocol::ProjectsPlanDeleteDirectoryResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.project_plan_delete_directory(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_project_apply_delete_directory(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ProjectsApplyDeleteDirectoryParams,
) -> Result<alcomd_protocol::ProjectsApplyDeleteDirectoryResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.project_apply_delete_directory(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_project_plan_unity_migration(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ProjectsPlanUnityMigrationParams,
) -> Result<alcomd_protocol::ProjectsPlanUnityMigrationResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.project_plan_unity_migration(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_project_apply_unity_migration(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ProjectsApplyUnityMigrationParams,
) -> Result<alcomd_protocol::ProjectsApplyUnityMigrationResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.project_apply_unity_migration(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

fn validated_registered_project_root(root_path: &str) -> Result<std::path::PathBuf, RpcError> {
    let registered = std::path::Path::new(root_path);
    if !registered.is_absolute() {
        return Err(RpcError::extension("project_directory_missing"));
    }
    let metadata = std::fs::metadata(registered).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            RpcError::extension("project_directory_missing")
        } else {
            RpcError::extension("project_directory_open_failed")
        }
    })?;
    if !metadata.is_dir() {
        return Err(RpcError::extension("project_directory_not_directory"));
    }
    std::fs::canonicalize(registered)
        .map_err(|_| RpcError::extension("project_directory_open_failed"))
}

#[tauri::command]
async fn gui_project_register(
    state: State<'_, GuiClientState>,
    path: String,
    idempotency_key: String,
) -> Result<alcomd_protocol::ProjectWriteResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.project_register(path, idempotency_key).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_project_refresh(
    state: State<'_, GuiClientState>,
    project_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<alcomd_protocol::ProjectWriteResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => {
            client
                .project_refresh(project_id, expected_revision, idempotency_key)
                .await
        }
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_project_set_favorite(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ProjectSetFavoriteParams,
) -> Result<alcomd_protocol::ProjectWriteResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.project_set_favorite(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_project_unregister(
    state: State<'_, GuiClientState>,
    project_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<alcomd_protocol::ProjectUnregisterResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => {
            client
                .project_unregister(project_id, expected_revision, idempotency_key)
                .await
        }
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_repositories_inspect(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::RepositoryInspectParams,
) -> Result<alcomd_protocol::RepositoryResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => {
            client
                .repository_inspect_with_credentials(params.source, params.credentials)
                .await
        }
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_repositories_list(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::RegistryListParams,
) -> Result<alcomd_protocol::RepositoriesListResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.repositories_list(params.cursor, params.limit).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_repository_get(
    state: State<'_, GuiClientState>,
    repository_id: String,
) -> Result<alcomd_protocol::RepositoryResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.repository_get(repository_id).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_repository_packages(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::RepositoryPackagesParams,
) -> Result<alcomd_protocol::RepositoryPackagesResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => {
            client
                .repository_packages(params.repository_id, params.cursor, params.limit)
                .await
        }
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
fn gui_open_package_link(url: String) -> Result<(), RpcError> {
    validate_package_link_expectation(&url)?;
    open::that(url).map_err(|_| RpcError::extension("package_link_open_failed"))
}

fn validate_package_link_expectation(url: &str) -> Result<(), RpcError> {
    let remainder = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .ok_or_else(|| RpcError::extension("package_link_invalid"))?;
    let authority = remainder.split(['/', '?', '#']).next().unwrap_or_default();
    if url.len() > 2_048
        || authority.is_empty()
        || authority.contains(['@', '\\'])
        || url
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        return Err(RpcError::extension("package_link_invalid"));
    }
    Ok(())
}

#[tauri::command]
async fn gui_repository_register(
    state: State<'_, GuiClientState>,
    source: alcomd_protocol::RepositorySource,
    idempotency_key: String,
    credentials: Option<alcomd_protocol::RepositoryCredentials>,
) -> Result<alcomd_protocol::RepositoryWriteResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => {
            client
                .repository_register_with_credentials(source, idempotency_key, credentials)
                .await
        }
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_repository_set_credentials(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::RepositorySetCredentialsParams,
) -> Result<alcomd_protocol::RepositoryWriteResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.repository_set_credentials(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_repository_set_display_name(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::RepositorySetDisplayNameParams,
) -> Result<alcomd_protocol::RepositoryWriteResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.repository_set_display_name(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_repository_refresh(
    repository_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<alcomd_protocol::RepositoryWriteResult, RpcError> {
    let mut client = None;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => {
            client
                .repository_refresh(repository_id, expected_revision, idempotency_key)
                .await
        }
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_repositories_refresh_all(
    params: alcomd_protocol::RepositoryRefreshAllParams,
) -> Result<alcomd_protocol::RepositoryRefreshAllResult, RpcError> {
    let mut client = None;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.repositories_refresh_all(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_repository_unregister(
    state: State<'_, GuiClientState>,
    repository_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<alcomd_protocol::RepositoryUnregisterResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => {
            client
                .repository_unregister(repository_id, expected_revision, idempotency_key)
                .await
        }
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_package_plan_install(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::PackagePlanInstallParams,
) -> Result<alcomd_protocol::PackagePlan, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.package_plan_install(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_package_plan_remove(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::PackagePlanRemoveParams,
) -> Result<alcomd_protocol::PackagePlan, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.package_plan_remove(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_package_plan_upgrade(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::PackagePlanUpgradeParams,
) -> Result<alcomd_protocol::PackagePlan, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.package_plan_upgrade(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_package_plan_downgrade(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::PackagePlanDowngradeParams,
) -> Result<alcomd_protocol::PackagePlan, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.package_plan_downgrade(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_package_plan_resolve(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::PackagePlanResolveParams,
) -> Result<alcomd_protocol::PackagePlan, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.package_plan_resolve(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_package_plan_reinstall(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::PackagePlanReinstallParams,
) -> Result<alcomd_protocol::PackagePlan, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.package_plan_reinstall(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_package_query_project_candidates(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ProjectCandidateRequest,
) -> Result<alcomd_protocol::ProjectCandidatePage, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.package_query_project_candidates(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_package_plan_bulk(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::PackagePlanBulkParams,
) -> Result<alcomd_protocol::PackagePlan, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.package_plan_bulk(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_user_packages_list(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::UserPackagesListParams,
) -> Result<alcomd_protocol::UserPackagesListResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.user_packages_list(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_user_package_get(
    state: State<'_, GuiClientState>,
    user_package_id: String,
) -> Result<alcomd_protocol::UserPackageResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => {
            client
                .user_package_get(alcomd_protocol::UserPackageGetParams { user_package_id })
                .await
        }
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_user_package_enroll(
    state: State<'_, GuiClientState>,
    source_path: String,
    idempotency_key: String,
) -> Result<alcomd_protocol::UserPackageWriteResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => {
            client
                .user_package_enroll(alcomd_protocol::UserPackageEnrollParams {
                    source_path,
                    idempotency_key,
                })
                .await
        }
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_user_package_refresh(
    state: State<'_, GuiClientState>,
    user_package_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<alcomd_protocol::UserPackageWriteResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => {
            client
                .user_package_refresh(alcomd_protocol::UserPackageMutationParams {
                    user_package_id,
                    expected_revision,
                    idempotency_key,
                })
                .await
        }
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_user_package_remove(
    state: State<'_, GuiClientState>,
    user_package_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<alcomd_protocol::UserPackageRemoveResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => {
            client
                .user_package_remove(alcomd_protocol::UserPackageMutationParams {
                    user_package_id,
                    expected_revision,
                    idempotency_key,
                })
                .await
        }
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_package_apply_plan(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::PackageApplyPlanParams,
) -> Result<alcomd_protocol::PackageApplyPlanResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.package_apply_plan(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_unity_installations_list(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::UnityInstallationsListParams,
) -> Result<alcomd_protocol::UnityInstallationsListResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.unity_installations_list(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_unity_installation_get(
    state: State<'_, GuiClientState>,
    installation_id: String,
) -> Result<alcomd_protocol::UnityInstallationResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.unity_installation_get(installation_id).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_unity_installation_register(
    state: State<'_, GuiClientState>,
    executable_path: String,
    idempotency_key: String,
) -> Result<alcomd_protocol::UnityInstallationResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => {
            client
                .unity_installation_register(alcomd_protocol::UnityInstallationRegisterParams {
                    executable_path,
                    idempotency_key,
                })
                .await
        }
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_unity_installation_remove(
    state: State<'_, GuiClientState>,
    installation_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<alcomd_protocol::UnityInstallationRemoveResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => {
            client
                .unity_installation_remove(alcomd_protocol::UnityInstallationRemoveParams {
                    installation_id,
                    expected_revision,
                    idempotency_key,
                })
                .await
        }
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_unity_installations_refresh(
    state: State<'_, GuiClientState>,
    idempotency_key: String,
) -> Result<alcomd_protocol::UnityInstallationsListResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.unity_installations_refresh(idempotency_key).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_unity_project_launch_config_get(
    state: State<'_, GuiClientState>,
    project_id: String,
) -> Result<alcomd_protocol::ProjectUnityLaunchConfigResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.unity_project_launch_config_get(project_id).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_unity_project_launch_config_set(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ProjectUnityLaunchConfigSetParams,
) -> Result<alcomd_protocol::ProjectUnityLaunchConfigMutationResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.unity_project_launch_config_set(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_unity_project_launch_config_clear(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ProjectUnityLaunchConfigClearParams,
) -> Result<alcomd_protocol::ProjectUnityLaunchConfigMutationResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.unity_project_launch_config_clear(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_unity_launch_options(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::UnityLaunchOptionsParams,
) -> Result<alcomd_protocol::UnityLaunchOptionsResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.unity_launch_options(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_unity_writer_state(
    state: State<'_, GuiClientState>,
    project_id: String,
) -> Result<alcomd_protocol::UnityWriterState, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.unity_writer_state(project_id).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_unity_launch(
    state: State<'_, GuiClientState>,
    windows: State<'_, unity_windows::UnityWindows>,
    params: alcomd_protocol::UnityLaunchParams,
) -> Result<alcomd_protocol::UnityLaunchResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.unity_launch(params).await,
        None => return Err(daemon_unavailable()),
    };
    let result = finish_call(&mut client, result);
    windows.refresh();
    result
}

#[tauri::command]
async fn gui_projects_create(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ProjectsCreateParams,
) -> Result<alcomd_protocol::ProjectsCreateResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.projects_create(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_project_templates_list(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ProjectTemplatesListParams,
) -> Result<alcomd_protocol::ProjectTemplatesListResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.project_templates_list(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_project_templates_save(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ProjectTemplatesSaveParams,
) -> Result<alcomd_protocol::ProjectTemplatesSaveResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.project_templates_save(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_project_templates_export(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ProjectTemplatesExportParams,
) -> Result<alcomd_protocol::ProjectTemplatesSaveResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.project_templates_export(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_project_templates_remove(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ProjectTemplatesRemoveParams,
) -> Result<alcomd_protocol::ProjectTemplatesRemoveResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.project_templates_remove(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_backups_list(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::BackupsListParams,
) -> Result<alcomd_protocol::BackupsListResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.backups_list(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_backup_get(
    state: State<'_, GuiClientState>,
    backup_id: String,
) -> Result<alcomd_protocol::BackupRecord, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.backup_get(backup_id).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_backup_create(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::BackupCreateParams,
) -> Result<alcomd_protocol::BackupCreateResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.backup_create(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_backup_plan_restore(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::BackupPlanRestoreParams,
) -> Result<alcomd_protocol::BackupRestorePlan, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.backup_plan_restore(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_backup_apply_restore(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::BackupApplyRestoreParams,
) -> Result<alcomd_protocol::BackupApplyRestoreResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.backup_apply_restore(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_extensions_list(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ExtensionsListParams,
) -> Result<alcomd_protocol::ExtensionsListResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.extensions_list(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_extension_get(
    state: State<'_, GuiClientState>,
    extension_id: String,
) -> Result<ExtensionResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.extension_get(extension_id).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_extension_permissions_list(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ExtensionPermissionsListParams,
) -> Result<alcomd_protocol::ExtensionPermissionsListResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.extension_permissions_list(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_extension_plan_install(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ExtensionPlanInstallParams,
) -> Result<alcomd_protocol::ExtensionPlanResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.extension_plan_install(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_extension_apply_install(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ExtensionApplyParams,
) -> Result<alcomd_protocol::ExtensionOperationResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.extension_apply_install(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_extension_enable(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ExtensionLifecycleParams,
) -> Result<ExtensionResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.extension_enable(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_extension_disable(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ExtensionLifecycleParams,
) -> Result<ExtensionResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.extension_disable(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_extension_plan_uninstall(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ExtensionPlanUninstallParams,
) -> Result<alcomd_protocol::ExtensionPlanResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.extension_plan_uninstall(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_extension_apply_uninstall(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ExtensionApplyParams,
) -> Result<alcomd_protocol::ExtensionOperationResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.extension_apply_uninstall(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_extension_set_grant(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ExtensionGrantParams,
) -> Result<alcomd_protocol::ExtensionGrantResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.extension_set_grant(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_extension_revoke_grant(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ExtensionGrantParams,
) -> Result<alcomd_protocol::ExtensionGrantResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.extension_revoke_grant(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_extension_ui_open(
    state: State<'_, GuiClientState>,
    params: ExtensionUiOpenParams,
) -> Result<ExtensionUiOpenResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.extension_ui_open(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_extension_ui_refresh(
    state: State<'_, GuiClientState>,
    params: ExtensionUiRefreshParams,
) -> Result<ExtensionUiSnapshotResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.extension_ui_refresh(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_extension_ui_dispatch(
    state: State<'_, GuiClientState>,
    params: ExtensionUiDispatchParams,
) -> Result<ExtensionUiDispatchResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.extension_ui_dispatch(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_extension_ui_close(
    state: State<'_, GuiClientState>,
    params: ExtensionUiCloseParams,
) -> Result<ExtensionUiCloseResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.extension_ui_close(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

async fn connect_if_needed(client: &mut Option<AlcomdClient>) -> Result<(), RpcError> {
    if client.is_none() {
        *client = Some(
            AlcomdClient::connect(ClientConfig::default().with_client_name(env!("CARGO_PKG_NAME")))
                .await
                .map_err(client_error)?,
        );
    }
    Ok(())
}

fn finish_call<T>(
    client: &mut Option<AlcomdClient>,
    result: Result<T, ClientError>,
) -> Result<T, RpcError> {
    match result {
        Ok(value) => Ok(value),
        Err(ClientError::Remote(error)) => Err(error),
        Err(error) => {
            *client = None;
            Err(client_error(error))
        }
    }
}

fn client_error(error: ClientError) -> RpcError {
    match error {
        ClientError::Remote(error) => error,
        _ => daemon_unavailable(),
    }
}

fn daemon_unavailable() -> RpcError {
    RpcError::extension("daemon_unavailable")
}

/// Starts the official ALCOMD GUI shell.
///
/// Business logic must remain in `alcomd`; this process is only a client and UI host.
pub fn run() {
    #[cfg(windows)]
    alcomd_platform::set_current_process_app_user_model_id(env!("ALCOMD_WINDOWS_AUMID"))
        .expect("failed to initialize Windows application identity");

    let builder = tauri::Builder::default()
        .manage(repository_links::RepositoryLinks::from_arguments())
        .plugin(tauri_plugin_single_instance::init(|app, arguments, _| {
            repository_links::forward_arguments(app, arguments);
        }));
    #[cfg(any(windows, target_os = "linux"))]
    let builder = builder.plugin(tauri_plugin_deep_link::init());
    builder
        .plugin(tauri_plugin_dialog::init())
        .manage(repository_links::VccRegistration::default())
        .manage(GuiClientState::default())
        .manage(events::EventsClient::default())
        .manage(unity_windows::UnityWindows::default())
        .on_page_load(|view, payload| {
            events::on_page_load(view, payload);
            if view.label() == "main"
                && matches!(payload.event(), tauri::webview::PageLoadEvent::Started)
            {
                view.state::<unity_windows::UnityWindows>().reset();
            }
        })
        .manage(window_state::WindowStateController::default())
        .on_window_event(|window, event| {
            window_state::on_window_event(window, event);
            events::on_window_event(window, event);
            if window.label() == "main" && matches!(event, tauri::WindowEvent::Destroyed) {
                window.state::<unity_windows::UnityWindows>().reset();
            }
        })
        .invoke_handler(tauri::generate_handler![
            repository_links::gui_take_repository_link,
            repository_links::gui_vcc_registration_available,
            repository_links::gui_register_vcc_links,
            window_state::gui_window_state_configure,
            gui_system_status,
            gui_state_check,
            gui_operations_list,
            gui_logs_read,
            gui_operation_get,
            gui_operation_cancel,
            gui_settings_get,
            gui_settings_update,
            events::gui_events_start,
            events::gui_events_ack,
            events::gui_events_cancel,
            gui_projects_inspect,
            gui_projects_list,
            gui_project_get,
            gui_open_project_directory,
            gui_select_directory,
            gui_select_repository_list_file,
            gui_select_project_template_file,
            gui_repository_list_inspect_file,
            gui_repository_list_import_file,
            gui_repository_list_cancel_registration,
            gui_repository_list_preview_export,
            gui_repository_list_export_file,
            gui_select_unity_hub,
            gui_unity_hub_open,
            gui_open_unity_hub_download,
            gui_project_plan_copy,
            gui_project_apply_copy,
            gui_project_plan_delete_directory,
            gui_project_apply_delete_directory,
            gui_project_plan_unity_migration,
            gui_project_apply_unity_migration,
            gui_project_register,
            gui_project_refresh,
            gui_project_set_favorite,
            gui_project_unregister,
            gui_repositories_inspect,
            gui_repositories_list,
            gui_repository_get,
            gui_repository_packages,
            gui_open_package_link,
            gui_repository_register,
            gui_repository_set_credentials,
            gui_repository_set_display_name,
            gui_repository_refresh,
            gui_repositories_refresh_all,
            gui_repository_unregister,
            gui_package_plan_install,
            gui_package_plan_remove,
            gui_package_plan_upgrade,
            gui_package_plan_downgrade,
            gui_package_plan_resolve,
            gui_package_plan_reinstall,
            gui_package_plan_bulk,
            gui_package_query_project_candidates,
            gui_package_apply_plan,
            gui_user_packages_list,
            gui_user_package_get,
            gui_user_package_enroll,
            gui_user_package_refresh,
            gui_user_package_remove,
            gui_unity_installations_list,
            gui_unity_installation_get,
            gui_unity_installation_register,
            gui_unity_installation_remove,
            gui_unity_installations_refresh,
            gui_unity_project_launch_config_get,
            gui_unity_project_launch_config_set,
            gui_unity_project_launch_config_clear,
            gui_unity_writer_state,
            gui_unity_launch_options,
            gui_unity_launch,
            unity_windows::gui_unity_windows_start,
            unity_windows::gui_unity_windows_next,
            unity_windows::gui_unity_windows_stop,
            unity_windows::gui_focus_project_unity,
            gui_projects_create,
            gui_project_templates_list,
            gui_project_templates_save,
            gui_project_templates_export,
            gui_project_templates_remove,
            gui_backups_list,
            gui_backup_get,
            gui_backup_create,
            gui_backup_plan_restore,
            gui_backup_apply_restore,
            gui_extensions_list,
            gui_extension_get,
            gui_extension_permissions_list,
            gui_extension_plan_install,
            gui_extension_apply_install,
            gui_extension_enable,
            gui_extension_disable,
            gui_extension_plan_uninstall,
            gui_extension_apply_uninstall,
            gui_extension_set_grant,
            gui_extension_revoke_grant,
            gui_extension_ui_open,
            gui_extension_ui_refresh,
            gui_extension_ui_dispatch,
            gui_extension_ui_close
        ])
        .build(tauri::generate_context!())
        .expect("failed to build alcomd-gui")
        .run(|_app, _event| {
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Opened { urls } = _event {
                repository_links::forward(_app, urls.into_iter().map(|url| url.to_string()));
            }
        });
}

#[cfg(test)]
mod tests {
    use super::{validate_package_link_expectation, validated_registered_project_root};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_test_path(name: &str) -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        std::env::temp_dir().join(format!("alcomd-gui-{name}-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn registered_project_root_must_exist_and_be_a_directory() {
        let directory = unique_test_path("directory");
        fs::create_dir(&directory).expect("create test directory");
        let resolved = validated_registered_project_root(
            directory.to_str().expect("test path must be Unicode"),
        )
        .expect("existing directory should resolve");
        assert!(resolved.is_absolute());

        let file = directory.join("not-a-directory");
        fs::write(&file, b"fixture").expect("create test file");
        let error =
            validated_registered_project_root(file.to_str().expect("test path must be Unicode"))
                .expect_err("regular file must be rejected");
        assert_eq!(error.code, "project_directory_not_directory");

        fs::remove_file(file).expect("remove test file");
        fs::remove_dir(directory).expect("remove test directory");
    }

    #[test]
    fn missing_and_relative_registered_roots_fail_closed() {
        let missing = unique_test_path("missing");
        let error =
            validated_registered_project_root(missing.to_str().expect("test path must be Unicode"))
                .expect_err("missing directory must fail");
        assert_eq!(error.code, "project_directory_missing");

        let error = validated_registered_project_root("relative-project")
            .expect_err("relative path must fail");
        assert_eq!(error.code, "project_directory_missing");
    }

    #[test]
    fn package_link_expectation_is_closed_to_sanitized_http_urls() {
        assert!(
            validate_package_link_expectation("https://example.invalid/docs?q=1#intro").is_ok()
        );
        assert!(validate_package_link_expectation("http://example.invalid/changelog").is_ok());
        assert!(validate_package_link_expectation("file:///tmp/secret").is_err());
        assert!(validate_package_link_expectation("https://user@example.invalid/docs").is_err());
        assert!(validate_package_link_expectation("https://").is_err());
    }
}
