//! Bounded discovery and filesystem evidence for VPM legacy declarations.
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use alcomd_application::{LegacyRemoval, LegacyRemovalKind, M4Error, M4ErrorCode};
use serde_json::Value;
use sha2::{Digest, Sha256};

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

pub(crate) fn collect_removals(
    root: &Path,
    declarations: &BTreeMap<String, LegacyMetadata>,
) -> Result<Vec<LegacyRemoval>, M4Error> {
    if declarations.values().all(LegacyMetadata::is_empty) {
        return Ok(Vec::new());
    }
    let mut snapshot = Snapshot::new(root)?;
    let mut targets = BTreeMap::new();
    let mut unresolved = Vec::new();
    for (package_id, metadata) in declarations {
        for (kind, entries) in [
            (LegacyRemovalKind::File, &metadata.files),
            (LegacyRemovalKind::Directory, &metadata.folders),
        ] {
            for (path, guid) in entries {
                let path = removal_path(path)?;
                if matches_kind(&mut snapshot, &path, kind)? {
                    add_target(&mut snapshot, &mut targets, path, kind, package_id)?;
                } else if !guid.is_empty() {
                    unresolved.push((guid.as_str(), kind, package_id.as_str()));
                }
            }
        }
        for old_package in &metadata.packages {
            let path = removal_path(&format!("Packages/{old_package}"))?;
            if matches_kind(&mut snapshot, &path, LegacyRemovalKind::Directory)? {
                add_target(
                    &mut snapshot,
                    &mut targets,
                    path,
                    LegacyRemovalKind::Directory,
                    package_id,
                )?;
            } else if !snapshot.missing(Path::new(&path))? {
                return Err(unsupported());
            }
        }
    }
    if !unresolved.is_empty() {
        let guids = unresolved.iter().map(|(guid, _, _)| *guid).collect();
        let found = find_moved_assets(&mut snapshot, &guids)?;
        for (guid, kind, package_id) in unresolved {
            if let Some(path) = found.get(guid)
                && matches_kind(&mut snapshot, path, kind)?
            {
                add_target(&mut snapshot, &mut targets, path.clone(), kind, package_id)?;
            }
        }
    }

    // A directory already covers descendants. Keep its sibling .meta separately.
    let mut removals = Vec::new();
    let mut identities = BTreeSet::new();
    for (path, (kind, package_id)) in &targets {
        if path.match_indices('/').any(|(index, _)| {
            targets
                .get(&path[..index])
                .is_some_and(|(kind, _)| *kind == LegacyRemovalKind::Directory)
        }) {
            continue;
        }
        if removals.len() >= 4096 {
            return Err(unsupported());
        }
        verify_boundary(root, Path::new(path))?;
        let absolute = root.join(path);
        let identity = snapshot.observe(&absolute)?.identity.clone();
        if !identities.insert(identity.clone()) {
            return Err(unsupported());
        }
        removals.push(LegacyRemoval {
            path: path.clone(),
            kind: *kind,
            package_id: package_id.clone(),
            identity,
            fingerprint: fingerprint(&absolute, *kind)?,
        });
    }
    snapshot.verify()?;
    Ok(removals)
}

fn add_target(
    snapshot: &mut Snapshot<'_>,
    targets: &mut BTreeMap<String, (LegacyRemovalKind, String)>,
    path: String,
    kind: LegacyRemovalKind,
    package_id: &str,
) -> Result<(), M4Error> {
    let meta = removal_path(&format!("{path}.meta"))?;
    targets
        .entry(path)
        .or_insert_with(|| (kind, package_id.to_owned()));
    if !snapshot.missing(Path::new(&meta))? {
        if !matches_kind(snapshot, &meta, LegacyRemovalKind::File)? {
            return Err(unsupported());
        }
        targets
            .entry(meta)
            .or_insert_with(|| (LegacyRemovalKind::File, package_id.to_owned()));
    }
    Ok(())
}

fn matches_kind(
    snapshot: &mut Snapshot<'_>,
    path: &str,
    kind: LegacyRemovalKind,
) -> Result<bool, M4Error> {
    if snapshot.missing(Path::new(path))? {
        return Ok(false);
    }
    let stamp = snapshot.observe(&snapshot.root.join(path))?;
    Ok(match kind {
        LegacyRemovalKind::File => stamp.file,
        LegacyRemovalKind::Directory => stamp.directory,
    })
}

fn find_moved_assets(
    snapshot: &mut Snapshot<'_>,
    guids: &BTreeSet<&str>,
) -> Result<BTreeMap<String, String>, M4Error> {
    let mut found = BTreeMap::new();
    let mut total_bytes = 0_usize;
    if snapshot.missing(Path::new("Assets"))? {
        return Ok(found);
    }
    let mut pending = vec![snapshot.root.join("Assets")];
    let mut visited = 0;
    while let Some(path) = pending.pop() {
        visited += 1;
        let relative = path
            .strip_prefix(snapshot.root)
            .map_err(|_| unsupported())?;
        verify_boundary(snapshot.root, relative)?;
        if snapshot.observe(&path)?.directory {
            for child in std::fs::read_dir(&path).map_err(|_| unsupported())? {
                pending.push(child.map_err(|_| unsupported())?.path());
                if pending.len() + visited > ENTRY_LIMIT {
                    return Err(unsupported());
                }
            }
        } else if let Some(asset) = relative
            .to_str()
            .and_then(|path| path.strip_suffix(".meta"))
        {
            let bytes = snapshot.read_file(&path, META_BYTE_LIMIT)?;
            total_bytes += bytes.len();
            if total_bytes > TOTAL_META_BYTE_LIMIT {
                return Err(unsupported());
            }
            let guid = meta_guid(&bytes)?;
            if guids.contains(guid.as_str()) && found.insert(guid, removal_path(asset)?).is_some() {
                return Err(unsupported());
            }
        }
    }
    Ok(found)
}

pub(crate) fn removal_path(value: &str) -> Result<String, M4Error> {
    let normalized = value.replace('\\', "/");
    if normalized.len() > 1024 {
        return Err(unsupported());
    }
    for part in normalized.split('/') {
        if part.is_empty()
            || part.chars().any(char::is_control)
            || part.contains(['<', '>', '"', '|', '?', '*'])
            || crate::archive::validate_segment(part).is_err()
        {
            return Err(unsupported());
        }
    }
    let lower = normalized.to_ascii_lowercase();
    let root = lower.split('/').next().ok_or_else(unsupported)?;
    if matches!(
        root,
        "library" | ".git" | ".hg" | ".svn" | "temp" | "obj" | "logs" | "usersettings"
    ) || matches!(
        lower.as_str(),
        "assets"
            | "packages"
            | "projectsettings"
            | "packages/manifest.json"
            | "packages/vpm-manifest.json"
            | "projectsettings/projectversion.txt"
    ) {
        return Err(unsupported());
    }
    Ok(normalized)
}

pub(crate) fn verify_boundary(root: &Path, relative: &Path) -> Result<(), M4Error> {
    alcomd_platform::verify_project_path_boundary(root, relative)
        .map_err(|cause| unsupported().with_source(cause))
}

pub(crate) fn verify_removal(path: &Path, removal: &LegacyRemoval) -> Result<(), M4Error> {
    let before = EntryStamp::read(path)?;
    if before.identity != removal.identity
        || fingerprint(path, removal.kind)? != removal.fingerprint
        || EntryStamp::read(path)? != before
    {
        return Err(unsupported());
    }
    Ok(())
}

fn fingerprint(path: &Path, kind: LegacyRemovalKind) -> Result<[u8; 32], M4Error> {
    let before = EntryStamp::read(path)?;
    let hash = match kind {
        LegacyRemovalKind::Directory if before.directory => {
            crate::plan::checked_tree_fingerprint(path, verify_boundary)?
        }
        LegacyRemovalKind::File if before.file && before.len <= 1_073_741_824 => {
            let mut file = std::fs::File::open(path)
                .map_err(|cause| unsupported().with_source(cause))?
                .take(before.len + 1);
            let mut digest = Sha256::new();
            let mut buffer = [0; 64 * 1024];
            let mut bytes = 0_u64;
            loop {
                let read = file
                    .read(&mut buffer)
                    .map_err(|cause| unsupported().with_source(cause))?;
                if read == 0 {
                    break;
                }
                bytes += read as u64;
                digest.update(&buffer[..read]);
            }
            if bytes != before.len {
                return Err(unsupported());
            }
            digest.finalize().into()
        }
        _ => return Err(unsupported()),
    };
    if EntryStamp::read(path)? != before {
        return Err(unsupported());
    }
    Ok(hash)
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
        let path = legacy_relative_path(path)?;
        if !snapshot.missing(Path::new(&path))?
            || !snapshot.missing(Path::new(&format!("{path}.meta")))?
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

/// Repository paths may use Windows separators. This is a read-only proof over
/// the two project roots VPM uses for legacy assets and old package files.
fn legacy_relative_path(value: &str) -> Result<String, M4Error> {
    let normalized = value.replace('\\', "/");
    let mut parts = normalized.split('/');
    if !matches!(parts.next(), Some("Assets" | "Packages")) || !normalized.contains('/') {
        return Err(unsupported());
    }
    for part in parts {
        if part.is_empty()
            || part.chars().any(char::is_control)
            || part.contains(['<', '>', '"', '|', '?', '*'])
            || crate::archive::validate_segment(part).is_err()
        {
            return Err(unsupported());
        }
    }
    Ok(normalized)
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
    fn windows_legacy_paths_are_checked_in_assets_and_packages_without_deleting() {
        let fixture = Fixture::new();
        let metadata = LegacyMetadata {
            folders: BTreeMap::from([(r"Assets\Old".to_owned(), String::new())]),
            files: BTreeMap::from([(r"Packages\old.tgz".to_owned(), String::new())]),
            ..Default::default()
        };
        verify_absent(&fixture.0, &metadata).expect("clean project needs no cleanup");
        let old = fixture.0.join("Packages/old.tgz");
        std::fs::write(&old, b"keep").expect("old installation fixture");
        assert!(verify_absent(&fixture.0, &metadata).is_err());
        assert_eq!(std::fs::read(&old).expect("retained legacy file"), b"keep");
        for invalid in [
            r"Assets\..\outside",
            r"C:\outside",
            r"\\host\share",
            r"Packages\CON",
            "Assets//old",
            "Packages/old.",
        ] {
            assert!(legacy_relative_path(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn standard_project_relative_paths_and_ambiguous_guids_are_handled_safely() {
        let fixture = Fixture::new();
        std::fs::write(fixture.0.join("ProjectVersion.txt"), b"legacy root file")
            .expect("seed an old file at the project root");
        let mut metadata = LegacyMetadata {
            files: BTreeMap::from([("ProjectVersion.txt".to_owned(), String::new())]),
            ..Default::default()
        };
        let collect = |metadata| {
            collect_removals(
                &fixture.0,
                &BTreeMap::from([("com.example.fixture".to_owned(), metadata)]),
            )
        };
        let plan = collect(metadata.clone()).expect("find the root-relative legacy file");
        assert_eq!(plan[0].path, "ProjectVersion.txt");
        assert!(fixture.0.join("ProjectVersion.txt").is_file());
        for path in ["../outside", "Packages/vpm-manifest.json", "Library/ALCOMD"] {
            assert!(removal_path(path).is_err());
        }
        let guid = "0123456789abcdef0123456789abcdef";
        metadata
            .files
            .insert("Assets/Old.txt".to_owned(), guid.to_owned());
        for name in ["Copy1", "Copy2"] {
            std::fs::write(fixture.0.join(format!("Assets/{name}.txt")), b"asset")
                .expect("seed duplicate-GUID asset");
            std::fs::write(
                fixture.0.join(format!("Assets/{name}.txt.meta")),
                format!("guid: {guid}\n"),
            )
            .expect("write duplicate-GUID metadata");
        }
        assert_eq!(
            collect(metadata)
                .expect_err("duplicate GUID is ambiguous")
                .code(),
            M4ErrorCode::PackageLegacyCleanupRequired
        );
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
