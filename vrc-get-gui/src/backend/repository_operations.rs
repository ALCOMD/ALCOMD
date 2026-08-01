use crate::commands::RustError;
use crate::state::{GuiConfigState, PackagesState, SettingsState};
use indexmap::IndexMap;
use std::collections::HashMap;
use url::Url;
use vrc_get_vpm::environment::{
    CURATED_REPOSITORY_ID, CURATED_URL_STR, OFFICIAL_REPOSITORY_ID, OFFICIAL_URL_STR, Settings,
    add_remote_repo, clear_package_cache,
};
use vrc_get_vpm::io::{DefaultEnvironmentIo, IoTrait};
use vrc_get_vpm::repositories_file::RepositoriesFile;
use vrc_get_vpm::repository::RemoteRepository;
use vrc_get_vpm::{HttpClient, PackageManifest, UserRepoSetting, VersionSelector};

#[derive(Debug, Clone)]
pub(crate) struct RepositoryDescriptor {
    pub(crate) url: Url,
    pub(crate) headers: IndexMap<Box<str>, Box<str>>,
}

#[derive(Debug, Clone)]
pub(crate) struct RepositoryFileContents {
    pub(crate) repositories: Vec<RepositoryDescriptor>,
    pub(crate) unparsable_lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UserRepositorySummary {
    pub(crate) id: String,
    pub(crate) url: String,
    pub(crate) display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepositorySettingsSnapshot {
    pub(crate) user_repositories: Vec<UserRepositorySummary>,
    pub(crate) hidden_user_repositories: Vec<String>,
    pub(crate) hide_local_user_packages: bool,
    pub(crate) show_prerelease_packages: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RepositoryDuplicateReason {
    Url,
    Id,
}

#[derive(Debug, Clone)]
pub(crate) struct DownloadedRepository {
    pub(crate) id: String,
    pub(crate) url: String,
    pub(crate) display_name: String,
    pub(crate) packages: Vec<PackageManifest>,
}

#[derive(Debug, Clone)]
pub(crate) enum DownloadRepositoryOutcome {
    Duplicated {
        reason: RepositoryDuplicateReason,
        duplicated_name: String,
    },
    DownloadError(String),
    Success(DownloadedRepository),
}

#[derive(Debug, Clone)]
pub(crate) struct RepositoryIdentitySnapshot {
    urls: HashMap<String, String>,
    ids: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AddedRepositoryInfo {
    pub(crate) id: Option<String>,
    pub(crate) url: String,
    pub(crate) display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemovedRepositoryInfo {
    pub(crate) id: Option<String>,
    pub(crate) url: String,
    pub(crate) display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RemoveRepositoryOutcome {
    Removed(RemovedRepositoryInfo),
    NotFound,
}

pub(crate) async fn add_repository(
    settings: &SettingsState,
    packages: &PackagesState,
    io: &DefaultEnvironmentIo,
    http: &reqwest::Client,
    url: Url,
    headers: IndexMap<Box<str>, Box<str>>,
) -> Result<AddedRepositoryInfo, RustError> {
    let repository_url = url.clone();
    let mut settings = settings.load_mut(io).await?;
    let previous_repo_count = settings.get_user_repos().len();
    add_remote_repo(&mut settings, url, None, headers, io, http).await?;

    let repository = settings
        .get_user_repos()
        .get(previous_repo_count)
        .filter(|repository| repository.url() == Some(&repository_url))
        .or_else(|| {
            settings
                .get_user_repos()
                .iter()
                .find(|repository| repository.url() == Some(&repository_url))
        })
        .ok_or_else(|| RustError::unrecoverable_str("added repository was not found"))?;
    let id = repository_identity(repository).map(str::to_string);
    let display_name = repository.name().map(str::to_string).or_else(|| id.clone());

    settings.save().await?;
    packages.clear_cache();

    Ok(AddedRepositoryInfo {
        id,
        url: repository_url.to_string(),
        display_name,
    })
}

pub(crate) async fn remove_repository(
    settings: &SettingsState,
    packages: &PackagesState,
    io: &DefaultEnvironmentIo,
    repository_url: Url,
) -> Result<RemoveRepositoryOutcome, RustError> {
    let mut settings = settings.load_mut(io).await?;
    let Some(index) = select_user_repository(settings.get_user_repos(), &repository_url) else {
        return Ok(RemoveRepositoryOutcome::NotFound);
    };

    let removed = settings
        .remove_repo_at_index(index)
        .expect("selected user repository must still exist while settings are locked");
    let id = repository_identity(&removed).map(str::to_string);
    let display_name = removed.name().map(str::to_string).or_else(|| id.clone());
    let local_path = removed.local_path().to_path_buf();

    settings.save().await?;
    io.remove_file(&local_path).await.ok();
    packages.clear_cache();

    Ok(RemoveRepositoryOutcome::Removed(RemovedRepositoryInfo {
        id,
        url: repository_url.to_string(),
        display_name,
    }))
}

pub(crate) async fn reorder_repositories(
    settings: &SettingsState,
    packages: &PackagesState,
    io: &DefaultEnvironmentIo,
    repository_urls: &[Url],
) -> Result<(), RustError> {
    let mut settings = settings.load_mut(io).await?;
    if !settings.reorder_user_repos(repository_urls) {
        return Err(RustError::unrecoverable_str(
            "Repository URLs must contain every remote user repository exactly once; please refresh.",
        ));
    }

    settings.save().await?;
    packages.clear_cache();
    Ok(())
}

pub(crate) async fn set_repository_hidden(
    config: &GuiConfigState,
    repository_id: String,
    hidden: bool,
) -> Result<(), RustError> {
    let mut config = config.load_mut().await?;
    if hidden {
        config.gui_hidden_repositories.insert(repository_id);
    } else {
        config.gui_hidden_repositories.shift_remove(&repository_id);
    }
    config.save().await?;
    Ok(())
}

pub(crate) async fn repository_settings_snapshot(
    settings: &SettingsState,
    config: &GuiConfigState,
    io: &DefaultEnvironmentIo,
) -> Result<RepositorySettingsSnapshot, RustError> {
    let config = config.get();
    let hidden_user_repositories = config.gui_hidden_repositories.iter().cloned().collect();
    let hide_local_user_packages = config.hide_local_user_packages;
    drop(config);

    let settings = settings.load(io).await?;
    let user_repositories = settings
        .get_user_repos()
        .iter()
        .filter_map(user_repository_summary)
        .collect();

    Ok(RepositorySettingsSnapshot {
        user_repositories,
        hidden_user_repositories,
        hide_local_user_packages,
        show_prerelease_packages: settings.show_prerelease_packages(),
    })
}

pub(crate) fn parse_repositories_file(contents: &str) -> RepositoryFileContents {
    let parsed = RepositoriesFile::parse(contents);
    RepositoryFileContents {
        repositories: parsed
            .parsed()
            .repositories()
            .iter()
            .map(|repository| RepositoryDescriptor {
                url: repository.url().clone(),
                headers: repository.headers().clone(),
            })
            .collect(),
        unparsable_lines: parsed.unparseable_lines().to_vec(),
    }
}

pub(crate) async fn add_repositories(
    settings: &SettingsState,
    packages: &PackagesState,
    io: &DefaultEnvironmentIo,
    http: &reqwest::Client,
    repositories: Vec<RepositoryDescriptor>,
) -> Result<(), RustError> {
    let mut settings = settings.load_mut(io).await?;
    let mut candidate = settings.clone();
    for repository in repositories {
        add_remote_repo(
            &mut candidate,
            repository.url,
            None,
            repository.headers,
            io,
            http,
        )
        .await?;
    }
    *settings = candidate;
    settings.save().await?;
    packages.clear_cache();
    Ok(())
}

pub(crate) async fn export_repositories(
    settings: &SettingsState,
    io: &DefaultEnvironmentIo,
    destination: &std::path::Path,
) -> Result<(), RustError> {
    let repositories = settings.load(io).await?.export_repositories();
    tokio::fs::write(destination, repositories).await?;
    Ok(())
}

pub(crate) async fn clear_repositories_cache(
    packages: &PackagesState,
    io: &DefaultEnvironmentIo,
) -> Result<(), RustError> {
    clear_package_cache(io).await?;
    packages.clear_cache();
    Ok(())
}

pub(crate) fn repository_identity_snapshot(settings: &Settings) -> RepositoryIdentitySnapshot {
    let mut urls = settings
        .get_user_repos()
        .iter()
        .map(|repository| {
            let url = repository
                .url()
                .expect("user repositories loaded by Settings must have a URL");
            (
                url.to_string(),
                repository
                    .name()
                    .or(repository.id())
                    .unwrap_or(url.as_str())
                    .to_string(),
            )
        })
        .collect::<HashMap<_, _>>();
    let mut ids = settings
        .get_user_repos()
        .iter()
        .filter_map(|repository| {
            repository
                .id()
                .map(|id| (id.to_string(), repository.name().unwrap_or(id).to_string()))
        })
        .collect::<HashMap<_, _>>();
    if !settings.ignore_curated_repository() {
        urls.insert(
            CURATED_URL_STR.to_string(),
            CURATED_REPOSITORY_ID.to_string(),
        );
        ids.insert(
            CURATED_REPOSITORY_ID.to_string(),
            CURATED_REPOSITORY_ID.to_string(),
        );
    }
    if !settings.ignore_official_repository() {
        urls.insert(
            OFFICIAL_URL_STR.to_string(),
            OFFICIAL_REPOSITORY_ID.to_string(),
        );
        ids.insert(
            OFFICIAL_REPOSITORY_ID.to_string(),
            OFFICIAL_REPOSITORY_ID.to_string(),
        );
    }
    RepositoryIdentitySnapshot { urls, ids }
}

pub(crate) async fn download_repository(
    client: &impl HttpClient,
    repository_url: &Url,
    headers: &IndexMap<Box<str>, Box<str>>,
    identities: &RepositoryIdentitySnapshot,
) -> Result<DownloadRepositoryOutcome, RustError> {
    if let Some(name) = identities.urls.get(repository_url.as_str()) {
        return Ok(DownloadRepositoryOutcome::Duplicated {
            reason: RepositoryDuplicateReason::Url,
            duplicated_name: name.clone(),
        });
    }
    let repository = match RemoteRepository::download(client, repository_url, headers).await {
        Ok((repository, _)) => repository,
        Err(error) => return Ok(DownloadRepositoryOutcome::DownloadError(error.to_string())),
    };
    let url = repository.url().unwrap_or(repository_url).as_str();
    let id = repository.id().unwrap_or(url);
    if let Some(name) = identities.ids.get(id) {
        return Ok(DownloadRepositoryOutcome::Duplicated {
            reason: RepositoryDuplicateReason::Id,
            duplicated_name: name.clone(),
        });
    }
    Ok(DownloadRepositoryOutcome::Success(DownloadedRepository {
        id: id.to_string(),
        url: url.to_string(),
        display_name: repository.name().unwrap_or(id).to_string(),
        packages: repository
            .get_packages()
            .filter_map(|package| package.get_latest(VersionSelector::latest_for(None, true)))
            .filter(|package| !package.is_yanked())
            .cloned()
            .collect(),
    }))
}

pub(crate) fn reserve_downloaded_repository(
    identities: &mut RepositoryIdentitySnapshot,
    outcome: &mut DownloadRepositoryOutcome,
) {
    let DownloadRepositoryOutcome::Success(repository) = outcome else {
        return;
    };
    if let Some(name) = identities.ids.get(&repository.id) {
        *outcome = DownloadRepositoryOutcome::Duplicated {
            reason: RepositoryDuplicateReason::Id,
            duplicated_name: name.clone(),
        };
    } else {
        identities
            .ids
            .insert(repository.id.clone(), repository.display_name.clone());
        identities
            .urls
            .insert(repository.url.clone(), repository.display_name.clone());
    }
}

fn select_user_repository(repositories: &[UserRepoSetting], repository_url: &Url) -> Option<usize> {
    repositories
        .iter()
        .enumerate()
        .find(|(_, repository)| repository.url() == Some(repository_url))
        .map(|(index, _)| index)
}

fn repository_identity(repository: &UserRepoSetting) -> Option<&str> {
    repository.id().or(repository.url().map(Url::as_str))
}

fn user_repository_summary(repository: &UserRepoSetting) -> Option<UserRepositorySummary> {
    let url = repository.url()?;
    let id = repository.id().unwrap_or(url.as_str());
    Some(UserRepositorySummary {
        id: id.to_string(),
        url: url.to_string(),
        display_name: repository.name().unwrap_or(id).to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn repository(url: &str, id: Option<&str>) -> UserRepoSetting {
        UserRepoSetting::new(
            PathBuf::from("Repos/example.json").into_boxed_path(),
            None,
            Some(Url::parse(url).unwrap()),
            id.map(Into::into),
        )
    }

    #[test]
    fn user_repository_selection_uses_url_as_primary_identity() {
        let repositories = vec![
            repository("https://example.com/first.json", Some("com.example.same")),
            repository("https://example.com/second.json", Some("com.example.same")),
        ];

        assert_eq!(
            select_user_repository(
                &repositories,
                &Url::parse("https://example.com/second.json").unwrap(),
            ),
            Some(1)
        );
    }

    #[test]
    fn user_repository_selection_requires_matching_url() {
        let repositories = vec![repository(
            "https://example.com/index.json",
            Some("com.example.repo"),
        )];

        assert_eq!(
            select_user_repository(
                &repositories,
                &Url::parse("https://example.com/other.json").unwrap(),
            ),
            None
        );
    }

    #[test]
    fn user_repository_summary_preserves_url_identity() {
        let repository = UserRepoSetting::new(
            PathBuf::from("Repos/remote.json").into_boxed_path(),
            Some("Example Repository".into()),
            Some(Url::parse("https://example.com/index.json").unwrap()),
            Some("com.example.repository".into()),
        );

        let summary = user_repository_summary(&repository).unwrap();

        assert_eq!(summary.id, "com.example.repository");
        assert_eq!(summary.url, "https://example.com/index.json");
        assert_eq!(summary.display_name, "Example Repository");
    }
}
