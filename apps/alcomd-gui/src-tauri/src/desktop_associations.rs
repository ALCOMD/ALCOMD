use serde::Serialize;
use tauri::AppHandle;

use crate::repository_links::{registration_available, registration_suppressed};

const SCHEMES: [&str; 3] = [env!("ALCOMD_URI_SCHEME"), "vcc", "alcom"];
const EXTENSIONS: [(&str, &str); 5] = [
    ("repositoryList", env!("ALCOMD_REPOSITORY_LIST_EXTENSION")),
    ("template", env!("ALCOMD_TEMPLATE_EXTENSION")),
    ("extension", env!("ALCOMD_EXTENSION_EXTENSION")),
    ("alcomRepositoryList", "alcomrepositories"),
    ("alcomTemplate", "alcomtemplate"),
];

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
enum Handler {
    Alcomd,
    Other,
    None,
    Unknown,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Association {
    id: &'static str,
    value: String,
    handler: Handler,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopAssociations {
    protocols: [Association; SCHEMES.len()],
    files: [Association; EXTENSIONS.len()],
    can_register_vcc: bool,
    vcc_registered: Option<bool>,
    can_open_default_apps: bool,
}

#[tauri::command]
pub(crate) async fn gui_desktop_associations(
    app: AppHandle,
) -> Result<DesktopAssociations, &'static str> {
    tauri::async_runtime::spawn_blocking(move || {
        let can_register_vcc = registration_available(&app) && !registration_suppressed();
        DesktopAssociations {
            protocols: SCHEMES.map(|scheme| Association {
                id: scheme,
                value: format!("{scheme}://"),
                handler: platform::handler(scheme, true),
            }),
            files: EXTENSIONS.map(|(id, extension)| Association {
                id,
                value: format!(".{extension}"),
                handler: platform::handler(extension, false),
            }),
            can_register_vcc,
            vcc_registered: platform::vcc_registered(&app),
            can_open_default_apps: cfg!(windows),
        }
    })
    .await
    .map_err(|_| "desktop_associations_unavailable")
}

#[tauri::command]
pub(crate) async fn gui_open_default_apps_settings() -> Result<(), &'static str> {
    if !cfg!(windows) {
        return Err("default_apps_settings_unavailable");
    }
    tauri::async_runtime::spawn_blocking(|| {
        open::that("ms-settings:defaultapps").map_err(|_| "default_apps_settings_unavailable")
    })
    .await
    .map_err(|_| "default_apps_settings_unavailable")?
}

#[cfg(windows)]
mod platform {
    use tauri::AppHandle;
    use tauri_plugin_deep_link::DeepLinkExt;

    use super::Handler;

    pub(super) fn handler(name: &str, protocol: bool) -> Handler {
        let key = if protocol {
            name.to_owned()
        } else {
            format!(".{name}")
        };
        match alcomd_platform::default_association_executable(&key, protocol) {
            Ok(Some(path)) => {
                let Ok(own) = tauri::utils::platform::current_exe() else {
                    return Handler::Unknown;
                };
                let display_path = alcomd_platform::external_filesystem_path(&own);
                // Shell registrations can name external locations. Compare the configured
                // handler path without opening it or contacting a share.
                if [own.as_path(), display_path.as_ref()]
                    .iter()
                    .any(|candidate| path.as_os_str().eq_ignore_ascii_case(candidate.as_os_str()))
                {
                    Handler::Alcomd
                } else {
                    Handler::Other
                }
            }
            Ok(None) => Handler::None,
            Err(_) => Handler::Unknown,
        }
    }

    pub(super) fn vcc_registered(app: &AppHandle) -> Option<bool> {
        app.deep_link().is_registered("vcc").ok()
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use gio::prelude::AppInfoExt;
    use tauri::{AppHandle, Manager};

    use super::Handler;

    const DESKTOP_ID: &str = concat!(env!("ALCOMD_LINUX_DESKTOP_ID"), ".desktop");
    const VCC_DESKTOP_ID: &str = concat!(env!("ALCOMD_LINUX_DESKTOP_ID"), ".vcc.desktop");

    pub(super) fn handler(name: &str, protocol: bool) -> Handler {
        let app = if protocol {
            gio::AppInfo::default_for_uri_scheme(name)
        } else {
            // Query by filename only. An unknown type has no extension-specific handler.
            let (content_type, uncertain) =
                gio::content_type_guess(Some(format!("file.{name}")), &[]);
            if gio::content_type_is_unknown(&content_type) {
                return Handler::None;
            }
            if uncertain {
                return Handler::Unknown;
            }
            gio::AppInfo::default_for_type(&content_type, false)
        };
        match app {
            Some(app) => match app.id().as_deref() {
                Some(id) if id == DESKTOP_ID || (name == "vcc" && id == VCC_DESKTOP_ID) => {
                    Handler::Alcomd
                }
                Some(_) => Handler::Other,
                None => Handler::Unknown,
            },
            None => Handler::None,
        }
    }

    pub(super) fn vcc_registered(app: &AppHandle) -> Option<bool> {
        let image = app.env().appimage?;
        let is_registered = gio::DesktopAppInfo::new(VCC_DESKTOP_ID)
            .is_some_and(|entry| entry.executable() == image);
        Some(is_registered)
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use objc2::rc::Retained;
    use objc2::sel;
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSBundle, NSObjectProtocol, NSString, NSURL};
    use objc2_uniform_type_identifiers::UTType;
    use tauri::AppHandle;

    use super::Handler;

    pub(super) fn handler(name: &str, protocol: bool) -> Handler {
        objc2::rc::autoreleasepool(|_| {
            let workspace = NSWorkspace::sharedWorkspace();
            let url = if protocol {
                let Some(address) = scheme_url(name) else {
                    return Handler::Unknown;
                };
                workspace.URLForApplicationToOpenURL(&address)
            } else {
                let Some(content_type) =
                    UTType::typeWithFilenameExtension(&NSString::from_str(name))
                else {
                    return Handler::None;
                };
                if content_type.isDynamic() {
                    return Handler::None;
                }
                file_type_handler(&workspace, &content_type)
            };
            let Some(url) = url else { return Handler::None };
            if url == NSBundle::mainBundle().bundleURL() {
                Handler::Alcomd
            } else {
                Handler::Other
            }
        })
    }

    fn file_type_handler(
        workspace: &NSWorkspace,
        content_type: &UTType,
    ) -> Option<Retained<NSURL>> {
        if workspace.respondsToSelector(sel!(URLForApplicationToOpenContentType:)) {
            return workspace.URLForApplicationToOpenContentType(content_type);
        }
        alcomd_platform::desktop::macos_application_for_content_type(&content_type.identifier())
    }

    fn scheme_url(name: &str) -> Option<Retained<NSURL>> {
        NSURL::URLWithString(&NSString::from_str(&format!("{name}://vpm/addRepo")))
    }

    pub(super) fn vcc_registered(_: &AppHandle) -> Option<bool> {
        objc2::rc::autoreleasepool(|_| {
            let workspace = NSWorkspace::sharedWorkspace();
            if !workspace.respondsToSelector(sel!(URLsForApplicationsToOpenURL:)) {
                return None;
            }
            let url = scheme_url("vcc")?;
            let own = NSBundle::mainBundle().bundleURL();
            let apps = workspace.URLsForApplicationsToOpenURL(&url);
            Some(apps.iter().any(|app| app == own))
        })
    }
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod platform {
    use super::{AppHandle, Handler};

    pub(super) fn handler(_: &str, _: bool) -> Handler {
        Handler::Unknown
    }

    pub(super) fn vcc_registered(_: &AppHandle) -> Option<bool> {
        None
    }
}
