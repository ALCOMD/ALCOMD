use crate::activity_log::{
    ActivityDetail, ActivityImportance, ActivityInput, ActivityKind, ActivityLogState,
    ActivitySource, operations, summarize_path, summarize_url, summarize_url_host,
    target_from_path,
};
use crate::backend::packages::{
    latest_package_infos_by_source, package_is_available_for_display,
    repository_id as cached_repository_id,
};
use crate::backend::repository_operations;
use crate::backend::user_packages;
use crate::commands::async_command::{AsyncCallResult, With, async_command};
use crate::commands::prelude::*;
use futures::future::try_join_all;
use indexmap::IndexMap;
use log::info;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::{AppHandle, Manager, State, Window};
use tauri_plugin_dialog::DialogExt;
use url::Url;
use vrc_get_vpm::PackageInfo;
use vrc_get_vpm::environment::{
    CURATED_REPOSITORY_ID, CURATED_URL_STR, OFFICIAL_REPOSITORY_ID, OFFICIAL_URL_STR,
};
use vrc_get_vpm::io::DefaultEnvironmentIo;

#[tauri::command]
#[specta::specta]
pub async fn environment_refetch_packages(
    app: AppHandle,
    packages: State<'_, PackagesState>,
    settings: State<'_, SettingsState>,
    io: State<'_, DefaultEnvironmentIo>,
    http: State<'_, reqwest::Client>,
) -> Result<(), RustError> {
    let activity = app.state::<ActivityLogState>();
    activity
        .track_result(
            Some(&app),
            ActivityInput::new(
                ActivitySource::Gui,
                ActivityKind::Maintenance,
                ActivityImportance::Primary,
                operations::PACKAGES_REFRESH_CACHE,
                "Refreshing package cache",
            ),
            "Package cache refreshed",
            Vec::new(),
            async move {
                let refreshed = {
                    let settings_snapshot = settings.load(io.inner()).await?;
                    packages
                        .load_force(&settings_snapshot, io.inner(), http.inner())
                        .await?
                };
                sync_repository_names(&settings, io.inner(), refreshed.collection()).await?;

                Ok(())
            },
        )
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn environment_packages(
    app_handle: AppHandle,
    packages: State<'_, PackagesState>,
    settings: State<'_, SettingsState>,
    repository_config: State<'_, RepositoryConfigState>,
    io: State<'_, DefaultEnvironmentIo>,
    http: State<'_, reqwest::Client>,
) -> Result<Vec<TauriPackage>, RustError> {
    let settings = settings.load(io.inner()).await?;
    let packages = packages
        .load(&settings, io.inner(), http.inner(), app_handle)
        .await?;

    let display_names = repository_config.get().display_names.clone();
    Ok(packages
        .packages()
        .map(|value| TauriPackage::new_with_repository_display_names(value, &display_names))
        .collect::<Vec<_>>())
}

#[derive(Serialize, specta::Type, Clone)]
pub struct TauriRepositoryPackageList {
    repository_id: String,
    packages: Vec<TauriBasePackageInfo>,
}

#[derive(Serialize, specta::Type, Clone)]
pub struct TauriDefaultRepository {
    id: String,
    url: String,
    kind: String,
    display_name: String,
}

#[tauri::command]
#[specta::specta]
pub async fn environment_default_repositories(
    repository_config: State<'_, RepositoryConfigState>,
) -> Result<Vec<TauriDefaultRepository>, RustError> {
    let repository_config = repository_config.get();
    Ok(vec![
        TauriDefaultRepository {
            id: OFFICIAL_REPOSITORY_ID.to_string(),
            url: OFFICIAL_URL_STR.to_string(),
            kind: "officialDefault".to_string(),
            display_name: repository_config
                .display_names
                .get(OFFICIAL_URL_STR)
                .cloned()
                .unwrap_or_else(|| OFFICIAL_REPOSITORY_ID.to_string()),
        },
        TauriDefaultRepository {
            id: CURATED_REPOSITORY_ID.to_string(),
            url: CURATED_URL_STR.to_string(),
            kind: "curatedDefault".to_string(),
            display_name: repository_config
                .display_names
                .get(CURATED_URL_STR)
                .cloned()
                .unwrap_or_else(|| CURATED_REPOSITORY_ID.to_string()),
        },
    ])
}

#[tauri::command]
#[specta::specta]
pub async fn environment_repository_package_lists(
    app_handle: AppHandle,
    packages: State<'_, PackagesState>,
    settings: State<'_, SettingsState>,
    io: State<'_, DefaultEnvironmentIo>,
    http: State<'_, reqwest::Client>,
) -> Result<Vec<TauriRepositoryPackageList>, RustError> {
    let settings = settings.load(io.inner()).await?;
    let show_prerelease_packages = settings.show_prerelease_packages();
    let packages = packages
        .load(&settings, io.inner(), http.inner(), app_handle)
        .await?;

    Ok(repository_package_lists(
        packages.packages(),
        show_prerelease_packages,
    ))
}

fn repository_package_lists<'package, 'env>(
    packages: impl IntoIterator<Item = &'package PackageInfo<'env>>,
    show_prerelease_packages: bool,
) -> Vec<TauriRepositoryPackageList>
where
    'env: 'package,
{
    let latest_packages = latest_package_infos_by_source(
        packages
            .into_iter()
            .filter(|package| package.repo().is_some())
            .filter(|package| package_is_available_for_display(package, show_prerelease_packages)),
    );

    let mut packages_by_repository = BTreeMap::<String, Vec<TauriBasePackageInfo>>::new();
    for package in latest_packages {
        let Some(repository_id) = package.repo().and_then(cached_repository_id) else {
            continue;
        };
        packages_by_repository
            .entry(repository_id.to_string())
            .or_default()
            .push(TauriBasePackageInfo::new(package.package_json()));
    }

    packages_by_repository
        .into_iter()
        .map(|(repository_id, mut packages)| {
            sort_base_package_infos(&mut packages);
            TauriRepositoryPackageList {
                repository_id,
                packages,
            }
        })
        .collect()
}

fn sort_base_package_infos(packages: &mut [TauriBasePackageInfo]) {
    packages.sort_by(|a, b| {
        let a_name = a.display_name.as_deref().unwrap_or(&a.name);
        let b_name = b.display_name.as_deref().unwrap_or(&b.name);
        a_name
            .cmp(b_name)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.version.major.cmp(&b.version.major))
            .then_with(|| a.version.minor.cmp(&b.version.minor))
            .then_with(|| a.version.patch.cmp(&b.version.patch))
    });
}

#[derive(Serialize, specta::Type)]
struct TauriUserRepository {
    id: String,
    url: String,
    name: String,
    display_name: String,
}

impl From<repository_operations::RepositorySummary> for TauriUserRepository {
    fn from(value: repository_operations::RepositorySummary) -> Self {
        Self {
            id: value.id,
            url: value.url,
            name: value.name,
            display_name: value.display_name,
        }
    }
}

#[derive(Serialize, specta::Type)]
pub struct TauriRepositoriesInfo {
    user_repositories: Vec<TauriUserRepository>,
    hidden_user_repositories: Vec<String>,
    hide_local_user_packages: bool,
    show_prerelease_packages: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn environment_repositories_info(
    settings: State<'_, SettingsState>,
    config: State<'_, GuiConfigState>,
    repository_config: State<'_, RepositoryConfigState>,
    io: State<'_, DefaultEnvironmentIo>,
) -> Result<TauriRepositoriesInfo, RustError> {
    let snapshot = repository_operations::repository_settings_snapshot(
        settings.inner(),
        config.inner(),
        repository_config.inner(),
        io.inner(),
    )
    .await?;

    Ok(TauriRepositoriesInfo {
        user_repositories: snapshot
            .repositories
            .into_iter()
            .filter(|repository| repository.kind == repository_operations::RepositoryKind::User)
            .map(Into::into)
            .collect(),
        hidden_user_repositories: snapshot.hidden_repository_ids,
        hide_local_user_packages: snapshot.hide_local_user_packages,
        show_prerelease_packages: snapshot.show_prerelease_packages,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn environment_hide_repository(
    app: AppHandle,
    config: State<'_, GuiConfigState>,
    repository: String,
) -> Result<(), RustError> {
    let activity = app.state::<ActivityLogState>();
    let target = repository_activity_target(&repository);
    let input = ActivityInput::new(
        ActivitySource::Gui,
        ActivityKind::Write,
        ActivityImportance::Primary,
        operations::REPOSITORY_HIDE,
        "Hiding repository",
    )
    .target(target);
    activity
        .track_result(
            Some(&app),
            input,
            "Repository hidden",
            Vec::new(),
            async move {
                repository_operations::set_repository_hidden(config.inner(), repository, true).await
            },
        )
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn environment_show_repository(
    app: AppHandle,
    config: State<'_, GuiConfigState>,
    repository: String,
) -> Result<(), RustError> {
    let activity = app.state::<ActivityLogState>();
    let target = repository_activity_target(&repository);
    let input = ActivityInput::new(
        ActivitySource::Gui,
        ActivityKind::Write,
        ActivityImportance::Primary,
        operations::REPOSITORY_SHOW,
        "Showing repository",
    )
    .target(target);
    activity
        .track_result(
            Some(&app),
            input,
            "Repository shown",
            Vec::new(),
            async move {
                repository_operations::set_repository_hidden(config.inner(), repository, false)
                    .await
            },
        )
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn environment_set_repository_display_name(
    app: AppHandle,
    repository_config: State<'_, RepositoryConfigState>,
    repository_url: String,
    display_name: String,
) -> Result<(), RustError> {
    let activity = app.state::<ActivityLogState>();
    let input = ActivityInput::new(
        ActivitySource::Gui,
        ActivityKind::Write,
        ActivityImportance::Primary,
        operations::REPOSITORY_DISPLAY_NAME_SET,
        "Updating repository display name",
    )
    .target(summarize_url(&repository_url));
    activity
        .track_result(
            Some(&app),
            input,
            "Repository display name updated",
            Vec::new(),
            async move {
                repository_operations::set_repository_display_name(
                    repository_config.inner(),
                    repository_url,
                    display_name,
                )
                .await
            },
        )
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn environment_set_hide_local_user_packages(
    app: AppHandle,
    config: State<'_, GuiConfigState>,
    value: bool,
) -> Result<(), RustError> {
    let activity = app.state::<ActivityLogState>();
    let input = ActivityInput::new(
        ActivitySource::Gui,
        ActivityKind::Write,
        ActivityImportance::Primary,
        operations::SETTINGS_SET,
        "Updating local user packages visibility",
    )
    .target("hideLocalUserPackages")
    .details(vec![ActivityDetail::new("value", value.to_string())]);
    activity
        .track_result(
            Some(&app),
            input,
            "Local user packages visibility updated",
            Vec::new(),
            async move { user_packages::set_user_packages_hidden(config.inner(), value).await },
        )
        .await
}

#[derive(Serialize, specta::Type, Clone)]
pub struct TauriRemoteRepositoryInfo {
    name: String,
    id: String,
    url: String,
    packages: Vec<TauriBasePackageInfo>,
}

#[derive(Serialize, specta::Type, Clone)]
#[serde(tag = "type")]
pub enum TauriDownloadRepository {
    BadUrl,
    Duplicated {
        reason: TauriDuplicatedReason,
        // Default repository ids use vrc_get_vpm::environment constants.
        duplicated_name: String,
        duplicated_original_name: Option<String>,
    },
    DownloadError {
        message: String,
    },
    Success {
        value: TauriRemoteRepositoryInfo,
    },
}

#[derive(Serialize, specta::Type, Clone)]
pub enum TauriDuplicatedReason {
    URLDuplicated,
    IDDuplicated,
}

impl From<repository_operations::DownloadRepositoryOutcome> for TauriDownloadRepository {
    fn from(value: repository_operations::DownloadRepositoryOutcome) -> Self {
        match value {
            repository_operations::DownloadRepositoryOutcome::Duplicated {
                reason,
                duplicated_name,
                duplicated_original_name,
            } => Self::Duplicated {
                reason: match reason {
                    repository_operations::RepositoryDuplicateReason::Url => {
                        TauriDuplicatedReason::URLDuplicated
                    }
                    repository_operations::RepositoryDuplicateReason::Id => {
                        TauriDuplicatedReason::IDDuplicated
                    }
                },
                duplicated_name,
                duplicated_original_name,
            },
            repository_operations::DownloadRepositoryOutcome::DownloadError(message) => {
                Self::DownloadError { message }
            }
            repository_operations::DownloadRepositoryOutcome::Success(repository) => {
                Self::Success {
                    value: TauriRemoteRepositoryInfo {
                        id: repository.id,
                        url: repository.url,
                        name: repository.name,
                        packages: repository
                            .packages
                            .iter()
                            .map(TauriBasePackageInfo::new)
                            .collect(),
                    },
                }
            }
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn environment_download_repository(
    settings: State<'_, SettingsState>,
    repository_config: State<'_, RepositoryConfigState>,
    io: State<'_, DefaultEnvironmentIo>,
    http: State<'_, reqwest::Client>,
    url: String,
    headers: IndexMap<Box<str>, Box<str>>,
) -> Result<TauriDownloadRepository, RustError> {
    let url: Url = match url.parse() {
        Err(_) => {
            return Ok(TauriDownloadRepository::BadUrl);
        }
        Ok(url) => url,
    };

    {
        let settings = settings.load(io.inner()).await?;
        let display_names = repository_config.get().display_names.clone();
        let identities =
            repository_operations::repository_identity_snapshot(&settings, &display_names);
        repository_operations::download_repository(http.inner(), &url, &headers, &identities)
            .await
            .map(Into::into)
    }
}

#[derive(Serialize, specta::Type)]
pub enum TauriAddRepositoryResult {
    BadUrl,
    Success,
}

#[tauri::command]
#[specta::specta]
pub async fn environment_add_repository(
    app: AppHandle,
    settings: State<'_, SettingsState>,
    packages: State<'_, PackagesState>,
    repository_config: State<'_, RepositoryConfigState>,
    io: State<'_, DefaultEnvironmentIo>,
    http: State<'_, reqwest::Client>,
    url: String,
    headers: IndexMap<Box<str>, Box<str>>,
) -> Result<TauriAddRepositoryResult, RustError> {
    let activity = app.state::<ActivityLogState>();
    let input = ActivityInput::new(
        ActivitySource::Gui,
        ActivityKind::Write,
        ActivityImportance::Primary,
        operations::REPOSITORY_ADD,
        "Adding repository",
    )
    .target(summarize_url_host(&url))
    .details(vec![ActivityDetail::new("url", summarize_url(&url))]);
    let url: Url = match url.parse() {
        Err(_) => {
            activity.record_failed(Some(&app), input, "Bad repository URL");
            return Ok(TauriAddRepositoryResult::BadUrl);
        }
        Ok(url) => url,
    };

    activity
        .track_result(
            Some(&app),
            input,
            "Repository added",
            Vec::new(),
            async move {
                repository_operations::add_repository(
                    settings.inner(),
                    packages.inner(),
                    repository_config.inner(),
                    io.inner(),
                    http.inner(),
                    url,
                    headers,
                )
                .await?;
                Ok(TauriAddRepositoryResult::Success)
            },
        )
        .await
}

fn repository_activity_target(repository_id: &str) -> String {
    Url::parse(repository_id)
        .ok()
        .map(|_| summarize_url(repository_id))
        .unwrap_or_else(|| repository_id.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn environment_remove_repository(
    app: AppHandle,
    settings: State<'_, SettingsState>,
    packages: State<'_, PackagesState>,
    repository_config: State<'_, RepositoryConfigState>,
    io: State<'_, DefaultEnvironmentIo>,
    repository_url: String,
) -> Result<(), RustError> {
    let activity = app.state::<ActivityLogState>();
    let target = repository_activity_target(&repository_url);
    let input = ActivityInput::new(
        ActivitySource::Gui,
        ActivityKind::Write,
        ActivityImportance::Primary,
        operations::REPOSITORY_REMOVE,
        "Removing repository",
    )
    .target(target)
    .details(vec![ActivityDetail::new(
        "repository_url",
        summarize_url(&repository_url),
    )]);
    let repository_url = repository_url
        .parse::<Url>()
        .map_err(|_| RustError::unrecoverable_str("repository_url must be a valid URL"))?;
    activity
        .track_result(
            Some(&app),
            input,
            "Repository removed",
            Vec::new(),
            async move {
                match repository_operations::remove_repository(
                    settings.inner(),
                    packages.inner(),
                    repository_config.inner(),
                    io.inner(),
                    repository_url,
                )
                .await?
                {
                    repository_operations::RemoveRepositoryOutcome::Removed(_) => Ok(()),
                    repository_operations::RemoveRepositoryOutcome::NotFound => {
                        Err(RustError::unrecoverable_str(
                            "repository_url was not found; please refresh",
                        ))
                    }
                }
            },
        )
        .await
}

#[derive(Serialize, specta::Type)]
#[serde(tag = "type")]
pub enum TauriImportRepositoryPickResult {
    NoFilePicked,
    ParsedRepositories {
        repositories: Vec<TauriRepositoryDescriptor>,
        unparsable_lines: Vec<String>,
    },
}

// workaround bug in specta::Type derive macro
type Headers = IndexMap<Box<str>, Box<str>>;

#[derive(Serialize, Deserialize, specta::Type, Clone)]
pub struct TauriRepositoryDescriptor {
    pub url: Url,
    pub headers: Headers,
}

#[tauri::command]
#[specta::specta]
pub async fn environment_reorder_repositories(
    app: AppHandle,
    settings: State<'_, SettingsState>,
    packages: State<'_, PackagesState>,
    io: State<'_, DefaultEnvironmentIo>,
    repository_urls: Vec<String>,
) -> Result<(), RustError> {
    let activity = app.state::<ActivityLogState>();
    let repo_count = repository_urls.len();
    let input = ActivityInput::new(
        ActivitySource::Gui,
        ActivityKind::Write,
        ActivityImportance::Primary,
        operations::REPOSITORY_REORDER,
        "Reordering repositories",
    )
    .details(vec![ActivityDetail::new(
        "repositories",
        repo_count.to_string(),
    )]);
    activity
        .track_result(
            Some(&app),
            input,
            "Repositories reordered",
            Vec::new(),
            async move {
                let repository_urls = repository_urls
                    .into_iter()
                    .map(|repository_url| {
                        repository_url.parse::<Url>().map_err(|_| {
                            RustError::unrecoverable_str(
                                "repository_urls must contain only valid URLs",
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                log::debug!(
                    "reorder remote user repositories: {} entries",
                    repository_urls.len()
                );
                repository_operations::reorder_repositories(
                    settings.inner(),
                    packages.inner(),
                    io.inner(),
                    &repository_urls,
                )
                .await
            },
        )
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn environment_import_repository_pick(
    window: Window,
) -> Result<TauriImportRepositoryPickResult, RustError> {
    let builder = window.dialog().file().set_parent(&window);

    let Some(repositories_path) = builder
        .blocking_pick_file()
        .map(|x| x.into_path_buf())
        .transpose()?
    else {
        return Ok(TauriImportRepositoryPickResult::NoFilePicked);
    };

    let repositories_file = tokio::fs::read_to_string(repositories_path).await?;

    let result = repository_operations::parse_repositories_file(&repositories_file);

    Ok(TauriImportRepositoryPickResult::ParsedRepositories {
        repositories: result
            .repositories
            .into_iter()
            .map(|repository| TauriRepositoryDescriptor {
                url: repository.url,
                headers: repository.headers,
            })
            .collect(),
        unparsable_lines: result.unparsable_lines,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn environment_import_download_repositories(
    window: Window,
    channel: String,
    repositories: Vec<TauriRepositoryDescriptor>,
) -> Result<
    AsyncCallResult<usize, Vec<(TauriRepositoryDescriptor, TauriDownloadRepository)>>,
    RustError,
> {
    async_command(channel, window.clone(), async move {
        With::<usize>::continue_async(|ctx| async move {
            let settings = window.state::<SettingsState>();
            let repository_config = window.state::<RepositoryConfigState>();
            let io = window.state::<DefaultEnvironmentIo>();
            let settings = settings.load(io.inner()).await?;
            {
                let display_names = repository_config.get().display_names.clone();
                let mut identities =
                    repository_operations::repository_identity_snapshot(&settings, &display_names);
                drop(settings);

                info!("downloading {} repositories", repositories.len());

                let counter = AtomicUsize::new(0);

                let counter_ref = &counter;
                let identities_ref = &identities;

                let http = window.state::<reqwest::Client>();
                let mut results = try_join_all(repositories.into_iter().map(|adding_repo| {
                    let ctx = ctx.clone();
                    let http = http.clone();
                    async move {
                        let downloaded = repository_operations::download_repository(
                            http.inner(),
                            &adding_repo.url,
                            &adding_repo.headers,
                            identities_ref,
                        )
                        .await?;

                        info!("downloaded repository: {:?}", adding_repo.url);

                        let count = counter_ref.fetch_add(1, Ordering::Relaxed) + 1;
                        if let Err(e) = ctx.emit(count) {
                            log::error!("failed to emit repository download progress: {e}");
                        }

                        Ok::<_, RustError>((adding_repo, downloaded))
                    }
                }))
                .await?;

                for (_, downloaded) in results.as_mut_slice() {
                    repository_operations::reserve_downloaded_repository(
                        &mut identities,
                        downloaded,
                    );
                }

                Ok(results
                    .into_iter()
                    .map(|(repository, outcome)| (repository, outcome.into()))
                    .collect())
            }
        })
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn environment_import_add_repositories(
    app: AppHandle,
    settings: State<'_, SettingsState>,
    packages: State<'_, PackagesState>,
    repository_config: State<'_, RepositoryConfigState>,
    http: State<'_, reqwest::Client>,
    io: State<'_, DefaultEnvironmentIo>,
    repositories: Vec<TauriRepositoryDescriptor>,
) -> Result<(), RustError> {
    let activity = app.state::<ActivityLogState>();
    let repo_count = repositories.len();
    let input = ActivityInput::new(
        ActivitySource::Gui,
        ActivityKind::Write,
        ActivityImportance::Primary,
        operations::REPOSITORY_IMPORT,
        "Importing repositories",
    )
    .details(vec![ActivityDetail::new(
        "repositories",
        repo_count.to_string(),
    )]);
    activity
        .track_result(
            Some(&app),
            input,
            "Repositories imported",
            Vec::new(),
            async move {
                repository_operations::add_repositories(
                    settings.inner(),
                    packages.inner(),
                    repository_config.inner(),
                    io.inner(),
                    http.inner(),
                    repositories
                        .into_iter()
                        .map(|repository| repository_operations::RepositoryDescriptor {
                            url: repository.url,
                            headers: repository.headers,
                        })
                        .collect(),
                )
                .await
            },
        )
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn environment_export_repositories(
    app: AppHandle,
    settings: State<'_, SettingsState>,
    io: State<'_, DefaultEnvironmentIo>,
    window: Window,
) -> Result<(), RustError> {
    let activity = app.state::<ActivityLogState>();
    let Some(path) = window
        .dialog()
        .file()
        .set_parent(&window)
        .add_filter("Text", &["txt"])
        .set_file_name("repositories.txt")
        .blocking_save_file()
        .map(|x| x.into_path_buf())
        .transpose()?
    else {
        activity.record_info(
            Some(&app),
            ActivityInput::new(
                ActivitySource::Gui,
                ActivityKind::Open,
                ActivityImportance::Secondary,
                operations::REPOSITORY_EXPORT,
                "Repository export cancelled",
            ),
        );
        return Ok(());
    };

    let input = ActivityInput::new(
        ActivitySource::Gui,
        ActivityKind::Write,
        ActivityImportance::Primary,
        operations::REPOSITORY_EXPORT,
        "Exporting repositories",
    )
    .target(target_from_path(&path))
    .details(vec![ActivityDetail::new("path", summarize_path(&path))]);
    activity
        .track_result(
            Some(&app),
            input,
            "Repositories exported",
            Vec::new(),
            async move {
                repository_operations::export_repositories(settings.inner(), io.inner(), &path)
                    .await
            },
        )
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn environment_clear_package_cache(
    app: AppHandle,
    packages: State<'_, PackagesState>,
    io: State<'_, DefaultEnvironmentIo>,
) -> Result<(), RustError> {
    let activity = app.state::<ActivityLogState>();
    activity
        .track_result(
            Some(&app),
            ActivityInput::new(
                ActivitySource::Gui,
                ActivityKind::Maintenance,
                ActivityImportance::Primary,
                operations::REPOSITORY_CLEAR_CACHE,
                "Clearing package cache",
            ),
            "Package cache cleared",
            Vec::new(),
            async move {
                repository_operations::clear_repositories_cache(packages.inner(), io.inner()).await
            },
        )
        .await
}

#[derive(Serialize, specta::Type)]
pub struct TauriUserPackage {
    path: String,
    package: TauriBasePackageInfo,
}

#[tauri::command]
#[specta::specta]
pub async fn environment_get_user_packages(
    settings: State<'_, SettingsState>,
    io: State<'_, DefaultEnvironmentIo>,
) -> Result<Vec<TauriUserPackage>, RustError> {
    Ok(
        user_packages::list_user_packages(settings.inner(), io.inner())
            .await?
            .into_iter()
            .filter_map(|user_package| {
                let path = user_package.path.into_os_string().into_string().ok()?;
                Some(TauriUserPackage {
                    path,
                    package: TauriBasePackageInfo::new(&user_package.package),
                })
            })
            .collect(),
    )
}

#[derive(Serialize, specta::Type)]
pub enum TauriAddUserPackageWithPickerResult {
    NoFolderSelected,
    InvalidSelection,
    AlreadyAdded,
    Successful,
}

#[tauri::command]
#[specta::specta]
pub async fn environment_add_user_package_with_picker(
    app: AppHandle,
    settings: State<'_, SettingsState>,
    packages: State<'_, PackagesState>,
    io: State<'_, DefaultEnvironmentIo>,
    window: Window,
) -> Result<TauriAddUserPackageWithPickerResult, RustError> {
    let activity = app.state::<ActivityLogState>();
    let Some(package_paths) = window
        .dialog()
        .file()
        .set_parent(&window)
        .blocking_pick_folders()
    else {
        activity.record_info(
            Some(&app),
            ActivityInput::new(
                ActivitySource::Gui,
                ActivityKind::Open,
                ActivityImportance::Secondary,
                operations::USER_PACKAGE_ADD,
                "User package selection cancelled",
            ),
        );
        return Ok(TauriAddUserPackageWithPickerResult::NoFolderSelected);
    };

    let Ok(package_paths) = package_paths
        .into_iter()
        .map(|path| path.into_path_buf())
        .collect::<Result<Vec<_>, _>>()
    else {
        activity.record_failed(
            Some(&app),
            ActivityInput::new(
                ActivitySource::Gui,
                ActivityKind::Write,
                ActivityImportance::Primary,
                operations::USER_PACKAGE_ADD,
                "Adding user packages",
            ),
            "Invalid user package selection",
        );
        return Ok(TauriAddUserPackageWithPickerResult::InvalidSelection);
    };

    let package_count = package_paths.len();
    let input = ActivityInput::new(
        ActivitySource::Gui,
        ActivityKind::Write,
        ActivityImportance::Primary,
        operations::USER_PACKAGE_ADD,
        "Adding user packages",
    )
    .details(vec![ActivityDetail::new(
        "packages",
        package_count.to_string(),
    )]);
    let tracker = activity.start_activity(Some(&app), input);
    let result = user_packages::add_user_packages(
        settings.inner(),
        packages.inner(),
        io.inner(),
        &package_paths,
    )
    .await
    .map(|outcome| match outcome {
        user_packages::AddUserPackagesOutcome::Added => {
            TauriAddUserPackageWithPickerResult::Successful
        }
        user_packages::AddUserPackagesOutcome::InvalidSelection => {
            TauriAddUserPackageWithPickerResult::InvalidSelection
        }
        user_packages::AddUserPackagesOutcome::AlreadyAdded => {
            TauriAddUserPackageWithPickerResult::AlreadyAdded
        }
    });
    match &result {
        Ok(TauriAddUserPackageWithPickerResult::Successful) => {
            activity.finish_success(Some(&app), &tracker, "User packages added", Vec::new());
        }
        Ok(TauriAddUserPackageWithPickerResult::InvalidSelection) => {
            activity.finish_failed(
                Some(&app),
                &tracker,
                "User package selection was invalid",
                Vec::new(),
                "selected folder did not contain a valid user package",
            );
        }
        Ok(TauriAddUserPackageWithPickerResult::AlreadyAdded) => {
            activity.finish_failed(
                Some(&app),
                &tracker,
                "User package was already added",
                Vec::new(),
                "selected user package was already added",
            );
        }
        Ok(TauriAddUserPackageWithPickerResult::NoFolderSelected) => {
            activity.finish_cancelled(
                Some(&app),
                &tracker,
                "User package selection cancelled",
                Vec::new(),
            );
        }
        Err(error) => {
            activity.finish_failed(
                Some(&app),
                &tracker,
                "User package add failed",
                Vec::new(),
                error,
            );
        }
    }
    result
}

#[tauri::command]
#[specta::specta]
pub async fn environment_remove_user_packages(
    app: AppHandle,
    settings: State<'_, SettingsState>,
    packages: State<'_, PackagesState>,
    io: State<'_, DefaultEnvironmentIo>,
    path: String,
) -> Result<(), RustError> {
    let activity = app.state::<ActivityLogState>();
    let input = ActivityInput::new(
        ActivitySource::Gui,
        ActivityKind::Write,
        ActivityImportance::Primary,
        operations::USER_PACKAGE_REMOVE,
        "Removing user package",
    )
    .target(target_from_path(&path))
    .details(vec![ActivityDetail::new("path", summarize_path(&path))]);
    activity
        .track_result(
            Some(&app),
            input,
            "User package removed",
            Vec::new(),
            async move {
                if !user_packages::remove_user_package(
                    settings.inner(),
                    packages.inner(),
                    io.inner(),
                    path.as_ref(),
                )
                .await?
                {
                    return Err(RustError::unrecoverable_str(
                        "user package path was not registered; please refresh",
                    ));
                }
                Ok(())
            },
        )
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use vrc_get_vpm::PackageManifest;
    use vrc_get_vpm::repository::{LocalCachedRepository, RemoteRepository};

    #[test]
    fn repository_package_lists_keep_latest_visible_version_per_package() {
        let older = test_package_manifest(json!({
            "name": "com.example.package",
            "displayName": "Example Package",
            "version": "1.0.0",
        }));
        let newer = test_package_manifest(json!({
            "name": "com.example.package",
            "displayName": "Example Package",
            "version": "1.1.0",
        }));
        let repository = test_cached_repository(json!({
            "id": "com.example.repo",
            "url": "https://example.com/index.json",
            "packages": {}
        }));
        let older = PackageInfo::remote(&older, &repository);
        let newer = PackageInfo::remote(&newer, &repository);

        let lists = repository_package_lists([&older, &newer], false);

        assert_eq!(lists.len(), 1);
        assert_eq!(lists[0].repository_id, "com.example.repo");
        assert_eq!(lists[0].packages.len(), 1);
        assert_eq!(lists[0].packages[0].name, "com.example.package");
        assert_eq!(lists[0].packages[0].version.major, 1);
        assert_eq!(lists[0].packages[0].version.minor, 1);
        assert_eq!(lists[0].packages[0].version.patch, 0);
    }

    #[test]
    fn repository_activity_target_sanitizes_url_ids() {
        assert_eq!(
            repository_activity_target("https://user:pass@example.com/index.json?token=secret"),
            "https://example.com/index.json"
        );
        assert_eq!(
            repository_activity_target("com.example.repo"),
            "com.example.repo"
        );
    }

    fn test_package_manifest(value: Value) -> PackageManifest {
        serde_json::from_value(value).unwrap()
    }

    fn test_cached_repository(value: Value) -> LocalCachedRepository {
        let Value::Object(repository) = value else {
            panic!("expected repository object");
        };
        LocalCachedRepository::new(
            RemoteRepository::parse(repository).unwrap(),
            IndexMap::new(),
        )
    }
}
