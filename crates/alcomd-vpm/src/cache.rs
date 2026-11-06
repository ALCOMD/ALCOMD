use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use alcomd_application::{ResourceKey, ResourceLockCoordinator};
use reqwest::header::CONTENT_LENGTH;
use reqwest::redirect::{Action, Attempt, Policy};
use reqwest::{Client, StatusCode, Url};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

use crate::archive::MAX_ARCHIVE_BYTES;

#[path = "legacy_manifest_cache.rs"]
mod legacy_manifest;
pub(super) use legacy_manifest::read_archive_legacy;

pub const MAX_CACHE_BYTES: u64 = 17_179_869_184;
pub(crate) const PACKAGE_DOWNLOAD_CONCURRENCY: usize = crate::HTTP_DOWNLOAD_CONCURRENCY;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);
const REDIRECT_LIMIT: usize = 10;
static CACHE_PUBLICATION: Mutex<()> = Mutex::const_new(());
static CACHE_OBJECT_LOCKS: OnceLock<ResourceLockCoordinator> = OnceLock::new();

#[derive(Clone, Copy, Default)]
pub(crate) struct ArchiveHeaders<'a> {
    pub(crate) repository: Option<&'a str>,
    pub(crate) package: Option<PackageHeaderLookup<'a>>,
}

#[derive(Clone, Copy)]
pub(crate) struct PackageHeaderLookup<'a> {
    pub(crate) source_identity: &'a str,
    pub(crate) local_path: Option<&'a str>,
    pub(crate) package_id: &'a str,
    pub(crate) version: &'a str,
    pub(crate) fingerprint: &'a [u8; 32],
}

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
    context: alcomd_application::TechnicalErrorContext,
}

impl CacheError {
    fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.context = self.context.with_reason(reason);
        self
    }

    #[must_use]
    pub fn with_source(mut self, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        self.context = self.context.with_source(source);
        self
    }
    #[must_use]
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.context = self.context.with_path(path);
        self
    }
    #[must_use]
    pub fn path(&self) -> Option<&str> {
        self.context.path.as_deref()
    }

    #[must_use]
    pub const fn code(&self) -> CacheErrorCode {
        self.code
    }
}

impl fmt::Display for CacheError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "package cache request failed: {:?}", self.code)?;
        if let Some(path) = self.context.path.as_deref() {
            write!(formatter, " at {path}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CacheError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.context.source_error()
    }
}

#[derive(Clone)]
pub struct PackageCache {
    root: PathBuf,
    client: Client,
    authenticated_client: Client,
}

impl PackageCache {
    pub fn new(root: PathBuf) -> Result<Self, CacheError> {
        if !root.is_absolute() {
            return Err(cache_error(CacheErrorCode::Io));
        }
        let client = Client::builder()
            .referer(false)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(DOWNLOAD_TIMEOUT)
            .redirect(Policy::custom(validate_redirect))
            .build()
            .map_err(|cause| {
                cache_error(CacheErrorCode::DownloadFailed).with_source(cause.without_url())
            })?;
        let authenticated_client =
            crate::repository_auth::client(DOWNLOAD_TIMEOUT, CONNECT_TIMEOUT)
                .map_err(|_| cache_error(CacheErrorCode::DownloadFailed))?;
        Ok(Self {
            root,
            client,
            authenticated_client,
        })
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
        self.get_with_repair(
            digest,
            artifact_url,
            offline,
            true,
            ArchiveHeaders::default(),
        )
        .await
    }

    /// Apply can fetch a missing pinned object, but a corrupt object is an
    /// integrity failure that must be reported before project mutation.
    pub(crate) async fn get_for_apply(
        &self,
        digest: [u8; 32],
        artifact_url: &str,
        offline: bool,
        headers: ArchiveHeaders<'_>,
    ) -> Result<PathBuf, CacheError> {
        self.get_with_repair(digest, artifact_url, offline, false, headers)
            .await
    }

    async fn get_with_repair(
        &self,
        digest: [u8; 32],
        artifact_url: &str,
        offline: bool,
        repair_corrupt: bool,
        headers: ArchiveHeaders<'_>,
    ) -> Result<PathBuf, CacheError> {
        let _object_guard = CACHE_OBJECT_LOCKS
            .get_or_init(ResourceLockCoordinator::default)
            .acquire(vec![ResourceKey::PackageCache(digest)])
            .await;
        let object = self.object_path(&digest);
        match verify_object(&object, &digest).await {
            Ok(true) => return Ok(object),
            Ok(false) => {}
            Err(error) if offline || !repair_corrupt => return Err(error),
            Err(_) => {
                let _publication = CACHE_PUBLICATION.lock().await;
                tokio::fs::remove_file(&object)
                    .await
                    .map_err(|cause| cache_error(CacheErrorCode::Corrupt).with_source(cause))?;
            }
        }
        if offline {
            return Err(cache_error(CacheErrorCode::OfflineMiss));
        }
        let url = parse_artifact_url(artifact_url)?;
        self.download(&url, &object, &digest, headers).await?;
        Ok(object)
    }

    /// Acquires a selected archive that has no publisher checksum.
    /// The caller must bind the returned digest to the observed package source.
    pub(crate) async fn download_first(
        &self,
        artifact_url: &str,
        package_id: &str,
        version: &str,
        headers: ArchiveHeaders<'_>,
    ) -> Result<[u8; 32], CacheError> {
        let url = parse_artifact_url(artifact_url)?;
        let staging = self.root.join("incoming");
        prepare_cache_directory(&self.root, &staging).await?;
        let path = staging.join(format!(
            "{}.zip.part",
            alcomd_application::OperationId::new()
        ));
        let mut partial = PartialDownload::create(path).await?;
        let (digest, _) = self.download_into(&url, &mut partial, headers).await?;
        self.finish_first_download(&mut partial, digest, package_id, version)
            .await?;
        Ok(digest)
    }

    async fn finish_first_download(
        &self,
        partial: &mut PartialDownload,
        digest: [u8; 32],
        package_id: &str,
        version: &str,
    ) -> Result<(), CacheError> {
        partial.sync().await?;
        let path = partial.path.clone();
        let package_id = package_id.to_owned();
        let version = version.to_owned();
        tokio::task::spawn_blocking(move || {
            crate::preflight_archive(&path)
                .map_err(|cause| cache_error(CacheErrorCode::Corrupt).with_source(cause))?;
            legacy_manifest::validate_archive_manifest_identity(&path, &package_id, &version)
        })
        .await
        .map_err(|cause| cache_error(CacheErrorCode::Io).with_source(cause))??;
        self.publish_owned(&partial.path, digest).await?;
        Ok(())
    }

    /// Atomically publishes an already validated ALCOMD-owned archive.
    pub async fn publish_owned(
        &self,
        source: &Path,
        digest: [u8; 32],
    ) -> Result<PathBuf, CacheError> {
        let _object_guard = CACHE_OBJECT_LOCKS
            .get_or_init(ResourceLockCoordinator::default)
            .acquire(vec![ResourceKey::PackageCache(digest)])
            .await;
        let source_metadata = tokio::fs::symlink_metadata(source).await.map_err(|cause| {
            cache_error(CacheErrorCode::Io)
                .with_source(cause)
                .with_path(source.display().to_string())
        })?;
        if !source_metadata.is_file()
            || is_link_or_reparse(&source_metadata)
            || source_metadata.len() > MAX_ARCHIVE_BYTES
        {
            return Err(cache_error(CacheErrorCode::QuotaExceeded));
        }
        if !verify_object(source, &digest).await? {
            return Err(cache_error(CacheErrorCode::IntegrityMismatch));
        }
        let object = self.object_path(&digest);
        match verify_object(&object, &digest).await {
            Ok(true) => {
                tokio::fs::remove_file(source).await.map_err(|cause| {
                    cache_error(CacheErrorCode::Io)
                        .with_source(cause)
                        .with_path(source.display().to_string())
                })?;
                return Ok(object);
            }
            Ok(false) => {}
            Err(error) => return Err(error),
        }
        let parent = object
            .parent()
            .ok_or_else(|| cache_error(CacheErrorCode::Io))?;
        prepare_cache_directory(&self.root, parent).await?;
        let publication = CACHE_PUBLICATION.lock().await;
        ensure_cache_capacity(&self.root, source_metadata.len()).await?;
        tokio::fs::rename(source, &object).await.map_err(|cause| {
            cache_error(CacheErrorCode::Io)
                .with_source(cause)
                .with_path(source.display().to_string())
        })?;
        drop(publication);
        let parent = parent.to_path_buf();
        tokio::task::spawn_blocking(move || alcomd_platform::sync_directory(&parent))
            .await
            .map_err(|cause| {
                cache_error(CacheErrorCode::Io)
                    .with_source(cause)
                    .with_path(source.display().to_string())
            })?
            .map_err(|cause| {
                cache_error(CacheErrorCode::Io)
                    .with_source(cause)
                    .with_path(source.display().to_string())
            })?;
        if verify_object(&object, &digest).await? {
            Ok(object)
        } else {
            Err(cache_error(CacheErrorCode::Corrupt))
        }
    }

    async fn download(
        &self,
        url: &Url,
        object: &Path,
        expected_digest: &[u8; 32],
        headers: ArchiveHeaders<'_>,
    ) -> Result<(), CacheError> {
        let parent = object
            .parent()
            .ok_or_else(|| cache_error(CacheErrorCode::Io))?;
        prepare_cache_directory(&self.root, parent).await?;
        let part = object.with_extension("zip.part");
        let mut guard = PartialDownload::create(part.clone()).await?;
        let (actual, archive_bytes) = self.download_into(url, &mut guard, headers).await?;
        if &actual != expected_digest {
            return Err(cache_error(CacheErrorCode::IntegrityMismatch));
        }
        guard.sync().await?;
        if tokio::fs::symlink_metadata(object).await.is_ok() {
            if verify_object(object, expected_digest).await? {
                return Ok(());
            }
            return Err(cache_error(CacheErrorCode::Corrupt));
        }
        let publication = CACHE_PUBLICATION.lock().await;
        ensure_cache_capacity(&self.root, archive_bytes).await?;
        tokio::fs::rename(&part, object).await.map_err(|cause| {
            cache_error(CacheErrorCode::Io)
                .with_source(cause)
                .with_path(object.display().to_string())
        })?;
        guard.published = true;
        drop(publication);
        let parent = parent.to_path_buf();
        tokio::task::spawn_blocking(move || alcomd_platform::sync_directory(&parent))
            .await
            .map_err(|cause| {
                cache_error(CacheErrorCode::Io)
                    .with_source(cause)
                    .with_path(object.display().to_string())
            })?
            .map_err(|cause| {
                cache_error(CacheErrorCode::Io)
                    .with_source(cause)
                    .with_path(object.display().to_string())
            })?;
        if verify_object(object, expected_digest).await? {
            Ok(())
        } else {
            Err(cache_error(CacheErrorCode::Corrupt))
        }
    }

    async fn download_into(
        &self,
        url: &Url,
        partial: &mut PartialDownload,
        headers: ArchiveHeaders<'_>,
    ) -> Result<([u8; 32], u64), CacheError> {
        let package = match headers.package {
            Some(lookup) => Some(
                self.read_package_download_headers(lookup, headers.repository, url)
                    .await?,
            ),
            None => None,
        };
        let _transfer = crate::HTTP_DOWNLOADS
            .acquire()
            .await
            .map_err(|cause| cache_error(CacheErrorCode::DownloadFailed).with_source(cause))?;
        let mut response = self
            .get_response(url, headers.repository, package.as_ref())
            .await?;
        if response.status() != StatusCode::OK {
            return Err(
                cache_error(CacheErrorCode::DownloadFailed).with_reason(format!(
                    "HTTP response status {}",
                    response.status().as_u16()
                )),
            );
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
        let existing = {
            let _publication = CACHE_PUBLICATION.lock().await;
            cache_size(&self.root).await?
        };
        let mut received = 0_u64;
        let mut hasher = Sha256::new();
        while let Some(chunk) = response.chunk().await.map_err(|cause| {
            cache_error(CacheErrorCode::DownloadFailed).with_source(cause.without_url())
        })? {
            received = received
                .checked_add(chunk.len() as u64)
                .filter(|size| *size <= MAX_ARCHIVE_BYTES)
                .ok_or_else(|| cache_error(CacheErrorCode::DownloadTooLarge))?;
            if existing.saturating_add(received) > MAX_CACHE_BYTES {
                return Err(cache_error(CacheErrorCode::QuotaExceeded));
            }
            hasher.update(&chunk);
            partial.file.write_all(&chunk).await.map_err(|cause| {
                cache_error(CacheErrorCode::Io)
                    .with_source(cause)
                    .with_path(partial.path.display().to_string())
            })?;
        }
        Ok((hasher.finalize().into(), received))
    }

    async fn get_response(
        &self,
        url: &Url,
        credential_reference: Option<&str>,
        package: Option<&crate::repository_auth::RepositoryAuth>,
    ) -> Result<reqwest::Response, CacheError> {
        let repository = if let Some(reference) = credential_reference {
            let credentials = crate::repository_auth::load(reference.to_owned())
                .await
                .map_err(|cause| cache_error(CacheErrorCode::DownloadFailed).with_source(cause))?;
            Some(
                crate::repository_auth::RepositoryAuth::new(&credentials).map_err(|cause| {
                    cache_error(CacheErrorCode::DownloadFailed).with_source(cause)
                })?,
            )
        } else {
            None
        };
        if repository.is_some() || package.is_some() {
            let layers = [repository.as_ref(), package]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>();
            return crate::repository_auth::send(url.clone(), &layers, |url| {
                self.authenticated_client.get(url)
            })
            .await
            .map_err(|cause| cache_error(CacheErrorCode::DownloadFailed).with_source(cause));
        }
        self.client.get(url.clone()).send().await.map_err(|cause| {
            cache_error(CacheErrorCode::DownloadFailed).with_source(cause.without_url())
        })
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
            .map_err(|cause| cache_error(CacheErrorCode::Io).with_source(cause))?;
        Ok(Self {
            path,
            file,
            published: false,
        })
    }

    async fn sync(&mut self) -> Result<(), CacheError> {
        let error = |cause| {
            cache_error(CacheErrorCode::Io)
                .with_source(cause)
                .with_path(self.path.display().to_string())
        };
        self.file.flush().await.map_err(error)?;
        self.file.sync_all().await.map_err(error)
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
    let mut file = tokio::fs::File::open(path).await.map_err(|cause| {
        cache_error(CacheErrorCode::Corrupt)
            .with_source(cause)
            .with_path(path.display().to_string())
    })?;
    let mut hasher = Sha256::new();
    // This buffer survives reads across await. Keep it off the nested Plan/RPC
    // futures so their construction and polling fit the normal worker stack.
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        let read = file.read(&mut buffer).await.map_err(|cause| {
            cache_error(CacheErrorCode::Corrupt)
                .with_source(cause)
                .with_path(path.display().to_string())
        })?;
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
    create_cache_directory(&current).await?;
    let relative = leaf
        .strip_prefix(root)
        .map_err(|cause| cache_error(CacheErrorCode::Io).with_source(cause))?;
    for component in relative.components() {
        current.push(component);
        create_cache_directory(&current).await?;
    }
    Ok(())
}

async fn create_cache_directory(path: &Path) -> Result<(), CacheError> {
    let error = |cause| {
        cache_error(CacheErrorCode::Io)
            .with_source(cause)
            .with_path(path.display().to_string())
    };
    let metadata = match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) => metadata,
        Err(cause) if cause.kind() == std::io::ErrorKind::NotFound => {
            if let Err(cause) = tokio::fs::create_dir(path).await
                && cause.kind() != std::io::ErrorKind::AlreadyExists
            {
                return Err(error(cause));
            }
            tokio::fs::symlink_metadata(path).await.map_err(error)?
        }
        Err(cause) => return Err(error(cause)),
    };
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
    while let Some(prefix) = prefixes.next_entry().await.map_err(|cause| {
        cache_error(CacheErrorCode::Io)
            .with_source(cause)
            .with_path(root.display().to_string())
    })? {
        let metadata = tokio::fs::symlink_metadata(prefix.path())
            .await
            .map_err(|cause| {
                cache_error(CacheErrorCode::Io)
                    .with_source(cause)
                    .with_path(root.display().to_string())
            })?;
        if !metadata.is_dir() || is_link_or_reparse(&metadata) {
            return Err(cache_error(CacheErrorCode::Io));
        }
        let mut objects = tokio::fs::read_dir(prefix.path()).await.map_err(|cause| {
            cache_error(CacheErrorCode::Io)
                .with_source(cause)
                .with_path(root.display().to_string())
        })?;
        while let Some(object) = objects.next_entry().await.map_err(|cause| {
            cache_error(CacheErrorCode::Io)
                .with_source(cause)
                .with_path(root.display().to_string())
        })? {
            let path = object.path();
            let counted = path.extension().is_some_and(|value| value == "zip")
                || object
                    .file_name()
                    .to_str()
                    .is_some_and(|value| value.ends_with(".manifest.json"));
            let metadata = match tokio::fs::symlink_metadata(path).await {
                Ok(metadata) => metadata,
                Err(cause) if !counted && cause.kind() == std::io::ErrorKind::NotFound => continue,
                Err(cause) => {
                    return Err(cache_error(CacheErrorCode::Io)
                        .with_source(cause)
                        .with_path(root.display().to_string()));
                }
            };
            if !metadata.is_file() || is_link_or_reparse(&metadata) {
                return Err(cache_error(CacheErrorCode::Io));
            }
            if counted {
                total = total
                    .checked_add(metadata.len())
                    .filter(|size| *size <= MAX_CACHE_BYTES)
                    .ok_or_else(|| cache_error(CacheErrorCode::QuotaExceeded))?;
            }
        }
    }
    Ok(total)
}

async fn ensure_cache_capacity(root: &Path, bytes: u64) -> Result<(), CacheError> {
    if cache_size(root).await?.saturating_add(bytes) > MAX_CACHE_BYTES {
        Err(cache_error(CacheErrorCode::QuotaExceeded))
    } else {
        Ok(())
    }
}

fn parse_artifact_url(value: &str) -> Result<Url, CacheError> {
    let mut url = Url::parse(value)
        .map_err(|cause| cache_error(CacheErrorCode::InvalidUrl).with_source(cause))?;
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

fn cache_error(code: CacheErrorCode) -> CacheError {
    CacheError {
        code,
        context: alcomd_application::TechnicalErrorContext::default(),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

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

    fn package_archive(unsafe_entry: bool) -> Vec<u8> {
        let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        archive
            .start_file("package.json", options)
            .expect("manifest entry");
        archive
            .write_all(br#"{"name":"com.example.fixture","version":"1.0.0"}"#)
            .expect("manifest contents");
        if unsafe_entry {
            archive
                .start_file("../outside.txt", options)
                .expect("unsafe entry");
            archive.write_all(b"fixture").expect("entry contents");
        }
        archive.finish().expect("archive").into_inner()
    }

    #[tokio::test]
    async fn first_download_rejects_non_http_url_before_allocating_staging() {
        let root = temporary_root("first-url");
        let cache = PackageCache::new(root.clone()).expect("cache");
        assert_eq!(
            cache
                .download_first(
                    "file:///package.zip",
                    "com.example.fixture",
                    "1.0.0",
                    ArchiveHeaders::default(),
                )
                .await
                .expect_err("non-HTTP(S) archive is rejected")
                .code(),
            CacheErrorCode::InvalidUrl
        );
        assert!(!root.exists());
    }

    #[cfg(windows)]
    #[tokio::test]
    #[ignore = "requires the isolated HTTPS and HTTP repository fixture"]
    async fn authenticated_index_and_archive_follow_v3_redirects_and_keep_digest_guards() {
        use alcomd_application::{
            M3ReadAdapter, RepositoryCredentials, RepositoryListFiles, RepositoryListRecord,
            RepositoryListSnapshot, RepositorySource,
        };
        use std::collections::BTreeMap;

        let first = std::env::var("ALCOMD_AUTH_TEST_ORIGIN_A").expect("first fixture origin");
        let http = std::env::var("ALCOMD_AUTH_TEST_ORIGIN_HTTP").expect("HTTP fixture origin");
        let credentials = RepositoryCredentials {
            headers: BTreeMap::from([
                (
                    "X-Repository-Token".to_owned(),
                    "isolated-repository-token".to_owned(),
                ),
                (
                    "Authorization".to_owned(),
                    "Bearer fixture-authorization".to_owned(),
                ),
                ("Cookie".to_owned(), "session=fixture-cookie".to_owned()),
            ]),
        };
        let client = Client::builder()
            .no_proxy()
            .referer(false)
            .danger_accept_invalid_certs(true)
            .redirect(Policy::none())
            .timeout(Duration::from_secs(5))
            .build()
            .expect("isolated loopback TLS client");
        let mut reader = crate::VpmReader::new().expect("repository reader");
        reader.authenticated_client = client.clone();
        let source = |path| RepositorySource::Remote {
            url: format!("{first}/{path}"),
        };
        for path in [
            "index",
            "same-origin",
            "cross-origin",
            "downgrade",
            "roundtrip",
        ] {
            reader
                .inspect_repository_authenticated(source(path), None, Some(credentials.clone()))
                .await
                .unwrap_or_else(|error| panic!("fixture route {path}: {error:?}"));
        }
        reader
            .inspect_repository_authenticated(
                RepositorySource::Remote {
                    url: format!("{http}/index"),
                },
                None,
                Some(credentials.clone()),
            )
            .await
            .expect("an initial HTTP request carries both standard and custom headers");
        assert!(
            reader
                .inspect_repository_authenticated(
                    source("reflect"),
                    None,
                    Some(credentials.clone())
                )
                .await
                .is_err(),
            "a reflected credential must not enter the index snapshot"
        );

        struct VaultCleanup(String);
        impl Drop for VaultCleanup {
            fn drop(&mut self) {
                let _ = alcomd_platform::delete_credential(&self.0);
            }
        }
        let reference = VaultCleanup(alcomd_application::OperationId::new().to_string());
        crate::repository_auth::save(reference.0.clone(), credentials.clone())
            .await
            .expect("store test credential");
        let root = temporary_root("authenticated-https");
        let mut cache = PackageCache::new(root.clone()).expect("package cache");
        cache.authenticated_client = client.clone();
        let digest = cache
            .download_first(
                &format!("{first}/archive.zip"),
                "com.example.fixture",
                "1.0.0",
                ArchiveHeaders {
                    repository: Some(&reference.0),
                    package: None,
                },
            )
            .await
            .expect("authenticated hashless archive download");
        assert!(cache.object_path(&digest).is_file());
        let files = crate::repository_list::RepositoryListFileAdapter::new(root.clone());
        let export = files
            .prepare(vec![RepositoryListRecord {
                repository_id: "fixture".to_owned(),
                revision: 1,
                source: source("index"),
                credential_reference: Some(reference.0.clone()),
            }])
            .await
            .expect("read isolated vault for export");
        let list_path = root.with_extension("txt");
        let list_path_text = list_path.to_string_lossy().into_owned();
        files
            .create_new(list_path_text.clone(), export.bytes, Vec::new())
            .await
            .expect("write a separate isolated export");
        let imported = files
            .inspect(list_path_text, RepositoryListSnapshot::default())
            .await
            .expect("reimport isolated export");
        assert_eq!(imported.entries[0].credentials.as_ref(), Some(&credentials));
        std::fs::remove_file(list_path).expect("remove owned list fixture");
        for url in [
            format!("{first}/archive-downgrade"),
            format!("{http}/initial-archive.zip"),
        ] {
            assert_eq!(
                cache
                    .download_first(
                        &url,
                        "com.example.fixture",
                        "1.0.0",
                        ArchiveHeaders {
                            repository: Some(&reference.0),
                            package: None
                        },
                    )
                    .await
                    .expect("HTTP hashless archive is verified and pinned"),
                digest,
            );
        }

        let alcomd_application::RepositoryReadOutcome::Fresh(index) = reader
            .inspect_repository_authenticated(source("package-index"), None, Some(credentials))
            .await
            .expect("index with package-supplied headers")
        else {
            panic!("fresh fixture index");
        };
        let metadata = index.packages[0]
            .resolver
            .as_ref()
            .expect("resolver metadata");
        assert!(metadata.download_headers_present);
        let fingerprint: [u8; 32] = metadata
            .manifest_fingerprint
            .as_slice()
            .try_into()
            .expect("manifest fingerprint");
        let source_identity = format!("remote:{first}/package-index");
        let lookup = PackageHeaderLookup {
            source_identity: &source_identity,
            local_path: None,
            package_id: "com.example.fixture",
            version: "1.0.0",
            fingerprint: &fingerprint,
        };
        let headers = ArchiveHeaders {
            repository: Some(&reference.0),
            package: Some(lookup),
        };
        let selected = cache
            .download_first(
                &metadata.artifact_url,
                lookup.package_id,
                lookup.version,
                headers,
            )
            .await
            .expect("package overrides reach the ZIP host and custom headers survive redirects");
        assert_eq!(selected, digest);

        let recovery_root = temporary_root("package-headers-recovery");
        let mut recovery = PackageCache::new(recovery_root.clone()).expect("empty recovery cache");
        recovery.authenticated_client = client.clone();
        let bad_fingerprint = [0; 32];
        let stale = ArchiveHeaders {
            package: Some(PackageHeaderLookup {
                fingerprint: &bad_fingerprint,
                ..lookup
            }),
            ..headers
        };
        assert_eq!(
            recovery
                .get_for_apply(selected, &metadata.artifact_url, false, stale)
                .await
                .expect_err("changed declaration must never authorize a download")
                .code(),
            CacheErrorCode::IntegrityMismatch
        );
        let recovered = recovery
            .get_for_apply(selected, &metadata.artifact_url, false, headers)
            .await
            .expect("cold pinned archive download");
        assert!(recovered.is_file());
        let http_root = temporary_root("checksummed-http");
        let mut checksummed = PackageCache::new(http_root.clone()).expect("empty HTTP cache");
        checksummed.authenticated_client = client;
        assert!(
            checksummed
                .get_for_apply(
                    digest,
                    &format!("{first}/archive-downgrade"),
                    false,
                    ArchiveHeaders {
                        repository: Some(&reference.0),
                        package: None
                    },
                )
                .await
                .expect("checksummed download may follow the HTTP redirect")
                .is_file()
        );
        std::fs::remove_dir_all(http_root).expect("remove owned HTTP cache fixture");
        std::fs::remove_dir_all(recovery_root).expect("remove owned recovery cache fixture");

        crate::repository_auth::delete(reference.0.clone())
            .await
            .expect("remove test credential");
        assert!(
            crate::repository_auth::load(reference.0.clone())
                .await
                .is_err()
        );
        std::fs::remove_dir_all(root).expect("remove owned cache fixture");
    }

    #[tokio::test]
    #[ignore = "requires a test process with HTTP_PROXY set to its loopback fixture"]
    async fn repository_and_package_clients_use_the_default_http_proxy() {
        use alcomd_application::{M3ReadAdapter, RepositorySource};

        let remote = "http://v3-alignment-proxy.invalid";
        let reader = crate::VpmReader::new().expect("repository reader");
        reader
            .inspect_repository(
                RepositorySource::Remote {
                    url: format!("{remote}/index"),
                },
                None,
            )
            .await
            .expect("registry request uses the configured proxy");
        let root = temporary_root("v3-http-proxy");
        let cache = PackageCache::new(root.clone()).expect("cache");
        let digest = cache
            .download_first(
                &format!("{remote}/archive.zip"),
                "com.example.fixture",
                "1.0.0",
                ArchiveHeaders::default(),
            )
            .await
            .expect("first archive download uses the same proxy and verifies the ZIP");
        assert!(cache.object_path(&digest).is_file());
        std::fs::remove_dir_all(root).expect("remove owned proxy fixture");
    }

    #[tokio::test]
    async fn first_download_only_publishes_a_safe_archive_with_the_selected_identity() {
        let root = temporary_root("first-zip");
        let cache = PackageCache::new(root.clone()).expect("cache");
        let staging = root.join("incoming");
        prepare_cache_directory(&root, &staging)
            .await
            .expect("staging");
        let part = staging.join("fixture.zip.part");
        for (unsafe_entry, version, error) in [
            (false, "2.0.0", Some(CacheErrorCode::IntegrityMismatch)),
            (true, "1.0.0", Some(CacheErrorCode::Corrupt)),
            (false, "1.0.0", None),
        ] {
            let bytes = package_archive(unsafe_entry);
            let digest: [u8; 32] = Sha256::digest(&bytes).into();
            let mut partial = PartialDownload::create(part.clone())
                .await
                .expect("staged archive");
            partial
                .file
                .write_all(&bytes)
                .await
                .expect("downloaded bytes");
            let result = cache
                .finish_first_download(&mut partial, digest, "com.example.fixture", version)
                .await;
            drop(partial);
            assert!(!part.exists());
            if let Some(error) = error {
                assert_eq!(result.expect_err("archive must be rejected").code(), error);
                assert!(!cache.object_path(&digest).exists());
            } else {
                result.expect("valid archive is published");
                assert_eq!(
                    cache.get(digest, "", true).await.expect("rehash hit"),
                    cache.object_path(&digest)
                );
                tokio::fs::write(cache.object_path(&digest), b"modified")
                    .await
                    .expect("tamper cache fixture");
                assert_eq!(
                    cache
                        .get_for_apply(digest, "", false, ArchiveHeaders::default())
                        .await
                        .expect_err("tampered cache is rejected online")
                        .code(),
                    CacheErrorCode::Corrupt
                );
                assert!(cache.object_path(&digest).exists());
            }
        }
        tokio::fs::remove_dir_all(root).await.expect("cleanup");
    }

    #[tokio::test]
    async fn online_apply_miss_requires_the_pinned_digest_before_publication() {
        let root = temporary_root("apply-verified");
        let cache = PackageCache::new(root.clone()).expect("cache");
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("fixture listener");
        let url = format!(
            "http://{}/package.zip",
            listener.local_addr().expect("address")
        );
        let body = b"archive fixture";
        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (stream, _) = listener.accept().await.expect("request");
                use tokio::io::AsyncBufReadExt as _;
                let mut reader = tokio::io::BufReader::new(stream);
                let mut header = String::new();
                while reader.read_line(&mut header).await.expect("header") > 0 && header != "\r\n" {
                    header.clear();
                }
                let mut stream = reader.into_inner();
                stream
                    .write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        )
                        .as_bytes(),
                    )
                    .await
                    .expect("response header");
                stream.write_all(body).await.expect("response body");
            }
        });
        let unexpected = [1_u8; 32];
        assert_eq!(
            tokio::time::timeout(
                Duration::from_secs(5),
                cache.get_for_apply(unexpected, &url, false, ArchiveHeaders::default())
            )
            .await
            .expect("response")
            .expect_err("unexpected bytes")
            .code(),
            CacheErrorCode::IntegrityMismatch
        );
        let unexpected_path = cache.object_path(&unexpected);
        assert!(!unexpected_path.exists());
        assert!(!unexpected_path.with_extension("zip.part").exists());
        let expected: [u8; 32] = Sha256::digest(body).into();
        assert_eq!(
            tokio::time::timeout(
                Duration::from_secs(5),
                cache.get_for_apply(expected, &url, false, ArchiveHeaders::default())
            )
            .await
            .expect("response")
            .expect("matches pin"),
            cache.object_path(&expected)
        );
        server.await.expect("fixture server");
        tokio::fs::remove_dir_all(root).await.expect("cleanup");
    }

    #[tokio::test]
    async fn repository_waits_for_an_archive_to_release_the_shared_download_slot() {
        let _reserved = tokio::time::timeout(
            Duration::from_secs(5),
            crate::HTTP_DOWNLOADS.acquire_many(63),
        )
        .await
        .expect("reserve all but one download slot")
        .expect("download gate remains open");
        let root = temporary_root("shared-downloads");
        let cache = PackageCache::new(root.clone()).expect("cache");
        let archive = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("archive listener");
        let repository = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .expect("repository listener");
        let archive_url = format!(
            "http://{}/package.zip",
            archive.local_addr().expect("archive address")
        );
        let repository_url = format!(
            "http://{}/index.json",
            repository.local_addr().expect("repository address")
        );
        let digest: [u8; 32] = Sha256::digest(b"archive").into();
        let package = tokio::spawn(async move {
            cache
                .get_for_apply(digest, &archive_url, false, ArchiveHeaders::default())
                .await
        });

        async fn begin_response(
            listener: &tokio::net::TcpListener,
            len: usize,
        ) -> tokio::net::TcpStream {
            use tokio::io::AsyncBufReadExt as _;
            let (stream, _) = listener.accept().await.expect("request");
            let mut reader = tokio::io::BufReader::new(stream);
            let mut header = String::new();
            while reader.read_line(&mut header).await.expect("header") > 0 && header != "\r\n" {
                header.clear();
            }
            let mut stream = reader.into_inner();
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {len}\r\nConnection: close\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .await
                .expect("response header");
            stream
        }

        let mut archive_response =
            tokio::time::timeout(Duration::from_secs(5), begin_response(&archive, 7))
                .await
                .expect("archive got the available slot");
        let reader = crate::VpmReader::new().expect("repository reader");
        let read = reader.inspect_remote_repository(repository_url, None, 1);
        tokio::pin!(read);
        tokio::select! {
            response = &mut read => panic!("repository completed before the archive released its slot: {response:?}"),
            connection = repository.accept() => panic!("repository opened a connection without a slot: {connection:?}"),
            () = tokio::time::sleep(Duration::from_millis(100)) => {}
        }
        archive_response
            .write_all(b"archive")
            .await
            .expect("archive body");
        let repository_response = async {
            begin_response(&repository, 2)
                .await
                .write_all(b"{}")
                .await
                .expect("repository body");
        };
        let (result, ()) = tokio::time::timeout(Duration::from_secs(5), async {
            tokio::join!(read, repository_response)
        })
        .await
        .expect("repository starts after the body releases the slot");
        assert!(matches!(
            result.expect("repository result"),
            crate::RepositoryReadOutcome::Fresh(_)
        ));
        package
            .await
            .expect("package task")
            .expect("archive verification");
        tokio::fs::remove_dir_all(root).await.expect("cleanup");
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
