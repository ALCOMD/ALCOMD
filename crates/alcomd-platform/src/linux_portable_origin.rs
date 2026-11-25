//! File identity and the per-user origin record for an installed AppImage runtime.

use std::fs::File;
use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use rustix::fs::{AtFlags, RenameFlags, fstat, fsync, renameat_with, statat};
use rustix::process::geteuid;
use serde::{Deserialize, Serialize};

use crate::core_installation::open_protected_file;

pub const PORTABLE_ORIGIN_FILE: &str = "portable-origin-v1.json";
const MAX_PATH_BYTES: usize = 4096;
// JSON may escape each path byte. Leave room for the fixed fields and file identity.
const MAX_RECORD_BYTES: usize = 2 * MAX_PATH_BYTES + 1024;

/// A selected AppImage opened through protected directories without following links.
pub struct PortableImage {
    path: PathBuf,
    file: File,
    identity: PortableImageIdentity,
}

impl PortableImage {
    pub fn open(path: &Path) -> io::Result<Self> {
        validate_absolute_path(path)?;
        let file = File::from(open_protected_file(path, true, true)?);
        let identity = PortableImageIdentity::of(&file)?;
        if identity.size == 0 {
            return Err(invalid_origin());
        }
        Ok(Self {
            path: path.to_owned(),
            file,
            identity,
        })
    }

    /// The descriptor Bootstrap must hash before binding the installed runtime.
    pub fn file(&self) -> &File {
        &self.file
    }

    pub fn size(&self) -> u64 {
        self.identity.size
    }

    /// Opens a replaceable AppImage owned by this account, in an account-owned directory.
    pub fn open_for_update(path: &Path) -> io::Result<Self> {
        let image = Self::open(path)?;
        if image.identity.uid != geteuid().as_raw() {
            return Err(untrusted_update());
        }
        update_parent(path)?;
        Ok(image)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Rechecks the two anchored names, exchanges them atomically and syncs both directories.
    pub fn exchange_for_update(&self, other: &Self) -> io::Result<()> {
        let left_directory = update_parent(&self.path)?;
        let right_directory = update_parent(&other.path)?;
        let left_name = self.path.file_name().ok_or_else(invalid_origin)?;
        let right_name = other.path.file_name().ok_or_else(invalid_origin)?;
        let left_stat = statat(&left_directory, left_name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(io::Error::from)?;
        let right_stat = statat(&right_directory, right_name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(io::Error::from)?;
        if self.identity == other.identity
            || self.identity.device != other.identity.device
            || PortableImageIdentity::from_stat(&left_stat) != self.identity
            || PortableImageIdentity::from_stat(&right_stat) != other.identity
        {
            return Err(changed_image());
        }
        renameat_with(
            &left_directory,
            left_name,
            &right_directory,
            right_name,
            RenameFlags::EXCHANGE,
        )
        .map_err(io::Error::from)?;
        fsync(&right_directory).map_err(io::Error::from)?;
        fsync(&left_directory).map_err(io::Error::from)
    }
}

fn update_parent(path: &Path) -> io::Result<rustix::fd::OwnedFd> {
    let parent = path.parent().ok_or_else(invalid_origin)?;
    let directory = crate::unix::open_protected_directory_chain(parent)?;
    let stat = fstat(&directory).map_err(io::Error::from)?;
    if stat.st_uid != geteuid().as_raw()
        || stat.st_mode & 0o022 != 0
        || stat.st_mode & 0o300 != 0o300
    {
        return Err(untrusted_update());
    }
    Ok(directory)
}

fn untrusted_update() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "AppImage is not owned by this account",
    )
}

/// The origin of a verified portable runtime installed for the current user.
#[derive(Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PortableOrigin {
    schema: u32,
    app_image_path: PathBuf,
    app_image_sha256: String,
    payload_manifest_sha256: String,
    file_identity: PortableImageIdentity,
}

#[derive(Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PortableImageIdentity {
    uid: u32,
    device: u64,
    inode: u64,
    size: u64,
    ctime_seconds: i64,
    ctime_nanoseconds: i64,
}

impl PortableImageIdentity {
    fn of(file: &File) -> io::Result<Self> {
        let metadata = file.metadata()?;
        Ok(Self {
            uid: metadata.uid(),
            device: metadata.dev(),
            inode: metadata.ino(),
            size: metadata.len(),
            ctime_seconds: metadata.ctime(),
            ctime_nanoseconds: metadata.ctime_nsec(),
        })
    }

    fn from_stat(stat: &rustix::fs::Stat) -> Self {
        Self {
            uid: stat.st_uid,
            device: stat.st_dev,
            inode: stat.st_ino,
            size: stat.st_size.cast_unsigned(),
            ctime_seconds: stat.st_ctime,
            ctime_nanoseconds: stat.st_ctime_nsec.cast_signed(),
        }
    }

    fn same_object(self, other: Self) -> bool {
        (self.uid, self.device, self.inode, self.size)
            == (other.uid, other.device, other.inode, other.size)
    }
}

impl PortableOrigin {
    /// Binds the digest of the open image and the digest of its verified payload.
    pub fn bind(
        image: &PortableImage,
        app_image_sha256: &str,
        payload_manifest_sha256: &str,
    ) -> io::Result<Self> {
        let record = Self {
            schema: 1,
            app_image_path: image.path.clone(),
            app_image_sha256: app_image_sha256.to_owned(),
            payload_manifest_sha256: payload_manifest_sha256.to_owned(),
            file_identity: image.identity,
        };
        if PortableImageIdentity::of(image.file())? != image.identity {
            return Err(changed_image());
        }
        record.revalidate()?;
        Ok(record)
    }

    /// Reads the protected origin record without reopening the AppImage.
    pub fn read(runtime: &Path) -> io::Result<Self> {
        validate_absolute_path(runtime)?;
        let file = File::from(open_protected_file(
            &runtime.join(PORTABLE_ORIGIN_FILE),
            false,
            false,
        )?);
        let mut bytes = Vec::new();
        file.take((MAX_RECORD_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        Self::parse(&bytes)
    }

    /// Serializes the record for Bootstrap's installation transaction to publish.
    pub fn json_bytes(&self) -> io::Result<Vec<u8>> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| invalid_origin())?;
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(invalid_origin());
        }
        Ok(bytes)
    }

    /// Reopens the image and checks its identity before a cold start.
    pub fn revalidate(&self) -> io::Result<()> {
        self.validate()?;
        let current = PortableImage::open(&self.app_image_path)?;
        if current.identity != self.file_identity {
            return Err(changed_image());
        }
        Ok(())
    }

    pub fn payload_manifest_sha256(&self) -> &str {
        &self.payload_manifest_sha256
    }

    pub fn image_path(&self) -> &Path {
        &self.app_image_path
    }

    pub fn image_sha256(&self) -> &str {
        &self.app_image_sha256
    }

    /// Tests the recorded inode after an atomic exchange has legitimately changed its ctime.
    pub fn identifies_image(&self, image: &PortableImage) -> bool {
        self.file_identity.same_object(image.identity)
    }

    fn parse(bytes: &[u8]) -> io::Result<Self> {
        if bytes.len() > MAX_RECORD_BYTES {
            return Err(invalid_origin());
        }
        let record: Self = serde_json::from_slice(bytes).map_err(|_| invalid_origin())?;
        record.validate()?;
        Ok(record)
    }

    fn validate(&self) -> io::Result<()> {
        validate_absolute_path(&self.app_image_path)?;
        if self.schema != 1
            || !is_sha256(&self.app_image_sha256)
            || !is_sha256(&self.payload_manifest_sha256)
            || self.file_identity.size == 0
            || !(0..1_000_000_000).contains(&self.file_identity.ctime_nanoseconds)
        {
            return Err(invalid_origin());
        }
        Ok(())
    }
}

fn validate_absolute_path(path: &Path) -> io::Result<()> {
    let text = path.to_str().ok_or_else(invalid_origin)?;
    if !path.is_absolute()
        || path.file_name().is_none()
        || text.len() > MAX_PATH_BYTES
        || text.chars().any(char::is_control)
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
        || path.components().collect::<PathBuf>().as_os_str() != path.as_os_str()
    {
        return Err(invalid_origin());
    }
    Ok(())
}

fn is_sha256(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid_origin() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid portable origin")
}

fn changed_image() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "portable image identity changed",
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt, symlink};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos();
            let base = fs::canonicalize(std::env::temp_dir()).expect("temp directory");
            let path = base.join(format!(
                "alcomd-portable-origin-{}-{stamp}",
                std::process::id()
            ));
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&path)
                .expect("private test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write_image(path: &Path) {
        fs::write(path, b"portable image fixture").expect("fixture bytes");
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).expect("fixture mode");
    }

    #[test]
    fn origin_round_trip_rejects_a_moved_replaced_or_linked_image() {
        let directory = TestDirectory::new();
        let path = directory.0.join("便携 测试.AppImage");
        write_image(&path);
        let image = PortableImage::open(&path).expect("open fixture");
        let image_digest = "a".repeat(64);
        let manifest_digest = "b".repeat(64);
        let origin = PortableOrigin::bind(&image, &image_digest, &manifest_digest)
            .expect("bind verified fixture");
        fs::write(
            directory.0.join(PORTABLE_ORIGIN_FILE),
            origin.json_bytes().expect("encode portable origin"),
        )
        .expect("store fixture record");
        let origin = PortableOrigin::read(&directory.0).expect("read record");
        assert_eq!(origin.app_image_path, path.as_path());
        assert_eq!(origin.app_image_sha256, image_digest);
        assert_eq!(origin.payload_manifest_sha256(), manifest_digest);
        origin.revalidate().expect("same file identity");

        let moved = directory.0.join("moved.AppImage");
        fs::rename(&path, &moved).expect("move image fixture");
        assert_eq!(
            origin.revalidate().expect_err("missing image").kind(),
            io::ErrorKind::NotFound
        );
        symlink(&moved, &path).expect("substitute a symbolic link");
        assert!(origin.revalidate().is_err());
        fs::remove_file(&path).expect("remove fixture link");
        write_image(&path);
        assert_eq!(
            origin.revalidate().expect_err("replacement image").kind(),
            io::ErrorKind::InvalidData
        );
        assert!(PortableOrigin::bind(&image, &image_digest, &manifest_digest).is_err());
    }

    #[test]
    fn atomic_exchange_preserves_both_images_and_refuses_replaced_stage() -> io::Result<()> {
        let directory = TestDirectory::new();
        let active = directory.0.join("便携 测试.AppImage");
        let stage = directory.0.join("stage");
        fs::DirBuilder::new().mode(0o700).create(&stage)?;
        let staged = stage.join("image.AppImage");
        write_image(&active);
        write_image(&staged);
        fs::write(&staged, b"next portable fixture")?;
        let old = PortableImage::open_for_update(&active)?;
        let original = PortableOrigin::bind(&old, &"a".repeat(64), &"b".repeat(64))?;
        let stale = PortableImage::open_for_update(&staged)?;
        let untouched = stage.join("unknown.AppImage");
        fs::rename(&staged, &untouched)?;
        write_image(&staged);
        assert!(old.exchange_for_update(&stale).is_err());
        assert_eq!(fs::read(&active)?, b"portable image fixture");
        assert_eq!(fs::read(&untouched)?, b"next portable fixture");
        fs::remove_file(&staged)?;
        fs::rename(untouched, &staged)?;
        let next = PortableImage::open_for_update(&staged)?;
        let candidate = PortableOrigin::bind(&next, &"c".repeat(64), &"d".repeat(64))?;
        old.exchange_for_update(&next)?;
        assert!(original.revalidate().is_err());
        assert!(original.identifies_image(&PortableImage::open_for_update(&staged)?));
        assert!(candidate.identifies_image(&PortableImage::open_for_update(&active)?));
        assert_eq!(fs::read(&active)?, b"next portable fixture");
        assert_eq!(fs::read(&staged)?, b"portable image fixture");
        Ok(())
    }

    #[test]
    fn origin_requires_one_bounded_record_with_current_fields() {
        let fixture = serde_json::json!({
            "schema": 1,
            "appImagePath": "/home/fixture/Test.AppImage",
            "appImageSha256": "a".repeat(64),
            "payloadManifestSha256": "b".repeat(64),
            "fileIdentity": {
                "uid": 1000,
                "device": 1,
                "inode": 2,
                "size": 3,
                "ctimeSeconds": 4,
                "ctimeNanoseconds": 5,
            },
        })
        .to_string();
        assert!(PortableOrigin::parse(fixture.as_bytes()).is_ok());
        for malformed in [
            fixture.replace("\"schema\":1", "\"schema\":2"),
            fixture.replace("\"schema\":1", "\"schema\":1,\"schema\":1"),
            fixture.replace("Test.AppImage", "../Test.AppImage"),
            fixture.replace("Test.AppImage", "/Test.AppImage"),
            fixture.replace(&"a".repeat(64), &"A".repeat(64)),
            fixture.replace("\"uid\":1000", "\"uid\":1000,\"extra\":0"),
            format!("{fixture}{{}}"),
            format!("{fixture}{}", " ".repeat(MAX_RECORD_BYTES)),
        ] {
            assert!(PortableOrigin::parse(malformed.as_bytes()).is_err());
        }
    }
}
