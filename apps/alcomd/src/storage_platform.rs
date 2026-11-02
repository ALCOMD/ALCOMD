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
        let (projects, backups) = platform::default_storage_directories()
            .map_err(|_| app::OfficialGuiError::Unavailable)?;
        Ok(app::StorageDirectories {
            project_directory: projects
                .into_os_string()
                .into_string()
                .map_err(|_| app::OfficialGuiError::Unavailable)?,
            backup_directory: backups
                .into_os_string()
                .into_string()
                .map_err(|_| app::OfficialGuiError::Unavailable)?,
        })
    }

    fn normalize_directory(&self, path: &str) -> Result<String, app::OfficialGuiError> {
        platform::normalize_storage_directory(path).map_err(|error| match error.kind() {
            std::io::ErrorKind::InvalidInput
            | std::io::ErrorKind::NotFound
            | std::io::ErrorKind::NotADirectory => app::OfficialGuiError::InvalidInput,
            _ => app::OfficialGuiError::Unavailable,
        })
    }
}

pub(super) struct ConfiguredBackupDirectory(pub M7OfficialGuiApplication);

impl app::BackupDirectoryProvider for ConfiguredBackupDirectory {
    fn directory(
        &self,
    ) -> Pin<Box<dyn Future<Output = Result<String, app::M5BackupError>> + Send + '_>> {
        Box::pin(async move {
            let snapshot =
                self.0.candidate_settings().await.map_err(|_| {
                    app::M5BackupError::new(app::M5BackupErrorCode::StoreUnavailable)
                })?;
            let directories = self
                .0
                .observe_storage_directories(&snapshot)
                .await
                .map_err(|_| app::M5BackupError::new(app::M5BackupErrorCode::StoreUnavailable))?;
            tokio::task::spawn_blocking(move || {
                let path = if snapshot.settings.paths.backup_directory.is_none() {
                    platform::prepare_storage_directory(Path::new(&directories.backup_directory))
                } else {
                    platform::normalize_storage_directory(&directories.backup_directory)
                        .map(std::path::PathBuf::from)
                }
                .map_err(|_| app::M5BackupError::new(app::M5BackupErrorCode::BackupSourceUnsafe))?;
                path.into_os_string().into_string().map_err(|_| {
                    app::M5BackupError::new(app::M5BackupErrorCode::BackupSourceUnsafe)
                })
            })
            .await
            .map_err(|_| app::M5BackupError::new(app::M5BackupErrorCode::Internal))?
        })
    }
}
