use std::path::PathBuf;

use alcomd_application::{
    ExtensionFilesystemJournalEntry, ExtensionJournalPhase, ExtensionJournalState,
    ExtensionPackageEvidence, ExtensionPlanRecord, ExtensionSourceKind, ExtensionUiProtocol,
    M6Error, M6ErrorCode, M6PackageAdapter, M6Store, OperationId,
};

use crate::{
    PackageError, PackageErrorCode, VerifiedExtensionPackage, extract_extension_package,
    inspect_extension_directory, inspect_extension_package,
};

#[derive(Clone)]
pub struct ExtensionEngine<S: M6Store> {
    store: S,
    root: PathBuf,
}

impl<S: M6Store> ExtensionEngine<S> {
    pub fn new(store: S, root: PathBuf) -> Result<Self, M6Error> {
        if !root.is_absolute() {
            return Err(error(M6ErrorCode::InvalidInput));
        }
        std::fs::create_dir_all(root.join("packages"))
            .and_then(|()| std::fs::create_dir_all(root.join("staging")))
            .and_then(|()| std::fs::create_dir_all(root.join("backups")))
            .map_err(|_| error(M6ErrorCode::RecoveryRequired))?;
        let root = std::fs::canonicalize(root).map_err(|_| error(M6ErrorCode::RecoveryRequired))?;
        Ok(Self { store, root })
    }

    fn package_path(&self, plan: &ExtensionPlanRecord) -> PathBuf {
        self.root
            .join("packages")
            .join(&plan.evidence.extension_id)
            .join(hex(&plan.evidence.package_digest))
    }

    fn staging_path(&self, operation_id: OperationId) -> PathBuf {
        self.root.join("staging").join(operation_id.to_string())
    }

    fn backup_path(&self, operation_id: OperationId) -> PathBuf {
        self.root.join("backups").join(operation_id.to_string())
    }

    async fn journal(
        &self,
        operation_id: OperationId,
        plan: &ExtensionPlanRecord,
        phase: ExtensionJournalPhase,
        state: ExtensionJournalState,
    ) -> Result<(), M6Error> {
        let step = self
            .store
            .next_filesystem_journal_step(operation_id)
            .await?;
        self.store
            .append_filesystem_journal(ExtensionFilesystemJournalEntry {
                operation_id,
                step,
                plan_id: plan.plan_id,
                extension_id: plan.evidence.extension_id.clone(),
                action: plan.action.clone(),
                phase,
                state,
                evidence_json: format!(
                    "{{\"packageDigest\":\"{}\",\"version\":1}}",
                    hex(&plan.evidence.package_digest)
                ),
                updated_at_ms: time_ms()?,
            })
            .await
    }
}

impl<S: M6Store> M6PackageAdapter for ExtensionEngine<S> {
    async fn inspect(
        &self,
        source_kind: ExtensionSourceKind,
        path: String,
    ) -> Result<ExtensionPackageEvidence, M6Error> {
        if source_kind == ExtensionSourceKind::NotApplicable {
            return Err(error(M6ErrorCode::InvalidInput));
        }
        let source = PathBuf::from(path);
        let (canonical, identity) = tokio::task::spawn_blocking(move || {
            let metadata = std::fs::symlink_metadata(&source)
                .map_err(|_| error(M6ErrorCode::PackageInvalid))?;
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                return Err(error(M6ErrorCode::PackageInvalid));
            }
            let canonical =
                std::fs::canonicalize(&source).map_err(|_| error(M6ErrorCode::PackageInvalid))?;
            let identity = alcomd_platform::file_identity_key(&canonical)
                .map_err(|_| error(M6ErrorCode::PackageInvalid))?;
            Ok((canonical, identity))
        })
        .await
        .map_err(|_| error(M6ErrorCode::Internal))??;
        let inspect_path = canonical.clone();
        let verified =
            tokio::task::spawn_blocking(move || inspect_extension_package(&inspect_path))
                .await
                .map_err(|_| error(M6ErrorCode::Internal))?
                .map_err(map_package_error)?;
        package_evidence(source_kind, canonical, identity, verified)
    }

    async fn verify_installed(
        &self,
        record: alcomd_application::ExtensionRecord,
        live_locator: String,
    ) -> Result<(), M6Error> {
        let path = PathBuf::from(live_locator);
        let verified = tokio::task::spawn_blocking(move || inspect_extension_directory(&path))
            .await
            .map_err(|_| error(M6ErrorCode::Internal))?
            .map_err(map_package_error)?;
        if record.extension_id != verified.manifest.id
            || record.version != verified.manifest.version
            || record.api_major != verified.manifest.api
            || record.package_digest != verified.package_digest
            || record.publisher_fingerprint != verified.publisher_fingerprint
            || record.ui_protocol != manifest_ui_protocol(&verified)
            || !required_interfaces_supported(&verified.manifest.interfaces.required)
        {
            return Err(error(M6ErrorCode::PackageInvalid));
        }
        Ok(())
    }

    async fn install(
        &self,
        operation_id: OperationId,
        plan: ExtensionPlanRecord,
    ) -> Result<String, M6Error> {
        let target = self.package_path(&plan);
        if self
            .store
            .filesystem_journal_has_phase(operation_id, ExtensionJournalPhase::PackagePublished)
            .await?
        {
            let recovery_target = target.clone();
            let expected = plan.clone();
            tokio::task::spawn_blocking(move || {
                let package =
                    inspect_extension_directory(&recovery_target).map_err(map_package_error)?;
                verify_plan(&expected, &package)
            })
            .await
            .map_err(|_| error(M6ErrorCode::Internal))??;
            self.journal(
                operation_id,
                &plan,
                ExtensionJournalPhase::StateCommitIntent,
                ExtensionJournalState::Intent,
            )
            .await?;
            return target
                .to_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| error(M6ErrorCode::RecoveryRequired));
        }
        let source = PathBuf::from(&plan.evidence.source_locator);
        let expected_identity = plan.evidence.source_identity.clone();
        let check_source = source.clone();
        let identity = tokio::task::spawn_blocking(move || {
            alcomd_platform::file_identity_key(&check_source)
                .map_err(|_| error(M6ErrorCode::PlanStale))
        })
        .await
        .map_err(|_| error(M6ErrorCode::Internal))??;
        if identity != expected_identity {
            return Err(error(M6ErrorCode::PlanStale));
        }
        self.journal(
            operation_id,
            &plan,
            ExtensionJournalPhase::SourceVerified,
            ExtensionJournalState::Completed,
        )
        .await?;
        let inspect_source = source.clone();
        let verified =
            tokio::task::spawn_blocking(move || inspect_extension_package(&inspect_source))
                .await
                .map_err(|_| error(M6ErrorCode::Internal))?
                .map_err(map_package_error)?;
        verify_plan(&plan, &verified)?;
        self.journal(
            operation_id,
            &plan,
            ExtensionJournalPhase::ArchiveVerified,
            ExtensionJournalState::Completed,
        )
        .await?;
        test_kill_gate("archive_verified")?;

        let staging = self.staging_path(operation_id);
        let extract_source = source;
        let extract_staging = staging.clone();
        let expected = plan.clone();
        tokio::task::spawn_blocking(move || {
            if extract_staging.exists() {
                std::fs::remove_dir_all(&extract_staging)
                    .map_err(|_| error(M6ErrorCode::RecoveryRequired))?;
            }
            let extracted = extract_extension_package(&extract_source, &extract_staging)
                .map_err(map_package_error)?;
            verify_plan(&expected, &extracted)
        })
        .await
        .map_err(|_| error(M6ErrorCode::Internal))??;
        self.journal(
            operation_id,
            &plan,
            ExtensionJournalPhase::StagingComplete,
            ExtensionJournalState::Completed,
        )
        .await?;
        test_kill_gate("staging_complete")?;
        self.journal(
            operation_id,
            &plan,
            ExtensionJournalPhase::PublishIntent,
            ExtensionJournalState::Intent,
        )
        .await?;
        let publish_staging = staging.clone();
        let publish_target = target.clone();
        let expected = plan.clone();
        tokio::task::spawn_blocking(move || {
            if publish_target.exists() {
                let existing =
                    inspect_extension_directory(&publish_target).map_err(map_package_error)?;
                verify_plan(&expected, &existing)?;
                if publish_staging.exists() {
                    std::fs::remove_dir_all(&publish_staging)
                        .map_err(|_| error(M6ErrorCode::RecoveryRequired))?;
                }
                return Ok(());
            }
            let parent = publish_target
                .parent()
                .ok_or_else(|| error(M6ErrorCode::Internal))?;
            std::fs::create_dir_all(parent).map_err(|_| error(M6ErrorCode::RecoveryRequired))?;
            std::fs::rename(&publish_staging, &publish_target)
                .map_err(|_| error(M6ErrorCode::RecoveryRequired))?;
            alcomd_platform::sync_directory(parent)
                .map_err(|_| error(M6ErrorCode::RecoveryRequired))
        })
        .await
        .map_err(|_| error(M6ErrorCode::Internal))??;
        self.journal(
            operation_id,
            &plan,
            ExtensionJournalPhase::PackagePublished,
            ExtensionJournalState::Completed,
        )
        .await?;
        test_kill_gate("package_published")?;
        self.journal(
            operation_id,
            &plan,
            ExtensionJournalPhase::StateCommitIntent,
            ExtensionJournalState::Intent,
        )
        .await?;
        target
            .to_str()
            .map(ToOwned::to_owned)
            .ok_or_else(|| error(M6ErrorCode::RecoveryRequired))
    }

    async fn uninstall(
        &self,
        operation_id: OperationId,
        plan: ExtensionPlanRecord,
    ) -> Result<(), M6Error> {
        self.journal(
            operation_id,
            &plan,
            ExtensionJournalPhase::HostStopped,
            ExtensionJournalState::Completed,
        )
        .await?;
        self.journal(
            operation_id,
            &plan,
            ExtensionJournalPhase::PackageBackupIntent,
            ExtensionJournalState::Intent,
        )
        .await?;
        let target = self.package_path(&plan);
        let backup = self.backup_path(operation_id);
        let expected = plan.clone();
        tokio::task::spawn_blocking(move || {
            if backup.exists() {
                let existing = inspect_extension_directory(&backup).map_err(map_package_error)?;
                return verify_plan(&expected, &existing);
            }
            let existing = inspect_extension_directory(&target).map_err(map_package_error)?;
            verify_plan(&expected, &existing)?;
            std::fs::rename(&target, &backup).map_err(|_| error(M6ErrorCode::RecoveryRequired))?;
            let parent = target
                .parent()
                .ok_or_else(|| error(M6ErrorCode::Internal))?;
            alcomd_platform::sync_directory(parent)
                .map_err(|_| error(M6ErrorCode::RecoveryRequired))
        })
        .await
        .map_err(|_| error(M6ErrorCode::Internal))??;
        self.journal(
            operation_id,
            &plan,
            ExtensionJournalPhase::PackageMovedToBackup,
            ExtensionJournalState::Completed,
        )
        .await?;
        test_kill_gate("package_moved_to_backup")?;
        if plan.data_disposition == Some(alcomd_application::ExtensionDataDisposition::DeleteData) {
            self.journal(
                operation_id,
                &plan,
                ExtensionJournalPhase::DataDeleteIntent,
                ExtensionJournalState::Intent,
            )
            .await?;
        }
        self.journal(
            operation_id,
            &plan,
            ExtensionJournalPhase::StateCommitIntent,
            ExtensionJournalState::Intent,
        )
        .await
    }

    async fn cleanup(
        &self,
        operation_id: OperationId,
        plan: ExtensionPlanRecord,
    ) -> Result<(), M6Error> {
        let staging = self.staging_path(operation_id);
        let backup = self.backup_path(operation_id);
        tokio::task::spawn_blocking(move || {
            for path in [staging, backup] {
                if path.exists() {
                    std::fs::remove_dir_all(path)
                        .map_err(|_| error(M6ErrorCode::RecoveryRequired))?;
                }
            }
            Ok(())
        })
        .await
        .map_err(|_| error(M6ErrorCode::Internal))??;
        self.journal(
            operation_id,
            &plan,
            ExtensionJournalPhase::CleanupComplete,
            ExtensionJournalState::Completed,
        )
        .await
    }
}

fn package_evidence(
    source_kind: ExtensionSourceKind,
    source: PathBuf,
    source_identity: Vec<u8>,
    package: VerifiedExtensionPackage,
) -> Result<ExtensionPackageEvidence, M6Error> {
    if !required_interfaces_supported(&package.manifest.interfaces.required) {
        return Err(error(M6ErrorCode::ApiUnsupported));
    }
    let source_locator = source
        .to_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| error(M6ErrorCode::PackageInvalid))?;
    let ui_protocol = manifest_ui_protocol(&package);
    Ok(ExtensionPackageEvidence {
        source_kind,
        source_locator,
        source_identity,
        extension_id: package.manifest.id,
        version: package.manifest.version,
        api_major: package.manifest.api,
        profile_version: 1,
        package_digest: package.package_digest,
        manifest_digest: package.manifest_digest,
        component_digest: package.component_digest,
        publisher_fingerprint: package.publisher_fingerprint,
        required_permissions: package.manifest.permissions.required,
        optional_permissions: package.manifest.permissions.optional,
        required_interfaces: package.manifest.interfaces.required,
        optional_interfaces: package.manifest.interfaces.optional,
        ui_protocol,
    })
}

fn required_interfaces_supported(interfaces: &[String]) -> bool {
    interfaces.iter().all(|interface| {
        matches!(
            interface.as_str(),
            "alcomd:extension/host-data@1.0.0" | "alcomd:extension/host-projects@1.0.0"
        )
    })
}

fn verify_plan(
    plan: &ExtensionPlanRecord,
    package: &VerifiedExtensionPackage,
) -> Result<(), M6Error> {
    let evidence = &plan.evidence;
    if evidence.extension_id != package.manifest.id
        || evidence.version != package.manifest.version
        || evidence.api_major != package.manifest.api
        || evidence.profile_version != 1
        || evidence.package_digest != package.package_digest
        || evidence.manifest_digest != package.manifest_digest
        || evidence.component_digest != package.component_digest
        || evidence.publisher_fingerprint != package.publisher_fingerprint
        || evidence.required_permissions != package.manifest.permissions.required
        || evidence.optional_permissions != package.manifest.permissions.optional
        || evidence.required_interfaces != package.manifest.interfaces.required
        || evidence.optional_interfaces != package.manifest.interfaces.optional
        || evidence.ui_protocol != manifest_ui_protocol(package)
    {
        return Err(error(M6ErrorCode::PlanStale));
    }
    Ok(())
}

fn manifest_ui_protocol(package: &VerifiedExtensionPackage) -> Option<ExtensionUiProtocol> {
    package
        .manifest
        .ui
        .as_ref()
        .map(|_| ExtensionUiProtocol::PortableV1)
}

fn map_package_error(error: PackageError) -> M6Error {
    match error.code() {
        PackageErrorCode::ManifestInvalid => M6Error::new(M6ErrorCode::ManifestInvalid),
        PackageErrorCode::SignatureInvalid => M6Error::new(M6ErrorCode::SignatureInvalid),
        PackageErrorCode::Io => M6Error::new(M6ErrorCode::RecoveryRequired),
        _ => M6Error::new(M6ErrorCode::PackageInvalid),
    }
}

fn error(code: M6ErrorCode) -> M6Error {
    M6Error::new(code)
}

fn time_ms() -> Result<u64, M6Error> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| error(M6ErrorCode::Internal))
        .and_then(|duration| {
            u64::try_from(duration.as_millis()).map_err(|_| error(M6ErrorCode::Internal))
        })
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

#[cfg(feature = "test-kill-gates")]
fn test_kill_gate(checkpoint: &str) -> Result<(), M6Error> {
    if std::env::var("ALCOMD_TEST_M6_KILL_GATE").as_deref() != Ok(checkpoint) {
        return Ok(());
    }
    let signal = std::env::var_os("ALCOMD_TEST_M6_KILL_SIGNAL")
        .ok_or_else(|| error(M6ErrorCode::Internal))?;
    std::fs::write(signal, checkpoint).map_err(|_| error(M6ErrorCode::RecoveryRequired))?;
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}

#[cfg(not(feature = "test-kill-gates"))]
fn test_kill_gate(_checkpoint: &str) -> Result<(), M6Error> {
    Ok(())
}
