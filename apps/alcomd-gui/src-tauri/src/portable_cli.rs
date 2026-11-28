use serde::{Deserialize, Serialize};
use tauri::WebviewWindow;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Action {
    Status,
    Enable,
    Remove,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Status {
    Unsupported,
    Disabled,
    Enabled,
    NeedsRepair,
    Conflict,
}

const UNAVAILABLE: &str = "portable_cli_integration_unavailable";

#[tauri::command]
pub(crate) async fn gui_portable_cli(
    window: WebviewWindow,
    action: Action,
) -> Result<Status, &'static str> {
    if window.label() != "main" {
        return Err(UNAVAILABLE);
    }
    #[cfg(target_os = "linux")]
    {
        tauri::async_runtime::spawn_blocking(move || run(action))
            .await
            .map_err(|_| UNAVAILABLE)?
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = action;
        Ok(Status::Unsupported)
    }
}

#[cfg(target_os = "linux")]
fn run(action: Action) -> Result<Status, &'static str> {
    use std::process::{Command, Stdio};

    let entry = std::env::current_exe().map_err(|_| UNAVAILABLE)?;
    let data = alcomd_platform::DataConfig::default();
    let Some(bootstrap) =
        alcomd_platform::linux_portable_cli_bootstrap(&entry, &data).map_err(|_| UNAVAILABLE)?
    else {
        return Ok(Status::Unsupported);
    };
    let action = match action {
        Action::Status => "status",
        Action::Enable => "enable",
        Action::Remove => "remove",
    };
    let result = Command::new(bootstrap)
        .args(["portable-cli", action])
        .env_remove("ALCOMD_MCP_CLIENT_CREDENTIAL")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .map_err(|_| UNAVAILABLE)?;
    if !result.status.success() || result.stdout.len() > 64 {
        return Err(UNAVAILABLE);
    }
    serde_json::from_slice(&result.stdout).map_err(|_| UNAVAILABLE)
}
