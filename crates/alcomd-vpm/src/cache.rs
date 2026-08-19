use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::header::CONTENT_LENGTH;
use reqwest::redirect::{Action, Attempt, Policy};
use reqwest::{Client, StatusCode, Url};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::archive::MAX_ARCHIVE_BYTES;

pub const MAX_CACHE_BYTES: u64 = 17_179_869_184;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);
const REDIRECT_LIMIT: usize = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CacheErrorCode {
    InvalidDigest,
    InvalidUrl,
    OfflineMiss,
    Corrupt,
    IntegrityMismatch,
    DownloadTooLarge,
    QuotaExceeded,
    DownloadFailed,
    Io,
}

#[derive(Debug)]
pub struct CacheError {
    code: CacheErrorCode,
}

impl CacheError {
    #[must_use]
    pub const fn code(&self) -> CacheErrorCode {
        self.code
    }
}

impl fmt::Display for CacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "package cache request failed: {:?}", self.code)
    }
}

impl std::error::Error for CacheError {}

#[derive(Clone)]
pub struct PackageCache {
    root: PathBuf,
    client: Client,
}

impl PackageCache {
    pub fn new(root: PathBuf) -> Result<Self, CacheError> {
        if !root.is_absolute() {
            return Err(cache_error(CacheErrorCode::Io));
        }
        let client = Client::builder()
            .no_proxy()
            .referer(false)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(DOWNLOAD_TIMEOUT)
            .redirect(Policy::custom(validate_redirect))
            .build()
            .map_err(|_| cache_error(CacheErrorCode::DownloadFailed))?;
        Ok(Self { root, client })
    }

    #[must_use]
    pub fn object_path(&self, digest: &[u8; 32]) -> PathBuf {
        let hex = digest_hex(digest);
        self.root
            .join("sha256")
            .join(&hex[..2])
            .join(format!("{hex}.zip"))
    }

    pub async fn get(
        &self,
        digest: [u8; 32],
        artifact_url: &str,
        offline: bool,
    ) -> Result<PathBuf, CacheError> {
        let object = self.object_path(&digest);
        match verify_object(&object, &digest).await {
            Ok(true) => return Ok(object),
            Ok(false) => {}
            Err(error) if offline => return Err(error),
            Err(_) => {
                tokio::fs::remove_file(&object)
                    .await
                    .map_err(|_| cache_error(CacheErrorCode::Corrupt))?;
            }
        }
        if offline {
            return Err(cache_error(CacheErrorCode::OfflineMiss));
        }
        let url = parse_artifact_url(artifact_url)?;
        self.download(&url, &object, &digest).await?;
        Ok(object)
    }

    async fn download(
        &self,
        url: &Url,
        object: &Path,
        expected_digest: &[u8; 32],
    ) -> Result<(), CacheError> {
        let parent = object
            .parent()
            .ok_or_else(|| cache_error(CacheErrorCode::Io))?;
        prepare_cache_directory(&self.root, parent).await?;
        let part = object.with_extension("zip.part");
        let mut guard = PartialDownload::create(part.clone()).await?;
        let mut response = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(|_| cache_error(CacheErrorCode::DownloadFailed))?;
        if response.status() != StatusCode::OK {
            return Err(cache_error(CacheErrorCode::DownloadFailed));
        }
        if response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .is_some_and(|size| size > MAX_ARCHIVE_BYTES)
        {
            return Err(cache_error(CacheErrorCode::DownloadTooLarge));
        }
        let existing = cache_size(&self.root).await?;
        let mut received = 0_u64;
        let mut hasher = Sha256::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| cache_error(CacheErrorCode::DownloadFailed))?
        {
            received = received
                .checked_add(chunk.len() as u64)
                .filter(|size| *size <= MAX_ARCHIVE_BYTES)
                .ok_or_else(|| cache_error(CacheErrorCode::DownloadTooLarge))?;
            if existing.saturating_add(received) > MAX_CACHE_BYTES {
                return Err(cache_error(CacheErrorCode::QuotaExceeded));
            }
            hasher.update(&chunk);
            guard
                .file
                .write_all(&chunk)
                .await
                .map_err(|_| cache_error(CacheErrorCode::Io))?;
        }
        let actual: [u8; 32] = hasher.finalize().into();
        if &actual != expected_digest {
            return Err(cache_error(CacheErrorCode::IntegrityMismatch));
        }
        guard
            .file
            .flush()
            .await
            .map_err(|_| cache_error(CacheErrorCode::Io))?;
        guard
            .file
            .sync_all()
            .await
            .map_err(|_| cache_error(CacheErrorCode::Io))?;
        if tokio::fs::symlink_metadata(object).await.is_ok() {
            if verify_object(object, expected_digest).await? {
                return Ok(());
            }
            return Err(cache_error(CacheErrorCode::Corrupt));
        }
        tokio::fs::rename(&part, object)
            .await
            .map_err(|_| cache_error(CacheErrorCode::Io))?;
        guard.published = true;
        let parent = parent.to_path_buf();
        tokio::task::spawn_blocking(move || alcomd_platform::sync_directory(&parent))
            .await
            .map_err(|_| cache_error(CacheErrorCode::Io))?
            .map_err(|_| cache_error(CacheErrorCode::Io))?;
        if verify_object(object, expected_digest).await? {
            Ok(())
        } else {
            Err(cache_error(CacheErrorCode::Corrupt))
        }
    }
}

struct PartialDownload {
    path: PathBuf,
    file: tokio::fs::File,
    published: bool,
}

impl PartialDownload {
    async fn create(path: PathBuf) -> Result<Self, CacheError> {
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
            .map_err(|_| cache_error(CacheErrorCode::Io))?;
        Ok(Self {
            path,
            file,
            published: false,
        })
    }
}

impl Drop for PartialDownload {
    fn drop(&mut self) {
        if !self.published {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

async fn verify_object(path: &Path, expected: &[u8; 32]) -> Result<bool, CacheError> {
    let metadata = match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(cache_error(CacheErrorCode::Io)),
    };
    if !metadata.is_file() || is_link_or_reparse(&metadata) || metadata.len() > MAX_ARCHIVE_BYTES {
        return Err(cache_error(CacheErrorCode::Corrupt));
    }
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|_| cache_error(CacheErrorCode::Corrupt))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(|_| cache_error(CacheErrorCode::Corrupt))?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(read as u64)
            .filter(|size| *size <= MAX_ARCHIVE_BYTES)
            .ok_or_else(|| cache_error(CacheErrorCode::Corrupt))?;
        hasher.update(&buffer[..read]);
    }
    let actual: [u8; 32] = hasher.finalize().into();
    if &actual == expected {
        Ok(true)
    } else {
        Err(cache_error(CacheErrorCode::Corrupt))
    }
}

async fn prepare_cache_directory(root: &Path, leaf: &Path) -> Result<(), CacheError> {
    if !root.is_absolute() || !leaf.starts_with(root) {
        return Err(cache_error(CacheErrorCode::Io));
    }
    let mut current = root.to_path_buf();
    if tokio::fs::symlink_metadata(&current).await.is_err() {
        tokio::fs::create_dir(&current)
            .await
            .map_err(|_| cache_error(CacheErrorCode::Io))?;
    }
    validate_directory(&current).await?;
    let relative = leaf
        .strip_prefix(root)
        .map_err(|_| cache_error(CacheErrorCode::Io))?;
    for component in relative.components() {
        current.push(component);
        match tokio::fs::symlink_metadata(&current).await {
            Ok(_) => validate_directory(&current).await?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                tokio::fs::create_dir(&current)
                    .await
                    .map_err(|_| cache_error(CacheErrorCode::Io))?;
                validate_directory(&current).await?;
            }
            Err(_) => return Err(cache_error(CacheErrorCode::Io)),
        }
    }
    Ok(())
}

async fn validate_directory(path: &Path) -> Result<(), CacheError> {
    let metadata = tokio::fs::symlink_metadata(path)
        .await
        .map_err(|_| cache_error(CacheErrorCode::Io))?;
    if metadata.is_dir() && !is_link_or_reparse(&metadata) {
        Ok(())
    } else {
        Err(cache_error(CacheErrorCode::Io))
    }
}

async fn cache_size(root: &Path) -> Result<u64, CacheError> {
    let sha_root = root.join("sha256");
    let mut prefixes = match tokio::fs::read_dir(&sha_root).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(_) => return Err(cache_error(CacheErrorCode::Io)),
    };
    let mut total = 0_u64;
    while let Some(prefix) = prefixes
        .next_entry()
        .await
        .map_err(|_| cache_error(CacheErrorCode::Io))?
    {
        let metadata = tokio::fs::symlink_metadata(prefix.path())
            .await
            .map_err(|_| cache_error(CacheErrorCode::Io))?;
        if !metadata.is_dir() || is_link_or_reparse(&metadata) {
            return Err(cache_error(CacheErrorCode::Io));
        }
        let mut objects = tokio::fs::read_dir(prefix.path())
            .await
            .map_err(|_| cache_error(CacheErrorCode::Io))?;
        while let Some(object) = objects
            .next_entry()
            .await
            .map_err(|_| cache_error(CacheErrorCode::Io))?
        {
            let metadata = tokio::fs::symlink_metadata(object.path())
                .await
                .map_err(|_| cache_error(CacheErrorCode::Io))?;
            if !metadata.is_file() || is_link_or_reparse(&metadata) {
                return Err(cache_error(CacheErrorCode::Io));
            }
            if object
                .path()
                .extension()
                .is_some_and(|value| value == "zip")
            {
                total = total
                    .checked_add(metadata.len())
                    .filter(|size| *size <= MAX_CACHE_BYTES)
                    .ok_or_else(|| cache_error(CacheErrorCode::QuotaExceeded))?;
            }
        }
    }
    Ok(total)
}

fn parse_artifact_url(value: &str) -> Result<Url, CacheError> {
    let mut url = Url::parse(value).map_err(|_| cache_error(CacheErrorCode::InvalidUrl))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(cache_error(CacheErrorCode::InvalidUrl));
    }
    url.set_fragment(None);
    Ok(url)
}

fn validate_redirect(attempt: Attempt<'_>) -> Action {
    if attempt.previous().len() >= REDIRECT_LIMIT {
        return attempt.error("redirect limit exceeded");
    }
    let next = attempt.url();
    if !matches!(next.scheme(), "http" | "https")
        || !next.username().is_empty()
        || next.password().is_some()
        || attempt
            .previous()
            .last()
            .is_some_and(|previous| previous.scheme() == "https" && next.scheme() == "http")
    {
        return attempt.error("redirect rejected");
    }
    attempt.follow()
}

#[cfg(windows)]
fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
}

#[cfg(unix)]
fn is_link_or_reparse(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn digest_hex(digest: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for byte in digest {
        result.push(char::from(HEX[(byte >> 4) as usize]));
        result.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    result
}

const fn cache_error(code: CacheErrorCode) -> CacheError {
    CacheError { code }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "alcomd-m4-cache-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    #[tokio::test]
    async fn valid_offline_object_is_rehashed_and_returned() {
        let root = temporary_root("valid");
        let cache = PackageCache::new(root.clone()).expect("cache");
        let digest: [u8; 32] = Sha256::digest(b"fixture").into();
        let object = cache.object_path(&digest);
        tokio::fs::create_dir_all(object.parent().expect("parent"))
            .await
            .expect("directory");
        tokio::fs::write(&object, b"fixture").await.expect("object");
        assert_eq!(
            cache
                .get(digest, "https://ignored.invalid/package.zip", true)
                .await
                .expect("offline hit"),
            object
        );
        tokio::fs::remove_dir_all(root).await.expect("cleanup");
    }

    #[tokio::test]
    async fn corrupt_offline_object_fails_instead_of_becoming_a_miss() {
        let root = temporary_root("corrupt");
        let cache = PackageCache::new(root.clone()).expect("cache");
        let digest = [1_u8; 32];
        let object = cache.object_path(&digest);
        tokio::fs::create_dir_all(object.parent().expect("parent"))
            .await
            .expect("directory");
        tokio::fs::write(&object, b"corrupt").await.expect("object");
        assert_eq!(
            cache
                .get(digest, "https://ignored.invalid/package.zip", true)
                .await
                .expect_err("corrupt")
                .code(),
            CacheErrorCode::Corrupt
        );
        tokio::fs::remove_dir_all(root).await.expect("cleanup");
    }

    #[tokio::test]
    async fn missing_offline_object_is_a_stable_miss() {
        let root = temporary_root("missing");
        let cache = PackageCache::new(root.clone()).expect("cache");
        assert_eq!(
            cache
                .get(
                    [2_u8; 32],
                    "https://network-must-not-be-used.invalid/package.zip",
                    true,
                )
                .await
                .expect_err("offline miss")
                .code(),
            CacheErrorCode::OfflineMiss
        );
    }

    #[tokio::test]
    async fn partial_download_claim_is_exclusive_and_unpublished_file_is_removed() {
        let root = temporary_root("partial");
        tokio::fs::create_dir_all(&root).await.expect("directory");
        let part = root.join("object.zip.part");
        let first = PartialDownload::create(part.clone())
            .await
            .expect("first claim");
        let second_error = match PartialDownload::create(part.clone()).await {
            Ok(_) => panic!("second claim must not overwrite"),
            Err(error) => error,
        };
        assert_eq!(second_error.code(), CacheErrorCode::Io);
        drop(first);
        assert!(!part.exists());
        drop(
            PartialDownload::create(part)
                .await
                .expect("claim after cleanup"),
        );
        tokio::fs::remove_dir_all(root).await.expect("cleanup");
    }

    #[test]
    fn cache_key_and_url_policy_are_fixed() {
        let cache = PackageCache::new(temporary_root("key")).expect("cache");
        assert!(cache.object_path(&[0xab; 32]).ends_with(Path::new(
            "sha256/ab/abababababababababababababababababababababababababababababababab.zip"
        )));
        assert!(parse_artifact_url("file:///package.zip").is_err());
        assert!(parse_artifact_url("https://token@example.invalid/package.zip").is_err());
        assert_eq!(
            parse_artifact_url("https://example.invalid/package.zip#ignored")
                .expect("URL")
                .as_str(),
            "https://example.invalid/package.zip"
        );
    }
}
