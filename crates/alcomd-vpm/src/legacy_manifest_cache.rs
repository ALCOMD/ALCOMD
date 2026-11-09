//! Frozen legacy evidence, addressed by the manifest fingerprint already pinned
//! in a package Plan. This is reconstructible cache data, never registry state.

use std::io::Read;

use serde_json::Value;

use super::*;
use crate::package::LegacyMetadata;
use crate::resolver::{PackageSource, PackageSourceAuthority};

const MANIFEST_LIMIT: usize = 4 * 1024 * 1024;

impl PackageCache {
    fn manifest_path(&self, digest: &[u8; 32]) -> PathBuf {
        self.object_path(digest).with_extension("manifest.json")
    }

    /// Reads only an already frozen manifest. Missing/corrupt evidence is an
    /// explicit error; Apply never fetches a new index or assumes no cleanup.
    pub(crate) fn read_pinned_legacy(
        &self,
        digest: &[u8; 32],
    ) -> Result<LegacyMetadata, CacheError> {
        let manifest = self.read_pinned_manifest(digest)?;
        legacy(&manifest)
    }

    fn read_pinned_manifest(&self, digest: &[u8; 32]) -> Result<Value, CacheError> {
        let path = self.manifest_path(digest);
        validate_existing_chain(
            &self.root,
            path.parent()
                .ok_or_else(|| cache_error(CacheErrorCode::Io))?,
        )?;
        let bytes = read_stable_file(&path, MANIFEST_LIMIT, None)?;
        let manifest = bounded_json(&bytes)?;
        let canonical =
            serde_json::to_vec(&manifest).map_err(|_| cache_error(CacheErrorCode::Corrupt))?;
        if <[u8; 32]>::from(Sha256::digest(&canonical)) != *digest {
            return Err(cache_error(CacheErrorCode::IntegrityMismatch));
        }
        Ok(manifest)
    }

    /// Planning may obtain the exact indexed manifest, but a changed repository
    /// cannot silently change an existing catalog observation. Local identity
    /// and canonical fingerprint must both match its registered evidence.
    pub(crate) async fn pin_legacy_manifest(
        &self,
        source: &PackageSource,
        package_id: &str,
        version: &str,
    ) -> Result<LegacyMetadata, CacheError> {
        match self.read_pinned_manifest(&source.manifest_fingerprint) {
            Ok(manifest) => {
                validate_identity(&manifest, package_id, version)?;
                return legacy(&manifest);
            }
            Err(error) if error.code() == CacheErrorCode::OfflineMiss => {}
            Err(error) => return Err(error),
        }
        let manifest = match &source.authority {
            PackageSourceAuthority::Repository {
                repository_locator,
                credential_reference,
                ..
            } => {
                self.read_repository_package_manifest(
                    &source.source_identity,
                    repository_locator.as_deref(),
                    credential_reference.as_deref(),
                    package_id,
                    version,
                )
                .await?
            }
            PackageSourceAuthority::UserPackage { .. } => {
                let digest = source
                    .archive_sha256
                    .ok_or_else(|| cache_error(CacheErrorCode::InvalidDigest))?;
                let archive = self.get(digest, "", true).await?;
                tokio::task::spawn_blocking(move || read_archive_manifest(&archive))
                    .await
                    .map_err(|_| cache_error(CacheErrorCode::Io))??
            }
        };
        validate_identity(&manifest, package_id, version)?;
        let bytes = match source.authority {
            PackageSourceAuthority::Repository {
                download_headers_present,
                ..
            } => repository_manifest_evidence(&manifest, download_headers_present)?,
            PackageSourceAuthority::UserPackage { .. } => {
                serde_json::to_vec(&manifest).map_err(|_| cache_error(CacheErrorCode::Corrupt))?
            }
        };
        if bytes.len() > MANIFEST_LIMIT {
            return Err(cache_error(CacheErrorCode::DownloadTooLarge));
        }
        if <[u8; 32]>::from(Sha256::digest(&bytes)) != source.manifest_fingerprint {
            return Err(cache_error(CacheErrorCode::IntegrityMismatch));
        }
        let metadata = legacy(&manifest)?;
        self.publish_manifest(&source.manifest_fingerprint, &bytes)
            .await?;
        Ok(metadata)
    }

    pub(super) async fn read_package_download_headers(
        &self,
        lookup: PackageHeaderLookup<'_>,
        credential_reference: Option<&str>,
        artifact: &Url,
    ) -> Result<crate::repository_auth::RepositoryAuth, CacheError> {
        let manifest = self
            .read_repository_package_manifest(
                lookup.source_identity,
                lookup.local_path,
                credential_reference,
                lookup.package_id,
                lookup.version,
            )
            .await?;
        validate_identity(&manifest, lookup.package_id, lookup.version)?;
        let declared_url = manifest
            .get("url")
            .and_then(Value::as_str)
            .ok_or_else(|| cache_error(CacheErrorCode::Corrupt))?;
        if parse_artifact_url(declared_url)? != *artifact {
            return Err(cache_error(CacheErrorCode::IntegrityMismatch));
        }
        let evidence = repository_manifest_evidence(&manifest, true)?;
        if <[u8; 32]>::from(Sha256::digest(evidence)) != *lookup.fingerprint {
            return Err(cache_error(CacheErrorCode::IntegrityMismatch));
        }
        crate::repository_auth::RepositoryAuth::for_package(manifest.get("headers"), artifact)
            .map_err(|_| cache_error(CacheErrorCode::Corrupt))?
            .ok_or_else(|| cache_error(CacheErrorCode::IntegrityMismatch))
    }

    async fn read_repository_package_manifest(
        &self,
        source_identity: &str,
        repository_locator: Option<&str>,
        credential_reference: Option<&str>,
        package_id: &str,
        version: &str,
    ) -> Result<Value, CacheError> {
        let bytes = if let Some(url) = source_identity.strip_prefix("remote:") {
            self.read_index(&parse_artifact_url(url)?, credential_reference)
                .await?
        } else if let Some(identity) = source_identity.strip_prefix("local-fileid-v1:") {
            let locator =
                repository_locator.ok_or_else(|| cache_error(CacheErrorCode::OfflineMiss))?;
            let path = PathBuf::from(locator);
            if !path.is_absolute() {
                return Err(cache_error(CacheErrorCode::InvalidUrl));
            }
            let identity = parse_identity_hex(identity)?;
            tokio::task::spawn_blocking(move || {
                read_stable_file(&path, crate::REPOSITORY_DOCUMENT_LIMIT, Some(&identity))
            })
            .await
            .map_err(|_| cache_error(CacheErrorCode::Io))??
        } else {
            return Err(cache_error(CacheErrorCode::OfflineMiss));
        };
        let index = bounded_json(&bytes)?;
        index
            .get("packages")
            .and_then(|value| value.get(package_id))
            .and_then(|value| value.get("versions"))
            .and_then(|value| value.get(version))
            .cloned()
            .ok_or_else(|| cache_error(CacheErrorCode::IntegrityMismatch))
    }

    async fn read_index(
        &self,
        url: &Url,
        credential_reference: Option<&str>,
    ) -> Result<Vec<u8>, CacheError> {
        let _transfer = crate::HTTP_DOWNLOADS
            .acquire()
            .await
            .map_err(|cause| cache_error(CacheErrorCode::DownloadFailed).with_source(cause))?;
        let mut response = tokio::time::timeout(
            crate::REQUEST_TIMEOUT,
            self.get_response(url, credential_reference, None),
        )
        .await
        .map_err(|_| cache_error(CacheErrorCode::DownloadFailed))??;
        if response.status() != StatusCode::OK {
            return Err(cache_error(CacheErrorCode::DownloadFailed));
        }
        if response
            .content_length()
            .is_some_and(|length| length > crate::REPOSITORY_DOCUMENT_LIMIT as u64)
        {
            return Err(cache_error(CacheErrorCode::DownloadTooLarge));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| cache_error(CacheErrorCode::DownloadFailed))?
        {
            if bytes.len().saturating_add(chunk.len()) > crate::REPOSITORY_DOCUMENT_LIMIT {
                return Err(cache_error(CacheErrorCode::DownloadTooLarge));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }

    async fn publish_manifest(&self, digest: &[u8; 32], bytes: &[u8]) -> Result<(), CacheError> {
        let object = self.manifest_path(digest);
        let parent = object
            .parent()
            .ok_or_else(|| cache_error(CacheErrorCode::Io))?;
        prepare_cache_directory(&self.root, parent).await?;
        let part = object.with_extension(format!(
            "json.{}.part",
            alcomd_application::OperationId::new()
        ));
        let mut guard = PartialDownload::create(part.clone()).await?;
        guard
            .file
            .write_all(bytes)
            .await
            .map_err(|_| cache_error(CacheErrorCode::Io))?;
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
        let publication = CACHE_PUBLICATION.lock().await;
        match self.read_pinned_manifest(digest) {
            Ok(_) => return Ok(()),
            Err(error) if error.code() == CacheErrorCode::OfflineMiss => {}
            Err(error) => return Err(error),
        }
        ensure_cache_capacity(&self.root, bytes.len() as u64).await?;
        if tokio::fs::rename(&part, &object).await.is_err() {
            // A concurrent publisher may have installed the same immutable key.
            self.read_pinned_manifest(digest)?;
            return Ok(());
        }
        guard.published = true;
        drop(publication);
        let parent = parent.to_owned();
        tokio::task::spawn_blocking(move || alcomd_platform::sync_directory(&parent))
            .await
            .map_err(|_| cache_error(CacheErrorCode::Io))?
            .map_err(|_| cache_error(CacheErrorCode::Io))?;
        self.read_pinned_manifest(digest).map(|_| ())
    }
}

fn repository_manifest_evidence(manifest: &Value, headers: bool) -> Result<Vec<u8>, CacheError> {
    crate::package::manifest_evidence_bytes(
        manifest
            .as_object()
            .ok_or_else(|| cache_error(CacheErrorCode::Corrupt))?,
        headers,
    )
    .map_err(|_| cache_error(CacheErrorCode::Corrupt))
}

fn bounded_json(bytes: &[u8]) -> Result<Value, CacheError> {
    crate::parse_bounded_json(bytes, crate::M3ErrorCode::RepositoryDocumentInvalid)
        .map_err(|_| cache_error(CacheErrorCode::Corrupt))
}

fn legacy(manifest: &Value) -> Result<LegacyMetadata, CacheError> {
    LegacyMetadata::parse(
        manifest
            .as_object()
            .ok_or_else(|| cache_error(CacheErrorCode::Corrupt))?,
    )
    .map_err(|_| cache_error(CacheErrorCode::Corrupt))
}

fn validate_identity(manifest: &Value, package_id: &str, version: &str) -> Result<(), CacheError> {
    if manifest.get("name").and_then(Value::as_str) != Some(package_id)
        || manifest.get("version").and_then(Value::as_str) != Some(version)
    {
        return Err(cache_error(CacheErrorCode::IntegrityMismatch));
    }
    Ok(())
}

fn parse_identity_hex(value: &str) -> Result<Vec<u8>, CacheError> {
    if value.is_empty() || value.len() > 8192 || !value.len().is_multiple_of(2) {
        return Err(cache_error(CacheErrorCode::Corrupt));
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text =
                std::str::from_utf8(pair).map_err(|_| cache_error(CacheErrorCode::Corrupt))?;
            u8::from_str_radix(text, 16).map_err(|_| cache_error(CacheErrorCode::Corrupt))
        })
        .collect()
}

fn validate_existing_chain(root: &Path, parent: &Path) -> Result<(), CacheError> {
    if !root.is_absolute() || !parent.starts_with(root) {
        return Err(cache_error(CacheErrorCode::Corrupt));
    }
    let mut path = root.to_owned();
    let relative = parent
        .strip_prefix(root)
        .map_err(|_| cache_error(CacheErrorCode::Corrupt))?;
    for component in std::iter::once(None).chain(relative.components().map(Some)) {
        if let Some(component) = component {
            path.push(component);
        }
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
            cache_error(if error.kind() == std::io::ErrorKind::NotFound {
                CacheErrorCode::OfflineMiss
            } else {
                CacheErrorCode::Io
            })
        })?;
        if !metadata.is_dir() || is_link_or_reparse(&metadata) {
            return Err(cache_error(CacheErrorCode::Corrupt));
        }
    }
    Ok(())
}

fn read_stable_file(
    path: &Path,
    limit: usize,
    expected_identity: Option<&[u8]>,
) -> Result<Vec<u8>, CacheError> {
    let before = std::fs::symlink_metadata(path).map_err(|error| {
        cache_error(if error.kind() == std::io::ErrorKind::NotFound {
            CacheErrorCode::OfflineMiss
        } else {
            CacheErrorCode::Io
        })
    })?;
    if !before.is_file() || is_link_or_reparse(&before) || before.len() > limit as u64 {
        return Err(cache_error(CacheErrorCode::Corrupt));
    }
    let identity =
        alcomd_platform::file_identity_key(path).map_err(|_| cache_error(CacheErrorCode::Io))?;
    if expected_identity.is_some_and(|expected| identity != expected) {
        return Err(cache_error(CacheErrorCode::IntegrityMismatch));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .map_err(|_| cache_error(CacheErrorCode::Io))?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| cache_error(CacheErrorCode::Io))?;
    let after =
        std::fs::symlink_metadata(path).map_err(|_| cache_error(CacheErrorCode::Corrupt))?;
    if bytes.len() > limit
        || before.len() != after.len()
        || before.len() != bytes.len() as u64
        || before.modified().ok() != after.modified().ok()
        || is_link_or_reparse(&after)
        || alcomd_platform::file_identity_key(path).map_err(|_| cache_error(CacheErrorCode::Io))?
            != identity
    {
        return Err(cache_error(CacheErrorCode::IntegrityMismatch));
    }
    Ok(bytes)
}

fn read_archive_manifest(archive: &Path) -> Result<Value, CacheError> {
    let bytes = read_archive_manifest_bytes(archive)?;
    let normalized = crate::user_package::normalized_manifest_json(&bytes)
        .map_err(|_| cache_error(CacheErrorCode::Corrupt))?;
    bounded_json(normalized.as_bytes())
}

pub(super) fn validate_archive_manifest_identity(
    archive: &Path,
    package_id: &str,
    version: &str,
) -> Result<(), CacheError> {
    let manifest = bounded_json(&read_archive_manifest_bytes(archive)?)?;
    validate_identity(&manifest, package_id, version)
}

fn read_archive_manifest_bytes(archive: &Path) -> Result<Vec<u8>, CacheError> {
    let file = std::fs::File::open(archive).map_err(|_| cache_error(CacheErrorCode::Corrupt))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|_| cache_error(CacheErrorCode::Corrupt))?;
    let manifest = archive
        .by_name("package.json")
        .map_err(|_| cache_error(CacheErrorCode::Corrupt))?;
    if manifest.size() > MANIFEST_LIMIT as u64 || manifest.is_dir() {
        return Err(cache_error(CacheErrorCode::Corrupt));
    }
    let mut bytes = Vec::new();
    manifest
        .take(MANIFEST_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| cache_error(CacheErrorCode::Corrupt))?;
    if bytes.len() > MANIFEST_LIMIT {
        return Err(cache_error(CacheErrorCode::Corrupt));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "alcomd-legacy-manifest-{}",
                alcomd_application::OperationId::new()
            ));
            std::fs::create_dir(&path).expect("owned fixture");
            Self(path)
        }
        fn source(&self, index: &Path, manifest: &Value) -> PackageSource {
            let identity = alcomd_platform::file_identity_key(index).expect("index identity");
            let identity = identity
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            PackageSource {
                authority: PackageSourceAuthority::Repository {
                    credential_reference: None,
                    download_headers_present: false,
                    repository_locator: Some(index.to_string_lossy().into_owned()),
                    repository_id: "fixture-repository".to_owned(),
                    repository_revision: 1,
                    priority: 1,
                    artifact_url: "https://archive-must-not-be-read.invalid/package.zip".to_owned(),
                },
                source_identity: format!("local-fileid-v1:{identity}"),
                manifest_fingerprint: Sha256::digest(
                    serde_json::to_vec(manifest).expect("canonical manifest"),
                )
                .into(),
                archive_sha256: Some([0; 32]),
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).expect("owned fixture cleanup");
        }
    }

    fn manifest() -> Value {
        serde_json::json!({"name":"com.example.package","version":"1.0.0",
            "legacyFolders":{"Assets/Old":"0123456789abcdef0123456789abcdef"},
            "zipSHA256":"a".repeat(64)})
    }

    fn index(manifest: Value) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"packages":{"com.example.package":{"versions":{"1.0.0":manifest}}}})).expect("index")
    }

    #[tokio::test]
    async fn indexed_legacy_is_frozen_by_manifest_hash_and_reused_without_source() {
        let fixture = Fixture::new();
        let path = fixture.0.join("repository.json");
        let manifest = manifest();
        std::fs::write(&path, index(manifest.clone())).expect("index");
        let source = fixture.source(&path, &manifest);
        let cache = PackageCache::new(fixture.0.join("cache")).expect("cache");
        let metadata = cache
            .pin_legacy_manifest(&source, "com.example.package", "1.0.0")
            .await
            .expect("pin indexed metadata");
        assert_eq!(
            metadata.folders["Assets/Old"],
            "0123456789abcdef0123456789abcdef"
        );
        std::fs::remove_file(&path).expect("source unavailable after freeze");
        assert_eq!(
            cache
                .read_pinned_legacy(&source.manifest_fingerprint)
                .expect("frozen offline read"),
            metadata
        );
        assert_eq!(
            cache
                .pin_legacy_manifest(&source, "com.example.package", "1.0.0")
                .await
                .expect("offline plan repeat"),
            metadata
        );
        let expected_bytes = serde_json::to_vec(&manifest).expect("manifest").len() as u64;
        assert_eq!(
            cache_size(&cache.root).await.expect("quota accounting"),
            expected_bytes
        );
        let pinned = cache.manifest_path(&source.manifest_fingerprint);
        std::fs::write(&pinned, b"{}").expect("corrupt owned fixture");
        assert_eq!(
            cache
                .read_pinned_legacy(&source.manifest_fingerprint)
                .expect_err("corruption is not absence")
                .code(),
            CacheErrorCode::IntegrityMismatch
        );
        assert_eq!(std::fs::read(&pinned).expect("evidence retained"), b"{}");
    }

    #[tokio::test]
    async fn changed_manifest_or_local_file_identity_cannot_discharge_obligations() {
        let fixture = Fixture::new();
        let path = fixture.0.join("repository.json");
        let manifest = manifest();
        std::fs::write(&path, index(manifest.clone())).expect("index");
        let source = fixture.source(&path, &manifest);
        let cache = PackageCache::new(fixture.0.join("cache")).expect("cache");
        let mut changed = manifest.clone();
        changed
            .as_object_mut()
            .expect("manifest")
            .remove("legacyFolders");
        std::fs::write(&path, index(changed)).expect("index omitted cleanup metadata");
        assert_eq!(
            cache
                .pin_legacy_manifest(&source, "com.example.package", "1.0.0")
                .await
                .expect_err("same version different frozen metadata")
                .code(),
            CacheErrorCode::IntegrityMismatch
        );
        assert!(!cache.manifest_path(&source.manifest_fingerprint).exists());
        std::fs::rename(&path, fixture.0.join("original.json")).expect("retain old identity");
        std::fs::write(&path, index(manifest)).expect("same bytes different identity");
        assert_eq!(
            cache
                .pin_legacy_manifest(&source, "com.example.package", "1.0.0")
                .await
                .expect_err("changed source identity")
                .code(),
            CacheErrorCode::IntegrityMismatch
        );
    }
}
