//! File identity and the per-user origin record for an installed AppImage runtime.

use std::fs::File;
use std::io::{self, Read};
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

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
