use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use alcomd_application::{
    FilesystemJournalEntry, FilesystemPhase, JournalState, M3ReadAdapter, M4Error, M4ErrorCode,
    M4PackageAdapter, M4Store, OperationId, PackageApplyCompletion, PackageMutationKind,
    PackagePlanDraft, PackagePlanRecord, PackagePlanRequest, PlanAction, ProjectDiscoveryMode,
    ProjectRecord, ResolverCatalog, ResourceKey, ResourceLockCoordinator, StateStore,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::package::validate_extracted_package;
use crate::{
    PackageCache, ResolveError, ResolveRequest, VpmReader, build_remove_plan,
    build_resolution_plan, candidates_from_catalog, extract_archive, inspect_package_project,
    materialize_vpm_manifest, resolve_packages,
};

#[derive(Clone)]
pub struct PackageEngine<S: M4Store + StateStore> {
    store: S,
    reader: VpmReader,
    cache: PackageCache,
}

impl<S: M4Store + StateStore> PackageEngine<S> {
    pub fn new(store: S, reader: VpmReader, cache_root: PathBuf) -> Result<Self, M4Error> {
        let cache = PackageCache::new(cache_root)
            .map_err(|_| M4Error::new(M4ErrorCode::PackageCacheCorrupt))?;
        Ok(Self {
            store,
            reader,
            cache,
        })
    }
}

impl<S: M4Store + StateStore> M4PackageAdapter for PackageEngine<S> {
    async fn prepare_plan(
        &self,
        project: ProjectRecord,
        catalog: ResolverCatalog,
        request: PackagePlanRequest,
    ) -> Result<PackagePlanDraft, M4Error> {
        let snapshot = tokio::task::spawn_blocking({
            let project = project.clone();
            move || inspect_package_project(&project)
        })
        .await
        .map_err(|_| M4Error::new(M4ErrorCode::Internal))??;
        if request.action == PlanAction::Remove {
            let package_id = request
                .package_id
                .as_deref()
                .ok_or_else(|| M4Error::new(M4ErrorCode::InvalidInput))?;
            let candidates = candidates_from_catalog(&catalog).map_err(map_resolve_error)?;
            let unity_version = parse_unity_editor_version(&project.observation.unity_version)?;
            let remaining_requests = project
                .observation
                .direct_dependencies
                .iter()
                .filter(|dependency| dependency.package_id != package_id)
                .map(|dependency| ResolveRequest {
                    package_id: dependency.package_id.clone(),
                    range: dependency.value.clone(),
                    repository_id: None,
                    include_prerelease: request.include_prerelease,
                    unity_version,
                })
                .collect::<Vec<_>>();
            let remaining = if remaining_requests.is_empty() {
                None
            } else {
                Some(
                    resolve_packages(&candidates, &remaining_requests)
                        .map_err(map_resolve_error)?,
                )
            };
            return build_remove_plan(&snapshot, package_id, remaining.as_ref());
        }
        let candidates = candidates_from_catalog(&catalog).map_err(map_resolve_error)?;
        let unity_version = parse_unity_editor_version(&project.observation.unity_version)?;
        let requests = resolution_requests(&project, &request, unity_version)?;
        let resolution = resolve_packages(&candidates, &requests).map_err(map_resolve_error)?;
        validate_action_direction(&project, &request, &resolution)?;
        build_resolution_plan(&snapshot, request.action, &resolution)
    }

    async fn revalidate_plan(
        &self,
        project: ProjectRecord,
        catalog: ResolverCatalog,
        plan: PackagePlanRecord,
    ) -> Result<(), M4Error> {
        revalidate(&project, &catalog, &plan).await
    }

    async fn execute_plan(
        &self,
        operation_id: OperationId,
        project: ProjectRecord,
        plan: PackagePlanRecord,
        locks: Arc<ResourceLockCoordinator>,
    ) -> Result<PackageApplyCompletion, M4Error> {
        self.execute(operation_id, project, plan, locks).await
    }
}

impl<S: M4Store + StateStore> PackageEngine<S> {
    async fn execute(
        &self,
        operation_id: OperationId,
        project: ProjectRecord,
        plan: PackagePlanRecord,
        locks: Arc<ResourceLockCoordinator>,
    ) -> Result<PackageApplyCompletion, M4Error> {
        let root = PathBuf::from(&project.observation.root_path);
        let (_, actual_identity) = alcomd_platform::resolve_directory_identity(&root)
            .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
        if actual_identity != project.observation.path_identity_key {
            return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
        }
        let project_identity_key = project.observation.path_identity_key.clone();
        let transaction_root = prepare_transaction_root(&root, operation_id)?;
        if let Some(completion) = self
            .recover_attempts(operation_id, &project, &plan, &transaction_root, &locks)
            .await?
        {
            return Ok(completion);
        }

        let mut next_step = self
            .store
            .next_filesystem_journal_step(operation_id)
            .await?;
        let attempt = transaction_root.join(format!("attempt-{next_step:020}"));
        std::fs::create_dir(&attempt).map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
        let staging = attempt.join("staging");
        let backup = attempt.join("backup");
        std::fs::create_dir(&staging)
            .and_then(|()| std::fs::create_dir(&backup))
            .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
        sync_directories(&[&attempt, &transaction_root])?;

        let mut archives = BTreeMap::<String, PathBuf>::new();
        for mutation in &plan.change_set.mutations {
            let Some(source) = &mutation.source else {
                continue;
            };
            let _cache_guard = locks
                .acquire(vec![ResourceKey::PackageCache(source.archive_sha256)])
                .await;
            let archive = self
                .cache
                .get(source.archive_sha256, &source.artifact_url, false)
                .await
                .map_err(map_cache_error)?;
            archives.insert(mutation.package_id.clone(), archive);
        }
        append_phase(
            &self.store,
            &plan,
            operation_id,
            &mut next_step,
            FilesystemPhase::ArchiveReady,
            JournalState::Completed,
            &project_identity_key,
            format!("{{\"objects\":{}}}", archives.len()),
        )
        .await
        .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
        write_marker(
            &attempt,
            &AttemptMarker::new(
                &plan,
                FilesystemPhase::ArchiveReady,
                JournalState::Completed,
            ),
            next_step - 1,
        )?;

        for (package_id, archive) in archives {
            let destination = staging.join(&package_id);
            std::fs::create_dir(&destination)
                .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
            let extraction_destination = destination.clone();
            tokio::task::spawn_blocking(move || extract_archive(&archive, &extraction_destination))
                .await
                .map_err(|_| M4Error::new(M4ErrorCode::Internal))?
                .map_err(map_archive_error)?;
            let expected_version = plan
                .change_set
                .mutations
                .iter()
                .find(|mutation| mutation.package_id == package_id)
                .and_then(|mutation| mutation.to_version.as_deref())
                .ok_or_else(|| M4Error::new(M4ErrorCode::Internal))?;
            validate_extracted_package(&destination, &package_id, expected_version)
                .map_err(|_| M4Error::new(M4ErrorCode::PackageManifestInvalid))?;
        }
        append_phase(
            &self.store,
            &plan,
            operation_id,
            &mut next_step,
            FilesystemPhase::Extracted,
            JournalState::Completed,
            &project_identity_key,
            format!(
                "{{\"packages\":{}}}",
                plan.change_set
                    .mutations
                    .iter()
                    .filter(|mutation| mutation.source.is_some())
                    .count()
            ),
        )
        .await
        .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
        write_marker(
            &attempt,
            &AttemptMarker::new(&plan, FilesystemPhase::Extracted, JournalState::Completed),
            next_step - 1,
        )?;

        if self
            .store
            .cancellation_requested(operation_id)
            .await
            .map_err(|error| M4Error::from(error.kind()))?
        {
            return Err(M4Error::new(M4ErrorCode::OperationCancelled));
        }

        let _project_guard = locks
            .acquire(vec![
                ResourceKey::Project(plan.project_id),
                ResourceKey::Operation(operation_id),
            ])
            .await;
        let catalog = self.store.resolver_catalog(plan.owner.clone()).await?;
        revalidate(&project, &catalog, &plan).await?;
        if self
            .store
            .cancellation_requested(operation_id)
            .await
            .map_err(|error| M4Error::from(error.kind()))?
        {
            return Err(M4Error::new(M4ErrorCode::OperationCancelled));
        }
        let packages_root = root.join("Packages");
        let manifest_path = packages_root.join("vpm-manifest.json");
        let upm_path = packages_root.join("manifest.json");
        let upm_before = hash_file(&upm_path)?;
        let current_manifest = read_regular_file(&manifest_path, crate::PROJECT_MANIFEST_LIMIT)?;
        let new_manifest = materialize_vpm_manifest(&current_manifest, &plan.change_set)?;
        if Sha256::digest(&new_manifest).as_slice() != plan.change_set.vpm_manifest_sha256 {
            return Err(M4Error::with_subreason(
                M4ErrorCode::PlanStale,
                "project_snapshot_changed",
            ));
        }
        write_new_file(&attempt.join("vpm-manifest.new"), &new_manifest)?;
        write_new_file(&attempt.join("vpm-manifest.old"), &current_manifest)?;
        write_new_file(
            &attempt.join("upm-manifest.sha256"),
            hex(&upm_before).as_bytes(),
        )?;
        append_phase(
            &self.store,
            &plan,
            operation_id,
            &mut next_step,
            FilesystemPhase::Prepared,
            JournalState::Completed,
            &project_identity_key,
            "{}".to_owned(),
        )
        .await
        .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
        write_marker(
            &attempt,
            &AttemptMarker::new(&plan, FilesystemPhase::Prepared, JournalState::Completed),
            next_step - 1,
        )?;

        append_phase(
            &self.store,
            &plan,
            operation_id,
            &mut next_step,
            FilesystemPhase::PackagesReplaced,
            JournalState::Intent,
            &project_identity_key,
            "{}".to_owned(),
        )
        .await?;
        write_marker(
            &attempt,
            &AttemptMarker::new(
                &plan,
                FilesystemPhase::PackagesReplaced,
                JournalState::Intent,
            ),
            next_step - 1,
        )?;
        if let Err(error) = replace_packages(&packages_root, &staging, &backup, &plan) {
            rollback_attempt(&packages_root, &manifest_path, &attempt, &plan)?;
            append_phase(
                &self.store,
                &plan,
                operation_id,
                &mut next_step,
                FilesystemPhase::RolledBack,
                JournalState::Completed,
                &project_identity_key,
                "{}".to_owned(),
            )
            .await
            .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
            return Err(error);
        }
        append_phase(
            &self.store,
            &plan,
            operation_id,
            &mut next_step,
            FilesystemPhase::PackagesReplaced,
            JournalState::Completed,
            &project_identity_key,
            "{}".to_owned(),
        )
        .await
        .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
        write_marker(
            &attempt,
            &AttemptMarker::new(
                &plan,
                FilesystemPhase::PackagesReplaced,
                JournalState::Completed,
            ),
            next_step - 1,
        )?;

        append_phase(
            &self.store,
            &plan,
            operation_id,
            &mut next_step,
            FilesystemPhase::VpmManifestCommitted,
            JournalState::Intent,
            &project_identity_key,
            "{}".to_owned(),
        )
        .await
        .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
        write_marker(
            &attempt,
            &AttemptMarker::new(
                &plan,
                FilesystemPhase::VpmManifestCommitted,
                JournalState::Intent,
            ),
            next_step - 1,
        )?;
        if let Err(error) = commit_manifest(&manifest_path, &attempt) {
            rollback_attempt(&packages_root, &manifest_path, &attempt, &plan)?;
            append_phase(
                &self.store,
                &plan,
                operation_id,
                &mut next_step,
                FilesystemPhase::RolledBack,
                JournalState::Completed,
                &project_identity_key,
                "{}".to_owned(),
            )
            .await
            .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
            return Err(error);
        }
        append_phase(
            &self.store,
            &plan,
            operation_id,
            &mut next_step,
            FilesystemPhase::VpmManifestCommitted,
            JournalState::Completed,
            &project_identity_key,
            "{}".to_owned(),
        )
        .await
        .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
        write_marker(
            &attempt,
            &AttemptMarker::new(
                &plan,
                FilesystemPhase::VpmManifestCommitted,
                JournalState::Completed,
            ),
            next_step - 1,
        )?;

        if hash_file(&upm_path)? != upm_before
            || hash_file(&manifest_path)? != plan.change_set.vpm_manifest_sha256
        {
            rollback_attempt(&packages_root, &manifest_path, &attempt, &plan)?;
            append_phase(
                &self.store,
                &plan,
                operation_id,
                &mut next_step,
                FilesystemPhase::RolledBack,
                JournalState::Completed,
                &project_identity_key,
                "{}".to_owned(),
            )
            .await
            .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
            return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply));
        }
        verify_final_state(&root, &attempt, &plan)?;
        append_phase(
            &self.store,
            &plan,
            operation_id,
            &mut next_step,
            FilesystemPhase::FilesystemCommitted,
            JournalState::Completed,
            &project_identity_key,
            "{}".to_owned(),
        )
        .await
        .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
        write_marker(
            &attempt,
            &AttemptMarker::new(
                &plan,
                FilesystemPhase::FilesystemCommitted,
                JournalState::Completed,
            ),
            next_step - 1,
        )?;
        append_phase(
            &self.store,
            &plan,
            operation_id,
            &mut next_step,
            FilesystemPhase::StateCommitted,
            JournalState::Intent,
            &project_identity_key,
            "{}".to_owned(),
        )
        .await
        .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
        self.completion(&root, &plan).await
    }

    async fn recover_attempts(
        &self,
        operation_id: OperationId,
        project: &ProjectRecord,
        plan: &PackagePlanRecord,
        transaction_root: &Path,
        locks: &Arc<ResourceLockCoordinator>,
    ) -> Result<Option<PackageApplyCompletion>, M4Error> {
        let mut attempts = std::fs::read_dir(transaction_root)
            .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("attempt-"))
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        attempts.sort();
        for attempt in attempts {
            let marker = match read_marker(&attempt) {
                Ok(marker) => marker,
                Err(_) => return Err(M4Error::new(M4ErrorCode::RecoveryRequired)),
            };
            if marker.plan_id != plan.plan_id.to_string()
                || marker.change_set_fingerprint != hex(&plan.change_set_fingerprint)
            {
                return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
            }
            if marker.phase == FilesystemPhase::FilesystemCommitted {
                let _guard = locks
                    .acquire(vec![
                        ResourceKey::Project(plan.project_id),
                        ResourceKey::Operation(operation_id),
                    ])
                    .await;
                verify_final_state(Path::new(&project.observation.root_path), &attempt, plan)?;
                return self
                    .completion(Path::new(&project.observation.root_path), plan)
                    .await
                    .map(Some);
            }
            if matches!(
                marker.phase,
                FilesystemPhase::PackagesReplaced | FilesystemPhase::VpmManifestCommitted
            ) {
                let _guard = locks
                    .acquire(vec![
                        ResourceKey::Project(plan.project_id),
                        ResourceKey::Operation(operation_id),
                    ])
                    .await;
                let root = Path::new(&project.observation.root_path);
                rollback_attempt(
                    &root.join("Packages"),
                    &root.join("Packages/vpm-manifest.json"),
                    &attempt,
                    plan,
                )?;
                write_marker(
                    &attempt,
                    &AttemptMarker::new(plan, FilesystemPhase::RolledBack, JournalState::Completed),
                    u64::MAX,
                )?;
            }
        }
        Ok(None)
    }

    async fn completion(
        &self,
        root: &Path,
        plan: &PackagePlanRecord,
    ) -> Result<PackageApplyCompletion, M4Error> {
        let observation = self
            .reader
            .inspect_project(
                root.to_string_lossy().into_owned(),
                ProjectDiscoveryMode::ExactRoot,
            )
            .await
            .map_err(|_| M4Error::new(M4ErrorCode::ProjectChangedDuringApply))?;
        Ok(PackageApplyCompletion {
            project_observation: observation,
            result_json: format!(
                "{{\"changeSetFingerprint\":\"{}\",\"planId\":\"{}\"}}",
                hex(&plan.change_set_fingerprint),
                plan.plan_id
            ),
        })
    }
}

async fn revalidate(
    project: &ProjectRecord,
    catalog: &ResolverCatalog,
    plan: &PackagePlanRecord,
) -> Result<(), M4Error> {
    if project.revision != plan.project_revision {
        return Err(M4Error::with_subreason(
            M4ErrorCode::PlanStale,
            "project_revision_changed",
        ));
    }
    let snapshot = tokio::task::spawn_blocking({
        let project = project.clone();
        move || inspect_package_project(&project)
    })
    .await
    .map_err(|_| M4Error::new(M4ErrorCode::Internal))??;
    if snapshot.fingerprint != plan.project_snapshot_fingerprint {
        return Err(M4Error::with_subreason(
            M4ErrorCode::PlanStale,
            "project_snapshot_changed",
        ));
    }
    for source in &plan.source_set {
        let row = catalog
            .entries
            .iter()
            .find(|row| {
                row.repository_id == source.repository_id
                    && row.package_id == source.package_id
                    && row.version == source.version
            })
            .ok_or_else(|| {
                M4Error::with_subreason(M4ErrorCode::PlanStale, "repository_revision_changed")
            })?;
        if row.repository_revision != source.repository_revision {
            return stale("repository_revision_changed");
        }
        if row.source_identity != source.source_identity {
            return stale("source_identity_changed");
        }
        if row.manifest_fingerprint != source.manifest_fingerprint {
            return stale("manifest_fingerprint_changed");
        }
        if row.artifact_url != source.artifact_url {
            return stale("artifact_url_changed");
        }
        if parse_digest(&row.zip_sha256)? != source.archive_sha256 {
            return stale("archive_digest_changed");
        }
    }
    Ok(())
}

fn resolution_requests(
    project: &ProjectRecord,
    request: &PackagePlanRequest,
    unity_version: Option<(u64, u64)>,
) -> Result<Vec<ResolveRequest>, M4Error> {
    if request.action == PlanAction::Resolve {
        let requests = project
            .observation
            .direct_dependencies
            .iter()
            .map(|dependency| ResolveRequest {
                package_id: dependency.package_id.clone(),
                range: dependency.value.clone(),
                repository_id: None,
                include_prerelease: request.include_prerelease,
                unity_version,
            })
            .collect::<Vec<_>>();
        if requests.is_empty() {
            return Err(M4Error::new(M4ErrorCode::PackageNotFound));
        }
        return Ok(requests);
    }
    let package_id = request
        .package_id
        .clone()
        .ok_or_else(|| M4Error::new(M4ErrorCode::InvalidInput))?;
    Ok(vec![ResolveRequest {
        package_id,
        range: request
            .version_range
            .clone()
            .unwrap_or_else(|| "*".to_owned()),
        repository_id: request.repository_id.clone(),
        include_prerelease: request.include_prerelease,
        unity_version,
    }])
}

fn validate_action_direction(
    project: &ProjectRecord,
    request: &PackagePlanRequest,
    resolution: &crate::Resolution,
) -> Result<(), M4Error> {
    if !matches!(request.action, PlanAction::Upgrade | PlanAction::Downgrade) {
        return Ok(());
    }
    let package_id = request
        .package_id
        .as_deref()
        .ok_or_else(|| M4Error::new(M4ErrorCode::InvalidInput))?;
    let installed = project
        .observation
        .locked_dependencies
        .iter()
        .find(|dependency| dependency.package_id == package_id)
        .ok_or_else(|| M4Error::new(M4ErrorCode::InvalidInput))?;
    let installed = Version::parse(&installed.value)
        .map_err(|_| M4Error::new(M4ErrorCode::PackageManifestInvalid))?;
    let selected = resolution
        .packages
        .iter()
        .find(|package| package.package_id == package_id)
        .ok_or_else(|| M4Error::new(M4ErrorCode::PackageNotFound))?;
    let ordering = selected.version.cmp_precedence(&installed);
    let valid = match request.action {
        PlanAction::Upgrade => ordering.is_gt(),
        PlanAction::Downgrade => ordering.is_lt(),
        _ => true,
    };
    valid
        .then_some(())
        .ok_or_else(|| M4Error::new(M4ErrorCode::InvalidInput))
}

fn parse_unity_editor_version(value: &str) -> Result<Option<(u64, u64)>, M4Error> {
    let mut parts = value.split('.');
    let major = parts
        .next()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| M4Error::new(M4ErrorCode::InvalidInput))?;
    let minor = parts
        .next()
        .and_then(|value| value.parse().ok())
        .ok_or_else(|| M4Error::new(M4ErrorCode::InvalidInput))?;
    Ok(Some((major, minor)))
}

fn prepare_transaction_root(root: &Path, operation_id: OperationId) -> Result<PathBuf, M4Error> {
    let relative = Path::new("Library")
        .join("ALCOMD")
        .join("transactions")
        .join(operation_id.to_string());
    ensure_directory_chain(root, &relative)?;
    Ok(root.join(relative))
}

fn ensure_directory_chain(root: &Path, relative: &Path) -> Result<(), M4Error> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.is_dir() && !is_link_or_reparse(&metadata) => {}
            Ok(_) => return Err(M4Error::new(M4ErrorCode::RecoveryRequired)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(&current)
                    .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
                let metadata = std::fs::symlink_metadata(&current)
                    .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
                if !metadata.is_dir() || is_link_or_reparse(&metadata) {
                    return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
                }
            }
            Err(_) => return Err(M4Error::new(M4ErrorCode::RecoveryRequired)),
        }
    }
    Ok(())
}

fn replace_packages(
    packages_root: &Path,
    staging: &Path,
    backup: &Path,
    plan: &PackagePlanRecord,
) -> Result<(), M4Error> {
    for mutation in &plan.change_set.mutations {
        let target = packages_root.join(&mutation.package_id);
        let backup_target = backup.join(&mutation.package_id);
        match std::fs::symlink_metadata(&target) {
            Ok(_) if mutation.kind == PackageMutationKind::Install => {
                return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply));
            }
            Ok(metadata) if metadata.is_dir() && !is_link_or_reparse(&metadata) => {
                std::fs::rename(&target, &backup_target)
                    .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
            }
            Ok(_) => return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if mutation.kind != PackageMutationKind::Install {
                    return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply));
                }
            }
            Err(_) => return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply)),
        }
        if mutation.kind != PackageMutationKind::Remove {
            std::fs::rename(staging.join(&mutation.package_id), &target)
                .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
        }
        alcomd_platform::sync_directory(packages_root)
            .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
    }
    Ok(())
}

fn commit_manifest(manifest_path: &Path, attempt: &Path) -> Result<(), M4Error> {
    let committed_old = attempt.join("vpm-manifest.commit-old");
    std::fs::rename(manifest_path, &committed_old)
        .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
    if std::fs::rename(attempt.join("vpm-manifest.new"), manifest_path).is_err() {
        let _ = std::fs::rename(&committed_old, manifest_path);
        return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
    }
    alcomd_platform::sync_directory(
        manifest_path
            .parent()
            .ok_or_else(|| M4Error::new(M4ErrorCode::Internal))?,
    )
    .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))
}

fn verify_final_state(
    root: &Path,
    attempt: &Path,
    plan: &PackagePlanRecord,
) -> Result<(), M4Error> {
    let packages_root = root.join("Packages");
    if hash_file(&packages_root.join("vpm-manifest.json"))? != plan.change_set.vpm_manifest_sha256 {
        return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
    }
    let expected_upm = read_regular_file(&attempt.join("upm-manifest.sha256"), 64)?;
    if expected_upm != hex(&hash_file(&packages_root.join("manifest.json"))?).as_bytes() {
        return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
    }
    for mutation in &plan.change_set.mutations {
        let target = packages_root.join(&mutation.package_id);
        if mutation.kind == PackageMutationKind::Remove {
            if std::fs::symlink_metadata(target).is_ok() {
                return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
            }
            continue;
        }
        let metadata = std::fs::symlink_metadata(&target)
            .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
        if !metadata.is_dir() || is_link_or_reparse(&metadata) {
            return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
        }
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
    Ok(())
}

fn rollback_attempt(
    packages_root: &Path,
    manifest_path: &Path,
    attempt: &Path,
    plan: &PackagePlanRecord,
) -> Result<(), M4Error> {
    let rolled = attempt.join("rolled-forward-content");
    if !rolled.exists() {
        std::fs::create_dir(&rolled).map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
    }
    let committed_old = attempt.join("vpm-manifest.commit-old");
    if committed_old.exists() {
        let rolled_manifest = rolled.join("vpm-manifest.new");
        if manifest_path.exists() && !rolled_manifest.exists() {
            std::fs::rename(manifest_path, &rolled_manifest)
                .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
        }
        if manifest_path.exists() {
            return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
        }
        std::fs::rename(&committed_old, manifest_path)
            .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
    }
    let backup = attempt.join("backup");
    for mutation in plan.change_set.mutations.iter().rev() {
        let target = packages_root.join(&mutation.package_id);
        let old = backup.join(&mutation.package_id);
        let rolled_target = rolled.join(&mutation.package_id);
        if mutation.kind == PackageMutationKind::Install {
            let staged = attempt.join("staging").join(&mutation.package_id);
            if !staged.exists() && target.exists() && !rolled_target.exists() {
                std::fs::rename(&target, &rolled_target)
                    .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
            }
        } else if old.exists() {
            if target.exists() && !rolled_target.exists() {
                std::fs::rename(&target, &rolled_target)
                    .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
            }
            if target.exists() {
                return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
            }
            std::fs::rename(&old, &target)
                .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
        }
    }
    sync_directories(&[packages_root, attempt])
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttemptMarker {
    version: u32,
    plan_id: String,
    change_set_fingerprint: String,
    phase: FilesystemPhase,
    state: JournalState,
}

impl AttemptMarker {
    fn new(plan: &PackagePlanRecord, phase: FilesystemPhase, state: JournalState) -> Self {
        Self {
            version: 1,
            plan_id: plan.plan_id.to_string(),
            change_set_fingerprint: hex(&plan.change_set_fingerprint),
            phase,
            state,
        }
    }
}

fn write_marker(attempt: &Path, marker: &AttemptMarker, step: u64) -> Result<(), M4Error> {
    let bytes = serde_json::to_vec(marker).map_err(|_| M4Error::new(M4ErrorCode::Internal))?;
    let marker_path = attempt.join(format!("marker-{step:020}.json"));
    write_new_file(&marker_path, &bytes)?;
    alcomd_platform::sync_directory(attempt)
        .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))
}

fn read_marker(attempt: &Path) -> Result<AttemptMarker, M4Error> {
    let marker_path = std::fs::read_dir(attempt)
        .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with("marker-") && name.ends_with(".json")
        })
        .map(|entry| entry.path())
        .max()
        .ok_or_else(|| M4Error::new(M4ErrorCode::RecoveryRequired))?;
    let bytes = read_regular_file(&marker_path, 65_536)?;
    let marker: AttemptMarker =
        serde_json::from_slice(&bytes).map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
    if marker.version != 1 {
        return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
    }
    Ok(marker)
}

#[expect(
    clippy::too_many_arguments,
    reason = "durable journal rows keep every invariant field explicit"
)]
async fn append_phase<S: M4Store>(
    store: &S,
    plan: &PackagePlanRecord,
    operation_id: OperationId,
    next_step: &mut u64,
    phase: FilesystemPhase,
    state: JournalState,
    project_identity_key: &[u8],
    evidence_json: String,
) -> Result<(), M4Error> {
    store
        .append_filesystem_journal(FilesystemJournalEntry {
            operation_id,
            step: *next_step,
            plan_id: plan.plan_id,
            project_id: plan.project_id,
            phase,
            state,
            project_identity_key: project_identity_key.to_vec(),
            change_set_fingerprint: plan.change_set_fingerprint,
            evidence_json,
            updated_at_ms: time_ms()?,
        })
        .await?;
    *next_step = next_step
        .checked_add(1)
        .ok_or_else(|| M4Error::new(M4ErrorCode::Internal))?;
    Ok(())
}

fn write_new_file(path: &Path, bytes: &[u8]) -> Result<(), M4Error> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
    file.write_all(bytes)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
        .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))
}

fn read_regular_file(path: &Path, limit: usize) -> Result<Vec<u8>, M4Error> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| M4Error::new(M4ErrorCode::ProjectChangedDuringApply))?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) || metadata.len() > limit as u64 {
        return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply));
    }
    let bytes =
        std::fs::read(path).map_err(|_| M4Error::new(M4ErrorCode::ProjectChangedDuringApply))?;
    if bytes.len() > limit {
        return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply));
    }
    Ok(bytes)
}

fn hash_file(path: &Path) -> Result<[u8; 32], M4Error> {
    Ok(Sha256::digest(read_regular_file(path, crate::PROJECT_MANIFEST_LIMIT)?).into())
}

fn sync_directories(paths: &[&Path]) -> Result<(), M4Error> {
    for path in paths {
        alcomd_platform::sync_directory(path)
            .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?;
    }
    Ok(())
}

fn parse_digest(value: &str) -> Result<[u8; 32], M4Error> {
    if value.len() != 64
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(M4Error::new(M4ErrorCode::PackageHashRequired));
    }
    let mut digest = [0_u8; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        digest[index] = (nibble(pair[0]) << 4) | nibble(pair[1]);
    }
    Ok(digest)
}

fn nibble(value: u8) -> u8 {
    match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        _ => 0,
    }
}

fn hex(value: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for byte in value {
        result.push(char::from(HEX[(byte >> 4) as usize]));
        result.push(char::from(HEX[(byte & 0x0f) as usize]));
    }
    result
}

fn stale(reason: &str) -> Result<(), M4Error> {
    Err(M4Error::with_subreason(M4ErrorCode::PlanStale, reason))
}

fn map_resolve_error(error: ResolveError) -> M4Error {
    match error {
        ResolveError::SourceAmbiguous { .. } => M4Error::new(M4ErrorCode::PackageSourceAmbiguous),
        ResolveError::LegacyCleanupRequired { .. } => {
            M4Error::new(M4ErrorCode::PackageLegacyCleanupRequired)
        }
        ResolveError::PackageNotFound { .. } => M4Error::new(M4ErrorCode::PackageNotFound),
        ResolveError::InvalidPackageId => M4Error::new(M4ErrorCode::InvalidInput),
        ResolveError::InvalidVersion => M4Error::new(M4ErrorCode::PackageVersionInvalid),
        ResolveError::InvalidRange => M4Error::new(M4ErrorCode::PackageRangeInvalid),
        ResolveError::DependencyMissing { .. } => {
            M4Error::new(M4ErrorCode::PackageDependencyMissing)
        }
        ResolveError::DependencyConflict { .. } => {
            M4Error::new(M4ErrorCode::PackageDependencyConflict)
        }
        ResolveError::UnityIncompatible { .. } => {
            M4Error::new(M4ErrorCode::PackageUnityIncompatible)
        }
        ResolveError::VersionYanked { .. } => M4Error::new(M4ErrorCode::PackageVersionYanked),
        ResolveError::TooManyRequirements => M4Error::new(M4ErrorCode::PlanTooLarge),
    }
}

fn map_cache_error(error: crate::CacheError) -> M4Error {
    use crate::CacheErrorCode as C;
    match error.code() {
        C::Corrupt => M4Error::new(M4ErrorCode::PackageCacheCorrupt),
        C::IntegrityMismatch => M4Error::new(M4ErrorCode::PackageIntegrityMismatch),
        C::DownloadTooLarge => M4Error::new(M4ErrorCode::PackageDownloadTooLarge),
        C::OfflineMiss => M4Error::new(M4ErrorCode::OfflineCacheMiss),
        C::QuotaExceeded => M4Error::new(M4ErrorCode::PackageCacheQuotaExceeded),
        C::InvalidDigest => M4Error::new(M4ErrorCode::PackageHashRequired),
        C::InvalidUrl => M4Error::new(M4ErrorCode::PackageManifestInvalid),
        C::DownloadFailed | C::Io => M4Error::new(M4ErrorCode::Internal),
    }
}

fn map_archive_error(error: crate::ArchiveError) -> M4Error {
    use crate::ArchiveErrorCode as C;
    match error.code() {
        C::UnsupportedCompression => {
            M4Error::new(M4ErrorCode::PackageArchiveUnsupportedCompression)
        }
        C::UnsafePath | C::LinkOrSpecialFile => M4Error::new(M4ErrorCode::PackageArchiveUnsafePath),
        C::PathCollision => M4Error::new(M4ErrorCode::PackagePathCollision),
        C::QuotaExceeded => M4Error::new(M4ErrorCode::PackageArchiveLimitExceeded),
        C::Invalid | C::Encrypted => M4Error::new(M4ErrorCode::PackageArchiveInvalid),
        C::Io => M4Error::new(M4ErrorCode::Internal),
    }
}

fn time_ms() -> Result<u64, M4Error> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| M4Error::new(M4ErrorCode::Internal))
        .and_then(|duration| {
            u64::try_from(duration.as_millis()).map_err(|_| M4Error::new(M4ErrorCode::Internal))
        })
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
    use alcomd_application::{
        DependencyIdentity, ManifestState, PackageChangeSet, PackageMutation, PackagePlanRecord,
        PlanState, PrincipalId, ProjectId, ProjectObservation, ProjectType, Revision,
    };

    #[test]
    fn rollback_restores_old_package_and_manifest_after_partial_commit() {
        let root = temporary_root("rollback");
        let packages = root.join("Packages");
        let attempt = root.join("Library/ALCOMD/transactions/operation/attempt-1");
        std::fs::create_dir_all(packages.join("com.example.fixture")).expect("new package");
        std::fs::write(
            packages.join("com.example.fixture/package.json"),
            b"new package",
        )
        .expect("new package file");
        std::fs::create_dir_all(attempt.join("backup/com.example.fixture"))
            .expect("backup package");
        std::fs::write(
            attempt.join("backup/com.example.fixture/package.json"),
            b"old package",
        )
        .expect("old package file");
        std::fs::write(packages.join("vpm-manifest.json"), b"new manifest").expect("new manifest");
        std::fs::write(attempt.join("vpm-manifest.commit-old"), b"old manifest")
            .expect("old manifest");

        rollback_attempt(
            &packages,
            &packages.join("vpm-manifest.json"),
            &attempt,
            &plan(PackageMutationKind::Replace),
        )
        .expect("rollback");

        assert_eq!(
            std::fs::read(packages.join("com.example.fixture/package.json"))
                .expect("restored package"),
            b"old package"
        );
        assert_eq!(
            std::fs::read(packages.join("vpm-manifest.json")).expect("restored manifest"),
            b"old manifest"
        );
        std::fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn rollback_does_not_move_an_untouched_existing_package() {
        let root = temporary_root("rollback-untouched");
        let packages = root.join("Packages");
        let attempt = root.join("Library/ALCOMD/transactions/operation/attempt-1");
        std::fs::create_dir_all(packages.join("com.example.fixture")).expect("old package");
        std::fs::create_dir_all(attempt.join("backup")).expect("empty backup");
        std::fs::write(
            packages.join("com.example.fixture/package.json"),
            b"untouched old package",
        )
        .expect("old package file");
        std::fs::write(packages.join("vpm-manifest.json"), b"old manifest").expect("old manifest");

        rollback_attempt(
            &packages,
            &packages.join("vpm-manifest.json"),
            &attempt,
            &plan(PackageMutationKind::Replace),
        )
        .expect("idempotent rollback");

        assert_eq!(
            std::fs::read(packages.join("com.example.fixture/package.json"))
                .expect("untouched package"),
            b"untouched old package"
        );
        std::fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn recovery_marker_is_append_only_and_latest_intent_is_visible() {
        let root = temporary_root("marker");
        std::fs::create_dir(&root).expect("attempt");
        let plan = plan(PackageMutationKind::Install);
        write_marker(
            &root,
            &AttemptMarker::new(&plan, FilesystemPhase::Prepared, JournalState::Completed),
            3,
        )
        .expect("prepared marker");
        write_marker(
            &root,
            &AttemptMarker::new(
                &plan,
                FilesystemPhase::PackagesReplaced,
                JournalState::Intent,
            ),
            4,
        )
        .expect("intent marker");
        let marker = read_marker(&root).expect("latest marker");
        assert_eq!(marker.phase, FilesystemPhase::PackagesReplaced);
        assert_eq!(marker.state, JournalState::Intent);
        assert_eq!(
            std::fs::read_dir(&root)
                .expect("markers")
                .filter_map(Result::ok)
                .count(),
            2
        );
        std::fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn upgrade_and_downgrade_validate_semver_precedence_not_build_metadata() {
        let project = project_with_locked_version("1.2.3+installed");
        let upgrade = package_request(PlanAction::Upgrade);
        let downgrade = package_request(PlanAction::Downgrade);

        assert!(validate_action_direction(&project, &upgrade, &resolution("1.2.4")).is_ok());
        assert!(validate_action_direction(&project, &downgrade, &resolution("1.2.2")).is_ok());
        assert_eq!(
            validate_action_direction(&project, &upgrade, &resolution("1.2.3+candidate"))
                .expect_err("build metadata must not create an upgrade")
                .code(),
            M4ErrorCode::InvalidInput
        );
        assert_eq!(
            validate_action_direction(&project, &downgrade, &resolution("1.2.4"))
                .expect_err("higher version must not be accepted as downgrade")
                .code(),
            M4ErrorCode::InvalidInput
        );
    }

    fn project_with_locked_version(version: &str) -> ProjectRecord {
        ProjectRecord {
            project_id: ProjectId::new(),
            observation: ProjectObservation {
                root_path: String::new(),
                path_identity_key: Vec::new(),
                project_type: ProjectType::Unknown,
                unity_version: "2022.3.22f1".to_owned(),
                unity_revision: None,
                vpm_manifest: ManifestState::Valid,
                upm_manifest: ManifestState::Valid,
                direct_dependencies: vec![DependencyIdentity {
                    package_id: "com.example.fixture".to_owned(),
                    value: version.to_owned(),
                }],
                locked_dependencies: vec![DependencyIdentity {
                    package_id: "com.example.fixture".to_owned(),
                    value: version.to_owned(),
                }],
                issues: Vec::new(),
                observed_at_ms: 1,
            },
            revision: Revision::INITIAL,
            registered_at_ms: 1,
        }
    }

    fn package_request(action: PlanAction) -> PackagePlanRequest {
        PackagePlanRequest {
            action,
            project_id: ProjectId::new(),
            expected_revision: Revision::INITIAL,
            package_id: Some("com.example.fixture".to_owned()),
            version_range: None,
            repository_id: None,
            include_prerelease: false,
        }
    }

    fn resolution(version: &str) -> crate::Resolution {
        crate::Resolution {
            packages: vec![crate::ResolvedPackage {
                package_id: "com.example.fixture".to_owned(),
                version: Version::parse(version).expect("version"),
                source: crate::PackageSource {
                    repository_id: "repo".to_owned(),
                    repository_revision: 1,
                    priority: 1,
                    source_identity: "repo-source".to_owned(),
                    manifest_fingerprint: [4; 32],
                    artifact_url: "https://example.invalid/package.zip".to_owned(),
                    archive_sha256: [5; 32],
                },
                direct: true,
            }],
            dependency_edges: Vec::new(),
        }
    }

    fn plan(kind: PackageMutationKind) -> PackagePlanRecord {
        PackagePlanRecord {
            plan_id: alcomd_application::PlanId::new(),
            owner: PrincipalId::local_owner(),
            project_id: ProjectId::new(),
            action: PlanAction::Install,
            state: PlanState::Applied,
            project_revision: Revision::INITIAL,
            project_snapshot_fingerprint: [1; 32],
            change_set_fingerprint: [2; 32],
            change_set: PackageChangeSet {
                format_version: 1,
                mutations: vec![PackageMutation {
                    kind,
                    package_id: "com.example.fixture".to_owned(),
                    from_version: (kind != PackageMutationKind::Install)
                        .then(|| "0.9.0".to_owned()),
                    to_version: (kind != PackageMutationKind::Remove).then(|| "1.0.0".to_owned()),
                    source: None,
                }],
                dependency_edges: Vec::new(),
                vpm_manifest_sha256: [3; 32],
            },
            source_set: Vec::new(),
            apply_operation_id: None,
            created_at_ms: 1,
        }
    }

    fn temporary_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "alcomd-m4-engine-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }
}
