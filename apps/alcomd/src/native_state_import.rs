use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use alcomd_application as app;
use alcomd_application::{
    ConfigStore, M3ReadAdapter, M3RegistryStore, M5UnityPlatform, M5UnityStore, ProjectCreateStore,
    StateStore, UserPackageStore,
};
use alcomd_import::{NativeState, RepositorySource, TemplateSource};
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::native_discord_import::{self, DiscordImportOptions};

const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_STATE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_ATTACHMENT_BYTES: u64 = 256 * 1024 * 1024;
const MAX_BUNDLE_BYTES: u64 = 1024 * 1024 * 1024;

#[cfg(test)]
#[path = "native_state_import_tests.rs"]
mod tests;

macro_rules! collect_pages {
    ($owner:expr, $store:ident.$method:ident, $field:ident) => {{
        let mut records = Vec::new();
        let mut cursor = None;
        loop {
            let page = $store
                .$method($owner.clone(), cursor, 100)
                .await
                .map_err(|_| anyhow::anyhow!("native reconciliation read failed"))?;
            records.extend(page.$field);
            ensure!(
                records.len() <= 10_000,
                "native reconciliation count exceeds its limit"
            );
            cursor = page.next_cursor;
            if cursor.is_none() {
                break records;
            }
        }
    }};
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    schema: u32,
    source_version: String,
    created_at: String,
    records: BTreeMap<String, usize>,
    files: Vec<BundleFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BundleFile {
    path: String,
    size: u64,
    sha256: String,
}

fn bounded_file(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let (file, _) = alcomd_platform::open_plain_file(path).context("bundle file unavailable")?;
    let size = file.metadata()?.len();
    ensure!(size <= limit, "bundle file exceeds its size limit");
    let mut bytes = Vec::with_capacity(size.try_into()?);
    file.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 == size,
        "bundle file changed during the read"
    );
    Ok(bytes)
}

fn attachment_path(root: &Path, relative: &str) -> Result<PathBuf> {
    ensure!(
        relative.len() <= 512
            && !relative.contains(['\\', ':'])
            && relative
                .split('/')
                .all(|part| !matches!(part, "" | "." | ".."))
            && !relative.chars().any(char::is_control),
        "invalid attachment name"
    );
    let path = Path::new(relative);
    ensure!(
        path.components()
            .all(|part| matches!(part, Component::Normal(_)))
            && path
                .extension()
                .is_some_and(|extension| extension == "alcomdtemplate"),
        "invalid attachment path"
    );
    let path = root.join(path);
    alcomd_platform::resolve_plain_storage_directory(path.parent().context("attachment parent")?)?;
    Ok(path)
}

fn validate_bundle(root: &Path) -> Result<(NativeState, BTreeSet<String>)> {
    let (_, identity) = alcomd_platform::resolve_plain_storage_directory(root)?;
    let manifest: Manifest = serde_json::from_slice(&bounded_file(
        &root.join("manifest.json"),
        MAX_MANIFEST_BYTES,
    )?)
    .map_err(|_| anyhow::anyhow!("invalid native bundle manifest"))?;
    ensure!(
        manifest.schema == 1
            && !manifest.source_version.is_empty()
            && manifest.created_at.len() <= 40,
        "unsupported native bundle"
    );
    ensure!(
        (1..=129).contains(&manifest.files.len()),
        "invalid bundle file count"
    );
    let mut names = BTreeSet::new();
    let mut total = 0_u64;
    let mut state = None;
    for entry in manifest.files {
        ensure!(names.insert(entry.path.clone()), "duplicate bundle file");
        total = total
            .checked_add(entry.size)
            .context("bundle size overflow")?;
        ensure!(total <= MAX_BUNDLE_BYTES, "bundle exceeds its size limit");
        let (path, limit) = if entry.path == "state.json" {
            (root.join("state.json"), MAX_STATE_BYTES)
        } else {
            (attachment_path(root, &entry.path)?, MAX_ATTACHMENT_BYTES)
        };
        let bytes = bounded_file(&path, limit)?;
        ensure!(
            bytes.len() as u64 == entry.size && digest_hex(&bytes) == entry.sha256,
            "bundle file did not match its digest"
        );
        if entry.path == "state.json" {
            state = Some(
                serde_json::from_slice::<NativeState>(&bytes)
                    .map_err(|_| anyhow::anyhow!("invalid native state"))?,
            );
        }
    }
    let mut state = state.context("native state is missing")?;
    ensure!(
        state.schema == manifest.schema && state.source_version == manifest.source_version,
        "state and bundle versions differ"
    );
    let counts = state.counts();
    ensure!(
        counts.len() == manifest.records.len()
            && counts
                .iter()
                .all(|(name, count)| manifest.records.get(*name) == Some(count)),
        "native state counts do not match its manifest"
    );
    names.remove("state.json");
    for template in &state.templates {
        if let TemplateSource::ProjectArchive { archive_path, .. } = &template.source {
            ensure!(
                names.contains(archive_path),
                "template attachment absent from verified manifest"
            );
        }
    }
    let (_, final_identity) = alcomd_platform::resolve_plain_storage_directory(root)?;
    ensure!(
        identity == final_identity,
        "bundle directory changed during verification"
    );
    state.templates =
        alcomd_import::order_template_bases(state.templates).map_err(anyhow::Error::msg)?;
    Ok((state, names))
}

/// Import a validated native bundle into an exclusively created staging directory.
/// The bridge manages publication, retry, and disposal of uncommitted stages.
pub async fn import_native_state(
    bundle: &Path,
    destination: &Path,
    discord: Option<DiscordImportOptions<'_>>,
) -> Result<serde_json::Value> {
    let (state, _) = validate_bundle(bundle)?;
    let parent = destination
        .parent()
        .context("missing native staging parent")?;
    alcomd_platform::resolve_plain_storage_directory(parent)?;
    ensure!(
        destination.is_absolute() && destination.file_name().is_some(),
        "native stage must be a new absolute directory"
    );
    let explicit_discord = explicit_discord(&state)?;
    let counts = state.counts();
    std::fs::create_dir(destination).context("native stage must not already exist")?;
    let (destination, _) = alcomd_platform::resolve_plain_storage_directory(destination)?;
    let data = alcomd_platform::DataConfig::isolated(destination.clone());
    let store = alcomd_store::StateStoreHandle::open(alcomd_platform::state_database_path(&data)?)?;
    let settings = app::M7OfficialApplication::new(
        Arc::new(alcomd_store::FileConfigStore::new(
            destination.join("config/settings.toml"),
        )),
        Arc::new(super::hub_platform::PlatformUnityHubAdapter),
        Arc::new(super::hub_platform::PlatformUnityHubLauncher::default()),
        Arc::new(super::storage_platform::PlatformStorageDirectories),
    );
    let access = app::AccessContext::local_owner();
    settings
        .initialize_settings()
        .await
        .map_err(|_| anyhow::anyhow!("native settings initialization failed"))?;
    let reader = alcomd_vpm::VpmReader::with_settings(settings.clone())?;
    let registry = app::M3Application::new(store.clone(), reader.clone());
    let platform = super::m5_platform::PlatformUnityAdapter;
    let unity = app::M5UnityApplication::new(store.clone(), platform, Arc::new(settings.clone()));
    let now: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis()
        .try_into()?;

    for (index, project) in state.projects.into_iter().enumerate() {
        let observation = reader
            .inspect_project(project.path, app::ProjectDiscoveryMode::ExactRoot)
            .await
            .map_err(|_| anyhow::anyhow!("project could not be verified"))?;
        let registered = store
            .register_project(
                access.principal().clone(),
                observation,
                key("project", index)?,
                project.registered_at_ms.unwrap_or(now),
            )
            .await
            .map_err(|_| anyhow::anyhow!("project could not be registered"))?
            .value;
        if project.favorite {
            registry
                .set_project_favorite(
                    &access,
                    registered.project_id,
                    true,
                    registered.revision,
                    key("favorite", index)?,
                )
                .await
                .map_err(|_| anyhow::anyhow!("project favorite could not be saved"))?;
        }
        if let Some(args) = project.unity_launch_arguments {
            unity
                .set_project_launch_config(
                    &access,
                    registered.project_id,
                    args,
                    None,
                    key("unity-arguments", index)?,
                )
                .await
                .map_err(|_| anyhow::anyhow!("project launch arguments could not be saved"))?;
        }
    }
    for (index, installation) in state.unity_installations.into_iter().enumerate() {
        let source = if installation.from_hub {
            app::UnitySourceKind::HubConfig
        } else {
            app::UnitySourceKind::Manual
        };
        let observation = platform
            .validate_installation(installation.executable_path, source)
            .await
            .map_err(|_| anyhow::anyhow!("Unity installation could not be verified"))?;
        store
            .register_installation(
                access.principal().clone(),
                observation,
                key("unity-installation", index)?,
                now,
            )
            .await
            .map_err(|_| anyhow::anyhow!("Unity installation could not be registered"))?;
    }
    let mut hidden = Vec::new();
    for (index, repo) in state.repositories.into_iter().enumerate() {
        let credential_reference = match (repo.has_credentials, repo.credential_ref) {
            (false, None) => None,
            (true, Some(reference)) => Some({
                let data = alcomd_platform::read_credential(&reference)
                    .map_err(|_| anyhow::anyhow!("repository credential transfer unavailable"))?;
                let credentials = serde_json::from_slice::<app::RepositoryCredentials>(&data)
                    .map_err(|_| anyhow::anyhow!("repository credentials are not valid"))?;
                ensure!(
                    !credentials.headers.is_empty(),
                    "repository credentials are empty"
                );
                reference
            }),
            _ => bail!("repository requires an authorized credential transfer"),
        };
        let observation = match repo.source {
            RepositorySource::Local { path } => {
                ensure!(
                    credential_reference.is_none(),
                    "local repositories have no credential transfer"
                );
                match reader
                    .inspect_repository(app::RepositorySource::Local { path }, None)
                    .await
                    .map_err(|_| anyhow::anyhow!("local repository could not be verified"))?
                {
                    app::RepositoryReadOutcome::Fresh(observation) => observation,
                    app::RepositoryReadOutcome::NotModified(_) => {
                        bail!("local repository observation missing")
                    }
                }
            }
            RepositorySource::Remote { url } => {
                let url = reader
                    .normalize_remote_repository_url(&url)
                    .map_err(|_| anyhow::anyhow!("remote repository address is invalid"))?;
                app::RepositoryObservation {
                    source_identity_key: url.as_bytes().to_vec(),
                    source: app::RepositorySource::Remote { url },
                    declared_id: repo.declared_id,
                    name: repo.display_name.clone(),
                    declared_url: None,
                    issues: Vec::new(),
                    packages: Vec::new(),
                    validators: app::RepositoryValidators::default(),
                    refreshed_at_ms: 0,
                }
            }
        };
        // The staged account already owns the imported vault records. Bind those exact
        // references and defer downloads to ordinary repository refresh after migration.
        let key = key("repository", index)?;
        let registered = match credential_reference {
            Some(reference) => {
                store
                    .register_repository_authenticated(
                        access.principal().clone(),
                        observation,
                        reference,
                        key,
                        now,
                    )
                    .await
            }
            None => {
                store
                    .register_repository(access.principal().clone(), observation, key, now)
                    .await
            }
        }
        .map_err(|_| anyhow::anyhow!("repository could not be registered"))?
        .value;
        if repo.hidden {
            hidden.push(registered.repository_id.to_string());
        }
        if repo.display_name.is_some() {
            registry
                .set_repository_display_name(
                    &access,
                    registered.repository_id,
                    registered.revision,
                    repo.display_name,
                    self::key("repository-name", index)?,
                )
                .await
                .map_err(|_| anyhow::anyhow!("repository name could not be saved"))?;
        }
    }
    if state.settings.hide_official_repository {
        hidden.push(app::VRCHAT_OFFICIAL_REPOSITORY_ID.to_owned());
    }
    if state.settings.hide_curated_repository {
        hidden.push(app::VRCHAT_CURATED_REPOSITORY_ID.to_owned());
    }
    let initial = settings
        .candidate_settings()
        .await
        .map_err(|_| anyhow::anyhow!("current settings unavailable"))?;
    settings
        .update_settings(
            &access,
            initial.revision,
            settings_update(state.settings, hidden)?,
        )
        .await
        .map_err(|_| anyhow::anyhow!("native settings validation failed"))?;

    let cache = destination.join("package-cache");
    let packages = app::UserPackageApplication::new(
        store.clone(),
        alcomd_vpm::UserPackageEngine::new(
            cache.clone(),
            destination.join("user-package-staging"),
        )?,
    );
    for (index, path) in state.local_packages.into_iter().enumerate() {
        packages
            .enroll(&access, path, key("local-package", index)?)
            .await
            .map_err(|_| anyhow::anyhow!("local package could not be enrolled"))?;
    }
    let templates = app::ProjectCreateApplication::with_locks(
        store.clone(),
        alcomd_vpm::ProjectCreateEngine::new(store.clone(), reader, cache)?,
        Arc::new(app::ResourceLockCoordinator::default()),
    );
    templates
        .seed_builtin_templates()
        .await
        .map_err(|_| anyhow::anyhow!("built-in template initialization failed"))?;
    for (index, template) in state.templates.into_iter().enumerate() {
        let source = match template.source {
            TemplateSource::ProjectArchive { archive_path, .. } => {
                app::ProjectTemplateSource::File {
                    source_file: attachment_path(bundle, &archive_path)?
                        .into_os_string()
                        .into_string()
                        .map_err(|_| anyhow::anyhow!("unsupported template path"))?,
                }
            }
            TemplateSource::Manual {
                base_template,
                unity_range,
                dependencies,
                unity_packages,
            } => {
                ensure!(
                    unity_packages.is_empty(),
                    "manual template contains unsupported Unity package overrides"
                );
                let (observed, _) = templates
                    .template_detail(&access, base_template.clone())
                    .await
                    .map_err(|_| anyhow::anyhow!("template base could not be reconciled"))?;
                app::ProjectTemplateSource::Manual {
                    id: template.id.context("manual template identifier missing")?,
                    name: template.name,
                    unity_version: unity_range,
                    dependencies: dependencies
                        .into_iter()
                        .map(
                            |(package_id, version_range)| app::ProjectTemplateDependency {
                                package_id,
                                version_range,
                            },
                        )
                        .collect(),
                    base_template: app::ProjectTemplateRef {
                        id: base_template,
                        revision: observed.revision,
                    },
                }
            }
        };
        let accepted = templates
            .save_template(&access, source, None, key("template", index)?)
            .await
            .map_err(|_| anyhow::anyhow!("template could not be accepted"))?;
        wait_for_task(&store, &access, accepted.operation_id).await?;
    }
    native_discord_import::stage(
        &store,
        &destination,
        &state.discord,
        explicit_discord,
        discord,
        now,
    )
    .await?;
    drop(templates);
    drop(packages);
    drop(registry);
    drop(unity);
    drop(store);
    alcomd_platform::sync_directory(&destination)?;
    Ok(receipt(counts))
}

fn receipt(counts: BTreeMap<&'static str, usize>) -> serde_json::Value {
    serde_json::json!({
        "schema": 1,
        "dataSchema": alcomd_store::CURRENT_DATA_SCHEMA,
        "configSchema": alcomd_store::FileConfigStore::SCHEMA_VERSION,
        "records": counts,
    })
}

fn digest_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Reopen the staged database and reconcile its current records against a verified bundle.
pub async fn check_native_state(
    bundle: &Path,
    destination: &Path,
    discord: Option<DiscordImportOptions<'_>>,
) -> Result<serde_json::Value> {
    let (expected, _) = validate_bundle(bundle)?;
    let explicit_discord = explicit_discord(&expected)?;
    let (_, identity) = alcomd_platform::resolve_plain_storage_directory(destination)?;
    let database = destination.join("state.db");
    let config = destination.join("config/settings.toml");
    ensure!(
        database.is_file() && config.is_file(),
        "staged state is incomplete"
    );
    super::preflight_data_layout(&database)?;
    let store = alcomd_store::StateStoreHandle::open(database)?;
    let saved = alcomd_store::FileConfigStore::new(config).load()?.settings;
    let owner = app::PrincipalId::local_owner();

    let projects = collect_pages!(owner, store.list_projects, projects);
    ensure!(
        projects.len() == expected.projects.len(),
        "project counts differ"
    );
    for original in &expected.projects {
        let project = projects
            .iter()
            .find(|candidate| same_path(&candidate.observation.root_path, &original.path))
            .context("registered project path differs")?;
        ensure!(
            project.favorite == original.favorite,
            "project favorite differs"
        );
        if let Some(registered) = original.registered_at_ms {
            ensure!(
                project.registered_at_ms == registered,
                "project registration time differs"
            );
        }
        let launch = store
            .get_project_launch_config(owner.clone(), project.project_id)
            .await
            .map_err(|_| anyhow::anyhow!("project launch configuration unavailable"))?;
        ensure!(
            launch.revision.is_some() == original.unity_launch_arguments.is_some()
                && launch.revision.is_some().then_some(&launch.arguments)
                    == original.unity_launch_arguments.as_ref(),
            "project launch configuration differs"
        );
    }
    let installations = collect_pages!(owner, store.list_installations, installations);
    ensure!(
        installations.len() == expected.unity_installations.len(),
        "Unity installation counts differ"
    );
    for original in &expected.unity_installations {
        let candidate = installations
            .iter()
            .find(|record| {
                same_path(
                    &record.observation.executable_path,
                    &original.executable_path,
                )
            })
            .context("Unity installation path differs")?;
        ensure!(
            original
                .version
                .as_ref()
                .is_none_or(|version| version == &candidate.observation.unity_version),
            "Unity version differs"
        );
        ensure!(
            (candidate.observation.source_kind == app::UnitySourceKind::HubConfig)
                == original.from_hub,
            "Unity registration source differs"
        );
    }
    let repos = collect_pages!(owner, store.list_repositories, repositories);
    ensure!(
        repos.len() == expected.repositories.len(),
        "repository counts differ"
    );
    let mut hidden = BTreeSet::new();
    for original in &expected.repositories {
        let candidate = repos
            .iter()
            .find(
                |record| match (&record.observation.source, &original.source) {
                    (
                        app::RepositorySource::Remote { url: current },
                        RepositorySource::Remote { url },
                    ) => current == url,
                    (
                        app::RepositorySource::Local { path: current },
                        RepositorySource::Local { path },
                    ) => same_path(current, path),
                    _ => false,
                },
            )
            .context("repository source differs")?;
        ensure!(
            candidate.credentials_configured == original.has_credentials,
            "repository credential state differs"
        );
        ensure!(
            candidate.display_name == original.display_name,
            "repository display name differs"
        );
        if original.hidden {
            hidden.insert(candidate.repository_id.to_string());
        }
    }
    if expected.settings.hide_official_repository {
        hidden.insert(app::VRCHAT_OFFICIAL_REPOSITORY_ID.to_owned());
    }
    if expected.settings.hide_curated_repository {
        hidden.insert(app::VRCHAT_CURATED_REPOSITORY_ID.to_owned());
    }
    ensure!(
        saved
            .packages
            .hidden_repository_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            == hidden,
        "repository visibility differs"
    );

    let packages = collect_pages!(owner, store.list_user_packages, user_packages);
    ensure!(
        packages.len() == expected.local_packages.len(),
        "local package counts differ"
    );
    for path in &expected.local_packages {
        ensure!(
            packages
                .iter()
                .any(|record| same_path(&record.source_root_path, path)),
            "local package path differs"
        );
    }
    let mut templates = Vec::new();
    let mut cursor = None;
    loop {
        let page = store
            .project_templates_list(owner.clone(), 100, cursor)
            .await
            .map_err(|_| anyhow::anyhow!("template catalog unavailable"))?;
        templates.extend(
            page.templates
                .into_iter()
                .filter(|template| !template.locked),
        );
        ensure!(templates.len() <= 128, "too many staged templates");
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    ensure!(
        templates.len() == expected.templates.len(),
        "template counts differ"
    );
    for original in &expected.templates {
        ensure!(
            templates
                .iter()
                .any(|record| record.declaration.name == original.name
                    && original
                        .id
                        .as_ref()
                        .is_none_or(|id| id == &record.declaration.id)),
            "template identity differs"
        );
    }
    let counts = expected.counts();
    let fields = settings_update(expected.settings, hidden.into_iter().collect())?;
    let appearance = fields.appearance.expect("native appearance fields");
    let packages = fields.packages.expect("native package fields");
    let backups = fields.backups.expect("native backup fields");
    let window = fields.window.expect("native window fields");
    let unity = fields.unity.expect("native Unity fields");
    let paths = fields.paths.expect("native path fields");
    ensure!(
        fields.locale == Some(saved.locale)
            && appearance.mode == Some(saved.appearance.mode)
            && appearance.motion == Some(saved.appearance.motion)
            && packages.show_prerelease == Some(saved.packages.show_prerelease)
            && packages.prefer_prerelease == Some(saved.packages.prefer_prerelease)
            && packages.hide_local_user_packages == Some(saved.packages.hide_local_user_packages)
            && backups.compression_mode == Some(saved.backups.compression_mode)
            && window.fullscreen == Some(saved.window.fullscreen)
            && fields.integrations == Some(saved.integrations)
            && optional_matches(
                unity.default_launch_arguments,
                saved.unity.default_launch_arguments
            )
            && optional_path_matches(unity.hub_path, saved.unity.hub_path)
            && optional_path_matches(paths.project_directory, saved.paths.project_directory)
            && optional_path_matches(paths.backup_directory, saved.paths.backup_directory),
        "native settings differ"
    );
    native_discord_import::check(
        &store,
        destination,
        &expected.discord,
        explicit_discord,
        discord,
    )
    .await?;
    let (_, reopened_identity) = alcomd_platform::resolve_plain_storage_directory(destination)?;
    ensure!(
        identity == reopened_identity,
        "native stage changed during check"
    );
    Ok(receipt(counts))
}

fn explicit_discord(state: &NativeState) -> Result<bool> {
    let mut ids = BTreeSet::new();
    for preference in &state.extensions {
        ensure!(
            ids.insert(preference.id.as_str()),
            "duplicate extension preference"
        );
        match preference.id.as_str() {
            "discord" if preference.visible => {}
            "mcp" if preference.enabled && preference.visible => {}
            _ => bail!("saved extension preference requires an equivalent current control"),
        }
    }
    Ok(ids.contains("discord"))
}

fn same_path(left: &str, right: &str) -> bool {
    left == right
        || std::fs::canonicalize(left)
            .is_ok_and(|left| std::fs::canonicalize(right).is_ok_and(|right| left == right))
}

fn optional_matches<T: PartialEq>(
    expected: app::ConfigNullableUpdate<T>,
    actual: Option<T>,
) -> bool {
    match expected {
        app::ConfigNullableUpdate::Clear | app::ConfigNullableUpdate::Unchanged => actual.is_none(),
        app::ConfigNullableUpdate::Set(value) => actual.as_ref() == Some(&value),
    }
}

fn optional_path_matches(
    expected: app::ConfigNullableUpdate<String>,
    actual: Option<String>,
) -> bool {
    match expected {
        app::ConfigNullableUpdate::Clear | app::ConfigNullableUpdate::Unchanged => actual.is_none(),
        app::ConfigNullableUpdate::Set(value) => {
            actual.is_some_and(|path| same_path(&value, &path))
        }
    }
}

fn key(kind: &str, index: usize) -> Result<app::IdempotencyKey> {
    app::IdempotencyKey::parse(format!("native-state-{kind}-{index}"))
        .map_err(|_| anyhow::anyhow!("invalid import operation key"))
}

async fn wait_for_task(
    store: &alcomd_store::StateStoreHandle,
    access: &app::AccessContext,
    id: app::OperationId,
) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(120), async {
        loop {
            let task = store
                .get_operation(access.principal().clone(), id)
                .await
                .map_err(|_| anyhow::anyhow!("native import task lookup failed"))?;
            if task.state.is_terminal() {
                ensure!(
                    task.state == app::OperationState::Succeeded,
                    "native import task failed"
                );
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .context("native import task timed out")?
}

fn optional<T>(value: Option<T>) -> app::ConfigNullableUpdate<T> {
    value.map_or(
        app::ConfigNullableUpdate::Clear,
        app::ConfigNullableUpdate::Set,
    )
}

fn settings_update(
    settings: alcomd_import::Settings,
    hidden: Vec<String>,
) -> Result<app::ConfigUpdate> {
    use app::{BackupCompression, ConfigAppearanceMode, ConfigAppearanceMotion, ConfigLocale};
    let locale = match settings.locale.as_str() {
        "system" => ConfigLocale::System,
        "en-US" => ConfigLocale::EnUs,
        "zh-CN" => ConfigLocale::ZhCn,
        "ja-JP" => ConfigLocale::JaJp,
        _ => bail!("unsupported locale"),
    };
    let mode = match settings.appearance_mode.as_str() {
        "system" => ConfigAppearanceMode::System,
        "light" => ConfigAppearanceMode::Light,
        "dark" => ConfigAppearanceMode::Dark,
        _ => bail!("unsupported appearance mode"),
    };
    let motion = match settings.appearance_motion.as_str() {
        "system" => ConfigAppearanceMotion::System,
        "full" => ConfigAppearanceMotion::Full,
        "reduced" => ConfigAppearanceMotion::Reduced,
        _ => bail!("unsupported appearance motion"),
    };
    let compression_mode = match settings.backup_compression.as_str() {
        "store" => BackupCompression::Store,
        "fast" => BackupCompression::Fast,
        "level3" => BackupCompression::Level3,
        "level5" => BackupCompression::Level5,
        "level7" => BackupCompression::Level7,
        "maximum" => BackupCompression::Maximum,
        _ => bail!("unsupported backup compression"),
    };
    Ok(app::ConfigUpdate {
        locale: Some(locale),
        appearance: Some(app::ConfigAppearanceUpdate {
            mode: Some(mode),
            motion: Some(motion),
            source_color: app::ConfigNullableUpdate::Unchanged,
            scheme: None,
        }),
        packages: Some(app::ConfigPackageSettingsUpdate {
            show_prerelease: Some(settings.show_prerelease),
            prefer_prerelease: Some(false),
            hidden_repository_ids: Some(hidden),
            hidden_repository_packages: None,
            hide_local_user_packages: Some(settings.hide_local_user_packages),
        }),
        unity: Some(app::ConfigUnityUpdate {
            hub_path: optional(settings.hub_path),
            default_launch_arguments: optional(settings.default_unity_launch_arguments),
        }),
        paths: Some(app::ConfigPathUpdate {
            project_directory: optional(settings.project_directory),
            backup_directory: optional(settings.backup_directory),
        }),
        window: Some(app::ConfigWindowUpdate {
            fullscreen: Some(settings.fullscreen),
            remember_state: settings.fullscreen.then_some(true),
        }),
        integrations: Some(app::ConfigIntegrationSettings {
            handle_vcc_links: settings.handle_vcc_links,
        }),
        backups: Some(app::ConfigBackupUpdate {
            compression_mode: Some(compression_mode),
            ..Default::default()
        }),
        ..Default::default()
    })
}
