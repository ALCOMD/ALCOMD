//! Independent bounded read-only Unity/VPM parsers used by M3.

mod archive;
mod backup;
mod cache;
mod candidates;
mod engine;
mod legacy;
mod package;
mod plan;
mod project_copy;
mod project_create;
mod project_delete;
mod range;
mod repository_auth;
mod repository_list;
mod resolver;
mod staging_package;
mod unity_migration;
mod unity_version;
mod user_package;

pub use archive::{
    ArchiveEntry, ArchiveError, ArchiveErrorCode, ArchiveLimits, ArchivePreflight, extract_archive,
    extract_archive_with_limits, preflight_archive, preflight_archive_with_limits,
};
pub use backup::BackupEngine;
pub use cache::{CacheError, CacheErrorCode, PackageCache};
pub use candidates::{
    ProjectCandidateEngine, evaluate_candidate_summary, evaluate_candidate_versions,
};
pub use engine::PackageEngine;
pub use package::{
    PackageManifestError, PackageManifestErrorCode, RepositoryPackageContext, ResolverReadyPackage,
    parse_resolver_ready_repository,
};
pub use plan::{
    ProjectPackageSnapshot, build_bulk_plan, build_reinstall_plan, build_remove_plan,
    build_resolution_plan, inspect_package_project, materialize_vpm_manifest,
};
pub use project_copy::ProjectCopyEngine;
pub use project_create::ProjectCreateEngine;
pub use project_delete::ProjectDeleteEngine;
pub use repository_list::RepositoryListFileAdapter;
pub use resolver::{
    PackageCandidate, PackageDependency, PackageDependencyEdge, PackageSource,
    PackageSourceAuthority, Resolution, ResolveError, ResolveRequest, ResolvedPackage,
    candidates_from_catalog, resolve_packages,
};
pub use staging_package::{
    FrozenPackageMaterializer, PreparedFrozenPackages, StagingPackageEvidence,
    StagingProjectEvidence,
};
pub use unity_migration::UnityMigrationEngine;
pub use unity_version::UnityVersion;
pub use user_package::UserPackageEngine;

use std::path::{Path, PathBuf};
use std::time::Duration;

use alcomd_application::{
    DependencyIdentity, M3Error, M3ErrorCode, M3ReadAdapter, ManifestState, ProjectDiscoveryMode,
    ProjectObservation, ProjectType, ReadIssue, RepositoryObservation, RepositoryPackageVersion,
    RepositoryReadOutcome, RepositorySource, RepositoryValidators, ResolverPackageMetadata,
};
use reqwest::header::{CONTENT_LENGTH, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use reqwest::redirect::{Action, Attempt, Policy};
use reqwest::{Client, StatusCode, Url};
use semver::Version;
use serde_json::{Map, Value};
use tokio::io::AsyncReadExt;
use tokio::sync::Semaphore;

pub const PROJECT_VERSION_LIMIT: usize = 64 * 1024;
pub const PROJECT_MANIFEST_LIMIT: usize = 4 * 1024 * 1024;
pub const REPOSITORY_DOCUMENT_LIMIT: usize = 16 * 1024 * 1024;
pub const JSON_DEPTH_LIMIT: usize = 64;
pub const JSON_COLLECTION_LIMIT: usize = 16_384;
pub const JSON_STRING_LIMIT: usize = 65_536;
pub const ISSUE_LIMIT: usize = 1_024;
pub const DEPENDENCY_LIMIT: usize = 4_096;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const REDIRECT_LIMIT: usize = 10;
const HTTP_DOWNLOAD_CONCURRENCY: usize = alcomd_application::DOWNLOAD_CONCURRENCY;
static HTTP_DOWNLOADS: Semaphore = Semaphore::const_new(HTTP_DOWNLOAD_CONCURRENCY);

/// Classifies a version with the single approved SemVer parser.
#[must_use]
pub fn classify_prerelease(version: &str) -> Option<bool> {
    Version::parse(version)
        .ok()
        .map(|version| !version.pre.is_empty())
}

/// Creates the same target lock used by project create, restore and copy publication.
#[must_use]
pub fn project_creation_resource(
    parent_identity: &[u8],
    target_leaf: String,
) -> alcomd_application::ResourceKey {
    use sha2::{Digest, Sha256};
    alcomd_application::ResourceKey::ProjectCreate {
        parent_identity_sha256: Sha256::digest(parent_identity).into(),
        target_leaf,
    }
}

/// Bounded M3 reader with anonymous and authenticated HTTP clients.
#[derive(Clone)]
pub struct VpmReader {
    client: Client,
    authenticated_client: Client,
}

impl VpmReader {
    pub fn new() -> Result<Self, M3Error> {
        let client = Client::builder()
            .referer(false)
            .timeout(REQUEST_TIMEOUT)
            .redirect(Policy::custom(validate_redirect))
            .build()
            .map_err(|_| M3Error::new(M3ErrorCode::Internal))?;
        let authenticated_client = repository_auth::client(REQUEST_TIMEOUT, REQUEST_TIMEOUT)
            .map_err(|_| M3Error::new(M3ErrorCode::Internal))?;
        Ok(Self {
            client,
            authenticated_client,
        })
    }

    pub async fn find_project_root(
        &self,
        path: &Path,
        mode: ProjectDiscoveryMode,
    ) -> Result<PathBuf, M3Error> {
        if !path.is_absolute() {
            return Err(M3Error::new(M3ErrorCode::ProjectNotFound));
        }
        let mut candidate = path.to_path_buf();
        if tokio::fs::metadata(&candidate)
            .await
            .is_ok_and(|metadata| metadata.is_file())
        {
            let _ = candidate.pop();
        }
        loop {
            if component_exists(&candidate, &["ProjectSettings", "ProjectVersion.txt"]).await? {
                return Ok(candidate);
            }
            if mode == ProjectDiscoveryMode::ExactRoot || !candidate.pop() {
                return Err(M3Error::new(M3ErrorCode::ProjectNotFound));
            }
        }
    }

    pub async fn inspect_project_root(
        &self,
        root: PathBuf,
        path_identity_key: Vec<u8>,
    ) -> Result<ProjectObservation, M3Error> {
        let root_text = root
            .to_str()
            .ok_or_else(|| M3Error::new(M3ErrorCode::PathEncodingUnsupported))?
            .to_owned();
        let project_version = read_component(
            &root,
            &["ProjectSettings", "ProjectVersion.txt"],
            PROJECT_VERSION_LIMIT,
        )
        .await?
        .ok_or_else(|| M3Error::new(M3ErrorCode::ProjectVersionMissing))?;
        let (unity_version, unity_revision) = parse_project_version(&project_version)?;
        let vpm_bytes = read_component(
            &root,
            &["Packages", "vpm-manifest.json"],
            PROJECT_MANIFEST_LIMIT,
        )
        .await?;
        let upm_bytes = read_component(
            &root,
            &["Packages", "manifest.json"],
            PROJECT_MANIFEST_LIMIT,
        )
        .await?;
        let (vpm_manifest, direct_dependencies, locked_dependencies) = match vpm_bytes {
            Some(bytes) => {
                let value = parse_bounded_json(&bytes, M3ErrorCode::ProjectManifestInvalid)
                    .map_err(|error| {
                        error.with_path(
                            root.join("Packages")
                                .join("vpm-manifest.json")
                                .display()
                                .to_string(),
                        )
                    })?;
                let object = value
                    .as_object()
                    .ok_or_else(|| M3Error::new(M3ErrorCode::ProjectManifestInvalid))?;
                (
                    ManifestState::Valid,
                    parse_vpm_dependencies(object.get("dependencies")).map_err(|error|error.with_path(root.join("Packages").join("vpm-manifest.json").display().to_string()).with_reason("VPM dependencies require version objects with valid versions"))?,
                    parse_vpm_dependencies(object.get("locked")).map_err(|error|error.with_path(root.join("Packages").join("vpm-manifest.json").display().to_string()).with_reason("VPM locked dependencies require version objects with valid versions"))?,
                )
            }
            None => (ManifestState::Missing, Vec::new(), Vec::new()),
        };
        let (upm_manifest, upm_dependencies) = match upm_bytes {
            Some(bytes) => {
                let value = parse_bounded_json(&bytes, M3ErrorCode::ProjectManifestInvalid)
                    .map_err(|error| {
                        error.with_path(
                            root.join("Packages")
                                .join("manifest.json")
                                .display()
                                .to_string(),
                        )
                    })?;
                let object = value
                    .as_object()
                    .ok_or_else(|| M3Error::new(M3ErrorCode::ProjectManifestInvalid))?;
                (
                    ManifestState::Valid,
                    parse_string_map(object.get("dependencies"))?,
                )
            }
            None => (ManifestState::Missing, Vec::new()),
        };
        let project_type =
            detect_project_type(&root, &locked_dependencies, &upm_dependencies).await?;
        Ok(ProjectObservation {
            root_path: root_text,
            path_identity_key,
            project_type,
            unity_version,
            unity_revision,
            vpm_manifest,
            upm_manifest,
            direct_dependencies,
            locked_dependencies,
            issues: Vec::new(),
        })
    }

    pub async fn inspect_local_repository(
        &self,
        path: PathBuf,
        source_identity_key: Vec<u8>,
        refreshed_at_ms: u64,
    ) -> Result<RepositoryReadOutcome, M3Error> {
        let path_text = path
            .to_str()
            .ok_or_else(|| M3Error::new(M3ErrorCode::PathEncodingUnsupported))?
            .to_owned();
        let bytes = read_bounded_file(&path, REPOSITORY_DOCUMENT_LIMIT, false).await?;
        Ok(RepositoryReadOutcome::Fresh(parse_repository(
            &bytes,
            RepositorySource::Local { path: path_text },
            source_identity_key,
            RepositoryValidators::default(),
            refreshed_at_ms,
        )?))
    }

    pub async fn inspect_remote_repository(
        &self,
        registration_url: String,
        validators: Option<RepositoryValidators>,
        refreshed_at_ms: u64,
    ) -> Result<RepositoryReadOutcome, M3Error> {
        self.read_remote_repository(registration_url, validators, refreshed_at_ms, None)
            .await
    }

    async fn read_remote_repository(
        &self,
        registration_url: String,
        validators: Option<RepositoryValidators>,
        refreshed_at_ms: u64,
        auth: Option<&repository_auth::RepositoryAuth>,
    ) -> Result<RepositoryReadOutcome, M3Error> {
        let normalized = normalize_remote_url(&registration_url)?;
        if auth.is_some_and(|auth| auth.contains_url_secret(&normalized)) {
            return Err(M3Error::new(M3ErrorCode::RepositorySourceInvalid));
        }
        let request = |client: &Client, url: Url| {
            let same_origin = url.origin() == normalized.origin();
            let mut request = client.get(url);
            if let Some(validators) = validators.as_ref().filter(|_| same_origin) {
                if let Some(etag) = &validators.etag {
                    request = request.header(IF_NONE_MATCH, etag);
                }
                if let Some(last_modified) = &validators.last_modified {
                    request = request.header(IF_MODIFIED_SINCE, last_modified);
                }
            }
            request
        };
        let transfer = HTTP_DOWNLOADS
            .acquire()
            .await
            .map_err(|cause| M3Error::new(M3ErrorCode::RepositoryUnavailable).with_source(cause))?;
        let mut response = if let Some(auth) = auth {
            repository_auth::send(normalized.clone(), &[auth], |url| {
                request(&self.authenticated_client, url)
            })
            .await?
        } else {
            request(&self.client, normalized.clone())
                .send()
                .await
                .map_err(|error| {
                    M3Error::new(M3ErrorCode::RepositoryUnavailable)
                        .with_source(error.without_url())
                })?
        };
        let validator = |value| {
            bounded_header(value)
                .filter(|value| !auth.is_some_and(|auth| auth.contains_secret(value)))
        };
        let response_validators = RepositoryValidators {
            etag: validator(response.headers().get(ETAG)),
            last_modified: validator(response.headers().get(LAST_MODIFIED)),
        };
        if response.status() == StatusCode::NOT_MODIFIED {
            let previous =
                validators.ok_or_else(|| M3Error::new(M3ErrorCode::RepositoryUnavailable))?;
            return Ok(RepositoryReadOutcome::NotModified(RepositoryValidators {
                etag: response_validators.etag.or(previous.etag),
                last_modified: response_validators.last_modified.or(previous.last_modified),
            }));
        }
        if !response.status().is_success() {
            return Err(M3Error::new(M3ErrorCode::RepositoryUnavailable));
        }
        if response
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .is_some_and(|length| length > REPOSITORY_DOCUMENT_LIMIT as u64)
        {
            return Err(M3Error::new(M3ErrorCode::RepositoryDocumentTooLarge));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|error| {
            M3Error::new(M3ErrorCode::RepositoryUnavailable).with_source(error.without_url())
        })? {
            if bytes.len().saturating_add(chunk.len()) > REPOSITORY_DOCUMENT_LIMIT {
                return Err(M3Error::new(M3ErrorCode::RepositoryDocumentTooLarge));
            }
            bytes.extend_from_slice(&chunk);
        }
        drop(transfer);
        if let Some(auth) = auth {
            auth.check_document(&bytes)?;
        }
        let source_text = normalized.to_string();
        Ok(RepositoryReadOutcome::Fresh(parse_repository(
            &bytes,
            RepositorySource::Remote {
                url: source_text.clone(),
            },
            source_text.as_bytes().to_vec(),
            response_validators,
            refreshed_at_ms,
        )?))
    }
}

impl M3ReadAdapter for VpmReader {
    async fn inspect_repository_authenticated(
        &self,
        source: RepositorySource,
        validators: Option<RepositoryValidators>,
        credentials: Option<alcomd_application::RepositoryCredentials>,
    ) -> Result<RepositoryReadOutcome, M3Error> {
        let Some(credentials) = credentials else {
            return self.inspect_repository(source, validators).await;
        };
        let RepositorySource::Remote { url } = source else {
            return Err(M3Error::new(M3ErrorCode::RepositorySourceInvalid));
        };
        let auth = repository_auth::RepositoryAuth::new(&credentials)?;
        self.read_remote_repository(url, validators, unix_time_ms()?, Some(&auth))
            .await
    }

    async fn save_repository_credentials(
        &self,
        reference: String,
        credentials: alcomd_application::RepositoryCredentials,
    ) -> Result<(), M3Error> {
        repository_auth::save(reference, credentials).await
    }

    async fn load_repository_credentials(
        &self,
        reference: String,
    ) -> Result<alcomd_application::RepositoryCredentials, M3Error> {
        repository_auth::load(reference).await
    }

    async fn delete_repository_credentials(&self, reference: String) -> Result<(), M3Error> {
        repository_auth::delete(reference).await
    }

    fn normalize_remote_repository_url(&self, url: &str) -> Result<String, M3Error> {
        normalize_remote_url(url).map(|url| url.to_string())
    }

    async fn inspect_project(
        &self,
        path: String,
        mode: ProjectDiscoveryMode,
    ) -> Result<ProjectObservation, M3Error> {
        let root = self
            .find_project_root(Path::new(&path), mode)
            .await
            .map_err(|error| error.with_path(path.clone()))?;
        let (root, identity) =
            tokio::task::spawn_blocking(move || alcomd_platform::resolve_directory_identity(&root))
                .await
                .map_err(|_| M3Error::new(M3ErrorCode::Internal))?
                .map_err(platform_project_error)?;
        self.inspect_project_root(root, identity)
            .await
            .map_err(|error| error.with_path(path))
    }

    async fn inspect_repository(
        &self,
        source: RepositorySource,
        validators: Option<RepositoryValidators>,
    ) -> Result<RepositoryReadOutcome, M3Error> {
        let now_ms = unix_time_ms()?;
        match source {
            RepositorySource::Local { path } => {
                let path = PathBuf::from(path);
                if !path.is_absolute() {
                    return Err(M3Error::new(M3ErrorCode::RepositorySourceInvalid));
                }
                let (path, identity) = tokio::task::spawn_blocking(move || {
                    let path = std::fs::canonicalize(path)?;
                    let identity = alcomd_platform::file_identity_key(&path)?;
                    Ok::<_, std::io::Error>((path, identity))
                })
                .await
                .map_err(|_| M3Error::new(M3ErrorCode::Internal))?
                .map_err(platform_repository_error)?;
                self.inspect_local_repository(path, identity, now_ms).await
            }
            RepositorySource::Remote { url } => {
                self.inspect_remote_repository(url, validators, now_ms)
                    .await
            }
        }
    }
}

fn unix_time_ms() -> Result<u64, M3Error> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| M3Error::new(M3ErrorCode::Internal))?;
    u64::try_from(duration.as_millis()).map_err(|_| M3Error::new(M3ErrorCode::Internal))
}

fn platform_project_error(error: std::io::Error) -> M3Error {
    let code = if error.kind() == std::io::ErrorKind::InvalidData {
        M3ErrorCode::PathEncodingUnsupported
    } else {
        M3ErrorCode::ProjectInaccessible
    };
    M3Error::new(code)
}

fn platform_repository_error(error: std::io::Error) -> M3Error {
    let code = if error.kind() == std::io::ErrorKind::InvalidData {
        M3ErrorCode::PathEncodingUnsupported
    } else {
        M3ErrorCode::RepositoryInaccessible
    };
    M3Error::new(code)
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
        return attempt.error("redirect target is not an anonymous HTTP(S) URL");
    }
    attempt.follow()
}

fn normalize_remote_url(value: &str) -> Result<Url, M3Error> {
    let mut url =
        Url::parse(value).map_err(|_| M3Error::new(M3ErrorCode::RepositorySourceInvalid))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(M3Error::new(M3ErrorCode::RepositorySourceInvalid));
    }
    url.set_fragment(None);
    Ok(url)
}

async fn read_component(
    root: &Path,
    components: &[&str],
    limit: usize,
) -> Result<Option<Vec<u8>>, M3Error> {
    let mut path = root.to_path_buf();
    for component in components {
        path.push(component);
        match tokio::fs::symlink_metadata(&path).await {
            Ok(metadata) if is_link_or_reparse(&metadata) => {
                return Err(M3Error::new(M3ErrorCode::ProjectInaccessible));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(M3Error::new(M3ErrorCode::ProjectInaccessible)
                    .with_path(path.display().to_string())
                    .with_source(error));
            }
        }
    }
    read_bounded_file(&path, limit, true)
        .await
        .map(Some)
        .map_err(|error| error.with_path(path.display().to_string()))
}

async fn component_exists(root: &Path, components: &[&str]) -> Result<bool, M3Error> {
    let mut path = root.to_path_buf();
    for component in components {
        path.push(component);
        match tokio::fs::symlink_metadata(&path).await {
            Ok(metadata) if is_link_or_reparse(&metadata) => {
                return Err(M3Error::new(M3ErrorCode::ProjectInaccessible));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => {
                return Err(M3Error::new(M3ErrorCode::ProjectInaccessible)
                    .with_path(path.display().to_string())
                    .with_source(error));
            }
        }
    }
    Ok(tokio::fs::metadata(path)
        .await
        .is_ok_and(|metadata| metadata.is_file()))
}

async fn read_bounded_file(path: &Path, limit: usize, project: bool) -> Result<Vec<u8>, M3Error> {
    let inaccessible = if project {
        M3ErrorCode::ProjectInaccessible
    } else {
        M3ErrorCode::RepositoryInaccessible
    };
    let metadata_before = tokio::fs::metadata(path).await.map_err(|error| {
        M3Error::new(if error.kind() == std::io::ErrorKind::NotFound {
            if project {
                M3ErrorCode::ProjectNotFound
            } else {
                M3ErrorCode::RepositoryNotFound
            }
        } else {
            inaccessible
        })
    })?;
    if !metadata_before.is_file() {
        return Err(M3Error::new(inaccessible));
    }
    if metadata_before.len() > limit as u64 {
        return Err(M3Error::new(if project {
            M3ErrorCode::ProjectManifestInvalid
        } else {
            M3ErrorCode::RepositoryDocumentTooLarge
        }));
    }
    let mut file = tokio::fs::File::open(path).await.map_err(|error| {
        M3Error::new(inaccessible)
            .with_path(path.display().to_string())
            .with_source(error)
    })?;
    let mut bytes = Vec::with_capacity((metadata_before.len() as usize).min(limit));
    (&mut file)
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| {
            M3Error::new(inaccessible)
                .with_path(path.display().to_string())
                .with_source(error)
        })?;
    if bytes.len() > limit {
        return Err(M3Error::new(if project {
            M3ErrorCode::ProjectManifestInvalid
        } else {
            M3ErrorCode::RepositoryDocumentTooLarge
        }));
    }
    let metadata_after = tokio::fs::metadata(path).await.map_err(|error| {
        M3Error::new(inaccessible)
            .with_path(path.display().to_string())
            .with_source(error)
    })?;
    if metadata_before.len() != metadata_after.len()
        || metadata_before.modified().ok() != metadata_after.modified().ok()
    {
        return Err(M3Error::new(inaccessible));
    }
    Ok(bytes)
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

fn parse_project_version(bytes: &[u8]) -> Result<(String, Option<String>), M3Error> {
    let text =
        std::str::from_utf8(bytes).map_err(|_| M3Error::new(M3ErrorCode::ProjectVersionInvalid))?;
    let mut version = None;
    let mut revision = None;
    for line in text.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if value.is_empty() || value.len() > 128 {
            return Err(M3Error::new(M3ErrorCode::ProjectVersionInvalid));
        }
        match key.trim() {
            "m_EditorVersion" if version.replace(value.to_owned()).is_some() => {
                return Err(M3Error::new(M3ErrorCode::ProjectVersionInvalid));
            }
            "m_EditorVersionWithRevision" if revision.replace(value.to_owned()).is_some() => {
                return Err(M3Error::new(M3ErrorCode::ProjectVersionInvalid));
            }
            _ => {}
        }
    }
    Ok((
        version.ok_or_else(|| M3Error::new(M3ErrorCode::ProjectVersionInvalid))?,
        revision,
    ))
}

fn parse_bounded_json(bytes: &[u8], code: M3ErrorCode) -> Result<Value, M3Error> {
    let value: Value = serde_json::from_slice(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes))
        .map_err(|error| M3Error::new(code).with_source(error))?;
    validate_json_value(&value, 0, code)?;
    Ok(value)
}

fn validate_json_value(value: &Value, depth: usize, code: M3ErrorCode) -> Result<(), M3Error> {
    if depth > JSON_DEPTH_LIMIT {
        return Err(M3Error::new(code));
    }
    match value {
        Value::String(value) if value.len() > JSON_STRING_LIMIT => Err(M3Error::new(code)),
        Value::Array(values) => {
            if values.len() > JSON_COLLECTION_LIMIT {
                return Err(M3Error::new(code));
            }
            values
                .iter()
                .try_for_each(|value| validate_json_value(value, depth + 1, code))
        }
        Value::Object(values) => {
            if values.len() > JSON_COLLECTION_LIMIT {
                return Err(M3Error::new(code));
            }
            for (key, value) in values {
                if key.len() > JSON_STRING_LIMIT {
                    return Err(M3Error::new(code));
                }
                validate_json_value(value, depth + 1, code)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn parse_vpm_dependencies(value: Option<&Value>) -> Result<Vec<DependencyIdentity>, M3Error> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let object = value
        .as_object()
        .ok_or_else(|| M3Error::new(M3ErrorCode::ProjectManifestInvalid))?;
    if object.len() > DEPENDENCY_LIMIT {
        return Err(M3Error::new(M3ErrorCode::ProjectManifestInvalid));
    }
    let mut dependencies = object
        .iter()
        .map(|(package_id, value)| {
            let version = value
                .as_object()
                .and_then(|object| object.get("version"))
                .and_then(Value::as_str)
                .ok_or_else(|| M3Error::new(M3ErrorCode::ProjectManifestInvalid))?;
            bounded_identity(package_id, version)
        })
        .collect::<Result<Vec<_>, _>>()?;
    dependencies.sort_by(|left, right| left.package_id.as_bytes().cmp(right.package_id.as_bytes()));
    Ok(dependencies)
}

fn parse_string_map(value: Option<&Value>) -> Result<Vec<DependencyIdentity>, M3Error> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let object = value
        .as_object()
        .ok_or_else(|| M3Error::new(M3ErrorCode::ProjectManifestInvalid))?;
    if object.len() > DEPENDENCY_LIMIT {
        return Err(M3Error::new(M3ErrorCode::ProjectManifestInvalid));
    }
    let mut dependencies = object
        .iter()
        .map(|(package_id, value)| {
            bounded_identity(
                package_id,
                value
                    .as_str()
                    .ok_or_else(|| M3Error::new(M3ErrorCode::ProjectManifestInvalid))?,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    dependencies.sort_by(|left, right| left.package_id.as_bytes().cmp(right.package_id.as_bytes()));
    Ok(dependencies)
}

fn bounded_identity(package_id: &str, value: &str) -> Result<DependencyIdentity, M3Error> {
    if package_id.is_empty() || package_id.len() > 1_024 || value.is_empty() || value.len() > 1_024
    {
        return Err(M3Error::new(M3ErrorCode::ProjectManifestInvalid));
    }
    Ok(DependencyIdentity {
        package_id: package_id.to_owned(),
        value: value.to_owned(),
    })
}

async fn detect_project_type(
    root: &Path,
    locked: &[DependencyIdentity],
    upm: &[DependencyIdentity],
) -> Result<ProjectType, M3Error> {
    if has_package(locked, "com.vrchat.avatars") {
        return Ok(ProjectType::Avatars);
    }
    if has_package(locked, "com.vrchat.worlds") {
        return Ok(ProjectType::Worlds);
    }
    if !locked.is_empty() {
        return Ok(ProjectType::VpmStarter);
    }
    if has_package(upm, "com.vrchat.avatars") {
        return Ok(ProjectType::UpmAvatars);
    }
    if has_package(upm, "com.vrchat.worlds") {
        return Ok(ProjectType::UpmWorlds);
    }
    if has_package(upm, "com.vrchat.base") {
        return Ok(ProjectType::UpmStarter);
    }
    for (components, project_type) in [
        (
            ["Assets", "VRCSDK", "Plugins", "VRCSDK2.dll"],
            ProjectType::LegacySdk2,
        ),
        (
            ["Assets", "VRCSDK", "Plugins", "VRCSDK3.dll"],
            ProjectType::LegacyWorlds,
        ),
        (
            ["Assets", "VRCSDK", "Plugins", "VRCSDK3A.dll"],
            ProjectType::LegacyAvatars,
        ),
    ] {
        if component_exists(root, &components).await? {
            return Ok(project_type);
        }
    }
    Ok(ProjectType::Unknown)
}

fn has_package(values: &[DependencyIdentity], package_id: &str) -> bool {
    values.iter().any(|value| value.package_id == package_id)
}

fn parse_repository(
    bytes: &[u8],
    source: RepositorySource,
    source_identity_key: Vec<u8>,
    validators: RepositoryValidators,
    refreshed_at_ms: u64,
) -> Result<RepositoryObservation, M3Error> {
    let value = parse_bounded_json(bytes, M3ErrorCode::RepositoryDocumentInvalid)?;
    let object = value
        .as_object()
        .ok_or_else(|| M3Error::new(M3ErrorCode::RepositoryDocumentInvalid))?;
    let declared_id = optional_bounded_string(object, "id", 1_024)?;
    let name = optional_bounded_string(object, "name", 4_096)?;
    let declared_url = optional_bounded_string(object, "url", 32_768)?;
    let source_identity = match &source {
        RepositorySource::Local { path } => format!("local:{path}"),
        RepositorySource::Remote { url } => format!("remote:{url}"),
    };
    let context = RepositoryPackageContext {
        repository_id: "pending:repository".to_owned(),
        repository_revision: 1,
        priority: 1,
        source_identity,
    };
    let mut rows = Vec::new();
    let mut issues = Vec::new();
    if let Some(packages_value) = object.get("packages") {
        let packages = packages_value
            .as_object()
            .ok_or_else(|| M3Error::new(M3ErrorCode::RepositoryDocumentInvalid))?;
        for (package_key, package_value) in packages {
            let Some(package) = package_value.as_object() else {
                push_issue(&mut issues, "package_invalid", "package", package_key)?;
                continue;
            };
            let Some(versions) = package.get("versions").and_then(Value::as_object) else {
                push_issue(&mut issues, "versions_invalid", "package", package_key)?;
                continue;
            };
            for (version_key, manifest_value) in versions {
                let item = format!("{package_key}@{version_key}");
                let Some(manifest) = manifest_value.as_object() else {
                    push_issue(&mut issues, "version_invalid", "version", &item)?;
                    continue;
                };
                if package_key.is_empty()
                    || package_key.len() > 1_024
                    || version_key.is_empty()
                    || version_key.len() > 1_024
                {
                    push_issue(&mut issues, "identity_invalid", "version", &item)?;
                    continue;
                }
                if manifest.get("name").and_then(Value::as_str) != Some(package_key) {
                    push_issue(&mut issues, "package_name_mismatch", "version", &item)?;
                }
                if manifest.get("version").and_then(Value::as_str) != Some(version_key) {
                    push_issue(&mut issues, "version_mismatch", "version", &item)?;
                }
                rows.push(RepositoryPackageVersion {
                    package_id: package_key.to_owned(),
                    version: version_key.to_owned(),
                    display_name: repository_display_field(
                        manifest,
                        "displayName",
                        4_096,
                        &item,
                        &mut issues,
                    )?,
                    description: repository_display_field(
                        manifest,
                        "description",
                        65_536,
                        &item,
                        &mut issues,
                    )?,
                    yanked: manifest
                        .get("yanked")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    unity: repository_display_field(manifest, "unity", 1_024, &item, &mut issues)?,
                    links: package_links(manifest),
                    resolver: match package::parse_manifest(
                        package_key,
                        version_key,
                        manifest,
                        &context,
                    ) {
                        Ok(package) => {
                            let PackageSourceAuthority::Repository {
                                artifact_url,
                                download_headers_present,
                                ..
                            } = package.candidate.source.authority
                            else {
                                unreachable!("repository context")
                            };
                            Some(ResolverPackageMetadata {
                                semantic_version: package.candidate.version.to_string(),
                                author_name: package.author_name,
                                author_email: package.author_email,
                                artifact_url,
                                zip_sha256: package
                                    .candidate
                                    .source
                                    .archive_sha256
                                    .as_ref()
                                    .map(digest_text),
                                unity_release: package.unity_release,
                                dependencies_json: package.dependencies_json,
                                manifest_fingerprint: package
                                    .candidate
                                    .source
                                    .manifest_fingerprint
                                    .to_vec(),
                                legacy_metadata_present: package.candidate.legacy_metadata_present,
                                download_headers_present,
                            })
                        }
                        Err(_) => {
                            push_issue(&mut issues, "resolver_metadata_invalid", "version", &item)?;
                            None
                        }
                    },
                });
            }
        }
    }
    rows.sort_by(|left, right| {
        left.package_id
            .as_bytes()
            .cmp(right.package_id.as_bytes())
            .then_with(|| left.version.as_bytes().cmp(right.version.as_bytes()))
    });
    issues.sort();
    Ok(RepositoryObservation {
        source,
        source_identity_key,
        declared_id,
        name,
        declared_url,
        issues,
        packages: rows,
        validators,
        refreshed_at_ms,
    })
}

fn repository_display_field(
    manifest: &Map<String, Value>,
    key: &str,
    limit: usize,
    item: &str,
    issues: &mut Vec<ReadIssue>,
) -> Result<Option<String>, M3Error> {
    match optional_bounded_string(manifest, key, limit) {
        Ok(value) => Ok(value),
        Err(_) => {
            push_issue(issues, "package_field_invalid", key, item)?;
            Ok(None)
        }
    }
}

fn package_links(
    manifest: &Map<String, Value>,
) -> Option<alcomd_application::RepositoryPackageLinks> {
    let documentation = sanitized_package_link(manifest.get("documentationUrl"));
    let changelog = sanitized_package_link(manifest.get("changelogUrl"));
    (documentation.is_some() || changelog.is_some()).then_some(
        alcomd_application::RepositoryPackageLinks {
            documentation,
            changelog,
        },
    )
}

pub(crate) fn sanitized_package_link(value: Option<&Value>) -> Option<String> {
    let text = value?.as_str()?;
    if text.is_empty() || text.len() > 2_048 {
        return None;
    }
    let url = Url::parse(text).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    let canonical = url.to_string();
    (canonical.len() <= 2_048).then_some(canonical)
}

fn digest_text(digest: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for byte in digest {
        result.push(char::from(HEX[(byte >> 4) as usize]));
        result.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    result
}

fn optional_bounded_string(
    object: &Map<String, Value>,
    key: &str,
    limit: usize,
) -> Result<Option<String>, M3Error> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.is_empty() && value.len() <= limit => {
            Ok(Some(value.clone()))
        }
        _ => Err(M3Error::new(M3ErrorCode::RepositoryDocumentInvalid)),
    }
}

fn push_issue(
    issues: &mut Vec<ReadIssue>,
    code: &str,
    component: &str,
    item: &str,
) -> Result<(), M3Error> {
    if issues.len() >= ISSUE_LIMIT || item.len() > 2_048 {
        return Err(M3Error::new(M3ErrorCode::RepositoryDocumentInvalid));
    }
    issues.push(ReadIssue {
        code: code.to_owned(),
        component: component.to_owned(),
        item: item.to_owned(),
        line: None,
        column: None,
    });
    Ok(())
}

fn bounded_header(value: Option<&reqwest::header::HeaderValue>) -> Option<String> {
    value
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.is_empty() && value.len() <= 4_096)
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vpm_versions_are_objects_and_upm_dependencies_are_strings() {
        let vpm = serde_json::json!({
            "com.vrchat.avatars": { "version": "^3.10.x" },
            "com.example.tool": { "version": "1.0.0" }
        });
        let parsed = parse_vpm_dependencies(Some(&vpm)).expect("VPM version objects");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[1].package_id, "com.vrchat.avatars");
        assert_eq!(parsed[1].value, "^3.10.x");
        let upm = serde_json::json!({ "com.unity.test": "file:../local-package" });
        assert_eq!(
            parse_string_map(Some(&upm)).expect("UPM string")[0].value,
            "file:../local-package"
        );
        assert!(parse_string_map(Some(&vpm)).is_err());
        for invalid in [
            serde_json::json!({"com.example.tool": "1.0.0"}),
            serde_json::json!({"com.example.tool": {}}),
            serde_json::json!({"com.example.tool": {"version": null}}),
            serde_json::json!({"com.example.tool": {"version": ""}}),
        ] {
            assert_eq!(
                parse_vpm_dependencies(Some(&invalid))
                    .expect_err("invalid VPM")
                    .code(),
                M3ErrorCode::ProjectManifestInvalid
            );
        }
    }

    #[test]
    fn one_invalid_manifest_preserves_healthy_rows_and_reports_incomplete_metadata() {
        let bytes =
            include_bytes!("../../alcomd-testing/fixtures/m4/repository-resolver-ready.json");
        let mut document: Value = serde_json::from_slice(bytes).expect("fixture");
        document["packages"]["com.example.feature"]["versions"]["2.0.0"]["description"] =
            serde_json::json!(["invalid"]);
        let parsed = parse_repository(
            &serde_json::to_vec(&document).expect("JSON"),
            RepositorySource::Remote {
                url: "https://fixtures.invalid/index.json".to_owned(),
            },
            vec![1],
            RepositoryValidators::default(),
            1,
        )
        .expect("read remaining versions");
        assert!(
            parsed
                .packages
                .iter()
                .any(|row| row.package_id == "com.example.base" && row.resolver.is_some())
        );
        assert!(
            parsed
                .packages
                .iter()
                .any(|row| row.package_id == "com.example.feature" && row.resolver.is_none())
        );
        assert!(
            parsed
                .issues
                .iter()
                .any(|issue| issue.code == "package_field_invalid")
        );
        assert!(
            parsed
                .issues
                .iter()
                .any(|issue| issue.code == "resolver_metadata_invalid")
        );
    }

    #[test]
    fn repository_rows_and_issues_are_deterministic_without_semver() {
        let document = br#"{
            "name":"Fixture",
            "packages":{
                "z.package":{"versions":{"1.0.0":{"name":"z.package","version":"1.0.0"}}},
                "a.package":{"versions":{
                    "not-semver":{"name":"other","version":"different","yanked":true},
                    "2":{"name":"a.package","version":"2"}
                }}
            }
        }"#;
        let parsed = parse_repository(
            document,
            RepositorySource::Local {
                path: "fixture.json".to_owned(),
            },
            vec![1],
            RepositoryValidators::default(),
            1,
        )
        .expect("repository");
        assert_eq!(
            parsed
                .packages
                .iter()
                .map(|row| (row.package_id.as_str(), row.version.as_str()))
                .collect::<Vec<_>>(),
            [
                ("a.package", "2"),
                ("a.package", "not-semver"),
                ("z.package", "1.0.0")
            ]
        );
        assert_eq!(parsed.issues.len(), 5);
        assert_eq!(
            parsed
                .issues
                .iter()
                .filter(|issue| issue.code == "resolver_metadata_invalid")
                .count(),
            3
        );
        assert!(parsed.packages.iter().all(|row| row.resolver.is_none()));
    }

    #[test]
    fn large_repository_without_archive_checksums_indexes_install_candidates() {
        let versions = (0..1_456)
            .map(|index| {
                let version = format!("1.{index}.0");
                let manifest = serde_json::json!({
                    "name": "com.example.tool",
                    "version": version,
                    "displayName": "Example Tool",
                    "url": "https://example.invalid/tool.zip",
                    "author": {"name": "Example"}
                });
                (version, manifest)
            })
            .collect::<Map<_, _>>();
        let bytes = serde_json::to_vec(&serde_json::json!({
            "packages": {"com.example.tool": {"versions": versions}}
        }))
        .expect("fixture repository");
        let parsed = parse_repository(
            &bytes,
            RepositorySource::Remote {
                url: "https://example.invalid/index.json".to_owned(),
            },
            vec![1],
            RepositoryValidators::default(),
            1,
        )
        .expect("missing checksums do not make the listing unreadable");
        assert_eq!(parsed.packages.len(), 1_456);
        assert!(
            parsed
                .packages
                .iter()
                .all(|row| row
                    .resolver
                    .as_ref()
                    .is_some_and(
                        |metadata| metadata.zip_sha256.is_none() && metadata.author_email.is_none()
                    ))
        );
        assert!(parsed.issues.is_empty());
    }

    #[test]
    fn prerelease_classification_uses_core_semver_and_preserves_unavailable() {
        assert_eq!(classify_prerelease("1.2.3"), Some(false));
        assert_eq!(classify_prerelease("1.2.3-beta.1+build.7"), Some(true));
        assert_eq!(classify_prerelease("not-semver"), None);
    }

    #[test]
    fn optional_package_links_are_sanitized_without_invalidating_package() {
        let document = br#"{
            "packages":{"com.example.package":{"versions":{"1.2.3":{
                "name":"com.example.package","version":"1.2.3",
                "displayName":"Example","url":"https://example.invalid/package.zip",
                "zipSHA256":"0000000000000000000000000000000000000000000000000000000000000000",
                "author":{"name":"Example","email":"dev@example.invalid"},
                "documentationUrl":"https://example.invalid/docs?q=1#intro",
                "changelogUrl":"https://user@example.invalid/changelog"
            }}}}
        }"#;
        let parsed = parse_repository(
            document,
            RepositorySource::Remote {
                url: "https://example.invalid/repository.json".to_owned(),
            },
            vec![1],
            RepositoryValidators::default(),
            1,
        )
        .expect("repository remains valid");
        assert!(parsed.packages[0].resolver.is_some());
        let links = parsed.packages[0].links.as_ref().expect("one valid link");
        assert_eq!(
            links.documentation.as_deref(),
            Some("https://example.invalid/docs?q=1#intro")
        );
        assert!(links.changelog.is_none());

        let wrong_type = document.to_vec();
        let text = String::from_utf8(wrong_type).expect("utf8").replace(
            "\"documentationUrl\":\"https://example.invalid/docs?q=1#intro\"",
            "\"documentationUrl\":42",
        );
        let parsed = parse_repository(
            text.as_bytes(),
            RepositorySource::Local {
                path: "fixture.json".to_owned(),
            },
            vec![1],
            RepositoryValidators::default(),
            1,
        )
        .expect("invalid optional link is ignored");
        assert!(parsed.packages[0].resolver.is_some());
        assert!(parsed.packages[0].links.is_none());
    }

    #[test]
    fn fragment_is_ignored_but_query_is_retained_and_userinfo_rejected() {
        assert_eq!(
            normalize_remote_url("https://example.invalid/repo.json?channel=beta#section")
                .expect("URL")
                .as_str(),
            "https://example.invalid/repo.json?channel=beta"
        );
        assert!(normalize_remote_url("https://user@example.invalid/repo.json").is_err());
        assert!(normalize_remote_url("file:///repo.json").is_err());
    }

    #[test]
    fn json_depth_and_issue_limits_fail_whole_parse() {
        let mut value = "null".to_owned();
        for _ in 0..=JSON_DEPTH_LIMIT {
            value = format!("[{value}]");
        }
        assert!(
            parse_bounded_json(value.as_bytes(), M3ErrorCode::RepositoryDocumentInvalid).is_err()
        );
        let mut issues = Vec::new();
        for index in 0..ISSUE_LIMIT {
            push_issue(&mut issues, "bad", "version", &index.to_string()).expect("bounded issue");
        }
        assert!(push_issue(&mut issues, "bad", "version", "overflow").is_err());
    }

    #[test]
    fn project_version_is_strict_and_bounded() {
        assert_eq!(
            parse_project_version(
                b"m_EditorVersion: 2022.3.22f1\nm_EditorVersionWithRevision: 2022.3.22f1 (abc)\n"
            )
            .expect("project version"),
            (
                "2022.3.22f1".to_owned(),
                Some("2022.3.22f1 (abc)".to_owned())
            )
        );
        assert!(parse_project_version(b"m_EditorVersion: a\nm_EditorVersion: b\n").is_err());
        assert!(parse_project_version(b"unrelated: value\n").is_err());
    }

    #[test]
    fn bom_null_and_malformed_repository_contract_is_stable() {
        let parsed = parse_repository(
            b"\xEF\xBB\xBF{\"id\":null,\"name\":null,\"packages\":{}}",
            RepositorySource::Local {
                path: "fixture.json".to_owned(),
            },
            vec![1],
            RepositoryValidators::default(),
            1,
        )
        .expect("BOM and nullable optional metadata");
        assert!(parsed.declared_id.is_none());
        assert!(parsed.name.is_none());
        assert!(
            parse_repository(
                b"{",
                RepositorySource::Local {
                    path: "fixture.json".to_owned(),
                },
                vec![1],
                RepositoryValidators::default(),
                1,
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn project_type_precedence_uses_locked_then_upm_then_legacy() {
        let root = std::env::temp_dir().join(format!(
            "alcomd-project-type-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        tokio::fs::create_dir_all(root.join("Assets/VRCSDK/Plugins"))
            .await
            .expect("create marker directory");
        tokio::fs::write(root.join("Assets/VRCSDK/Plugins/VRCSDK3A.dll"), b"")
            .await
            .expect("write marker");
        let dependency = |package_id: &str| DependencyIdentity {
            package_id: package_id.to_owned(),
            value: "raw".to_owned(),
        };
        assert_eq!(
            detect_project_type(
                &root,
                &[dependency("com.vrchat.worlds")],
                &[dependency("com.vrchat.avatars")],
            )
            .await
            .expect("locked precedence"),
            ProjectType::Worlds
        );
        assert_eq!(
            detect_project_type(&root, &[], &[dependency("com.vrchat.base")])
                .await
                .expect("UPM precedence"),
            ProjectType::UpmStarter
        );
        assert_eq!(
            detect_project_type(&root, &[], &[])
                .await
                .expect("legacy marker"),
            ProjectType::LegacyAvatars
        );
        tokio::fs::remove_dir_all(root)
            .await
            .expect("remove fixture");
    }
}
