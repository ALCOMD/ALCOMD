//! Maps selected fields from the published 3.4.0 profile to native staging values.
use std::collections::{BTreeMap, BTreeSet};
use std::io::{ErrorKind, Read};
use std::path::{Component, Path, PathBuf};

use alcomd_import::{
    DiscordPreference, ExtensionPreference, NativeState, Project, Repository, RepositorySource,
    Settings, UnityInstallation,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use url::Url;

use crate::{SOURCE_VERSION, litedb, templates};

const JSON_BYTES: u64 = 8 * 1024 * 1024;
const DATABASE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_PATH_BYTES: usize = 4096;
const MAX_RECORDS: usize = 16_384;

pub struct Profile {
    pub native: NativeState,
    pub credentials: Vec<Option<Headers>>,
    pub template_files: Vec<templates::Archive>,
    pub skipped_templates: usize,
}

#[derive(Serialize)]
pub struct Headers {
    headers: BTreeMap<String, String>,
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Vpm {
    user_projects: Vec<String>,
    unity_editors: Vec<String>,
    user_package_folders: Vec<String>,
    user_repos: Vec<RemoteSetting>,
    path_to_unity_hub: Option<String>,
    default_project_path: Option<String>,
    project_backup_path: Option<String>,
    show_prerelease_packages: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RemoteSetting {
    local_path: String,
    url: Option<String>,
    id: Option<String>,
    name: Option<String>,
    #[serde(default)]
    headers: BTreeMap<String, String>,
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Gui {
    gui_hidden_repositories: BTreeSet<String>,
    hide_local_user_packages: bool,
    fullscreen: bool,
    language: Option<String>,
    backup_format: Option<String>,
    use_alcom_for_vcc_protocol: Option<bool>,
    default_unity_arguments: Option<Vec<String>>,
    gui_animation: Option<bool>,
    unity_discord_sharing_enabled: Option<bool>,
    unity_discord_display_options: Option<V3Discord>,
    favorite_templates: Vec<String>,
    last_used_template: Option<String>,
    sidebar_extensions: Vec<SidebarPreference>,
    extensions: BTreeMap<String, ExtensionState>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SidebarPreference {
    id: String,
    enabled: Option<bool>,
    visible: Option<bool>,
}

#[derive(Deserialize)]
struct ExtensionState {
    enabled: Option<bool>,
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct V3Discord {
    project_name: Option<bool>,
    unity_version: Option<bool>,
    editor_count: Option<bool>,
    custom_text: String,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Theme {
    theme: Option<String>,
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct RepositoryPreferences {
    display_names: BTreeMap<String, String>,
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct BuiltinRepositoryPreferences {
    ignore_official_repository: bool,
    ignore_curated_repository: bool,
}

pub fn read(root: &Path) -> Result<Profile> {
    let (root, root_id) =
        alcomd_platform::resolve_plain_storage_directory(root).context("invalid source profile")?;
    let settings: Option<Vpm> = json(&root, "settings.json")?;
    let database = read_file(&root, "vcc.liteDb", DATABASE_BYTES)?;
    ensure!(
        settings.is_some() || database.is_some(),
        "no published profile records found"
    );
    let wal = read_file(&root, "vcc-log.liteDb", 0)?;
    ensure!(
        wal.is_none_or(|data| data.is_empty()),
        "uncheckpointed database source"
    );
    let gui: Gui = match json(&root, "config/gui-config.json")? {
        Some(gui) => gui,
        None => json(&root, "vrc-get/gui-config.json")?.unwrap_or_default(),
    };
    let theme: Theme = json(&root, "config/theme-config.json")?.unwrap_or_default();
    let repo_settings: RepositoryPreferences =
        json(&root, "config/repositories.json")?.unwrap_or_default();
    let builtin: BuiltinRepositoryPreferences =
        json(&root, "config/repository-settings.json")?.unwrap_or_default();
    let settings = settings.unwrap_or_default();
    let collections = database
        .as_deref()
        .map(litedb::read)
        .transpose()?
        .unwrap_or_default();

    let projects = projects(collections.projects, settings.user_projects)?;
    let unity_installations = unity_installations(collections.unity, settings.unity_editors)?;
    let (repositories, credentials) = repositories(
        settings.user_repos,
        &gui.gui_hidden_repositories,
        repo_settings.display_names,
    )?;
    let inventory = templates::inventory(&root)?;
    let extensions = extensions(&gui);
    let discord_extension_enabled = extensions
        .iter()
        .find(|item| item.id == "discord")
        .is_none_or(|item| item.enabled);
    let options = gui.unity_discord_display_options.unwrap_or_default();
    ensure!(
        options.custom_text.len() <= 1024 && options.custom_text.chars().all(|c| !c.is_control()),
        "invalid Discord preference"
    );
    let discord = DiscordPreference {
        enabled: discord_extension_enabled && gui.unity_discord_sharing_enabled.unwrap_or(false),
        project_name: options.project_name.unwrap_or(true),
        unity_version: options.unity_version.unwrap_or(true),
        editor_count: options.editor_count.unwrap_or(true),
        custom_text: options.custom_text,
    };
    let local_packages = unique_paths(settings.user_package_folders)?;
    let settings = Settings {
        locale: locale(gui.language.as_deref())?.into(),
        appearance_mode: appearance(theme.theme.as_deref())?.into(),
        appearance_motion: if gui.gui_animation.unwrap_or(true) {
            "full"
        } else {
            "reduced"
        }
        .into(),
        fullscreen: gui.fullscreen,
        handle_vcc_links: gui.use_alcom_for_vcc_protocol.unwrap_or(true),
        show_prerelease: settings.show_prerelease_packages,
        hide_local_user_packages: gui.hide_local_user_packages,
        hide_official_repository: builtin.ignore_official_repository,
        hide_curated_repository: builtin.ignore_curated_repository,
        project_directory: existing_storage_path(settings.default_project_path)?,
        backup_directory: existing_storage_path(settings.project_backup_path)?,
        hub_path: optional_path(settings.path_to_unity_hub)?,
        default_unity_launch_arguments: arguments(gui.default_unity_arguments)?,
        backup_compression: compression(gui.backup_format.as_deref()).into(),
        favorite_template_ids: gui.favorite_templates,
        last_selected_template_id: gui.last_used_template,
    };
    let (_, current_root) = alcomd_platform::resolve_plain_storage_directory(&root)
        .context("source profile became unavailable")?;
    ensure!(
        current_root == root_id,
        "source profile changed during read"
    );
    Ok(Profile {
        native: NativeState {
            schema: 1,
            source_version: SOURCE_VERSION.into(),
            projects,
            unity_installations,
            repositories,
            local_packages,
            templates: inventory.templates,
            settings,
            extensions,
            discord,
        },
        credentials,
        template_files: inventory.attachments,
        skipped_templates: inventory.skipped_derived,
    })
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ProjectSubset {
    user_projects: Option<Vec<String>>,
    unity_editors: Option<Vec<String>>,
}

pub(super) fn selected_projects(root: &Path) -> Result<(Vec<Project>, Vec<UnityInstallation>)> {
    let settings = json::<ProjectSubset>(root, "settings.json")?.unwrap_or_default();
    let database = read_file(root, "vcc.liteDb", DATABASE_BYTES)?;
    ensure!(
        database.is_some() || settings.user_projects.is_some() || settings.unity_editors.is_some(),
        "no recognized project or Unity data in selected profile",
    );
    let _ = read_file(root, "vcc-log.liteDb", 0)?;
    let records = database
        .as_deref()
        .map(litedb::read)
        .transpose()?
        .unwrap_or_default();
    Ok((
        projects(records.projects, settings.user_projects.unwrap_or_default())?,
        unity_installations(records.unity, settings.unity_editors.unwrap_or_default())?,
    ))
}

pub(super) fn selected_theme(root: &Path, beta: bool) -> Result<&'static str> {
    if beta
        && let Some(theme) =
            json::<Theme>(root, "vrc-get/theme-config.json")?.and_then(|theme| theme.theme)
    {
        return appearance(Some(&theme));
    }
    let theme = json::<Theme>(root, "vrc-get/gui-config.json")?
        .and_then(|theme| theme.theme)
        .context("no recognized legacy theme choice")?;
    appearance(Some(&theme))
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct ResourceSubset {
    user_repos: Option<Vec<RemoteSetting>>,
    user_package_folders: Option<Vec<String>>,
}

pub(super) fn selected_resources(root: &Path) -> Result<(Vec<Repository>, Vec<String>)> {
    let data =
        json::<ResourceSubset>(root, "settings.json")?.context("no source resource settings")?;
    ensure!(
        data.user_repos.is_some() || data.user_package_folders.is_some(),
        "no recognized resource selection"
    );
    let (repositories, credentials) = repositories(
        data.user_repos.unwrap_or_default(),
        &BTreeSet::new(),
        BTreeMap::new(),
    )?;
    ensure!(
        credentials.iter().all(Option::is_none),
        "selected repositories require an explicit credential transfer"
    );
    Ok((
        repositories,
        unique_paths(data.user_package_folders.unwrap_or_default())?,
    ))
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct SettingsSubset {
    language: Option<String>,
    fullscreen: Option<bool>,
    use_alcom_for_vcc_protocol: Option<bool>,
    gui_animation: Option<bool>,
    backup_format: Option<String>,
    default_unity_arguments: Option<Vec<String>>,
}

pub(super) fn selected_settings(
    root: &Path,
    beta: bool,
) -> Result<alcomd_protocol::SettingsUpdate> {
    use alcomd_protocol::{
        AppearanceMotion, AppearanceSettingsUpdate, BackupSettingsUpdate, IntegrationSettings,
        NullableUpdate, SettingsUpdate, WindowSettingsUpdate,
    };

    let gui = json::<SettingsSubset>(root, "vrc-get/gui-config.json")?.unwrap_or_default();
    let arguments = arguments(gui.default_unity_arguments)?;
    let mut patch = SettingsUpdate {
        locale: gui
            .language
            .as_deref()
            .map(|value| locale(Some(value)))
            .transpose()?
            .map(|value| serde_json::from_value(serde_json::json!(value)))
            .transpose()?,
        window: gui.fullscreen.map(|fullscreen| WindowSettingsUpdate {
            fullscreen: Some(fullscreen),
            ..Default::default()
        }),
        integrations: gui
            .use_alcom_for_vcc_protocol
            .map(|handle_vcc_links| IntegrationSettings { handle_vcc_links }),
        appearance: gui.gui_animation.map(|enabled| AppearanceSettingsUpdate {
            motion: Some(if enabled {
                AppearanceMotion::Full
            } else {
                AppearanceMotion::Reduced
            }),
            ..Default::default()
        }),
        unity: arguments.map(|arguments| alcomd_protocol::UnitySettingsUpdate {
            default_launch_arguments: NullableUpdate::Set(arguments),
            ..Default::default()
        }),
        ..Default::default()
    };
    if let Some(format) = gui.backup_format.as_deref() {
        patch.backups = Some(BackupSettingsUpdate {
            compression_mode: Some(serde_json::from_value(serde_json::json!(compression(
                Some(format)
            )))?),
            ..Default::default()
        });
    }
    if !beta {
        #[derive(Default, Deserialize)]
        #[serde(default, rename_all = "camelCase")]
        struct PackageSettings {
            show_prerelease_packages: Option<bool>,
            default_project_path: Option<String>,
            project_backup_path: Option<String>,
        }
        let source = json::<PackageSettings>(root, "settings.json")?.unwrap_or_default();
        patch.packages = source.show_prerelease_packages.map(|show_prerelease| {
            alcomd_protocol::PackageSettingsUpdate {
                show_prerelease: Some(show_prerelease),
                ..Default::default()
            }
        });
        let project = existing_storage_path(source.default_project_path)?;
        let backup = existing_storage_path(source.project_backup_path)?;
        if project.is_some() || backup.is_some() {
            patch.paths = Some(alcomd_protocol::PathSettingsUpdate {
                project_directory: project.map_or(NullableUpdate::Unchanged, NullableUpdate::Set),
                backup_directory: backup.map_or(NullableUpdate::Unchanged, NullableUpdate::Set),
            });
        }
    }
    ensure!(
        patch != SettingsUpdate::default(),
        "no recognized settings in selected profile"
    );
    Ok(patch)
}

fn json<T: DeserializeOwned>(root: &Path, relative: &str) -> Result<Option<T>> {
    let Some(bytes) = read_file(root, relative, JSON_BYTES)? else {
        return Ok(None);
    };
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
    let value = serde_json::from_slice(bytes).map_err(|e| {
        anyhow::anyhow!(
            "malformed profile JSON at line {} column {}",
            e.line(),
            e.column()
        )
    })?;
    Ok(Some(value))
}

pub fn read_file(root: &Path, relative: impl AsRef<Path>, limit: u64) -> Result<Option<Vec<u8>>> {
    let relative = relative.as_ref();
    ensure!(
        relative
            .components()
            .all(|part| matches!(part, Component::Normal(_))),
        "invalid profile item name"
    );
    let path = root.join(relative);
    let parent = path.parent().context("missing profile item directory")?;
    let (_, parent_id) = match alcomd_platform::resolve_plain_storage_directory(parent) {
        Ok(value) => value,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(_) => anyhow::bail!("unsafe profile item path"),
    };
    let (mut file, identity) = match alcomd_platform::open_plain_file(&path) {
        Ok(value) => value,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(_) => anyhow::bail!("unreadable profile item"),
    };
    let stamp = file.metadata()?;
    ensure!(stamp.len() <= limit, "profile item exceeds its byte limit");
    let mut contents = Vec::new();
    (&mut file).take(limit + 1).read_to_end(&mut contents)?;
    ensure!(
        contents.len() as u64 == stamp.len(),
        "profile item changed during read"
    );
    let (current, current_id) =
        alcomd_platform::open_plain_file(&path).context("profile item was replaced")?;
    let (_, current_parent) = alcomd_platform::resolve_plain_storage_directory(parent)
        .context("profile item parent was replaced")?;
    ensure!(
        identity == current_id
            && parent_id == current_parent
            && stamp.len() == current.metadata()?.len()
            && stamp.modified()? == current.metadata()?.modified()?,
        "profile item changed during read"
    );
    Ok(Some(contents))
}

fn projects(records: Vec<litedb::Document>, paths: Vec<String>) -> Result<Vec<Project>> {
    let mut projects = BTreeMap::new();
    for document in records {
        let path =
            checked_path(string(&document, "Path")?.context("project record lacks a path")?)?;
        let unity_version = string(&document, "UnityVersion")?;
        let metadata = match document.get("vrc-get") {
            None | Some(Value::Null) => None,
            Some(value) => Some(value.as_object().context("invalid project details")?),
        };
        let mut unity_revision = None;
        let mut unity_executable_path = None;
        let mut launch = None;
        if let Some(details) = metadata {
            if string(details, "cached_unity_version")? == unity_version {
                unity_revision = string(details, "unity_revision")?;
            }
            unity_executable_path = optional_path(string(details, "unity_path")?)?;
            let args = match details.get("custom_unity_args") {
                None | Some(Value::Null) => None,
                Some(value) => Some(
                    serde_json::from_value(value.clone())
                        .context("invalid project launch arguments")?,
                ),
            };
            launch = arguments(args)?;
        }
        let project = Project {
            path: path.clone(),
            favorite: boolean(&document, "Favorite")?.unwrap_or(false),
            registered_at_ms: timestamp(&document, "CreatedAt")?,
            last_modified_at_ms: timestamp(&document, "LastModified")?,
            unity_version,
            unity_revision,
            unity_executable_path,
            unity_launch_arguments: launch,
        };
        ensure!(
            projects.insert(path, project).is_none(),
            "duplicate project database path"
        );
    }
    for path in unique_paths(paths)? {
        projects.entry(path.clone()).or_insert(Project {
            path,
            favorite: false,
            registered_at_ms: None,
            last_modified_at_ms: None,
            unity_version: None,
            unity_revision: None,
            unity_executable_path: None,
            unity_launch_arguments: None,
        });
    }
    ensure!(
        projects.len() <= MAX_RECORDS,
        "too many unique project paths"
    );
    Ok(projects.into_values().collect())
}

fn unity_installations(
    records: Vec<litedb::Document>,
    paths: Vec<String>,
) -> Result<Vec<UnityInstallation>> {
    let mut editors = BTreeMap::new();
    for document in records {
        let executable_path =
            checked_path(string(&document, "Path")?.context("Unity registration lacks a path")?)?;
        let installation = UnityInstallation {
            executable_path: executable_path.clone(),
            version: string(&document, "Version")?,
            from_hub: boolean(&document, "LoadedFromHub")?.unwrap_or(false),
        };
        ensure!(
            editors.insert(executable_path, installation).is_none(),
            "duplicate Unity registration"
        );
    }
    for executable_path in unique_paths(paths)? {
        editors
            .entry(executable_path.clone())
            .or_insert(UnityInstallation {
                executable_path,
                version: None,
                from_hub: false,
            });
    }
    ensure!(editors.len() <= MAX_RECORDS, "too many Unity registrations");
    Ok(editors.into_values().collect())
}

fn repositories(
    records: Vec<RemoteSetting>,
    hidden: &BTreeSet<String>,
    display_names: BTreeMap<String, String>,
) -> Result<(Vec<Repository>, Vec<Option<Headers>>)> {
    ensure!(records.len() <= MAX_RECORDS, "too many repository records");
    let mut repos = Vec::new();
    let mut credentials = Vec::new();
    let mut locations = BTreeSet::new();
    for record in records {
        let mut display_name = record.name;
        let mut is_hidden = false;
        if let Some(id) = &record.id {
            if let Some(value) = display_names.get(id) {
                display_name = Some(value.clone());
            }
            is_hidden = hidden.contains(id);
        }
        let auth = (!record.headers.is_empty()).then_some(Headers {
            headers: record.headers,
        });
        for text in [record.id.as_deref(), display_name.as_deref()]
            .into_iter()
            .flatten()
        {
            ensure!(
                !auth.as_ref().is_some_and(|auth| has_credential(text, auth)),
                "repository metadata contains credential material"
            );
        }
        let (location, source) = if let Some(url) = record.url {
            let url = normalized_url(&url, auth.as_ref())?;
            (url.clone(), RepositorySource::Remote { url })
        } else {
            ensure!(auth.is_none(), "local repository claims remote credentials");
            let path = checked_path(record.local_path)?;
            (path.clone(), RepositorySource::Local { path })
        };
        ensure!(locations.insert(location), "duplicate repository location");
        repos.push(Repository {
            source,
            declared_id: record.id,
            display_name,
            hidden: is_hidden,
            has_credentials: auth.is_some(),
            credential_ref: None,
        });
        credentials.push(auth);
    }
    Ok((repos, credentials))
}

pub fn credential_bytes(headers: &Headers) -> Result<Vec<u8>> {
    const RESERVED: &[&str] = &[
        "host",
        "connection",
        "content-length",
        "transfer-encoding",
        "proxy-authorization",
        "proxy-connection",
        "upgrade",
        "te",
        "trailer",
        "if-none-match",
        "if-modified-since",
    ];
    let mut names = BTreeSet::new();
    ensure!(
        (1..=16).contains(&headers.headers.len()),
        "credential has unsupported header count"
    );
    for (name, value) in &headers.headers {
        let normalized = name.to_ascii_lowercase();
        ensure!(
            !name.is_empty()
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
                && names.insert(normalized.clone())
                && !RESERVED.contains(&normalized.as_str()),
            "credential has unsupported header name"
        );
        ensure!(
            !value.trim().is_empty() && value.bytes().all(|b| b == b'\t' || (32..127).contains(&b)),
            "credential has unsupported header value"
        );
    }
    let bytes = serde_json::to_vec(headers).context("could not serialize credential")?;
    ensure!(
        bytes.len() <= 2560,
        "credential exceeds the current vault byte limit"
    );
    Ok(bytes)
}

fn normalized_url(raw: &str, auth: Option<&Headers>) -> Result<String> {
    ensure!(
        raw.len() <= 8192
            && raw.trim() == raw
            && !raw.contains('\\')
            && !raw.chars().any(char::is_control),
        "invalid repository URL"
    );
    let url = Url::parse(raw).map_err(|_| anyhow::anyhow!("invalid repository URL"))?;
    ensure!(
        matches!(url.scheme(), "http" | "https")
            && url.has_host()
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none(),
        "unsupported repository URL"
    );
    if let Some(auth) = auth {
        ensure!(
            !has_credential(raw, auth)
                && !has_credential(url.as_str(), auth)
                && !url.query_pairs().any(
                    |(name, value)| has_credential(&name, auth) || has_credential(&value, auth)
                ),
            "repository URL contains credential material"
        );
    }
    Ok(url.to_string())
}

fn has_credential(text: &str, auth: &Headers) -> bool {
    auth.headers.values().any(|header| {
        let trimmed = header.trim();
        let matches = |secret: &str| {
            !secret.is_empty() && (text == secret || (secret.len() >= 8 && text.contains(secret)))
        };
        matches(trimmed)
            || trimmed
                .split_once(' ')
                .is_some_and(|(_, token)| matches(token))
    })
}

fn extensions(gui: &Gui) -> Vec<ExtensionPreference> {
    let mut found = BTreeMap::new();
    for item in &gui.sidebar_extensions {
        let id = match item.id.as_str() {
            "unity-discord-status" => "discord",
            id => id,
        };
        if matches!(id, "discord" | "mcp") {
            found.insert(
                id,
                (item.enabled.unwrap_or(true), item.visible.unwrap_or(true)),
            );
        }
    }
    for id in ["discord", "mcp"] {
        let discord = id == "discord";
        let stored = gui.extensions.get(id).or_else(|| {
            if discord {
                gui.extensions.get("unity-discord-status")
            } else {
                None
            }
        });
        let stored_policy = discord
            && (gui.unity_discord_sharing_enabled.is_some()
                || gui.unity_discord_display_options.is_some());
        if stored.is_some() || stored_policy {
            let preference = found.entry(id).or_insert((true, true));
            if let Some(enabled) = stored.and_then(|config| config.enabled) {
                preference.0 = enabled;
            }
        }
    }
    found
        .into_iter()
        .map(|(id, (enabled, visible))| ExtensionPreference {
            id: id.into(),
            enabled,
            visible,
        })
        .collect()
}

fn locale(value: Option<&str>) -> Result<&'static str> {
    Ok(match value {
        None | Some("" | "en" | "en-US") => "en-US",
        Some("zh" | "zh-CN" | "zh-Hans" | "zh_hans") => "zh-CN",
        Some("ja" | "ja-JP") => "ja-JP",
        Some("system") => "system",
        Some(_) => {
            anyhow::bail!("profile locale is not supported by the current settings contract")
        }
    })
}

fn appearance(value: Option<&str>) -> Result<&'static str> {
    Ok(match value {
        None | Some("" | "system") => "system",
        Some("light") => "light",
        Some("dark") => "dark",
        Some(_) => anyhow::bail!("unknown appearance mode"),
    })
}

fn compression(value: Option<&str>) -> &'static str {
    match value.unwrap_or_default().to_ascii_lowercase().as_str() {
        "zip-store" => "store",
        "zip-best" => "maximum",
        // ALCOMD3 3.4.0 writes ZIP with fast deflate for absent or unknown preferences.
        _ => "fast",
    }
}

pub(super) fn checked_path(value: String) -> Result<String> {
    let path = PathBuf::from(&value);
    ensure!(
        path.is_absolute()
            && value.len() <= MAX_PATH_BYTES
            && !value.chars().any(char::is_control)
            && !path
                .components()
                .any(|part| matches!(part, Component::ParentDir | Component::CurDir)),
        "invalid absolute profile path"
    );
    Ok(value)
}

fn unique_paths(paths: Vec<String>) -> Result<Vec<String>> {
    ensure!(paths.len() <= MAX_RECORDS, "too many registered paths");
    Ok(paths
        .into_iter()
        .map(checked_path)
        .collect::<Result<BTreeSet<_>>>()?
        .into_iter()
        .collect())
}

fn optional_path(path: Option<String>) -> Result<Option<String>> {
    path.filter(|text| !text.is_empty())
        .map(checked_path)
        .transpose()
}

fn existing_storage_path(path: Option<String>) -> Result<Option<String>> {
    let Some(path) = optional_path(path)? else {
        return Ok(None);
    };
    match std::fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.is_dir() => Ok(Some(path)),
        Ok(_) => anyhow::bail!("saved storage path is not a directory"),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).context("could not inspect saved storage directory"),
    }
}

fn arguments(values: Option<Vec<String>>) -> Result<Option<Vec<String>>> {
    if let Some(values) = &values {
        ensure!(
            values.len() <= 64
                && values
                    .iter()
                    .all(|item| item.len() <= 4096 && !item.as_bytes().contains(&0)),
            "invalid saved Unity arguments"
        );
    }
    Ok(values)
}

fn string(document: &litedb::Document, field: &str) -> Result<Option<String>> {
    match document.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .map(|text| Some(text.to_owned()))
            .context("invalid database text field"),
    }
}

fn boolean(document: &litedb::Document, field: &str) -> Result<Option<bool>> {
    match document.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_bool()
            .map(Some)
            .context("invalid database boolean field"),
    }
}

fn timestamp(document: &litedb::Document, field: &str) -> Result<Option<u64>> {
    match document.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .context("invalid database clock field"),
    }
}
