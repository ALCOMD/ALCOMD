//! Narrow filesystem primitives for the M7 Project Directory Delete workflow.
//!
//! These functions intentionally expose no general-purpose deletion API. They validate and
//! remove exactly one daemon-owned quarantine payload while refusing mount traversal and root
//! links/reparse points.

use std::io;
use std::path::Path;

/// Safe classification used by the Project Delete application adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectDeleteFilesystemErrorKind {
    MountBoundary,
    MountGuardUnavailable,
    UnsafeEntry,
    Io,
}

/// A path-redacted filesystem failure.
#[derive(Debug)]
pub struct ProjectDeleteFilesystemError {
    kind: ProjectDeleteFilesystemErrorKind,
    source: Option<io::Error>,
}

impl ProjectDeleteFilesystemError {
    #[must_use]
    pub const fn kind(&self) -> ProjectDeleteFilesystemErrorKind {
        self.kind
    }

    fn classified(kind: ProjectDeleteFilesystemErrorKind) -> Self {
        Self { kind, source: None }
    }

    fn io(source: io::Error) -> Self {
        Self {
            kind: ProjectDeleteFilesystemErrorKind::Io,
            source: Some(source),
        }
    }
}

impl std::fmt::Display for ProjectDeleteFilesystemError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Project Delete filesystem validation failed")
    }
}

impl std::error::Error for ProjectDeleteFilesystemError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|error| error as &(dyn std::error::Error + 'static))
    }
}

/// Streaming pre-quarantine mount/link safety scan.
pub fn project_delete_preflight(root: &Path) -> Result<u64, ProjectDeleteFilesystemError> {
    platform::preflight(root)
}

/// Streaming, no-follow removal of one quarantine payload directory.
pub fn project_delete_cleanup(root: &Path) -> Result<u64, ProjectDeleteFilesystemError> {
    platform::cleanup(root)
}

#[cfg(windows)]
mod platform {
    use std::fs;
    use std::os::windows::fs::MetadataExt;

    use super::*;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;

    pub(super) fn preflight(root: &Path) -> Result<u64, ProjectDeleteFilesystemError> {
        let metadata = fs::symlink_metadata(root).map_err(ProjectDeleteFilesystemError::io)?;
        if !metadata.is_dir() || reparse(&metadata) {
            return Err(ProjectDeleteFilesystemError::classified(
                ProjectDeleteFilesystemErrorKind::UnsafeEntry,
            ));
        }
        scan(root)
    }

    pub(super) fn cleanup(root: &Path) -> Result<u64, ProjectDeleteFilesystemError> {
        let count = preflight(root)?;
        fs::remove_dir_all(root).map_err(ProjectDeleteFilesystemError::io)?;
        Ok(count)
    }

    fn scan(directory: &Path) -> Result<u64, ProjectDeleteFilesystemError> {
        let mut count = 0_u64;
        for entry in fs::read_dir(directory).map_err(ProjectDeleteFilesystemError::io)? {
            let entry = entry.map_err(ProjectDeleteFilesystemError::io)?;
            let metadata =
                fs::symlink_metadata(entry.path()).map_err(ProjectDeleteFilesystemError::io)?;
            count = count.saturating_add(1);
            if metadata.is_dir() && !reparse(&metadata) {
                count = count.saturating_add(scan(&entry.path())?);
            }
        }
        Ok(count)
    }

    fn reparse(metadata: &fs::Metadata) -> bool {
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{
        ProjectDeleteFilesystemErrorKind, project_delete_cleanup, project_delete_preflight,
    };

    #[test]
    fn nested_directory_link_is_unlinked_without_touching_external_target() {
        let fixture = TestDirectory::new();
        let payload = fixture.path().join("payload");
        let external = fixture.path().join("external");
        fs::create_dir(&payload).expect("payload");
        fs::create_dir(&external).expect("external");
        fs::write(external.join("sentinel.txt"), b"preserve me").expect("sentinel");
        create_directory_link(&external, &payload.join("linked"));

        assert_eq!(project_delete_preflight(&payload).expect("preflight"), 1);
        assert_eq!(project_delete_cleanup(&payload).expect("cleanup"), 1);
        assert!(!payload.exists());
        assert_eq!(
            fs::read(external.join("sentinel.txt")).expect("external sentinel"),
            b"preserve me"
        );
    }

    #[test]
    fn hard_link_name_is_removed_while_external_link_content_survives() {
        let fixture = TestDirectory::new();
        let payload = fixture.path().join("payload");
        let external = fixture.path().join("external.txt");
        fs::create_dir(&payload).expect("payload");
        fs::write(&external, b"shared content").expect("external file");
        fs::hard_link(&external, payload.join("linked.txt")).expect("hard link");

        assert_eq!(project_delete_preflight(&payload).expect("preflight"), 1);
        assert_eq!(project_delete_cleanup(&payload).expect("cleanup"), 1);
        assert!(!payload.exists());
        assert_eq!(
            fs::read(external).expect("external content"),
            b"shared content"
        );
    }

    #[test]
    fn root_link_is_rejected_and_target_survives() {
        let fixture = TestDirectory::new();
        let target = fixture.path().join("target");
        let root_link = fixture.path().join("payload-link");
        fs::create_dir(&target).expect("target");
        fs::write(target.join("sentinel.txt"), b"preserve me").expect("sentinel");
        create_directory_link(&target, &root_link);

        let error = project_delete_preflight(&root_link).expect_err("root link must fail closed");
        assert_eq!(error.kind(), ProjectDeleteFilesystemErrorKind::UnsafeEntry);
        assert_eq!(
            fs::read(target.join("sentinel.txt")).expect("target sentinel"),
            b"preserve me"
        );
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "requires a real nested mount prepared by the hosted CI fixture"]
    fn real_nested_mount_is_rejected_and_its_sentinel_survives() {
        let root = PathBuf::from(
            std::env::var_os("ALCOMD_TEST_PROJECT_DELETE_MOUNT_ROOT")
                .expect("mounted fixture root"),
        );
        let sentinel = PathBuf::from(
            std::env::var_os("ALCOMD_TEST_PROJECT_DELETE_MOUNT_SENTINEL")
                .expect("mounted fixture sentinel"),
        );
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::fs::MetadataExt;

            let nested = sentinel.parent().expect("nested mount");
            assert_eq!(
                fs::metadata(&root).expect("root metadata").dev(),
                fs::metadata(nested).expect("nested metadata").dev(),
                "Linux fixture must be a same-filesystem bind mount"
            );
        }
        let error = project_delete_preflight(&root).expect_err("nested mount must fail closed");
        assert_eq!(
            error.kind(),
            ProjectDeleteFilesystemErrorKind::MountBoundary
        );
        assert_eq!(
            fs::read(sentinel).expect("mounted sentinel"),
            b"preserve me"
        );
    }

    #[cfg(windows)]
    #[test]
    fn nested_junction_is_removed_without_touching_external_target() {
        use std::process::{Command, Stdio};

        let fixture = TestDirectory::new();
        let payload = fixture.path().join("payload");
        let external = fixture.path().join("external-junction-target");
        let junction = payload.join("junction");
        fs::create_dir(&payload).expect("payload");
        fs::create_dir(&external).expect("external target");
        fs::write(external.join("sentinel.txt"), b"preserve me").expect("sentinel");
        let status = Command::new("cmd.exe")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&external)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("create junction");
        assert!(status.success(), "junction fixture creation failed");

        assert_eq!(project_delete_preflight(&payload).expect("preflight"), 1);
        assert_eq!(project_delete_cleanup(&payload).expect("cleanup"), 1);
        assert!(!payload.exists());
        assert_eq!(
            fs::read(external.join("sentinel.txt")).expect("external sentinel"),
            b"preserve me"
        );
    }

    #[cfg(unix)]
    fn create_directory_link(target: &Path, link: &Path) {
        std::os::unix::fs::symlink(target, link).expect("create directory symlink");
    }

    #[cfg(windows)]
    fn create_directory_link(target: &Path, link: &Path) {
        std::os::windows::fs::symlink_dir(target, link).expect("create directory symlink");
    }

    struct TestDirectory(PathBuf);

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "alcomd-project-delete-platform-{}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("fixture directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::ffi::CStr;

    use rustix::fd::OwnedFd;
    use rustix::fs::{
        AtFlags, CWD, Dir, FileType, Mode, OFlags, ResolveFlags, openat, openat2, statat, unlinkat,
    };
    use rustix::io::Errno;

    use super::*;

    const DIRECTORY_FLAGS: OFlags = OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::CLOEXEC)
        .union(OFlags::NOFOLLOW);
    const RESOLVE_FLAGS: ResolveFlags = ResolveFlags::BENEATH
        .union(ResolveFlags::NO_XDEV)
        .union(ResolveFlags::NO_MAGICLINKS)
        .union(ResolveFlags::NO_SYMLINKS);

    pub(super) fn preflight(root: &Path) -> Result<u64, ProjectDeleteFilesystemError> {
        let (parent, leaf) = split_root(root)?;
        let parent_fd = open_parent(parent)?;
        verify_root_directory(&parent_fd, leaf)?;
        let root_fd = open_directory(&parent_fd, leaf)?;
        walk(&root_fd, false)
    }

    pub(super) fn cleanup(root: &Path) -> Result<u64, ProjectDeleteFilesystemError> {
        let (parent, leaf) = split_root(root)?;
        let parent_fd = open_parent(parent)?;
        verify_root_directory(&parent_fd, leaf)?;
        let root_fd = open_directory(&parent_fd, leaf)?;
        let count = walk(&root_fd, true)?;
        unlinkat(&parent_fd, leaf, AtFlags::REMOVEDIR).map_err(map_errno)?;
        Ok(count)
    }

    fn split_root(root: &Path) -> Result<(&Path, &std::ffi::OsStr), ProjectDeleteFilesystemError> {
        if !root.is_absolute() {
            return Err(ProjectDeleteFilesystemError::classified(
                ProjectDeleteFilesystemErrorKind::UnsafeEntry,
            ));
        }
        let parent = root.parent().ok_or_else(|| {
            ProjectDeleteFilesystemError::classified(ProjectDeleteFilesystemErrorKind::UnsafeEntry)
        })?;
        let leaf = root.file_name().ok_or_else(|| {
            ProjectDeleteFilesystemError::classified(ProjectDeleteFilesystemErrorKind::UnsafeEntry)
        })?;
        Ok((parent, leaf))
    }

    fn open_parent(parent: &Path) -> Result<OwnedFd, ProjectDeleteFilesystemError> {
        openat(CWD, parent, DIRECTORY_FLAGS, Mode::empty()).map_err(map_errno)
    }

    fn verify_root_directory(
        parent: &OwnedFd,
        leaf: &std::ffi::OsStr,
    ) -> Result<(), ProjectDeleteFilesystemError> {
        let stat = statat(parent, leaf, AtFlags::SYMLINK_NOFOLLOW).map_err(map_errno)?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
            return Err(ProjectDeleteFilesystemError::classified(
                ProjectDeleteFilesystemErrorKind::UnsafeEntry,
            ));
        }
        Ok(())
    }

    fn open_directory<Fd: std::os::fd::AsFd, P: rustix::path::Arg>(
        parent: Fd,
        name: P,
    ) -> Result<OwnedFd, ProjectDeleteFilesystemError> {
        openat2(parent, name, DIRECTORY_FLAGS, Mode::empty(), RESOLVE_FLAGS).map_err(map_errno)
    }

    fn verify_non_directory<Fd: std::os::fd::AsFd>(
        parent: Fd,
        name: &CStr,
    ) -> Result<(), ProjectDeleteFilesystemError> {
        openat2(
            parent,
            name,
            OFlags::PATH | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
            RESOLVE_FLAGS,
        )
        .map(|_| ())
        .map_err(map_errno)
    }

    fn walk(directory: &OwnedFd, remove: bool) -> Result<u64, ProjectDeleteFilesystemError> {
        let entries = Dir::read_from(directory).map_err(map_errno)?;
        let mut count = 0_u64;
        for entry in entries {
            let entry = entry.map_err(map_errno)?;
            let name = entry.file_name();
            if name.to_bytes() == b"." || name.to_bytes() == b".." {
                continue;
            }
            let stat = statat(directory, name, AtFlags::SYMLINK_NOFOLLOW).map_err(map_errno)?;
            let kind = FileType::from_raw_mode(stat.st_mode);
            count = count.saturating_add(1);
            if kind == FileType::Directory {
                let child = open_directory(directory, name)?;
                count = count.saturating_add(walk(&child, remove)?);
                if remove {
                    unlinkat(directory, name, AtFlags::REMOVEDIR).map_err(map_errno)?;
                }
            } else {
                if kind != FileType::Symlink {
                    verify_non_directory(directory, name)?;
                }
                if remove {
                    unlinkat(directory, name, AtFlags::empty()).map_err(map_errno)?;
                }
            }
        }
        Ok(count)
    }

    fn map_errno(error: Errno) -> ProjectDeleteFilesystemError {
        match error {
            Errno::XDEV => ProjectDeleteFilesystemError::classified(
                ProjectDeleteFilesystemErrorKind::MountBoundary,
            ),
            Errno::NOSYS | Errno::INVAL => ProjectDeleteFilesystemError::classified(
                ProjectDeleteFilesystemErrorKind::MountGuardUnavailable,
            ),
            _ => ProjectDeleteFilesystemError::io(error.into()),
        }
    }

    #[cfg(test)]
    mod linux_tests {
        use super::*;

        #[test]
        fn unavailable_openat2_fails_closed_without_a_fallback_walker() {
            for error in [Errno::NOSYS, Errno::INVAL] {
                assert_eq!(
                    map_errno(error).kind(),
                    ProjectDeleteFilesystemErrorKind::MountGuardUnavailable
                );
            }
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use rustix::fd::OwnedFd;
    use rustix::fs::{
        AtFlags, CWD, Dir, FileType, Mode, OFlags, fstat, fstatfs, openat, statat, unlinkat,
    };
    use rustix::io::Errno;

    use super::*;

    const DIRECTORY_FLAGS: OFlags = OFlags::RDONLY
        .union(OFlags::DIRECTORY)
        .union(OFlags::CLOEXEC)
        .union(OFlags::NOFOLLOW);

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct MacMountToken {
        device: u64,
        mount_name: Vec<u8>,
    }

    pub(super) fn preflight(root: &Path) -> Result<u64, ProjectDeleteFilesystemError> {
        let (parent, leaf) = split_root(root)?;
        let parent_fd = open_parent(parent)?;
        let root_token = mount_token(&parent_fd)?;
        verify_root_directory(&parent_fd, leaf)?;
        let root_fd = open_directory(&parent_fd, leaf)?;
        verify_mount(&root_fd, &root_token)?;
        walk(&root_fd, &root_token, false)
    }

    pub(super) fn cleanup(root: &Path) -> Result<u64, ProjectDeleteFilesystemError> {
        let (parent, leaf) = split_root(root)?;
        let parent_fd = open_parent(parent)?;
        let root_token = mount_token(&parent_fd)?;
        verify_root_directory(&parent_fd, leaf)?;
        let root_fd = open_directory(&parent_fd, leaf)?;
        verify_mount(&root_fd, &root_token)?;
        let count = walk(&root_fd, &root_token, true)?;
        unlinkat(&parent_fd, leaf, AtFlags::REMOVEDIR).map_err(map_errno)?;
        Ok(count)
    }

    fn split_root(root: &Path) -> Result<(&Path, &std::ffi::OsStr), ProjectDeleteFilesystemError> {
        if !root.is_absolute() {
            return Err(ProjectDeleteFilesystemError::classified(
                ProjectDeleteFilesystemErrorKind::UnsafeEntry,
            ));
        }
        let parent = root.parent().ok_or_else(|| {
            ProjectDeleteFilesystemError::classified(ProjectDeleteFilesystemErrorKind::UnsafeEntry)
        })?;
        let leaf = root.file_name().ok_or_else(|| {
            ProjectDeleteFilesystemError::classified(ProjectDeleteFilesystemErrorKind::UnsafeEntry)
        })?;
        Ok((parent, leaf))
    }

    fn open_parent(parent: &Path) -> Result<OwnedFd, ProjectDeleteFilesystemError> {
        openat(CWD, parent, DIRECTORY_FLAGS, Mode::empty()).map_err(map_errno)
    }

    fn verify_root_directory(
        parent: &OwnedFd,
        leaf: &std::ffi::OsStr,
    ) -> Result<(), ProjectDeleteFilesystemError> {
        let stat = statat(parent, leaf, AtFlags::SYMLINK_NOFOLLOW).map_err(map_errno)?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
            return Err(ProjectDeleteFilesystemError::classified(
                ProjectDeleteFilesystemErrorKind::UnsafeEntry,
            ));
        }
        Ok(())
    }

    fn open_directory<Fd: std::os::fd::AsFd, P: rustix::path::Arg>(
        parent: Fd,
        name: P,
    ) -> Result<OwnedFd, ProjectDeleteFilesystemError> {
        openat(parent, name, DIRECTORY_FLAGS, Mode::empty()).map_err(map_errno)
    }

    fn walk(
        directory: &OwnedFd,
        root_token: &MacMountToken,
        remove: bool,
    ) -> Result<u64, ProjectDeleteFilesystemError> {
        verify_mount(directory, root_token)?;
        let entries = Dir::read_from(directory).map_err(map_errno)?;
        let mut count = 0_u64;
        for entry in entries {
            let entry = entry.map_err(map_errno)?;
            let name = entry.file_name();
            if name.to_bytes() == b"." || name.to_bytes() == b".." {
                continue;
            }
            let stat = statat(directory, name, AtFlags::SYMLINK_NOFOLLOW).map_err(map_errno)?;
            let kind = FileType::from_raw_mode(stat.st_mode);
            count = count.saturating_add(1);
            if kind == FileType::Directory {
                let child = open_directory(directory, name)?;
                verify_mount(&child, root_token)?;
                count = count.saturating_add(walk(&child, root_token, remove)?);
                if remove {
                    unlinkat(directory, name, AtFlags::REMOVEDIR).map_err(map_errno)?;
                }
            } else if remove {
                unlinkat(directory, name, AtFlags::empty()).map_err(map_errno)?;
            }
        }
        Ok(count)
    }

    fn mount_token(fd: &OwnedFd) -> Result<MacMountToken, ProjectDeleteFilesystemError> {
        let stat = fstat(fd).map_err(map_errno)?;
        let filesystem = fstatfs(fd).map_err(map_errno)?;
        let mount_name = filesystem
            .f_mntonname
            .iter()
            .take_while(|byte| **byte != 0)
            .map(|byte| *byte as u8)
            .collect();
        Ok(MacMountToken {
            device: stat.st_dev as u64,
            mount_name,
        })
    }

    fn verify_mount(
        fd: &OwnedFd,
        expected: &MacMountToken,
    ) -> Result<(), ProjectDeleteFilesystemError> {
        mount_matches(expected, &mount_token(fd)?)
            .then_some(())
            .ok_or_else(|| {
                ProjectDeleteFilesystemError::classified(
                    ProjectDeleteFilesystemErrorKind::MountBoundary,
                )
            })
    }

    fn mount_matches(expected: &MacMountToken, actual: &MacMountToken) -> bool {
        expected == actual
    }

    fn map_errno(error: Errno) -> ProjectDeleteFilesystemError {
        ProjectDeleteFilesystemError::io(error.into())
    }

    #[cfg(test)]
    mod macos_tests {
        use super::*;

        #[test]
        fn same_device_with_a_different_mount_name_is_a_boundary() {
            let expected = MacMountToken {
                device: 7,
                mount_name: b"/private/tmp/root".to_vec(),
            };
            let nested = MacMountToken {
                device: 7,
                mount_name: b"/private/tmp/root/nested".to_vec(),
            };
            assert!(!mount_matches(&expected, &nested));
        }
    }
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
mod platform {
    use super::*;

    pub(super) fn preflight(_: &Path) -> Result<u64, ProjectDeleteFilesystemError> {
        Err(ProjectDeleteFilesystemError::classified(
            ProjectDeleteFilesystemErrorKind::MountGuardUnavailable,
        ))
    }

    pub(super) fn cleanup(_: &Path) -> Result<u64, ProjectDeleteFilesystemError> {
        Err(ProjectDeleteFilesystemError::classified(
            ProjectDeleteFilesystemErrorKind::MountGuardUnavailable,
        ))
    }
}
