use std::collections::VecDeque;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Manager, State, Url};

#[cfg(any(target_os = "linux", test))]
#[path = "repository_links/linux.rs"]
mod linux;

const MAX_LINKS: usize = 8;
const MAX_LINK_BYTES: usize = 16 * 1024;

#[derive(Default)]
pub(crate) struct VccRegistration {
    desired: Mutex<Option<bool>>,
    applied: Mutex<Option<bool>>,
}

#[derive(Default)]
pub(crate) struct RepositoryLinks(Mutex<VecDeque<String>>);

impl RepositoryLinks {
    pub(crate) fn from_arguments() -> Self {
        let inbox = Self::default();
        for argument in std::env::args_os()
            .skip(1)
            .filter_map(|argument| argument.into_string().ok())
        {
            inbox.offer(argument);
        }
        inbox
    }

    pub(crate) fn offer(&self, value: String) -> bool {
        if value.len() > MAX_LINK_BYTES
            || value.contains('|')
            || !Url::parse(&value).is_ok_and(|url| {
                url.scheme() == "vcc"
                    && url.host_str() == Some("vpm")
                    && url.path() == "/addRepo"
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.port().is_none()
            })
        {
            return false;
        }
        let Ok(mut links) = self.0.lock() else {
            return false;
        };
        if links.len() == MAX_LINKS || links.contains(&value) {
            return false;
        }
        links.push_back(value);
        true
    }

    pub(crate) fn take(&self) -> Option<String> {
        self.0.lock().ok()?.pop_front()
    }
}

pub(crate) fn forward_arguments(app: &AppHandle, mut arguments: Vec<String>) {
    if arguments.len() == 1 {
        let _ = focus_main(app);
        return;
    }
    // Windows plugin 2.4.x separates forwarded arguments with a pipe. A raw
    // pipe is not valid URI syntax. Reject split activations as a whole.
    if arguments.len() == 2
        && let Some(link) = arguments.pop()
    {
        forward(app, [link]);
    }
}

fn focus_main(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    let window = app.get_webview_window("main")?;
    let _ = window.unminimize();
    let _ = window.set_focus();
    Some(window)
}

pub(crate) fn forward(app: &AppHandle, links: impl IntoIterator<Item = String>) {
    let inbox = app.state::<RepositoryLinks>();
    let mut added = false;
    for link in links {
        added |= inbox.offer(link);
    }
    if added && let Some(window) = focus_main(app) {
        let _ = window.emit_to("main", "alcomd-repository-link-available", ());
    }
}

fn registration_available(app: &AppHandle) -> bool {
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

fn registration_suppressed() -> bool {
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
pub(crate) fn gui_vcc_registration_available(app: AppHandle) -> bool {
    registration_available(&app) && !registration_suppressed()
}

#[tauri::command]
pub(crate) async fn gui_register_vcc_links(app: AppHandle) -> Result<(), &'static str> {
    tauri::async_runtime::spawn_blocking(move || apply_registration(&app, true))
        .await
        .map_err(|_| "vcc_registration_failed")?
}

#[tauri::command]
pub(crate) fn gui_take_repository_link(links: State<'_, RepositoryLinks>) -> Option<String> {
    links.take()
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

    #[test]
    fn inbox_accepts_only_repository_activations_and_drains_once_in_order() {
        let links = RepositoryLinks::default();
        assert!(!links.offer("vcc://other/addRepo?url=https://example.test".into()));
        assert!(!links.offer("vcc://vpm/removeRepo?url=https://example.test".into()));
        assert!(!links.offer("vcc://vpm/addRepo?url=https://example.test|extra".into()));
        let expected = (0..MAX_LINKS)
            .map(|id| format!("vcc://vpm/addRepo?url=https://example.test/{id}"))
            .collect::<Vec<_>>();
        for value in &expected {
            assert!(links.offer(value.clone()));
            assert!(!links.offer(value.clone()));
        }
        assert!(!links.offer("vcc://vpm/addRepo?url=https://example.test/overflow".into()));
        assert_eq!(
            std::iter::from_fn(|| links.take()).collect::<Vec<_>>(),
            expected
        );
        assert!(links.take().is_none());
    }
}
