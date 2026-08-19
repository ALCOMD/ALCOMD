use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, Write};
use std::path::{Path, PathBuf};

use unicode_normalization::UnicodeNormalization;
use zip::CompressionMethod;
use zip::ZipArchive;

pub const MAX_ARCHIVE_BYTES: u64 = 1_073_741_824;
pub const MAX_ARCHIVE_ENTRIES: usize = 65_536;
pub const MAX_ENTRY_BYTES: u64 = 1_073_741_824;
pub const MAX_TOTAL_UNCOMPRESSED_BYTES: u64 = 4_294_967_296;
pub const MAX_PATH_DEPTH: usize = 64;
pub const MAX_NORMALIZED_PATH_BYTES: usize = 1_024;
pub const MAX_EXPANSION_RATIO: u64 = 1_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArchiveErrorCode {
    Invalid,
    UnsupportedCompression,
    Encrypted,
    UnsafePath,
    LinkOrSpecialFile,
    PathCollision,
    QuotaExceeded,
    Io,
}

#[derive(Debug)]
pub struct ArchiveError {
    code: ArchiveErrorCode,
}

impl ArchiveError {
    #[must_use]
    pub const fn code(&self) -> ArchiveErrorCode {
        self.code
    }
}

impl fmt::Display for ArchiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "package archive rejected: {:?}", self.code)
    }
}

impl std::error::Error for ArchiveError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchiveEntry {
    pub index: usize,
    pub relative_path: PathBuf,
    pub directory: bool,
    pub compressed_size: u64,
    pub uncompressed_size: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArchivePreflight {
    pub entries: Vec<ArchiveEntry>,
    pub total_uncompressed_bytes: u64,
}

pub fn preflight_archive(path: &Path) -> Result<ArchivePreflight, ArchiveError> {
    let metadata = std::fs::metadata(path).map_err(|_| archive_error(ArchiveErrorCode::Io))?;
    if !metadata.is_file() || metadata.len() > MAX_ARCHIVE_BYTES {
        return Err(archive_error(ArchiveErrorCode::QuotaExceeded));
    }
    let file = File::open(path).map_err(|_| archive_error(ArchiveErrorCode::Io))?;
    preflight_reader(file)
}

pub fn extract_archive(
    archive_path: &Path,
    destination: &Path,
) -> Result<ArchivePreflight, ArchiveError> {
    validate_empty_destination(destination)?;
    let preflight = preflight_archive(archive_path)?;
    let file = File::open(archive_path).map_err(|_| archive_error(ArchiveErrorCode::Io))?;
    let mut archive =
        ZipArchive::new(file).map_err(|_| archive_error(ArchiveErrorCode::Invalid))?;
    let mut streamed_total = 0_u64;
    for planned in &preflight.entries {
        let mut entry = archive
            .by_index(planned.index)
            .map_err(|_| archive_error(ArchiveErrorCode::Invalid))?;
        if entry.name_raw() != normalized_archive_name(&entry)?.as_bytes()
            && std::str::from_utf8(entry.name_raw()).is_err()
        {
            return Err(archive_error(ArchiveErrorCode::UnsafePath));
        }
        let target = destination.join(&planned.relative_path);
        if planned.directory {
            create_directory_path(destination, &planned.relative_path)?;
            continue;
        }
        let parent = planned
            .relative_path
            .parent()
            .unwrap_or_else(|| Path::new(""));
        create_directory_path(destination, parent)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|_| archive_error(ArchiveErrorCode::Io))?;
        let copied = copy_bounded(&mut entry, &mut output, planned.uncompressed_size)?;
        if copied != planned.uncompressed_size {
            return Err(archive_error(ArchiveErrorCode::Invalid));
        }
        streamed_total = streamed_total
            .checked_add(copied)
            .filter(|total| *total <= MAX_TOTAL_UNCOMPRESSED_BYTES)
            .ok_or_else(|| archive_error(ArchiveErrorCode::QuotaExceeded))?;
        output
            .flush()
            .and_then(|()| output.sync_all())
            .map_err(|_| archive_error(ArchiveErrorCode::Io))?;
    }
    if streamed_total
        != preflight
            .entries
            .iter()
            .filter(|entry| !entry.directory)
            .map(|entry| entry.uncompressed_size)
            .sum::<u64>()
    {
        return Err(archive_error(ArchiveErrorCode::Invalid));
    }
    sync_tree_directories(destination, &preflight.entries)?;
    Ok(preflight)
}

fn preflight_reader<R: Read + Seek>(reader: R) -> Result<ArchivePreflight, ArchiveError> {
    let mut archive =
        ZipArchive::new(reader).map_err(|_| archive_error(ArchiveErrorCode::Invalid))?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(archive_error(ArchiveErrorCode::QuotaExceeded));
    }
    let mut entries = Vec::with_capacity(archive.len());
    let mut total = 0_u64;
    let mut normalized_paths = BTreeMap::<String, bool>::new();
    let mut explicit_paths = BTreeSet::<PathBuf>::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|_| archive_error(ArchiveErrorCode::Invalid))?;
        validate_entry_type(&entry)?;
        let raw_name = std::str::from_utf8(entry.name_raw())
            .map_err(|_| archive_error(ArchiveErrorCode::UnsafePath))?;
        if raw_name != entry.name() {
            return Err(archive_error(ArchiveErrorCode::UnsafePath));
        }
        let (relative_path, collision_key) = normalize_path(raw_name)?;
        let directory = entry.is_dir();
        validate_collision(
            &relative_path,
            &collision_key,
            directory,
            &mut normalized_paths,
            &mut explicit_paths,
        )?;
        let uncompressed_size = entry.size();
        let compressed_size = entry.compressed_size();
        if uncompressed_size > MAX_ENTRY_BYTES
            || expansion_ratio_exceeded(uncompressed_size, compressed_size)
        {
            return Err(archive_error(ArchiveErrorCode::QuotaExceeded));
        }
        total = total
            .checked_add(uncompressed_size)
            .filter(|total| *total <= MAX_TOTAL_UNCOMPRESSED_BYTES)
            .ok_or_else(|| archive_error(ArchiveErrorCode::QuotaExceeded))?;
        entries.push(ArchiveEntry {
            index,
            relative_path,
            directory,
            compressed_size,
            uncompressed_size,
        });
    }
    Ok(ArchivePreflight {
        entries,
        total_uncompressed_bytes: total,
    })
}

fn validate_entry_type<R: Read>(entry: &zip::read::ZipFile<'_, R>) -> Result<(), ArchiveError> {
    if entry.encrypted() {
        return Err(archive_error(ArchiveErrorCode::Encrypted));
    }
    if !matches!(
        entry.compression(),
        CompressionMethod::Stored | CompressionMethod::Deflated
    ) {
        return Err(archive_error(ArchiveErrorCode::UnsupportedCompression));
    }
    if let Some(mode) = entry.unix_mode() {
        let file_type = mode & 0o170000;
        let valid = file_type == 0 || file_type == 0o040000 || file_type == 0o100000;
        if !valid || (entry.is_dir() && file_type == 0o100000) {
            return Err(archive_error(ArchiveErrorCode::LinkOrSpecialFile));
        }
    }
    Ok(())
}

fn normalized_archive_name<R: Read>(
    entry: &zip::read::ZipFile<'_, R>,
) -> Result<String, ArchiveError> {
    std::str::from_utf8(entry.name_raw())
        .map(str::to_owned)
        .map_err(|_| archive_error(ArchiveErrorCode::UnsafePath))
}

fn normalize_path(raw: &str) -> Result<(PathBuf, String), ArchiveError> {
    if raw.is_empty()
        || raw.starts_with('/')
        || raw.starts_with('\\')
        || raw.bytes().any(|byte| byte == 0 || byte.is_ascii_control())
    {
        return Err(archive_error(ArchiveErrorCode::UnsafePath));
    }
    let normalized_separators = raw.replace('\\', "/");
    let trimmed = normalized_separators
        .strip_suffix('/')
        .unwrap_or(&normalized_separators);
    if trimmed.is_empty()
        || trimmed.starts_with("//")
        || trimmed.split('/').any(|segment| segment.is_empty())
    {
        return Err(archive_error(ArchiveErrorCode::UnsafePath));
    }
    let segments = trimmed.split('/').collect::<Vec<_>>();
    if segments.len() > MAX_PATH_DEPTH {
        return Err(archive_error(ArchiveErrorCode::QuotaExceeded));
    }
    for segment in &segments {
        validate_segment(segment)?;
    }
    let normalized = segments
        .iter()
        .map(|segment| segment.nfc().collect::<String>())
        .collect::<Vec<_>>()
        .join("/");
    if normalized.len() > MAX_NORMALIZED_PATH_BYTES {
        return Err(archive_error(ArchiveErrorCode::QuotaExceeded));
    }
    let collision_key = normalized.to_lowercase();
    Ok((PathBuf::from(normalized), collision_key))
}

fn validate_segment(segment: &str) -> Result<(), ArchiveError> {
    if matches!(segment, "." | "..")
        || segment.contains(':')
        || segment.ends_with('.')
        || segment.ends_with(' ')
    {
        return Err(archive_error(ArchiveErrorCode::UnsafePath));
    }
    let stem = segment
        .split('.')
        .next()
        .unwrap_or(segment)
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|suffix| suffix.len() == 1 && matches!(suffix.as_bytes()[0], b'1'..=b'9'));
    if reserved {
        return Err(archive_error(ArchiveErrorCode::UnsafePath));
    }
    Ok(())
}

fn validate_collision(
    path: &Path,
    collision_key: &str,
    directory: bool,
    normalized_paths: &mut BTreeMap<String, bool>,
    explicit_paths: &mut BTreeSet<PathBuf>,
) -> Result<(), ArchiveError> {
    if normalized_paths
        .insert(collision_key.to_owned(), directory)
        .is_some()
        || !explicit_paths.insert(path.to_path_buf())
    {
        return Err(archive_error(ArchiveErrorCode::PathCollision));
    }
    let mut ancestor = path.parent();
    while let Some(parent) = ancestor {
        if parent.as_os_str().is_empty() {
            break;
        }
        let key = parent.to_string_lossy().replace('\\', "/").to_lowercase();
        if normalized_paths.get(&key) == Some(&false) {
            return Err(archive_error(ArchiveErrorCode::PathCollision));
        }
        ancestor = parent.parent();
    }
    if !directory {
        let prefix = format!("{collision_key}/");
        if normalized_paths.keys().any(|key| key.starts_with(&prefix)) {
            return Err(archive_error(ArchiveErrorCode::PathCollision));
        }
    }
    Ok(())
}

fn expansion_ratio_exceeded(uncompressed: u64, compressed: u64) -> bool {
    if uncompressed == 0 {
        false
    } else if compressed == 0 {
        true
    } else {
        uncompressed > compressed.saturating_mul(MAX_EXPANSION_RATIO)
    }
}

fn validate_empty_destination(destination: &Path) -> Result<(), ArchiveError> {
    let metadata =
        std::fs::symlink_metadata(destination).map_err(|_| archive_error(ArchiveErrorCode::Io))?;
    if !metadata.is_dir() || is_link_or_reparse(&metadata) {
        return Err(archive_error(ArchiveErrorCode::UnsafePath));
    }
    if std::fs::read_dir(destination)
        .map_err(|_| archive_error(ArchiveErrorCode::Io))?
        .next()
        .is_some()
    {
        return Err(archive_error(ArchiveErrorCode::PathCollision));
    }
    Ok(())
}

fn create_directory_path(root: &Path, relative: &Path) -> Result<(), ArchiveError> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.is_dir() && !is_link_or_reparse(&metadata) => {}
            Ok(_) => return Err(archive_error(ArchiveErrorCode::UnsafePath)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                std::fs::create_dir(&current).map_err(|_| archive_error(ArchiveErrorCode::Io))?;
                let metadata = std::fs::symlink_metadata(&current)
                    .map_err(|_| archive_error(ArchiveErrorCode::Io))?;
                if !metadata.is_dir() || is_link_or_reparse(&metadata) {
                    return Err(archive_error(ArchiveErrorCode::UnsafePath));
                }
            }
            Err(_) => return Err(archive_error(ArchiveErrorCode::Io)),
        }
    }
    Ok(())
}

fn copy_bounded<R: Read, W: Write>(
    reader: &mut R,
    writer: &mut W,
    expected: u64,
) -> Result<u64, ArchiveError> {
    let mut limited = reader.take(expected.saturating_add(1));
    let copied = io::copy(&mut limited, writer).map_err(|_| archive_error(ArchiveErrorCode::Io))?;
    if copied > expected || copied > MAX_ENTRY_BYTES {
        return Err(archive_error(ArchiveErrorCode::QuotaExceeded));
    }
    Ok(copied)
}

fn sync_tree_directories(root: &Path, entries: &[ArchiveEntry]) -> Result<(), ArchiveError> {
    let mut directories = entries
        .iter()
        .flat_map(|entry| entry.relative_path.ancestors().skip(1))
        .map(|relative| root.join(relative))
        .collect::<BTreeSet<_>>();
    directories.insert(root.to_path_buf());
    for directory in directories.into_iter().rev() {
        alcomd_platform::sync_directory(&directory)
            .map_err(|_| archive_error(ArchiveErrorCode::Io))?;
    }
    Ok(())
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

const fn archive_error(code: ArchiveErrorCode) -> ArchiveError {
    ArchiveError { code }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use zip::write::SimpleFileOptions;

    static NEXT_TEMPORARY_PATH: AtomicU64 = AtomicU64::new(0);

    fn temporary_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "alcomd-m4-archive-{name}-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos(),
            NEXT_TEMPORARY_PATH.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn archive_with(entries: &[(&str, &[u8])]) -> (PathBuf, PathBuf) {
        let archive_path = temporary_path("input.zip");
        let destination = temporary_path("output");
        let file = File::create(&archive_path).expect("create archive");
        let mut writer = zip::ZipWriter::new(file);
        for (name, bytes) in entries {
            writer
                .start_file(
                    *name,
                    SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
                )
                .expect("entry");
            writer.write_all(bytes).expect("content");
        }
        writer.finish().expect("finish archive");
        std::fs::create_dir(&destination).expect("destination");
        (archive_path, destination)
    }

    #[test]
    fn accepted_paths_extract_with_exact_content() {
        let (archive, destination) =
            archive_with(&[("package.json", b"{}"), ("Runtime/Example.dll", b"fixture")]);
        let result = extract_archive(&archive, &destination).expect("extract");
        assert_eq!(result.entries.len(), 2);
        assert_eq!(
            std::fs::read(destination.join("Runtime/Example.dll")).expect("read"),
            b"fixture"
        );
        std::fs::remove_file(archive).expect("remove archive");
        std::fs::remove_dir_all(destination).expect("remove destination");
    }

    #[test]
    fn frozen_adversarial_paths_are_rejected() {
        for path in [
            "../escape",
            "/escape",
            "C:/escape",
            "//server/share/file",
            "//?/C:/escape",
            "file.txt:stream",
            "Runtime//file",
            "Runtime/line\nbreak",
            "Runtime/CON",
            "Runtime/name.",
            "Runtime/name ",
        ] {
            let (archive, destination) = archive_with(&[(path, b"bad")]);
            assert!(preflight_archive(&archive).is_err(), "{path}");
            std::fs::remove_file(archive).expect("remove archive");
            std::fs::remove_dir_all(destination).expect("remove destination");
        }
    }

    #[test]
    fn duplicate_case_unicode_and_file_directory_collisions_are_rejected() {
        for entries in [
            vec![("Runtime/File", b"1".as_slice()), ("runtime/file", b"2")],
            vec![("Runtime/café", b"1"), ("Runtime/cafe\u{301}", b"2")],
            vec![("Runtime/a", b"1"), ("Runtime/a/b", b"2")],
        ] {
            let (archive, destination) = archive_with(&entries);
            assert_eq!(
                preflight_archive(&archive).expect_err("collision").code(),
                ArchiveErrorCode::PathCollision
            );
            std::fs::remove_file(archive).expect("remove archive");
            std::fs::remove_dir_all(destination).expect("remove destination");
        }
    }

    #[test]
    fn destination_must_be_owned_empty_staging_not_a_prepopulated_tree() {
        let (archive, destination) = archive_with(&[("package.json", b"{}")]);
        std::fs::write(destination.join("existing"), b"do not overwrite").expect("prepopulate");
        assert_eq!(
            extract_archive(&archive, &destination)
                .expect_err("nonempty")
                .code(),
            ArchiveErrorCode::PathCollision
        );
        std::fs::remove_file(archive).expect("remove archive");
        std::fs::remove_dir_all(destination).expect("remove destination");
    }
}
