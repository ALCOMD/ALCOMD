use std::sync::Mutex;

use tauri::{AppHandle, Manager};

#[cfg(any(target_os = "linux", test))]
#[path = "repository_links/linux.rs"]
mod linux;

#[derive(Default)]
pub(crate) struct VccRegistration {
    desired: Mutex<Option<bool>>,
    applied: Mutex<Option<bool>>,
}

pub(crate) fn registration_available(app: &AppHandle) -> bool {
    #[cfg(target_os = "linux")]
    {
        app.env().appimage.is_some()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = app;
        cfg!(windows)
    }
}

fn automatic_registration_allowed(available: bool, development: bool, suppressed: bool) -> bool {
    available && !development && !suppressed
}

pub(crate) fn registration_suppressed() -> bool {
    std::env::var_os("ALCOMD_DISABLE_VCC_REGISTRATION").is_some()
}

pub(crate) fn accept_registration_preference(app: &AppHandle, enabled: bool) {
    let registration = app.state::<VccRegistration>();
    let Ok(mut desired) = registration.desired.lock() else {
        return;
    };
    if desired.replace(enabled) == Some(enabled) {
        return;
    }
    if automatic_registration_allowed(
        registration_available(app),
        cfg!(debug_assertions) || tauri::is_dev(),
        registration_suppressed(),
    ) {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if apply_registration(&app, false).is_err() {
                eprintln!("ALCOMD could not apply its VCC link preference");
            }
        });
    }
}

fn apply_registration(app: &AppHandle, manual: bool) -> Result<(), &'static str> {
    if registration_suppressed() || !registration_available(app) {
        return Err("vcc_registration_unavailable");
    }
    let registration = app.state::<VccRegistration>();
    let mut applied = registration
        .applied
        .lock()
        .map_err(|_| "vcc_registration_failed")?;
    let enabled = registration
        .desired
        .lock()
        .map_err(|_| "vcc_registration_failed")?
        .ok_or("vcc_registration_unavailable")?;
    if manual && !enabled {
        return Err("vcc_registration_unavailable");
    }
    if !manual && *applied == Some(enabled) {
        return Ok(());
    }
    set_registration(app, enabled)?;
    *applied = Some(enabled);
    Ok(())
}

fn set_registration(app: &AppHandle, enabled: bool) -> Result<(), &'static str> {
    #[cfg(windows)]
    {
        use tauri_plugin_deep_link::DeepLinkExt;
        let links = app.deep_link();
        if !enabled {
            let exe =
                tauri::utils::platform::current_exe().map_err(|_| "vcc_registration_failed")?;
            return alcomd_platform::unregister_owned_url_handler(
                "vcc",
                &app.config().identifier,
                &alcomd_platform::external_filesystem_path(&exe),
            )
            .map_err(|_| "vcc_registration_failed");
        }
        if links.is_registered("vcc").unwrap_or(false) {
            return Ok(());
        }
        links.register("vcc").map_err(|_| "vcc_registration_failed")
    }
    #[cfg(target_os = "linux")]
    {
        let image = app.env().appimage.ok_or("vcc_registration_unavailable")?;
        let applications = app
            .path()
            .data_dir()
            .map_err(|_| "vcc_registration_failed")?
            .join("applications");
        let config = app.config();
        let name = config
            .product_name
            .as_deref()
            .unwrap_or(&app.package_info().name);
        linux::set_registration(
            &applications,
            &config.identifier,
            name,
            std::path::Path::new(&image),
            enabled,
        )
        .map_err(|_| "vcc_registration_failed")
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = (app, enabled);
        Err("vcc_registration_unavailable")
    }
}

#[tauri::command]
pub(crate) async fn gui_register_vcc_links(app: AppHandle) -> Result<(), &'static str> {
    tauri::async_runtime::spawn_blocking(move || apply_registration(&app, true))
        .await
        .map_err(|_| "vcc_registration_failed")?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_supported_release_sessions_register_automatically() {
        assert!(automatic_registration_allowed(true, false, false));
        assert!(!automatic_registration_allowed(false, false, false));
        assert!(!automatic_registration_allowed(true, true, false));
        assert!(!automatic_registration_allowed(true, false, true));
    }
}
