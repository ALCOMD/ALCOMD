use alcomd_client::{AlcomdClient, ClientConfig, ClientError};
use alcomd_protocol::{
    ExtensionResult, ExtensionUiCloseParams, ExtensionUiCloseResult, ExtensionUiDispatchParams,
    ExtensionUiDispatchResult, ExtensionUiOpenParams, ExtensionUiOpenResult,
    ExtensionUiRefreshParams, ExtensionUiSnapshotResult, RpcError,
};
use tauri::State;
use tauri::async_runtime::Mutex;
use tauri_plugin_dialog::DialogExt;

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
async fn gui_operations_list(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::OperationsListParams,
) -> Result<alcomd_protocol::OperationsListResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.operations_list(params.cursor, params.limit).await,
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
    state: State<'_, GuiClientState>,
) -> Result<alcomd_protocol::SettingsGetResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.settings_get().await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_settings_update(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::SettingsUpdateParams,
) -> Result<alcomd_protocol::SettingsGetResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.settings_update(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_activity_list(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ActivityListParams,
) -> Result<alcomd_protocol::ActivityListResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.activity_list(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_diagnostics_list(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::DiagnosticsListParams,
) -> Result<alcomd_protocol::DiagnosticsListResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.diagnostics_list(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
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
    params: alcomd_protocol::RegistryListParams,
) -> Result<alcomd_protocol::ProjectsListResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.projects_list(params.cursor, params.limit).await,
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
    open::that(root).map_err(|_| RpcError::extension("project_directory_open_failed"))
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
        Some(client) => client.repository_inspect(params.source).await,
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
async fn gui_repository_register(
    state: State<'_, GuiClientState>,
    source: alcomd_protocol::RepositorySource,
    idempotency_key: String,
) -> Result<alcomd_protocol::RepositoryWriteResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.repository_register(source, idempotency_key).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_repository_refresh(
    state: State<'_, GuiClientState>,
    repository_id: String,
    expected_revision: u64,
    idempotency_key: String,
) -> Result<alcomd_protocol::RepositoryWriteResult, RpcError> {
    let mut client = state.client.lock().await;
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
async fn gui_unity_project_editor_get(
    state: State<'_, GuiClientState>,
    project_id: String,
) -> Result<alcomd_protocol::ProjectEditorResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.unity_project_editor_get(project_id).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_unity_project_editor_set(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ProjectEditorSetParams,
) -> Result<alcomd_protocol::ProjectEditorResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.unity_project_editor_set(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_unity_project_editor_selection_get(
    state: State<'_, GuiClientState>,
    project_id: String,
) -> Result<alcomd_protocol::ProjectEditorSelectionResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.project_editor_selection_get(project_id).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_unity_project_editor_clear(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::ProjectEditorClearParams,
) -> Result<alcomd_protocol::ProjectEditorClearResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.project_editor_clear(params).await,
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
    params: alcomd_protocol::UnityLaunchParams,
) -> Result<alcomd_protocol::UnityLaunchResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.unity_launch(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_unity_launch_status(
    state: State<'_, GuiClientState>,
    launch_id: String,
) -> Result<alcomd_protocol::UnityLaunchResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.unity_launch_status(launch_id).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_templates_list(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::TemplatesListParams,
) -> Result<alcomd_protocol::TemplatesListResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.templates_list(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_template_get(
    state: State<'_, GuiClientState>,
    template_id: String,
) -> Result<alcomd_protocol::TemplateRecordResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.template_get(template_id).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_template_inspect_bundle(
    state: State<'_, GuiClientState>,
    bundle_path: String,
) -> Result<alcomd_protocol::TemplateBundleInspection, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.template_inspect_bundle(bundle_path).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_template_plan_import(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::TemplatePlanImportParams,
) -> Result<alcomd_protocol::TemplatePlan, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.template_plan_import(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_template_apply_import(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::TemplateApplyPlanParams,
) -> Result<alcomd_protocol::TemplateApplyResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.template_apply_import(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_template_plan_derive(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::TemplatePlanDeriveParams,
) -> Result<alcomd_protocol::TemplatePlan, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.template_plan_derive(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_template_apply_derive(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::TemplateApplyPlanParams,
) -> Result<alcomd_protocol::TemplateApplyResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.template_apply_derive(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_template_export(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::TemplateExportParams,
) -> Result<alcomd_protocol::TemplateExportResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.template_export(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_template_set_favorite(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::TemplateSetFavoriteParams,
) -> Result<alcomd_protocol::TemplateRecordResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.template_set_favorite(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_template_remove(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::TemplateRemoveParams,
) -> Result<alcomd_protocol::TemplateRemoveResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.template_remove(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_template_plan_create_project(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::TemplatePlanCreateProjectParams,
) -> Result<alcomd_protocol::TemplatePlan, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.template_plan_create_project(params).await,
        None => return Err(daemon_unavailable()),
    };
    finish_call(&mut client, result)
}

#[tauri::command]
async fn gui_template_apply_create_project(
    state: State<'_, GuiClientState>,
    params: alcomd_protocol::TemplateApplyPlanParams,
) -> Result<alcomd_protocol::TemplateApplyResult, RpcError> {
    let mut client = state.client.lock().await;
    connect_if_needed(&mut client).await?;
    let result = match client.as_mut() {
        Some(client) => client.template_apply_create_project(params).await,
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
            AlcomdClient::connect(ClientConfig::default())
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
        ClientError::Transport(_)
        | ClientError::StartDaemon(_)
        | ClientError::DaemonPathUnavailable
        | ClientError::StartTimeout
        | ClientError::InvalidResponse => daemon_unavailable(),
    }
}

fn daemon_unavailable() -> RpcError {
    RpcError::extension("daemon_unavailable")
}

/// Starts the official ALCOMD GUI shell.
///
/// Business logic must remain in `alcomd`; this process is only a client and UI host.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(GuiClientState::default())
        .invoke_handler(tauri::generate_handler![
            gui_system_status,
            gui_state_check,
            gui_operations_list,
            gui_operation_get,
            gui_operation_cancel,
            gui_settings_get,
            gui_settings_update,
            gui_activity_list,
            gui_diagnostics_list,
            gui_projects_inspect,
            gui_projects_list,
            gui_project_get,
            gui_open_project_directory,
            gui_select_directory,
            gui_project_plan_copy,
            gui_project_apply_copy,
            gui_project_register,
            gui_project_refresh,
            gui_project_set_favorite,
            gui_project_unregister,
            gui_repositories_inspect,
            gui_repositories_list,
            gui_repository_get,
            gui_repository_packages,
            gui_repository_register,
            gui_repository_refresh,
            gui_repository_unregister,
            gui_package_plan_install,
            gui_package_plan_remove,
            gui_package_plan_upgrade,
            gui_package_plan_downgrade,
            gui_package_plan_resolve,
            gui_package_apply_plan,
            gui_unity_installations_list,
            gui_unity_installation_get,
            gui_unity_installation_register,
            gui_unity_installation_remove,
            gui_unity_installations_refresh,
            gui_unity_project_editor_get,
            gui_unity_project_editor_set,
            gui_unity_project_editor_selection_get,
            gui_unity_project_editor_clear,
            gui_unity_writer_state,
            gui_unity_launch,
            gui_unity_launch_status,
            gui_templates_list,
            gui_template_get,
            gui_template_inspect_bundle,
            gui_template_plan_import,
            gui_template_apply_import,
            gui_template_plan_derive,
            gui_template_apply_derive,
            gui_template_export,
            gui_template_set_favorite,
            gui_template_remove,
            gui_template_plan_create_project,
            gui_template_apply_create_project,
            gui_backups_list,
            gui_backup_get,
            gui_backup_create,
            gui_backup_plan_restore,
            gui_backup_apply_restore,
            gui_extensions_list,
            gui_extension_get,
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
        .run(tauri::generate_context!())
        .expect("failed to run alcomd-gui");
}

#[cfg(test)]
mod tests {
    use super::validated_registered_project_root;
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
}
