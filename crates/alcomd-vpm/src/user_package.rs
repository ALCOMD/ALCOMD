use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use alcomd_application::{
    UserPackageAdapter, UserPackageError, UserPackageErrorCode, UserPackageSnapshot,
};
use semver::Version;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

use crate::archive::{ArchiveLimits, MAX_ARCHIVE_BYTES, normalize_path, validate_collision};
use crate::range::VpmRange;
use crate::{PackageCache, preflight_archive};

const MANIFEST_LIMIT: u64 = 1_048_576;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub struct UserPackageEngine {
    cache: PackageCache,
    staging_root: PathBuf,
}

impl UserPackageEngine {
    pub fn new(cache_root: PathBuf, staging_root: PathBuf) -> Result<Self, UserPackageError> {
        let cache = PackageCache::new(cache_root).map_err(map_cache)?;
        Ok(Self {
            cache,
            staging_root,
        })
    }
}

impl UserPackageAdapter for UserPackageEngine {
    async fn snapshot(&self, source_path: String) -> Result<UserPackageSnapshot, UserPackageError> {
        let staging_root = self.staging_root.clone();
        let prepared = tokio::task::spawn_blocking(move || {
            prepare_snapshot(Path::new(&source_path), &staging_root)
        })
        .await
        .map_err(|_| internal())??;
        self.cache
            .publish_owned(&prepared.archive_path, prepared.snapshot.archive_sha256)
            .await
            .map_err(map_cache)?;
        Ok(prepared.snapshot.clone())
    }
}

#[derive(Debug)]
struct PreparedSnapshot {
    snapshot: UserPackageSnapshot,
    archive_path: PathBuf,
}

impl Drop for PreparedSnapshot {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.archive_path);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EntryKind {
    Directory,
    File,
}

#[derive(Clone, Debug)]
struct SourceEntry {
    relative: String,
    path: PathBuf,
    kind: EntryKind,
    size: u64,
    identity: Vec<u8>,
    executable: bool,
    digest: [u8; 32],
}

fn prepare_snapshot(
    source: &Path,
    staging_root: &Path,
) -> Result<PreparedSnapshot, UserPackageError> {
    validate_root(source)?;
    let (root, source_identity_key) = alcomd_platform::resolve_directory_identity(source)
        .map_err(|_| error(UserPackageErrorCode::SourceUnavailable))?;
    prepare_staging_root(staging_root)?;
    let entries = inventory(&root)?;
    let manifest_entry = entries
        .iter()
        .find(|entry| entry.kind == EntryKind::File && entry.relative == "package.json")
        .ok_or_else(|| error(UserPackageErrorCode::ManifestInvalid))?;
    if manifest_entry.size > MANIFEST_LIMIT {
        return Err(error(UserPackageErrorCode::ManifestInvalid));
    }
    let manifest_bytes = read_exact_verified(manifest_entry)?;
    let ParsedManifest {
        package_id,
        version,
        display_name,
        manifest_json,
        dependencies_json,
    } = parse_manifest(&manifest_bytes)?;
    let manifest_fingerprint = Sha256::digest(manifest_json.as_bytes()).into();
    let content_fingerprint = fingerprint(&entries);
    let archive_path = staging_root.join(format!(
        "user-package-{}-{}.zip.part",
        std::process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    let archive_sha256 = write_archive(&archive_path, &entries)?;
    preflight_archive(&archive_path).map_err(|_| error(UserPackageErrorCode::SourceUnsafe))?;
    let (final_root, final_identity) = alcomd_platform::resolve_directory_identity(&root)
        .map_err(|_| error(UserPackageErrorCode::SourceChanged))?;
    if final_root != root || final_identity != source_identity_key {
        return Err(error(UserPackageErrorCode::SourceChanged));
    }
    Ok(PreparedSnapshot {
        snapshot: UserPackageSnapshot {
            source_root_path: root
                .to_str()
                .ok_or_else(|| error(UserPackageErrorCode::SourceUnsafe))?
                .to_owned(),
            source_identity_key,
            package_id,
            version,
            display_name,
            manifest_json,
            dependencies_json,
            manifest_fingerprint,
            content_fingerprint,
            archive_sha256,
        },
        archive_path,
    })
}

fn validate_root(path: &Path) -> Result<(), UserPackageError> {
    if !path.is_absolute() || path.to_str().is_none() {
        return Err(error(UserPackageErrorCode::SourceUnsafe));
    }
    #[cfg(windows)]
    if !matches!(
        path.components().next(),
        Some(std::path::Component::Prefix(prefix))
            if matches!(
                prefix.kind(),
                std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)
            )
    ) {
        return Err(error(UserPackageErrorCode::SourceUnsafe));
    }
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| error(UserPackageErrorCode::SourceUnavailable))?;
    if !metadata.is_dir() || is_link_or_reparse(&metadata) {
        return Err(error(UserPackageErrorCode::SourceUnsafe));
    }
    Ok(())
}

fn prepare_staging_root(path: &Path) -> Result<(), UserPackageError> {
    if !path.is_absolute() {
        return Err(internal());
    }
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !is_link_or_reparse(&metadata) => Ok(()),
        Ok(_) => Err(internal()),
        Err(error_value) if error_value.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir(path).map_err(|_| internal())?;
            let metadata = std::fs::symlink_metadata(path).map_err(|_| internal())?;
            if metadata.is_dir() && !is_link_or_reparse(&metadata) {
                Ok(())
            } else {
                Err(internal())
            }
        }
        Err(_) => Err(internal()),
    }
}

fn inventory(root: &Path) -> Result<Vec<SourceEntry>, UserPackageError> {
    inventory_with_limits(root, ArchiveLimits::package())
}

fn inventory_with_limits(
    root: &Path,
    limits: ArchiveLimits,
) -> Result<Vec<SourceEntry>, UserPackageError> {
    let mut entries = Vec::new();
    let mut normalized_paths = BTreeMap::<String, bool>::new();
    let mut explicit_paths = BTreeSet::<PathBuf>::new();
    walk(
        root,
        root,
        limits,
        &mut normalized_paths,
        &mut explicit_paths,
        &mut entries,
    )?;
    entries.sort_by(|left, right| left.relative.as_bytes().cmp(right.relative.as_bytes()));
    Ok(entries)
}

fn walk(
    root: &Path,
    directory: &Path,
    limits: ArchiveLimits,
    normalized_paths: &mut BTreeMap<String, bool>,
    explicit_paths: &mut BTreeSet<PathBuf>,
    entries: &mut Vec<SourceEntry>,
) -> Result<(), UserPackageError> {
    let mut children = std::fs::read_dir(directory)
        .map_err(|_| error(UserPackageErrorCode::SourceUnavailable))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| error(UserPackageErrorCode::SourceUnavailable))?;
    children.sort_by_key(std::fs::DirEntry::file_name);
    for child in children {
        if entries.len() >= limits.entries {
            return Err(error(UserPackageErrorCode::LimitExceeded));
        }
        let path = child.path();
        let relative_os = path
            .strip_prefix(root)
            .map_err(|_| error(UserPackageErrorCode::SourceUnsafe))?;
        let relative_text = relative_os
            .to_str()
            .ok_or_else(|| error(UserPackageErrorCode::SourceUnsafe))?
            .replace('\\', "/");
        let (relative, collision_key) =
            normalize_path(&relative_text, limits).map_err(map_archive)?;
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|_| error(UserPackageErrorCode::SourceUnavailable))?;
        if is_link_or_reparse(&metadata) {
            return Err(error(UserPackageErrorCode::SourceUnsafe));
        }
        let kind = if metadata.is_dir() {
            EntryKind::Directory
        } else if metadata.is_file() {
            EntryKind::File
        } else {
            return Err(error(UserPackageErrorCode::SourceUnsafe));
        };
        validate_collision(
            &relative,
            &collision_key,
            kind == EntryKind::Directory,
            normalized_paths,
            explicit_paths,
        )
        .map_err(map_archive)?;
        if kind == EntryKind::File {
            verify_single_link(&path, &metadata)?;
            if metadata.len() > limits.entry_bytes {
                return Err(error(UserPackageErrorCode::LimitExceeded));
            }
        }
        let executable = is_executable(&metadata);
        let digest = if kind == EntryKind::File {
            hash_file(&path, metadata.len())?
        } else {
            [0_u8; 32]
        };
        entries.push(SourceEntry {
            relative: relative
                .to_str()
                .ok_or_else(|| error(UserPackageErrorCode::SourceUnsafe))?
                .replace('\\', "/"),
            path: path.clone(),
            kind,
            size: if kind == EntryKind::File {
                metadata.len()
            } else {
                0
            },
            identity: alcomd_platform::file_identity_key(&path)
                .map_err(|_| error(UserPackageErrorCode::SourceUnsafe))?,
            executable,
            digest,
        });
        if kind == EntryKind::Directory {
            walk(
                root,
                &path,
                limits,
                normalized_paths,
                explicit_paths,
                entries,
            )?;
        }
    }
    let total = entries
        .iter()
        .filter(|entry| entry.kind == EntryKind::File)
        .try_fold(0_u64, |total, entry| total.checked_add(entry.size))
        .filter(|total| *total <= limits.total_uncompressed_bytes)
        .ok_or_else(|| error(UserPackageErrorCode::LimitExceeded))?;
    let _ = total;
    Ok(())
}

struct ParsedManifest {
    package_id: String,
    version: String,
    display_name: Option<String>,
    manifest_json: String,
    dependencies_json: String,
}

fn parse_manifest(bytes: &[u8]) -> Result<ParsedManifest, UserPackageError> {
    let mut value: Value =
        serde_json::from_slice(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes))
            .map_err(|_| error(UserPackageErrorCode::ManifestInvalid))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| error(UserPackageErrorCode::ManifestInvalid))?;
    let package_id = required_string(object, "name", 128)?;
    validate_package_id(&package_id)?;
    let version_text = required_string(object, "version", 1_024)?;
    let version = Version::parse(&version_text)
        .map_err(|_| error(UserPackageErrorCode::ManifestInvalid))?
        .to_string();
    object.insert("version".to_owned(), Value::String(version.clone()));
    let display_name = optional_string(object, "displayName", 1_024)?;
    let _ = optional_string(object, "description", 65_536)?;
    let _ = optional_string(object, "unity", 1_024)?;
    for field in ["documentationUrl", "changelogUrl"] {
        match crate::sanitized_package_link(object.get(field)) {
            Some(url) => {
                object.insert(field.to_owned(), Value::String(url));
            }
            None => {
                object.remove(field);
            }
        }
    }
    let dependencies = match object.get("vpmDependencies") {
        None => Map::new(),
        Some(Value::Object(values)) => {
            if values.len() > 4_096 {
                return Err(error(UserPackageErrorCode::LimitExceeded));
            }
            for (dependency_id, range) in values {
                validate_package_id(dependency_id)?;
                let range = range
                    .as_str()
                    .ok_or_else(|| error(UserPackageErrorCode::ManifestInvalid))?;
                VpmRange::parse(range).map_err(|_| error(UserPackageErrorCode::ManifestInvalid))?;
            }
            values.clone()
        }
        Some(_) => return Err(error(UserPackageErrorCode::ManifestInvalid)),
    };
    object.insert(
        "vpmDependencies".to_owned(),
        Value::Object(dependencies.clone()),
    );
    let manifest_json = serde_json::to_string(&value).map_err(|_| internal())?;
    if manifest_json.len() > MANIFEST_LIMIT as usize {
        return Err(error(UserPackageErrorCode::ManifestInvalid));
    }
    let dependencies_json = serde_json::to_string(&dependencies).map_err(|_| internal())?;
    Ok(ParsedManifest {
        package_id,
        version,
        display_name,
        manifest_json,
        dependencies_json,
    })
}

fn required_string(
    object: &Map<String, Value>,
    field: &str,
    max: usize,
) -> Result<String, UserPackageError> {
    optional_string(object, field, max)?
        .filter(|value| !value.is_empty())
        .ok_or_else(|| error(UserPackageErrorCode::ManifestInvalid))
}

fn optional_string(
    object: &Map<String, Value>,
    field: &str,
    max: usize,
) -> Result<Option<String>, UserPackageError> {
    match object.get(field) {
        None => Ok(None),
        Some(Value::String(value)) if !value.is_empty() && value.len() <= max => {
            Ok(Some(value.clone()))
        }
        Some(_) => Err(error(UserPackageErrorCode::ManifestInvalid)),
    }
}

fn validate_package_id(value: &str) -> Result<(), UserPackageError> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    valid
        .then_some(())
        .ok_or_else(|| error(UserPackageErrorCode::ManifestInvalid))
}

fn fingerprint(entries: &[SourceEntry]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hash_field(&mut hasher, b"alcomd-user-package-tree-v1");
    for entry in entries {
        hash_field(&mut hasher, entry.relative.as_bytes());
        hash_field(
            &mut hasher,
            &[match entry.kind {
                EntryKind::Directory => 0,
                EntryKind::File => 1,
            }],
        );
        hash_field(&mut hasher, &entry.size.to_le_bytes());
        hash_field(&mut hasher, &entry.digest);
        hash_field(&mut hasher, &[u8::from(entry.executable)]);
    }
    hasher.finalize().into()
}

fn write_archive(path: &Path, entries: &[SourceEntry]) -> Result<[u8; 32], UserPackageError> {
    write_archive_with_limit(path, entries, MAX_ARCHIVE_BYTES)
}

fn write_archive_with_limit(
    path: &Path,
    entries: &[SourceEntry],
    archive_limit: u64,
) -> Result<[u8; 32], UserPackageError> {
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| internal())?;
    let mut archive = zip::ZipWriter::new(BoundedFile {
        file: output,
        limit: archive_limit,
    });
    let directory_options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .unix_permissions(0o755);
    for entry in entries {
        match entry.kind {
            EntryKind::Directory => {
                if archive
                    .add_directory(format!("{}/", entry.relative), directory_options)
                    .is_err()
                {
                    let _ = archive.finish();
                    return Err(error(UserPackageErrorCode::LimitExceeded));
                }
            }
            EntryKind::File => {
                let options = SimpleFileOptions::default()
                    .compression_method(CompressionMethod::Deflated)
                    .compression_level(Some(6))
                    .unix_permissions(if entry.executable { 0o755 } else { 0o644 });
                if archive.start_file(&entry.relative, options).is_err() {
                    let _ = archive.finish();
                    return Err(error(UserPackageErrorCode::LimitExceeded));
                }
                if let Err(error_value) = write_verified_file(entry, &mut archive) {
                    let _ = archive.abort_file();
                    let _ = archive.finish();
                    return Err(error_value);
                }
            }
        }
    }
    let mut output = archive
        .finish()
        .map_err(|_| error(UserPackageErrorCode::LimitExceeded))?;
    output.flush().map_err(|_| internal())?;
    output.file.sync_all().map_err(|_| internal())?;
    hash_file(path, output.file.metadata().map_err(|_| internal())?.len())
}

fn write_verified_file<W: Write + Seek>(
    entry: &SourceEntry,
    output: &mut zip::ZipWriter<W>,
) -> Result<(), UserPackageError> {
    let metadata = std::fs::symlink_metadata(&entry.path)
        .map_err(|_| error(UserPackageErrorCode::SourceChanged))?;
    if !metadata.is_file()
        || is_link_or_reparse(&metadata)
        || metadata.len() != entry.size
        || alcomd_platform::file_identity_key(&entry.path)
            .map_err(|_| error(UserPackageErrorCode::SourceChanged))?
            != entry.identity
    {
        return Err(error(UserPackageErrorCode::SourceChanged));
    }
    verify_single_link(&entry.path, &metadata)?;
    let mut input =
        File::open(&entry.path).map_err(|_| error(UserPackageErrorCode::SourceChanged))?;
    let mut hasher = Sha256::new();
    let mut remaining = entry.size;
    let mut buffer = [0_u8; 64 * 1024];
    while remaining > 0 {
        let maximum = remaining.min(buffer.len() as u64) as usize;
        let read = input
            .read(&mut buffer[..maximum])
            .map_err(|_| error(UserPackageErrorCode::SourceChanged))?;
        if read == 0 {
            return Err(error(UserPackageErrorCode::SourceChanged));
        }
        hasher.update(&buffer[..read]);
        output
            .write_all(&buffer[..read])
            .map_err(map_archive_write_error)?;
        remaining -= read as u64;
    }
    if input
        .read(&mut buffer[..1])
        .map_err(|_| error(UserPackageErrorCode::SourceChanged))?
        != 0
        || <[u8; 32]>::from(hasher.finalize()) != entry.digest
    {
        return Err(error(UserPackageErrorCode::SourceChanged));
    }
    Ok(())
}

fn read_exact_verified(entry: &SourceEntry) -> Result<Vec<u8>, UserPackageError> {
    let mut bytes = Vec::with_capacity(entry.size as usize);
    File::open(&entry.path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .map_err(|_| error(UserPackageErrorCode::SourceChanged))?;
    if bytes.len() as u64 != entry.size || <[u8; 32]>::from(Sha256::digest(&bytes)) != entry.digest
    {
        return Err(error(UserPackageErrorCode::SourceChanged));
    }
    Ok(bytes)
}

fn hash_file(path: &Path, expected: u64) -> Result<[u8; 32], UserPackageError> {
    let mut file = File::open(path).map_err(|_| error(UserPackageErrorCode::SourceUnavailable))?;
    let mut remaining = expected;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    while remaining > 0 {
        let maximum = remaining.min(buffer.len() as u64) as usize;
        let read = file
            .read(&mut buffer[..maximum])
            .map_err(|_| error(UserPackageErrorCode::SourceUnavailable))?;
        if read == 0 {
            return Err(error(UserPackageErrorCode::SourceChanged));
        }
        hasher.update(&buffer[..read]);
        remaining -= read as u64;
    }
    if file
        .read(&mut buffer[..1])
        .map_err(|_| error(UserPackageErrorCode::SourceChanged))?
        != 0
    {
        return Err(error(UserPackageErrorCode::SourceChanged));
    }
    Ok(hasher.finalize().into())
}

fn hash_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value);
}

struct BoundedFile {
    file: File,
    limit: u64,
}

impl Write for BoundedFile {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let requested = self
            .file
            .stream_position()?
            .checked_add(buffer.len() as u64)
            .ok_or_else(archive_limit_error)?;
        if requested > self.limit {
            return Err(archive_limit_error());
        }
        self.file.write(buffer)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

fn archive_limit_error() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::FileTooLarge,
        "User Package archive quota exceeded",
    )
}

fn map_archive_write_error(error_value: std::io::Error) -> UserPackageError {
    if error_value.kind() == std::io::ErrorKind::FileTooLarge {
        error(UserPackageErrorCode::LimitExceeded)
    } else {
        internal()
    }
}

impl Seek for BoundedFile {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.file.seek(position)
    }
}

#[cfg(unix)]
fn verify_single_link(_: &Path, metadata: &std::fs::Metadata) -> Result<(), UserPackageError> {
    use std::os::unix::fs::MetadataExt;
    if metadata.nlink() == 1 {
        Ok(())
    } else {
        Err(error(UserPackageErrorCode::SourceUnsafe))
    }
}

#[cfg(windows)]
fn verify_single_link(path: &Path, _: &std::fs::Metadata) -> Result<(), UserPackageError> {
    match alcomd_platform::file_link_count(path) {
        Ok(1) => Ok(()),
        Ok(_) | Err(_) => Err(error(UserPackageErrorCode::SourceUnsafe)),
    }
}

#[cfg(unix)]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(windows)]
fn is_executable(_: &std::fs::Metadata) -> bool {
    false
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

fn map_archive(error_value: crate::ArchiveError) -> UserPackageError {
    match error_value.code() {
        crate::ArchiveErrorCode::QuotaExceeded => error(UserPackageErrorCode::LimitExceeded),
        _ => error(UserPackageErrorCode::SourceUnsafe),
    }
}

fn map_cache(error_value: crate::CacheError) -> UserPackageError {
    match error_value.code() {
        crate::CacheErrorCode::QuotaExceeded | crate::CacheErrorCode::DownloadTooLarge => {
            error(UserPackageErrorCode::LimitExceeded)
        }
        _ => internal(),
    }
}

fn error(code: UserPackageErrorCode) -> UserPackageError {
    UserPackageError::new(code)
}

fn internal() -> UserPackageError {
    error(UserPackageErrorCode::Internal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alcomd_application::UserPackageAdapter;

    fn temporary_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "alcomd-user-package-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    #[cfg(unix)]
    fn short_unix_socket_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "aup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_millis()
        ))
    }

    fn assert_path_collision(left: &str, right: &str) {
        let limits = ArchiveLimits::package();
        let (left_path, left_key) = normalize_path(left, limits).expect("left path");
        let (right_path, right_key) = normalize_path(right, limits).expect("right path");
        let mut normalized_paths = BTreeMap::new();
        let mut explicit_paths = BTreeSet::new();
        validate_collision(
            &left_path,
            &left_key,
            false,
            &mut normalized_paths,
            &mut explicit_paths,
        )
        .expect("first path");
        assert_eq!(
            validate_collision(
                &right_path,
                &right_key,
                false,
                &mut normalized_paths,
                &mut explicit_paths,
            )
            .expect_err("collision")
            .code(),
            crate::ArchiveErrorCode::PathCollision
        );
    }

    fn fixture(root: &Path) {
        std::fs::create_dir_all(root.join("Runtime")).expect("fixture directories");
        std::fs::write(
            root.join("package.json"),
            br#"{"name":"com.example.local","version":"1.2.3-beta.1","displayName":"Local fixture","vpmDependencies":{"com.example.dep":"^1.0.0"}}"#,
        )
        .expect("manifest");
        std::fs::write(root.join("Runtime").join("fixture.txt"), b"fixture").expect("payload");
    }

    #[tokio::test]
    async fn valid_loose_directory_produces_deterministic_owned_archive() {
        let root = temporary_root("deterministic");
        let source = root.join("source");
        fixture(&source);
        let engine =
            UserPackageEngine::new(root.join("cache"), root.join("staging")).expect("engine");
        let first = engine
            .snapshot(source.to_string_lossy().into_owned())
            .await
            .expect("first snapshot");
        let second = engine
            .snapshot(source.to_string_lossy().into_owned())
            .await
            .expect("second snapshot");
        assert_eq!(first.package_id, "com.example.local");
        assert_eq!(first.version, "1.2.3-beta.1");
        assert_eq!(first.content_fingerprint, second.content_fingerprint);
        assert_eq!(first.archive_sha256, second.archive_sha256);
        assert!(engine.cache.object_path(&first.archive_sha256).is_file());
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn hard_link_is_rejected_fail_closed() {
        let root = temporary_root("hardlink");
        let source = root.join("source");
        fixture(&source);
        std::fs::hard_link(
            source.join("Runtime").join("fixture.txt"),
            source.join("Runtime").join("alias.txt"),
        )
        .expect("hard link");
        let error_value = prepare_snapshot(&source, &root.join("staging")).expect_err("reject");
        assert_eq!(error_value.code(), UserPackageErrorCode::SourceUnsafe);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn archive_file_is_not_accepted_as_a_loose_package_root() {
        let root = temporary_root("archive-root");
        std::fs::create_dir_all(&root).expect("root");
        let archive = root.join("package.zip");
        std::fs::write(&archive, b"not a directory").expect("archive fixture");
        let error_value = prepare_snapshot(&archive, &root.join("staging")).expect_err("reject");
        assert_eq!(error_value.code(), UserPackageErrorCode::SourceUnsafe);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn root_symlink_is_rejected_fail_closed() {
        use std::os::unix::fs::symlink;

        let root = temporary_root("root-symlink");
        let source = root.join("source");
        let link = root.join("linked-source");
        fixture(&source);
        symlink(&source, &link).expect("root symlink");
        let error_value = prepare_snapshot(&link, &root.join("staging")).expect_err("reject");
        assert_eq!(error_value.code(), UserPackageErrorCode::SourceUnsafe);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(windows)]
    #[test]
    fn root_reparse_directory_is_rejected_fail_closed() {
        let root = temporary_root("root-reparse");
        let source = root.join("source");
        let link = root.join("linked-source");
        fixture(&source);
        std::os::windows::fs::symlink_dir(&source, &link).expect("directory reparse point");
        let error_value = prepare_snapshot(&link, &root.join("staging")).expect_err("reject");
        assert_eq!(error_value.code(), UserPackageErrorCode::SourceUnsafe);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn nested_symlink_is_rejected_fail_closed() {
        use std::os::unix::fs::symlink;

        let root = temporary_root("symlink");
        let source = root.join("source");
        fixture(&source);
        symlink(root.join("outside"), source.join("Runtime").join("link")).expect("symlink");
        let error_value = prepare_snapshot(&source, &root.join("staging")).expect_err("reject");
        assert_eq!(error_value.code(), UserPackageErrorCode::SourceUnsafe);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(windows)]
    #[test]
    fn nested_reparse_directory_is_rejected_fail_closed() {
        let root = temporary_root("nested-reparse");
        let source = root.join("source");
        let outside = root.join("outside");
        fixture(&source);
        std::fs::create_dir_all(&outside).expect("outside directory");
        std::os::windows::fs::symlink_dir(&outside, source.join("Runtime").join("link"))
            .expect("nested directory reparse point");
        let error_value = prepare_snapshot(&source, &root.join("staging")).expect_err("reject");
        assert_eq!(error_value.code(), UserPackageErrorCode::SourceUnsafe);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn package_manifest_requires_strict_semver() {
        let root = temporary_root("semver");
        let source = root.join("source");
        std::fs::create_dir_all(&source).expect("source");
        std::fs::write(
            source.join("package.json"),
            br#"{"name":"com.example.local","version":"1.2"}"#,
        )
        .expect("manifest");
        let error_value = prepare_snapshot(&source, &root.join("staging")).expect_err("reject");
        assert_eq!(error_value.code(), UserPackageErrorCode::ManifestInvalid);
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn missing_and_malformed_manifests_are_rejected() {
        let root = temporary_root("manifest-invalid");
        let missing = root.join("missing");
        std::fs::create_dir_all(&missing).expect("missing fixture");
        assert_eq!(
            prepare_snapshot(&missing, &root.join("staging-missing"))
                .expect_err("missing manifest")
                .code(),
            UserPackageErrorCode::ManifestInvalid
        );
        let malformed = root.join("malformed");
        std::fs::create_dir_all(&malformed).expect("malformed fixture");
        std::fs::write(malformed.join("package.json"), b"{").expect("malformed manifest");
        assert_eq!(
            prepare_snapshot(&malformed, &root.join("staging-malformed"))
                .expect_err("malformed manifest")
                .code(),
            UserPackageErrorCode::ManifestInvalid
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn invalid_optional_links_are_omitted_without_invalidating_manifest() {
        let manifest = parse_manifest(
            br#"{"name":"com.example.local","version":"1.2.3","documentationUrl":42,"changelogUrl":"file:///private/changelog"}"#,
        )
        .expect("optional presentation links must not invalidate package authority");
        let value: Value = serde_json::from_str(&manifest.manifest_json).expect("manifest JSON");
        assert!(value.get("documentationUrl").is_none());
        assert!(value.get("changelogUrl").is_none());
    }

    #[cfg(unix)]
    #[test]
    fn special_and_non_utf8_entries_are_rejected() {
        use std::os::unix::ffi::OsStringExt;

        let special_root = short_unix_socket_root();
        let special_source = special_root.join("s");
        fixture(&special_source);
        let socket = std::os::unix::net::UnixListener::bind(special_source.join("x"))
            .expect("socket fixture");
        assert_eq!(
            prepare_snapshot(&special_source, &special_root.join("t"))
                .expect_err("special file")
                .code(),
            UserPackageErrorCode::SourceUnsafe
        );
        drop(socket);
        std::fs::remove_dir_all(special_root).expect("cleanup special");

        let text_root = temporary_root("non-utf8");
        let text_source = text_root.join("source");
        fixture(&text_source);
        let invalid_name = std::ffi::OsString::from_vec(vec![b'i', b'n', b'v', 0xff]);
        let invalid_path = text_source.join(&invalid_name);
        match std::fs::write(&invalid_path, b"invalid") {
            Ok(()) => assert_eq!(
                prepare_snapshot(&text_source, &text_root.join("staging"))
                    .expect_err("non-UTF-8 entry")
                    .code(),
                UserPackageErrorCode::SourceUnsafe
            ),
            Err(error_value) if cfg!(target_os = "macos") => {
                assert!(invalid_name.to_str().is_none());
                assert!(!invalid_path.exists());
                assert_ne!(error_value.kind(), std::io::ErrorKind::NotFound);
            }
            Err(error_value) => panic!("non-UTF-8 fixture: {error_value}"),
        }
        std::fs::remove_dir_all(text_root).expect("cleanup text");
    }

    #[test]
    fn case_and_unicode_normalization_collisions_are_rejected() {
        assert_path_collision("A.txt", "a.txt");
        assert_path_collision("é.txt", "e\u{301}.txt");

        #[cfg(target_os = "linux")]
        {
            let case_root = temporary_root("case-collision");
            let case_source = case_root.join("source");
            fixture(&case_source);
            std::fs::write(case_source.join("A.txt"), b"A").expect("uppercase");
            std::fs::write(case_source.join("a.txt"), b"a").expect("lowercase");
            assert_eq!(
                prepare_snapshot(&case_source, &case_root.join("staging"))
                    .expect_err("case collision")
                    .code(),
                UserPackageErrorCode::SourceUnsafe
            );
            std::fs::remove_dir_all(case_root).expect("cleanup case");

            let unicode_root = temporary_root("unicode-collision");
            let unicode_source = unicode_root.join("source");
            fixture(&unicode_source);
            std::fs::write(unicode_source.join("é.txt"), b"one").expect("composed");
            std::fs::write(unicode_source.join("e\u{301}.txt"), b"two").expect("decomposed");
            assert_eq!(
                prepare_snapshot(&unicode_source, &unicode_root.join("staging"))
                    .expect_err("Unicode collision")
                    .code(),
                UserPackageErrorCode::SourceUnsafe
            );
            std::fs::remove_dir_all(unicode_root).expect("cleanup unicode");
        }
    }

    #[test]
    fn package_archive_profile_limits_are_reused_exactly() {
        let limits = ArchiveLimits::package();
        assert_eq!(limits.entries, 65_536);
        assert_eq!(limits.entry_bytes, 1_073_741_824);
        assert_eq!(limits.total_uncompressed_bytes, 4_294_967_296);
        assert_eq!(limits.path_depth, 64);
        assert_eq!(limits.normalized_path_bytes, 1_024);
        assert_eq!(MAX_ARCHIVE_BYTES, 1_073_741_824);
    }

    #[test]
    fn source_tree_and_final_archive_quotas_fail_closed() {
        let root = temporary_root("injected-quotas");
        let source = root.join("source");
        fixture(&source);
        for limits in [
            ArchiveLimits {
                entries: 2,
                ..ArchiveLimits::package()
            },
            ArchiveLimits {
                entry_bytes: 4,
                ..ArchiveLimits::package()
            },
            ArchiveLimits {
                total_uncompressed_bytes: 4,
                ..ArchiveLimits::package()
            },
            ArchiveLimits {
                path_depth: 1,
                ..ArchiveLimits::package()
            },
            ArchiveLimits {
                normalized_path_bytes: 5,
                ..ArchiveLimits::package()
            },
        ] {
            assert_eq!(
                inventory_with_limits(&source, limits)
                    .expect_err("injected source-tree quota")
                    .code(),
                UserPackageErrorCode::LimitExceeded
            );
        }

        let entries = inventory(&source).expect("normal inventory");
        assert_eq!(
            write_archive_with_limit(&root.join("too-small.zip"), &entries, 160)
                .expect_err("owned archive quota")
                .code(),
            UserPackageErrorCode::LimitExceeded
        );
        std::fs::remove_dir_all(root).expect("cleanup");
    }
}
