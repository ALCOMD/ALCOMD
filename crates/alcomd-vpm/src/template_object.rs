use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

static PARTIAL_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug)]
pub struct TemplateObjectStore {
    root: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateObject {
    pub digest: [u8; 32],
    pub locator: String,
    path: PathBuf,
}

impl TemplateObject {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemplateObjectErrorCode {
    DigestMismatch,
    ObjectMissing,
    TargetExists,
    Io,
}

#[derive(Debug)]
pub struct TemplateObjectError {
    code: TemplateObjectErrorCode,
}

impl TemplateObjectError {
    #[must_use]
    pub const fn code(&self) -> TemplateObjectErrorCode {
        self.code
    }
}

impl fmt::Display for TemplateObjectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Template object operation failed")
    }
}

impl std::error::Error for TemplateObjectError {}

impl TemplateObjectStore {
    pub fn new(root: PathBuf) -> Result<Self, TemplateObjectError> {
        std::fs::create_dir_all(&root).map_err(|_| error(TemplateObjectErrorCode::Io))?;
        let metadata =
            std::fs::symlink_metadata(&root).map_err(|_| error(TemplateObjectErrorCode::Io))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(error(TemplateObjectErrorCode::Io));
        }
        Ok(Self { root })
    }

    pub fn publish(
        &self,
        source: &Path,
        expected_digest: [u8; 32],
    ) -> Result<TemplateObject, TemplateObjectError> {
        let final_path = self.object_path(expected_digest);
        if final_path.exists() {
            verify_file(&final_path, expected_digest)?;
            return Ok(object(expected_digest, final_path));
        }
        let partial = self.partial_path(expected_digest);
        let result = (|| {
            let mut input = File::open(source).map_err(|_| error(TemplateObjectErrorCode::Io))?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&partial)
                .map_err(|_| error(TemplateObjectErrorCode::Io))?;
            let actual = copy_and_hash(&mut input, &mut output)?;
            if actual != expected_digest {
                return Err(error(TemplateObjectErrorCode::DigestMismatch));
            }
            output
                .flush()
                .and_then(|()| output.sync_all())
                .map_err(|_| error(TemplateObjectErrorCode::Io))?;
            drop(output);
            match std::fs::rename(&partial, &final_path) {
                Ok(()) => {}
                Err(_) if final_path.exists() => {
                    verify_file(&final_path, expected_digest)?;
                    std::fs::remove_file(&partial)
                        .map_err(|_| error(TemplateObjectErrorCode::Io))?;
                }
                Err(_) => return Err(error(TemplateObjectErrorCode::Io)),
            }
            alcomd_platform::sync_directory(&self.root)
                .map_err(|_| error(TemplateObjectErrorCode::Io))?;
            verify_file(&final_path, expected_digest)?;
            Ok(object(expected_digest, final_path))
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&partial);
        }
        result
    }

    pub fn open_verified(&self, digest: [u8; 32]) -> Result<TemplateObject, TemplateObjectError> {
        let path = self.object_path(digest);
        if !path.is_file() {
            return Err(error(TemplateObjectErrorCode::ObjectMissing));
        }
        verify_file(&path, digest)?;
        Ok(object(digest, path))
    }

    pub fn export_create_new(
        &self,
        digest: [u8; 32],
        target: &Path,
    ) -> Result<(), TemplateObjectError> {
        let source = self.open_verified(digest)?;
        let mut input =
            File::open(source.path()).map_err(|_| error(TemplateObjectErrorCode::Io))?;
        let mut output = match OpenOptions::new().write(true).create_new(true).open(target) {
            Ok(output) => output,
            Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(error(TemplateObjectErrorCode::TargetExists));
            }
            Err(_) => return Err(error(TemplateObjectErrorCode::Io)),
        };
        let result = (|| {
            let actual = copy_and_hash(&mut input, &mut output)?;
            if actual != digest {
                return Err(error(TemplateObjectErrorCode::DigestMismatch));
            }
            output
                .flush()
                .and_then(|()| output.sync_all())
                .map_err(|_| error(TemplateObjectErrorCode::Io))
        })();
        drop(output);
        if result.is_err() {
            let _ = std::fs::remove_file(target);
        }
        result
    }

    fn object_path(&self, digest: [u8; 32]) -> PathBuf {
        self.root.join(format!("{}.alcomdtemplate", hex(digest)))
    }

    fn partial_path(&self, digest: [u8; 32]) -> PathBuf {
        self.root.join(format!(
            ".{}.partial.{}.{}",
            hex(digest),
            std::process::id(),
            PARTIAL_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }
}

fn object(digest: [u8; 32], path: PathBuf) -> TemplateObject {
    TemplateObject {
        digest,
        locator: format!("sha256:{}", hex(digest)),
        path,
    }
}

fn verify_file(path: &Path, expected: [u8; 32]) -> Result<(), TemplateObjectError> {
    let mut file = File::open(path).map_err(|_| error(TemplateObjectErrorCode::ObjectMissing))?;
    let mut sink = std::io::sink();
    let actual = copy_and_hash(&mut file, &mut sink)?;
    if actual != expected {
        return Err(error(TemplateObjectErrorCode::DigestMismatch));
    }
    Ok(())
}

fn copy_and_hash(
    reader: &mut impl Read,
    writer: &mut impl Write,
) -> Result<[u8; 32], TemplateObjectError> {
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|_| error(TemplateObjectErrorCode::Io))?;
        if read == 0 {
            break;
        }
        writer
            .write_all(&buffer[..read])
            .map_err(|_| error(TemplateObjectErrorCode::Io))?;
        digest.update(&buffer[..read]);
    }
    Ok(digest.finalize().into())
}

fn hex(digest: [u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

const fn error(code: TemplateObjectErrorCode) -> TemplateObjectError {
    TemplateObjectError { code }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn publish_is_content_addressed_verified_and_export_is_create_new() {
        let root = temporary("objects");
        let source = temporary("source");
        let target = temporary("export");
        std::fs::create_dir(&root).expect("root");
        std::fs::write(&source, b"streamed object").expect("source");
        let digest: [u8; 32] = Sha256::digest(b"streamed object").into();
        let store = TemplateObjectStore::new(root.clone()).expect("store");
        let object = store.publish(&source, digest).expect("publish");
        assert_eq!(object.locator, format!("sha256:{}", hex(digest)));
        assert_eq!(store.open_verified(digest).expect("verify"), object);
        store
            .export_create_new(digest, &target)
            .expect("create export");
        assert_eq!(
            std::fs::read(&target).expect("read export"),
            b"streamed object"
        );
        assert_eq!(
            store
                .export_create_new(digest, &target)
                .expect_err("existing target")
                .code(),
            TemplateObjectErrorCode::TargetExists
        );
        std::fs::remove_dir_all(root).expect("remove root");
        std::fs::remove_file(source).expect("remove source");
        std::fs::remove_file(target).expect("remove target");
    }

    #[test]
    fn concurrent_equal_digest_never_publishes_partial_content() {
        let root = temporary("concurrent");
        let source = temporary("concurrent-source");
        std::fs::create_dir(&root).expect("root");
        std::fs::write(&source, vec![7_u8; 256 * 1024]).expect("source");
        let digest: [u8; 32] = Sha256::digest(vec![7_u8; 256 * 1024]).into();
        let store = Arc::new(TemplateObjectStore::new(root.clone()).expect("store"));
        let handles = (0..8)
            .map(|_| {
                let store = Arc::clone(&store);
                let source = source.clone();
                std::thread::spawn(move || store.publish(&source, digest))
            })
            .collect::<Vec<_>>();
        for handle in handles {
            handle.join().expect("join").expect("publish");
        }
        store.open_verified(digest).expect("final object");
        assert_eq!(
            std::fs::read_dir(&root)
                .expect("read objects")
                .filter_map(Result::ok)
                .count(),
            1
        );
        std::fs::remove_dir_all(root).expect("remove root");
        std::fs::remove_file(source).expect("remove source");
    }

    fn temporary(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "alcomd-template-object-{name}-{}-{}",
            std::process::id(),
            PARTIAL_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }
}
