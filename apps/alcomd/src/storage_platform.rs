use std::future::Future;
use std::path::Path;
use std::pin::Pin;

use alcomd_application as app;
use alcomd_platform as platform;

use super::M7OfficialGuiApplication;

#[derive(Clone, Copy, Default)]
pub(super) struct PlatformStorageDirectories;

impl app::StorageDirectoryPlatform for PlatformStorageDirectories {
    fn default_directories(&self) -> Result<app::StorageDirectories, app::OfficialGuiError> {
        let (projects, backups) = platform::default_storage_directories().map_err(|error| {
            app::OfficialGuiError::Unavailable
                .with_source(error)
                .record("failed to resolve default storage directories")
        })?;
        Ok(app::StorageDirectories {
            project_directory: projects.into_os_string().into_string().map_err(|path| {
                app::OfficialGuiError::Unavailable
                    .with_reason("default project directory cannot be represented as UTF-8")
                    .with_path(path.to_string_lossy().into_owned())
                    .record("failed to resolve default project directory")
            })?,
            backup_directory: backups.into_os_string().into_string().map_err(|path| {
                app::OfficialGuiError::Unavailable
                    .with_reason("default backup directory cannot be represented as UTF-8")
                    .with_path(path.to_string_lossy().into_owned())
                    .record("failed to resolve default backup directory")
            })?,
        })
    }

    fn normalize_directory(&self, path: &str) -> Result<String, app::OfficialGuiError> {
        platform::normalize_storage_directory(path).map_err(|error| {
            let classified = match error.kind() {
                std::io::ErrorKind::InvalidInput
                | std::io::ErrorKind::NotFound
                | std::io::ErrorKind::NotADirectory => app::OfficialGuiError::InvalidInput,
                _ => app::OfficialGuiError::Unavailable,
            };
            classified
                .with_source(error)
                .with_path(path)
                .record("failed to validate storage directory")
        })
    }
}

pub(super) struct ConfiguredBackupDirectory(pub M7OfficialGuiApplication);

impl app::BackupDirectoryProvider for ConfiguredBackupDirectory {
    fn defaults(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<app::BackupDefaults, app::M5BackupError>> + Send + '_>>
    {
        Box::pin(async move {
            let snapshot = self
                .0
                .candidate_settings()
                .await
                .map_err(backup_settings_error)?;
            let directories = self
                .0
                .observe_storage_directories(&snapshot)
                .await
                .map_err(backup_settings_error)?;
            tokio::task::spawn_blocking(move || {
                let path = if snapshot.settings.paths.backup_directory.is_none() {
                    platform::prepare_storage_directory(Path::new(&directories.backup_directory))
                } else {
                    platform::normalize_storage_directory(&directories.backup_directory)
                        .map(std::path::PathBuf::from)
                }
                .map_err(|error| {
                    app::M5BackupError::new(app::M5BackupErrorCode::BackupSourceUnsafe)
                        .with_source(error)
                        .with_path(&directories.backup_directory)
                })?;
                let directory = path.into_os_string().into_string().map_err(|path| {
                    app::M5BackupError::new(app::M5BackupErrorCode::BackupSourceUnsafe)
                        .with_reason("backup directory cannot be represented as UTF-8")
                        .with_path(path.to_string_lossy().into_owned())
                })?;
                Ok(app::BackupDefaults {
                    directory: Some(directory),
                    compression_mode: snapshot.settings.backups.compression_mode,
                    include_vpm_packages: snapshot.settings.backups.include_vpm_packages,
                    include_library: snapshot.settings.backups.include_library,
                })
            })
            .await
            .map_err(|error| {
                app::M5BackupError::new(app::M5BackupErrorCode::Internal).with_source(error)
            })?
        })
    }
}

fn backup_settings_error(error: app::OfficialGuiError) -> app::M5BackupError {
    let diagnostic = error.diagnostic_id().map(str::to_owned);
    let mut value =
        app::M5BackupError::new(app::M5BackupErrorCode::StoreUnavailable).with_source(error);
    if let Some(diagnostic) = diagnostic {
        value = value.with_diagnostic_id(diagnostic);
    }
    value
}
