//! Bounded desktop effects executed in the Core user's interactive session.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopError {
    Unavailable,
    DirectoryMissing,
    NotDirectory,
    DirectoryOpenFailed,
    DownloadOpenFailed,
    WindowNotFound,
    FocusRejected,
    FocusFailed,
}

pub fn registered_root(path: &Path) -> Result<PathBuf, DesktopError> {
    if !path.is_absolute() {
        return Err(DesktopError::DirectoryMissing);
    }
    let metadata = path.metadata().map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => DesktopError::DirectoryMissing,
        _ => DesktopError::DirectoryOpenFailed,
    })?;
    if !metadata.is_dir() {
        return Err(DesktopError::NotDirectory);
    }
    path.canonicalize()
        .map_err(|_| DesktopError::DirectoryOpenFailed)
}

pub trait DesktopDriver: Send + Sync {
    fn focus_unity(&self, project_root: &Path) -> Result<(), DesktopError>;
    fn open_directory(&self, project_root: &Path) -> Result<(), DesktopError>;
    fn open_unity_download(&self) -> Result<(), DesktopError>;
}

pub struct SystemDesktop;

impl DesktopDriver for SystemDesktop {
    fn focus_unity(&self, project_root: &Path) -> Result<(), DesktopError> {
        #[cfg(windows)]
        {
            interactive_session()?;
            match crate::focus_unity_window(project_root) {
                Ok(crate::UnityWindowFocus::Activated) => Ok(()),
                Ok(crate::UnityWindowFocus::WindowNotFound) => Err(DesktopError::WindowNotFound),
                Ok(crate::UnityWindowFocus::Rejected) => Err(DesktopError::FocusRejected),
                Err(_) => Err(DesktopError::FocusFailed),
            }
        }
        #[cfg(not(windows))]
        {
            let _ = project_root;
            Err(DesktopError::Unavailable)
        }
    }

    fn open_directory(&self, project_root: &Path) -> Result<(), DesktopError> {
        interactive_session()?;
        open::that(crate::external_filesystem_path(project_root).as_os_str())
            .map_err(|_| DesktopError::DirectoryOpenFailed)
    }

    fn open_unity_download(&self) -> Result<(), DesktopError> {
        interactive_session()?;
        open::that("https://unity.com/download").map_err(|_| DesktopError::DownloadOpenFailed)
    }
}

fn interactive_session() -> Result<(), DesktopError> {
    #[cfg(windows)]
    if !crate::windows_unity_window::interactive_desktop_available() {
        return Err(DesktopError::Unavailable);
    }
    #[cfg(target_os = "linux")]
    if ["DISPLAY", "WAYLAND_DISPLAY"]
        .iter()
        .all(|key| std::env::var_os(key).is_none_or(|value| value.is_empty()))
    {
        return Err(DesktopError::Unavailable);
    }
    Ok(())
}
