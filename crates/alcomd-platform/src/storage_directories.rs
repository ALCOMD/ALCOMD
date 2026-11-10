//! User home discovery and the narrow filesystem operations for storage roots.

use std::io;
use std::path::{Component, Path, PathBuf};

const MAX_PATH_BYTES: usize = 4096;

/// Resolves platform defaults without creating directories or inspecting project data.
pub fn default_storage_directories() -> io::Result<(PathBuf, PathBuf)> {
    let home = home_directory()?;
    validate_path_text(&home)?;
    let root = home.join(env!("ALCOMD_PRODUCT_DIRECTORY"));
    let projects = root.join("Projects");
    let backups = root.join("Backups");
    validate_path_text(&projects)?;
    validate_path_text(&backups)?;
    Ok((projects, backups))
}

#[cfg(windows)]
fn home_directory() -> io::Result<PathBuf> {
    crate::windows::user_profile_directory()
}

#[cfg(unix)]
fn home_directory() -> io::Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| invalid("HOME is unavailable"))
}

fn validate_path_text(path: &Path) -> io::Result<()> {
    let text = path
        .to_str()
        .ok_or_else(|| invalid("directory path is not Unicode"))?;
    if !path.is_absolute()
        || text.len() > MAX_PATH_BYTES
        || text.chars().any(char::is_control)
        || path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err(invalid("directory must be an absolute bounded path"));
    }
    Ok(())
}

/// Validates a selected existing directory and returns its canonical Unicode path.
pub fn normalize_storage_directory(path: &str) -> io::Result<String> {
    let path = Path::new(path);
    validate_path_text(path)?;
    let (canonical, _) = crate::resolve_directory_identity(path)?;
    validate_path_text(&canonical)?;
    canonical
        .into_os_string()
        .into_string()
        .map_err(|_| invalid("directory path is not Unicode"))
}

/// Prepares a Core-selected storage root. No project, archive or existing entry is removed.
pub fn prepare_storage_directory(path: &Path) -> io::Result<PathBuf> {
    validate_path_text(path)?;
    let mut missing = Vec::new();
    let mut existing = path;
    while !existing.try_exists()? {
        missing.push(
            existing
                .file_name()
                .ok_or_else(|| invalid("storage directory has no parent"))?
                .to_owned(),
        );
        existing = existing
            .parent()
            .ok_or_else(|| invalid("storage directory has no parent"))?;
    }
    let (mut canonical, _) = crate::resolve_directory_identity(existing)?;
    for leaf in missing.into_iter().rev() {
        canonical.push(leaf);
        match std::fs::create_dir(&canonical) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let metadata = std::fs::symlink_metadata(&canonical)?;
        if !metadata.is_dir() || is_link_or_reparse(&metadata) {
            return Err(invalid("storage directory is not a plain directory"));
        }
    }
    let (canonical, _) = crate::resolve_directory_identity(&canonical)?;
    validate_path_text(&canonical)?;
    Ok(canonical)
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(unix)]
fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

/// Atomically moves a file or directory without replacing an existing destination.
#[cfg(unix)]
pub fn publish_path_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        source,
        rustix::fs::CWD,
        destination,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(io::Error::from)
}

#[cfg(windows)]
pub fn publish_path_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    crate::windows::publish_path_no_replace(source, destination)
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        std::env::temp_dir().join(format!(
            "alcomd-storage-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    #[test]
    fn selected_directory_validation_and_default_preparation_are_separate() {
        let root = fixture();
        let target = root.join("projects with spaces");
        assert!(normalize_storage_directory(target.to_str().expect("path")).is_err());
        let prepared = prepare_storage_directory(&target).expect("prepare");
        assert_eq!(
            Path::new(
                &normalize_storage_directory(target.to_str().expect("path")).expect("normalize")
            ),
            prepared
        );
        assert!(normalize_storage_directory("relative/path").is_err());
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn publishing_never_replaces_an_existing_file() {
        let root = fixture();
        std::fs::create_dir(&root).expect("directory");
        let source = root.join("archive.part");
        let target = root.join("archive.zip");
        std::fs::write(&source, b"new").expect("source");
        std::fs::write(&target, b"existing").expect("target");
        assert!(publish_path_no_replace(&source, &target).is_err());
        assert_eq!(std::fs::read(&source).expect("source"), b"new");
        assert_eq!(std::fs::read(&target).expect("target"), b"existing");
        std::fs::remove_file(&target).expect("remove target");
        publish_path_no_replace(&source, &target).expect("publish");
        assert!(!source.exists());
        assert_eq!(std::fs::read(&target).expect("published"), b"new");
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn publishing_a_directory_never_replaces_an_empty_destination() {
        let root = fixture();
        let source = root.join("staging").join("project");
        let target = root.join("project");
        std::fs::create_dir_all(&source).expect("staging project");
        std::fs::write(source.join("contents"), b"new").expect("project contents");
        std::fs::create_dir(&target).expect("existing empty directory");
        assert!(publish_path_no_replace(&source, &target).is_err());
        assert!(source.join("contents").is_file());
        assert!(
            std::fs::read_dir(&target)
                .expect("existing directory")
                .next()
                .is_none()
        );
        std::fs::remove_dir(&target).expect("release target");
        publish_path_no_replace(&source, &target).expect("publish");
        assert_eq!(
            std::fs::read(target.join("contents")).expect("published project"),
            b"new"
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn default_roots_are_under_user_home() {
        let home = home_directory().expect("user home");
        let (projects, backups) = default_storage_directories().expect("default roots");
        let product = home.join(env!("ALCOMD_PRODUCT_DIRECTORY"));
        assert_eq!(projects.parent(), Some(product.as_path()));
        assert_eq!(backups.parent(), Some(product.as_path()));
        assert_eq!(projects.file_name(), Some(std::ffi::OsStr::new("Projects")));
        assert_eq!(backups.file_name(), Some(std::ffi::OsStr::new("Backups")));
    }
}
