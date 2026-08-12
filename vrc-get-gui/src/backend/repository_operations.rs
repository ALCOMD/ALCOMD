use crate::commands::RustError;
use crate::state::{GuiConfigState, PackagesState, RepositoryConfigState, SettingsState};
use indexmap::IndexMap;
use std::collections::{BTreeMap, HashMap};
use url::Url;
use vrc_get_vpm::environment::{
    CURATED_REPOSITORY_ID, CURATED_URL_STR, DownloadedRemoteRepository, OFFICIAL_REPOSITORY_ID,
    OFFICIAL_URL_STR, Settings, add_downloaded_remote_repo, clear_package_cache,
    download_remote_repo,
};
use vrc_get_vpm::io::{DefaultEnvironmentIo, IoTrait};
use vrc_get_vpm::repositories_file::RepositoriesFile;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RepositoryKind {
    OfficialDefault,
    CuratedDefault,
    User,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepositorySummary {
    pub(crate) id: String,
    pub(crate) url: String,
    pub(crate) name: String,
    pub(crate) display_name: String,
    pub(crate) kind: RepositoryKind,
    pub(crate) hidden: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepositoryNames {
    pub(crate) name: String,
    pub(crate) display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RepositorySettingsSnapshot {
    pub(crate) repositories: Vec<RepositorySummary>,
    pub(crate) hidden_repository_ids: Vec<String>,
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
    pub(crate) name: String,
    pub(crate) packages: Vec<PackageManifest>,
    pub(crate) repository: DownloadedRemoteRepository,
}

#[derive(Debug, Clone)]
pub(crate) enum DownloadRepositoryOutcome {
    Duplicated {
        reason: RepositoryDuplicateReason,
        duplicated_name: String,
        duplicated_original_name: Option<String>,
    },
    DownloadError(String),
    Success(DownloadedRepository),
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedRepository {
    pub(crate) url: Url,
    pub(crate) headers: IndexMap<Box<str>, Box<str>>,
    pub(crate) repository: DownloadedRemoteRepository,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AddRepositoryFailure {
    pub(crate) index: usize,
    pub(crate) message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AddRepositoriesOutcome {
    pub(crate) succeeded: Vec<usize>,
    pub(crate) failures: Vec<AddRepositoryFailure>,
}

#[derive(Debug, Clone)]
pub(crate) struct RepositoryIdentitySnapshot {
    urls: HashMap<String, RepositoryNames>,
    ids: HashMap<String, RepositoryNames>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AddedRepositoryInfo {
    pub(crate) id: Option<String>,
    pub(crate) url: String,
    pub(crate) name: String,
    pub(crate) display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RemovedRepositoryInfo {
    pub(crate) id: Option<String>,
    pub(crate) url: String,
    pub(crate) name: String,
    pub(crate) display_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RemoveRepositoryOutcome {
    Removed(RemovedRepositoryInfo),
    NotFound,
}

pub(crate) async fn add_repository(
    settings: &SettingsState,
    packages: &PackagesState,
    repository_config: &RepositoryConfigState,
    io: &DefaultEnvironmentIo,
    http: &reqwest::Client,
    url: Url,
    headers: IndexMap<Box<str>, Box<str>>,
) -> Result<AddedRepositoryInfo, RustError> {
    let repository = download_remote_repo(&url, &headers, http).await?;
    add_prepared_repository(
        settings,
        packages,
        repository_config,
        io,
        PreparedRepository {
            url,
            headers,
            repository,
        },
    )
    .await
}

pub(crate) async fn add_prepared_repository(
    settings: &SettingsState,
    packages: &PackagesState,
    repository_config: &RepositoryConfigState,
    io: &DefaultEnvironmentIo,
    repository: PreparedRepository,
) -> Result<AddedRepositoryInfo, RustError> {
    let repository_url = repository.url.clone();
    let mut settings = settings.load_mut(io).await?;
    let previous_repo_count = settings.get_user_repos().len();
    add_downloaded_remote_repo(
        &mut settings,
        repository.url,
        None,
        repository.headers,
        io,
        repository.repository,
    )
    .await?;

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
    let name = repository
        .name()
        .or(repository.id())
        .unwrap_or(repository_url.as_str())
        .to_string();

    let settings_snapshot = settings.clone();
    settings.save().await?;
    set_repository_display_name(repository_config, repository_url.to_string(), name.clone())
        .await?;
    packages.reload_from_cache(&settings_snapshot, io).await?;

    Ok(AddedRepositoryInfo {
        id,
        url: repository_url.to_string(),
        display_name: name.clone(),
        name,
    })
}

pub(crate) async fn remove_repository(
    settings: &SettingsState,
    packages: &PackagesState,
    repository_config: &RepositoryConfigState,
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
    let name = removed
        .name()
        .or(removed.id())
        .unwrap_or(repository_url.as_str())
        .to_string();
    let display_name = repository_config
        .get()
        .display_names
        .get(repository_url.as_str())
        .cloned()
        .unwrap_or_else(|| name.clone());
    let local_path = removed.local_path().to_path_buf();

    settings.save().await?;
    remove_repository_display_name(repository_config, repository_url.as_str()).await?;
    io.remove_file(&local_path).await.ok();
    packages.clear_cache();

    Ok(RemoveRepositoryOutcome::Removed(RemovedRepositoryInfo {
        id,
        url: repository_url.to_string(),
        name,
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

pub(crate) async fn set_repository_display_name(
    repository_config: &RepositoryConfigState,
    repository_url: String,
    display_name: String,
) -> Result<(), RustError> {
    let repository_url = Url::parse(&repository_url)
        .map_err(|_| RustError::unrecoverable_str("repository_url must be a valid URL"))?
        .to_string();
    let display_name = display_name.trim();
    if display_name.is_empty() {
        return Err(RustError::unrecoverable_str(
            "Repository display names must not be empty.",
        ));
    }
    if display_name.chars().count() > 100 {
        return Err(RustError::unrecoverable_str(
            "Repository display names must be 100 characters or fewer.",
        ));
    }

    let mut config = repository_config.load_mut().await;
    config
        .display_names
        .insert(repository_url, display_name.to_string());
    config.save().await?;
    Ok(())
}

async fn remove_repository_display_name(
    repository_config: &RepositoryConfigState,
    repository_url: &str,
) -> Result<(), RustError> {
    let mut config = repository_config.load_mut().await;
    config.display_names.remove(repository_url);
    config.save().await?;
    Ok(())
}

pub(crate) async fn repository_settings_snapshot(
    settings: &SettingsState,
    config: &GuiConfigState,
    repository_config: &RepositoryConfigState,
    io: &DefaultEnvironmentIo,
) -> Result<RepositorySettingsSnapshot, RustError> {
    let config = config.get();
    let hidden_repository_ids = config
        .gui_hidden_repositories
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    let hide_local_user_packages = config.hide_local_user_packages;
    drop(config);
    let repository_display_names = repository_config.get().display_names.clone();

    let settings = settings.load(io).await?;
    let mut repositories = Vec::new();
    if !settings.ignore_official_repository() {
        repositories.push(default_repository_summary(
            OFFICIAL_REPOSITORY_ID,
            OFFICIAL_URL_STR,
            repository_display_names.get(OFFICIAL_URL_STR).cloned(),
            RepositoryKind::OfficialDefault,
            hidden_repository_ids
                .iter()
                .any(|hidden| hidden == OFFICIAL_REPOSITORY_ID),
        ));
    }
    if !settings.ignore_curated_repository() {
        repositories.push(default_repository_summary(
            CURATED_REPOSITORY_ID,
            CURATED_URL_STR,
            repository_display_names.get(CURATED_URL_STR).cloned(),
            RepositoryKind::CuratedDefault,
            hidden_repository_ids
                .iter()
                .any(|hidden| hidden == CURATED_REPOSITORY_ID),
        ));
    }
    repositories.extend(settings.get_user_repos().iter().filter_map(|repository| {
        user_repository_summary(
            repository,
            repository
                .url()
                .and_then(|url| repository_display_names.get(url.as_str()).cloned()),
            repository_identity(repository)
                .is_some_and(|id| hidden_repository_ids.iter().any(|hidden| hidden == id)),
        )
    }));

    Ok(RepositorySettingsSnapshot {
        repositories,
        hidden_repository_ids,
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
    repository_config: &RepositoryConfigState,
    io: &DefaultEnvironmentIo,
    repositories: Vec<PreparedRepository>,
) -> Result<AddRepositoriesOutcome, RustError> {
    let mut settings = settings.load_mut(io).await?;
    let mut candidate = settings.clone();
    let repository_urls = repositories
        .iter()
        .map(|repository| repository.url.clone())
        .collect::<Vec<_>>();

    let mut succeeded = Vec::new();
    let mut failures = Vec::new();
    for (index, repository) in repositories.into_iter().enumerate() {
        let repository_url = repository.url.clone();
        match add_downloaded_remote_repo(
            &mut candidate,
            repository.url,
            None,
            repository.headers,
            io,
            repository.repository,
        )
        .await
        {
            Ok(()) => {
                succeeded.push(index);
            }
            Err(error) => {
                let message = error.to_string();
                log::warn!("failed to import repository {repository_url}: {message}");
                failures.push(AddRepositoryFailure { index, message });
            }
        }
    }

    if succeeded.is_empty() {
        settings.maybe_save().await?;
        return Ok(AddRepositoriesOutcome {
            succeeded,
            failures,
        });
    }

    let repository_display_names = succeeded
        .iter()
        .map(|&index| {
            let repository_url = &repository_urls[index];
            let repository = candidate
                .get_user_repos()
                .iter()
                .find(|repository| repository.url() == Some(repository_url))
                .expect("successfully downloaded repository must exist in candidate settings");
            let display_name = repository
                .name()
                .or(repository.id())
                .unwrap_or(repository_url.as_str())
                .to_string();
            (repository_url.to_string(), display_name)
        })
        .collect::<Vec<_>>();

    let mut config = repository_config.load_mut().await;
    for (repository_url, display_name) in repository_display_names {
        config.display_names.insert(repository_url, display_name);
    }
    config.save().await?;

    candidate.save(io).await?;
    *settings = candidate;
    let settings_snapshot = settings.clone();
    settings.maybe_save().await?;
    packages.reload_from_cache(&settings_snapshot, io).await?;

    Ok(AddRepositoriesOutcome {
        succeeded,
        failures,
    })
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

pub(crate) fn repository_identity_snapshot(
    settings: &Settings,
    display_names: &BTreeMap<String, String>,
) -> RepositoryIdentitySnapshot {
    let mut urls = settings
        .get_user_repos()
        .iter()
        .map(|repository| {
            let url = repository
                .url()
                .expect("user repositories loaded by Settings must have a URL");
            let names =
                repository_names(Some(url), repository.name(), repository.id(), display_names);
            (url.to_string(), names)
        })
        .collect::<HashMap<_, _>>();
    let mut ids = settings
        .get_user_repos()
        .iter()
        .filter_map(|repository| {
            repository.id().map(|id| {
                (
                    id.to_string(),
                    repository_names(repository.url(), repository.name(), Some(id), display_names),
                )
            })
        })
        .collect::<HashMap<_, _>>();
    if !settings.ignore_curated_repository() {
        urls.insert(
            CURATED_URL_STR.to_string(),
            repository_names(
                Url::parse(CURATED_URL_STR).ok().as_ref(),
                Some(CURATED_REPOSITORY_ID),
                Some(CURATED_REPOSITORY_ID),
                display_names,
            ),
        );
        ids.insert(
            CURATED_REPOSITORY_ID.to_string(),
            repository_names(
                Url::parse(CURATED_URL_STR).ok().as_ref(),
                Some(CURATED_REPOSITORY_ID),
                Some(CURATED_REPOSITORY_ID),
                display_names,
            ),
        );
    }
    if !settings.ignore_official_repository() {
        urls.insert(
            OFFICIAL_URL_STR.to_string(),
            repository_names(
                Url::parse(OFFICIAL_URL_STR).ok().as_ref(),
                Some(OFFICIAL_REPOSITORY_ID),
                Some(OFFICIAL_REPOSITORY_ID),
                display_names,
            ),
        );
        ids.insert(
            OFFICIAL_REPOSITORY_ID.to_string(),
            repository_names(
                Url::parse(OFFICIAL_URL_STR).ok().as_ref(),
                Some(OFFICIAL_REPOSITORY_ID),
                Some(OFFICIAL_REPOSITORY_ID),
                display_names,
            ),
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
    if let Some(names) = identities.urls.get(repository_url.as_str()) {
        return Ok(DownloadRepositoryOutcome::Duplicated {
            reason: RepositoryDuplicateReason::Url,
            duplicated_name: names.display_name.clone(),
            duplicated_original_name: (names.display_name != names.name)
                .then(|| names.name.clone()),
        });
    }
    let repository = match download_remote_repo(repository_url, headers, client).await {
        Ok(repository) => repository,
        Err(error) => return Ok(DownloadRepositoryOutcome::DownloadError(error.to_string())),
    };
    let remote_repository = repository.repository();
    let url = remote_repository.url().unwrap_or(repository_url).as_str();
    let id = remote_repository.id().unwrap_or(url);
    if let Some(names) = identities.ids.get(id) {
        return Ok(DownloadRepositoryOutcome::Duplicated {
            reason: RepositoryDuplicateReason::Id,
            duplicated_name: names.display_name.clone(),
            duplicated_original_name: (names.display_name != names.name)
                .then(|| names.name.clone()),
        });
    }
    Ok(DownloadRepositoryOutcome::Success(DownloadedRepository {
        id: id.to_string(),
        url: url.to_string(),
        name: remote_repository.name().unwrap_or(id).to_string(),
        packages: remote_repository
            .get_packages()
            .filter_map(|package| package.get_latest(VersionSelector::latest_for(None, true)))
            .filter(|package| !package.is_yanked())
            .cloned()
            .collect(),
        repository,
    }))
}

pub(crate) fn reserve_downloaded_repository(
    identities: &mut RepositoryIdentitySnapshot,
    outcome: &mut DownloadRepositoryOutcome,
) {
    let DownloadRepositoryOutcome::Success(repository) = outcome else {
        return;
    };
    if let Some(names) = identities.ids.get(&repository.id) {
        *outcome = DownloadRepositoryOutcome::Duplicated {
            reason: RepositoryDuplicateReason::Id,
            duplicated_name: names.display_name.clone(),
            duplicated_original_name: (names.display_name != names.name)
                .then(|| names.name.clone()),
        };
    } else {
        let names = RepositoryNames {
            name: repository.name.clone(),
            display_name: repository.name.clone(),
        };
        identities.ids.insert(repository.id.clone(), names.clone());
        identities.urls.insert(repository.url.clone(), names);
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

pub(crate) fn repository_names(
    url: Option<&Url>,
    name: Option<&str>,
    id: Option<&str>,
    display_names: &BTreeMap<String, String>,
) -> RepositoryNames {
    let name = name.or(id).or(url.map(Url::as_str)).unwrap_or("Unknown");
    let display_name = url
        .and_then(|url| display_names.get(url.as_str()).cloned())
        .unwrap_or_else(|| name.to_string());
    RepositoryNames {
        name: name.to_string(),
        display_name,
    }
}

fn default_repository_summary(
    id: &str,
    url: &str,
    display_name: Option<String>,
    kind: RepositoryKind,
    hidden: bool,
) -> RepositorySummary {
    RepositorySummary {
        id: id.to_string(),
        url: url.to_string(),
        name: id.to_string(),
        display_name: display_name.unwrap_or_else(|| id.to_string()),
        kind,
        hidden,
    }
}

fn user_repository_summary(
    repository: &UserRepoSetting,
    display_name: Option<String>,
    hidden: bool,
) -> Option<RepositorySummary> {
    let url = repository.url()?;
    let id = repository.id().unwrap_or(url.as_str());
    Some(RepositorySummary {
        id: id.to_string(),
        url: url.to_string(),
        name: repository.name().unwrap_or(id).to_string(),
        display_name: display_name.unwrap_or_else(|| repository.name().unwrap_or(id).to_string()),
        kind: RepositoryKind::User,
        hidden,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::AsyncRead;
    use futures::io::Cursor;
    use std::io as std_io;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct RepositoryImportHttp {
        requests: AtomicUsize,
    }

    impl HttpClient for RepositoryImportHttp {
        async fn get(
            &self,
            _url: &Url,
            _headers: &IndexMap<&str, &str>,
        ) -> std_io::Result<impl AsyncRead + Send> {
            Ok(Cursor::new(Vec::new()))
        }

        async fn get_with_etag(
            &self,
            url: &Url,
            _headers: &IndexMap<Box<str>, Box<str>>,
            _current_etag: Option<&str>,
        ) -> std_io::Result<Option<(impl AsyncRead + Send, Option<Box<str>>)>> {
            self.requests.fetch_add(1, Ordering::Relaxed);
            if url.path().contains("timeout") {
                return Err(std_io::Error::new(
                    std_io::ErrorKind::TimedOut,
                    "repository request timed out",
                ));
            }

            let repository_id = url
                .path_segments()
                .and_then(|mut segments| segments.next_back())
                .unwrap_or("repository")
                .trim_end_matches(".json");
            let contents = format!(
                r#"{{"id":"com.example.{repository_id}","name":"{repository_id}","url":"{url}","customField":"preserved","packages":{{}}}}"#
            );
            Ok(Some((
                Cursor::new(contents.into_bytes()),
                Some("test-etag".into()),
            )))
        }
    }

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

        let summary =
            user_repository_summary(&repository, Some("My Repository".to_string()), true).unwrap();

        assert_eq!(summary.id, "com.example.repository");
        assert_eq!(summary.url, "https://example.com/index.json");
        assert_eq!(summary.name, "Example Repository");
        assert_eq!(summary.display_name, "My Repository");
        assert_eq!(summary.kind, RepositoryKind::User);
        assert!(summary.hidden);
    }

    #[test]
    fn removing_repository_clears_its_display_name() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(removing_repository_clears_its_display_name_inner());
    }

    async fn removing_repository_clears_its_display_name_inner() {
        let temp = tempfile::tempdir().unwrap();
        let io = DefaultEnvironmentIo::new(temp.path().into());
        tokio::fs::write(
            temp.path().join("settings.json"),
            br#"{"userRepos":[{"localPath":"Repos/example.json","url":"https://example.com/index.json","id":"com.example.repository","headers":{}}]}"#,
        )
        .await
        .unwrap();
        let settings = SettingsState::new();
        let packages = PackagesState::new();
        let repository_config = RepositoryConfigState::new_load(&io).await.unwrap();
        set_repository_display_name(
            &repository_config,
            "https://example.com/index.json".to_string(),
            "Example".to_string(),
        )
        .await
        .unwrap();

        let outcome = remove_repository(
            &settings,
            &packages,
            &repository_config,
            &io,
            Url::parse("https://example.com/index.json").unwrap(),
        )
        .await
        .unwrap();

        assert!(matches!(outcome, RemoveRepositoryOutcome::Removed(_)));
        assert!(repository_config.get().display_names.is_empty());
        assert!(
            settings
                .load(&io)
                .await
                .unwrap()
                .get_user_repos()
                .is_empty()
        );
    }

    #[test]
    fn repository_import_reuses_prepared_downloads() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(repository_import_reuses_prepared_downloads_inner());
    }

    async fn repository_import_reuses_prepared_downloads_inner() {
        let temp = tempfile::tempdir().unwrap();
        let io = DefaultEnvironmentIo::new(temp.path().into());
        let settings = SettingsState::new();
        let packages = PackagesState::new();
        let repository_config = RepositoryConfigState::new_load(&io).await.unwrap();
        let descriptors = ["first.json", "second.json"]
            .into_iter()
            .map(|name| RepositoryDescriptor {
                url: Url::parse(&format!("https://example.com/{name}")).unwrap(),
                headers: IndexMap::new(),
            })
            .collect::<Vec<_>>();
        let http = RepositoryImportHttp::default();
        let mut repositories = Vec::new();
        for descriptor in descriptors {
            let repository = download_remote_repo(&descriptor.url, &descriptor.headers, &http)
                .await
                .unwrap();
            repositories.push(PreparedRepository {
                url: descriptor.url,
                headers: descriptor.headers,
                repository,
            });
        }
        assert_eq!(http.requests.load(Ordering::Relaxed), 2);

        let outcome = add_repositories(&settings, &packages, &repository_config, &io, repositories)
            .await
            .unwrap();

        assert_eq!(outcome.succeeded, vec![0, 1]);
        assert!(outcome.failures.is_empty());
        assert_eq!(http.requests.load(Ordering::Relaxed), 2);
        assert!(packages.get().is_some());

        let settings = settings.load(&io).await.unwrap();
        assert_eq!(settings.get_user_repos().len(), 2);
        assert!(settings.get_user_repos().iter().any(|repository| {
            repository
                .url()
                .is_some_and(|url| url.path() == "/first.json")
        }));
        assert!(settings.get_user_repos().iter().any(|repository| {
            repository
                .url()
                .is_some_and(|url| url.path() == "/second.json")
        }));
        for repository in settings.get_user_repos() {
            let cache = tokio::fs::read_to_string(repository.local_path())
                .await
                .unwrap();
            assert!(cache.contains(r#""customField": "preserved""#));
            assert!(cache.contains(r#""etag": "test-etag""#));
        }
        assert_eq!(repository_config.get().display_names.len(), 2);
    }

    #[test]
    fn repository_preview_reports_download_failure() {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                let http = RepositoryImportHttp::default();
                let outcome = download_repository(
                    &http,
                    &Url::parse("https://example.com/timeout.json").unwrap(),
                    &IndexMap::new(),
                    &RepositoryIdentitySnapshot {
                        urls: HashMap::new(),
                        ids: HashMap::new(),
                    },
                )
                .await
                .unwrap();

                assert!(matches!(
                    outcome,
                    DownloadRepositoryOutcome::DownloadError(_)
                ));
                assert_eq!(http.requests.load(Ordering::Relaxed), 1);
            });
    }
}
