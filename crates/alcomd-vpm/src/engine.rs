use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use alcomd_application::{
    FilesystemJournalEntry, FilesystemPhase, JournalState, M3ReadAdapter, M4Error, M4ErrorCode,
    M4PackageAdapter, M4Store, OperationId, PackageApplyCompletion, PackageBulkIntent,
    PackageMutationKind, PackagePlanDraft, PackagePlanRecord, PackagePlanRequest,
    PackageSourceSelector, PlanAction, PrincipalId, ProjectDiscoveryMode, ProjectRecord,
    ReinstallSelection, RepositoryArchivePinStore, RepositoryPackageSourcePin, ResolverCatalog,
    ResourceKey, ResourceLockCoordinator, StateStore,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::package::{LegacyMetadata, validate_extracted_package};
use crate::staging_package::{
    PACKAGE_DOWNLOAD_CONCURRENCY, extract_package_objects, fetch_package_objects, run_ordered_jobs,
};
use crate::{
    PackageCache, ResolveError, ResolveRequest, VpmReader, build_bulk_plan, build_reinstall_plan,
    build_remove_plan, build_resolution_plan, candidates_from_catalog, inspect_package_project,
};

#[derive(Clone)]
pub struct PackageEngine<S: M4Store + StateStore> {
    store: S,
    reader: VpmReader,
    cache: PackageCache,
}

impl<S: M4Store + StateStore> PackageEngine<S> {
    pub fn new(store: S, reader: VpmReader, cache_root: PathBuf) -> Result<Self, M4Error> {
        let cache = reader
            .package_cache(cache_root)
            .map_err(|cause| M4Error::new(M4ErrorCode::PackageCacheCorrupt).with_source(cause))?;
        Ok(Self {
            store,
            reader,
            cache,
        })
    }
}

impl<S: M4Store + StateStore> M4PackageAdapter for PackageEngine<S> {
    async fn source_settings(&self) -> Result<alcomd_application::ConfigPackageSettings, M4Error> {
        self.reader.package_settings().await
    }
    async fn prepare_plan(
        &self,
        _owner: PrincipalId,
        project: ProjectRecord,
        catalog: ResolverCatalog,
        request: PackagePlanRequest,
    ) -> Result<PackagePlanDraft, M4Error> {
        let override_target = if request.allow_unity_incompatible {
            if !matches!(
                request.action,
                PlanAction::Install | PlanAction::Upgrade | PlanAction::Downgrade
            ) || request.plan_version != alcomd_application::PackagePlanVersion::V2
            {
                return Err(M4Error::new(M4ErrorCode::InvalidInput));
            }
            let version = request
                .version_range
                .as_deref()
                .and_then(|value| value.strip_prefix('='))
                .and_then(|value| Version::parse(value).ok())
                .ok_or_else(|| M4Error::new(M4ErrorCode::InvalidInput))?;
            Some(alcomd_application::PackageUnityCompatibilityOverride {
                package_id: request
                    .package_id
                    .clone()
                    .ok_or_else(|| M4Error::new(M4ErrorCode::InvalidInput))?,
                version: version.to_string(),
            })
        } else {
            None
        };
        let (project, snapshot) = inspect_current_package_project(&self.reader, project).await?;
        let draft = if request.action == PlanAction::Reinstall {
            prepare_reinstall(&self.cache, &snapshot, &project, &catalog, &request).await?
        } else if request.action == PlanAction::Bulk {
            prepare_bulk(&self.cache, &snapshot, &project, &catalog, &request).await?
        } else if request.action == PlanAction::Remove {
            let package_id = request
                .package_id
                .as_deref()
                .ok_or_else(|| M4Error::new(M4ErrorCode::InvalidInput))?;
            return build_remove_plan(&snapshot, package_id);
        } else {
            let mut candidates = candidates_from_catalog(&catalog).map_err(map_resolve_error)?;
            let unity_version = parse_unity_editor_version(&project.observation.unity_version)?;
            let mut requests = resolution_requests(&project, &request, unity_version)?;
            if request.action == PlanAction::Resolve {
                requests.extend(
                    snapshot
                        .unlocked
                        .values()
                        .flat_map(|package| &package.dependencies)
                        .filter(|dependency| {
                            !snapshot.unlocked.contains_key(&dependency.package_id)
                        })
                        .map(|dependency| ResolveRequest {
                            allow_unity_incompatible: false,
                            package_id: dependency.package_id.clone(),
                            range: dependency.range.clone(),
                            source: None,
                            include_prerelease: request.include_prerelease,
                            unity_version,
                        }),
                );
            }
            if request.action == PlanAction::Resolve {
                let locked = project
                    .observation
                    .locked_dependencies
                    .iter()
                    .map(|entry| (entry.package_id.as_str(), entry.value.as_str()))
                    .collect::<BTreeMap<_, _>>();
                for candidate in &mut candidates {
                    if locked
                        .get(candidate.package_id.as_str())
                        .is_some_and(|version| candidate.version.to_string() == *version)
                    {
                        candidate.yanked = false;
                        candidate.unity_minimum = None;
                        candidate.dependencies.clear();
                    }
                }
                candidates.retain(|candidate| {
                    locked
                        .get(candidate.package_id.as_str())
                        .is_none_or(|version| candidate.version.to_string() == *version)
                });
            }
            let protected: BTreeSet<_> = request.package_id.iter().cloned().collect();
            let rules = snapshot
                .resolution_rules(protected.clone(), request.action != PlanAction::Resolve)?;
            let mut resolution = if requests.is_empty() {
                crate::Resolution {
                    packages: Vec::new(),
                    dependency_edges: Vec::new(),
                }
            } else {
                resolve_migrating_packages(
                    &self.cache,
                    &candidates,
                    &requests,
                    &protected,
                    Some(&rules),
                )
                .await?
            };
            if request.action == PlanAction::Resolve {
                normalize_direct_authority(&project, &mut resolution);
            }
            validate_action_direction(&project, &request, &resolution)?;
            build_resolution_plan(&snapshot, request.action, &resolution)?
        };
        let mut draft = prepare_legacy_removals(&self.cache, &snapshot, &catalog, draft).await?;
        if let Some(target) = override_target {
            draft.change_set.format_version = 4;
            draft.change_set.unity_compatibility_overrides = vec![target];
            draft.change_set.validate_bounds()?;
            draft.change_set_fingerprint = Sha256::digest(
                serde_json::to_vec(&draft.change_set)
                    .map_err(|_| M4Error::new(M4ErrorCode::Internal))?,
            )
            .into();
        }
        Ok(draft)
    }

    async fn revalidate_plan(
        &self,
        project: ProjectRecord,
        catalog: ResolverCatalog,
        plan: PackagePlanRecord,
    ) -> Result<(), M4Error> {
        alcomd_application::validate_package_source_settings(
            &self.reader.package_settings().await?,
            &plan.source_set,
        )?;
        revalidate(&self.reader, &project, &catalog, &plan)
            .await
            .map(|_| ())
    }

    async fn execute_plan(
        &self,
        operation_id: OperationId,
        project: ProjectRecord,
        plan: PackagePlanRecord,
        locks: Arc<ResourceLockCoordinator>,
    ) -> Result<PackageApplyCompletion, M4Error> {
        let path = project.observation.root_path.clone();
        self.execute(operation_id, project, plan, locks)
            .await
            .map_err(|source| source.with_path(path))
    }
}

async fn prepare_reinstall(
    cache: &PackageCache,
    snapshot: &crate::ProjectPackageSnapshot,
    project: &ProjectRecord,
    catalog: &ResolverCatalog,
    request: &PackagePlanRequest,
) -> Result<PackagePlanDraft, M4Error> {
    let selection = request
        .reinstall_selection
        .as_ref()
        .ok_or_else(|| M4Error::new(M4ErrorCode::InvalidInput))?;
    let locked = project
        .observation
        .locked_dependencies
        .iter()
        .map(|dependency| (dependency.package_id.clone(), dependency.value.clone()))
        .collect::<BTreeMap<_, _>>();
    let targets = match selection {
        ReinstallSelection::Packages(package_ids) => {
            if package_ids.is_empty() || package_ids.len() > 256 {
                return selection_limit();
            }
            let targets = package_ids.iter().cloned().collect::<BTreeSet<_>>();
            if targets.len() != package_ids.len() {
                return Err(M4Error::new(M4ErrorCode::InvalidInput));
            }
            for package_id in &targets {
                if !locked.contains_key(package_id) {
                    return Err(M4Error::new(M4ErrorCode::PackageNotInstalled));
                }
            }
            targets
        }
        ReinstallSelection::All => {
            if !request.reinstall_sources.is_empty() {
                return Err(M4Error::new(M4ErrorCode::InvalidInput));
            }
            if locked.len() > 256 {
                return selection_limit();
            }
            locked.keys().cloned().collect()
        }
    };
    let selectors = reinstall_selectors(&targets, &request.reinstall_sources)?;
    let unity_version = parse_unity_editor_version(&project.observation.unity_version)?;
    let requests = targets
        .iter()
        .map(|package_id| ResolveRequest {
            allow_unity_incompatible: false,
            package_id: package_id.clone(),
            range: format!("={}", locked[package_id]),
            source: selectors.get(package_id).cloned(),
            include_prerelease: locked[package_id].contains('-'),
            unity_version,
        })
        .collect::<Vec<_>>();
    // Reinstall selects artifacts, not a new dependency graph. The project's
    // original locked graph and manifest remain authoritative, including sources
    // that are unavailable for packages outside this explicit selection.
    let candidates = candidates_from_catalog(catalog)
        .map_err(map_resolve_error)?
        .into_iter()
        .filter(|candidate| {
            targets.contains(&candidate.package_id)
                && locked
                    .get(&candidate.package_id)
                    .is_some_and(|version| *version == candidate.version.to_string())
        })
        .map(|mut candidate| {
            candidate.dependencies.clear();
            candidate.yanked = false;
            candidate.unity_minimum = None;
            candidate
        })
        .collect::<Vec<_>>();
    let resolution = if requests.is_empty() {
        crate::Resolution {
            packages: Vec::new(),
            dependency_edges: Vec::new(),
        }
    } else {
        resolve_migrating_packages(cache, &candidates, &requests, &targets, None).await?
    };
    build_reinstall_plan(snapshot, &resolution, &targets)
}

async fn prepare_bulk(
    cache: &PackageCache,
    snapshot: &crate::ProjectPackageSnapshot,
    project: &ProjectRecord,
    catalog: &ResolverCatalog,
    request: &PackagePlanRequest,
) -> Result<PackagePlanDraft, M4Error> {
    if request.bulk_intents.is_empty() || request.bulk_intents.len() > 256 {
        return selection_limit();
    }
    let mut intents = BTreeMap::new();
    for intent in &request.bulk_intents {
        if intents
            .insert(intent.package_id().to_owned(), intent)
            .is_some()
        {
            return Err(M4Error::new(M4ErrorCode::PackageIntentConflict));
        }
    }
    if request.is_removal_only() {
        let removed = intents.keys().cloned().collect();
        return crate::plan::build_removal_plan(snapshot, &removed, PlanAction::Bulk);
    }
    let locked = project
        .observation
        .locked_dependencies
        .iter()
        .map(|dependency| (dependency.package_id.clone(), dependency.value.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut desired = BTreeMap::new();
    let mut reinstall_targets = BTreeSet::new();
    let mut upgrade_targets = BTreeSet::new();
    let mut removed = BTreeSet::new();
    for intent in intents.values() {
        match intent {
            PackageBulkIntent::Install {
                package_id,
                version_range,
                source,
                include_prerelease,
            } => {
                desired.insert(
                    package_id.clone(),
                    (
                        version_range.clone().unwrap_or_else(|| "*".to_owned()),
                        source.clone(),
                        *include_prerelease,
                    ),
                );
            }
            PackageBulkIntent::Upgrade {
                package_id,
                version_range,
                source,
                include_prerelease,
            } => {
                if !locked.contains_key(package_id) {
                    return Err(M4Error::new(M4ErrorCode::PackageNotInstalled));
                }
                desired.insert(
                    package_id.clone(),
                    (
                        version_range.clone().unwrap_or_else(|| "*".to_owned()),
                        source.clone(),
                        *include_prerelease,
                    ),
                );
                upgrade_targets.insert(package_id.clone());
            }
            PackageBulkIntent::Remove { package_id } => {
                if !locked.contains_key(package_id) {
                    return Err(M4Error::new(M4ErrorCode::PackageNotFound));
                }
                removed.insert(package_id.clone());
            }
            PackageBulkIntent::Reinstall { package_id, source } => {
                let version = locked
                    .get(package_id)
                    .ok_or_else(|| M4Error::new(M4ErrorCode::PackageNotInstalled))?;
                desired.insert(
                    package_id.clone(),
                    (format!("={version}"), source.clone(), version.contains('-')),
                );
                reinstall_targets.insert(package_id.clone());
            }
        }
    }
    let unity_version = parse_unity_editor_version(&project.observation.unity_version)?;
    let requests = desired
        .iter()
        .map(
            |(package_id, (range, source, include_prerelease))| ResolveRequest {
                allow_unity_incompatible: false,
                package_id: package_id.clone(),
                range: range.clone(),
                source: source.clone(),
                include_prerelease: *include_prerelease,
                unity_version,
            },
        )
        .collect::<Vec<_>>();
    let mut candidates = candidates_from_catalog(catalog).map_err(map_resolve_error)?;
    candidates.retain(|candidate| {
        !reinstall_targets.contains(&candidate.package_id)
            || locked
                .get(&candidate.package_id)
                .is_some_and(|version| *version == candidate.version.to_string())
    });
    for candidate in &mut candidates {
        if reinstall_targets.contains(&candidate.package_id)
            && locked
                .get(&candidate.package_id)
                .is_some_and(|version| *version == candidate.version.to_string())
        {
            candidate.dependencies.clear();
            candidate.yanked = false;
            candidate.unity_minimum = None;
        }
    }
    let mut resolution = if requests.is_empty() {
        crate::Resolution {
            packages: Vec::new(),
            dependency_edges: Vec::new(),
        }
    } else {
        let explicit = intents.keys().cloned().collect();
        let mut rules = snapshot.resolution_rules(explicit, true)?;
        rules.locked.retain(|id, _| !removed.contains(id));
        rules.direct.retain(|id, _| !removed.contains(id));
        resolve_migrating_packages(cache, &candidates, &requests, &rules.explicit, Some(&rules))
            .await?
    };
    let direct_ids = project
        .observation
        .direct_dependencies
        .iter()
        .map(|dependency| dependency.package_id.as_str())
        .collect::<BTreeSet<_>>();
    let desired_ids = desired
        .keys()
        .filter(|id| !reinstall_targets.contains(*id) || direct_ids.contains(id.as_str()))
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    for package in &mut resolution.packages {
        package.direct = desired_ids.contains(package.package_id.as_str());
    }
    resolution
        .dependency_edges
        .retain(|edge| !edge.direct || desired_ids.contains(edge.to_package_id.as_str()));
    if removed.iter().any(|package_id| {
        resolution
            .packages
            .iter()
            .any(|package| &package.package_id == package_id)
    }) {
        return Err(M4Error::new(M4ErrorCode::PackageDependencyConflict));
    }
    for package_id in &reinstall_targets {
        let expected = locked
            .get(package_id)
            .ok_or_else(|| M4Error::new(M4ErrorCode::PackageNotInstalled))?;
        let actual = resolution
            .packages
            .iter()
            .find(|package| &package.package_id == package_id)
            .ok_or_else(|| M4Error::new(M4ErrorCode::PackageNotFound))?;
        if actual.version.to_string() != *expected {
            return Err(M4Error::new(M4ErrorCode::PackageDependencyConflict));
        }
    }
    for package_id in upgrade_targets {
        let installed = Version::parse(
            locked
                .get(&package_id)
                .ok_or_else(|| M4Error::new(M4ErrorCode::PackageNotInstalled))?,
        )
        .map_err(|cause| M4Error::new(M4ErrorCode::PackageManifestInvalid).with_source(cause))?;
        let selected = resolution
            .packages
            .iter()
            .find(|package| package.package_id == package_id)
            .ok_or_else(|| M4Error::new(M4ErrorCode::PackageNotFound))?;
        if !selected.version.cmp_precedence(&installed).is_gt() {
            return Err(M4Error::new(M4ErrorCode::InvalidInput));
        }
    }
    build_bulk_plan(snapshot, &resolution, &reinstall_targets, &removed)
}

fn reinstall_selectors(
    targets: &BTreeSet<String>,
    values: &[(String, PackageSourceSelector)],
) -> Result<BTreeMap<String, PackageSourceSelector>, M4Error> {
    if values.len() > 256 {
        return selection_limit();
    }
    let mut selectors = BTreeMap::new();
    for (package_id, source) in values {
        if !targets.contains(package_id)
            || selectors
                .insert(package_id.clone(), source.clone())
                .is_some()
        {
            return Err(M4Error::new(M4ErrorCode::InvalidInput));
        }
    }
    Ok(selectors)
}

fn normalize_direct_authority(project: &ProjectRecord, resolution: &mut crate::Resolution) {
    let direct = project
        .observation
        .direct_dependencies
        .iter()
        .map(|dependency| dependency.package_id.as_str())
        .collect::<BTreeSet<_>>();
    for package in &mut resolution.packages {
        package.direct = direct.contains(package.package_id.as_str());
    }
    // A pinned transitive root is a solver constraint, not a package self-dependency.
    resolution
        .dependency_edges
        .retain(|edge| !edge.direct || direct.contains(edge.to_package_id.as_str()));
}

fn selection_limit<T>() -> Result<T, M4Error> {
    Err(M4Error::with_subreason(
        M4ErrorCode::PlanTooLarge,
        "selection_limit",
    ))
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
            .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
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

        alcomd_application::validate_package_source_settings(
            &self.reader.package_settings().await?,
            &plan.source_set,
        )?;

        let mut next_step = self
            .store
            .next_filesystem_journal_step(operation_id)
            .await?;
        let attempt = transaction_root.join(format!("attempt-{next_step:020}"));
        std::fs::create_dir(&attempt)
            .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
        let staging = attempt.join("staging");
        let backup = attempt.join("backup");
        std::fs::create_dir(&staging)
            .and_then(|()| std::fs::create_dir(&backup))
            .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
        sync_directories(&[&attempt, &transaction_root])?;

        let sources = pin_apply_archives(
            &self.cache,
            &self.store,
            &plan.owner,
            self.reader.package_settings().await?,
            plan.change_set
                .mutations
                .iter()
                .filter_map(|mutation| mutation.source.clone())
                .collect(),
        )
        .await?;
        let archives = fetch_package_objects(&self.cache, Some(&locks), sources).await?;
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
        .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
        write_marker(
            &attempt,
            &AttemptMarker::new(
                &plan,
                FilesystemPhase::ArchiveReady,
                JournalState::Completed,
            ),
            next_step - 1,
        )?;
        test_kill_gate(&attempt, "archive_ready")?;

        extract_package_objects(&staging, &plan.change_set, &archives).await?;
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
        .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
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
        let catalog = if plan.source_set.is_empty() {
            ResolverCatalog {
                entries: Vec::new(),
                complete: true,
            }
        } else {
            self.store
                .resolver_catalog(
                    plan.owner.clone(),
                    plan.source_set
                        .iter()
                        .any(alcomd_application::PackageSourcePin::is_user_package),
                    self.reader.package_settings().await?,
                )
                .await?
        };
        alcomd_application::validate_package_source_settings(
            &self.reader.package_settings().await?,
            &plan.source_set,
        )?;
        let snapshot = revalidate(&self.reader, &project, &catalog, &plan).await?;
        let installed_legacy = snapshot
            .installed_legacy(&plan.source_set, &plan.change_set.legacy_reinstall_packages)?;
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
        let new_manifest = crate::plan::materialize_plan_manifest(
            &current_manifest,
            &plan.change_set,
            crate::plan::preserves_repaired_locks(plan.action),
        )?;
        if Sha256::digest(&new_manifest).as_slice() != plan.change_set.vpm_manifest_sha256 {
            if plan.change_set.format_version < 3 {
                return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
            }
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
        .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
        write_marker(
            &attempt,
            &AttemptMarker::new(&plan, FilesystemPhase::Prepared, JournalState::Completed),
            next_step - 1,
        )?;
        test_kill_gate(&attempt, "prepared")?;

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
        // No awaited work separates this proof from the first project write.
        verify_plan_legacy(
            &self.cache,
            &root,
            &staging,
            &catalog,
            &plan,
            &installed_legacy,
            false,
        )?;
        let replaced = quarantine_legacy(&root, &attempt, &plan)
            .and_then(|()| replace_packages(&packages_root, &staging, &backup, &attempt, &plan));
        if let Err(error) = replaced {
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
            .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
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
        .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
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
        .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
        write_marker(
            &attempt,
            &AttemptMarker::new(
                &plan,
                FilesystemPhase::VpmManifestCommitted,
                JournalState::Intent,
            ),
            next_step - 1,
        )?;
        // Core's package swap must not introduce or conceal a cleanup obligation.
        // A late target change uses the same established rollback path.
        let commit = verify_plan_legacy(
            &self.cache,
            &root,
            &packages_root,
            &catalog,
            &plan,
            &installed_legacy,
            true,
        )
        .and_then(|()| commit_manifest(&manifest_path, &attempt));
        if let Err(error) = commit {
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
            .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
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
        .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
        write_marker(
            &attempt,
            &AttemptMarker::new(
                &plan,
                FilesystemPhase::VpmManifestCommitted,
                JournalState::Completed,
            ),
            next_step - 1,
        )?;
        test_kill_gate(&attempt, "vpm_manifest_committed")?;

        if let Err(error) = verify_final_state(&root, &attempt, &plan) {
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
            .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
            return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply).with_source(error));
        }
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
        .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
        write_marker(
            &attempt,
            &AttemptMarker::new(
                &plan,
                FilesystemPhase::FilesystemCommitted,
                JournalState::Completed,
            ),
            next_step - 1,
        )?;
        test_kill_gate(&attempt, "filesystem_committed")?;
        cleanup_legacy(&root, &attempt, &plan)?;
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
        .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
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
            .map_err(|cause| {
                M4Error::new(M4ErrorCode::RecoveryRequired)
                    .with_source(cause)
                    .with_path(transaction_root.display().to_string())
            })?
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("attempt-"))
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        attempts.sort();
        for attempt in attempts {
            let marker = match read_marker(&attempt) {
                Ok(marker) => marker,
                Err(cause) => {
                    return Err(M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause));
                }
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
                let root = Path::new(&project.observation.root_path);
                verify_final_state(root, &attempt, plan)?;
                cleanup_legacy(root, &attempt, plan)?;
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
            .map_err(|cause| {
                M4Error::new(M4ErrorCode::ProjectChangedDuringApply)
                    .with_source(cause)
                    .with_path(root.display().to_string())
            })?;
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

// Registration owns project identity and revision. Package inputs come from the
// current files, just as they do for candidate reads, without refreshing the registry.
async fn inspect_current_package_project(
    reader: &VpmReader,
    mut project: ProjectRecord,
) -> Result<(ProjectRecord, crate::ProjectPackageSnapshot), M4Error> {
    let observation = reader
        .inspect_project(
            project.observation.root_path.clone(),
            ProjectDiscoveryMode::ExactRoot,
        )
        .await
        .map_err(|cause| {
            let code = if cause.code() == crate::M3ErrorCode::ProjectManifestInvalid {
                M4ErrorCode::PackageManifestInvalid
            } else {
                M4ErrorCode::ProjectChangedDuringApply
            };
            M4Error::new(code).with_source(cause)
        })?;
    if observation.path_identity_key != project.observation.path_identity_key {
        return stale("project_identity_changed");
    }
    project.observation = observation;
    let snapshot = tokio::task::spawn_blocking({
        let project = project.clone();
        move || inspect_package_project(&project)
    })
    .await
    .map_err(|cause| M4Error::new(M4ErrorCode::Internal).with_source(cause))??;
    Ok((project, snapshot))
}

async fn revalidate(
    reader: &VpmReader,
    project: &ProjectRecord,
    catalog: &ResolverCatalog,
    plan: &PackagePlanRecord,
) -> Result<crate::ProjectPackageSnapshot, M4Error> {
    if plan.state == alcomd_application::PlanState::Unapplied && plan.change_set.format_version < 3
    {
        return stale("implementation_basis_changed");
    }
    if project.revision != plan.project_revision {
        return Err(M4Error::with_subreason(
            M4ErrorCode::PlanStale,
            "project_revision_changed",
        ));
    }
    let (_, snapshot) = inspect_current_package_project(reader, project.clone()).await?;
    if snapshot.fingerprint != plan.project_snapshot_fingerprint {
        return Err(M4Error::with_subreason(
            M4ErrorCode::PlanStale,
            "project_snapshot_changed",
        ));
    }
    for source in &plan.source_set {
        if matches!(source, alcomd_application::PackageSourcePin::UserPackage(_)) {
            // A v2 User Package plan is authorized by its immutable owned cache digest.
            // Registry refresh/removal must not rewrite or invalidate an existing plan.
            continue;
        }
        let row = catalog
            .entries
            .iter()
            .find(|row| row.package_id == source.package_id() && row.version == source.version())
            .and_then(|row| match (&row.source, source) {
                (
                    alcomd_application::ResolverCatalogSource::Repository { repository_id, .. },
                    alcomd_application::PackageSourcePin::Repository(pin),
                ) if repository_id == &pin.repository_id => Some(row),
                (
                    alcomd_application::ResolverCatalogSource::UserPackage {
                        user_package_id, ..
                    },
                    alcomd_application::PackageSourcePin::UserPackage(pin),
                ) if user_package_id == &pin.user_package_id => Some(row),
                _ => None,
            })
            .ok_or_else(|| {
                M4Error::with_subreason(M4ErrorCode::PlanStale, "source_revision_changed")
            })?;
        match (&row.source, source) {
            (
                alcomd_application::ResolverCatalogSource::Repository {
                    repository_locator,
                    repository_revision,
                    source_identity,
                    artifact_url,
                    credential_reference,
                    download_headers_present,
                    zip_sha256,
                    local_zip_sha256,
                    manifest_fingerprint,
                    ..
                },
                alcomd_application::PackageSourcePin::Repository(pin),
            ) => {
                if repository_revision != &pin.repository_revision {
                    return stale("repository_revision_changed");
                }
                if source_identity != &pin.source_identity {
                    return stale("source_identity_changed");
                }
                if manifest_fingerprint != &pin.manifest_fingerprint {
                    return stale("manifest_fingerprint_changed");
                }
                if artifact_url != &pin.artifact_url {
                    return stale("artifact_url_changed");
                }
                if credential_reference != &pin.credential_reference {
                    return stale("repository_revision_changed");
                }
                let local_path = source_identity
                    .starts_with("local-fileid-v1:")
                    .then_some(repository_locator.as_str());
                if *download_headers_present != pin.download_headers.is_some()
                    || pin
                        .download_headers
                        .as_ref()
                        .is_some_and(|headers| headers.local_path.as_deref() != local_path)
                {
                    return stale("manifest_fingerprint_changed");
                }
                let current_digest = zip_sha256
                    .as_deref()
                    .map(parse_digest)
                    .transpose()?
                    .or(*local_zip_sha256);
                if pin.archive_sha256.is_some() && current_digest != pin.archive_sha256 {
                    return stale("archive_digest_changed");
                }
            }
            (
                alcomd_application::ResolverCatalogSource::UserPackage {
                    source_revision,
                    source_identity,
                    archive_sha256,
                    manifest_fingerprint,
                    ..
                },
                alcomd_application::PackageSourcePin::UserPackage(pin),
            ) => {
                if source_revision != &pin.source_revision {
                    return stale("source_revision_changed");
                }
                if source_identity != &pin.source_identity {
                    return stale("source_identity_changed");
                }
                if manifest_fingerprint != &pin.manifest_fingerprint {
                    return stale("manifest_fingerprint_changed");
                }
                if archive_sha256 != &pin.archive_sha256 {
                    return stale("archive_digest_changed");
                }
            }
            _ => return stale("source_authority_changed"),
        }
    }
    Ok(snapshot)
}

fn resolution_requests(
    project: &ProjectRecord,
    request: &PackagePlanRequest,
    unity_version: Option<crate::UnityVersion>,
) -> Result<Vec<ResolveRequest>, M4Error> {
    if request.action == PlanAction::Resolve {
        let mut requests = project
            .observation
            .direct_dependencies
            .iter()
            .filter(|dependency| {
                !project
                    .observation
                    .locked_dependencies
                    .iter()
                    .any(|locked| locked.package_id == dependency.package_id)
            })
            .map(|dependency| {
                Ok(ResolveRequest {
                    allow_unity_incompatible: false,
                    package_id: dependency.package_id.clone(),
                    range: crate::range::ProjectDependencyRange::parse(&dependency.value)
                        .map_err(|cause| {
                            M4Error::new(M4ErrorCode::PackageManifestInvalid).with_source(cause)
                        })?
                        .canonical(),
                    source: None,
                    include_prerelease: request.include_prerelease,
                    unity_version,
                })
            })
            .collect::<Result<Vec<_>, M4Error>>()?;
        requests.extend(
            project
                .observation
                .locked_dependencies
                .iter()
                .map(|dependency| ResolveRequest {
                    allow_unity_incompatible: false,
                    package_id: dependency.package_id.clone(),
                    range: format!("={}", dependency.value),
                    source: None,
                    include_prerelease: request.include_prerelease
                        || Version::parse(&dependency.value)
                            .is_ok_and(|version| !version.pre.is_empty()),
                    unity_version,
                }),
        );
        return Ok(requests);
    }
    let package_id = request
        .package_id
        .clone()
        .ok_or_else(|| M4Error::new(M4ErrorCode::InvalidInput))?;
    Ok(vec![ResolveRequest {
        allow_unity_incompatible: request.allow_unity_incompatible,
        package_id,
        range: request
            .version_range
            .clone()
            .unwrap_or_else(|| "*".to_owned()),
        source: request.source_selector.clone().or_else(|| {
            request.repository_id.clone().map(|repository_id| {
                alcomd_application::PackageSourceSelector::Repository { repository_id }
            })
        }),
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
        .map_err(|cause| M4Error::new(M4ErrorCode::PackageManifestInvalid).with_source(cause))?;
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

pub(crate) fn parse_unity_editor_version(
    value: &str,
) -> Result<Option<crate::UnityVersion>, M4Error> {
    Ok(crate::UnityVersion::parse_editor(value))
}

/// Solving collects metadata first; only a read-only target proof can discharge
/// its cleanup obligations. The package archive is pinned by its approved hash.
pub(crate) async fn resolve_project_packages(
    cache: &PackageCache,
    store: &impl RepositoryArchivePinStore,
    owner: &PrincipalId,
    root: &Path,
    candidates: &[crate::PackageCandidate],
    requests: &[ResolveRequest],
) -> Result<crate::Resolution, M4Error> {
    let mut resolved = crate::resolver::resolve_unity_migration_packages(candidates, requests)
        .map_err(map_resolve_error)?;
    let selected_ids = resolved
        .resolution
        .packages
        .iter()
        .map(|package| package.package_id.as_str())
        .collect::<BTreeSet<_>>();
    for package in &resolved.resolution.packages {
        let needs_proof = resolved
            .legacy_cleanup_obligations
            .iter()
            .any(|obligation| obligation.package_id == package.package_id)
            || matches!(
                package.source.authority,
                crate::PackageSourceAuthority::UserPackage { .. }
            );
        if needs_proof {
            let metadata = cache
                .pin_legacy_manifest(
                    &package.source,
                    &package.package_id,
                    &package.version.to_string(),
                )
                .await
                .map_err(map_cache_error)?;
            if metadata
                .packages
                .iter()
                .any(|id| selected_ids.contains(id.as_str()))
            {
                return Err(M4Error::new(M4ErrorCode::PackageLegacyCleanupRequired));
            }
            crate::legacy::verify_absent(root, &metadata)?;
        }
    }
    pin_resolution_archives(cache, store, owner, &mut resolved.resolution).await?;
    Ok(resolved.resolution)
}

async fn resolve_migrating_packages(
    cache: &PackageCache,
    candidates: &[crate::PackageCandidate],
    requests: &[ResolveRequest],
    protected: &BTreeSet<String>,
    rules: Option<&crate::resolver::ProjectResolutionRules>,
) -> Result<crate::Resolution, M4Error> {
    let mut remaining = requests.to_vec();
    let mut omitted = BTreeSet::new();
    let mut first_error = None;
    loop {
        let result = match rules {
            Some(rules) => crate::resolver::resolve_project_change(candidates, &remaining, rules),
            None => crate::resolver::resolve_unity_migration_packages(candidates, &remaining),
        };
        let solved = match result {
            Ok(solved) => solved,
            Err(error) => {
                let id = match &error {
                    ResolveError::PackageNotFound { package_id }
                    | ResolveError::DependencyMissing { package_id }
                    | ResolveError::DependencyConflict { package_id } => package_id,
                    _ => return Err(map_resolve_error(error)),
                };
                if protected.contains(id)
                    || !remaining.iter().any(|request| &request.package_id == id)
                {
                    return Err(map_resolve_error(error));
                }
                omitted.insert(id.clone());
                remaining.retain(|request| &request.package_id != id);
                first_error.get_or_insert(error);
                if remaining.is_empty() {
                    return Err(map_resolve_error(first_error.expect("a root failed")));
                }
                continue;
            }
        };
        let mut obsolete = BTreeSet::new();
        for package in &solved.resolution.packages {
            if solved
                .legacy_cleanup_obligations
                .iter()
                .any(|item| item.package_id == package.package_id)
                || matches!(
                    package.source.authority,
                    crate::PackageSourceAuthority::UserPackage { .. }
                )
            {
                let metadata = cache
                    .pin_legacy_manifest(
                        &package.source,
                        &package.package_id,
                        &package.version.to_string(),
                    )
                    .await
                    .map_err(map_cache_error)?;
                if metadata.packages.contains(&package.package_id) {
                    return Err(M4Error::new(M4ErrorCode::PackageDependencyConflict));
                }
                obsolete.extend(metadata.packages);
            }
        }
        let conflicts = solved
            .resolution
            .packages
            .iter()
            .filter(|package| obsolete.contains(&package.package_id))
            .map(|package| package.package_id.clone())
            .collect::<BTreeSet<_>>();
        if conflicts.is_empty() {
            if !omitted.is_subset(&obsolete) {
                return Err(first_error.map_or_else(
                    || M4Error::new(M4ErrorCode::PackageDependencyConflict),
                    map_resolve_error,
                ));
            }
            return Ok(solved.resolution);
        }
        if conflicts.iter().any(|id| {
            protected.contains(id) || !remaining.iter().any(|request| &request.package_id == id)
        }) {
            return Err(M4Error::new(M4ErrorCode::PackageDependencyConflict));
        }
        remaining.retain(|request| !conflicts.contains(&request.package_id));
        omitted.extend(conflicts);
    }
}

async fn prepare_legacy_removals(
    cache: &PackageCache,
    snapshot: &crate::ProjectPackageSnapshot,
    catalog: &ResolverCatalog,
    mut draft: PackagePlanDraft,
) -> Result<PackagePlanDraft, M4Error> {
    let mut declarations = pinned_legacy_declarations(cache, catalog, &draft.source_set)?;
    let snapshot = snapshot.clone();
    tokio::task::spawn_blocking(move || {
        let installed = snapshot.installed_legacy(
            &draft.source_set,
            &draft.change_set.legacy_reinstall_packages,
        )?;
        exclude_installed_legacy_paths(&mut declarations, &installed)?;
        let obsolete = legacy_package_ids(&declarations)?;
        let removals = crate::legacy::collect_removals(&snapshot.root, &declarations)?;
        crate::plan::attach_legacy_removals(&snapshot, &mut draft, obsolete, removals)?;
        Ok(draft)
    })
    .await
    .map_err(|cause| M4Error::new(M4ErrorCode::Internal).with_source(cause))?
}

fn exclude_installed_legacy_paths(
    declarations: &mut BTreeMap<String, LegacyMetadata>,
    installed: &BTreeMap<String, LegacyMetadata>,
) -> Result<(), M4Error> {
    for (id, metadata) in declarations {
        let Some(old) = installed.get(id) else {
            continue;
        };
        for (paths, previous) in [
            (&mut metadata.files, &old.files),
            (&mut metadata.folders, &old.folders),
        ] {
            let previous = previous
                .keys()
                .map(|path| crate::legacy::removal_path(path))
                .collect::<Result<BTreeSet<_>, _>>()?;
            let mut new_paths = BTreeMap::new();
            for (path, guid) in std::mem::take(paths) {
                let path = crate::legacy::removal_path(&path)?;
                if !previous.contains(&path) {
                    new_paths.insert(path, guid);
                }
            }
            *paths = new_paths;
        }
    }
    Ok(())
}

fn merge_legacy(
    declarations: &mut BTreeMap<String, LegacyMetadata>,
    id: &str,
    metadata: LegacyMetadata,
) -> Result<(), M4Error> {
    let combined = declarations.entry(id.to_owned()).or_default();
    for (target, incoming) in [
        (&mut combined.files, metadata.files),
        (&mut combined.folders, metadata.folders),
    ] {
        for (path, guid) in incoming {
            let path = crate::legacy::removal_path(&path)?;
            let previous = target.entry(path).or_default();
            if previous.is_empty() {
                *previous = guid;
            } else if !guid.is_empty() && *previous != guid {
                return Err(M4Error::new(M4ErrorCode::PackageLegacyCleanupRequired));
            }
        }
    }
    combined.packages.extend(metadata.packages);
    Ok(())
}

fn legacy_package_ids(
    declarations: &BTreeMap<String, LegacyMetadata>,
) -> Result<BTreeSet<String>, M4Error> {
    let mut obsolete = BTreeSet::new();
    for (source, metadata) in declarations {
        for id in &metadata.packages {
            if source.eq_ignore_ascii_case(id) {
                return Err(M4Error::new(M4ErrorCode::PackageDependencyConflict));
            }
            obsolete.insert(id.clone());
        }
    }
    Ok(obsolete)
}

/// Resolve unknown archive digests only after the user has accepted the plan.
/// The immutable plan retains its original source observation; the existing
/// source pin store binds every observed digest to that exact observation.
async fn pin_apply_archives(
    cache: &PackageCache,
    store: &impl M4Store,
    owner: &PrincipalId,
    settings: alcomd_application::ConfigPackageSettings,
    mut sources: Vec<alcomd_application::PackageSourcePin>,
) -> Result<Vec<alcomd_application::PackageSourcePin>, M4Error> {
    if sources
        .iter()
        .any(|source| source.archive_sha256().is_none())
    {
        let catalog = store
            .resolver_catalog(owner.clone(), false, settings)
            .await?;
        for source in &mut sources {
            let alcomd_application::PackageSourcePin::Repository(pin) = source else {
                continue;
            };
            if pin.archive_sha256.is_some() {
                continue;
            }
            // A prior confirmed execution may already have acquired this source.
            // Reuse its durable digest on retry/restart without rewriting Plan.
            let row = catalog.entries.iter().find(|row| row.package_id == pin.package_id && row.version == pin.version
                && matches!(&row.source, alcomd_application::ResolverCatalogSource::Repository {
                    repository_id, repository_revision, source_identity, manifest_fingerprint, artifact_url, ..
                } if repository_id == &pin.repository_id && repository_revision == &pin.repository_revision
                    && source_identity == &pin.source_identity && manifest_fingerprint == &pin.manifest_fingerprint
                    && artifact_url == &pin.artifact_url))
                .ok_or_else(|| M4Error::with_subreason(M4ErrorCode::PlanStale, "source_revision_changed"))?;
            if let alcomd_application::ResolverCatalogSource::Repository {
                local_zip_sha256, ..
            } = &row.source
            {
                pin.archive_sha256 = *local_zip_sha256;
            }
        }
    }
    let acquired = run_ordered_jobs(sources, PACKAGE_DOWNLOAD_CONCURRENCY, |mut source| {
        let cache = cache.clone();
        async move {
            let mut observed = false;
            if let alcomd_application::PackageSourcePin::Repository(pin) = &mut source
                && pin.archive_sha256.is_none()
            {
                let digest = cache
                    .download_first(
                        &pin.artifact_url,
                        &pin.package_id,
                        &pin.version,
                        crate::cache::ArchiveHeaders {
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
                    )
                    .await
                    .map_err(map_cache_error)?;
                pin.archive_sha256 = Some(digest);
                observed = true;
            }
            Ok((source, observed))
        }
    })
    .await?;
    let mut sources = Vec::with_capacity(acquired.len());
    for (source, observed) in acquired {
        if observed && let alcomd_application::PackageSourcePin::Repository(pin) = &source {
            store
                .pin_repository_archive(owner.clone(), pin.clone())
                .await?;
        }
        sources.push(source);
    }
    Ok(sources)
}

pub(crate) async fn pin_resolution_archives(
    cache: &PackageCache,
    store: &impl RepositoryArchivePinStore,
    owner: &PrincipalId,
    resolution: &mut crate::Resolution,
) -> Result<(), M4Error> {
    let mut pending = Vec::new();
    for (index, package) in resolution.packages.iter().enumerate() {
        if package.source.archive_sha256.is_some() {
            continue;
        }
        pending.push((index, package.clone()));
    }
    let downloaded = run_ordered_jobs(pending, PACKAGE_DOWNLOAD_CONCURRENCY, |(index, package)| {
        let cache = cache.clone();
        async move {
            let download_headers = package.source.download_headers();
            let crate::PackageSourceAuthority::Repository {
                repository_id,
                repository_revision,
                artifact_url,
                credential_reference,
                ..
            } = package.source.authority
            else {
                return Err(M4Error::new(M4ErrorCode::PackageHashRequired));
            };
            let version = package.version.to_string();
            let archive_sha256 = cache
                .download_first(
                    &artifact_url,
                    &package.package_id,
                    &version,
                    crate::cache::ArchiveHeaders {
                        repository: credential_reference.as_deref(),
                        package: download_headers.as_ref().map(|headers| {
                            crate::cache::PackageHeaderLookup {
                                source_identity: &package.source.source_identity,
                                local_path: headers.local_path.as_deref(),
                                package_id: &package.package_id,
                                version: &version,
                                fingerprint: &package.source.manifest_fingerprint,
                            }
                        }),
                    },
                )
                .await
                .map_err(map_cache_error)?;
            Ok((
                index,
                RepositoryPackageSourcePin {
                    repository_id,
                    repository_revision,
                    source_identity: package.source.source_identity,
                    manifest_fingerprint: package.source.manifest_fingerprint,
                    package_id: package.package_id,
                    version,
                    artifact_url,
                    credential_reference,
                    download_headers,
                    archive_sha256: Some(archive_sha256),
                },
            ))
        }
    })
    .await?;
    for (index, pin) in downloaded {
        let digest = pin.archive_sha256;
        store.pin_repository_archive(owner.clone(), pin).await?;
        resolution.packages[index].source.archive_sha256 = digest;
    }
    Ok(())
}

fn extracted_legacy(extracted: &Path) -> Result<LegacyMetadata, M4Error> {
    let bytes = read_regular_file(&extracted.join("package.json"), 4 * 1024 * 1024)?;
    let manifest: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|cause| M4Error::new(M4ErrorCode::PackageManifestInvalid).with_source(cause))?;
    LegacyMetadata::parse(
        manifest
            .as_object()
            .ok_or_else(|| M4Error::new(M4ErrorCode::PackageManifestInvalid))?,
    )
    .map_err(|cause| M4Error::new(M4ErrorCode::PackageManifestInvalid).with_source(cause))
}

fn verify_archive_plan_metadata(
    payload_root: &Path,
    catalog: &ResolverCatalog,
    plan: &PackagePlanRecord,
) -> Result<(), M4Error> {
    for source in plan
        .change_set
        .mutations
        .iter()
        .filter_map(|mutation| mutation.source.as_ref())
    {
        let id = source.package_id();
        let bytes = read_regular_file(
            &payload_root.join(id).join("package.json"),
            crate::PROJECT_MANIFEST_LIMIT,
        )?;
        let manifest =
            crate::parse_bounded_json(&bytes, crate::M3ErrorCode::RepositoryDocumentInvalid)
                .map_err(|cause| {
                    M4Error::new(M4ErrorCode::PackageManifestInvalid).with_source(cause)
                })?;
        let object = manifest
            .as_object()
            .ok_or_else(|| M4Error::new(M4ErrorCode::PackageManifestInvalid))?;
        if let alcomd_application::PackageSourcePin::Repository(pin) = source {
            let row = catalog.entries.iter().find(|row| row.package_id == id && row.version == source.version()
                && matches!(&row.source, alcomd_application::ResolverCatalogSource::Repository { repository_id, manifest_fingerprint, .. }
                    if repository_id == &pin.repository_id && manifest_fingerprint == &pin.manifest_fingerprint))
                .ok_or_else(|| M4Error::with_subreason(M4ErrorCode::PlanStale, "source_revision_changed"))?;
            let expected_dependencies: serde_json::Value =
                serde_json::from_str(&row.dependencies_json).map_err(|cause| {
                    M4Error::new(M4ErrorCode::PackageManifestInvalid).with_source(cause)
                })?;
            let actual_dependencies = object
                .get("vpmDependencies")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            if actual_dependencies != expected_dependencies
                || object.get("unity").and_then(serde_json::Value::as_str) != row.unity.as_deref()
                || object
                    .get("unityRelease")
                    .and_then(serde_json::Value::as_str)
                    != row.unity_release.as_deref()
            {
                return stale("manifest_fingerprint_changed");
            }
        }
    }
    Ok(())
}

fn pinned_legacy_declarations(
    cache: &PackageCache,
    catalog: &ResolverCatalog,
    sources: &[alcomd_application::PackageSourcePin],
) -> Result<BTreeMap<String, LegacyMetadata>, M4Error> {
    let mut declarations = BTreeMap::new();
    for source in sources {
        let fingerprint = match source {
            alcomd_application::PackageSourcePin::Repository(pin) => pin.manifest_fingerprint,
            alcomd_application::PackageSourcePin::UserPackage(pin) => pin.manifest_fingerprint,
        };
        let requires_pin = source.is_user_package()
            || catalog.entries.iter().any(|entry| {
                entry.legacy_metadata_present
                    && entry.package_id == source.package_id()
                    && entry.version == source.version()
                    && match (&entry.source, source) {
                        (
                            alcomd_application::ResolverCatalogSource::Repository {
                                repository_id,
                                manifest_fingerprint,
                                ..
                            },
                            alcomd_application::PackageSourcePin::Repository(pin),
                        ) => {
                            repository_id == &pin.repository_id
                                && manifest_fingerprint == &pin.manifest_fingerprint
                        }
                        _ => false,
                    }
            });
        if requires_pin {
            let metadata = cache
                .read_pinned_legacy(&fingerprint)
                .map_err(map_cache_error)?;
            declarations.insert(source.package_id().to_owned(), metadata);
        }
    }
    Ok(declarations)
}

fn verify_plan_legacy(
    cache: &PackageCache,
    root: &Path,
    payload_root: &Path,
    catalog: &ResolverCatalog,
    plan: &PackagePlanRecord,
    installed: &BTreeMap<String, LegacyMetadata>,
    after_replace: bool,
) -> Result<(), M4Error> {
    if plan.change_set.format_version < 3 {
        return Ok(());
    }
    verify_archive_plan_metadata(payload_root, catalog, plan)?;
    let mut declarations = pinned_legacy_declarations(cache, catalog, &plan.source_set)?;
    for mutation in &plan.change_set.mutations {
        if mutation.source.is_some() {
            let metadata = extracted_legacy(&payload_root.join(&mutation.package_id))?;
            merge_legacy(&mut declarations, &mutation.package_id, metadata)?;
        }
    }
    exclude_installed_legacy_paths(&mut declarations, installed)?;
    if plan.change_set.format_version == 3 {
        crate::legacy::verify_all_absent(root, declarations.values())?;
    }
    if plan.change_set.format_version == 4 {
        let actual = crate::legacy::collect_removals(root, &declarations)?;
        let expected = if after_replace {
            &[][..]
        } else {
            &plan.change_set.legacy_removals[..]
        };
        if actual != expected
            || !legacy_package_ids(&declarations)?.into_iter().eq(plan
                .change_set
                .legacy_packages
                .iter()
                .cloned())
        {
            return stale("project_snapshot_changed");
        }
    }
    Ok(())
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
                std::fs::create_dir(&current).map_err(|cause| {
                    M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause)
                })?;
                let metadata = std::fs::symlink_metadata(&current).map_err(|cause| {
                    M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause)
                })?;
                if !metadata.is_dir() || is_link_or_reparse(&metadata) {
                    return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
                }
            }
            Err(cause) => {
                return Err(M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause));
            }
        }
    }
    Ok(())
}

fn replace_packages(
    packages_root: &Path,
    staging: &Path,
    backup: &Path,
    attempt: &Path,
    plan: &PackagePlanRecord,
) -> Result<(), M4Error> {
    for mutation in &plan.change_set.mutations {
        let target = packages_root.join(&mutation.package_id);
        if mutation.kind == PackageMutationKind::Remove
            && (plan
                .change_set
                .legacy_packages
                .contains(&mutation.package_id)
                || plan
                    .change_set
                    .absent_removal_packages
                    .contains(&mutation.package_id))
        {
            require_absent(&target)?;
            continue;
        }
        let backup_target = backup.join(&mutation.package_id);
        match std::fs::symlink_metadata(&target) {
            Ok(_) if mutation.kind == PackageMutationKind::Install => {
                return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply));
            }
            Ok(metadata) if metadata.is_dir() && !is_link_or_reparse(&metadata) => {
                std::fs::rename(&target, &backup_target).map_err(|cause| {
                    M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause)
                })?;
                sync_directories(&[packages_root, backup])?;
                test_kill_gate(attempt, "old_package_backed_up")?;
            }
            Ok(_) => return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if mutation.kind != PackageMutationKind::Install {
                    return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply));
                }
            }
            Err(cause) => {
                return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply).with_source(cause));
            }
        }
        if mutation.kind != PackageMutationKind::Remove {
            std::fs::rename(staging.join(&mutation.package_id), &target)
                .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
            sync_directories(&[packages_root, staging])?;
            test_kill_gate(attempt, "new_package_published")?;
        }
        alcomd_platform::sync_directory(packages_root)
            .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
    }
    Ok(())
}

#[cfg(feature = "test-kill-gates")]
fn test_kill_gate(attempt: &Path, checkpoint: &str) -> Result<(), M4Error> {
    if std::env::var("ALCOMD_TEST_M4_KILL_GATE").as_deref() != Ok(checkpoint) {
        return Ok(());
    }
    write_new_file(
        &attempt.join(format!("test-kill-gate-{checkpoint}.json")),
        format!("{{\"checkpoint\":\"{checkpoint}\"}}\n").as_bytes(),
    )?;
    alcomd_platform::sync_directory(attempt).map_err(|cause| {
        M4Error::new(M4ErrorCode::RecoveryRequired)
            .with_source(cause)
            .with_path(attempt.display().to_string())
    })?;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}

#[cfg(not(feature = "test-kill-gates"))]
fn test_kill_gate(_attempt: &Path, _checkpoint: &str) -> Result<(), M4Error> {
    Ok(())
}

fn commit_manifest(manifest_path: &Path, attempt: &Path) -> Result<(), M4Error> {
    let committed_old = attempt.join("vpm-manifest.commit-old");
    std::fs::rename(manifest_path, &committed_old)
        .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
    if std::fs::rename(attempt.join("vpm-manifest.new"), manifest_path).is_err() {
        let _ = std::fs::rename(&committed_old, manifest_path);
        return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
    }
    alcomd_platform::sync_directory(
        manifest_path
            .parent()
            .ok_or_else(|| M4Error::new(M4ErrorCode::Internal))?,
    )
    .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))
}

fn legacy_vault(attempt: &Path) -> PathBuf {
    attempt.join("legacy-backup")
}

fn checked_boundary(root: &Path, path: &Path) -> Result<(), M4Error> {
    if root != path {
        crate::legacy::verify_boundary(
            root,
            path.strip_prefix(root)
                .map_err(|_| M4Error::new(M4ErrorCode::RecoveryRequired))?,
        )?;
    }
    Ok(())
}

fn require_absent(path: &Path) -> Result<(), M4Error> {
    match std::fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        _ => Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply)),
    }
}

fn quarantine_legacy(root: &Path, attempt: &Path, plan: &PackagePlanRecord) -> Result<(), M4Error> {
    if plan.change_set.legacy_removals.is_empty() {
        return Ok(());
    }
    checked_boundary(root, attempt)?;
    let vault = legacy_vault(attempt);
    std::fs::create_dir(&vault)
        .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
    sync_directories(&[attempt])?;
    for (index, removal) in plan.change_set.legacy_removals.iter().enumerate() {
        let source = root.join(crate::legacy::removal_path(&removal.path)?);
        let parent = source
            .parent()
            .ok_or_else(|| M4Error::new(M4ErrorCode::Internal))?;
        checked_boundary(root, &source)?;
        crate::legacy::verify_removal(&source, removal)?;
        let slot = vault.join(index.to_string());
        alcomd_platform::publish_path_no_replace(&source, &slot)
            .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
        sync_directories(&[parent, &vault])?;
        if crate::legacy::verify_removal(&slot, removal).is_err() {
            // A concurrent writer changed what the rename moved. Restore it without
            // replacing anything at the original path, then reject the stale plan.
            checked_boundary(root, &slot).map_err(legacy_recovery_error)?;
            checked_boundary(root, parent).map_err(legacy_recovery_error)?;
            alcomd_platform::publish_path_no_replace(&slot, &source)
                .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
            sync_directories(&[parent, &vault])?;
            return stale("project_snapshot_changed");
        }
        test_kill_gate(attempt, "legacy_asset_quarantined")?;
    }
    Ok(())
}

fn restore_legacy(root: &Path, attempt: &Path, plan: &PackagePlanRecord) -> Result<(), M4Error> {
    if plan.change_set.legacy_removals.is_empty() {
        return Ok(());
    }
    let vault = legacy_vault(attempt);
    if require_absent(&vault).is_ok() {
        return Ok(());
    }
    checked_boundary(root, &vault).map_err(legacy_recovery_error)?;
    for (index, removal) in plan.change_set.legacy_removals.iter().enumerate().rev() {
        let slot = vault.join(index.to_string());
        if require_absent(&slot).is_ok() {
            continue;
        }
        checked_boundary(root, &slot).map_err(legacy_recovery_error)?;
        crate::legacy::verify_removal(&slot, removal).map_err(legacy_recovery_error)?;
        let target =
            root.join(crate::legacy::removal_path(&removal.path).map_err(legacy_recovery_error)?);
        let parent = target
            .parent()
            .ok_or_else(|| M4Error::new(M4ErrorCode::RecoveryRequired))?;
        checked_boundary(root, parent).map_err(legacy_recovery_error)?;
        alcomd_platform::publish_path_no_replace(&slot, &target)
            .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
        sync_directories(&[parent, &vault])?;
    }
    Ok(())
}

fn cleanup_legacy(root: &Path, attempt: &Path, plan: &PackagePlanRecord) -> Result<(), M4Error> {
    if plan.change_set.legacy_removals.is_empty() {
        return Ok(());
    }
    let vault = legacy_vault(attempt);
    if require_absent(&vault).is_ok() {
        return Ok(());
    }
    checked_boundary(root, &vault).map_err(legacy_recovery_error)?;
    let recovery =
        |cause: std::io::Error| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause);
    for (index, removal) in plan.change_set.legacy_removals.iter().enumerate() {
        let slot = vault.join(index.to_string());
        if require_absent(&slot).is_ok() {
            continue;
        }
        checked_boundary(root, &slot).map_err(legacy_recovery_error)?;
        if alcomd_platform::file_identity_key(&slot).map_err(recovery)? != removal.identity {
            return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
        }
        match removal.kind {
            alcomd_application::LegacyRemovalKind::File => {
                std::fs::remove_file(&slot).map_err(recovery)?
            }
            alcomd_application::LegacyRemovalKind::Directory => {
                alcomd_platform::project_delete_cleanup(&slot).map_err(|cause| {
                    M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause)
                })?;
            }
        }
        sync_directories(&[&vault])?;
    }
    std::fs::remove_dir(&vault).map_err(recovery)?;
    sync_directories(&[attempt])
}

fn legacy_recovery_error(cause: M4Error) -> M4Error {
    M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause)
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
    for removal in &plan.change_set.legacy_removals {
        require_absent(&root.join(crate::legacy::removal_path(&removal.path)?))?;
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
            .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
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
        .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
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
        std::fs::create_dir(&rolled)
            .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
    }
    let committed_old = attempt.join("vpm-manifest.commit-old");
    if committed_old.exists() {
        let rolled_manifest = rolled.join("vpm-manifest.new");
        if manifest_path.exists() && !rolled_manifest.exists() {
            std::fs::rename(manifest_path, &rolled_manifest)
                .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
        }
        if manifest_path.exists() {
            return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
        }
        std::fs::rename(&committed_old, manifest_path)
            .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
    }
    let backup = attempt.join("backup");
    for mutation in plan.change_set.mutations.iter().rev() {
        let target = packages_root.join(&mutation.package_id);
        let old = backup.join(&mutation.package_id);
        let rolled_target = rolled.join(&mutation.package_id);
        if mutation.kind == PackageMutationKind::Install {
            let staged = attempt.join("staging").join(&mutation.package_id);
            if !staged.exists() && target.exists() && !rolled_target.exists() {
                std::fs::rename(&target, &rolled_target).map_err(|cause| {
                    M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause)
                })?;
            }
        } else if old.exists() {
            if target.exists() && !rolled_target.exists() {
                std::fs::rename(&target, &rolled_target).map_err(|cause| {
                    M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause)
                })?;
            }
            if target.exists() {
                return Err(M4Error::new(M4ErrorCode::RecoveryRequired));
            }
            std::fs::rename(&old, &target)
                .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
        }
    }
    restore_legacy(
        packages_root
            .parent()
            .ok_or_else(|| M4Error::new(M4ErrorCode::RecoveryRequired))?,
        attempt,
        plan,
    )?;
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
    let bytes = serde_json::to_vec(marker).map_err(|cause| {
        M4Error::new(M4ErrorCode::Internal)
            .with_source(cause)
            .with_path(attempt.display().to_string())
    })?;
    let marker_path = attempt.join(format!("marker-{step:020}.json"));
    write_new_file(&marker_path, &bytes)?;
    alcomd_platform::sync_directory(attempt).map_err(|cause| {
        M4Error::new(M4ErrorCode::RecoveryRequired)
            .with_source(cause)
            .with_path(attempt.display().to_string())
    })
}

fn read_marker(attempt: &Path) -> Result<AttemptMarker, M4Error> {
    let marker_path = std::fs::read_dir(attempt)
        .map_err(|cause| {
            M4Error::new(M4ErrorCode::RecoveryRequired)
                .with_source(cause)
                .with_path(attempt.display().to_string())
        })?
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
    let marker: AttemptMarker = serde_json::from_slice(&bytes).map_err(|cause| {
        M4Error::new(M4ErrorCode::RecoveryRequired)
            .with_source(cause)
            .with_path(attempt.display().to_string())
    })?;
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
        .map_err(|cause| {
            M4Error::new(M4ErrorCode::RecoveryRequired)
                .with_source(cause)
                .with_path(path.display().to_string())
        })?;
    file.write_all(bytes)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
        .map_err(|cause| {
            M4Error::new(M4ErrorCode::RecoveryRequired)
                .with_source(cause)
                .with_path(path.display().to_string())
        })
}

fn read_regular_file(path: &Path, limit: usize) -> Result<Vec<u8>, M4Error> {
    let metadata = std::fs::symlink_metadata(path).map_err(|cause| {
        M4Error::new(M4ErrorCode::ProjectChangedDuringApply)
            .with_source(cause)
            .with_path(path.display().to_string())
    })?;
    if !metadata.is_file() || is_link_or_reparse(&metadata) || metadata.len() > limit as u64 {
        return Err(M4Error::new(M4ErrorCode::ProjectChangedDuringApply));
    }
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(limit as u64 + 1).read_to_end(&mut bytes))
        .map_err(|cause| {
            M4Error::new(M4ErrorCode::ProjectChangedDuringApply)
                .with_source(cause)
                .with_path(path.display().to_string())
        })?;
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
            .map_err(|cause| M4Error::new(M4ErrorCode::RecoveryRequired).with_source(cause))?;
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

fn stale<T>(reason: &str) -> Result<T, M4Error> {
    Err(M4Error::with_subreason(M4ErrorCode::PlanStale, reason))
}

fn map_resolve_error(error: ResolveError) -> M4Error {
    (match error {
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
    })
    .with_source(error)
}

pub(crate) fn map_cache_error(error: crate::CacheError) -> M4Error {
    use crate::CacheErrorCode as C;
    (match error.code() {
        C::Corrupt => M4Error::new(M4ErrorCode::PackageCacheCorrupt),
        C::IntegrityMismatch => M4Error::new(M4ErrorCode::PackageIntegrityMismatch),
        C::DownloadTooLarge => M4Error::new(M4ErrorCode::PackageDownloadTooLarge),
        C::OfflineMiss => M4Error::new(M4ErrorCode::OfflineCacheMiss),
        C::QuotaExceeded => M4Error::new(M4ErrorCode::PackageCacheQuotaExceeded),
        C::InvalidDigest => M4Error::new(M4ErrorCode::PackageHashRequired),
        C::InvalidUrl => M4Error::new(M4ErrorCode::PackageManifestInvalid),
        C::DownloadFailed | C::Io => M4Error::new(M4ErrorCode::Internal),
    })
    .with_source(error)
}

pub(crate) fn map_archive_error(error: crate::ArchiveError) -> M4Error {
    use crate::ArchiveErrorCode as C;
    (match error.code() {
        C::UnsupportedCompression => {
            M4Error::new(M4ErrorCode::PackageArchiveUnsupportedCompression)
        }
        C::UnsafePath | C::LinkOrSpecialFile => M4Error::new(M4ErrorCode::PackageArchiveUnsafePath),
        C::PathCollision => M4Error::new(M4ErrorCode::PackagePathCollision),
        C::QuotaExceeded => M4Error::new(M4ErrorCode::PackageArchiveLimitExceeded),
        C::Invalid | C::Encrypted => M4Error::new(M4ErrorCode::PackageArchiveInvalid),
        C::Io => M4Error::new(M4ErrorCode::Internal),
    })
    .with_source(error)
}

fn time_ms() -> Result<u64, M4Error> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|cause| M4Error::new(M4ErrorCode::Internal).with_source(cause))
        .and_then(|duration| {
            u64::try_from(duration.as_millis())
                .map_err(|cause| M4Error::new(M4ErrorCode::Internal).with_source(cause))
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

    #[test]
    fn package_snapshot_reads_metadata_without_opening_payload_files() {
        let installed = vec![("com.example.fixture".to_owned(), "1.0.0".to_owned())];
        let (root, project, _) = planning_fixture(&installed, &installed);
        let folder = root.join("Packages/com.example.fixture");
        std::fs::create_dir_all(folder.join("Runtime")).expect("package folder");
        let manifest = folder.join("package.json");
        let metadata = br#"{"name":"com.example.fixture","version":"1.0.0"}"#;
        std::fs::write(&manifest, metadata).expect("package metadata");
        let payload = folder.join("Runtime/payload.bin");
        std::fs::write(&payload, b"first payload").expect("payload");
        let before = inspect_package_project(&project).expect("metadata snapshot");
        let migration_before = crate::plan::inspect_migration_package_project(&project)
            .expect("migration still freezes payload contents");
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let _exclusive = std::fs::OpenOptions::new()
                .read(true)
                .share_mode(0)
                .open(&payload)
                .expect("payload denies every other reader");
            assert_eq!(
                inspect_package_project(&project)
                    .expect("payload is not opened")
                    .fingerprint,
                before.fingerprint
            );
        }
        #[cfg(unix)]
        {
            let link = folder.join("Runtime/not-a-planning-input");
            std::os::unix::fs::symlink("missing-payload", &link).expect("unreadable payload entry");
            assert_eq!(
                inspect_package_project(&project)
                    .expect("payload is not traversed")
                    .fingerprint,
                before.fingerprint
            );
            std::fs::remove_file(link).expect("remove link");
        }
        std::fs::write(
            &payload,
            b"payload changed independently of package metadata",
        )
        .expect("edit payload");
        assert_ne!(
            crate::plan::inspect_migration_package_project(&project)
                .expect("migration detects payload edit")
                .fingerprint,
            migration_before.fingerprint
        );
        assert_eq!(
            inspect_package_project(&project)
                .expect("payload edit")
                .fingerprint,
            before.fingerprint
        );
        std::fs::write(
            &manifest,
            br#"{"name":"com.example.fixture","version":"2.0.0"}"#,
        )
        .expect("edit metadata");
        assert_ne!(
            inspect_package_project(&project)
                .expect("metadata edit")
                .fingerprint,
            before.fingerprint
        );
        std::fs::write(&manifest, metadata).expect("restore metadata");
        std::fs::rename(&folder, root.join("old-package")).expect("replace directory identity");
        std::fs::create_dir(&folder).expect("replacement directory");
        std::fs::write(&manifest, metadata).expect("same metadata in replacement");
        assert_ne!(
            inspect_package_project(&project)
                .expect("directory replacement")
                .fingerprint,
            before.fingerprint
        );
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[tokio::test]
    async fn old_unaccepted_plan_is_stale_before_filesystem_or_source_reads() {
        let mut old = plan(PackageMutationKind::Install);
        old.state = PlanState::Unapplied;
        let project = project_with_locked_version("1.0.0");
        let error = revalidate(
            &VpmReader::new().expect("reader"),
            &project,
            &ResolverCatalog {
                entries: Vec::new(),
                complete: true,
            },
            &old,
        )
        .await
        .expect_err("old basis is stale");
        assert_eq!(error.code(), M4ErrorCode::PlanStale);
        assert_eq!(error.subreason(), Some("implementation_basis_changed"));
    }
    use alcomd_application::{
        DependencyIdentity, ManifestState, PackageChangeSet, PackageMutation, PackagePlanRecord,
        PackagePlanVersion, PlanState, PrincipalId, ProjectId, ProjectObservation, ProjectType,
        ResolverCatalogEntry, ResolverCatalogSource, Revision,
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
    fn recovery_restores_a_partially_quarantined_plan_and_is_repeatable() {
        let root = temporary_root("legacy-partial");
        let attempt = root.join("Library/ALCOMD/transactions/operation/attempt-1");
        let vault = legacy_vault(&attempt);
        let assets = root.join("Assets");
        let packages = root.join("Packages");
        std::fs::create_dir_all(&vault).expect("create quarantine fixture");
        std::fs::create_dir(&assets).expect("create assets fixture");
        std::fs::create_dir(&packages).expect("create packages fixture");
        for name in ["first.txt", "second.txt"] {
            std::fs::write(assets.join(name), name.as_bytes()).expect("seed a legacy asset");
        }
        let mut plan = plan(PackageMutationKind::Install);
        plan.change_set.format_version = 4;
        plan.change_set.mutations.clear();
        plan.change_set.legacy_removals = crate::legacy::collect_removals(
            &root,
            &BTreeMap::from([(
                "com.example.fixture".to_owned(),
                LegacyMetadata {
                    files: BTreeMap::from([
                        ("Assets/first.txt".to_owned(), String::new()),
                        ("Assets/second.txt".to_owned(), String::new()),
                    ]),
                    ..Default::default()
                },
            )]),
        )
        .expect("freeze both legacy assets");
        // Model an interrupted Apply after the first atomic quarantine move.
        alcomd_platform::publish_path_no_replace(&assets.join("first.txt"), &vault.join("0"))
            .expect("simulate the first quarantine move");
        for _ in 0..2 {
            rollback_attempt(
                &packages,
                &packages.join("vpm-manifest.json"),
                &attempt,
                &plan,
            )
            .expect("restore the partially quarantined plan");
            assert_eq!(
                std::fs::read(assets.join("first.txt")).expect("read restored asset"),
                b"first.txt"
            );
            assert_eq!(
                std::fs::read(assets.join("second.txt")).expect("read untouched asset"),
                b"second.txt"
            );
        }
        std::fs::remove_dir_all(&root).expect("clean the owned fixture");
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

    #[test]
    fn unlocked_requirements_protect_installed_versions_and_payloads_are_not_read() {
        for (installed_version, selected_version, range) in
            [("1.0.0", "2.0.0", "^1.0.0"), ("2.0.0", "1.0.0", ">=2.0.0")]
        {
            let installed = [(
                "com.example.fixture".to_owned(),
                installed_version.to_owned(),
            )];
            let (root, project, _) = planning_fixture(&installed, &installed);
            let local = root.join("Packages/com.example.local");
            std::fs::create_dir(&local).expect("owned local package");
            std::fs::write(local.join("package.json"), format!(r#"{{"name":"com.example.local","version":"1.0.0","vpmDependencies":{{"com.example.fixture":"{range}"}}}}"#)).expect("local requirements");
            std::fs::write(local.join("payload.bin"), b"not a planning input").expect("payload");
            let snapshot = inspect_package_project(&project).expect("metadata snapshot");
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                let _held = std::fs::OpenOptions::new()
                    .read(true)
                    .share_mode(0)
                    .open(local.join("payload.bin"))
                    .expect("deny payload access");
                assert_eq!(
                    inspect_package_project(&project)
                        .expect("no payload reads")
                        .fingerprint,
                    snapshot.fingerprint
                );
            }
            let rules = snapshot
                .resolution_rules(BTreeSet::from(["com.example.fixture".into()]), true)
                .expect("rules include local requirements");
            let candidates =
                candidates_from_catalog(&catalog(&[("com.example.fixture", selected_version)]))
                    .expect("candidate");
            let error = crate::resolver::resolve_project_change(
                &candidates,
                &[ResolveRequest {
                    allow_unity_incompatible: false,
                    package_id: "com.example.fixture".into(),
                    range: format!("={selected_version}"),
                    source: None,
                    include_prerelease: false,
                    unity_version: None,
                }],
                &rules,
            )
            .expect_err("local package blocks an incompatible selected version");
            assert!(matches!(error, ResolveError::DependencyConflict { .. }));
            std::fs::remove_dir_all(root).expect("owned fixture cleanup");
        }
    }

    #[test]
    fn frozen_absent_removal_and_repeated_rollback_do_not_touch_a_later_directory() {
        let root = temporary_root("absent-removal-race");
        let packages = root.join("Packages");
        let attempt = root.join("attempt");
        let staging = attempt.join("staging");
        let backup = attempt.join("backup");
        for path in [&packages, &staging, &backup] {
            std::fs::create_dir_all(path).expect("owned directories");
        }
        let mut plan = plan(PackageMutationKind::Remove);
        plan.change_set.format_version = 4;
        plan.change_set.absent_removal_packages = vec!["com.example.fixture".into()];
        plan.change_set
            .validate_bounds()
            .expect("bound absent evidence");
        replace_packages(&packages, &staging, &backup, &attempt, &plan)
            .expect("frozen absent target needs no filesystem mutation");
        let later = packages.join("com.example.fixture");
        std::fs::create_dir(&later).expect("later external package");
        std::fs::write(later.join("sentinel"), b"preserve later data").expect("external content");
        assert!(replace_packages(&packages, &staging, &backup, &attempt, &plan).is_err());
        for _ in 0..2 {
            rollback_attempt(
                &packages,
                &packages.join("vpm-manifest.json"),
                &attempt,
                &plan,
            )
            .expect("idempotent rollback");
            assert_eq!(
                std::fs::read(later.join("sentinel")).expect("later package remains"),
                b"preserve later data"
            );
        }
        std::fs::remove_dir_all(root).expect("owned fixture cleanup");
    }

    #[tokio::test]
    async fn reinstall_targets_direct_transitive_and_all_without_changing_locked_versions() {
        let installed = [
            ("com.example.direct".to_owned(), "1.0.0".to_owned()),
            ("com.example.transitive".to_owned(), "2.0.0".to_owned()),
        ];
        let (root, project, snapshot) = planning_fixture(&installed[..1], &installed);
        let cache = PackageCache::new(root.join("owned-cache")).expect("cache");
        let catalog = catalog(&[
            ("com.example.direct", "1.0.0"),
            ("com.example.direct", "1.1.0"),
            ("com.example.transitive", "2.0.0"),
            ("com.example.transitive", "3.0.0"),
        ]);

        let mut request = package_request(PlanAction::Reinstall);
        request.plan_version = PackagePlanVersion::V2;
        request.reinstall_selection = Some(ReinstallSelection::Packages(vec![
            "com.example.transitive".to_owned(),
        ]));
        let transitive = prepare_reinstall(&cache, &snapshot, &project, &catalog, &request)
            .await
            .expect("reinstall transitive package");
        assert_eq!(transitive.change_set.mutations.len(), 1);
        assert_eq!(
            transitive.change_set.mutations[0].kind,
            PackageMutationKind::Replace
        );
        assert_eq!(
            transitive.change_set.mutations[0].from_version.as_deref(),
            Some("2.0.0")
        );
        assert_eq!(
            transitive.change_set.mutations[0].to_version.as_deref(),
            Some("2.0.0")
        );

        request.reinstall_selection = Some(ReinstallSelection::All);
        let all = prepare_reinstall(&cache, &snapshot, &project, &catalog, &request)
            .await
            .expect("reinstall all locked packages");
        assert_eq!(all.change_set.mutations.len(), 2);
        assert!(all.change_set.mutations.iter().all(|mutation| {
            mutation.kind == PackageMutationKind::Replace
                && mutation.from_version == mutation.to_version
        }));

        request.reinstall_selection = Some(ReinstallSelection::Packages(vec![
            "com.example.missing".to_owned(),
        ]));
        assert_eq!(
            prepare_reinstall(&cache, &snapshot, &project, &catalog, &request)
                .await
                .expect_err("uninstalled target must fail")
                .code(),
            M4ErrorCode::PackageNotInstalled
        );
        std::fs::remove_dir_all(root).expect("remove fixture");

        let (empty_root, empty_project, empty_snapshot) = planning_fixture(&[], &[]);
        request.reinstall_selection = Some(ReinstallSelection::All);
        let empty = prepare_reinstall(
            &cache,
            &empty_snapshot,
            &empty_project,
            &ResolverCatalog {
                entries: Vec::new(),
                complete: true,
            },
            &request,
        )
        .await
        .expect("empty all is a successful empty Plan");
        assert!(empty.change_set.mutations.is_empty());
        std::fs::remove_dir_all(empty_root).expect("remove empty fixture");
    }

    #[test]
    fn same_version_install_remains_noop_while_reinstall_forces_replace() {
        let installed = [("com.example.fixture".to_owned(), "1.0.0".to_owned())];
        let (root, _project, snapshot) = planning_fixture(&installed, &installed);
        let resolved = resolution("1.0.0");
        let install = build_resolution_plan(&snapshot, PlanAction::Install, &resolved)
            .expect("same-version install plan");
        assert!(install.change_set.mutations.is_empty());

        let reinstall = build_reinstall_plan(
            &snapshot,
            &resolved,
            &BTreeSet::from(["com.example.fixture".to_owned()]),
        )
        .expect("same-version reinstall plan");
        assert_eq!(reinstall.change_set.mutations.len(), 1);
        assert_eq!(
            reinstall.change_set.mutations[0].kind,
            PackageMutationKind::Replace
        );
        assert_eq!(
            reinstall.change_set.mutations[0].from_version,
            reinstall.change_set.mutations[0].to_version
        );
        std::fs::remove_dir_all(root).expect("remove fixture");
    }

    #[tokio::test]
    async fn bulk_transitive_reinstall_does_not_write_its_solver_pin_as_a_self_dependency() {
        let direct = [("com.example.alpha".to_owned(), "1.0.0".to_owned())];
        let locked = [
            direct[0].clone(),
            ("com.example.beta".to_owned(), "1.0.0".to_owned()),
        ];
        let (root, project, snapshot) = planning_fixture(&direct, &locked);
        let cache = PackageCache::new(root.join("owned-cache")).expect("cache");
        let mut catalog = catalog(&[
            ("com.example.alpha", "1.0.0"),
            ("com.example.beta", "1.0.0"),
        ]);
        catalog.entries[0].dependencies_json = r#"{"com.example.beta":"1.0.0"}"#.to_owned();
        let mut request = package_request(PlanAction::Bulk);
        request.plan_version = PackagePlanVersion::V2;
        request.bulk_intents = vec![PackageBulkIntent::Reinstall {
            package_id: "com.example.beta".to_owned(),
            source: None,
        }];
        let plan = prepare_bulk(&cache, &snapshot, &project, &catalog, &request)
            .await
            .expect("transitive reinstall");
        assert!(
            !plan
                .change_set
                .dependency_edges
                .iter()
                .any(|edge| !edge.direct && edge.from_package_id == edge.to_package_id)
        );
        let before = std::fs::read(root.join("Packages/vpm-manifest.json")).expect("manifest");
        let after =
            crate::materialize_vpm_manifest(&before, &plan.change_set).expect("frozen output");
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(&after)),
            plan.change_set.vpm_manifest_sha256
        );
        let after: serde_json::Value = serde_json::from_slice(&after).expect("JSON");
        assert_eq!(
            after["locked"]["com.example.beta"]["dependencies"],
            serde_json::json!({})
        );
        std::fs::remove_dir_all(root).expect("fixture cleanup");
    }

    #[tokio::test]
    async fn bulk_supports_all_four_intents_in_one_canonical_atomic_plan() {
        let direct = [
            ("com.example.alpha".to_owned(), "1.0.0".to_owned()),
            ("com.example.beta".to_owned(), "1.0.0".to_owned()),
            ("com.example.delta".to_owned(), "1.0.0".to_owned()),
        ];
        let (root, project, snapshot) = planning_fixture(&direct, &direct);
        let cache = PackageCache::new(root.join("owned-cache")).expect("cache");
        let catalog = catalog(&[
            ("com.example.alpha", "1.0.0"),
            ("com.example.alpha", "1.1.0"),
            ("com.example.beta", "1.0.0"),
            ("com.example.gamma", "1.0.0"),
        ]);
        let mut request = package_request(PlanAction::Bulk);
        request.plan_version = PackagePlanVersion::V2;
        request.bulk_intents = vec![
            PackageBulkIntent::Reinstall {
                package_id: "com.example.beta".to_owned(),
                source: None,
            },
            PackageBulkIntent::Install {
                package_id: "com.example.gamma".to_owned(),
                version_range: Some("=1.0.0".to_owned()),
                source: None,
                include_prerelease: false,
            },
            PackageBulkIntent::Remove {
                package_id: "com.example.delta".to_owned(),
            },
            PackageBulkIntent::Upgrade {
                package_id: "com.example.alpha".to_owned(),
                version_range: Some("=1.1.0".to_owned()),
                source: None,
                include_prerelease: false,
            },
        ];

        let plan = prepare_bulk(&cache, &snapshot, &project, &catalog, &request)
            .await
            .expect("one atomic mixed plan");
        assert_eq!(plan.action, PlanAction::Bulk);
        assert_eq!(
            plan.change_set
                .mutations
                .iter()
                .map(|mutation| mutation.package_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "com.example.alpha",
                "com.example.beta",
                "com.example.delta",
                "com.example.gamma"
            ]
        );
        assert_eq!(
            plan.change_set
                .mutations
                .iter()
                .map(|mutation| mutation.kind)
                .collect::<Vec<_>>(),
            vec![
                PackageMutationKind::Replace,
                PackageMutationKind::Replace,
                PackageMutationKind::Remove,
                PackageMutationKind::Install
            ]
        );
        request.bulk_intents.reverse();
        let reversed = prepare_bulk(&cache, &snapshot, &project, &catalog, &request)
            .await
            .expect("caller order must not change the Plan");
        assert_eq!(reversed.change_set, plan.change_set);
        assert_eq!(reversed.change_set_fingerprint, plan.change_set_fingerprint);
        std::fs::remove_dir_all(root).expect("remove fixture");
    }

    #[tokio::test]
    async fn bulk_rejects_duplicate_intents_and_257_but_accepts_256() {
        let installed = (0..256)
            .map(|value| {
                (
                    format!("com.example.package-{value:03}"),
                    "1.0.0".to_owned(),
                )
            })
            .collect::<Vec<_>>();
        let (root, project, snapshot) = planning_fixture(&installed, &installed);
        let cache = PackageCache::new(root.join("owned-cache")).expect("cache");
        let mut request = package_request(PlanAction::Bulk);
        request.plan_version = PackagePlanVersion::V2;
        request.bulk_intents = installed
            .iter()
            .map(|(package_id, _)| PackageBulkIntent::Remove {
                package_id: package_id.clone(),
            })
            .collect();
        let plan = prepare_bulk(
            &cache,
            &snapshot,
            &project,
            &ResolverCatalog {
                entries: Vec::new(),
                complete: true,
            },
            &request,
        )
        .await
        .expect("256 intents are accepted");
        assert_eq!(plan.change_set.mutations.len(), 256);

        request.bulk_intents.push(PackageBulkIntent::Install {
            package_id: "com.example.too-many".to_owned(),
            version_range: None,
            source: None,
            include_prerelease: false,
        });
        let too_many = prepare_bulk(
            &cache,
            &snapshot,
            &project,
            &ResolverCatalog {
                entries: Vec::new(),
                complete: true,
            },
            &request,
        )
        .await
        .expect_err("257 intents must fail");
        assert_eq!(too_many.code(), M4ErrorCode::PlanTooLarge);
        assert_eq!(too_many.subreason(), Some("selection_limit"));

        request.bulk_intents = vec![
            PackageBulkIntent::Remove {
                package_id: installed[0].0.clone(),
            },
            PackageBulkIntent::Reinstall {
                package_id: installed[0].0.clone(),
                source: None,
            },
        ];
        assert_eq!(
            prepare_bulk(
                &cache,
                &snapshot,
                &project,
                &ResolverCatalog {
                    entries: Vec::new(),
                    complete: true,
                },
                &request,
            )
            .await
            .expect_err("duplicate intent must fail")
            .code(),
            M4ErrorCode::PackageIntentConflict
        );
        std::fs::remove_dir_all(root).expect("remove fixture");

        let installed_over = (0..257)
            .map(|value| {
                (
                    format!("com.example.reinstall-{value:03}"),
                    "1.0.0".to_owned(),
                )
            })
            .collect::<Vec<_>>();
        let (over_root, over_project, over_snapshot) =
            planning_fixture(&installed_over, &installed_over);
        let mut reinstall = package_request(PlanAction::Reinstall);
        reinstall.plan_version = PackagePlanVersion::V2;
        reinstall.reinstall_selection = Some(ReinstallSelection::All);
        let too_many = prepare_reinstall(
            &cache,
            &over_snapshot,
            &over_project,
            &ResolverCatalog {
                entries: Vec::new(),
                complete: true,
            },
            &reinstall,
        )
        .await
        .expect_err("reinstall all over 256 must fail");
        assert_eq!(too_many.code(), M4ErrorCode::PlanTooLarge);
        assert_eq!(too_many.subreason(), Some("selection_limit"));
        std::fs::remove_dir_all(over_root).expect("remove over-limit fixture");
    }

    #[tokio::test]
    async fn mixed_bulk_still_rejects_removing_a_dependency_required_by_a_reinstall() {
        let installed = [
            ("com.example.alpha".to_owned(), "1.0.0".to_owned()),
            ("com.example.beta".to_owned(), "1.0.0".to_owned()),
        ];
        let (root, project, _snapshot) = planning_fixture(&installed, &installed);
        // Reinstall preserves the installed locked graph, not new declarations published for the same version.
        let path = root.join("Packages/vpm-manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).expect("manifest")).expect("JSON");
        manifest["locked"]["com.example.alpha"]["dependencies"] =
            serde_json::json!({"com.example.beta":"1.0.0"});
        std::fs::write(path, serde_json::to_vec(&manifest).expect("JSON"))
            .expect("dependency graph");
        let snapshot = inspect_package_project(&project).expect("updated installed graph");
        let cache = PackageCache::new(root.join("owned-cache")).expect("cache");
        let mut catalog = catalog(&[
            ("com.example.alpha", "1.0.0"),
            ("com.example.beta", "1.0.0"),
        ]);
        catalog.entries[0].dependencies_json = r#"{"com.example.beta":"1.0.0"}"#.into();
        let mut request = package_request(PlanAction::Bulk);
        request.plan_version = PackagePlanVersion::V2;
        request.bulk_intents = vec![
            PackageBulkIntent::Remove {
                package_id: "com.example.beta".into(),
            },
            PackageBulkIntent::Reinstall {
                package_id: "com.example.alpha".into(),
                source: None,
            },
        ];
        assert!(!request.is_removal_only());
        let error = prepare_bulk(&cache, &snapshot, &project, &catalog, &request)
            .await
            .expect_err("mixed intent cannot authorize missing dependencies");
        assert_eq!(error.code(), M4ErrorCode::PackageDependencyConflict);
        std::fs::remove_dir_all(root).expect("remove owned fixture");
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
            },
            revision: Revision::INITIAL,
            registered_at_ms: 1,
            last_modified_at_ms: None,
            favorite: false,
        }
    }

    fn package_request(action: PlanAction) -> PackagePlanRequest {
        PackagePlanRequest {
            allow_unity_incompatible: false,
            action,
            project_id: ProjectId::new(),
            expected_revision: Revision::INITIAL,
            package_id: Some("com.example.fixture".to_owned()),
            version_range: None,
            repository_id: None,
            include_prerelease: false,
            plan_version: alcomd_application::PackagePlanVersion::V1,
            source_selector: None,
            reinstall_selection: None,
            reinstall_sources: Vec::new(),
            bulk_intents: Vec::new(),
        }
    }

    fn resolution(version: &str) -> crate::Resolution {
        crate::Resolution {
            packages: vec![crate::ResolvedPackage {
                package_id: "com.example.fixture".to_owned(),
                version: Version::parse(version).expect("version"),
                source: crate::PackageSource {
                    authority: crate::PackageSourceAuthority::Repository {
                        credential_reference: None,
                        download_headers_present: false,
                        repository_locator: None,
                        repository_id: "repo".to_owned(),
                        repository_revision: 1,
                        priority: 1,
                        artifact_url: "https://example.invalid/package.zip".to_owned(),
                    },
                    source_identity: "repo-source".to_owned(),
                    manifest_fingerprint: [4; 32],
                    archive_sha256: Some([5; 32]),
                },
                direct: true,
            }],
            dependency_edges: Vec::new(),
        }
    }

    fn planning_fixture(
        direct: &[(String, String)],
        locked: &[(String, String)],
    ) -> (PathBuf, ProjectRecord, crate::ProjectPackageSnapshot) {
        let root = temporary_root("planning");
        std::fs::create_dir_all(root.join("Packages")).expect("Packages");
        for (id, version) in locked {
            let folder = root.join("Packages").join(id);
            std::fs::create_dir_all(&folder).expect("installed fixture directory");
            std::fs::write(
                folder.join("package.json"),
                serde_json::to_vec(
                    &serde_json::json!({"name":id,"version":version,"vpmDependencies":{}}),
                )
                .expect("metadata"),
            )
            .expect("installed fixture metadata");
        }

        let dependencies = direct
            .iter()
            .map(|(package_id, version)| {
                (
                    package_id.clone(),
                    serde_json::json!({ "version": version }),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        let locked_json = locked
            .iter()
            .map(|(package_id, version)| {
                (
                    package_id.clone(),
                    serde_json::json!({ "version": version, "dependencies": {} }),
                )
            })
            .collect::<serde_json::Map<_, _>>();
        std::fs::write(
            root.join("Packages/vpm-manifest.json"),
            serde_json::to_vec(&serde_json::json!({
                "dependencies": dependencies,
                "locked": locked_json,
                "preserved": true
            }))
            .expect("serialize VPM manifest"),
        )
        .expect("VPM manifest");
        std::fs::write(
            root.join("Packages/manifest.json"),
            b"{\"dependencies\":{}}\n",
        )
        .expect("UPM manifest");
        let (_, identity) =
            alcomd_platform::resolve_directory_identity(&root).expect("project identity");
        let project = ProjectRecord {
            project_id: ProjectId::new(),
            observation: ProjectObservation {
                root_path: root.to_string_lossy().into_owned(),
                path_identity_key: identity,
                project_type: ProjectType::Unknown,
                unity_version: "2022.3.22f1".to_owned(),
                unity_revision: None,
                vpm_manifest: ManifestState::Valid,
                upm_manifest: ManifestState::Valid,
                direct_dependencies: direct
                    .iter()
                    .map(|(package_id, value)| DependencyIdentity {
                        package_id: package_id.clone(),
                        value: value.clone(),
                    })
                    .collect(),
                locked_dependencies: locked
                    .iter()
                    .map(|(package_id, value)| DependencyIdentity {
                        package_id: package_id.clone(),
                        value: value.clone(),
                    })
                    .collect(),
                issues: Vec::new(),
            },
            revision: Revision::INITIAL,
            registered_at_ms: 1,
            last_modified_at_ms: None,
            favorite: false,
        };
        let snapshot = inspect_package_project(&project).expect("inspect package project");
        (root, project, snapshot)
    }

    fn catalog(values: &[(&str, &str)]) -> ResolverCatalog {
        ResolverCatalog {
            entries: values
                .iter()
                .enumerate()
                .map(|(index, (package_id, version))| ResolverCatalogEntry {
                    source: ResolverCatalogSource::Repository {
                        repository_locator: String::new(),
                        repository_id: "00000000-0000-4000-8000-000000000001".to_owned(),
                        repository_revision: 1,
                        repository_priority: 1,
                        source_identity: "fixture-repository".to_owned(),
                        credential_reference: None,
                        download_headers_present: false,
                        artifact_url: format!("https://example.invalid/{package_id}/{version}.zip"),
                        zip_sha256: Some(format!("{index:064x}")),
                        local_zip_sha256: None,
                        manifest_fingerprint: [u8::try_from(index % 255).expect("byte"); 32],
                    },
                    package_id: (*package_id).to_owned(),
                    version: (*version).to_owned(),
                    yanked: false,
                    unity: None,
                    author_name: "Fixture".to_owned(),
                    author_email: Some("fixture@example.invalid".to_owned()),
                    unity_release: None,
                    dependencies_json: "{}".to_owned(),
                    legacy_metadata_present: false,
                })
                .collect(),
            complete: true,
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
                unity_compatibility_overrides: Vec::new(),
                removal_conflicts: Vec::new(),
                absent_removal_packages: Vec::new(),
                legacy_reinstall_packages: Vec::new(),
                format_version: 1,
                legacy_removals: Vec::new(),
                legacy_packages: Vec::new(),
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
