use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use alcomd_application::{
    M4Error, M4ErrorCode, PackageChangeSet, PackageMutationKind, PackageSourcePin, ResourceKey,
    ResourceLockCoordinator,
};
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::package::validate_extracted_package;
use crate::{
    PROJECT_MANIFEST_LIMIT, PackageCache, extract_archive, materialize_vpm_manifest,
    parse_project_version,
};

pub(crate) use crate::cache::PACKAGE_DOWNLOAD_CONCURRENCY;
const PACKAGE_STAGING_CONCURRENCY: usize = 32;
static PACKAGE_STAGING_SLOTS: Semaphore = Semaphore::const_new(PACKAGE_STAGING_CONCURRENCY);

pub(crate) async fn run_ordered_jobs<J, T, F>(
    jobs: Vec<J>,
    limit: usize,
    prepare: impl Fn(J) -> F,
) -> Result<Vec<T>, M4Error>
where
    T: Send + 'static,
    F: Future<Output = Result<T, M4Error>> + Send + 'static,
{
    let mut remaining = jobs.into_iter().enumerate();
    let mut workers = JoinSet::new();
    let mut results = BTreeMap::new();
    let mut failed = false;
    let mut join_error = None;
    loop {
        while !failed && workers.len() < limit {
            let Some((index, job)) = remaining.next() else {
                break;
            };
            let future = prepare(job);
            workers.spawn(async move { (index, future.await) });
        }
        match workers.join_next().await {
            Some(Ok((index, result))) => {
                failed |= result.is_err();
                results.insert(index, result);
            }
            Some(Err(cause)) => {
                failed = true;
                join_error = Some(cause);
            }
            None => break,
        }
    }
    if let Some(cause) = join_error {
        return Err(M4Error::new(M4ErrorCode::Internal).with_source(cause));
    }
    // Blocking extraction may still be writing when another job fails. Join
    // every admitted job before returning to transaction cleanup.
    results.into_values().collect()
}

pub(crate) async fn fetch_package_objects(
    cache: &PackageCache,
    locks: Option<&Arc<ResourceLockCoordinator>>,
    sources: Vec<PackageSourcePin>,
) -> Result<BTreeMap<String, PathBuf>, M4Error> {
    let objects = run_ordered_jobs(sources, PACKAGE_DOWNLOAD_CONCURRENCY, |source| {
        let cache = cache.clone();
        let locks = locks.cloned();
        async move {
            let digest = source.archive_sha256();
            let _guard = match locks {
                Some(locks) => Some(locks.acquire(vec![ResourceKey::PackageCache(digest)]).await),
                None => None,
            };
            let archive = cache
                .get_for_apply(
                    digest,
                    source.artifact_url().unwrap_or(""),
                    source.is_user_package(),
                    match &source {
                        PackageSourcePin::Repository(pin) => crate::cache::ArchiveHeaders {
                            repository: pin.credential_reference.as_deref(),
                            package: pin.download_headers.as_ref().map(|headers| {
                                crate::cache::PackageHeaderLookup {
                                    source_identity: &pin.source_identity,
                                    local_path: headers.local_path.as_deref(),
                                    package_id: &pin.package_id,
                                    version: &pin.version,
                                    fingerprint: &pin.manifest_fingerprint,
                                }
                            }),
                        },
                        PackageSourcePin::UserPackage(_) => crate::cache::ArchiveHeaders::default(),
                    },
                )
                .await
                .map_err(crate::engine::map_cache_error)?;
            Ok((source.package_id().to_owned(), archive))
        }
    })
    .await?;
    Ok(objects.into_iter().collect())
}

pub(crate) async fn extract_package_objects(
    stage: &Path,
    change_set: &PackageChangeSet,
    objects: &BTreeMap<String, PathBuf>,
) -> Result<(), M4Error> {
    let jobs = change_set
        .mutations
        .iter()
        .filter(|mutation| mutation.source.is_some())
        .map(|mutation| {
            let archive = objects
                .get(&mutation.package_id)
                .ok_or_else(|| M4Error::new(M4ErrorCode::PlanStale))?;
            let version = mutation
                .to_version
                .clone()
                .ok_or_else(|| M4Error::new(M4ErrorCode::PlanStale))?;
            Ok((mutation.package_id.clone(), version, archive.clone()))
        })
        .collect::<Result<Vec<_>, M4Error>>()?;
    run_ordered_jobs(
        jobs,
        PACKAGE_STAGING_CONCURRENCY,
        |(package, version, archive)| {
            let destination = stage.join(&package);
            async move {
                let slot = PACKAGE_STAGING_SLOTS
                    .acquire()
                    .await
                    .map_err(|cause| M4Error::new(M4ErrorCode::Internal).with_source(cause))?;
                tokio::task::spawn_blocking(move || {
                    let _slot = slot;
                    std::fs::create_dir(&destination).map_err(|cause| {
                        M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause)
                    })?;
                    extract_archive(&archive, &destination)
                        .map_err(crate::engine::map_archive_error)?;
                    validate_extracted_package(&destination, &package, &version).map_err(|cause| {
                        M4Error::new(M4ErrorCode::PackageManifestInvalid).with_source(cause)
                    })
                })
                .await
                .map_err(|cause| M4Error::new(M4ErrorCode::Internal).with_source(cause))?
            }
        },
    )
    .await?;
    Ok(())
}

/// Frozen evidence for an operation-owned staging Unity project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagingProjectEvidence {
    pub root: PathBuf,
    pub root_identity: Vec<u8>,
    pub initial_vpm_manifest_sha256: [u8; 32],
    pub initial_upm_manifest_sha256: [u8; 32],
    pub final_vpm_manifest_sha256: [u8; 32],
}

/// Bounded non-sensitive evidence returned to the project creation journal.
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
    pub(crate) fn cache(&self) -> &PackageCache {
        &self.cache
    }

    #[must_use]
    pub fn new(cache: PackageCache) -> Self {
        Self { cache }
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
        let objects = fetch_package_objects(
            &self.cache,
            Some(&locks),
            required.into_iter().cloned().collect(),
        )
        .await?;
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
        self.materialize_with_ranges(prepared, evidence, false)
            .await
    }

    pub(crate) async fn materialize_template(
        &self,
        prepared: PreparedFrozenPackages,
        evidence: StagingProjectEvidence,
    ) -> Result<StagingPackageEvidence, M4Error> {
        self.materialize_with_ranges(prepared, evidence, true).await
    }

    async fn materialize_with_ranges(
        &self,
        prepared: PreparedFrozenPackages,
        evidence: StagingProjectEvidence,
        preserve_input_ranges: bool,
    ) -> Result<StagingPackageEvidence, M4Error> {
        let (prepared, work) = tokio::task::spawn_blocking(move || {
            prepare_materialization(prepared, evidence, preserve_input_ranges)
        })
        .await
        .map_err(|cause| M4Error::new(M4ErrorCode::Internal).with_source(cause))??;
        extract_package_objects(&work.stage, &prepared.change_set, &prepared.objects).await?;
        tokio::task::spawn_blocking(move || finish_materialization(prepared.change_set, work))
            .await
            .map_err(|cause| M4Error::new(M4ErrorCode::Internal).with_source(cause))?
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
        if !seen.insert(source.package_id()) || !expected.contains(&source) {
            return Err(M4Error::new(M4ErrorCode::PlanStale));
        }
    }
    let mut ordered = source_set.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.archive_sha256()
            .cmp(&right.archive_sha256())
            .then_with(|| left.package_id().cmp(right.package_id()))
    });
    Ok(ordered)
}

struct PackageMaterialization {
    stage: PathBuf,
    packages: PathBuf,
    initial_upm: Vec<u8>,
    final_vpm: Vec<u8>,
}

fn prepare_materialization(
    prepared: PreparedFrozenPackages,
    evidence: StagingProjectEvidence,
    preserve_input_ranges: bool,
) -> Result<(PreparedFrozenPackages, PackageMaterialization), M4Error> {
    validate_staging_root(&evidence)?;
    let packages = evidence.root.join("Packages");
    let vpm = packages.join("vpm-manifest.json");
    let upm = packages.join("manifest.json");
    let project_version = read_regular_file(
        &evidence.root.join("ProjectSettings/ProjectVersion.txt"),
        crate::PROJECT_VERSION_LIMIT,
    )?;
    parse_project_version(&project_version)
        .map_err(|cause| M4Error::new(M4ErrorCode::PlanStale).with_source(cause))?;
    let initial_vpm = read_regular_file(&vpm, PROJECT_MANIFEST_LIMIT)?;
    let initial_upm = read_regular_file(&upm, PROJECT_MANIFEST_LIMIT)?;
    if digest(&initial_vpm) != evidence.initial_vpm_manifest_sha256
        || digest(&initial_upm) != evidence.initial_upm_manifest_sha256
        || evidence.final_vpm_manifest_sha256 != prepared.change_set.vpm_manifest_sha256
    {
        return Err(M4Error::new(M4ErrorCode::PlanStale));
    }
    let final_vpm = if preserve_input_ranges {
        crate::plan::materialize_template_manifest(&initial_vpm, &prepared.change_set)?
    } else {
        materialize_vpm_manifest(&initial_vpm, &prepared.change_set)?
    };
    if digest(&final_vpm) != evidence.final_vpm_manifest_sha256 {
        return Err(M4Error::new(M4ErrorCode::PlanStale));
    }

    let work = evidence
        .root
        .join("Library/ALCOMD/project-create-package-staging");
    ensure_new_directory_chain(&evidence.root, Path::new("Library/ALCOMD"))?;
    match std::fs::symlink_metadata(&work) {
        Ok(_) => return Err(M4Error::new(M4ErrorCode::RecoveryRequired)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(M4Error::new(M4ErrorCode::RecoveryRequired)),
    }
    std::fs::create_dir(&work)
        .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
    for mutation in &prepared.change_set.mutations {
        validate_existing_state(&packages, mutation)?;
    }
    Ok((
        prepared,
        PackageMaterialization {
            stage: work,
            packages,
            initial_upm,
            final_vpm,
        },
    ))
}

fn finish_materialization(
    change_set: PackageChangeSet,
    materialization: PackageMaterialization,
) -> Result<StagingPackageEvidence, M4Error> {
    let PackageMaterialization {
        stage: work,
        packages,
        initial_upm,
        final_vpm,
    } = materialization;
    let vpm = packages.join("vpm-manifest.json");
    let upm = packages.join("manifest.json");
    alcomd_platform::sync_directory(&work)
        .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;

    for mutation in &change_set.mutations {
        let target = packages.join(&mutation.package_id);
        if target.exists() {
            std::fs::remove_dir_all(&target)
                .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
        }
        if mutation.kind != PackageMutationKind::Remove {
            std::fs::rename(work.join(&mutation.package_id), &target)
                .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
        }
    }
    let vpm_new = work.join("vpm-manifest.new");
    write_new_file(&vpm_new, &final_vpm)?;
    let vpm_old = work.join("vpm-manifest.old");
    std::fs::rename(&vpm, &vpm_old)
        .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
    if std::fs::rename(&vpm_new, &vpm).is_err() {
        let _ = std::fs::rename(&vpm_old, &vpm);
        return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
    }
    alcomd_platform::sync_directory(&packages)
        .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
    verify_final_state(&packages, &change_set, &initial_upm)?;
    std::fs::remove_file(&vpm_old)
        .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
    std::fs::remove_dir(&work)
        .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
    Ok(StagingPackageEvidence {
        package_count: u32::try_from(change_set.mutations.len())
            .map_err(|cause| M4Error::new(M4ErrorCode::Internal).with_source(cause))?,
        vpm_manifest_sha256: digest(&read_regular_file(&vpm, PROJECT_MANIFEST_LIMIT)?),
        upm_manifest_sha256: digest(&read_regular_file(&upm, PROJECT_MANIFEST_LIMIT)?),
    })
}

fn validate_staging_root(evidence: &StagingProjectEvidence) -> Result<(), M4Error> {
    if !evidence.root.is_absolute() || evidence.root_identity.is_empty() {
        return Err(M4Error::new(M4ErrorCode::PlanStale));
    }
    let metadata = std::fs::symlink_metadata(&evidence.root)
        .map_err(|cause| M4Error::new(M4ErrorCode::PlanStale).with_source(cause))?;
    if !metadata.is_dir() || is_link_or_reparse(&metadata) {
        return Err(M4Error::new(M4ErrorCode::PlanStale));
    }
    let (_, identity) = alcomd_platform::resolve_directory_identity(&evidence.root)
        .map_err(|cause| M4Error::new(M4ErrorCode::PlanStale).with_source(cause))?;
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
            let metadata = std::fs::symlink_metadata(&target).map_err(|cause| {
                M4Error::new(M4ErrorCode::PlanStale)
                    .with_source(cause)
                    .with_path(packages.display().to_string())
            })?;
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
            .map_err(|cause| {
                M4Error::new(M4ErrorCode::PlanStale)
                    .with_source(cause)
                    .with_path(packages.display().to_string())
            })
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
            .map_err(|cause| {
                M4Error::new(M4ErrorCode::RecoveryRequired)
                    .with_source(cause)
                    .with_path(packages.display().to_string())
            })?;
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
                std::fs::create_dir(&current).map_err(|cause| {
                    M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause)
                })?;
            }
            Err(_) => return Err(M4Error::new(M4ErrorCode::RecoveryRequired)),
        }
    }
    Ok(())
}

fn read_regular_file(path: &Path, limit: usize) -> Result<Vec<u8>, M4Error> {
    let metadata = std::fs::symlink_metadata(path).map_err(|cause| {
        M4Error::new(M4ErrorCode::PlanStale)
            .with_source(cause)
            .with_path(path.display().to_string())
    })?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) || metadata.len() > limit as u64 {
        return Err(M4Error::new(M4ErrorCode::PlanStale));
    }
    std::fs::read(path).map_err(|cause| {
        M4Error::new(M4ErrorCode::PlanStale)
            .with_source(cause)
            .with_path(path.display().to_string())
    })
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), M4Error> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|cause| {
            M4Error::new(M4ErrorCode::RecoveryRequired)
                .with_source(cause)
                .with_path(path.display().to_string())
        })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|cause| {
            M4Error::new(M4ErrorCode::RecoveryRequired)
                .with_source(cause)
                .with_path(path.display().to_string())
        })
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

    use alcomd_application::{PackageDependencyEdge, PackageMutation, RepositoryPackageSourcePin};
    use zip::CompressionMethod;
    use zip::write::SimpleFileOptions;

    static NEXT_TEMPORARY_PATH: AtomicU64 = AtomicU64::new(0);

    #[tokio::test]
    async fn package_jobs_are_bounded_and_return_in_input_order() {
        let gates = Arc::new([Semaphore::new(0), Semaphore::new(0), Semaphore::new(0)]);
        let (started, mut observed) = tokio::sync::mpsc::unbounded_channel();
        let running = tokio::spawn({
            let gates = Arc::clone(&gates);
            async move {
                run_ordered_jobs(vec![0, 1, 2], 2, |job| {
                    let gates = Arc::clone(&gates);
                    let started = started.clone();
                    async move {
                        started.send(job).expect("record start");
                        let _gate = gates[job].acquire().await.expect("release job");
                        Ok(job)
                    }
                })
                .await
            }
        });
        let mut first = [next_job(&mut observed).await, next_job(&mut observed).await];
        first.sort_unstable();
        assert_eq!(first, [0, 1]);
        assert!(observed.try_recv().is_err());
        gates[1].add_permits(1);
        assert_eq!(next_job(&mut observed).await, 2);
        gates[2].add_permits(1);
        gates[0].add_permits(1);
        assert_eq!(running.await.expect("join").expect("results"), [0, 1, 2]);
    }

    #[tokio::test]
    async fn package_failure_drains_started_jobs_before_returning() {
        let gates = Arc::new([Semaphore::new(0), Semaphore::new(0), Semaphore::new(0)]);
        let (started, mut observed) = tokio::sync::mpsc::unbounded_channel();
        let mut running = tokio::spawn({
            let gates = Arc::clone(&gates);
            async move {
                run_ordered_jobs(vec![0, 1, 2], 2, |job| {
                    let gates = Arc::clone(&gates);
                    let started = started.clone();
                    async move {
                        started.send(job).expect("record start");
                        let _gate = gates[job].acquire().await.expect("release job");
                        if job == 0 {
                            Err(M4Error::new(M4ErrorCode::PackageCacheCorrupt))
                        } else {
                            Ok(job)
                        }
                    }
                })
                .await
            }
        });
        let mut first = [next_job(&mut observed).await, next_job(&mut observed).await];
        first.sort_unstable();
        assert_eq!(first, [0, 1]);
        gates[0].add_permits(1);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(50), &mut running)
                .await
                .is_err()
        );
        assert!(observed.try_recv().is_err());
        gates[1].add_permits(1);
        gates[2].add_permits(1);
        assert_eq!(
            running
                .await
                .expect("join")
                .expect_err("package failure")
                .code(),
            M4ErrorCode::PackageCacheCorrupt
        );
        assert!(observed.try_recv().is_err());
    }

    async fn next_job(observed: &mut tokio::sync::mpsc::UnboundedReceiver<usize>) -> usize {
        tokio::time::timeout(std::time::Duration::from_secs(2), observed.recv())
            .await
            .expect("job started before deadline")
            .expect("job index")
    }

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
        let source = PackageSourcePin::Repository(RepositoryPackageSourcePin {
            credential_reference: None,
            download_headers: None,
            repository_id: "repository-fixture".to_owned(),
            repository_revision: 7,
            source_identity: "fixture-source".to_owned(),
            manifest_fingerprint: [3; 32],
            package_id: "com.example.fixture".to_owned(),
            version: "1.2.3".to_owned(),
            artifact_url: "https://example.invalid/com.example.fixture.zip".to_owned(),
            archive_sha256,
        });
        let mut change_set = PackageChangeSet {
            unity_compatibility_overrides: Vec::new(),
            format_version: 1,
            legacy_removals: Vec::new(),
            legacy_packages: Vec::new(),
            mutations: vec![PackageMutation {
                kind: PackageMutationKind::Install,
                package_id: source.package_id().to_owned(),
                from_version: None,
                to_version: Some(source.version().to_owned()),
                source: Some(source.clone()),
            }],
            dependency_edges: vec![PackageDependencyEdge {
                from_package_id: "project".to_owned(),
                to_package_id: source.package_id().to_owned(),
                range: source.version().to_owned(),
                direct: true,
            }],
            vpm_manifest_sha256: [0; 32],
        };
        let final_vpm = materialize_vpm_manifest(&initial_vpm, &change_set)
            .expect("materialize expected manifest");
        change_set.vpm_manifest_sha256 = digest(&final_vpm);
        let materializer =
            FrozenPackageMaterializer::new(PackageCache::new(cache_root.clone()).expect("cache"));
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
        let PackageSourcePin::Repository(value) = &mut mismatched else {
            panic!("repository source");
        };
        value.repository_revision += 1;
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
                .object_path(&fixture.source.archive_sha256()),
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
            .acquire(vec![ResourceKey::PackageCache(
                first.source.archive_sha256(),
            )])
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
