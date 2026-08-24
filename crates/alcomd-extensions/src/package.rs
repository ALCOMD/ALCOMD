use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use ed25519_dalek::{Signature, VerifyingKey};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
use zip::{CompressionMethod, ZipArchive};

const MANIFEST_PATH: &str = "alcomd-extension.toml";
const SIGNATURE_PATH: &str = "META-INF/alcomd-signature-v1.json";
const COMPONENT_PATH: &str = "component/extension.wasm";
const MAX_ARCHIVE_BYTES: u64 = 67_108_864;
const MAX_ENTRIES: usize = 1_024;
const MAX_ENTRY_BYTES: u64 = 33_554_432;
const MAX_TOTAL_BYTES: u64 = 134_217_728;
const MAX_MANIFEST_BYTES: u64 = 65_536;
const MAX_SIGNATURE_BYTES: u64 = 8_192;
const MAX_UI_BYTES: u64 = 67_108_864;
const MAX_PATH_DEPTH: usize = 16;
const MAX_PATH_BYTES: usize = 512;
const MAX_EXPANSION_RATIO: u64 = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PackageErrorCode {
    Io,
    InvalidArchive,
    ArchiveLimitExceeded,
    UnsafePath,
    PathCollision,
    UnsupportedEntry,
    ManifestInvalid,
    SignatureInvalid,
}

#[derive(Debug, thiserror::Error)]
#[error("extension package rejected: {code:?}")]
pub struct PackageError {
    code: PackageErrorCode,
}

impl PackageError {
    #[must_use]
    pub const fn code(&self) -> PackageErrorCode {
        self.code
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ExtensionManifest {
    pub schema: u32,
    pub id: String,
    pub name: String,
    pub version: String,
    pub api: u32,
    pub publisher_name: String,
    pub publisher_key_fingerprint: String,
    pub license: String,
    pub entrypoints: ManifestEntrypoints,
    pub interfaces: ManifestRequirements,
    pub permissions: ManifestRequirements,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ManifestEntrypoints {
    pub background_component: Option<String>,
    pub ui_entry: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ManifestRequirements {
    pub required: Vec<String>,
    pub optional: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedExtensionPackage {
    pub manifest: ExtensionManifest,
    pub package_digest: [u8; 32],
    pub manifest_digest: [u8; 32],
    pub component_digest: [u8; 32],
    pub publisher_fingerprint: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SignatureEnvelope {
    format_version: u32,
    algorithm: String,
    package_digest: String,
    public_key: String,
    publisher_fingerprint: String,
    signature: String,
}

#[derive(Clone, Debug)]
struct EntryPlan {
    index: usize,
    path: String,
    size: u64,
}

pub fn inspect_extension_package(path: &Path) -> Result<VerifiedExtensionPackage, PackageError> {
    let metadata = std::fs::metadata(path).map_err(|_| error(PackageErrorCode::Io))?;
    if !metadata.is_file() || metadata.len() == 0 || metadata.len() > MAX_ARCHIVE_BYTES {
        return Err(error(PackageErrorCode::ArchiveLimitExceeded));
    }
    let file = File::open(path).map_err(|_| error(PackageErrorCode::Io))?;
    let mut archive = ZipArchive::new(file).map_err(|_| error(PackageErrorCode::InvalidArchive))?;
    let plans = preflight(&mut archive)?;
    let manifest_bytes = read_named(&mut archive, &plans, MANIFEST_PATH, MAX_MANIFEST_BYTES)?;
    let signature_bytes = read_named(&mut archive, &plans, SIGNATURE_PATH, MAX_SIGNATURE_BYTES)?;
    let manifest = parse_manifest(&manifest_bytes)?;
    let package_digest = canonical_digest(&mut archive, &plans)?;
    let manifest_digest = Sha256::digest(&manifest_bytes).into();
    let component_bytes = read_named(&mut archive, &plans, COMPONENT_PATH, MAX_ENTRY_BYTES)?;
    let component_digest = Sha256::digest(&component_bytes).into();
    let envelope: SignatureEnvelope = serde_json::from_slice(&signature_bytes)
        .map_err(|_| error(PackageErrorCode::SignatureInvalid))?;
    verify_signature(&manifest, package_digest, &envelope)?;
    Ok(VerifiedExtensionPackage {
        manifest,
        package_digest,
        manifest_digest,
        component_digest,
        publisher_fingerprint: envelope.publisher_fingerprint,
    })
}

pub fn extract_extension_package(
    source: &Path,
    destination: &Path,
) -> Result<VerifiedExtensionPackage, PackageError> {
    let verified = inspect_extension_package(source)?;
    if destination.exists() {
        return Err(error(PackageErrorCode::UnsafePath));
    }
    std::fs::create_dir(destination).map_err(|_| error(PackageErrorCode::Io))?;
    let result = extract_into(source, destination);
    if result.is_err() {
        let _ = std::fs::remove_dir_all(destination);
    }
    result.map(|()| verified)
}

pub fn inspect_extension_directory(root: &Path) -> Result<VerifiedExtensionPackage, PackageError> {
    let mut files = BTreeMap::new();
    collect_directory(root, root, 0, &mut files)?;
    if files.len() > MAX_ENTRIES {
        return Err(error(PackageErrorCode::ArchiveLimitExceeded));
    }
    let manifest_bytes = read_bounded_file(
        files
            .get(MANIFEST_PATH)
            .ok_or_else(|| error(PackageErrorCode::InvalidArchive))?,
        MAX_MANIFEST_BYTES,
    )?;
    let signature_bytes = read_bounded_file(
        files
            .get(SIGNATURE_PATH)
            .ok_or_else(|| error(PackageErrorCode::InvalidArchive))?,
        MAX_SIGNATURE_BYTES,
    )?;
    let component_bytes = read_bounded_file(
        files
            .get(COMPONENT_PATH)
            .ok_or_else(|| error(PackageErrorCode::InvalidArchive))?,
        MAX_ENTRY_BYTES,
    )?;
    let manifest = parse_manifest(&manifest_bytes)?;
    let package_digest = directory_digest(&files)?;
    let envelope: SignatureEnvelope = serde_json::from_slice(&signature_bytes)
        .map_err(|_| error(PackageErrorCode::SignatureInvalid))?;
    verify_signature(&manifest, package_digest, &envelope)?;
    Ok(VerifiedExtensionPackage {
        manifest,
        package_digest,
        manifest_digest: Sha256::digest(&manifest_bytes).into(),
        component_digest: Sha256::digest(&component_bytes).into(),
        publisher_fingerprint: envelope.publisher_fingerprint,
    })
}

fn collect_directory(
    root: &Path,
    directory: &Path,
    depth: usize,
    files: &mut BTreeMap<String, PathBuf>,
) -> Result<(), PackageError> {
    if depth > MAX_PATH_DEPTH {
        return Err(error(PackageErrorCode::ArchiveLimitExceeded));
    }
    let entries = std::fs::read_dir(directory).map_err(|_| error(PackageErrorCode::Io))?;
    for entry in entries {
        let entry = entry.map_err(|_| error(PackageErrorCode::Io))?;
        let metadata =
            std::fs::symlink_metadata(entry.path()).map_err(|_| error(PackageErrorCode::Io))?;
        if metadata.file_type().is_symlink() {
            return Err(error(PackageErrorCode::UnsupportedEntry));
        }
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|_| error(PackageErrorCode::UnsafePath))?
            .components()
            .map(|component| {
                component
                    .as_os_str()
                    .to_str()
                    .ok_or_else(|| error(PackageErrorCode::UnsafePath))
            })
            .collect::<Result<Vec<_>, _>>()?
            .join("/");
        let (normalized, collision) = normalize_path(&relative)?;
        if normalized != relative || files.keys().any(|path| collision_key(path) == collision) {
            return Err(error(PackageErrorCode::PathCollision));
        }
        validate_allowed_root(&normalized, metadata.is_dir())?;
        if metadata.is_dir() {
            collect_directory(root, &entry.path(), depth + 1, files)?;
        } else if metadata.is_file() {
            if metadata.len() > MAX_ENTRY_BYTES || files.len() >= MAX_ENTRIES {
                return Err(error(PackageErrorCode::ArchiveLimitExceeded));
            }
            files.insert(normalized, entry.path());
        } else {
            return Err(error(PackageErrorCode::UnsupportedEntry));
        }
    }
    Ok(())
}

fn collision_key(path: &str) -> String {
    path.chars().flat_map(char::to_lowercase).collect()
}

fn read_bounded_file(path: &Path, maximum: u64) -> Result<Vec<u8>, PackageError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| error(PackageErrorCode::Io))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > maximum {
        return Err(error(PackageErrorCode::ArchiveLimitExceeded));
    }
    std::fs::read(path).map_err(|_| error(PackageErrorCode::Io))
}

fn directory_digest(files: &BTreeMap<String, PathBuf>) -> Result<[u8; 32], PackageError> {
    let mut digest = Sha256::new();
    digest.update(b"ALCOMD-EXT-CONTENT-SHA256-V1\0");
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    for (path, source) in files {
        if path == SIGNATURE_PATH {
            continue;
        }
        let size = std::fs::metadata(source)
            .map_err(|_| error(PackageErrorCode::Io))?
            .len();
        total = total
            .checked_add(size)
            .filter(|value| *value <= MAX_TOTAL_BYTES)
            .ok_or_else(|| error(PackageErrorCode::ArchiveLimitExceeded))?;
        digest.update(
            u32::try_from(path.len())
                .map_err(|_| error(PackageErrorCode::ArchiveLimitExceeded))?
                .to_le_bytes(),
        );
        digest.update(path.as_bytes());
        digest.update(size.to_le_bytes());
        let mut file = File::open(source).map_err(|_| error(PackageErrorCode::Io))?;
        let mut read = 0_u64;
        loop {
            let count = file
                .read(&mut buffer)
                .map_err(|_| error(PackageErrorCode::Io))?;
            if count == 0 {
                break;
            }
            read = read
                .checked_add(u64::try_from(count).unwrap_or(u64::MAX))
                .filter(|value| *value <= size)
                .ok_or_else(|| error(PackageErrorCode::ArchiveLimitExceeded))?;
            digest.update(&buffer[..count]);
        }
        if read != size {
            return Err(error(PackageErrorCode::InvalidArchive));
        }
    }
    Ok(digest.finalize().into())
}

fn extract_into(source: &Path, destination: &Path) -> Result<(), PackageError> {
    let file = File::open(source).map_err(|_| error(PackageErrorCode::Io))?;
    let mut archive = ZipArchive::new(file).map_err(|_| error(PackageErrorCode::InvalidArchive))?;
    let plans = preflight(&mut archive)?;
    let mut buffer = [0_u8; 64 * 1024];
    for plan in plans {
        let target = safe_target(destination, &plan.path)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|_| error(PackageErrorCode::Io))?;
        }
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|_| error(PackageErrorCode::Io))?;
        let mut entry = archive
            .by_index(plan.index)
            .map_err(|_| error(PackageErrorCode::InvalidArchive))?;
        let mut written = 0_u64;
        loop {
            let count = entry
                .read(&mut buffer)
                .map_err(|_| error(PackageErrorCode::Io))?;
            if count == 0 {
                break;
            }
            written = written
                .checked_add(u64::try_from(count).unwrap_or(u64::MAX))
                .filter(|value| *value <= plan.size)
                .ok_or_else(|| error(PackageErrorCode::ArchiveLimitExceeded))?;
            output
                .write_all(&buffer[..count])
                .map_err(|_| error(PackageErrorCode::Io))?;
        }
        if written != plan.size {
            return Err(error(PackageErrorCode::InvalidArchive));
        }
        output.sync_all().map_err(|_| error(PackageErrorCode::Io))?;
    }
    alcomd_platform::sync_directory(destination).map_err(|_| error(PackageErrorCode::Io))
}

fn safe_target(root: &Path, relative: &str) -> Result<PathBuf, PackageError> {
    let mut target = root.to_path_buf();
    for segment in relative.split('/') {
        target.push(segment);
    }
    if !target.starts_with(root) {
        return Err(error(PackageErrorCode::UnsafePath));
    }
    Ok(target)
}

fn preflight<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<Vec<EntryPlan>, PackageError> {
    if archive.is_empty() || archive.len() > MAX_ENTRIES {
        return Err(error(PackageErrorCode::ArchiveLimitExceeded));
    }
    let mut plans = Vec::with_capacity(archive.len());
    let mut paths = BTreeMap::<String, bool>::new();
    let mut total = 0_u64;
    let mut ui_total = 0_u64;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|_| error(PackageErrorCode::InvalidArchive))?;
        if entry.encrypted()
            || !matches!(
                entry.compression(),
                CompressionMethod::Stored | CompressionMethod::Deflated
            )
        {
            return Err(error(PackageErrorCode::UnsupportedEntry));
        }
        if let Some(mode) = entry.unix_mode() {
            let kind = mode & 0o170000;
            if !matches!(kind, 0 | 0o040000 | 0o100000) {
                return Err(error(PackageErrorCode::UnsupportedEntry));
            }
        }
        let raw = std::str::from_utf8(entry.name_raw())
            .map_err(|_| error(PackageErrorCode::UnsafePath))?;
        if raw != entry.name() {
            return Err(error(PackageErrorCode::UnsafePath));
        }
        let (path, collision) = normalize_path(raw)?;
        let directory = entry.is_dir();
        validate_collision(&path, &collision, directory, &paths)?;
        if paths.insert(collision, directory).is_some() {
            return Err(error(PackageErrorCode::PathCollision));
        }
        validate_allowed_root(&path, directory)?;
        let size = entry.size();
        let compressed = entry.compressed_size();
        if size > MAX_ENTRY_BYTES
            || (size > 0
                && (compressed == 0 || size > compressed.saturating_mul(MAX_EXPANSION_RATIO)))
        {
            return Err(error(PackageErrorCode::ArchiveLimitExceeded));
        }
        total = total
            .checked_add(size)
            .filter(|value| *value <= MAX_TOTAL_BYTES)
            .ok_or_else(|| error(PackageErrorCode::ArchiveLimitExceeded))?;
        if path.starts_with("ui/") {
            ui_total = ui_total
                .checked_add(size)
                .filter(|value| *value <= MAX_UI_BYTES)
                .ok_or_else(|| error(PackageErrorCode::ArchiveLimitExceeded))?;
        }
        if !directory {
            plans.push(EntryPlan { index, path, size });
        }
    }
    for required in [MANIFEST_PATH, SIGNATURE_PATH, COMPONENT_PATH] {
        if plans.iter().filter(|entry| entry.path == required).count() != 1 {
            return Err(error(PackageErrorCode::InvalidArchive));
        }
    }
    Ok(plans)
}

fn normalize_path(raw: &str) -> Result<(String, String), PackageError> {
    if raw.is_empty()
        || raw.starts_with('/')
        || raw.starts_with('\\')
        || raw.contains('\\')
        || raw.bytes().any(|byte| byte == 0 || byte.is_ascii_control())
    {
        return Err(error(PackageErrorCode::UnsafePath));
    }
    let trimmed = raw.strip_suffix('/').unwrap_or(raw);
    let segments = trimmed.split('/').collect::<Vec<_>>();
    if trimmed.is_empty()
        || segments.len() > MAX_PATH_DEPTH
        || segments.iter().any(|segment| {
            segment.is_empty()
                || matches!(*segment, "." | "..")
                || segment.contains(':')
                || segment.ends_with('.')
                || segment.ends_with(' ')
        })
    {
        return Err(error(PackageErrorCode::UnsafePath));
    }
    let normalized = segments
        .iter()
        .map(|segment| segment.nfc().collect::<String>())
        .collect::<Vec<_>>()
        .join("/");
    if normalized.len() > MAX_PATH_BYTES || is_windows_device_name(&segments) {
        return Err(error(PackageErrorCode::UnsafePath));
    }
    let collision = normalized
        .chars()
        .flat_map(char::to_lowercase)
        .collect::<String>();
    Ok((normalized, collision))
}

fn is_windows_device_name(segments: &[&str]) -> bool {
    segments.iter().any(|segment| {
        let stem = segment
            .split('.')
            .next()
            .unwrap_or(segment)
            .to_ascii_uppercase();
        matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || stem
                .strip_prefix("COM")
                .or_else(|| stem.strip_prefix("LPT"))
                .is_some_and(|suffix| {
                    suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9')
                })
    })
}

fn validate_allowed_root(path: &str, directory: bool) -> Result<(), PackageError> {
    if (directory && matches!(path, "META-INF" | "component" | "ui"))
        || path == MANIFEST_PATH
        || path == SIGNATURE_PATH
        || path.starts_with("META-INF/")
        || path.starts_with("component/")
        || path.starts_with("ui/")
    {
        Ok(())
    } else {
        Err(error(PackageErrorCode::UnsafePath))
    }
}

fn validate_collision(
    path: &str,
    collision: &str,
    directory: bool,
    paths: &BTreeMap<String, bool>,
) -> Result<(), PackageError> {
    if paths.contains_key(collision) {
        return Err(error(PackageErrorCode::PathCollision));
    }
    let mut prefix = String::new();
    let segments = collision.split('/').collect::<Vec<_>>();
    for segment in segments.iter().take(segments.len().saturating_sub(1)) {
        if !prefix.is_empty() {
            prefix.push('/');
        }
        prefix.push_str(segment);
        if paths.get(&prefix) == Some(&false) {
            return Err(error(PackageErrorCode::PathCollision));
        }
    }
    if !directory {
        let child_prefix = format!("{collision}/");
        if paths
            .keys()
            .any(|existing| existing.starts_with(&child_prefix))
        {
            return Err(error(PackageErrorCode::PathCollision));
        }
    }
    let _ = path;
    Ok(())
}

fn read_named<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    plans: &[EntryPlan],
    name: &str,
    maximum: u64,
) -> Result<Vec<u8>, PackageError> {
    let plan = plans
        .iter()
        .find(|entry| entry.path == name)
        .ok_or_else(|| error(PackageErrorCode::InvalidArchive))?;
    if plan.size > maximum {
        return Err(error(PackageErrorCode::ArchiveLimitExceeded));
    }
    let entry = archive
        .by_index(plan.index)
        .map_err(|_| error(PackageErrorCode::InvalidArchive))?;
    let mut bytes = Vec::with_capacity(
        usize::try_from(plan.size).map_err(|_| error(PackageErrorCode::ArchiveLimitExceeded))?,
    );
    entry
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| error(PackageErrorCode::Io))?;
    if u64::try_from(bytes.len()).ok() != Some(plan.size) {
        return Err(error(PackageErrorCode::InvalidArchive));
    }
    Ok(bytes)
}

fn canonical_digest<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    plans: &[EntryPlan],
) -> Result<[u8; 32], PackageError> {
    let mut sorted = plans
        .iter()
        .filter(|entry| entry.path != SIGNATURE_PATH)
        .collect::<Vec<_>>();
    sorted.sort_by(|left, right| left.path.as_bytes().cmp(right.path.as_bytes()));
    let mut digest = Sha256::new();
    digest.update(b"ALCOMD-EXT-CONTENT-SHA256-V1\0");
    let mut buffer = [0_u8; 64 * 1024];
    for plan in sorted {
        digest.update(
            u32::try_from(plan.path.len())
                .map_err(|_| error(PackageErrorCode::ArchiveLimitExceeded))?
                .to_le_bytes(),
        );
        digest.update(plan.path.as_bytes());
        digest.update(plan.size.to_le_bytes());
        let mut entry = archive
            .by_index(plan.index)
            .map_err(|_| error(PackageErrorCode::InvalidArchive))?;
        let mut read = 0_u64;
        loop {
            let count = entry
                .read(&mut buffer)
                .map_err(|_| error(PackageErrorCode::Io))?;
            if count == 0 {
                break;
            }
            read = read
                .checked_add(u64::try_from(count).unwrap_or(u64::MAX))
                .ok_or_else(|| error(PackageErrorCode::ArchiveLimitExceeded))?;
            if read > plan.size {
                return Err(error(PackageErrorCode::InvalidArchive));
            }
            digest.update(&buffer[..count]);
        }
        if read != plan.size {
            return Err(error(PackageErrorCode::InvalidArchive));
        }
    }
    Ok(digest.finalize().into())
}

fn parse_manifest(bytes: &[u8]) -> Result<ExtensionManifest, PackageError> {
    let text = std::str::from_utf8(bytes).map_err(|_| error(PackageErrorCode::ManifestInvalid))?;
    if text.starts_with('\u{feff}') || text.bytes().any(|byte| byte == 0) {
        return Err(error(PackageErrorCode::ManifestInvalid));
    }
    let manifest: ExtensionManifest =
        toml::from_str(text).map_err(|_| error(PackageErrorCode::ManifestInvalid))?;
    if manifest.schema != 1
        || manifest.api != 1
        || !valid_extension_id(&manifest.id)
        || Version::parse(&manifest.version).is_err()
        || manifest.name.is_empty()
        || manifest.name.len() > 120
        || manifest.publisher_name.is_empty()
        || manifest.publisher_name.len() > 120
        || manifest.license.is_empty()
        || manifest.license.len() > 128
        || manifest.entrypoints.background_component.as_deref() != Some(COMPONENT_PATH)
        || !valid_fingerprint(&manifest.publisher_key_fingerprint)
        || !valid_requirements(&manifest.interfaces, 32, valid_interface)
        || !valid_requirements(&manifest.permissions, 64, valid_permission)
    {
        return Err(error(PackageErrorCode::ManifestInvalid));
    }
    if let Some(ui) = &manifest.entrypoints.ui_entry
        && (!ui.starts_with("ui/") || ui.len() > MAX_PATH_BYTES)
    {
        return Err(error(PackageErrorCode::ManifestInvalid));
    }
    Ok(manifest)
}

fn valid_extension_id(value: &str) -> bool {
    value.len() <= 255
        && value.split('.').count() >= 2
        && value.split('.').all(|part| {
            !part.is_empty()
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

fn valid_requirements(
    values: &ManifestRequirements,
    maximum: usize,
    validate: fn(&str) -> bool,
) -> bool {
    for list in [&values.required, &values.optional] {
        if list.len() > maximum
            || !list.windows(2).all(|pair| pair[0] < pair[1])
            || !list.iter().all(|value| validate(value))
        {
            return false;
        }
    }
    values
        .required
        .iter()
        .all(|value| !values.optional.contains(value))
}

fn valid_interface(value: &str) -> bool {
    value.len() <= 128
        && value.contains(':')
        && value.contains('/')
        && value.rsplit_once('@').is_some_and(|(_, version)| {
            Version::parse(version).is_ok() && version.split('.').count() == 3
        })
}

fn valid_permission(value: &str) -> bool {
    (3..=128).contains(&value.len())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
}

fn verify_signature(
    manifest: &ExtensionManifest,
    package_digest: [u8; 32],
    envelope: &SignatureEnvelope,
) -> Result<(), PackageError> {
    if envelope.format_version != 1
        || envelope.algorithm != "ed25519"
        || envelope.package_digest != hex(&package_digest)
        || envelope.publisher_fingerprint != manifest.publisher_key_fingerprint
        || !valid_fingerprint(&envelope.publisher_fingerprint)
    {
        return Err(error(PackageErrorCode::SignatureInvalid));
    }
    let public_key: [u8; 32] = decode_hex(&envelope.public_key)?;
    let signature: [u8; 64] = decode_hex(&envelope.signature)?;
    let expected_fingerprint = format!("ed25519-sha256:{}", hex(&Sha256::digest(public_key)));
    if envelope.publisher_fingerprint != expected_fingerprint {
        return Err(error(PackageErrorCode::SignatureInvalid));
    }
    let key = VerifyingKey::from_bytes(&public_key)
        .map_err(|_| error(PackageErrorCode::SignatureInvalid))?;
    let signature = Signature::from_bytes(&signature);
    let mut message = Vec::with_capacity(29 + package_digest.len());
    message.extend_from_slice(b"ALCOMD-EXT-SIGNATURE-V1\0");
    message.extend_from_slice(&package_digest);
    key.verify_strict(&message, &signature)
        .map_err(|_| error(PackageErrorCode::SignatureInvalid))
}

fn valid_fingerprint(value: &str) -> bool {
    value.len() == 79
        && value.strip_prefix("ed25519-sha256:").is_some_and(|hex| {
            hex.bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        })
}

fn decode_hex<const N: usize>(value: &str) -> Result<[u8; N], PackageError> {
    if value.len() != N * 2 {
        return Err(error(PackageErrorCode::SignatureInvalid));
    }
    let mut output = [0_u8; N];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let pair =
            std::str::from_utf8(pair).map_err(|_| error(PackageErrorCode::SignatureInvalid))?;
        output[index] =
            u8::from_str_radix(pair, 16).map_err(|_| error(PackageErrorCode::SignatureInvalid))?;
    }
    Ok(output)
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[(byte >> 4) as usize]));
        output.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    output
}

const fn error(code: PackageErrorCode) -> PackageError {
    PackageError { code }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use zip::write::SimpleFileOptions;

    static NEXT_PATH: AtomicU64 = AtomicU64::new(0);

    fn temporary_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "alcomd-m6-extension-{name}-{}-{}",
            std::process::id(),
            NEXT_PATH.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn canonical_for(entries: &[(&str, &[u8])]) -> [u8; 32] {
        let mut entries = entries.to_vec();
        entries.sort_by(|left, right| left.0.as_bytes().cmp(right.0.as_bytes()));
        let mut digest = Sha256::new();
        digest.update(b"ALCOMD-EXT-CONTENT-SHA256-V1\0");
        for (path, content) in entries {
            digest.update(
                u32::try_from(path.len())
                    .expect("path length")
                    .to_le_bytes(),
            );
            digest.update(path.as_bytes());
            digest.update(
                u64::try_from(content.len())
                    .expect("content length")
                    .to_le_bytes(),
            );
            digest.update(content);
        }
        digest.finalize().into()
    }

    fn signed_package(extra: &[(&str, &[u8])], fingerprint_override: Option<&str>) -> PathBuf {
        let signing = SigningKey::from_bytes(&[7_u8; 32]);
        let public_key = signing.verifying_key().to_bytes();
        let fingerprint = format!("ed25519-sha256:{}", hex(&Sha256::digest(public_key)));
        let manifest = format!(
            "schema = 1\nid = \"dev.example.fixture\"\nname = \"Fixture\"\nversion = \"1.0.0\"\napi = 1\npublisher_name = \"Fixture Publisher\"\npublisher_key_fingerprint = \"{}\"\nlicense = \"MIT\"\n\n[entrypoints]\nbackground_component = \"component/extension.wasm\"\n\n[interfaces]\nrequired = [\"alcomd:extension/host-data@1.0.0\", \"alcomd:extension/host-projects@1.0.0\"]\noptional = []\n\n[permissions]\nrequired = [\"background.run\", \"projects.read\"]\noptional = []\n",
            fingerprint_override.unwrap_or(&fingerprint)
        );
        let component = b"\0asm\x0d\0\x01\0";
        let mut content_entries = vec![
            (MANIFEST_PATH, manifest.as_bytes()),
            (COMPONENT_PATH, component.as_slice()),
        ];
        content_entries.extend_from_slice(extra);
        let package_digest = canonical_for(&content_entries);
        let mut message = b"ALCOMD-EXT-SIGNATURE-V1\0".to_vec();
        message.extend_from_slice(&package_digest);
        let signature = signing.sign(&message).to_bytes();
        let envelope = serde_json::json!({
            "formatVersion": 1,
            "algorithm": "ed25519",
            "packageDigest": hex(&package_digest),
            "publicKey": hex(&public_key),
            "publisherFingerprint": fingerprint,
            "signature": hex(&signature),
        });
        let path = temporary_path("package.alcomdext");
        let file = File::create(&path).expect("create package");
        let mut writer = zip::ZipWriter::new(file);
        for (name, content) in content_entries {
            writer
                .start_file(
                    name,
                    SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
                )
                .expect("start content entry");
            writer.write_all(content).expect("write content entry");
        }
        writer
            .start_file(
                SIGNATURE_PATH,
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .expect("start signature");
        writer
            .write_all(envelope.to_string().as_bytes())
            .expect("write signature");
        writer.finish().expect("finish package");
        path
    }

    #[test]
    fn signed_package_is_bounded_parsed_and_strictly_verified() {
        let path = signed_package(&[("ui/index.html", b"fixture")], None);
        let package = inspect_extension_package(&path).expect("verify package");
        assert_eq!(package.manifest.id, "dev.example.fixture");
        assert_eq!(package.manifest.version, "1.0.0");
        assert_eq!(package.manifest.api, 1);
        assert_eq!(package.publisher_fingerprint.len(), 79);
        std::fs::remove_file(path).expect("remove package");
    }

    #[test]
    fn manifest_publisher_mismatch_and_hostile_paths_fail_closed() {
        let mismatch = signed_package(
            &[],
            Some("ed25519-sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
        );
        assert_eq!(
            inspect_extension_package(&mismatch)
                .expect_err("publisher mismatch")
                .code(),
            PackageErrorCode::SignatureInvalid
        );
        std::fs::remove_file(mismatch).expect("remove mismatch");

        let signing = SigningKey::from_bytes(&[7_u8; 32]);
        let fingerprint = format!(
            "ed25519-sha256:{}",
            hex(&Sha256::digest(signing.verifying_key().to_bytes()))
        );
        let path = signed_package(&[("ui/../escape", b"bad")], Some(fingerprint.as_str()));
        assert_eq!(
            inspect_extension_package(&path)
                .expect_err("hostile path")
                .code(),
            PackageErrorCode::UnsafePath
        );
        std::fs::remove_file(path).expect("remove hostile package");
    }

    #[test]
    fn case_collision_and_link_entries_fail_closed() {
        let collision = signed_package(
            &[("ui/Panel.html", b"first"), ("ui/panel.html", b"second")],
            None,
        );
        assert_eq!(
            inspect_extension_package(&collision)
                .expect_err("case collision")
                .code(),
            PackageErrorCode::PathCollision
        );
        std::fs::remove_file(collision).expect("remove collision package");

        let link = temporary_path("link.alcomdext");
        let file = File::create(&link).expect("create link package");
        let mut writer = zip::ZipWriter::new(file);
        for (name, content) in [
            (MANIFEST_PATH, b"invalid".as_slice()),
            (COMPONENT_PATH, b"component".as_slice()),
            (SIGNATURE_PATH, b"{}".as_slice()),
        ] {
            writer
                .start_file(name, SimpleFileOptions::default())
                .expect("required entry");
            writer.write_all(content).expect("required content");
        }
        writer
            .add_symlink("ui/link", "target", SimpleFileOptions::default())
            .expect("link entry");
        writer.finish().expect("finish link package");
        assert_eq!(
            inspect_extension_package(&link)
                .expect_err("link entry")
                .code(),
            PackageErrorCode::UnsupportedEntry
        );
        std::fs::remove_file(link).expect("remove link package");
    }
}
