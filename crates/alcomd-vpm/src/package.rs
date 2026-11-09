use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use reqwest::Url;
use semver::Version;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::range::VpmRange;
use crate::resolver::{PackageCandidate, PackageDependency, PackageSource, PackageSourceAuthority};
use crate::unity_version::package_minimum;
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
    pub author_email: Option<String>,
    pub unity_release: Option<String>,
    pub dependencies_json: String,
    pub(crate) ignored_publisher_digest: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageManifestErrorCode {
    DocumentInvalid,
    ManifestInvalid,
    IdentityMismatch,
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
    let expected_version = Version::parse(expected_version)
        .map_err(|_| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
    if extracted_package_version(root, expected_name)? != expected_version {
        return Err(manifest_error(PackageManifestErrorCode::IdentityMismatch));
    }
    Ok(())
}

pub(crate) fn extracted_package_version(
    root: &Path,
    expected_name: &str,
) -> Result<Version, PackageManifestError> {
    validate_package_id(expected_name)?;
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
    if name != expected_name {
        return Err(manifest_error(PackageManifestErrorCode::IdentityMismatch));
    }
    Ok(version)
}

pub(crate) fn parse_manifest(
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
    let download_headers_present = if manifest.contains_key("headers") {
        let artifact = reqwest::Url::parse(&artifact_url)
            .map_err(|_| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
        crate::repository_auth::RepositoryAuth::for_package(manifest.get("headers"), &artifact)
            .map_err(|_| manifest_error(PackageManifestErrorCode::ManifestInvalid))?
            .is_some()
    } else {
        false
    };
    let declared_digest = manifest.get("zipSHA256");
    let archive_sha256 = declared_digest
        .and_then(Value::as_str)
        .and_then(parse_sha256);
    let author = manifest
        .get("author")
        .and_then(Value::as_object)
        .ok_or_else(|| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
    let author_name = required_string(author, "name", AUTHOR_FIELD_LIMIT)?.to_owned();
    let author_email = optional_string(author, "email", AUTHOR_FIELD_LIMIT)?;
    let unity = optional_string(manifest, "unity", UNITY_LIMIT)?;
    let unity_release = optional_string(manifest, "unityRelease", UNITY_RELEASE_LIMIT)?;
    let unity_minimum = package_minimum(unity.as_deref(), unity_release.as_deref())
        .map_err(|_| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
    let yanked = alcomd_application::package_yanked(manifest)
        .ok_or_else(|| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
    let (dependencies, dependencies_json) = parse_dependencies(manifest.get("vpmDependencies"))?;
    let legacy_metadata = LegacyMetadata::parse(manifest)?;
    let legacy_metadata_present = !legacy_metadata.is_empty();
    let manifest_fingerprint = Sha256::digest(
        manifest_evidence_bytes(manifest, download_headers_present)
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
                authority: PackageSourceAuthority::Repository {
                    credential_reference: None,
                    download_headers_present,
                    repository_locator: None,
                    repository_id: context.repository_id.clone(),
                    repository_revision: context.repository_revision,
                    priority: context.priority,
                    artifact_url,
                },
                source_identity: context.source_identity.clone(),
                manifest_fingerprint,
                archive_sha256,
            },
        },
        display_name,
        description,
        author_name,
        author_email,
        unity_release,
        dependencies_json,
        ignored_publisher_digest: declared_digest.is_some() && archive_sha256.is_none(),
    })
}

/// Seal the complete declaration while excluding the plaintext HTTP headers
/// from any legacy-evidence file addressed by this fingerprint.
pub(crate) fn manifest_evidence_bytes(
    manifest: &Map<String, Value>,
    download_headers_present: bool,
) -> Result<Vec<u8>, serde_json::Error> {
    if !download_headers_present {
        return serde_json::to_vec(manifest);
    }
    let mut evidence = manifest.clone();
    let headers = serde_json::to_vec(&evidence.remove("headers"))?;
    let digest: [u8; 32] = Sha256::digest(headers).into();
    evidence.insert("headers".into(), Value::String(crate::digest_text(&digest)));
    serde_json::to_vec(&evidence)
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

fn parse_sha256(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut digest = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        digest[index] = (hex_nibble(pair[0]) << 4) | hex_nibble(pair[1]);
    }
    Some(digest)
}

fn hex_nibble(value: u8) -> u8 {
    match value.to_ascii_lowercase() {
        b'0'..=b'9' => value - b'0',
        lower => lower - b'a' + 10,
    }
}

pub(crate) fn parse_dependencies(
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

/// VPM paths map to GUIDs. They are evidence, never permission to delete.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct LegacyMetadata {
    pub folders: BTreeMap<String, String>,
    pub files: BTreeMap<String, String>,
    pub packages: Vec<String>,
}

impl LegacyMetadata {
    pub(crate) fn parse(manifest: &Map<String, Value>) -> Result<Self, PackageManifestError> {
        let paths = |key| -> Result<BTreeMap<String, String>, PackageManifestError> {
            let Some(value) = manifest.get(key) else {
                return Ok(BTreeMap::new());
            };
            let entries = value
                .as_object()
                .filter(|entries| entries.len() <= DEPENDENCY_LIMIT)
                .ok_or_else(|| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
            entries
                .iter()
                .map(|(path, guid)| {
                    let guid = guid
                        .as_str()
                        .filter(|guid| {
                            guid.is_empty()
                                || guid.len() == 32
                                    && guid.bytes().all(|byte| byte.is_ascii_hexdigit())
                        })
                        .ok_or_else(|| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
                    if path.is_empty()
                        || path.len() > LEGACY_ENTRY_LIMIT
                        || path.chars().any(char::is_control)
                    {
                        return Err(manifest_error(PackageManifestErrorCode::ManifestInvalid));
                    }
                    Ok((path.clone(), guid.to_ascii_lowercase()))
                })
                .collect()
        };
        let packages = match manifest.get("legacyPackages") {
            None => Vec::new(),
            Some(value) => value
                .as_array()
                .filter(|values| values.len() <= DEPENDENCY_LIMIT)
                .ok_or_else(|| manifest_error(PackageManifestErrorCode::ManifestInvalid))?
                .iter()
                .map(|value| {
                    let id = value
                        .as_str()
                        .ok_or_else(|| manifest_error(PackageManifestErrorCode::ManifestInvalid))?;
                    validate_package_id(id)?;
                    Ok(id.to_owned())
                })
                .collect::<Result<Vec<_>, PackageManifestError>>()?,
        };
        Ok(Self {
            folders: paths("legacyFolders")?,
            files: paths("legacyFiles")?,
            packages,
        })
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.folders.is_empty() && self.files.is_empty() && self.packages.is_empty()
    }
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
    fn frozen_official_legacy_shape_is_readable_and_preserves_guids() {
        let bytes =
            include_bytes!("../../alcomd-testing/fixtures/m4/official-vpm-legacy-sample.json");
        let parsed = parse_resolver_ready_repository(bytes, &context()).expect("official sample");
        assert_eq!(parsed.len(), 1);
        assert!(parsed[0].candidate.legacy_metadata_present);
        let document: Value = serde_json::from_slice(bytes).expect("sample");
        let package = document["packages"]
            .as_object()
            .expect("packages")
            .values()
            .next()
            .expect("package");
        let manifest = package["versions"]
            .as_object()
            .expect("versions")
            .values()
            .next()
            .expect("version")
            .as_object()
            .expect("manifest");
        let legacy = LegacyMetadata::parse(manifest).expect("legacy");
        for (path, guid) in &legacy.folders {
            assert_eq!(manifest["legacyFolders"][path], *guid);
        }
        assert!(
            LegacyMetadata::parse(
                serde_json::json!({"legacyFolders":[]})
                    .as_object()
                    .expect("object")
            )
            .is_err()
        );
        assert!(
            LegacyMetadata::parse(
                serde_json::json!({"legacyPackages":{}})
                    .as_object()
                    .expect("object")
            )
            .is_err()
        );
    }

    #[test]
    fn public_fixture_becomes_resolver_ready_without_hidden_defaults() {
        let parsed = parse_resolver_ready_repository(READY, &context()).expect("ready fixture");
        assert_eq!(parsed.len(), 2);
        let base = &parsed[0];
        assert_eq!(base.candidate.package_id, "com.example.base");
        assert_eq!(base.candidate.version, Version::new(1, 2, 3));
        assert_eq!(
            base.candidate.unity_minimum,
            crate::UnityVersion::parse_editor("2022.3.22f1")
        );
        assert!(!base.candidate.legacy_metadata_present);
        let digest = base
            .candidate
            .source
            .archive_sha256
            .expect("declared digest");
        assert_eq!(digest[0], 0x01);
        assert_eq!(digest[31], 0xef);
        assert_eq!(base.dependencies_json, "{}");
        let feature = &parsed[1];
        assert_eq!(feature.candidate.dependencies.len(), 1);
        assert_eq!(feature.candidate.dependencies[0].range, "^1.2.0");
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
    fn package_ids_cannot_encode_paths_or_platform_roots() {
        for package_id in [
            "../escape",
            "com/example/package",
            r"com\example\package",
            "C:escape",
            r"C:\escape",
            r"\\server\share",
            ".leading",
            "trailing/",
        ] {
            assert!(validate_package_id(package_id).is_err(), "{package_id}");
        }
        validate_package_id("com.example-valid_package-1").expect("valid package ID");
    }

    #[test]
    fn invalid_publisher_digest_is_ignored_and_hex_case_is_accepted() {
        for (hash, expected, ignored) in [
            (None, None, false),
            (Some(Value::Null), None, true),
            (Some(serde_json::json!(123)), None, true),
            (Some(Value::String("bad".to_owned())), None, true),
            (Some(Value::String("z".repeat(64))), None, true),
            (
                Some(Value::String("aB".repeat(32))),
                Some([0xab; 32]),
                false,
            ),
        ] {
            let mut document: Value = serde_json::from_slice(READY).expect("fixture");
            let manifest = &mut document["packages"]["com.example.base"]["versions"]["1.2.3"];
            if let Some(hash) = hash {
                manifest["zipSHA256"] = hash;
            } else {
                manifest
                    .as_object_mut()
                    .expect("manifest")
                    .remove("zipSHA256");
            }
            let packages = parse_resolver_ready_repository(
                &serde_json::to_vec(&document).expect("document"),
                &context(),
            )
            .expect("package remains resolver-ready");
            assert_eq!(packages[0].candidate.source.archive_sha256, expected);
            assert_eq!(packages[0].ignored_publisher_digest, ignored);
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
    fn evidence_seals_the_header_values_without_writing_them() {
        let mut declaration = serde_json::json!({
            "name": "com.example.package", "version": "1.0.0",
            "legacyFolders": {"Assets/OldFixture": null},
            "headers": {"Authorization": "fixture-package-secret"}
        });
        let original =
            manifest_evidence_bytes(declaration.as_object().expect("fixture object"), true)
                .expect("sealed evidence");
        let sealed: Value = serde_json::from_slice(&original).expect("readable legacy evidence");
        assert_eq!(sealed["legacyFolders"], declaration["legacyFolders"]);
        assert!(
            !String::from_utf8(original.clone())
                .expect("UTF-8 evidence")
                .contains("fixture-package-secret")
        );
        declaration["headers"]["Authorization"] = serde_json::json!("changed-fixture-secret");
        assert_ne!(
            original,
            manifest_evidence_bytes(declaration.as_object().expect("fixture object"), true)
                .expect("new header is bound")
        );
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
