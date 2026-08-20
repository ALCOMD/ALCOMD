use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use alcomd_application::TemplateId;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use zip::ZipArchive;

use crate::archive::{
    ArchiveEntry, ArchiveErrorCode, ArchiveLimits, extract_archive_with_limits,
    preflight_archive_with_limits,
};
use crate::range::VpmRange;

pub const TEMPLATE_MANIFEST_BYTES: u64 = 1_048_576;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TemplateErrorCode {
    BundleInvalid,
    ManifestInvalid,
    DigestMismatch,
    ResourceInvalid,
    Io,
}

#[derive(Debug)]
pub struct TemplateError {
    code: TemplateErrorCode,
}

impl TemplateError {
    #[must_use]
    pub const fn code(&self) -> TemplateErrorCode {
        self.code
    }
}

impl fmt::Display for TemplateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Template bundle rejected")
    }
}

impl std::error::Error for TemplateError {}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplateManifest {
    pub format_version: u32,
    pub template_id: String,
    pub template_version: String,
    pub display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub unity: TemplateUnityCompatibility,
    pub dependencies: Vec<TemplateDependency>,
    pub additional_resources: Vec<TemplateResource>,
    pub payload: TemplatePayload,
    pub provenance: TemplateProvenance,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemplateUnityCompatibility {
    pub major: u32,
    pub minor: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplateDependency {
    pub package_id: String,
    pub version_range: String,
    #[serde(default)]
    pub include_prerelease: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplateResource {
    pub bundle_path: String,
    pub target_path: String,
    pub sha256: String,
    pub byte_size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplatePayload {
    pub root: String,
    pub tree_sha256: String,
    pub entry_count: u64,
    pub total_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TemplateProvenance {
    pub created_by: TemplateProvenanceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_from_template_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_from_project_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateProvenanceKind {
    Authored,
    Imported,
    Derived,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateInspection {
    pub manifest: TemplateManifest,
    pub normalized_manifest_json: String,
    pub bundle_sha256: [u8; 32],
    pub manifest_fingerprint: [u8; 32],
    pub payload_tree_sha256: [u8; 32],
    pub entry_count: u64,
    pub total_uncompressed_bytes: u64,
}

pub fn inspect_template_bundle(path: &Path) -> Result<TemplateInspection, TemplateError> {
    inspect_template_bundle_with_limits(path, ArchiveLimits::template())
}

pub fn inspect_template_bundle_with_limits(
    path: &Path,
    limits: ArchiveLimits,
) -> Result<TemplateInspection, TemplateError> {
    let preflight = preflight_archive_with_limits(path, limits).map_err(map_archive_error)?;
    let layout = validate_layout(&preflight.entries)?;
    let file = File::open(path).map_err(|_| error(TemplateErrorCode::Io))?;
    let mut archive = ZipArchive::new(file).map_err(|_| error(TemplateErrorCode::BundleInvalid))?;
    let manifest_bytes =
        read_entry_bounded(&mut archive, layout.manifest_index, TEMPLATE_MANIFEST_BYTES)?;
    if manifest_bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        return Err(error(TemplateErrorCode::ManifestInvalid));
    }
    let manifest: TemplateManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| error(TemplateErrorCode::ManifestInvalid))?;
    validate_manifest(&manifest)?;
    let normalized_manifest_json = canonical_manifest(&manifest)?;
    let manifest_fingerprint = sha256(normalized_manifest_json.as_bytes());
    let (payload_tree_sha256, payload_bytes, payload_count) =
        hash_payload_tree(&mut archive, &layout.payload_files)?;
    if payload_tree_sha256 != parse_digest(&manifest.payload.tree_sha256)?
        || payload_bytes != manifest.payload.total_bytes
        || payload_count != manifest.payload.entry_count
    {
        return Err(error(TemplateErrorCode::DigestMismatch));
    }
    validate_resources(&mut archive, &layout.resource_files, &manifest)?;
    let bundle_sha256 = hash_file(path)?;
    Ok(TemplateInspection {
        manifest,
        normalized_manifest_json,
        bundle_sha256,
        manifest_fingerprint,
        payload_tree_sha256,
        entry_count: u64::try_from(preflight.entries.len())
            .map_err(|_| error(TemplateErrorCode::BundleInvalid))?,
        total_uncompressed_bytes: preflight.total_uncompressed_bytes,
    })
}

/// Materializes one already validated bundle into an empty unpublished project root.
pub(crate) fn materialize_template_bundle(
    bundle: &Path,
    destination: &Path,
) -> Result<TemplateInspection, TemplateError> {
    let inspection = inspect_template_bundle(bundle)?;
    let destination_metadata =
        std::fs::symlink_metadata(destination).map_err(|_| error(TemplateErrorCode::Io))?;
    if !destination_metadata.is_dir()
        || template_path_is_link(&destination_metadata)
        || std::fs::read_dir(destination)
            .map_err(|_| error(TemplateErrorCode::Io))?
            .next()
            .is_some()
    {
        return Err(error(TemplateErrorCode::Io));
    }
    let source = destination.join(".alcomd-template-source");
    std::fs::create_dir(&source).map_err(|_| error(TemplateErrorCode::Io))?;
    if let Err(source_error) =
        extract_archive_with_limits(bundle, &source, ArchiveLimits::template())
    {
        let _ = std::fs::remove_dir_all(&source);
        return Err(map_archive_error(source_error));
    }
    let payload = source.join("payload");
    let entries = std::fs::read_dir(&payload).map_err(|_| error(TemplateErrorCode::Io))?;
    for entry in entries {
        let entry = entry.map_err(|_| error(TemplateErrorCode::Io))?;
        let target = destination.join(entry.file_name());
        if std::fs::symlink_metadata(&target).is_ok() {
            return Err(error(TemplateErrorCode::ResourceInvalid));
        }
        std::fs::rename(entry.path(), target).map_err(|_| error(TemplateErrorCode::Io))?;
    }
    std::fs::remove_dir(&payload).map_err(|_| error(TemplateErrorCode::Io))?;
    for resource in &inspection.manifest.additional_resources {
        let input = source.join(&resource.bundle_path);
        let output = destination.join(&resource.target_path);
        let parent = output
            .parent()
            .ok_or_else(|| error(TemplateErrorCode::ResourceInvalid))?;
        ensure_safe_directory_chain(destination, parent)?;
        if std::fs::symlink_metadata(&output).is_ok() {
            return Err(error(TemplateErrorCode::ResourceInvalid));
        }
        std::fs::rename(input, output).map_err(|_| error(TemplateErrorCode::Io))?;
    }
    std::fs::remove_dir_all(&source).map_err(|_| error(TemplateErrorCode::Io))?;
    Ok(inspection)
}

fn ensure_safe_directory_chain(root: &Path, target: &Path) -> Result<(), TemplateError> {
    let relative = target
        .strip_prefix(root)
        .map_err(|_| error(TemplateErrorCode::ResourceInvalid))?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.is_dir() && !template_path_is_link(&metadata) => {}
            Ok(_) => return Err(error(TemplateErrorCode::ResourceInvalid)),
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                std::fs::create_dir(&current).map_err(|_| error(TemplateErrorCode::Io))?;
            }
            Err(_) => return Err(error(TemplateErrorCode::Io)),
        }
    }
    Ok(())
}

#[cfg(windows)]
fn template_path_is_link(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_type().is_symlink() || metadata.file_attributes() & 0x400 != 0
}

#[cfg(unix)]
fn template_path_is_link(metadata: &std::fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

struct TemplateLayout {
    manifest_index: usize,
    payload_files: Vec<(String, usize, u64)>,
    resource_files: BTreeMap<String, (usize, u64)>,
}

fn validate_layout(entries: &[ArchiveEntry]) -> Result<TemplateLayout, TemplateError> {
    let mut manifest_index = None;
    let mut payload_files = Vec::new();
    let mut resource_files = BTreeMap::new();
    let mut required = BTreeSet::new();
    for entry in entries {
        let path = slash_path(&entry.relative_path)?;
        if path == "template.json" {
            if entry.directory || manifest_index.replace(entry.index).is_some() {
                return Err(error(TemplateErrorCode::BundleInvalid));
            }
            if entry.uncompressed_size > TEMPLATE_MANIFEST_BYTES {
                return Err(error(TemplateErrorCode::ManifestInvalid));
            }
            continue;
        }
        if let Some(relative) = path.strip_prefix("payload/") {
            if relative.is_empty() {
                if !entry.directory {
                    return Err(error(TemplateErrorCode::BundleInvalid));
                }
            } else if !entry.directory {
                required.insert(relative.to_owned());
                payload_files.push((relative.to_owned(), entry.index, entry.uncompressed_size));
            }
            continue;
        }
        if let Some(relative) = path.strip_prefix("resources/") {
            if relative.is_empty() {
                if !entry.directory {
                    return Err(error(TemplateErrorCode::BundleInvalid));
                }
            } else if !entry.directory {
                resource_files.insert(path, (entry.index, entry.uncompressed_size));
            }
            continue;
        }
        return Err(error(TemplateErrorCode::BundleInvalid));
    }
    if !required.contains("ProjectSettings/ProjectVersion.txt")
        || !required.contains("Packages/manifest.json")
        || payload_files.is_empty()
    {
        return Err(error(TemplateErrorCode::BundleInvalid));
    }
    payload_files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(TemplateLayout {
        manifest_index: manifest_index.ok_or_else(|| error(TemplateErrorCode::ManifestInvalid))?,
        payload_files,
        resource_files,
    })
}

fn validate_manifest(manifest: &TemplateManifest) -> Result<(), TemplateError> {
    if manifest.format_version != 1
        || TemplateId::parse(&manifest.template_id).is_err()
        || !valid_token(&manifest.template_version, 128)
        || !valid_text(&manifest.display_name, 128, false)
        || manifest
            .description
            .as_ref()
            .is_some_and(|value| !valid_text(value, 4_096, true))
        || manifest.unity.major == 0
        || manifest.unity.major > 9_999
        || manifest.unity.minor > 9_999
        || manifest.dependencies.len() > 1_024
        || manifest.additional_resources.len() > 4_096
        || manifest.payload.root != "payload/"
        || manifest.payload.entry_count == 0
        || manifest.payload.entry_count > 100_000
        || manifest.payload.total_bytes == 0
        || manifest.payload.total_bytes > ArchiveLimits::template().total_uncompressed_bytes
    {
        return Err(error(TemplateErrorCode::ManifestInvalid));
    }
    let mut dependencies = BTreeSet::new();
    for dependency in &manifest.dependencies {
        if !valid_package_id(&dependency.package_id)
            || VpmRange::parse(&dependency.version_range).is_err()
            || !dependencies.insert((
                dependency.package_id.clone(),
                dependency.version_range.clone(),
                dependency.include_prerelease,
            ))
        {
            return Err(error(TemplateErrorCode::ManifestInvalid));
        }
    }
    let mut resources = BTreeSet::new();
    let mut targets = BTreeSet::new();
    for resource in &manifest.additional_resources {
        if !resource.bundle_path.starts_with("resources/")
            || !valid_relative_contract_path(&resource.bundle_path)
            || !valid_relative_contract_path(&resource.target_path)
            || parse_digest(&resource.sha256).is_err()
            || resource.byte_size > ArchiveLimits::template().entry_bytes
            || !resources.insert(resource.bundle_path.clone())
            || !targets.insert(resource.target_path.to_lowercase())
        {
            return Err(error(TemplateErrorCode::ManifestInvalid));
        }
    }
    for id in [
        manifest.provenance.derived_from_template_id.as_deref(),
        manifest.provenance.derived_from_project_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        if TemplateId::parse(id).is_err() {
            return Err(error(TemplateErrorCode::ManifestInvalid));
        }
    }
    Ok(())
}

fn validate_resources<R: Read + io::Seek>(
    archive: &mut ZipArchive<R>,
    files: &BTreeMap<String, (usize, u64)>,
    manifest: &TemplateManifest,
) -> Result<(), TemplateError> {
    let declared = manifest
        .additional_resources
        .iter()
        .map(|resource| (resource.bundle_path.as_str(), resource))
        .collect::<BTreeMap<_, _>>();
    if files.len() != declared.len()
        || files
            .keys()
            .any(|path| !declared.contains_key(path.as_str()))
    {
        return Err(error(TemplateErrorCode::ResourceInvalid));
    }
    for (path, (index, size)) in files {
        let resource = declared
            .get(path.as_str())
            .ok_or_else(|| error(TemplateErrorCode::ResourceInvalid))?;
        if *size != resource.byte_size {
            return Err(error(TemplateErrorCode::ResourceInvalid));
        }
        let mut entry = archive
            .by_index(*index)
            .map_err(|_| error(TemplateErrorCode::BundleInvalid))?;
        let digest = hash_reader(&mut entry, *size)?;
        if digest != parse_digest(&resource.sha256)? {
            return Err(error(TemplateErrorCode::ResourceInvalid));
        }
    }
    Ok(())
}

fn hash_payload_tree<R: Read + io::Seek>(
    archive: &mut ZipArchive<R>,
    files: &[(String, usize, u64)],
) -> Result<([u8; 32], u64, u64), TemplateError> {
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    for (path, index, size) in files {
        let path = path.as_bytes();
        digest.update(
            u32::try_from(path.len())
                .map_err(|_| error(TemplateErrorCode::BundleInvalid))?
                .to_le_bytes(),
        );
        digest.update(path);
        digest.update(size.to_le_bytes());
        let mut entry = archive
            .by_index(*index)
            .map_err(|_| error(TemplateErrorCode::BundleInvalid))?;
        copy_into_digest(&mut entry, *size, &mut digest)?;
        total = total
            .checked_add(*size)
            .ok_or_else(|| error(TemplateErrorCode::BundleInvalid))?;
    }
    Ok((digest.finalize().into(), total, files.len() as u64))
}

fn read_entry_bounded<R: Read + io::Seek>(
    archive: &mut ZipArchive<R>,
    index: usize,
    maximum: u64,
) -> Result<Vec<u8>, TemplateError> {
    let mut entry = archive
        .by_index(index)
        .map_err(|_| error(TemplateErrorCode::BundleInvalid))?;
    let capacity = usize::try_from(entry.size().min(maximum))
        .map_err(|_| error(TemplateErrorCode::ManifestInvalid))?;
    let mut bytes = Vec::with_capacity(capacity);
    entry
        .by_ref()
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| error(TemplateErrorCode::Io))?;
    if bytes.len() as u64 > maximum || bytes.len() as u64 != entry.size() {
        return Err(error(TemplateErrorCode::ManifestInvalid));
    }
    Ok(bytes)
}

fn canonical_manifest(manifest: &TemplateManifest) -> Result<String, TemplateError> {
    let mut value =
        serde_json::to_value(manifest).map_err(|_| error(TemplateErrorCode::ManifestInvalid))?;
    sort_json(&mut value);
    serde_json::to_string(&value).map_err(|_| error(TemplateErrorCode::ManifestInvalid))
}

fn sort_json(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let original = std::mem::take(object);
            let mut sorted = original.into_iter().collect::<BTreeMap<_, _>>();
            for value in sorted.values_mut() {
                sort_json(value);
            }
            *object = sorted.into_iter().collect::<Map<_, _>>();
        }
        Value::Array(values) => values.iter_mut().for_each(sort_json),
        _ => {}
    }
}

fn hash_file(path: &Path) -> Result<[u8; 32], TemplateError> {
    let mut file = File::open(path).map_err(|_| error(TemplateErrorCode::Io))?;
    let size = file
        .metadata()
        .map_err(|_| error(TemplateErrorCode::Io))?
        .len();
    hash_reader(&mut file, size)
}

fn hash_reader(reader: &mut impl Read, expected: u64) -> Result<[u8; 32], TemplateError> {
    let mut digest = Sha256::new();
    copy_into_digest(reader, expected, &mut digest)?;
    Ok(digest.finalize().into())
}

fn copy_into_digest(
    reader: &mut impl Read,
    expected: u64,
    digest: &mut Sha256,
) -> Result<(), TemplateError> {
    let mut remaining = expected;
    let mut buffer = [0_u8; 64 * 1024];
    while remaining > 0 {
        let maximum = usize::try_from(remaining.min(buffer.len() as u64))
            .map_err(|_| error(TemplateErrorCode::BundleInvalid))?;
        let read = reader
            .read(&mut buffer[..maximum])
            .map_err(|_| error(TemplateErrorCode::Io))?;
        if read == 0 {
            return Err(error(TemplateErrorCode::BundleInvalid));
        }
        digest.update(&buffer[..read]);
        remaining -= read as u64;
    }
    let mut extra = [0_u8; 1];
    if reader
        .read(&mut extra)
        .map_err(|_| error(TemplateErrorCode::Io))?
        != 0
    {
        return Err(error(TemplateErrorCode::BundleInvalid));
    }
    Ok(())
}

fn parse_digest(value: &str) -> Result<[u8; 32], TemplateError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(error(TemplateErrorCode::ManifestInvalid));
    }
    let mut digest = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        digest[index] = (hex_nibble(pair[0])? << 4) | hex_nibble(pair[1])?;
    }
    Ok(digest)
}

fn hex_nibble(value: u8) -> Result<u8, TemplateError> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err(error(TemplateErrorCode::ManifestInvalid)),
    }
}

fn slash_path(path: &Path) -> Result<String, TemplateError> {
    path.to_str()
        .map(|value| value.replace('\\', "/"))
        .ok_or_else(|| error(TemplateErrorCode::BundleInvalid))
}

fn valid_token(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index != 0 && matches!(byte, b'.' | b'_' | b'-'))
        })
}

fn valid_text(value: &str, maximum: usize, empty_allowed: bool) -> bool {
    (empty_allowed || !value.is_empty())
        && value.len() <= maximum
        && !value.chars().any(char::is_control)
}

fn valid_package_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index != 0 && matches!(byte, b'.' | b'_' | b'-'))
        })
}

fn valid_relative_contract_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 1_024
        && !value.starts_with('/')
        && !value.contains('\\')
        && value
            .split('/')
            .all(|segment| !segment.is_empty() && !matches!(segment, "." | ".."))
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn map_archive_error(source: crate::archive::ArchiveError) -> TemplateError {
    match source.code() {
        ArchiveErrorCode::Io => error(TemplateErrorCode::Io),
        _ => error(TemplateErrorCode::BundleInvalid),
    }
}

const fn error(code: TemplateErrorCode) -> TemplateError {
    TemplateError { code }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use zip::CompressionMethod;
    use zip::write::SimpleFileOptions;

    static SEQUENCE: AtomicU64 = AtomicU64::new(1);

    #[test]
    fn template_limits_are_independent_and_parameterized() {
        assert_eq!(ArchiveLimits::template().archive_bytes, 2_147_483_648);
        let path = fixture_bundle(&manifest("00", 3, 3), &[]);
        let mut tiny = ArchiveLimits::template();
        tiny.archive_bytes = 1;
        assert_eq!(
            inspect_template_bundle_with_limits(&path, tiny)
                .expect_err("tiny archive quota")
                .code(),
            TemplateErrorCode::BundleInvalid
        );
        std::fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn valid_bundle_is_streamed_and_cross_checked() {
        let payload_digest = fixture_payload_digest();
        let path = fixture_bundle(&manifest(&payload_digest, 3, 5), &[]);
        let inspection = inspect_template_bundle(&path).expect("inspect Template");
        assert_eq!(inspection.manifest.template_version, "1");
        assert_eq!(
            inspection.payload_tree_sha256,
            parse_digest(&payload_digest).expect("fixture digest is valid")
        );
        std::fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn undeclared_resource_and_digest_mismatch_fail_closed() {
        let payload_digest = fixture_payload_digest();
        let path = fixture_bundle(
            &manifest(&payload_digest, 3, 5),
            &[("resources/extra.bin", b"extra")],
        );
        assert_eq!(
            inspect_template_bundle(&path)
                .expect_err("undeclared resource")
                .code(),
            TemplateErrorCode::ResourceInvalid
        );
        std::fs::remove_file(path).expect("remove fixture");
    }

    fn manifest(tree: &str, entries: u64, bytes: u64) -> String {
        format!(
            r#"{{"formatVersion":1,"templateId":"7e2233c8-0b3f-4cf2-aeb4-57d3d240b001","templateVersion":"1","displayName":"Blank","unity":{{"major":2022,"minor":3}},"dependencies":[],"additionalResources":[],"payload":{{"root":"payload/","treeSha256":"{tree}","entryCount":{entries},"totalBytes":{bytes}}},"provenance":{{"createdBy":"authored"}}}}"#
        )
    }

    fn fixture_payload_digest() -> String {
        let mut digest = Sha256::new();
        for (path, bytes) in fixture_payload() {
            digest.update((path.len() as u32).to_le_bytes());
            digest.update(path.as_bytes());
            digest.update((bytes.len() as u64).to_le_bytes());
            digest.update(bytes);
        }
        digest
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    fn fixture_payload() -> [(&'static str, &'static [u8]); 3] {
        [
            ("Packages/manifest.json", b"{}"),
            ("Packages/vpm-manifest.json", b"{}"),
            ("ProjectSettings/ProjectVersion.txt", b"x"),
        ]
    }

    fn fixture_bundle(manifest: &str, extras: &[(&str, &[u8])]) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "alcomd-template-parser-{}-{}.alcomdtemplate",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut writer = zip::ZipWriter::new(File::create(&path).expect("create fixture"));
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        writer
            .start_file("template.json", options)
            .expect("manifest");
        writer
            .write_all(manifest.as_bytes())
            .expect("manifest bytes");
        for (name, bytes) in fixture_payload()
            .into_iter()
            .map(|(path, bytes)| (format!("payload/{path}"), bytes))
            .chain(
                extras
                    .iter()
                    .map(|(path, bytes)| ((*path).to_owned(), *bytes)),
            )
        {
            writer.start_file(name, options).expect("payload entry");
            writer.write_all(bytes).expect("payload bytes");
        }
        writer.finish().expect("finish fixture");
        path
    }
}
