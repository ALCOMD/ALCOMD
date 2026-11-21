use super::{Registration, UnityApp, UnityAppTarget};
use crate::windows_app_identity::unity_hub_msix_registered;
use crate::windows_url_handler::open_read_only;
use std::io;
use std::path::{Path, PathBuf};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_WOW64_32KEY, KEY_WOW64_64KEY,
};

pub(crate) fn query(app: UnityApp) -> Registration {
    let mut result = Registration::default();
    if app == UnityApp::Hub {
        match unity_hub_msix_registered() {
            Ok(true) => {
                result.targets.push(UnityAppTarget::Msix);
                return result;
            }
            Ok(false) => {}
            Err(_) => result.unavailable = true,
        }
    }
    let exe = match app {
        UnityApp::Hub => "Unity Hub.exe",
        UnityApp::Editor => "Unity.exe",
    };
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
            if registry_view(hive, view, app, exe, &mut result.targets).is_err() {
                result.unavailable = true;
            }
        }
    }
    result
}

fn registry_view(
    hive: HKEY,
    view: u32,
    app: UnityApp,
    exe: &str,
    targets: &mut Vec<UnityAppTarget>,
) -> io::Result<()> {
    let app_path = format!(r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths\{exe}");
    if let Some(key) = open_read_only(hive, &app_path, view)?
        && let Some(path) = key.value("")?.and_then(|value| registered_exe(&value, exe))
    {
        targets.push(UnityAppTarget::Executable(path));
    }
    let Some(uninstall) = open_read_only(
        hive,
        r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        view,
    )?
    else {
        return Ok(());
    };
    for index in 0..4096 {
        let Some(name) = uninstall.child_name(index)? else {
            break;
        };
        let Some(key) = uninstall.child(&name, view)? else {
            continue;
        };
        let Some(display_name) = key.value("DisplayName")? else {
            continue;
        };
        let matches = match app {
            UnityApp::Hub => {
                display_name.eq_ignore_ascii_case("Unity Hub")
                    || display_name
                        .strip_prefix("Unity Hub ")
                        .is_some_and(|suffix| {
                            suffix.starts_with(|character: char| character.is_ascii_digit())
                        })
            }
            UnityApp::Editor => display_name
                .strip_prefix("Unity ")
                .is_some_and(|suffix| suffix.starts_with(|ch: char| ch.is_ascii_digit())),
        };
        if !matches {
            continue;
        }
        if let Some(location) = key.value("InstallLocation")? {
            let root = Path::new(location.trim().trim_matches('"'));
            if root.is_absolute() {
                targets.push(UnityAppTarget::Executable(root.join(exe)));
                if app == UnityApp::Editor {
                    targets.push(UnityAppTarget::Executable(root.join("Editor").join(exe)));
                }
            }
        }
        if let Some(path) = key
            .value("DisplayIcon")?
            .and_then(|value| registered_exe(&value, exe))
        {
            targets.push(UnityAppTarget::Executable(path));
        }
    }
    Ok(())
}

fn registered_exe(value: &str, expected: &str) -> Option<PathBuf> {
    let value = value.trim();
    let value = value
        .rsplit_once(',')
        .filter(|(_, index)| index.trim().parse::<i32>().is_ok())
        .map_or(value, |(path, _)| path.trim())
        .trim_matches('"');
    let path = Path::new(value);
    (path.is_absolute() && path.file_name()?.eq_ignore_ascii_case(expected)).then(|| path.into())
}

pub(crate) fn verify_cli(path: &Path) -> io::Result<()> {
    crate::windows::verify_program_publisher(path, "Unity Technologies SF")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_icon_accepts_only_the_expected_absolute_executable() {
        assert_eq!(
            registered_exe(r#""D:\Unity Builds\Editor\Unity.exe",0"#, "Unity.exe"),
            Some(PathBuf::from(r"D:\Unity Builds\Editor\Unity.exe"))
        );
        for wrong in [
            r"Unity.exe",
            r"D:\Unity\Unity.exe -projectPath abc",
            r"D:\Unity\Uninstall.exe",
        ] {
            assert_eq!(registered_exe(wrong, "Unity.exe"), None);
        }
    }
}
