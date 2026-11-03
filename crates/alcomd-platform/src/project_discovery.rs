//! Read-only, bounded enumeration of plain immediate child directories.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredDirectory {
    pub path: PathBuf,
    pub identity: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryDiscovery {
    pub root: DiscoveredDirectory,
    pub children: Vec<DiscoveredDirectory>,
    pub reached_limit: bool,
}

/// Enumerates at most `limit` directory entries; it never descends or follows a child link.
pub fn discover_directories(root: &Path, limit: usize) -> io::Result<DirectoryDiscovery> {
    if limit == 0 {
        return Err(invalid("directory discovery limit must be nonzero"));
    }
    let root = plain_directory(root)?;
    let mut children = Vec::new();
    let mut observed = 0;
    for entry in fs::read_dir(&root.path)?.take(limit) {
        observed += 1;
        let entry = entry?;
        let Ok(child) = plain_directory(&entry.path()) else {
            continue;
        };
        if child.path.parent() == Some(root.path.as_path()) {
            children.push(child);
        }
    }
    if plain_directory(&root.path)? != root {
        return Err(invalid("directory discovery root changed"));
    }
    children.sort_unstable_by(|left, right| left.path.cmp(&right.path));
    Ok(DirectoryDiscovery {
        root,
        children,
        // Reaching the bound is reported conservatively without reading one extra entry.
        reached_limit: observed == limit,
    })
}

/// A candidate is usable only while both objects remain plain directories with the same parent.
pub fn discovered_directory_is_current(
    root: &DiscoveredDirectory,
    child: &DiscoveredDirectory,
) -> io::Result<bool> {
    let current_root = match plain_directory(&root.path) {
        Ok(value) => value,
        Err(error) if rejected_or_missing(&error) => return Ok(false),
        Err(error) => return Err(error),
    };
    if current_root != *root || child.path.parent() != Some(root.path.as_path()) {
        return Ok(false);
    }
    match plain_directory(&child.path) {
        Ok(current) => Ok(current == *child && current.path.parent() == Some(root.path.as_path())),
        Err(error) if rejected_or_missing(&error) => Ok(false),
        Err(error) => Err(error),
    }
}

fn plain_directory(path: &Path) -> io::Result<DiscoveredDirectory> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || is_link_or_reparse(&metadata) {
        return Err(invalid("directory discovery requires a plain directory"));
    }
    let (path, identity) = crate::resolve_directory_identity(path)?;
    Ok(DiscoveredDirectory { path, identity })
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(unix)]
fn is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn rejected_or_missing(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory | io::ErrorKind::InvalidInput
    )
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "alcomd-discovery-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("clock")
                    .as_nanos()
            ));
            fs::create_dir(&path).expect("create owned fixture");
            Self(path)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove owned fixture");
        }
    }

    #[test]
    fn enumeration_is_bounded_and_only_returns_immediate_directories() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.0.join("a/nested/project")).expect("nested fixture");
        fs::create_dir(fixture.0.join("b")).expect("second child");
        fs::write(fixture.0.join("file"), b"keep").expect("non-directory");
        let all = discover_directories(&fixture.0, 10).expect("enumerate");
        assert_eq!(all.children.len(), 2);
        assert!(!all.reached_limit);
        assert!(
            all.children
                .iter()
                .all(|child| child.path.parent() == Some(all.root.path.as_path()))
        );
        assert!(
            all.children
                .iter()
                .all(|child| discovered_directory_is_current(&all.root, child).expect("validate"))
        );
        let bounded = discover_directories(&fixture.0, 1).expect("bounded enumeration");
        assert!(bounded.reached_limit);
        assert!(bounded.children.len() <= 1);
        assert!(discover_directories(&fixture.0, 0).is_err());
        assert_eq!(
            fs::read(fixture.0.join("file")).expect("unchanged file"),
            b"keep"
        );
    }

    #[test]
    fn replaced_root_or_candidate_identity_is_rejected() {
        let fixture = Fixture::new();
        let root = fixture.0.join("root");
        fs::create_dir_all(root.join("project")).expect("fixture project");
        let all = discover_directories(&root, 10).expect("enumerate");
        let child = &all.children[0];
        fs::rename(root.join("project"), fixture.0.join("old-project")).expect("move child");
        fs::create_dir(root.join("project")).expect("replace child");
        assert!(!discovered_directory_is_current(&all.root, child).expect("reject child identity"));
        fs::rename(&root, fixture.0.join("old-root")).expect("move root");
        fs::create_dir(&root).expect("replace root");
        assert!(!discovered_directory_is_current(&all.root, child).expect("reject root identity"));
    }

    #[cfg(unix)]
    #[test]
    fn links_are_not_enumerated_or_followed_as_roots() {
        let fixture = Fixture::new();
        let root = fixture.0.join("root");
        let external = fixture.0.join("external");
        fs::create_dir(&root).expect("root");
        fs::create_dir(&external).expect("external");
        std::os::unix::fs::symlink(&external, root.join("linked")).expect("child symlink");
        std::os::unix::fs::symlink(&root, fixture.0.join("linked-root")).expect("root symlink");
        let all = discover_directories(&root, 10).expect("enumerate");
        assert!(all.children.is_empty());
        assert!(discover_directories(&fixture.0.join("linked-root"), 10).is_err());
    }

    #[cfg(windows)]
    #[test]
    fn junctions_are_not_enumerated_or_followed_as_roots() {
        use std::process::{Command, Stdio};

        let fixture = Fixture::new();
        let root = fixture.0.join("root");
        let external = fixture.0.join("external");
        fs::create_dir(&root).expect("root");
        fs::create_dir(&external).expect("external");
        fs::write(external.join("sentinel"), b"keep").expect("sentinel");
        let child_link = root.join("linked");
        let root_link = fixture.0.join("linked-root");
        for (link, target) in [(&child_link, &external), (&root_link, &root)] {
            let status = Command::new("cmd.exe")
                .args(["/D", "/C", "mklink", "/J"])
                .arg(link)
                .arg(target)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .expect("create junction");
            assert!(status.success(), "junction fixture creation failed");
        }
        let all = discover_directories(&root, 10).expect("enumerate");
        assert!(all.children.is_empty());
        assert!(discover_directories(&root_link, 10).is_err());
        assert_eq!(
            fs::read(external.join("sentinel")).expect("sentinel"),
            b"keep"
        );
        // Remove only the links, so fixture teardown never traverses a junction.
        fs::remove_dir(&root_link).expect("remove root junction");
        fs::remove_dir(&child_link).expect("remove child junction");
    }
}
