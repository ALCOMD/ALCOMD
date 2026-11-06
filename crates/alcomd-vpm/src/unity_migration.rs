//! Migration-private VPM and Unity process adapter for the sealed M7 workflow.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use alcomd_application::{
    IdempotencyKey, M3ErrorCode, M3ReadAdapter, M4Store, M7UnityMigrationAdapter,
    M7UnityMigrationError, M7UnityMigrationErrorCode, OperationId, PackageChangeSet,
    PackageMutationKind, PackageSourcePin, PlanId, ProjectAuthorityFingerprint,
    ProjectDiscoveryMode, ProjectObservation, ProjectRecord, UNITY_MIGRATION_PLAN_EXPIRY_MS,
    UnityInstallationRecord, UnityMigrationEvidence, UnityMigrationPlanAuthority,
    UnityMigrationPlanDraft, UnityMigrationPlanRecord, UnityMigrationRecoveryDisposition,
    UnityMigrationReobservation, UnityWriterState, UnityWriterStateKind,
};
use alcomd_platform::{
    UnityMigrationExit, UnityMigrationProcess, spawn_unity_editor_migration,
    validate_unity_executable,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization as _;

use crate::package::validate_extracted_package;
use crate::plan::tree_fingerprint;
use crate::resolver::{
    LegacyCleanupObligation, PackageSourceAuthority, ResolveRequest,
    resolve_unity_migration_packages,
};
use crate::staging_package::{extract_package_objects, fetch_package_objects};
use crate::{
    PackageCache, VpmReader, build_bulk_plan, candidates_from_catalog, inspect_package_project,
    materialize_vpm_manifest,
};

const PROJECT_VERSION_LIMIT: u64 = 64 * 1024;
const RECOVERY_OBSERVATION_LIMIT_MS: u64 = 300_000;
const PREPARATION_ARTIFACT_LIMIT: u64 = 4 * 1024 * 1024;
const LEGACY_ENTRY_LIMIT: usize = 4_096;
const LEGACY_PATH_DEPTH_LIMIT: usize = 64;
const LEGACY_PATH_LENGTH_LIMIT: usize = 1_024;
const VRCHAT_PACKAGE_IDS: [&str; 4] = [
    "com.vrchat.base",
    "com.vrchat.avatars",
    "com.vrchat.worlds",
    "com.vrchat.core.vpm-resolver",
];
const XR_UPM_DEPENDENCIES: [&str; 2] = [
    "com.unity.xr.oculus.standalone",
    "com.unity.xr.openvr.standalone",
];

/// The sole VPM implementation adapter for the sealed Project Unity migration use case.
#[derive(Clone)]
pub struct UnityMigrationEngine<S> {
    store: S,
    reader: VpmReader,
    cache: PackageCache,
    recovery_root: PathBuf,
}

impl<S> UnityMigrationEngine<S> {
    pub fn new(
        store: S,
        reader: VpmReader,
        cache_root: PathBuf,
        recovery_root: PathBuf,
    ) -> Result<Self, M7UnityMigrationError> {
        if !recovery_root.is_absolute() {
            return Err(error(M7UnityMigrationErrorCode::Internal));
        }
        let cache = reader
            .package_cache(cache_root)
            .map_err(|cause| error(M7UnityMigrationErrorCode::Internal).with_source(cause))?;
        Ok(Self {
            store,
            reader,
            cache,
            recovery_root,
        })
    }

    fn operation_root(&self, operation_id: OperationId) -> PathBuf {
        self.recovery_root
            .join("unity-migration")
            .join(operation_id.to_string())
    }
}

impl<S> M7UnityMigrationAdapter for UnityMigrationEngine<S>
where
    S: M4Store,
{
    type Process = UnityMigrationProcess;

    async fn plan(
        &self,
        project: ProjectRecord,
        target: UnityInstallationRecord,
        writer: UnityWriterState,
        authority: UnityMigrationPlanAuthority,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> Result<Option<UnityMigrationPlanDraft>, M7UnityMigrationError> {
        require_no_target_writer(&writer)?;
        if project.observation.unity_version != authority.source_unity_version
            || target.observation.unity_version != authority.target_unity_version
        {
            return Err(error(M7UnityMigrationErrorCode::SourceChanged));
        }
        let root = PathBuf::from(&project.observation.root_path);
        let (canonical_root, identity) = alcomd_platform::resolve_directory_identity(&root)
            .map_err(|cause| error(M7UnityMigrationErrorCode::SourceChanged).with_source(cause))?;
        if identity != project.observation.path_identity_key {
            return Err(error(M7UnityMigrationErrorCode::SourceChanged));
        }
        let marker_sha256 = project_version_marker_sha256(&canonical_root)?;
        let executable = validate_unity_executable(Path::new(&target.observation.executable_path))
            .map_err(|cause| {
                error(M7UnityMigrationErrorCode::InstallationNotFound).with_source(cause)
            })?;
        if executable.filesystem_identity() != target.observation.filesystem_identity {
            return Err(error(M7UnityMigrationErrorCode::InstallationNotFound));
        }
        let plan_id = PlanId::new();
        let request_fingerprint = fingerprint(&PlanRequestFingerprint {
            version: 1,
            project_id: project.project_id.to_string(),
            project_revision: project.revision.get(),
            target_installation_id: target.installation_id.to_string(),
            target_installation_revision: target.revision.get(),
            idempotency_key: key.as_str(),
        })?;
        let plan_fingerprint = fingerprint(&PlanAuthorityFingerprint {
            version: 1,
            project: ProjectAuthorityFingerprint::from(&project),
            source_unity_version: &authority.source_unity_version,
            project_root_identity: &identity,
            project_version_marker_sha256: marker_sha256,
            target: &target,
            writer: &writer,
            classification: authority.classification.as_str(),
            preparation_profile: authority.preparation_profile.as_deref(),
        })?;
        let expires_at_ms = now_ms
            .checked_add(UNITY_MIGRATION_PLAN_EXPIRY_MS)
            .ok_or_else(|| error(M7UnityMigrationErrorCode::Internal))?;
        Ok(Some(UnityMigrationPlanDraft {
            plan_id,
            source_unity_version: authority.source_unity_version,
            source_revision_metadata: project.observation.unity_revision.clone(),
            project_root_identity: identity,
            project_version_marker_sha256: marker_sha256,
            target_unity_version: authority.target_unity_version,
            target_revision_metadata: None,
            target_installation: target,
            writer_evidence: writer,
            classification: authority.classification,
            preparation_profile: authority.preparation_profile,
            plan_fingerprint,
            request_fingerprint,
            plan_idempotency_key: key,
            created_at_ms: now_ms,
            expires_at_ms,
            project,
        }))
    }

    async fn revalidate(
        &self,
        plan: UnityMigrationPlanRecord,
        project: ProjectRecord,
        target: UnityInstallationRecord,
    ) -> Result<(), M7UnityMigrationError> {
        if project.project_id != plan.draft.project.project_id
            || project.revision != plan.draft.project.revision
            || project.observation.unity_version != plan.draft.source_unity_version
            || target.installation_id != plan.draft.target_installation.installation_id
            || target.revision != plan.draft.target_installation.revision
            || target.observation.unity_version != plan.draft.target_unity_version
            || target.observation.filesystem_identity
                != plan
                    .draft
                    .target_installation
                    .observation
                    .filesystem_identity
        {
            return Err(error(M7UnityMigrationErrorCode::PlanStale));
        }
        let root = PathBuf::from(&project.observation.root_path);
        let (canonical_root, identity) = alcomd_platform::resolve_directory_identity(&root)
            .map_err(|cause| error(M7UnityMigrationErrorCode::SourceChanged).with_source(cause))?;
        if identity != plan.draft.project_root_identity
            || project_version_marker_sha256(&canonical_root)?
                != plan.draft.project_version_marker_sha256
        {
            return Err(error(M7UnityMigrationErrorCode::SourceChanged));
        }
        let executable = validate_unity_executable(Path::new(&target.observation.executable_path))
            .map_err(|cause| {
                error(M7UnityMigrationErrorCode::InstallationNotFound).with_source(cause)
            })?;
        if executable.filesystem_identity() != target.observation.filesystem_identity {
            return Err(error(M7UnityMigrationErrorCode::InstallationNotFound));
        }
        Ok(())
    }

    async fn materialize_preparation(
        &self,
        operation_id: OperationId,
        plan: UnityMigrationPlanRecord,
        mut evidence: UnityMigrationEvidence,
    ) -> Result<UnityMigrationEvidence, M7UnityMigrationError> {
        let operation_root = self.operation_root(operation_id);
        let artifact = operation_root.join("preparation-plan.json");
        if artifact.exists() {
            let bytes = read_regular_file(&artifact, PREPARATION_ARTIFACT_LIMIT)?;
            let digest: [u8; 32] = Sha256::digest(&bytes).into();
            if evidence.preparation_artifact_sha256 == Some(digest) {
                return Ok(evidence);
            }
            return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
        }
        if plan.draft.preparation_profile.is_some() {
            return self
                .materialize_vrchat_2019_to_2022(operation_id, plan, evidence)
                .await;
        }
        std::fs::create_dir_all(&operation_root)
            .map_err(|cause| error(M7UnityMigrationErrorCode::Internal).with_source(cause))?;
        let bytes = serde_json::to_vec(&EmptyPreparationArtifact {
            version: 1,
            operation_id: operation_id.to_string(),
            plan_fingerprint: &plan.draft.plan_fingerprint,
            preparation_kind: "none",
        })
        .map_err(|cause| error(M7UnityMigrationErrorCode::Internal).with_source(cause))?;
        write_new_synced_file(&artifact, &bytes)?;
        alcomd_platform::sync_directory(&operation_root)
            .map_err(|cause| error(M7UnityMigrationErrorCode::Internal).with_source(cause))?;
        evidence.preparation_kind = "none".to_owned();
        evidence.preparation_operation_id = Some(operation_id);
        evidence.preparation_artifact_sha256 = Some(Sha256::digest(&bytes).into());
        Ok(evidence)
    }

    async fn apply_preparation(
        &self,
        operation_id: OperationId,
        plan: UnityMigrationPlanRecord,
        mut evidence: UnityMigrationEvidence,
    ) -> Result<UnityMigrationEvidence, M7UnityMigrationError> {
        let artifact = self
            .operation_root(operation_id)
            .join("preparation-plan.json");
        let bytes = read_regular_file(&artifact, PREPARATION_ARTIFACT_LIMIT)?;
        if evidence.preparation_artifact_sha256 != Some(Sha256::digest(&bytes).into()) {
            return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
        }
        if plan.draft.preparation_profile.is_some() {
            return self
                .apply_vrchat_2019_to_2022(operation_id, plan, evidence)
                .await;
        }
        evidence.preparation_complete = true;
        Ok(evidence)
    }

    async fn spawn(
        &self,
        plan: UnityMigrationPlanRecord,
        mut evidence: UnityMigrationEvidence,
    ) -> Result<(Self::Process, UnityMigrationEvidence), M7UnityMigrationError> {
        if plan.draft.preparation_profile.is_some() {
            let operation_id = evidence
                .preparation_operation_id
                .ok_or_else(|| error(M7UnityMigrationErrorCode::RecoveryRequired))?;
            let operation_root = self.operation_root(operation_id);
            if latest_transaction_phase(&operation_root)?
                != Some(PreparationTransactionPhase::Complete)
            {
                return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
            }
        }
        let root = PathBuf::from(&plan.draft.project.observation.root_path);
        let (root, identity) = alcomd_platform::resolve_directory_identity(&root)
            .map_err(|cause| error(M7UnityMigrationErrorCode::SourceChanged).with_source(cause))?;
        if identity != plan.draft.project_root_identity {
            return Err(error(M7UnityMigrationErrorCode::SourceChanged));
        }
        if project_version_marker_sha256(&root)? != plan.draft.project_version_marker_sha256 {
            return Err(error(M7UnityMigrationErrorCode::SourceChanged));
        }
        let executable = validate_unity_executable(Path::new(
            &plan.draft.target_installation.observation.executable_path,
        ))
        .map_err(|cause| {
            error(M7UnityMigrationErrorCode::InstallationNotFound).with_source(cause)
        })?;
        if executable.filesystem_identity()
            != plan
                .draft
                .target_installation
                .observation
                .filesystem_identity
        {
            return Err(error(M7UnityMigrationErrorCode::InstallationNotFound));
        }
        let process = spawn_unity_editor_migration(&executable, &root)
            .map_err(|cause| error(M7UnityMigrationErrorCode::Internal).with_source(cause))?;
        evidence.spawn_accepted = true;
        evidence
            .safe_evidence
            .push("exact_target_spawn_accepted".to_owned());
        Ok((process, evidence))
    }

    async fn wait(
        &self,
        process: Self::Process,
        mut evidence: UnityMigrationEvidence,
    ) -> Result<UnityMigrationEvidence, M7UnityMigrationError> {
        let exit = tokio::task::spawn_blocking(move || process.wait())
            .await
            .map_err(|cause| error(M7UnityMigrationErrorCode::Internal).with_source(cause))?
            .map_err(|cause| error(M7UnityMigrationErrorCode::Internal).with_source(cause))?;
        evidence.exit_observation = Some(
            match exit {
                UnityMigrationExit::Success => "success",
                UnityMigrationExit::NonZero => "non_zero",
            }
            .to_owned(),
        );
        Ok(evidence)
    }

    async fn recover_after_launch_intent(
        &self,
        plan: UnityMigrationPlanRecord,
        writer: UnityWriterState,
        mut evidence: UnityMigrationEvidence,
    ) -> Result<UnityMigrationRecoveryDisposition, M7UnityMigrationError> {
        match writer.state {
            UnityWriterStateKind::RunningConfirmed | UnityWriterStateKind::RunningSuspected => {
                let started =
                    recovery_observation_started(&evidence).unwrap_or(writer.checked_at_ms);
                if writer.checked_at_ms.saturating_sub(started) >= RECOVERY_OBSERVATION_LIMIT_MS {
                    return Ok(UnityMigrationRecoveryDisposition::RecoveryRequired(
                        evidence,
                    ));
                }
                if recovery_observation_started(&evidence).is_none() {
                    evidence
                        .safe_evidence
                        .push(format!("recovery_observation_started:{started}"));
                }
                return Ok(UnityMigrationRecoveryDisposition::Pending(evidence));
            }
            UnityWriterStateKind::Unknown
                if !matches!(
                    evidence.exit_observation.as_deref(),
                    Some("success" | "non_zero")
                ) =>
            {
                return Ok(UnityMigrationRecoveryDisposition::RecoveryRequired(
                    evidence,
                ));
            }
            UnityWriterStateKind::NotObserved | UnityWriterStateKind::Unknown => {}
        }
        let observation = self.inspect_planned_project(&plan).await?;
        if observation.unity_version == plan.draft.target_unity_version {
            let reobserved = self.ready_reobservation(&plan, writer, observation, evidence)?;
            return Ok(UnityMigrationRecoveryDisposition::ProjectReady(reobserved));
        }
        if observation.unity_version == plan.draft.source_unity_version
            && evidence.preparation_kind == "none"
            && observation.path_identity_key == plan.draft.project_root_identity
            && project_version_marker_sha256(Path::new(&observation.root_path))?
                == plan.draft.project_version_marker_sha256
        {
            evidence.safe_terminal_failure = true;
            evidence.writer_inactive_checked_at_ms = Some(writer.checked_at_ms);
            evidence
                .safe_evidence
                .push("safe_source_snapshot_unchanged".to_owned());
            return Ok(UnityMigrationRecoveryDisposition::SafelyUnchanged(evidence));
        }
        Ok(UnityMigrationRecoveryDisposition::RecoveryRequired(
            evidence,
        ))
    }

    async fn resume_project_reobserved(
        &self,
        plan: UnityMigrationPlanRecord,
        writer: UnityWriterState,
        evidence: UnityMigrationEvidence,
    ) -> Result<UnityMigrationReobservation, M7UnityMigrationError> {
        require_no_target_writer(&writer)?;
        if evidence.reobserved_version.as_deref() != Some(plan.draft.target_unity_version.as_str())
            || evidence.reobserved_root_identity.as_deref()
                != Some(plan.draft.project_root_identity.as_slice())
            || evidence.reobserved_marker_sha256.is_none()
            || evidence.writer_inactive_checked_at_ms.is_none()
            || evidence.reobserved_at_ms.is_none()
        {
            return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
        }
        let observation = self.inspect_planned_project(&plan).await?;
        if observation.unity_version != plan.draft.target_unity_version
            || evidence.reobserved_marker_sha256
                != Some(project_version_marker_sha256(Path::new(
                    &observation.root_path,
                ))?)
        {
            return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
        }
        Ok(UnityMigrationReobservation {
            observation,
            evidence,
        })
    }

    async fn cleanup(
        &self,
        operation_id: OperationId,
        _plan: UnityMigrationPlanRecord,
        mut evidence: UnityMigrationEvidence,
    ) -> Result<UnityMigrationEvidence, M7UnityMigrationError> {
        let root = self.operation_root(operation_id);
        if root.exists() {
            std::fs::remove_dir_all(&root).map_err(|cause| {
                error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause)
            })?;
            if let Some(parent) = root.parent() {
                alcomd_platform::sync_directory(parent).map_err(|cause| {
                    error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause)
                })?;
            }
        }
        evidence
            .safe_evidence
            .push("private_recovery_artifact_removed".to_owned());
        Ok(evidence)
    }
}

impl<S> UnityMigrationEngine<S>
where
    S: M4Store,
{
    async fn inspect_planned_project(
        &self,
        plan: &UnityMigrationPlanRecord,
    ) -> Result<ProjectObservation, M7UnityMigrationError> {
        let observation = self
            .reader
            .inspect_project(
                plan.draft.project.observation.root_path.clone(),
                ProjectDiscoveryMode::ExactRoot,
            )
            .await
            .map_err(|cause| {
                error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause)
            })?;
        if observation.path_identity_key != plan.draft.project_root_identity {
            return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
        }
        Ok(observation)
    }

    fn ready_reobservation(
        &self,
        plan: &UnityMigrationPlanRecord,
        writer: UnityWriterState,
        observation: ProjectObservation,
        mut evidence: UnityMigrationEvidence,
    ) -> Result<UnityMigrationReobservation, M7UnityMigrationError> {
        let marker_sha256 = project_version_marker_sha256(Path::new(&observation.root_path))?;
        evidence.reobserved_version = Some(observation.unity_version.clone());
        evidence.reobserved_root_identity = Some(observation.path_identity_key.clone());
        evidence.reobserved_marker_sha256 = Some(marker_sha256);
        evidence.writer_inactive_checked_at_ms = Some(writer.checked_at_ms);
        evidence
            .safe_evidence
            .push("target_project_reobserved".to_owned());
        if evidence.reobserved_version.as_deref() != Some(plan.draft.target_unity_version.as_str())
        {
            return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
        }
        evidence.reobserved_at_ms = Some(
            crate::unix_time_ms()
                .map_err(|cause| error(M7UnityMigrationErrorCode::Internal).with_source(cause))?,
        );
        Ok(UnityMigrationReobservation {
            observation,
            evidence,
        })
    }

    async fn materialize_vrchat_2019_to_2022(
        &self,
        operation_id: OperationId,
        plan: UnityMigrationPlanRecord,
        mut evidence: UnityMigrationEvidence,
    ) -> Result<UnityMigrationEvidence, M7UnityMigrationError> {
        if plan.draft.preparation_profile.as_deref() != Some("vrchat-2019-to-2022-v1") {
            return Err(error(M7UnityMigrationErrorCode::Unsupported));
        }
        let operation_root = self.operation_root(operation_id);
        if operation_root.exists() {
            std::fs::remove_dir_all(&operation_root)
                .map_err(|cause| error(M7UnityMigrationErrorCode::Internal).with_source(cause))?;
        }
        let materialized = operation_root.join("materialized");
        let package_staging = materialized.join("packages");
        std::fs::create_dir_all(&package_staging)
            .map_err(|cause| error(M7UnityMigrationErrorCode::Internal).with_source(cause))?;

        let snapshot = inspect_package_project(&plan.draft.project)
            .map_err(|cause| error(M7UnityMigrationErrorCode::SourceChanged).with_source(cause))?;
        let catalog = self
            .store
            .resolver_catalog(plan.owner.clone(), true)
            .await
            .map_err(|cause| error(M7UnityMigrationErrorCode::Internal).with_source(cause))?;
        if !catalog.complete {
            return Err(error(M7UnityMigrationErrorCode::SourceChanged));
        }
        let candidates = candidates_from_catalog(&catalog)
            .map_err(|cause| error(M7UnityMigrationErrorCode::Unsupported).with_source(cause))?;
        let requests = migration_requests(&plan.draft.project, &plan.draft.target_unity_version)?;
        let mut migration_resolution = if requests.is_empty() {
            crate::resolver::UnityMigrationResolution {
                resolution: crate::Resolution {
                    packages: Vec::new(),
                    dependency_edges: Vec::new(),
                },
                legacy_cleanup_obligations: Vec::new(),
            }
        } else {
            resolve_unity_migration_packages(&candidates, &requests)
                .map_err(|cause| error(M7UnityMigrationErrorCode::Unsupported).with_source(cause))?
        };
        validate_obligation_scope(&migration_resolution.legacy_cleanup_obligations)?;
        crate::engine::pin_resolution_archives(
            &self.cache,
            &self.store,
            &plan.owner,
            &mut migration_resolution.resolution,
        )
        .await
        .map_err(|cause| error(M7UnityMigrationErrorCode::SourceChanged).with_source(cause))?;
        let package_plan = build_bulk_plan(
            &snapshot,
            &migration_resolution.resolution,
            &BTreeSet::new(),
        )
        .map_err(|cause| error(M7UnityMigrationErrorCode::Unsupported).with_source(cause))?;

        let root = PathBuf::from(&plan.draft.project.observation.root_path);
        let current_vpm = read_project_component(&root, &["Packages", "vpm-manifest.json"])?;
        let next_vpm = materialize_vpm_manifest(&current_vpm, &package_plan.change_set)
            .map_err(|cause| error(M7UnityMigrationErrorCode::Unsupported).with_source(cause))?;
        if Sha256::digest(&next_vpm).as_slice() != package_plan.change_set.vpm_manifest_sha256 {
            return Err(error(M7UnityMigrationErrorCode::Internal));
        }
        let current_upm = read_project_component(&root, &["Packages", "manifest.json"])?;
        let next_upm = remove_fixed_xr_dependencies(&current_upm)?;
        write_new_synced_file(&materialized.join("vpm-manifest.new"), &next_vpm)?;
        write_new_synced_file(&materialized.join("upm-manifest.new"), &next_upm)?;

        let archives = fetch_package_objects(
            &self.cache,
            None,
            package_plan
                .change_set
                .mutations
                .iter()
                .filter_map(|mutation| mutation.source.clone())
                .collect(),
        )
        .await
        .map_err(|cause| error(M7UnityMigrationErrorCode::SourceChanged).with_source(cause))?;
        extract_package_objects(&package_staging, &package_plan.change_set, &archives)
            .await
            .map_err(|cause| error(M7UnityMigrationErrorCode::Unsupported).with_source(cause))?;
        let mut prepared_package_trees = BTreeMap::new();
        for package_id in archives.keys() {
            let destination = package_staging.join(package_id);
            prepared_package_trees.insert(
                package_id.clone(),
                tree_fingerprint(&destination).map_err(|cause| {
                    error(M7UnityMigrationErrorCode::Unsupported).with_source(cause)
                })?,
            );
        }

        let selected_ids = migration_resolution
            .resolution
            .packages
            .iter()
            .map(|package| package.package_id.as_str())
            .collect::<BTreeSet<_>>();
        let old_locked = plan
            .draft
            .project
            .observation
            .locked_dependencies
            .iter()
            .map(|dependency| dependency.package_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut legacy_paths = Vec::new();
        let mut legacy_packages = BTreeSet::new();
        let mut pinned_obligations = Vec::new();
        for obligation in &migration_resolution.legacy_cleanup_obligations {
            let version = obligation.version.to_string();
            let source = package_plan
                .source_set
                .iter()
                .find(|source| {
                    source.package_id() == obligation.package_id && source.version() == version
                })
                .ok_or_else(|| error(M7UnityMigrationErrorCode::SourceChanged))?;
            let metadata = self
                .cache
                .pin_legacy_manifest(&obligation.source, &obligation.package_id, &version)
                .await
                .map_err(|cause| {
                    error(M7UnityMigrationErrorCode::SourceChanged).with_source(cause)
                })?;
            for package_id in &metadata.packages {
                if selected_ids.contains(package_id.as_str()) {
                    return Err(error(M7UnityMigrationErrorCode::Unsupported));
                }
                if old_locked.contains(package_id.as_str()) {
                    legacy_packages.insert(package_id.clone());
                } else if plan
                    .draft
                    .project
                    .observation
                    .direct_dependencies
                    .iter()
                    .any(|dependency| dependency.package_id == *package_id)
                {
                    return Err(error(M7UnityMigrationErrorCode::Unsupported));
                }
            }
            legacy_paths.extend(sealed_legacy_paths(&root, &metadata)?);
            pinned_obligations.push(PinnedLegacyObligation::from_obligation(
                obligation,
                source.archive_sha256(),
            ));
        }
        let legacy_paths = validate_legacy_paths(&root, legacy_paths)?;
        let unlocked_conflicts =
            find_unlocked_conflicts(&root, &plan.draft.project, &package_plan.change_set)?;
        validate_legacy_package_removals(&package_plan.change_set, &legacy_packages)?;

        let artifact = MigrationPreparationArtifact {
            version: 1,
            operation_id: operation_id.to_string(),
            plan_fingerprint: plan.draft.plan_fingerprint.clone(),
            profile: "vrchat-2019-to-2022-v1".to_owned(),
            project_snapshot_fingerprint: package_plan.project_snapshot_fingerprint,
            change_set_fingerprint: package_plan.change_set_fingerprint,
            change_set: package_plan.change_set,
            source_set: package_plan.source_set,
            pinned_legacy_obligations: pinned_obligations,
            legacy_packages: legacy_packages.into_iter().collect(),
            legacy_paths,
            unlocked_conflicts,
            xr_upm_dependencies: XR_UPM_DEPENDENCIES.map(str::to_owned).to_vec(),
            prepared_package_trees,
            vpm_manifest_before_sha256: Sha256::digest(&current_vpm).into(),
            vpm_manifest_sha256: Sha256::digest(&next_vpm).into(),
            upm_manifest_before_sha256: Sha256::digest(&current_upm).into(),
            upm_manifest_after_sha256: Sha256::digest(&next_upm).into(),
        };
        let bytes = serde_json::to_vec(&artifact)
            .map_err(|cause| error(M7UnityMigrationErrorCode::Internal).with_source(cause))?;
        if bytes.len() as u64 > PREPARATION_ARTIFACT_LIMIT {
            return Err(error(M7UnityMigrationErrorCode::Unsupported));
        }
        write_new_synced_file(&operation_root.join("preparation-plan.json"), &bytes)?;
        sync_tree(&operation_root)?;
        evidence.preparation_kind = artifact.profile;
        evidence.preparation_operation_id = Some(operation_id);
        evidence.preparation_artifact_sha256 = Some(Sha256::digest(&bytes).into());
        evidence.safe_evidence.push(format!(
            "preparation_materialized:packages={}:obligations={}:paths={}",
            artifact.change_set.mutations.len(),
            artifact.pinned_legacy_obligations.len(),
            artifact.legacy_paths.len()
        ));
        Ok(evidence)
    }

    async fn apply_vrchat_2019_to_2022(
        &self,
        operation_id: OperationId,
        plan: UnityMigrationPlanRecord,
        mut evidence: UnityMigrationEvidence,
    ) -> Result<UnityMigrationEvidence, M7UnityMigrationError> {
        let operation_root = self.operation_root(operation_id);
        let bytes = read_regular_file(
            &operation_root.join("preparation-plan.json"),
            PREPARATION_ARTIFACT_LIMIT,
        )?;
        if evidence.preparation_artifact_sha256 != Some(Sha256::digest(&bytes).into()) {
            return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
        }
        let artifact: MigrationPreparationArtifact =
            serde_json::from_slice(&bytes).map_err(|cause| {
                error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause)
            })?;
        if artifact.version != 1
            || artifact.operation_id != operation_id.to_string()
            || artifact.plan_fingerprint != plan.draft.plan_fingerprint
            || artifact.profile != "vrchat-2019-to-2022-v1"
            || artifact.xr_upm_dependencies != XR_UPM_DEPENDENCIES.map(str::to_owned).to_vec()
        {
            return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
        }
        if Sha256::digest(
            serde_json::to_vec(&artifact.change_set)
                .map_err(|cause| error(M7UnityMigrationErrorCode::Internal).with_source(cause))?,
        )
        .as_slice()
            != artifact.change_set_fingerprint
        {
            return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
        }
        let root = PathBuf::from(&plan.draft.project.observation.root_path);
        let (root, identity) =
            alcomd_platform::resolve_directory_identity(&root).map_err(|cause| {
                error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause)
            })?;
        if identity != plan.draft.project_root_identity {
            return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
        }
        let mut phase = latest_transaction_phase(&operation_root)?;
        if phase.is_none() {
            let snapshot = inspect_package_project(&plan.draft.project).map_err(|cause| {
                error(M7UnityMigrationErrorCode::SourceChanged).with_source(cause)
            })?;
            if snapshot.fingerprint != artifact.project_snapshot_fingerprint
                || project_version_marker_sha256(&root)? != plan.draft.project_version_marker_sha256
            {
                return Err(error(M7UnityMigrationErrorCode::SourceChanged));
            }
            if Sha256::digest(read_project_component(
                &root,
                &["Packages", "manifest.json"],
            )?)
            .as_slice()
                != artifact.upm_manifest_before_sha256
            {
                return Err(error(M7UnityMigrationErrorCode::SourceChanged));
            }
            validate_materialized_packages(&operation_root, &artifact)?;
            write_transaction_marker(
                &operation_root,
                operation_id,
                PreparationTransactionPhase::Prepared,
            )?;
            phase = Some(PreparationTransactionPhase::Prepared);
        }
        if phase == Some(PreparationTransactionPhase::Prepared) {
            replace_migration_packages(&root, &operation_root, &artifact)?;
            write_transaction_marker(
                &operation_root,
                operation_id,
                PreparationTransactionPhase::PackagesReplaced,
            )?;
            phase = Some(PreparationTransactionPhase::PackagesReplaced);
        }
        if phase == Some(PreparationTransactionPhase::PackagesReplaced) {
            replace_migration_manifest(
                &root.join("Packages").join("vpm-manifest.json"),
                &operation_root,
                "vpm-manifest",
                artifact.vpm_manifest_before_sha256,
                artifact.vpm_manifest_sha256,
            )?;
            write_transaction_marker(
                &operation_root,
                operation_id,
                PreparationTransactionPhase::VpmManifestReplaced,
            )?;
            phase = Some(PreparationTransactionPhase::VpmManifestReplaced);
        }
        if phase == Some(PreparationTransactionPhase::VpmManifestReplaced) {
            replace_migration_manifest(
                &root.join("Packages").join("manifest.json"),
                &operation_root,
                "upm-manifest",
                artifact.upm_manifest_before_sha256,
                artifact.upm_manifest_after_sha256,
            )?;
            write_transaction_marker(
                &operation_root,
                operation_id,
                PreparationTransactionPhase::UpmManifestReplaced,
            )?;
            phase = Some(PreparationTransactionPhase::UpmManifestReplaced);
        }
        if phase == Some(PreparationTransactionPhase::UpmManifestReplaced) {
            quarantine_legacy_paths(&root, &operation_root, &artifact.legacy_paths)?;
            verify_prepared_project(&root, &artifact)?;
            write_transaction_marker(
                &operation_root,
                operation_id,
                PreparationTransactionPhase::Complete,
            )?;
            phase = Some(PreparationTransactionPhase::Complete);
        }
        if phase != Some(PreparationTransactionPhase::Complete) {
            return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
        }
        verify_prepared_project(&root, &artifact)?;
        evidence.preparation_complete = true;
        evidence
            .safe_evidence
            .push(format!("preparation_operation:{operation_id}"));
        evidence
            .safe_evidence
            .push("preparation_transaction_complete".to_owned());
        Ok(evidence)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanRequestFingerprint<'a> {
    version: u32,
    project_id: String,
    project_revision: u64,
    target_installation_id: String,
    target_installation_revision: u64,
    idempotency_key: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlanAuthorityFingerprint<'a> {
    version: u32,
    project: ProjectAuthorityFingerprint<'a>,
    source_unity_version: &'a str,
    project_root_identity: &'a [u8],
    project_version_marker_sha256: [u8; 32],
    target: &'a UnityInstallationRecord,
    writer: &'a UnityWriterState,
    classification: &'a str,
    preparation_profile: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EmptyPreparationArtifact<'a> {
    version: u32,
    operation_id: String,
    plan_fingerprint: &'a str,
    preparation_kind: &'a str,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationPreparationArtifact {
    version: u32,
    operation_id: String,
    plan_fingerprint: String,
    profile: String,
    project_snapshot_fingerprint: [u8; 32],
    change_set_fingerprint: [u8; 32],
    change_set: PackageChangeSet,
    source_set: Vec<PackageSourcePin>,
    pinned_legacy_obligations: Vec<PinnedLegacyObligation>,
    legacy_packages: Vec<String>,
    legacy_paths: Vec<LegacyPathPlan>,
    unlocked_conflicts: Vec<String>,
    xr_upm_dependencies: Vec<String>,
    prepared_package_trees: BTreeMap<String, [u8; 32]>,
    vpm_manifest_before_sha256: [u8; 32],
    vpm_manifest_sha256: [u8; 32],
    upm_manifest_before_sha256: [u8; 32],
    upm_manifest_after_sha256: [u8; 32],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PinnedLegacyObligation {
    package_id: String,
    version: String,
    authority_kind: String,
    authority_id: String,
    authority_revision: u64,
    source_identity: String,
    manifest_fingerprint: [u8; 32],
    archive_sha256: [u8; 32],
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum LegacyPathKind {
    Missing,
    File,
    Directory,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct LegacyPathPlan {
    relative_path: String,
    kind: LegacyPathKind,
    filesystem_identity: Option<Vec<u8>>,
}

impl PinnedLegacyObligation {
    fn from_obligation(value: &LegacyCleanupObligation, archive_sha256: [u8; 32]) -> Self {
        let (authority_kind, authority_id, authority_revision) = match &value.source.authority {
            PackageSourceAuthority::Repository {
                repository_id,
                repository_revision,
                ..
            } => (
                "repository".to_owned(),
                repository_id.clone(),
                *repository_revision,
            ),
            PackageSourceAuthority::UserPackage {
                user_package_id,
                source_revision,
            } => (
                "user_package".to_owned(),
                user_package_id.to_string(),
                *source_revision,
            ),
        };
        Self {
            package_id: value.package_id.clone(),
            version: value.version.to_string(),
            authority_kind,
            authority_id,
            authority_revision,
            source_identity: value.source.source_identity.clone(),
            manifest_fingerprint: value.source.manifest_fingerprint,
            archive_sha256,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
enum PreparationTransactionPhase {
    Prepared,
    PackagesReplaced,
    VpmManifestReplaced,
    UpmManifestReplaced,
    Complete,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PreparationTransactionMarker {
    version: u32,
    operation_id: String,
    phase: PreparationTransactionPhase,
}

fn latest_transaction_phase(
    operation_root: &Path,
) -> Result<Option<PreparationTransactionPhase>, M7UnityMigrationError> {
    let marker_root = operation_root.join("transaction-markers");
    let entries = match std::fs::read_dir(&marker_root) {
        Ok(entries) => entries,
        Err(value) if value.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(cause) => {
            return Err(error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause));
        }
    };
    let marker_paths = entries
        .map(|entry| {
            entry.map(|entry| entry.path()).map_err(|cause| {
                error(M7UnityMigrationErrorCode::RecoveryRequired)
                    .with_source(cause)
                    .with_path(operation_root.display().to_string())
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if marker_paths.len() > 5
        || marker_paths.iter().any(|path| {
            !path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.len() == "step-0000.json".len()
                        && name.starts_with("step-")
                        && name.ends_with(".json")
                        && name[5..9].bytes().all(|byte| byte.is_ascii_digit())
                })
        })
    {
        return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
    }
    let marker_path = marker_paths.into_iter().max();
    let Some(marker_path) = marker_path else {
        return Ok(None);
    };
    let bytes = read_regular_file(&marker_path, 65_536)?;
    let marker: PreparationTransactionMarker = serde_json::from_slice(&bytes).map_err(|cause| {
        error(M7UnityMigrationErrorCode::RecoveryRequired)
            .with_source(cause)
            .with_path(operation_root.display().to_string())
    })?;
    if marker.version != 1
        || marker.operation_id
            != operation_root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
    {
        return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
    }
    Ok(Some(marker.phase))
}

fn write_transaction_marker(
    operation_root: &Path,
    operation_id: OperationId,
    phase: PreparationTransactionPhase,
) -> Result<(), M7UnityMigrationError> {
    let marker_root = operation_root.join("transaction-markers");
    std::fs::create_dir_all(&marker_root).map_err(|cause| {
        error(M7UnityMigrationErrorCode::RecoveryRequired)
            .with_source(cause)
            .with_path(operation_root.display().to_string())
    })?;
    let step = std::fs::read_dir(&marker_root)
        .map_err(|cause| {
            error(M7UnityMigrationErrorCode::RecoveryRequired)
                .with_source(cause)
                .with_path(operation_root.display().to_string())
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|cause| {
            error(M7UnityMigrationErrorCode::RecoveryRequired)
                .with_source(cause)
                .with_path(operation_root.display().to_string())
        })?
        .len()
        .checked_add(1)
        .filter(|step| *step <= 5)
        .ok_or_else(|| error(M7UnityMigrationErrorCode::RecoveryRequired))?;
    let bytes = serde_json::to_vec(&PreparationTransactionMarker {
        version: 1,
        operation_id: operation_id.to_string(),
        phase,
    })
    .map_err(|cause| {
        error(M7UnityMigrationErrorCode::Internal)
            .with_source(cause)
            .with_path(operation_root.display().to_string())
    })?;
    write_new_synced_file(&marker_root.join(format!("step-{step:04}.json")), &bytes)?;
    alcomd_platform::sync_directory(&marker_root).map_err(|cause| {
        error(M7UnityMigrationErrorCode::RecoveryRequired)
            .with_source(cause)
            .with_path(operation_root.display().to_string())
    })?;
    Ok(())
}

fn validate_materialized_packages(
    operation_root: &Path,
    artifact: &MigrationPreparationArtifact,
) -> Result<(), M7UnityMigrationError> {
    for mutation in &artifact.change_set.mutations {
        let Some(_) = mutation.source else {
            continue;
        };
        let path = operation_root
            .join("materialized")
            .join("packages")
            .join(&mutation.package_id);
        let expected_version = mutation
            .to_version
            .as_deref()
            .ok_or_else(|| error(M7UnityMigrationErrorCode::RecoveryRequired))?;
        validate_extracted_package(&path, &mutation.package_id, expected_version).map_err(
            |cause| {
                error(M7UnityMigrationErrorCode::RecoveryRequired)
                    .with_source(cause)
                    .with_path(operation_root.display().to_string())
            },
        )?;
        if artifact.prepared_package_trees.get(&mutation.package_id)
            != Some(&tree_fingerprint(&path).map_err(|cause| {
                error(M7UnityMigrationErrorCode::RecoveryRequired)
                    .with_source(cause)
                    .with_path(operation_root.display().to_string())
            })?)
        {
            return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
        }
    }
    Ok(())
}

fn replace_migration_packages(
    root: &Path,
    operation_root: &Path,
    artifact: &MigrationPreparationArtifact,
) -> Result<(), M7UnityMigrationError> {
    let packages_root = root.join("Packages");
    let backup_root = operation_root.join("backup").join("packages");
    std::fs::create_dir_all(&backup_root)
        .map_err(|cause| error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause))?;
    for mutation in &artifact.change_set.mutations {
        let target = packages_root.join(&mutation.package_id);
        let backup = backup_root.join(&mutation.package_id);
        let prepared = operation_root
            .join("materialized")
            .join("packages")
            .join(&mutation.package_id);
        if !backup.exists() {
            match std::fs::symlink_metadata(&target) {
                Ok(metadata)
                    if metadata.is_dir()
                        && !metadata.file_type().is_symlink()
                        && !is_reparse(&metadata) =>
                {
                    if mutation.kind == PackageMutationKind::Install
                        && !artifact.unlocked_conflicts.contains(&mutation.package_id)
                    {
                        return Err(error(M7UnityMigrationErrorCode::SourceChanged));
                    }
                    std::fs::rename(&target, &backup).map_err(|cause| {
                        error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause)
                    })?;
                    alcomd_platform::sync_directory(&packages_root)
                        .and_then(|()| alcomd_platform::sync_directory(&backup_root))
                        .map_err(|cause| {
                            error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause)
                        })?;
                }
                Ok(_) => return Err(error(M7UnityMigrationErrorCode::RecoveryRequired)),
                Err(value) if value.kind() == std::io::ErrorKind::NotFound => {
                    if mutation.kind != PackageMutationKind::Install {
                        return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
                    }
                }
                Err(cause) => {
                    return Err(
                        error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause)
                    );
                }
            }
        }
        if mutation.kind == PackageMutationKind::Remove {
            if target.exists() {
                return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
            }
            continue;
        }
        let expected_version = mutation
            .to_version
            .as_deref()
            .ok_or_else(|| error(M7UnityMigrationErrorCode::RecoveryRequired))?;
        if !target.exists() {
            if !prepared.exists() {
                return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
            }
            std::fs::rename(&prepared, &target).map_err(|cause| {
                error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause)
            })?;
            alcomd_platform::sync_directory(&packages_root).map_err(|cause| {
                error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause)
            })?;
        }
        validate_extracted_package(&target, &mutation.package_id, expected_version).map_err(
            |cause| error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause),
        )?;
        if artifact.prepared_package_trees.get(&mutation.package_id)
            != Some(&tree_fingerprint(&target).map_err(|cause| {
                error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause)
            })?)
        {
            return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
        }
    }
    Ok(())
}

fn replace_migration_manifest(
    target: &Path,
    operation_root: &Path,
    name: &str,
    expected_before: [u8; 32],
    expected_after: [u8; 32],
) -> Result<(), M7UnityMigrationError> {
    let backup_root = operation_root.join("backup");
    std::fs::create_dir_all(&backup_root)
        .map_err(|cause| error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause))?;
    let backup = backup_root.join(format!("{name}.old"));
    let prepared = operation_root
        .join("materialized")
        .join(format!("{name}.new"));
    if hash_optional_regular_file(target)? == Some(expected_after) {
        return Ok(());
    }
    if !backup.exists() {
        if hash_optional_regular_file(target)? != Some(expected_before) {
            return Err(error(M7UnityMigrationErrorCode::SourceChanged));
        }
        std::fs::rename(target, &backup).map_err(|cause| {
            error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause)
        })?;
        sync_parent(target)?;
        alcomd_platform::sync_directory(&backup_root).map_err(|cause| {
            error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause)
        })?;
    } else if target.exists() {
        return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
    }
    if !prepared.exists() {
        return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
    }
    std::fs::rename(&prepared, target)
        .map_err(|cause| error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause))?;
    sync_parent(target)?;
    if hash_optional_regular_file(target)? != Some(expected_after) {
        return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
    }
    Ok(())
}

fn quarantine_legacy_paths(
    root: &Path,
    operation_root: &Path,
    paths: &[LegacyPathPlan],
) -> Result<(), M7UnityMigrationError> {
    let quarantine_root = operation_root.join("backup").join("legacy");
    std::fs::create_dir_all(&quarantine_root)
        .map_err(|cause| error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause))?;
    for planned in paths {
        let relative = Path::new(&planned.relative_path);
        let target = root.join(relative);
        let quarantine = quarantine_root.join(relative);
        if quarantine.exists() {
            if target.exists()
                || alcomd_platform::file_identity_key(&quarantine).ok()
                    != planned.filesystem_identity
            {
                return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
            }
            continue;
        }
        match planned.kind {
            LegacyPathKind::Missing => {
                if target.exists() {
                    return Err(error(M7UnityMigrationErrorCode::SourceChanged));
                }
            }
            LegacyPathKind::File | LegacyPathKind::Directory => {
                validate_existing_component_chain(root, relative)?;
                if alcomd_platform::file_identity_key(&target).ok() != planned.filesystem_identity {
                    return Err(error(M7UnityMigrationErrorCode::SourceChanged));
                }
                let parent = quarantine
                    .parent()
                    .ok_or_else(|| error(M7UnityMigrationErrorCode::RecoveryRequired))?;
                std::fs::create_dir_all(parent).map_err(|cause| {
                    error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause)
                })?;
                std::fs::rename(&target, &quarantine).map_err(|cause| {
                    error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause)
                })?;
                sync_parent(&target)?;
                alcomd_platform::sync_directory(parent).map_err(|cause| {
                    error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause)
                })?;
            }
        }
    }
    Ok(())
}

fn verify_prepared_project(
    root: &Path,
    artifact: &MigrationPreparationArtifact,
) -> Result<(), M7UnityMigrationError> {
    if hash_optional_regular_file(&root.join("Packages").join("vpm-manifest.json"))?
        != Some(artifact.vpm_manifest_sha256)
        || hash_optional_regular_file(&root.join("Packages").join("manifest.json"))?
            != Some(artifact.upm_manifest_after_sha256)
    {
        return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
    }
    for mutation in &artifact.change_set.mutations {
        let target = root.join("Packages").join(&mutation.package_id);
        if mutation.kind == PackageMutationKind::Remove {
            if target.exists() {
                return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
            }
            continue;
        }
        let expected_version = mutation
            .to_version
            .as_deref()
            .ok_or_else(|| error(M7UnityMigrationErrorCode::RecoveryRequired))?;
        validate_extracted_package(&target, &mutation.package_id, expected_version).map_err(
            |cause| {
                error(M7UnityMigrationErrorCode::RecoveryRequired)
                    .with_source(cause)
                    .with_path(root.display().to_string())
            },
        )?;
        if artifact.prepared_package_trees.get(&mutation.package_id)
            != Some(&tree_fingerprint(&target).map_err(|cause| {
                error(M7UnityMigrationErrorCode::RecoveryRequired)
                    .with_source(cause)
                    .with_path(root.display().to_string())
            })?)
        {
            return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
        }
    }
    Ok(())
}

fn hash_optional_regular_file(path: &Path) -> Result<Option<[u8; 32]>, M7UnityMigrationError> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata)
            if metadata.is_file()
                && !metadata.file_type().is_symlink()
                && !is_reparse(&metadata)
                && metadata.len() <= 4 * 1024 * 1024 =>
        {
            Ok(Some(
                Sha256::digest(std::fs::read(path).map_err(|cause| {
                    error(M7UnityMigrationErrorCode::RecoveryRequired)
                        .with_source(cause)
                        .with_path(path.display().to_string())
                })?)
                .into(),
            ))
        }
        Ok(_) => Err(error(M7UnityMigrationErrorCode::RecoveryRequired)),
        Err(value) if value.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(cause) => Err(error(M7UnityMigrationErrorCode::RecoveryRequired).with_source(cause)),
    }
}

fn sync_parent(path: &Path) -> Result<(), M7UnityMigrationError> {
    alcomd_platform::sync_directory(
        path.parent()
            .ok_or_else(|| error(M7UnityMigrationErrorCode::RecoveryRequired))?,
    )
    .map_err(|cause| {
        error(M7UnityMigrationErrorCode::RecoveryRequired)
            .with_source(cause)
            .with_path(path.display().to_string())
    })
}

fn migration_requests(
    project: &ProjectRecord,
    target_unity_version: &str,
) -> Result<Vec<ResolveRequest>, M7UnityMigrationError> {
    let unity_version = crate::UnityVersion::parse_editor(target_unity_version)
        .ok_or_else(|| error(M7UnityMigrationErrorCode::Unsupported))?;
    let locked = project
        .observation
        .locked_dependencies
        .iter()
        .map(|dependency| (dependency.package_id.as_str(), dependency.value.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut requests = BTreeMap::<String, String>::new();
    for dependency in &project.observation.direct_dependencies {
        requests.insert(
            dependency.package_id.clone(),
            crate::range::ProjectDependencyRange::parse(&dependency.value)
                .map_err(|cause| error(M7UnityMigrationErrorCode::Unsupported).with_source(cause))?
                .canonical(),
        );
    }
    for package_id in VRCHAT_PACKAGE_IDS {
        if let Some(version) = locked.get(package_id) {
            requests.insert(package_id.to_owned(), format!(">={version}"));
        }
    }
    Ok(requests
        .into_iter()
        .map(|(package_id, range)| ResolveRequest {
            package_id,
            range,
            source: None,
            include_prerelease: false,
            unity_version: Some(unity_version),
        })
        .collect())
}

fn validate_obligation_scope(
    obligations: &[LegacyCleanupObligation],
) -> Result<(), M7UnityMigrationError> {
    if obligations.len() > LEGACY_ENTRY_LIMIT
        || obligations
            .iter()
            .any(|obligation| !VRCHAT_PACKAGE_IDS.contains(&obligation.package_id.as_str()))
    {
        return Err(error(M7UnityMigrationErrorCode::Unsupported));
    }
    Ok(())
}

fn sealed_legacy_paths(
    root: &Path,
    metadata: &crate::package::LegacyMetadata,
) -> Result<Vec<String>, M7UnityMigrationError> {
    let paths = metadata
        .files
        .keys()
        .chain(metadata.folders.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    // Validate the existing sealed path scope before reading any target metadata.
    let planned = validate_legacy_paths(root, paths.iter().cloned().collect())?;
    let mut result = Vec::new();
    for path in planned {
        let guid = metadata
            .files
            .get(&path.relative_path)
            .or_else(|| metadata.folders.get(&path.relative_path))
            .ok_or_else(|| error(M7UnityMigrationErrorCode::Unsupported))?;
        let single = crate::package::LegacyMetadata {
            files: BTreeMap::from([(path.relative_path.clone(), guid.clone())]),
            ..Default::default()
        };
        if crate::legacy::verify_absent(root, &single).is_ok() {
            continue;
        }
        if path.kind == LegacyPathKind::Missing {
            return Err(error(M7UnityMigrationErrorCode::Unsupported));
        }
        let meta_relative = format!("{}.meta", path.relative_path);
        let meta_path = root.join(&meta_relative);
        match std::fs::symlink_metadata(&meta_path) {
            Ok(_) if !paths.contains(&meta_relative) => {
                return Err(error(M7UnityMigrationErrorCode::Unsupported));
            }
            Ok(_) => {}
            Err(cause) if cause.kind() == std::io::ErrorKind::NotFound && guid.is_empty() => {}
            Err(cause) => {
                return Err(error(M7UnityMigrationErrorCode::Unsupported).with_source(cause));
            }
        }
        if !guid.is_empty() {
            let bytes = read_regular_file(&meta_path, 64 * 1024)?;
            let text = std::str::from_utf8(&bytes).map_err(|cause| {
                error(M7UnityMigrationErrorCode::Unsupported).with_source(cause)
            })?;
            if !text.lines().any(|line| {
                line.trim()
                    .strip_prefix("guid:")
                    .is_some_and(|value| value.trim().eq_ignore_ascii_case(guid))
            }) {
                return Err(error(M7UnityMigrationErrorCode::Unsupported));
            }
        }
        result.push(path.relative_path);
    }
    Ok(result)
}

fn validate_legacy_paths(
    root: &Path,
    paths: Vec<String>,
) -> Result<Vec<LegacyPathPlan>, M7UnityMigrationError> {
    if paths.len() > LEGACY_ENTRY_LIMIT {
        return Err(error(M7UnityMigrationErrorCode::Unsupported));
    }
    let mut collisions = BTreeMap::<String, String>::new();
    let mut result = BTreeSet::new();
    for value in paths {
        if value.is_empty()
            || value.len() > LEGACY_PATH_LENGTH_LIMIT
            || value.contains('\\')
            || value.starts_with("//")
        {
            return Err(error(M7UnityMigrationErrorCode::Unsupported));
        }
        let relative = Path::new(&value);
        let components = relative.components().collect::<Vec<_>>();
        if components.is_empty()
            || components.len() > LEGACY_PATH_DEPTH_LIMIT
            || components
                .iter()
                .any(|component| !matches!(component, Component::Normal(_)))
            || !matches!(
                components[0].as_os_str().to_str(),
                Some("Assets" | "Packages")
            )
        {
            return Err(error(M7UnityMigrationErrorCode::Unsupported));
        }
        let normalized = components
            .iter()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        let collision_key = normalized
            .nfc()
            .flat_map(char::to_lowercase)
            .collect::<String>();
        if collisions
            .insert(collision_key, normalized.clone())
            .is_some_and(|existing| existing != normalized)
        {
            return Err(error(M7UnityMigrationErrorCode::Unsupported));
        }
        validate_existing_component_chain(root, relative)?;
        result.insert(normalized);
    }
    let values = result.into_iter().collect::<Vec<_>>();
    for pair in values.windows(2) {
        if pair[1].starts_with(&format!("{}/", pair[0])) {
            return Err(error(M7UnityMigrationErrorCode::Unsupported));
        }
    }
    values
        .into_iter()
        .map(|relative_path| {
            let target = root.join(&relative_path);
            let (kind, filesystem_identity) = match std::fs::symlink_metadata(&target) {
                Ok(metadata) if metadata.file_type().is_symlink() || is_reparse(&metadata) => {
                    return Err(error(M7UnityMigrationErrorCode::Unsupported));
                }
                Ok(metadata) if metadata.is_file() => (
                    LegacyPathKind::File,
                    Some(
                        alcomd_platform::file_identity_key(&target).map_err(|cause| {
                            error(M7UnityMigrationErrorCode::SourceChanged)
                                .with_source(cause)
                                .with_path(root.display().to_string())
                        })?,
                    ),
                ),
                Ok(metadata) if metadata.is_dir() => (
                    LegacyPathKind::Directory,
                    Some(
                        alcomd_platform::file_identity_key(&target).map_err(|cause| {
                            error(M7UnityMigrationErrorCode::SourceChanged)
                                .with_source(cause)
                                .with_path(root.display().to_string())
                        })?,
                    ),
                ),
                Ok(_) => return Err(error(M7UnityMigrationErrorCode::Unsupported)),
                Err(value) if value.kind() == std::io::ErrorKind::NotFound => {
                    (LegacyPathKind::Missing, None)
                }
                Err(cause) => {
                    return Err(error(M7UnityMigrationErrorCode::SourceChanged).with_source(cause));
                }
            };
            Ok(LegacyPathPlan {
                relative_path,
                kind,
                filesystem_identity,
            })
        })
        .collect()
}

fn validate_existing_component_chain(
    root: &Path,
    relative: &Path,
) -> Result<(), M7UnityMigrationError> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if !metadata.file_type().is_symlink() && !is_reparse(&metadata) => {}
            Ok(_) => return Err(error(M7UnityMigrationErrorCode::Unsupported)),
            Err(value) if value.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(cause) => {
                return Err(error(M7UnityMigrationErrorCode::SourceChanged).with_source(cause));
            }
        }
    }
    Ok(())
}

fn find_unlocked_conflicts(
    root: &Path,
    project: &ProjectRecord,
    change_set: &PackageChangeSet,
) -> Result<Vec<String>, M7UnityMigrationError> {
    let old_locked = project
        .observation
        .locked_dependencies
        .iter()
        .map(|dependency| dependency.package_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut conflicts = Vec::new();
    for mutation in &change_set.mutations {
        if mutation.kind != PackageMutationKind::Install
            || old_locked.contains(mutation.package_id.as_str())
        {
            continue;
        }
        let target = root.join("Packages").join(&mutation.package_id);
        match std::fs::symlink_metadata(&target) {
            Ok(metadata)
                if metadata.is_dir()
                    && !metadata.file_type().is_symlink()
                    && !is_reparse(&metadata) =>
            {
                conflicts.push(mutation.package_id.clone());
            }
            Ok(_) => return Err(error(M7UnityMigrationErrorCode::Unsupported)),
            Err(value) if value.kind() == std::io::ErrorKind::NotFound => {}
            Err(cause) => {
                return Err(error(M7UnityMigrationErrorCode::SourceChanged).with_source(cause));
            }
        }
    }
    Ok(conflicts)
}

fn validate_legacy_package_removals(
    change_set: &PackageChangeSet,
    legacy_packages: &BTreeSet<String>,
) -> Result<(), M7UnityMigrationError> {
    for package_id in legacy_packages {
        if !change_set.mutations.iter().any(|mutation| {
            mutation.package_id == *package_id && mutation.kind == PackageMutationKind::Remove
        }) {
            return Err(error(M7UnityMigrationErrorCode::Unsupported));
        }
    }
    Ok(())
}

fn remove_fixed_xr_dependencies(bytes: &[u8]) -> Result<Vec<u8>, M7UnityMigrationError> {
    let mut value = crate::parse_bounded_json(bytes, M3ErrorCode::ProjectManifestInvalid)
        .map_err(|cause| error(M7UnityMigrationErrorCode::Unsupported).with_source(cause))?;
    let dependencies = value
        .as_object_mut()
        .and_then(|object| object.get_mut("dependencies"))
        .and_then(Value::as_object_mut)
        .ok_or_else(|| error(M7UnityMigrationErrorCode::Unsupported))?;
    for package_id in XR_UPM_DEPENDENCIES {
        dependencies.remove(package_id);
    }
    let mut result = serde_json::to_vec_pretty(&value)
        .map_err(|cause| error(M7UnityMigrationErrorCode::Internal).with_source(cause))?;
    result.push(b'\n');
    Ok(result)
}

fn read_project_component(
    root: &Path,
    components: &[&str],
) -> Result<Vec<u8>, M7UnityMigrationError> {
    let mut path = root.to_path_buf();
    for (index, component) in components.iter().enumerate() {
        path.push(component);
        let metadata = std::fs::symlink_metadata(&path).map_err(|cause| {
            error(M7UnityMigrationErrorCode::SourceChanged)
                .with_source(cause)
                .with_path(root.display().to_string())
        })?;
        if metadata.file_type().is_symlink()
            || is_reparse(&metadata)
            || (index + 1 == components.len() && !metadata.is_file())
            || (index + 1 < components.len() && !metadata.is_dir())
        {
            return Err(error(M7UnityMigrationErrorCode::SourceChanged));
        }
    }
    read_regular_file(&path, 4 * 1024 * 1024).map_err(|cause| {
        error(M7UnityMigrationErrorCode::SourceChanged)
            .with_source(cause)
            .with_path(root.display().to_string())
    })
}

fn sync_tree(root: &Path) -> Result<(), M7UnityMigrationError> {
    let mut directories = vec![(root.to_path_buf(), 0_usize)];
    let mut sync_order = Vec::new();
    let mut entries = 0_usize;
    while let Some((directory, depth)) = directories.pop() {
        if depth > LEGACY_PATH_DEPTH_LIMIT {
            return Err(error(M7UnityMigrationErrorCode::Unsupported));
        }
        sync_order.push(directory.clone());
        for entry in std::fs::read_dir(&directory).map_err(|cause| {
            error(M7UnityMigrationErrorCode::Internal)
                .with_source(cause)
                .with_path(root.display().to_string())
        })? {
            entries = entries
                .checked_add(1)
                .filter(|count| *count <= 65_536)
                .ok_or_else(|| error(M7UnityMigrationErrorCode::Unsupported))?;
            let entry = entry.map_err(|cause| {
                error(M7UnityMigrationErrorCode::Internal)
                    .with_source(cause)
                    .with_path(root.display().to_string())
            })?;
            let metadata = std::fs::symlink_metadata(entry.path()).map_err(|cause| {
                error(M7UnityMigrationErrorCode::Internal)
                    .with_source(cause)
                    .with_path(root.display().to_string())
            })?;
            if metadata.file_type().is_symlink() || is_reparse(&metadata) {
                return Err(error(M7UnityMigrationErrorCode::Unsupported));
            }
            if metadata.is_dir() {
                directories.push((entry.path(), depth + 1));
            } else if metadata.is_file() {
                std::fs::OpenOptions::new()
                    .read(true)
                    .open(entry.path())
                    .and_then(|file| file.sync_all())
                    .map_err(|cause| {
                        error(M7UnityMigrationErrorCode::Internal)
                            .with_source(cause)
                            .with_path(root.display().to_string())
                    })?;
            } else {
                return Err(error(M7UnityMigrationErrorCode::Unsupported));
            }
        }
    }
    for directory in sync_order.into_iter().rev() {
        alcomd_platform::sync_directory(&directory).map_err(|cause| {
            error(M7UnityMigrationErrorCode::Internal)
                .with_source(cause)
                .with_path(root.display().to_string())
        })?;
    }
    Ok(())
}

fn fingerprint(value: &impl Serialize) -> Result<String, M7UnityMigrationError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|cause| error(M7UnityMigrationErrorCode::Internal).with_source(cause))?;
    Ok(hex(&Sha256::digest(bytes).into()))
}

fn project_version_marker_sha256(root: &Path) -> Result<[u8; 32], M7UnityMigrationError> {
    let path = root.join("ProjectSettings").join("ProjectVersion.txt");
    let metadata = std::fs::symlink_metadata(&path).map_err(|cause| {
        error(M7UnityMigrationErrorCode::SourceChanged)
            .with_source(cause)
            .with_path(root.display().to_string())
    })?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > PROJECT_VERSION_LIMIT
        || is_reparse(&metadata)
    {
        return Err(error(M7UnityMigrationErrorCode::SourceChanged));
    }
    let bytes = std::fs::read(path).map_err(|cause| {
        error(M7UnityMigrationErrorCode::SourceChanged)
            .with_source(cause)
            .with_path(root.display().to_string())
    })?;
    Ok(Sha256::digest(bytes).into())
}

fn read_regular_file(path: &Path, limit: u64) -> Result<Vec<u8>, M7UnityMigrationError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|cause| {
        error(M7UnityMigrationErrorCode::RecoveryRequired)
            .with_source(cause)
            .with_path(path.display().to_string())
    })?;
    if !metadata.is_file()
        || metadata.file_type().is_symlink()
        || metadata.len() > limit
        || is_reparse(&metadata)
    {
        return Err(error(M7UnityMigrationErrorCode::RecoveryRequired));
    }
    std::fs::read(path).map_err(|cause| {
        error(M7UnityMigrationErrorCode::RecoveryRequired)
            .with_source(cause)
            .with_path(path.display().to_string())
    })
}

fn write_new_synced_file(path: &Path, bytes: &[u8]) -> Result<(), M7UnityMigrationError> {
    use std::io::Write as _;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|cause| {
            error(M7UnityMigrationErrorCode::Internal)
                .with_source(cause)
                .with_path(path.display().to_string())
        })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|cause| {
            error(M7UnityMigrationErrorCode::Internal)
                .with_source(cause)
                .with_path(path.display().to_string())
        })
}

fn require_no_target_writer(writer: &UnityWriterState) -> Result<(), M7UnityMigrationError> {
    if matches!(
        writer.state,
        UnityWriterStateKind::NotObserved | UnityWriterStateKind::Unknown
    ) {
        Ok(())
    } else {
        Err(error(M7UnityMigrationErrorCode::ProjectRunning))
    }
}

fn recovery_observation_started(evidence: &UnityMigrationEvidence) -> Option<u64> {
    evidence.safe_evidence.iter().find_map(|value| {
        value
            .strip_prefix("recovery_observation_started:")
            .and_then(|value| value.parse().ok())
    })
}

fn hex(value: &[u8; 32]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(64);
    for byte in value {
        result.push(DIGITS[(byte >> 4) as usize] as char);
        result.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    result
}

#[cfg(windows)]
fn is_reparse(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt as _;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse(_: &std::fs::Metadata) -> bool {
    false
}

fn error(code: M7UnityMigrationErrorCode) -> M7UnityMigrationError {
    M7UnityMigrationError::new(code)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn migration_authority_projection_preserves_frozen_bytes_and_hash() {
        // These are the complete pre-projection authority bytes, not an object-map re-encoding.
        const FROZEN: &str = r#"{"version":1,"project":{"project_id":"00000000-0000-4000-8000-000000000101","observation":{"root_path":"unopened-project","path_identity_key":[1],"project_type":"unknown","unity_version":"2022.3.22f1","unity_revision":null,"vpm_manifest":"valid","upm_manifest":"missing","direct_dependencies":[],"locked_dependencies":[],"issues":[]},"revision":1,"registered_at_ms":11,"favorite":true},"sourceUnityVersion":"2022.3.22f1","projectRootIdentity":[1],"projectVersionMarkerSha256":[3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3,3],"target":{"installation_id":"00000000-0000-4000-8000-000000000102","observation":{"executable_path":"unopened-editor","filesystem_identity":[2],"unity_version":"2022.3.23f1","architecture":"x86_64","source_kind":"manual","observed_at_ms":1},"revision":1,"updated_at_ms":1},"writer":{"project_id":"00000000-0000-4000-8000-000000000101","state":"not_observed","evidence":[],"checked_at_ms":1},"classification":"patch_or_minor_upgrade","preparationProfile":null}"#;
        const FROZEN_HASH: &str =
            "c3fd587292c757f21ab0faf1f4c012522b5bc717864eb9412d4c9a9cc254c8bb";
        let fixture: Value = serde_json::from_str(FROZEN).expect("frozen authority JSON");
        let mut project: ProjectRecord =
            serde_json::from_value(fixture["project"].clone()).expect("frozen project");
        let target: UnityInstallationRecord =
            serde_json::from_value(fixture["target"].clone()).expect("frozen target");
        let writer: UnityWriterState =
            serde_json::from_value(fixture["writer"].clone()).expect("frozen writer");
        for activity in [None, Some(0), Some(99)] {
            project.last_modified_at_ms = activity;
            let authority = PlanAuthorityFingerprint {
                version: 1,
                project: ProjectAuthorityFingerprint::from(&project),
                source_unity_version: "2022.3.22f1",
                project_root_identity: &[1],
                project_version_marker_sha256: [3; 32],
                target: &target,
                writer: &writer,
                classification: "patch_or_minor_upgrade",
                preparation_profile: None,
            };
            assert_eq!(
                serde_json::to_vec(&authority).expect("production authority bytes"),
                FROZEN.as_bytes()
            );
            assert_eq!(
                fingerprint(&authority).expect("production authority hash"),
                FROZEN_HASH
            );
        }
    }

    #[tokio::test]
    async fn migration_authority_survives_reinspection_but_tracks_project_evidence() {
        use alcomd_application::{
            ProjectId, Revision, UnityArchitecture, UnityInstallationId,
            UnityInstallationObservation, UnitySourceKind,
        };

        let fixture = TestDirectory::new("authority-reinspection");
        fs::create_dir(fixture.path().join("ProjectSettings")).expect("project settings");
        let marker = fixture.path().join("ProjectSettings/ProjectVersion.txt");
        fs::write(&marker, b"m_EditorVersion: 2022.3.22f1\n").expect("project version");
        let reader = VpmReader::new().expect("reader");
        let inspect = || {
            reader.inspect_project(
                fixture.path().to_string_lossy().into_owned(),
                ProjectDiscoveryMode::ExactRoot,
            )
        };
        let mut project = ProjectRecord {
            project_id: ProjectId::new(),
            observation: inspect().await.expect("inspect actual project"),
            revision: Revision::INITIAL,
            registered_at_ms: 1,
            last_modified_at_ms: Some(10),
            favorite: false,
        };
        let target = UnityInstallationRecord {
            installation_id: UnityInstallationId::new(),
            observation: UnityInstallationObservation {
                executable_path: "unopened-editor".to_owned(),
                filesystem_identity: vec![2],
                unity_version: "2022.3.23f1".to_owned(),
                architecture: UnityArchitecture::X86_64,
                source_kind: UnitySourceKind::Manual,
                observed_at_ms: 1,
            },
            revision: Revision::INITIAL,
            updated_at_ms: 1,
        };
        let writer = UnityWriterState {
            project_id: project.project_id,
            state: UnityWriterStateKind::NotObserved,
            evidence: Vec::new(),
            checked_at_ms: 1,
        };
        let hash = |project: &ProjectRecord| {
            fingerprint(&PlanAuthorityFingerprint {
                version: 1,
                project: ProjectAuthorityFingerprint::from(project),
                source_unity_version: "2022.3.22f1",
                project_root_identity: &[1],
                project_version_marker_sha256: [3; 32],
                target: &target,
                writer: &writer,
                classification: "patch_or_minor_upgrade",
                preparation_profile: None,
            })
            .expect("hash production migration authority")
        };
        let before = hash(&project);
        project.observation = inspect().await.expect("reinspect unchanged project");
        assert_eq!(hash(&project), before);
        project.last_modified_at_ms = Some(20);
        assert_eq!(hash(&project), before);
        project.last_modified_at_ms = None;
        assert_eq!(hash(&project), before);
        project.revision = project.revision.checked_next().expect("next revision");
        assert_ne!(hash(&project), before);
        project.revision = Revision::INITIAL;
        fs::write(&marker, b"m_EditorVersion: 2022.3.23f1\n").expect("change project version");
        project.observation = inspect().await.expect("reinspect changed project");
        assert_ne!(hash(&project), before);
    }

    #[test]
    fn sealed_profile_removes_only_the_two_fixed_xr_dependencies() {
        let result = remove_fixed_xr_dependencies(
            br#"{
                "dependencies": {
                    "com.unity.xr.oculus.standalone": "1.0.0",
                    "com.unity.xr.openvr.standalone": "2.0.0",
                    "com.example.keep": "3.0.0"
                },
                "scopedRegistries": [{"name":"keep"}]
            }"#,
        )
        .expect("rewrite UPM manifest");
        let value: Value = serde_json::from_slice(&result).expect("parse rewritten manifest");
        let dependencies = value["dependencies"]
            .as_object()
            .expect("dependency object");
        assert!(!dependencies.contains_key("com.unity.xr.oculus.standalone"));
        assert!(!dependencies.contains_key("com.unity.xr.openvr.standalone"));
        assert_eq!(dependencies["com.example.keep"], "3.0.0");
        assert_eq!(value["scopedRegistries"][0]["name"], "keep");
    }

    #[test]
    fn legacy_metadata_cannot_expand_the_sealed_companion_file_scope() {
        let fixture = TestDirectory::new("legacy-guid-scope");
        let root = fixture.path();
        fs::create_dir(root.join("Assets")).expect("assets");
        let mut metadata = crate::package::LegacyMetadata {
            folders: BTreeMap::from([(
                "Assets/Old".to_owned(),
                "0123456789abcdef0123456789abcdef".to_owned(),
            )]),
            ..Default::default()
        };
        assert!(
            sealed_legacy_paths(root, &metadata)
                .expect("no actual cleanup")
                .is_empty()
        );
        fs::create_dir(root.join("Assets/Old")).expect("legacy target");
        fs::write(
            root.join("Assets/Old.meta"),
            "guid: 0123456789abcdef0123456789abcdef\n",
        )
        .expect("companion");
        assert_eq!(
            sealed_legacy_paths(root, &metadata)
                .expect_err("no implicit companion deletion")
                .code(),
            M7UnityMigrationErrorCode::Unsupported
        );
        metadata
            .files
            .insert("Assets/Old.meta".to_owned(), String::new());
        assert_eq!(
            sealed_legacy_paths(root, &metadata).expect("explicit exact paths only"),
            ["Assets/Old", "Assets/Old.meta"]
        );
    }

    #[test]
    fn legacy_paths_are_root_confined_collision_safe_and_idempotently_quarantined() {
        let fixture = TestDirectory::new("legacy-paths");
        let root = fixture.path().join("Project");
        let operation_root = fixture.path().join("Operation");
        fs::create_dir_all(root.join("Assets/Legacy")).expect("create legacy directory");
        fs::create_dir_all(root.join("Packages")).expect("create Packages");
        fs::write(root.join("Assets/Legacy/file.txt"), b"legacy").expect("write legacy file");

        let plans = validate_legacy_paths(
            &root,
            vec![
                "Assets/Legacy/file.txt".to_owned(),
                "Packages/Missing".to_owned(),
            ],
        )
        .expect("validate fixed legacy paths");
        assert_eq!(plans.len(), 2);
        quarantine_legacy_paths(&root, &operation_root, &plans).expect("quarantine legacy paths");
        quarantine_legacy_paths(&root, &operation_root, &plans)
            .expect("resume completed quarantine idempotently");
        assert!(!root.join("Assets/Legacy/file.txt").exists());
        assert_eq!(
            fs::read(operation_root.join("backup/legacy/Assets/Legacy/file.txt"))
                .expect("read quarantined file"),
            b"legacy"
        );

        for rejected in [
            "../escape",
            "/absolute",
            "Assets\\windows-separator",
            "ProjectSettings/ProjectVersion.txt",
        ] {
            assert_eq!(
                validate_legacy_paths(&root, vec![rejected.to_owned()])
                    .expect_err("reject unsealed legacy path")
                    .code(),
                M7UnityMigrationErrorCode::Unsupported
            );
        }
        assert_eq!(
            validate_legacy_paths(
                &root,
                vec!["Assets/Foo".to_owned(), "Assets/foo".to_owned()],
            )
            .expect_err("reject case collision")
            .code(),
            M7UnityMigrationErrorCode::Unsupported
        );
        assert_eq!(
            validate_legacy_paths(
                &root,
                vec!["Assets/Legacy".to_owned(), "Assets/Legacy/child".to_owned()],
            )
            .expect_err("reject parent-child overlap")
            .code(),
            M7UnityMigrationErrorCode::Unsupported
        );
    }

    #[test]
    fn manifest_replace_and_transaction_markers_resume_without_reapplying_mutation() {
        let fixture = TestDirectory::new("transaction-recovery");
        let operation_id = OperationId::new();
        let operation_root = fixture.path().join(operation_id.to_string());
        let project_manifest = fixture.path().join("Project/Packages/vpm-manifest.json");
        let materialized = operation_root.join("materialized/vpm-manifest.new");
        fs::create_dir_all(project_manifest.parent().expect("manifest parent"))
            .expect("create Project Packages");
        fs::create_dir_all(materialized.parent().expect("materialized parent"))
            .expect("create materialized directory");
        let before = b"{\"locked\":{}}\n";
        let after = b"{\"locked\":{\"com.vrchat.base\":\"3.7.0\"}}\n";
        fs::write(&project_manifest, before).expect("write old manifest");
        fs::write(&materialized, after).expect("write new manifest");

        replace_migration_manifest(
            &project_manifest,
            &operation_root,
            "vpm-manifest",
            Sha256::digest(before).into(),
            Sha256::digest(after).into(),
        )
        .expect("replace manifest");
        replace_migration_manifest(
            &project_manifest,
            &operation_root,
            "vpm-manifest",
            Sha256::digest(before).into(),
            Sha256::digest(after).into(),
        )
        .expect("resume already-published manifest");
        assert_eq!(
            fs::read(&project_manifest).expect("read final manifest"),
            after
        );
        assert_eq!(
            fs::read(operation_root.join("backup/vpm-manifest.old")).expect("read manifest backup"),
            before
        );

        for phase in [
            PreparationTransactionPhase::Prepared,
            PreparationTransactionPhase::PackagesReplaced,
            PreparationTransactionPhase::VpmManifestReplaced,
            PreparationTransactionPhase::UpmManifestReplaced,
            PreparationTransactionPhase::Complete,
        ] {
            write_transaction_marker(&operation_root, operation_id, phase)
                .expect("append transaction marker");
            assert_eq!(
                latest_transaction_phase(&operation_root).expect("read latest phase"),
                Some(phase)
            );
        }
        assert_eq!(
            write_transaction_marker(
                &operation_root,
                operation_id,
                PreparationTransactionPhase::Complete,
            )
            .expect_err("bounded marker journal rejects a sixth entry")
            .code(),
            M7UnityMigrationErrorCode::RecoveryRequired
        );
    }

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("alcomd-m7-unity-{label}-{}", OperationId::new()));
            fs::create_dir(&path).expect("create test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
