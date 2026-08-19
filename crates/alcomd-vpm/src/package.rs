use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use reqwest::Url;
use semver::Version;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::range::VpmRange;
use crate::resolver::{PackageCandidate, PackageDependency, PackageSource};
use crate::{JSON_COLLECTION_LIMIT, M3ErrorCode, parse_bounded_json};

const PACKAGE_ID_LIMIT: usize = 128;
const VERSION_LIMIT: usize = 1_024;
const DISPLAY_NAME_LIMIT: usize = 4_096;
const DESCRIPTION_LIMIT: usize = 65_536;
const AUTHOR_FIELD_LIMIT: usize = 4_096;
const ARTIFACT_URL_LIMIT: usize = 32_768;
const UNITY_LIMIT: usize = 32;
const UNITY_RELEASE_LIMIT: usize = 64;
const LEGACY_ENTRY_LIMIT: usize = 1_024;
const DEPENDENCY_LIMIT: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryPackageContext {
    pub repository_id: String,
    pub repository_revision: u64,
    pub priority: u64,
    pub source_identity: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolverReadyPackage {
    pub candidate: PackageCandidate,
    pub display_name: String,
    pub description: Option<String>,
    pub author_name: String,
    pub author_email: String,
    pub unity_release: Option<String>,
    pub dependencies_json: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageManifestErrorCode {
    DocumentInvalid,
    ManifestInvalid,
    IdentityMismatch,
    HashRequired,
    SourceInvalid,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageManifestError {
    code: PackageManifestErrorCode,
}

impl PackageManifestError {
    #[must_use]
    pub const fn code(&self) -> PackageManifestErrorCode {
        self.code
    }
}

impl fmt::Display for PackageManifestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "package manifest rejected: {:?}", self.code)
    }
}

impl std::error::Error for PackageManifestError {}

pub fn parse_resolver_ready_repository(
    bytes: &[u8],
    context: &RepositoryPackageContext,
) -> Result<Vec<ResolverReadyPackage>, PackageManifestError> {
    validate_context(context)?;
    let value = parse_bounded_json(bytes, M3ErrorCode::RepositoryDocumentInvalid)
        .map_err(|_| manifest_error(PackageManifestErrorCode::DocumentInvalid))?;
    let repository = value
        .as_object()
        .ok_or_else(|| manifest_error(PackageManifestErrorCode::DocumentInvalid))?;
    let packages = repository
        .get("packages")
        .and_then(Value::as_object)
        .ok_or_else(|| manifest_error(PackageManifestErrorCode::DocumentInvalid))?;
    if packages.len() > JSON_COLLECTION_LIMIT {
        return Err(manifest_error(PackageManifestErrorCode::DocumentInvalid));
    }

    let mut parsed = Vec::new();
    for (package_key, package) in packages {
        validate_package_id(package_key)?;
        let versions = package
            .as_object()
            .and_then(|package| package.get("versions"))
            .and_then(Value::as_object)
            .ok_or_else(|| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
        if versions.len() > JSON_COLLECTION_LIMIT {
            return Err(manifest_error(PackageManifestErrorCode::DocumentInvalid));
        }
        for (version_key, manifest) in versions {
            let manifest = manifest
                .as_object()
                .ok_or_else(|| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
            parsed.push(parse_manifest(package_key, version_key, manifest, context)?);
        }
    }
    parsed.sort_by(|left, right| {
        left.candidate
            .package_id
            .as_bytes()
            .cmp(right.candidate.package_id.as_bytes())
            .then_with(|| {
                left.candidate
                    .version
                    .to_string()
                    .as_bytes()
                    .cmp(right.candidate.version.to_string().as_bytes())
            })
    });
    Ok(parsed)
}

pub(crate) fn validate_extracted_package(
    root: &Path,
    expected_name: &str,
    expected_version: &str,
) -> Result<(), PackageManifestError> {
    validate_package_id(expected_name)?;
    let expected_version = Version::parse(expected_version)
        .map_err(|_| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
    let path = root.join("package.json");
    let metadata = std::fs::symlink_metadata(&path)
        .map_err(|_| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 4 * 1024 * 1024
    {
        return Err(manifest_error(PackageManifestErrorCode::ManifestInvalid));
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return Err(manifest_error(PackageManifestErrorCode::ManifestInvalid));
        }
    }
    let bytes = std::fs::read(path)
        .map_err(|_| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
    let value = parse_bounded_json(&bytes, M3ErrorCode::RepositoryDocumentInvalid)
        .map_err(|_| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
    let object = value
        .as_object()
        .ok_or_else(|| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
    let name = required_string(object, "name", PACKAGE_ID_LIMIT)?;
    let version = Version::parse(required_string(object, "version", VERSION_LIMIT)?)
        .map_err(|_| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
    if name != expected_name || version != expected_version {
        return Err(manifest_error(PackageManifestErrorCode::IdentityMismatch));
    }
    Ok(())
}

fn parse_manifest(
    package_key: &str,
    version_key: &str,
    manifest: &Map<String, Value>,
    context: &RepositoryPackageContext,
) -> Result<ResolverReadyPackage, PackageManifestError> {
    let name = required_string(manifest, "name", PACKAGE_ID_LIMIT)?;
    let version_text = required_string(manifest, "version", VERSION_LIMIT)?;
    if name != package_key || version_text != version_key {
        return Err(manifest_error(PackageManifestErrorCode::IdentityMismatch));
    }
    validate_package_id(name)?;
    let version = Version::parse(version_text)
        .map_err(|_| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
    let display_name = required_string(manifest, "displayName", DISPLAY_NAME_LIMIT)?.to_owned();
    let description = optional_string(manifest, "description", DESCRIPTION_LIMIT)?;
    let artifact_url = parse_artifact_url(required_string(manifest, "url", ARTIFACT_URL_LIMIT)?)?;
    let archive_sha256 = parse_sha256(
        manifest
            .get("zipSHA256")
            .and_then(Value::as_str)
            .ok_or_else(|| manifest_error(PackageManifestErrorCode::HashRequired))?,
    )?;
    let author = manifest
        .get("author")
        .and_then(Value::as_object)
        .ok_or_else(|| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
    let author_name = required_string(author, "name", AUTHOR_FIELD_LIMIT)?.to_owned();
    let author_email = required_string(author, "email", AUTHOR_FIELD_LIMIT)?;
    if author_email.len() < 3 {
        return Err(manifest_error(PackageManifestErrorCode::ManifestInvalid));
    }
    let unity_minimum = optional_string(manifest, "unity", UNITY_LIMIT)?
        .as_deref()
        .map(parse_unity)
        .transpose()?;
    let unity_release = optional_string(manifest, "unityRelease", UNITY_RELEASE_LIMIT)?;
    let yanked = match manifest.get("yanked") {
        None => false,
        Some(Value::Bool(value)) => *value,
        Some(_) => return Err(manifest_error(PackageManifestErrorCode::ManifestInvalid)),
    };
    let (dependencies, dependencies_json) = parse_dependencies(manifest.get("vpmDependencies"))?;
    let legacy_metadata_present = ["legacyFolders", "legacyFiles", "legacyPackages"]
        .into_iter()
        .map(|key| parse_legacy(manifest.get(key)))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .any(|present| present);
    let manifest_fingerprint = Sha256::digest(
        serde_json::to_vec(manifest)
            .map_err(|_| manifest_error(PackageManifestErrorCode::ManifestInvalid))?,
    )
    .into();

    Ok(ResolverReadyPackage {
        candidate: PackageCandidate {
            package_id: name.to_owned(),
            version,
            yanked,
            unity_minimum,
            legacy_metadata_present,
            dependencies,
            source: PackageSource {
                repository_id: context.repository_id.clone(),
                repository_revision: context.repository_revision,
                priority: context.priority,
                source_identity: context.source_identity.clone(),
                manifest_fingerprint,
                artifact_url,
                archive_sha256,
            },
        },
        display_name,
        description,
        author_name,
        author_email: author_email.to_owned(),
        unity_release,
        dependencies_json,
    })
}

fn validate_context(context: &RepositoryPackageContext) -> Result<(), PackageManifestError> {
    if context.repository_id.is_empty()
        || context.repository_id.len() > 128
        || context.repository_revision == 0
        || context.priority == 0
        || context.source_identity.is_empty()
        || context.source_identity.len() > 2_048
    {
        return Err(manifest_error(PackageManifestErrorCode::SourceInvalid));
    }
    Ok(())
}

fn validate_package_id(value: &str) -> Result<(), PackageManifestError> {
    if value.is_empty()
        || value.len() > PACKAGE_ID_LIMIT
        || !value.as_bytes()[0].is_ascii_alphanumeric()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(manifest_error(PackageManifestErrorCode::ManifestInvalid));
    }
    Ok(())
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    limit: usize,
) -> Result<&'a str, PackageManifestError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= limit)
        .ok_or_else(|| manifest_error(PackageManifestErrorCode::ManifestInvalid))
}

fn optional_string(
    object: &Map<String, Value>,
    key: &str,
    limit: usize,
) -> Result<Option<String>, PackageManifestError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if !value.is_empty() && value.len() <= limit => {
            Ok(Some(value.clone()))
        }
        _ => Err(manifest_error(PackageManifestErrorCode::ManifestInvalid)),
    }
}

fn parse_artifact_url(value: &str) -> Result<String, PackageManifestError> {
    let mut url =
        Url::parse(value).map_err(|_| manifest_error(PackageManifestErrorCode::SourceInvalid))?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(manifest_error(PackageManifestErrorCode::SourceInvalid));
    }
    url.set_fragment(None);
    Ok(url.to_string())
}

fn parse_sha256(value: &str) -> Result<[u8; 32], PackageManifestError> {
    if value.len() != 64
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(manifest_error(PackageManifestErrorCode::HashRequired));
    }
    let mut digest = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        digest[index] = (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]);
    }
    Ok(digest)
}

fn hex_nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => 0,
    }
}

fn parse_unity(value: &str) -> Result<(u64, u64), PackageManifestError> {
    let Some((major, minor)) = value.split_once('.') else {
        return Err(manifest_error(PackageManifestErrorCode::ManifestInvalid));
    };
    if major.is_empty()
        || minor.is_empty()
        || minor.contains('.')
        || !major.bytes().all(|byte| byte.is_ascii_digit())
        || !minor.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(manifest_error(PackageManifestErrorCode::ManifestInvalid));
    }
    Ok((
        major
            .parse()
            .map_err(|_| manifest_error(PackageManifestErrorCode::ManifestInvalid))?,
        minor
            .parse()
            .map_err(|_| manifest_error(PackageManifestErrorCode::ManifestInvalid))?,
    ))
}

fn parse_dependencies(
    value: Option<&Value>,
) -> Result<(Vec<PackageDependency>, String), PackageManifestError> {
    let Some(value) = value else {
        return Ok((Vec::new(), "{}".to_owned()));
    };
    let dependencies = value
        .as_object()
        .ok_or_else(|| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
    if dependencies.len() > DEPENDENCY_LIMIT {
        return Err(manifest_error(PackageManifestErrorCode::ManifestInvalid));
    }
    let mut normalized = BTreeMap::<String, String>::new();
    let mut parsed = Vec::with_capacity(dependencies.len());
    for (package_id, range_value) in dependencies {
        validate_package_id(package_id)?;
        let range_text = range_value
            .as_str()
            .filter(|value| !value.is_empty() && value.len() <= VERSION_LIMIT)
            .ok_or_else(|| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
        let range = VpmRange::parse(range_text)
            .map_err(|_| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
        let canonical = range.canonical();
        normalized.insert(package_id.clone(), canonical.clone());
        parsed.push(PackageDependency {
            package_id: package_id.clone(),
            range: canonical,
        });
    }
    let json = serde_json::to_string(&normalized)
        .map_err(|_| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
    Ok((parsed, json))
}

fn parse_legacy(value: Option<&Value>) -> Result<bool, PackageManifestError> {
    let Some(value) = value else {
        return Ok(false);
    };
    let entries = value
        .as_array()
        .ok_or_else(|| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
    if entries.len() > DEPENDENCY_LIMIT {
        return Err(manifest_error(PackageManifestErrorCode::ManifestInvalid));
    }
    for entry in entries {
        let valid = entry
            .as_str()
            .is_some_and(|value| !value.is_empty() && value.len() <= LEGACY_ENTRY_LIMIT);
        if !valid {
            return Err(manifest_error(PackageManifestErrorCode::ManifestInvalid));
        }
    }
    Ok(!entries.is_empty())
}

const fn manifest_error(code: PackageManifestErrorCode) -> PackageManifestError {
    PackageManifestError { code }
}

#[cfg(test)]
mod tests {
    use super::*;

    const READY: &[u8] =
        include_bytes!("../../alcomd-testing/fixtures/m4/repository-resolver-ready.json");
    const MISMATCH: &[u8] =
        include_bytes!("../../alcomd-testing/fixtures/m4/repository-key-mismatch.json");

    fn context() -> RepositoryPackageContext {
        RepositoryPackageContext {
            repository_id: "00000000-0000-4000-8000-000000000301".to_owned(),
            repository_revision: 4,
            priority: 1,
            source_identity: "remote:https://fixtures.invalid/repository.json".to_owned(),
        }
    }

    #[test]
    fn public_fixture_becomes_resolver_ready_without_hidden_defaults() {
        let parsed = parse_resolver_ready_repository(READY, &context()).expect("ready fixture");
        assert_eq!(parsed.len(), 2);
        let base = &parsed[0];
        assert_eq!(base.candidate.package_id, "com.example.base");
        assert_eq!(base.candidate.version, Version::new(1, 2, 3));
        assert_eq!(base.candidate.unity_minimum, Some((2022, 3)));
        assert!(!base.candidate.legacy_metadata_present);
        assert_eq!(base.candidate.source.archive_sha256[0], 0x01);
        assert_eq!(base.candidate.source.archive_sha256[31], 0xef);
        assert_eq!(base.dependencies_json, "{}");
        let feature = &parsed[1];
        assert_eq!(feature.candidate.dependencies.len(), 1);
        assert_eq!(feature.candidate.dependencies[0].range, "<2.0.0 >=1.2.0");
    }

    #[test]
    fn map_key_mismatch_fails_the_complete_refresh() {
        assert_eq!(
            parse_resolver_ready_repository(MISMATCH, &context())
                .expect_err("mismatch")
                .code(),
            PackageManifestErrorCode::IdentityMismatch
        );
    }

    #[test]
    fn missing_malformed_or_uppercase_hash_is_never_resolver_ready() {
        for hash in [None, Some("bad"), Some(&"A".repeat(64))] {
            let mut document: Value = serde_json::from_slice(READY).expect("fixture");
            let manifest = &mut document["packages"]["com.example.base"]["versions"]["1.2.3"];
            match hash {
                Some(value) => manifest["zipSHA256"] = Value::String(value.to_owned()),
                None => {
                    manifest
                        .as_object_mut()
                        .expect("manifest")
                        .remove("zipSHA256");
                }
            }
            assert_eq!(
                parse_resolver_ready_repository(
                    &serde_json::to_vec(&document).expect("document"),
                    &context(),
                )
                .expect_err("hash must fail")
                .code(),
                PackageManifestErrorCode::HashRequired
            );
        }
    }

    #[test]
    fn artifact_credentials_and_non_http_schemes_fail_closed() {
        for url in [
            "https://token@fixtures.invalid/package.zip",
            "file:///package.zip",
        ] {
            let mut document: Value = serde_json::from_slice(READY).expect("fixture");
            document["packages"]["com.example.base"]["versions"]["1.2.3"]["url"] =
                Value::String(url.to_owned());
            assert_eq!(
                parse_resolver_ready_repository(
                    &serde_json::to_vec(&document).expect("document"),
                    &context(),
                )
                .expect_err("source must fail")
                .code(),
                PackageManifestErrorCode::SourceInvalid
            );
        }
    }

    #[test]
    fn fingerprint_is_independent_of_object_key_order() {
        let left = parse_resolver_ready_repository(READY, &context()).expect("left");
        let value: Value = serde_json::from_slice(READY).expect("fixture");
        let right = parse_resolver_ready_repository(
            &serde_json::to_vec_pretty(&value).expect("reformatted"),
            &context(),
        )
        .expect("right");
        assert_eq!(
            left[0].candidate.source.manifest_fingerprint,
            right[0].candidate.source.manifest_fingerprint
        );
    }
}
