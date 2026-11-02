//! System Documents discovery and the narrow filesystem operations for storage roots.

use std::io;
#[cfg(target_os = "linux")]
use std::io::Read;
use std::path::{Component, Path, PathBuf};

const MAX_PATH_BYTES: usize = 4096;

/// Resolves platform defaults without creating directories or inspecting project data.
pub fn default_storage_directories() -> io::Result<(PathBuf, PathBuf)> {
    let documents = documents_directory()?;
    validate_path_text(&documents)?;
    let root = documents.join(env!("ALCOMD_PRODUCT_DIRECTORY"));
    let projects = root.join("Projects");
    let backups = root.join("Backups");
    validate_path_text(&projects)?;
    validate_path_text(&backups)?;
    Ok((projects, backups))
}

#[cfg(windows)]
fn documents_directory() -> io::Result<PathBuf> {
    crate::windows::documents_directory()
}

#[cfg(unix)]
fn documents_directory() -> io::Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| invalid("HOME is unavailable"))?;
    validate_path_text(&home)?;
    #[cfg(target_os = "linux")]
    {
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        validate_path_text(&config)?;
        match std::fs::File::open(config.join("user-dirs.dirs")) {
            Ok(file) => {
                let mut text = String::new();
                file.take(16 * 1024 + 1).read_to_string(&mut text)?;
                if text.len() > 16 * 1024 {
                    return Err(invalid("user directory configuration is too large"));
                }
                return documents_from_user_dirs(&home, &text);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    Ok(home.join("Documents"))
}

#[cfg(any(target_os = "linux", test))]
fn documents_from_user_dirs(home: &Path, text: &str) -> io::Result<PathBuf> {
    let mut documents = None;
    for line in text.lines().map(str::trim) {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "XDG_DOCUMENTS_DIR" {
            continue;
        }
        if documents.is_some() {
            return Err(invalid("duplicate Documents directory"));
        }
        let mut quoted = value
            .trim()
            .strip_prefix('"')
            .ok_or_else(|| invalid("Documents directory is not quoted"))?
            .chars();
        let mut decoded = String::new();
        let mut closed = false;
        while let Some(character) = quoted.next() {
            match character {
                '"' => {
                    closed = true;
                    break;
                }
                '\\' => match quoted.next() {
                    Some(character @ ('\\' | '"')) => decoded.push(character),
                    _ => return Err(invalid("unsupported Documents directory escape")),
                },
                '`' => return Err(invalid("Documents directory contains command expansion")),
                character => decoded.push(character),
            }
        }
        let suffix = quoted.as_str().trim();
        if !closed || (!suffix.is_empty() && !suffix.starts_with('#')) {
            return Err(invalid("invalid Documents directory"));
        }
        let path = if decoded == "$HOME" || decoded == "${HOME}" {
            home.to_path_buf()
        } else if let Some(relative) = decoded
            .strip_prefix("$HOME/")
            .or_else(|| decoded.strip_prefix("${HOME}/"))
        {
            if relative.contains('$') {
                return Err(invalid("unsupported Documents directory expansion"));
            }
            home.join(relative)
        } else {
            if decoded.contains('$') {
                return Err(invalid("unsupported Documents directory expansion"));
            }
            PathBuf::from(decoded)
        };
        validate_path_text(&path)?;
        documents = Some(path);
    }
    Ok(documents.unwrap_or_else(|| home.join("Documents")))
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

/// Atomically moves a completed file without replacing an existing destination.
#[cfg(unix)]
pub fn publish_file_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
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
pub fn publish_file_no_replace(source: &Path, destination: &Path) -> io::Result<()> {
    crate::windows::publish_file_no_replace(source, destination)
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
        assert!(publish_file_no_replace(&source, &target).is_err());
        assert_eq!(std::fs::read(&source).expect("source"), b"new");
        assert_eq!(std::fs::read(&target).expect("target"), b"existing");
        std::fs::remove_file(&target).expect("remove target");
        publish_file_no_replace(&source, &target).expect("publish");
        assert!(!source.exists());
        assert_eq!(std::fs::read(&target).expect("published"), b"new");
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn linux_documents_configuration_is_data_and_never_shell_code() {
        let root = fixture();
        let home = root.as_path();
        assert_eq!(
            documents_from_user_dirs(home, "XDG_DOCUMENTS_DIR=\"$HOME/文档\"\n")
                .expect("documents"),
            home.join("文档")
        );
        assert_eq!(
            documents_from_user_dirs(
                home,
                &format!(
                    "XDG_DOCUMENTS_DIR=\"{}\" # location\n",
                    root.join("user docs")
                        .to_str()
                        .expect("path")
                        .replace('\\', "\\\\")
                )
            )
            .expect("documents"),
            root.join("user docs")
        );
        assert!(documents_from_user_dirs(home, "XDG_DOCUMENTS_DIR=\"$(echo /tmp)\"").is_err());
        assert!(
            documents_from_user_dirs(home, "XDG_DOCUMENTS_DIR=\"$HOME/../elsewhere\"").is_err()
        );
        assert!(documents_from_user_dirs(home, "XDG_DOCUMENTS_DIR=\"relative\"").is_err());
    }
}
