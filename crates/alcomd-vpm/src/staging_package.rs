use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use alcomd_application::{
    M4Error, M4ErrorCode, PackageChangeSet, PackageMutationKind, PackageSourcePin, ResourceKey,
    ResourceLockCoordinator,
};
use sha2::{Digest, Sha256};

use crate::package::validate_extracted_package;
use crate::{
    PROJECT_MANIFEST_LIMIT, PackageCache, extract_archive, materialize_vpm_manifest,
    parse_project_version,
};

/// Frozen evidence for an operation-owned staging Unity project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagingProjectEvidence {
    pub root: PathBuf,
    pub root_identity: Vec<u8>,
    pub initial_vpm_manifest_sha256: [u8; 32],
    pub initial_upm_manifest_sha256: [u8; 32],
    pub final_vpm_manifest_sha256: [u8; 32],
}

/// Bounded non-sensitive evidence returned to the outer Template journal.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagingPackageEvidence {
    pub package_count: u32,
    pub vpm_manifest_sha256: [u8; 32],
    pub upm_manifest_sha256: [u8; 32],
}

/// Prepared frozen package objects. Paths and cache details remain private to `alcomd-vpm`.
#[derive(Debug)]
pub struct PreparedFrozenPackages {
    change_set: PackageChangeSet,
    objects: BTreeMap<String, PathBuf>,
}

/// Narrow M4 package primitive for constructing an unpublished staging project image.
#[derive(Clone)]
pub struct FrozenPackageMaterializer {
    cache: PackageCache,
}

impl FrozenPackageMaterializer {
    pub fn new(cache_root: PathBuf) -> Result<Self, M4Error> {
        Ok(Self {
            cache: PackageCache::new(cache_root)
                .map_err(|_| M4Error::new(M4ErrorCode::PackageCacheCorrupt))?,
        })
    }

    /// Acquires and verifies every frozen archive before any ProjectCreate lock is taken.
    pub async fn prefetch(
        &self,
        change_set: PackageChangeSet,
        source_set: Vec<PackageSourcePin>,
        locks: Arc<ResourceLockCoordinator>,
    ) -> Result<PreparedFrozenPackages, M4Error> {
        change_set.validate_bounds()?;
        let required = validate_frozen_sources(&change_set, &source_set)?;
        let mut objects = BTreeMap::new();
        for source in required {
            let guard = locks
                .acquire(vec![ResourceKey::PackageCache(source.archive_sha256)])
                .await;
            let object = self
                .cache
                .get(source.archive_sha256, &source.artifact_url, false)
                .await
                .map_err(|_| M4Error::new(M4ErrorCode::PackageCacheCorrupt))?;
            drop(guard);
            objects.insert(source.package_id.clone(), object);
        }
        Ok(PreparedFrozenPackages {
            change_set,
            objects,
        })
    }

    /// Materializes the already verified objects into an unpublished staging Unity project.
    pub async fn materialize(
        &self,
        prepared: PreparedFrozenPackages,
        evidence: StagingProjectEvidence,
    ) -> Result<StagingPackageEvidence, M4Error> {
        tokio::task::spawn_blocking(move || materialize_blocking(prepared, evidence))
            .await
            .map_err(|_| M4Error::new(M4ErrorCode::Internal))?
    }

    #[cfg(test)]
    fn object_path(&self, digest: &[u8; 32]) -> PathBuf {
        self.cache.object_path(digest)
    }
}

fn validate_frozen_sources<'a>(
    change_set: &'a PackageChangeSet,
    source_set: &'a [PackageSourcePin],
) -> Result<Vec<&'a PackageSourcePin>, M4Error> {
    let expected = change_set
        .mutations
        .iter()
        .filter_map(|mutation| mutation.source.as_ref())
        .collect::<Vec<_>>();
    if expected.len() != source_set.len() {
        return Err(M4Error::new(M4ErrorCode::PlanStale));
    }
    let mut seen = BTreeSet::new();
    for source in source_set {
        if !seen.insert(source.package_id.as_str()) || !expected.contains(&source) {
            return Err(M4Error::new(M4ErrorCode::PlanStale));
        }
    }
    let mut ordered = source_set.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.archive_sha256
            .cmp(&right.archive_sha256)
            .then_with(|| left.package_id.cmp(&right.package_id))
    });
    Ok(ordered)
}

fn materialize_blocking(
    prepared: PreparedFrozenPackages,
    evidence: StagingProjectEvidence,
) -> Result<StagingPackageEvidence, M4Error> {
    validate_staging_root(&evidence)?;
    let packages = evidence.root.join("Packages");
    let vpm = packages.join("vpm-manifest.json");
    let upm = packages.join("manifest.json");
    let project_version = read_regular_file(
        &evidence.root.join("ProjectSettings/ProjectVersion.txt"),
        crate::PROJECT_VERSION_LIMIT,
    )?;
    parse_project_version(&project_version).map_err(|_| M4Error::new(M4ErrorCode::PlanStale))?;
    let initial_vpm = read_regular_file(&vpm, PROJECT_MANIFEST_LIMIT)?;
    let initial_upm = read_regular_file(&upm, PROJECT_MANIFEST_LIMIT)?;
    if digest(&initial_vpm) != evidence.initial_vpm_manifest_sha256
        || digest(&initial_upm) != evidence.initial_upm_manifest_sha256
        || evidence.final_vpm_manifest_sha256 != prepared.change_set.vpm_manifest_sha256
    {
        return Err(M4Error::new(M4ErrorCode::PlanStale));
    }
    let final_vpm = materialize_vpm_manifest(&initial_vpm, &prepared.change_set)?;
    if digest(&final_vpm) != evidence.final_vpm_manifest_sha256 {
        return Err(M4Error::new(M4ErrorCode::PlanStale));
    }

    let work = evidence
        .root
        .join("Library/ALCOMD/template-package-staging");
    ensure_new_directory_chain(&evidence.root, Path::new("Library/ALCOMD"))?;
    match std::fs::symlink_metadata(&work) {
        Ok(_) => return Err(M4Error::new(M4ErrorCode::RecoveryRequired)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(M4Error::new(M4ErrorCode::RecoveryRequired)),
    }
    std::fs::create_dir(&work).map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
    for mutation in &prepared.change_set.mutations {
        validate_existing_state(&packages, mutation)?;
        let Some(object) = prepared.objects.get(&mutation.package_id) else {
            if mutation.kind == PackageMutationKind::Remove {
                continue;
            }
            return Err(M4Error::new(M4ErrorCode::PlanStale));
        };
        let destination = work.join(&mutation.package_id);
        std::fs::create_dir(&destination)
            .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
        extract_archive(object, &destination)
            .map_err(|_| M4Error::new(M4ErrorCode::PackageArchiveInvalid))?;
        validate_extracted_package(
            &destination,
            &mutation.package_id,
            mutation
                .to_version
                .as_deref()
                .ok_or_else(|| M4Error::new(M4ErrorCode::PlanStale))?,
        )
        .map_err(|_| M4Error::new(M4ErrorCode::PackageManifestInvalid))?;
    }
    alcomd_platform::sync_directory(&work)
        .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;

    for mutation in &prepared.change_set.mutations {
        let target = packages.join(&mutation.package_id);
        if target.exists() {
            std::fs::remove_dir_all(&target)
                .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
        }
        if mutation.kind != PackageMutationKind::Remove {
            std::fs::rename(work.join(&mutation.package_id), &target)
                .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
        }
    }
    let vpm_new = work.join("vpm-manifest.new");
    write_new_file(&vpm_new, &final_vpm)?;
    let vpm_old = work.join("vpm-manifest.old");
    std::fs::rename(&vpm, &vpm_old).map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
    if std::fs::rename(&vpm_new, &vpm).is_err() {
        let _ = std::fs::rename(&vpm_old, &vpm);
        return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
    }
    alcomd_platform::sync_directory(&packages)
        .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
    verify_final_state(&packages, &prepared.change_set, &initial_upm)?;
    std::fs::remove_file(&vpm_old).map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
    std::fs::remove_dir(&work).map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
    Ok(StagingPackageEvidence {
        package_count: u32::try_from(prepared.change_set.mutations.len())
            .map_err(|_| M4Error::new(M4ErrorCode::Internal))?,
        vpm_manifest_sha256: digest(&read_regular_file(&vpm, PROJECT_MANIFEST_LIMIT)?),
        upm_manifest_sha256: digest(&read_regular_file(&upm, PROJECT_MANIFEST_LIMIT)?),
    })
}

fn validate_staging_root(evidence: &StagingProjectEvidence) -> Result<(), M4Error> {
    if !evidence.root.is_absolute() || evidence.root_identity.is_empty() {
        return Err(M4Error::new(M4ErrorCode::PlanStale));
    }
    let metadata = std::fs::symlink_metadata(&evidence.root)
        .map_err(|_| M4Error::new(M4ErrorCode::PlanStale))?;
    if !metadata.is_dir() || is_link_or_reparse(&metadata) {
        return Err(M4Error::new(M4ErrorCode::PlanStale));
    }
    let (_, identity) = alcomd_platform::resolve_directory_identity(&evidence.root)
        .map_err(|_| M4Error::new(M4ErrorCode::PlanStale))?;
    if identity != evidence.root_identity {
        return Err(M4Error::new(M4ErrorCode::PlanStale));
    }
    Ok(())
}

fn validate_existing_state(
    packages: &Path,
    mutation: &alcomd_application::PackageMutation,
) -> Result<(), M4Error> {
    let target = packages.join(&mutation.package_id);
    match mutation.kind {
        PackageMutationKind::Install => match std::fs::symlink_metadata(target) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            _ => Err(M4Error::new(M4ErrorCode::PlanStale)),
        },
        PackageMutationKind::Remove | PackageMutationKind::Replace => {
            let metadata = std::fs::symlink_metadata(&target)
                .map_err(|_| M4Error::new(M4ErrorCode::PlanStale))?;
            if !metadata.is_dir() || is_link_or_reparse(&metadata) {
                return Err(M4Error::new(M4ErrorCode::PlanStale));
            }
            validate_extracted_package(
                &target,
                &mutation.package_id,
                mutation
                    .from_version
                    .as_deref()
                    .ok_or_else(|| M4Error::new(M4ErrorCode::PlanStale))?,
            )
            .map_err(|_| M4Error::new(M4ErrorCode::PlanStale))
        }
    }
}

fn verify_final_state(
    packages: &Path,
    change_set: &PackageChangeSet,
    initial_upm: &[u8],
) -> Result<(), M4Error> {
    if read_regular_file(&packages.join("manifest.json"), PROJECT_MANIFEST_LIMIT)? != initial_upm {
        return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
    }
    let vpm = read_regular_file(&packages.join("vpm-manifest.json"), PROJECT_MANIFEST_LIMIT)?;
    if digest(&vpm) != change_set.vpm_manifest_sha256 {
        return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
    }
    for mutation in &change_set.mutations {
        let target = packages.join(&mutation.package_id);
        if mutation.kind == PackageMutationKind::Remove {
            if target.exists() {
                return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
            }
        } else {
            validate_extracted_package(
                &target,
                &mutation.package_id,
                mutation
                    .to_version
                    .as_deref()
                    .ok_or_else(|| M4Error::new(M4ErrorCode::Internal))?,
            )
            .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
        }
    }
    Ok(())
}

fn ensure_new_directory_chain(root: &Path, relative: &Path) -> Result<(), M4Error> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.is_dir() && !is_link_or_reparse(&metadata) => {}
            Ok(_) => return Err(M4Error::new(M4ErrorCode::RecoveryRequired)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(&current)
                    .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
            }
            Err(_) => return Err(M4Error::new(M4ErrorCode::RecoveryRequired)),
        }
    }
    Ok(())
}

fn read_regular_file(path: &Path, limit: usize) -> Result<Vec<u8>, M4Error> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| M4Error::new(M4ErrorCode::PlanStale))?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) || metadata.len() > limit as u64 {
        return Err(M4Error::new(M4ErrorCode::PlanStale));
    }
    std::fs::read(path).map_err(|_| M4Error::new(M4ErrorCode::PlanStale))
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), M4Error> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    use alcomd_application::{PackageDependencyEdge, PackageMutation};
    use zip::CompressionMethod;
    use zip::write::SimpleFileOptions;

    static NEXT_TEMPORARY_PATH: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        cache_root: PathBuf,
        materializer: FrozenPackageMaterializer,
        source: PackageSourcePin,
        change_set: PackageChangeSet,
        evidence: StagingProjectEvidence,
    }

    fn temporary_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "alcomd-m5-staging-package-{name}-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos(),
            NEXT_TEMPORARY_PATH.fetch_add(1, Ordering::Relaxed)
        ))
    }

    fn package_archive(payload: &[u8]) -> Vec<u8> {
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut cursor);
            let options =
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            writer
                .start_file("package.json", options)
                .expect("package entry");
            writer
                .write_all(br#"{"name":"com.example.fixture","version":"1.2.3"}"#)
                .expect("package manifest");
            writer
                .start_file("Runtime/fixture.txt", options)
                .expect("payload entry");
            writer.write_all(payload).expect("payload");
            writer.finish().expect("archive");
        }
        cursor.into_inner()
    }

    fn fixture(name: &str) -> Fixture {
        fixture_with_payload(name, b"fixture")
    }

    fn fixture_with_payload(name: &str, payload: &[u8]) -> Fixture {
        let root = temporary_path(&format!("{name}-project"));
        let cache_root = temporary_path(&format!("{name}-cache"));
        std::fs::create_dir_all(root.join("ProjectSettings")).expect("project settings");
        std::fs::create_dir_all(root.join("Packages")).expect("packages");
        std::fs::write(
            root.join("ProjectSettings/ProjectVersion.txt"),
            b"m_EditorVersion: 2022.3.22f1\n",
        )
        .expect("project version");
        let initial_vpm = br#"{"dependencies":{},"locked":{}}"#.to_vec();
        let initial_upm = br#"{
    "dependencies": {
        "com.unity.modules.jsonserialize": "1.0.0"
    }
}
"#
        .to_vec();
        std::fs::write(root.join("Packages/vpm-manifest.json"), &initial_vpm)
            .expect("vpm manifest");
        std::fs::write(root.join("Packages/manifest.json"), &initial_upm).expect("upm manifest");

        let archive = package_archive(payload);
        let archive_sha256: [u8; 32] = Sha256::digest(&archive).into();
        let source = PackageSourcePin {
            repository_id: "repository-fixture".to_owned(),
            repository_revision: 7,
            source_identity: "fixture-source".to_owned(),
            manifest_fingerprint: [3; 32],
            package_id: "com.example.fixture".to_owned(),
            version: "1.2.3".to_owned(),
            artifact_url: "https://example.invalid/com.example.fixture.zip".to_owned(),
            archive_sha256,
        };
        let mut change_set = PackageChangeSet {
            format_version: 1,
            mutations: vec![PackageMutation {
                kind: PackageMutationKind::Install,
                package_id: source.package_id.clone(),
                from_version: None,
                to_version: Some(source.version.clone()),
                source: Some(source.clone()),
            }],
            dependency_edges: vec![PackageDependencyEdge {
                from_package_id: "project".to_owned(),
                to_package_id: source.package_id.clone(),
                range: source.version.clone(),
                direct: true,
            }],
            vpm_manifest_sha256: [0; 32],
        };
        let final_vpm = materialize_vpm_manifest(&initial_vpm, &change_set)
            .expect("materialize expected manifest");
        change_set.vpm_manifest_sha256 = digest(&final_vpm);
        let materializer =
            FrozenPackageMaterializer::new(cache_root.clone()).expect("materializer");
        let object = materializer.object_path(&archive_sha256);
        std::fs::create_dir_all(object.parent().expect("cache parent")).expect("cache directory");
        std::fs::write(object, archive).expect("cache object");
        let (_, root_identity) =
            alcomd_platform::resolve_directory_identity(&root).expect("root identity");
        let evidence = StagingProjectEvidence {
            root: root.clone(),
            root_identity,
            initial_vpm_manifest_sha256: digest(&initial_vpm),
            initial_upm_manifest_sha256: digest(&initial_upm),
            final_vpm_manifest_sha256: change_set.vpm_manifest_sha256,
        };
        Fixture {
            root,
            cache_root,
            materializer,
            source,
            change_set,
            evidence,
        }
    }

    fn cleanup(fixture: &Fixture) {
        std::fs::remove_dir_all(&fixture.root).expect("remove project");
        std::fs::remove_dir_all(&fixture.cache_root).expect("remove cache");
    }

    #[tokio::test]
    async fn frozen_install_materializes_without_changing_upm_and_is_reproducible() {
        let first = fixture("materialize-first");
        let second = fixture("materialize-second");
        let first_upm = std::fs::read(first.root.join("Packages/manifest.json")).expect("upm");
        let first_prepared = first
            .materializer
            .prefetch(
                first.change_set.clone(),
                vec![first.source.clone()],
                Arc::new(ResourceLockCoordinator::default()),
            )
            .await
            .expect("prefetch");
        let first_result = first
            .materializer
            .materialize(first_prepared, first.evidence.clone())
            .await
            .expect("materialize");
        assert_eq!(first_result.package_count, 1);
        assert_eq!(
            first_result.vpm_manifest_sha256,
            first.change_set.vpm_manifest_sha256
        );
        assert_eq!(first_result.upm_manifest_sha256, digest(&first_upm));
        assert_eq!(
            std::fs::read(first.root.join("Packages/manifest.json")).expect("final upm"),
            first_upm
        );

        let second_prepared = second
            .materializer
            .prefetch(
                second.change_set.clone(),
                vec![second.source.clone()],
                Arc::new(ResourceLockCoordinator::default()),
            )
            .await
            .expect("second prefetch");
        let second_result = second
            .materializer
            .materialize(second_prepared, second.evidence.clone())
            .await
            .expect("second materialize");
        assert_eq!(first_result, second_result);
        assert_eq!(
            std::fs::read(first.root.join("Packages/vpm-manifest.json")).expect("first vpm"),
            std::fs::read(second.root.join("Packages/vpm-manifest.json")).expect("second vpm")
        );
        assert_eq!(
            std::fs::read(first.root.join("Packages/com.example.fixture/package.json"))
                .expect("first package"),
            std::fs::read(
                second
                    .root
                    .join("Packages/com.example.fixture/package.json")
            )
            .expect("second package")
        );
        cleanup(&first);
        cleanup(&second);
    }

    #[tokio::test]
    async fn source_pin_or_initial_manifest_mismatch_fails_closed() {
        let fixture = fixture("stale");
        let mut mismatched = fixture.source.clone();
        mismatched.repository_revision += 1;
        assert_eq!(
            fixture
                .materializer
                .prefetch(
                    fixture.change_set.clone(),
                    vec![mismatched],
                    Arc::new(ResourceLockCoordinator::default()),
                )
                .await
                .expect_err("source mismatch")
                .code(),
            M4ErrorCode::PlanStale
        );
        let prepared = fixture
            .materializer
            .prefetch(
                fixture.change_set.clone(),
                vec![fixture.source.clone()],
                Arc::new(ResourceLockCoordinator::default()),
            )
            .await
            .expect("prefetch");
        let mut wrong_evidence = fixture.evidence.clone();
        wrong_evidence.initial_upm_manifest_sha256 = [9; 32];
        assert_eq!(
            fixture
                .materializer
                .materialize(prepared, wrong_evidence)
                .await
                .expect_err("manifest mismatch")
                .code(),
            M4ErrorCode::PlanStale
        );
        cleanup(&fixture);
    }

    #[tokio::test]
    async fn corruption_after_prefetch_is_rejected_before_project_mutation() {
        let fixture = fixture("corrupt");
        let prepared = fixture
            .materializer
            .prefetch(
                fixture.change_set.clone(),
                vec![fixture.source.clone()],
                Arc::new(ResourceLockCoordinator::default()),
            )
            .await
            .expect("prefetch");
        std::fs::write(
            fixture
                .materializer
                .object_path(&fixture.source.archive_sha256),
            b"corrupt",
        )
        .expect("corrupt object");
        assert_eq!(
            fixture
                .materializer
                .materialize(prepared, fixture.evidence.clone())
                .await
                .expect_err("corrupt archive")
                .code(),
            M4ErrorCode::PackageArchiveInvalid
        );
        assert!(!fixture.root.join("Packages/com.example.fixture").exists());
        assert_eq!(
            digest(
                &std::fs::read(fixture.root.join("Packages/vpm-manifest.json"))
                    .expect("unchanged vpm")
            ),
            fixture.evidence.initial_vpm_manifest_sha256
        );
        cleanup(&fixture);
    }

    #[tokio::test]
    async fn package_cache_lock_is_shared_by_digest_without_global_serialization() {
        let first = fixture("lock-first");
        let second = fixture_with_payload("lock-second", b"other fixture");
        let locks = Arc::new(ResourceLockCoordinator::default());
        let held = locks
            .acquire(vec![ResourceKey::PackageCache(first.source.archive_sha256)])
            .await;
        let waiting = {
            let materializer = first.materializer.clone();
            let change_set = first.change_set.clone();
            let source = first.source.clone();
            let locks = Arc::clone(&locks);
            tokio::spawn(
                async move { materializer.prefetch(change_set, vec![source], locks).await },
            )
        };
        tokio::task::yield_now().await;
        assert!(!waiting.is_finished());

        let other = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            second.materializer.prefetch(
                second.change_set.clone(),
                vec![second.source.clone()],
                Arc::clone(&locks),
            ),
        )
        .await
        .expect("different digest must not block")
        .expect("different digest prefetch");
        drop(other);
        drop(held);
        waiting
            .await
            .expect("same digest waiter")
            .expect("prefetch after release");
        cleanup(&first);
        cleanup(&second);
    }
}
