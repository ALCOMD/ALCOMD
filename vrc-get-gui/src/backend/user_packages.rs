use crate::commands::RustError;
use crate::state::{GuiConfigState, PackagesState, SettingsState};
use std::path::{Path, PathBuf};
use vrc_get_vpm::PackageManifest;
use vrc_get_vpm::environment::{AddUserPackageResult, UserPackageCollection};
use vrc_get_vpm::io::DefaultEnvironmentIo;

#[derive(Debug, Clone)]
pub(crate) struct UserPackageInfo {
    pub(crate) path: PathBuf,
    pub(crate) package: PackageManifest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AddUserPackagesOutcome {
    Added,
    InvalidSelection,
    AlreadyAdded,
}

pub(crate) async fn list_user_packages(
    settings: &SettingsState,
    io: &DefaultEnvironmentIo,
) -> Result<Vec<UserPackageInfo>, RustError> {
    let settings = settings.load(io).await?;
    let packages = UserPackageCollection::load(&settings, io).await;
    Ok(packages
        .packages()
        .map(|(path, package)| UserPackageInfo {
            path: path.to_path_buf(),
            package: package.clone(),
        })
        .collect())
}

pub(crate) async fn add_user_packages(
    settings: &SettingsState,
    packages: &PackagesState,
    io: &DefaultEnvironmentIo,
    package_paths: &[PathBuf],
) -> Result<AddUserPackagesOutcome, RustError> {
    let mut normalized_paths = Vec::with_capacity(package_paths.len());
    for package_path in package_paths {
        if !package_path.is_absolute() {
            return Ok(AddUserPackagesOutcome::InvalidSelection);
        }
        let canonical = match tokio::fs::canonicalize(package_path).await {
            Ok(path) if path.is_absolute() => path,
            _ => return Ok(AddUserPackagesOutcome::InvalidSelection),
        };
        if normalized_paths.iter().any(|path| path == &canonical) {
            return Ok(AddUserPackagesOutcome::AlreadyAdded);
        }
        normalized_paths.push(canonical);
    }

    let mut settings = settings.load_mut(io).await?;
    let mut candidate = settings.clone();
    for package_path in &normalized_paths {
        match candidate.add_user_package(package_path, io).await {
            AddUserPackageResult::Success => {}
            AddUserPackageResult::NonAbsolute | AddUserPackageResult::BadPackage => {
                return Ok(AddUserPackagesOutcome::InvalidSelection);
            }
            AddUserPackageResult::AlreadyAdded => {
                return Ok(AddUserPackagesOutcome::AlreadyAdded);
            }
        }
    }
    *settings = candidate;
    settings.save().await?;
    packages.clear_cache();
    Ok(AddUserPackagesOutcome::Added)
}

pub(crate) async fn remove_user_package(
    settings: &SettingsState,
    packages: &PackagesState,
    io: &DefaultEnvironmentIo,
    package_path: &Path,
) -> Result<bool, RustError> {
    let mut settings = settings.load_mut(io).await?;
    let Some(stored_path) = settings
        .user_package_folders()
        .iter()
        .find(|stored| same_package_path(stored, package_path))
        .cloned()
    else {
        settings.maybe_save().await?;
        return Ok(false);
    };
    settings.remove_user_package(&stored_path);
    settings.save().await?;
    packages.clear_cache();
    Ok(true)
}

pub(crate) async fn set_user_packages_hidden(
    config: &GuiConfigState,
    hidden: bool,
) -> Result<(), RustError> {
    let mut config = config.load_mut().await?;
    config.hide_local_user_packages = hidden;
    config.save().await?;
    Ok(())
}

fn same_package_path(stored: &Path, requested: &Path) -> bool {
    if stored == requested {
        return true;
    }
    match (
        std::fs::canonicalize(stored),
        std::fs::canonicalize(requested),
    ) {
        (Ok(stored), Ok(requested)) => stored == requested,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_user_package_paths_match_without_io() {
        let path = Path::new("C:/Packages/com.example.package");
        assert!(same_package_path(path, path));
    }
}
