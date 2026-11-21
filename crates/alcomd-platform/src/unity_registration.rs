//! Registration-based discovery for Unity products on the Core user's desktop.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::time::Duration;
use tokio::io::AsyncReadExt as _;

#[cfg(windows)]
#[path = "windows_unity_registration.rs"]
mod windows;

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) enum UnityApp {
    Hub,
    Editor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnityAppTarget {
    Executable(PathBuf),
    #[cfg(windows)]
    Msix,
    #[cfg(target_os = "linux")]
    Flatpak {
        launcher: PathBuf,
        user: bool,
    },
}

#[derive(Default)]
pub(crate) struct Registration {
    pub(crate) targets: Vec<UnityAppTarget>,
    pub(crate) unavailable: bool,
}

pub(crate) fn query(app: UnityApp) -> Registration {
    #[cfg(windows)]
    return windows::query(app);
    #[cfg(target_os = "macos")]
    return query_macos(app);
    #[cfg(target_os = "linux")]
    return query_linux(app);
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        let _ = app;
        Registration::default()
    }
}

#[cfg(windows)]
pub(crate) use windows::verify_cli;

#[cfg(target_os = "macos")]
fn query_macos(app: UnityApp) -> Registration {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSBundle, NSString};

    let identifier = match app {
        UnityApp::Hub => "com.unity3d.unityhub",
        UnityApp::Editor => "com.unity3d.UnityEditor5.x",
    };
    objc2::rc::autoreleasepool(|_| {
        let urls = NSWorkspace::sharedWorkspace()
            .URLsForApplicationsWithBundleIdentifier(&NSString::from_str(identifier));
        let targets = (0..urls.count().min(256))
            .filter_map(|index| {
                let bundle = NSBundle::bundleWithURL(&urls.objectAtIndex(index))?;
                let executable = PathBuf::from(bundle.executableURL()?.path()?.to_string());
                Some(UnityAppTarget::Executable(executable))
            })
            .collect();
        Registration {
            targets,
            unavailable: urls.count() > 256,
        }
    })
}

#[cfg(target_os = "linux")]
pub(crate) const FLATPAK_HUB_ID: &str = "com.unity.UnityHub";

#[cfg(target_os = "linux")]
fn query_linux(app: UnityApp) -> Registration {
    use std::collections::HashSet;
    use std::fs;

    let mut result = Registration::default();
    let user_data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| std::env::home_dir().map(|home| home.join(".local/share")));
    let system_data = std::env::var_os("XDG_DATA_DIRS")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    let data_roots = user_data
        .into_iter()
        .chain(std::env::split_paths(&system_data));
    let mut seen = HashSet::new();
    for root in data_roots.filter(|path| path.is_absolute()).take(16) {
        let directory = root.join("applications");
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => {
                result.unavailable = true;
                continue;
            }
        };
        let mut registrations = Vec::new();
        for (index, entry) in entries.take(4097).enumerate() {
            if index == 4096 {
                result.unavailable = true;
                break;
            }
            match entry {
                Ok(entry) => registrations.push(entry),
                Err(_) => result.unavailable = true,
            }
        }
        registrations.sort_by_key(std::fs::DirEntry::file_name);
        for entry in registrations {
            let path = entry.path();
            if path
                .extension()
                .is_none_or(|extension| extension != "desktop")
                || !seen.insert(entry.file_name())
            {
                continue;
            }
            let text = match read_desktop_entry(&path) {
                Ok(text) => text,
                Err(error) => {
                    if app == UnityApp::Hub
                        && error.kind() != io::ErrorKind::NotFound
                        && path.file_stem().is_some_and(|name| {
                            name == "unityhub" || name == "unity-hub" || name == FLATPAK_HUB_ID
                        })
                    {
                        result.unavailable = true;
                    }
                    continue;
                }
            };
            if let Some(path) = desktop_executable(app, &text) {
                result.targets.push(UnityAppTarget::Executable(path));
            }
        }
    }
    if app == UnityApp::Hub && !has_usable_native_hub(&result) {
        read_linux_package_registrations(&mut result);
    }
    result
}

#[cfg(target_os = "linux")]
fn has_usable_native_hub(result: &Registration) -> bool {
    result.targets.iter().any(|target| match target {
        UnityAppTarget::Executable(path) => path
            .to_str()
            .is_some_and(|path| crate::unity_hub::normalize_unity_hub_path(path).is_ok()),
        UnityAppTarget::Flatpak { .. } => false,
    })
}

#[cfg(target_os = "linux")]
fn read_linux_package_registrations(result: &mut Registration) {
    for (tool, args) in [
        ("dpkg-query", ["-L", "unityhub"]),
        ("rpm", ["-ql", "unityhub"]),
    ] {
        let Some(launcher) = absolute_tool(tool) else {
            continue;
        };
        match bounded_output(&launcher, &args) {
            Ok((status, stdout)) if status.success() => {
                let Ok(paths) = std::str::from_utf8(&stdout) else {
                    result.unavailable = true;
                    continue;
                };
                let paths = paths.lines().map(PathBuf::from).filter(|path| {
                    path.is_absolute()
                        && path.is_file()
                        && path
                            .file_name()
                            .is_some_and(|leaf| leaf == "unityhub" || leaf == "unity-hub")
                });
                result.targets.extend(paths.map(UnityAppTarget::Executable));
            }
            Ok((status, _)) if status.code() == Some(1) => {}
            _ => result.unavailable = true,
        }
    }
    if has_usable_native_hub(result) {
        return;
    }
    let Some(launcher) = absolute_tool("flatpak") else {
        return;
    };
    for (scope, user) in [("--user", true), ("--system", false)] {
        match bounded_output(
            &launcher,
            &["list", scope, "--app", "--columns=application"],
        ) {
            Ok((status, stdout)) if status.success() => {
                if stdout
                    .split(|byte| *byte == b'\n')
                    .any(|line| line == FLATPAK_HUB_ID.as_bytes())
                {
                    result
                        .targets
                        .push(UnityAppTarget::Flatpak { launcher, user });
                    return;
                }
            }
            _ => result.unavailable = true,
        }
    }
}

#[cfg(target_os = "linux")]
fn read_desktop_entry(path: &Path) -> io::Result<String> {
    use std::io::Read as _;
    if !std::fs::metadata(path)?.is_file() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let mut text = String::new();
    std::fs::File::open(path)?
        .take(65_537)
        .read_to_string(&mut text)?;
    if text.len() > 65_536 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(text)
}

#[cfg(target_os = "linux")]
fn desktop_executable(app: UnityApp, text: &str) -> Option<PathBuf> {
    let block = text
        .lines()
        .skip_while(|line| *line != "[Desktop Entry]")
        .skip(1)
        .take_while(|line| !line.starts_with('['));
    let mut name = "";
    let mut exec = "";
    let mut application = false;
    for line in block {
        match line.split_once('=') {
            Some(("Hidden", "true")) => return None,
            Some(("Type", "Application")) => application = true,
            Some(("Name", value)) => name = value,
            Some(("Exec", value)) => exec = value,
            _ => {}
        }
    }
    if !application {
        return None;
    }
    let exec = exec.trim_start();
    let program = if let Some(quoted) = exec.strip_prefix('"') {
        quoted.split_once('"')?.0
    } else {
        exec.split_whitespace().next()?
    };
    let candidate = Path::new(program);
    let leaf = candidate.file_name()?.to_str()?;
    match app {
        UnityApp::Hub if leaf == "unityhub" || leaf == "unity-hub" => {
            if candidate.is_absolute() {
                Some(candidate.into())
            } else if program == leaf {
                absolute_tool(leaf)
            } else {
                None
            }
        }
        UnityApp::Hub
            if name == "Unity Hub"
                && candidate.is_absolute()
                && leaf.to_ascii_lowercase().ends_with(".appimage") =>
        {
            Some(candidate.into())
        }
        UnityApp::Editor if candidate.is_absolute() && leaf == "Unity" => Some(candidate.into()),
        _ => None,
    }
}

/// Hub's installed CLI can be visible before a running Core has a refreshed PATH.
pub(crate) fn unity_cli_candidates(name: &str) -> impl Iterator<Item = PathBuf> {
    use std::io::Read as _;

    #[cfg(windows)]
    let roots = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|base| vec![base.join("UnityHub")]);
    #[cfg(target_os = "macos")]
    let roots =
        std::env::home_dir().map(|home| vec![home.join("Library/Application Support/UnityHub")]);
    #[cfg(target_os = "linux")]
    let roots = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|base| base.is_absolute())
        .or_else(|| std::env::home_dir().map(|home| home.join(".config")))
        .map(|base| vec![base.join("UnityHub"), base.join("unityhub")]);
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    let roots: Option<Vec<PathBuf>> = None;

    let registered = roots.into_iter().flatten().filter_map(move |root| {
        let registration = root.join("cli-install.json");
        if !registration.is_absolute() || !registration.is_file() {
            return None;
        }
        let mut bytes = Vec::new();
        std::fs::File::open(registration)
            .ok()?
            .take(16_385)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() > 16_384 {
            return None;
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
        let path = Path::new(value.get("path")?.as_str()?);
        if !path.is_absolute() || !path.file_name()?.eq_ignore_ascii_case(name) {
            return None;
        }
        let candidate = std::fs::canonicalize(path).ok()?;
        Some(crate::external_filesystem_path(&candidate).into_owned())
    });
    registered.chain(absolute_tools(name))
}

fn absolute_tools(name: &str) -> impl Iterator<Item = PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    // Own the directories independently of the temporary PATH string.
    let directories: Vec<_> = std::env::split_paths(&path)
        .filter(|directory| directory.is_absolute())
        .take(256)
        .collect();
    directories
        .into_iter()
        .map(move |directory| directory.join(name))
        .filter(|executable| executable.is_file())
        .filter_map(|executable| std::fs::canonicalize(executable).ok())
        .map(|executable| crate::external_filesystem_path(&executable).into_owned())
}

#[cfg(target_os = "linux")]
pub(crate) fn absolute_tool(name: &str) -> Option<PathBuf> {
    absolute_tools(name).next()
}

pub(crate) fn bounded_output(tool: &Path, args: &[&str]) -> io::Result<(ExitStatus, Vec<u8>)> {
    const LIMIT: u64 = 1024 * 1024;
    if !tool.is_absolute() {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    // Called by the platform's blocking workers. A dedicated current-thread runtime
    // lets a timed-out read release its pipe and kill only the child owned here.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(10), async {
            let directory = tool.parent().ok_or(io::ErrorKind::InvalidInput)?;
            let mut command = tokio::process::Command::new(tool);
            command
                .args(args)
                .current_dir(directory)
                .env("UNITY_NO_UPDATE_CHECK", "1")
                .env("UNITY_NO_CLI_INVOKED_TELEMETRY", "1")
                .env("UNITY_NO_CRASH_REPORT", "1")
                .env("UNITY_NO_CONSENT_PROMPT", "1")
                .stdin(Stdio::null())
                .stderr(Stdio::null())
                .stdout(Stdio::piped())
                .kill_on_drop(true);
            let mut child = command.spawn()?;
            let stdout = child.stdout.take().ok_or(io::ErrorKind::BrokenPipe)?;
            let mut output = Vec::new();
            stdout.take(LIMIT + 1).read_to_end(&mut output).await?;
            if output.len() as u64 > LIMIT {
                return Err(io::ErrorKind::InvalidData.into());
            }
            Ok((child.wait().await?, output))
        })
        .await
        .map_err(|_| io::ErrorKind::TimedOut)?
    })
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn non_executable_native_registration_does_not_hide_packaged_hub() {
        use std::os::unix::fs::PermissionsExt;

        let path =
            std::env::temp_dir().join(format!("alcomd-hub-registration-{}", std::process::id()));
        let f = std::fs::File::create_new(&path).expect("registration fixture");
        let result = Registration {
            targets: vec![UnityAppTarget::Executable(path.clone())],
            unavailable: false,
        };
        f.set_permissions(std::fs::Permissions::from_mode(0o600))
            .expect("make non-executable");
        let blocked_fallback = has_usable_native_hub(&result);
        f.set_permissions(std::fs::Permissions::from_mode(0o700))
            .expect("make executable");
        let usable = has_usable_native_hub(&result);
        drop(f);
        std::fs::remove_file(path).expect("remove registration fixture");
        assert!(!blocked_fallback);
        assert!(usable);
    }

    #[test]
    fn desktop_registration_resolves_only_direct_files() {
        let editor =
            "[Desktop Entry]\nName=Unity\nType=Application\nExec=\"/some place/Editor/Unity\" %F\n";
        assert_eq!(
            desktop_executable(UnityApp::Editor, editor),
            Some(PathBuf::from("/some place/Editor/Unity"))
        );
        assert_eq!(desktop_executable(UnityApp::Hub, editor), None);
        assert_eq!(
            desktop_executable(UnityApp::Editor, &format!("{editor}Hidden=true\n")),
            None
        );
        assert_eq!(
            desktop_executable(UnityApp::Editor, &editor.replace("Exec=", "Exec=env ")),
            None
        );
    }
}
