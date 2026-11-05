//! Read-only, bounded evidence that the selected package requires no cleanup.
//! Observed paths are never eligible for deletion through this module.
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use alcomd_application::{M4Error, M4ErrorCode};
use serde_json::Value;

use crate::package::LegacyMetadata;

const ENTRY_LIMIT: usize = 65_536;
const META_BYTE_LIMIT: usize = 64 * 1024;
const TOTAL_META_BYTE_LIMIT: usize = 64 * 1024 * 1024;

fn unsupported() -> M4Error {
    M4Error::new(M4ErrorCode::PackageLegacyCleanupRequired)
}

#[derive(Eq, PartialEq)]
struct EntryStamp {
    identity: Vec<u8>,
    modified: SystemTime,
    len: u64,
    directory: bool,
    file: bool,
}

impl EntryStamp {
    fn read(path: &Path) -> Result<Self, M4Error> {
        let metadata = std::fs::symlink_metadata(path).map_err(|_| unsupported())?;
        if unsafe_entry(&metadata) || !metadata.is_dir() && !metadata.is_file() {
            return Err(unsupported());
        }
        Ok(Self {
            identity: alcomd_platform::file_identity_key(path).map_err(|_| unsupported())?,
            modified: metadata.modified().map_err(|_| unsupported())?,
            len: metadata.len(),
            directory: metadata.is_dir(),
            file: metadata.is_file(),
        })
    }
}

/// Bounded metadata retained only for this synchronous proof. In particular,
/// directory identities/timestamps are rechecked after the entire traversal so
/// a child added to an already visited directory cannot become proof of absence.
struct Snapshot<'a> {
    root: &'a Path,
    entries: BTreeMap<PathBuf, EntryStamp>,
    absent: BTreeSet<PathBuf>,
}

impl<'a> Snapshot<'a> {
    fn new(root: &'a Path) -> Result<Self, M4Error> {
        let mut snapshot = Self {
            root,
            entries: BTreeMap::new(),
            absent: BTreeSet::new(),
        };
        if !snapshot.observe(root)?.directory {
            return Err(unsupported());
        }
        Ok(snapshot)
    }

    fn observe(&mut self, path: &Path) -> Result<&EntryStamp, M4Error> {
        let stamp = EntryStamp::read(path)?;
        if let Some(previous) = self.entries.get(path) {
            if previous != &stamp {
                return Err(unsupported());
            }
        } else {
            if self.entries.len() >= ENTRY_LIMIT {
                return Err(unsupported());
            }
            self.entries.insert(path.to_owned(), stamp);
        }
        self.entries.get(path).ok_or_else(unsupported)
    }

    fn missing(&mut self, relative: &Path) -> Result<bool, M4Error> {
        if relative
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(unsupported());
        }
        let mut path = self.root.to_owned();
        let mut parts = relative.components().peekable();
        while let Some(part) = parts.next() {
            path.push(part);
            match std::fs::symlink_metadata(&path) {
                Ok(_) => {
                    let entry = self.observe(&path)?;
                    if parts.peek().is_some() && !entry.directory {
                        return Err(unsupported());
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    if self.absent.len() >= ENTRY_LIMIT {
                        return Err(unsupported());
                    }
                    self.absent.insert(path);
                    return Ok(true);
                }
                Err(_) => return Err(unsupported()),
            }
        }
        Ok(false)
    }

    fn read_file(&mut self, path: &Path, limit: usize) -> Result<Vec<u8>, M4Error> {
        let before = self.observe(path)?;
        if !before.file || before.len > limit as u64 {
            return Err(unsupported());
        }
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .map_err(|_| unsupported())?
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| unsupported())?;
        if bytes.len() > limit {
            return Err(unsupported());
        }
        if self.observe(path)?.len != bytes.len() as u64 {
            return Err(unsupported());
        }
        Ok(bytes)
    }

    fn verify(&self) -> Result<(), M4Error> {
        for (path, before) in &self.entries {
            if &EntryStamp::read(path)? != before {
                return Err(unsupported());
            }
        }
        for path in &self.absent {
            match std::fs::symlink_metadata(path) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                _ => return Err(unsupported()),
            }
        }
        Ok(())
    }
}

pub(crate) fn verify_absent(root: &Path, metadata: &LegacyMetadata) -> Result<(), M4Error> {
    verify_absent_before_final_check(root, metadata, || {})
}

fn verify_absent_before_final_check(
    root: &Path,
    metadata: &LegacyMetadata,
    before_final_check: impl FnOnce(),
) -> Result<(), M4Error> {
    if metadata.is_empty() {
        return Ok(());
    }
    let mut snapshot = Snapshot::new(root)?;
    for path in metadata.folders.keys().chain(metadata.files.keys()) {
        if path.contains(['\\', ':']) {
            return Err(unsupported());
        }
        let path = Path::new(path);
        if !path.starts_with("Assets")
            || path.components().count() < 2
            || path
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(unsupported());
        }
        if !snapshot.missing(path)?
            || !snapshot.missing(&PathBuf::from(format!("{}.meta", path.to_string_lossy())))?
        {
            return Err(unsupported());
        }
    }
    if !metadata.packages.is_empty() {
        let relative = Path::new("Packages/vpm-manifest.json");
        if snapshot.missing(relative)? {
            return Err(unsupported());
        }
        let bytes = snapshot.read_file(&root.join(relative), crate::PROJECT_MANIFEST_LIMIT)?;
        let manifest: Value =
            serde_json::from_slice(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes))
                .map_err(|_| unsupported())?;
        let manifest = manifest.as_object().ok_or_else(unsupported)?;
        for key in ["dependencies", "locked"] {
            if manifest.get(key).is_some_and(|value| !value.is_object()) {
                return Err(unsupported());
            }
        }
        for package in &metadata.packages {
            if ["dependencies", "locked"].iter().any(|key| {
                manifest
                    .get(*key)
                    .and_then(|value| value.get(package))
                    .is_some()
            }) || !snapshot.missing(&Path::new("Packages").join(package))?
            {
                return Err(unsupported());
            }
        }
    }
    let guids = metadata
        .folders
        .values()
        .chain(metadata.files.values())
        .filter(|guid| !guid.is_empty())
        .cloned()
        .collect::<BTreeSet<_>>();
    if !guids.is_empty() && !snapshot.missing(Path::new("Assets"))? {
        let mut pending = vec![root.join("Assets")];
        let mut visited = 0_usize;
        let mut total_bytes = 0_usize;
        while let Some(path) = pending.pop() {
            visited += 1;
            if visited > ENTRY_LIMIT {
                return Err(unsupported());
            }
            let entry = snapshot.observe(&path)?;
            if entry.directory {
                for child in std::fs::read_dir(&path).map_err(|_| unsupported())? {
                    pending.push(child.map_err(|_| unsupported())?.path());
                    if pending.len() + visited > ENTRY_LIMIT {
                        return Err(unsupported());
                    }
                }
            } else if path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("meta"))
            {
                let bytes = snapshot.read_file(&path, META_BYTE_LIMIT)?;
                total_bytes = total_bytes
                    .checked_add(bytes.len())
                    .ok_or_else(unsupported)?;
                if total_bytes > TOTAL_META_BYTE_LIMIT {
                    return Err(unsupported());
                }
                if guids.contains(&meta_guid(&bytes)?) {
                    return Err(unsupported());
                }
            }
        }
    }
    before_final_check();
    snapshot.verify()
}

/// Unity-generated meta files use one unquoted hexadecimal GUID. Other syntax
/// is unknown to this bounded reader and cannot establish absence.
fn meta_guid(bytes: &[u8]) -> Result<String, M4Error> {
    let text = std::str::from_utf8(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes))
        .map_err(|_| unsupported())?;
    let mut guids = text.lines().filter_map(|line| {
        let (key, value) = line.trim().split_once(':')?;
        (key.trim() == "guid").then_some(value.trim())
    });
    let guid = guids.next().ok_or_else(unsupported)?;
    if guid.len() != 32
        || !guid.bytes().all(|byte| byte.is_ascii_hexdigit())
        || guids.next().is_some()
    {
        return Err(unsupported());
    }
    Ok(guid.to_ascii_lowercase())
}

fn unsafe_entry(metadata: &std::fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    metadata.file_type().is_symlink()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "alcomd-legacy-{}",
                alcomd_application::OperationId::new()
            ));
            std::fs::create_dir_all(root.join("Packages")).expect("fixture");
            std::fs::create_dir(root.join("Assets")).expect("assets");
            std::fs::write(
                root.join("Packages/vpm-manifest.json"),
                br#"{"dependencies":{},"locked":{}}"#,
            )
            .expect("manifest");
            Self(root)
        }
        fn metadata(&self) -> LegacyMetadata {
            let source = serde_json::json!({"legacyFolders":{"Assets/Old":"0123456789abcdef0123456789abcdef"},"legacyFiles":{},"legacyPackages":["com.example.old"]});
            LegacyMetadata::parse(source.as_object().expect("object")).expect("official shapes")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).expect("owned fixture cleanup");
        }
    }

    #[test]
    fn metadata_is_not_cleanup_but_path_guid_and_package_evidence_block() {
        let fixture = Fixture::new();
        let root = &fixture.0;
        let metadata = fixture.metadata();
        assert_eq!(
            metadata.folders["Assets/Old"],
            "0123456789abcdef0123456789abcdef"
        );
        verify_absent(root, &metadata).expect("metadata alone does not require cleanup");
        std::fs::write(
            root.join("Assets/Moved.meta"),
            "guid: 0123456789abcdef0123456789abcdef\n",
        )
        .expect("moved guid");
        assert_eq!(
            verify_absent(root, &metadata)
                .expect_err("moved object requires cleanup")
                .code(),
            M4ErrorCode::PackageLegacyCleanupRequired
        );
        std::fs::remove_file(root.join("Assets/Moved.meta")).expect("remove fixture");
        std::fs::create_dir(root.join("Assets/Old")).expect("path evidence");
        assert!(verify_absent(root, &metadata).is_err());
        std::fs::remove_dir(root.join("Assets/Old")).expect("remove fixture");
        for manifest in [
            br#"{"dependencies":{"com.example.old":{"version":"1.0.0"}},"locked":{}}"#.as_slice(),
            br#"{"dependencies":{},"locked":{"com.example.old":{"version":"1.0.0"}}}"#,
        ] {
            std::fs::write(root.join("Packages/vpm-manifest.json"), manifest)
                .expect("direct/lock evidence");
            assert!(verify_absent(root, &metadata).is_err());
        }
    }

    #[test]
    fn unreadable_or_oversized_meta_never_becomes_absence() {
        let fixture = Fixture::new();
        let root = &fixture.0;
        let path = root.join("Assets/Unrelated.meta");
        for bytes in [
            Vec::new(),
            b"guid: malformed\n".to_vec(),
            vec![b'x'; META_BYTE_LIMIT + 1],
        ] {
            std::fs::write(&path, bytes).expect("unknown meta");
            assert!(verify_absent(root, &fixture.metadata()).is_err());
        }
        std::fs::write(&path, "guid: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n").expect("unrelated meta");
        verify_absent(root, &fixture.metadata()).expect("valid unrelated GUID");
    }

    #[test]
    fn late_child_directory_and_same_length_meta_replacements_invalidate_proof() {
        for directory in [true, false] {
            let fixture = Fixture::new();
            let root = &fixture.0;
            let path = root.join("Assets/Unrelated.meta");
            std::fs::write(&path, "guid: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n")
                .expect("unrelated meta");
            let result = verify_absent_before_final_check(root, &fixture.metadata(), || {
                if directory {
                    std::fs::rename(root.join("Assets"), root.join("OriginalAssets"))
                        .expect("move old directory");
                    std::fs::create_dir(root.join("Assets")).expect("replace directory");
                } else {
                    std::fs::rename(&path, root.join("original.meta")).expect("move old meta");
                    std::fs::write(&path, "guid: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\n")
                        .expect("replace same length");
                }
            });
            assert!(result.is_err(), "directory={directory}");
        }
        let fixture = Fixture::new();
        let result = verify_absent_before_final_check(&fixture.0, &fixture.metadata(), || {
            std::fs::create_dir(fixture.0.join("Assets/Old")).expect("late legacy path");
        });
        assert!(result.is_err());
    }

    #[test]
    fn file_identity_is_checked_even_when_length_and_modified_time_match() {
        let fixture = Fixture::new();
        let path = fixture.0.join("Assets/Unrelated.meta");
        let bytes = b"guid: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n";
        std::fs::write(&path, bytes).expect("initial meta");
        let mut snapshot = Snapshot::new(&fixture.0).expect("snapshot");
        snapshot
            .read_file(&path, META_BYTE_LIMIT)
            .expect("original stable read");
        let original = EntryStamp::read(&path).expect("original stamp");
        std::fs::rename(&path, fixture.0.join("original.meta")).expect("preserve old file");
        std::fs::write(&path, bytes).expect("same bytes in new file");
        std::fs::File::options()
            .write(true)
            .open(&path)
            .expect("open replacement")
            .set_modified(original.modified)
            .expect("preserve modified time");
        let replaced = EntryStamp::read(&path).expect("replacement stamp");
        assert_eq!(original.len, replaced.len);
        assert_eq!(original.modified, replaced.modified);
        assert_ne!(original.identity, replaced.identity);
        assert!(snapshot.read_file(&path, META_BYTE_LIMIT).is_err());
    }
}
