//! Scope-limited imports into an existing account with the public native client.
use std::path::PathBuf;

use alcomd_client::{AlcomdClient, ClientConfig, ClientError};
use alcomd_import::{Project, UnityInstallation};
use alcomd_protocol::{
    AppearanceMode, AppearanceSettingsUpdate, CAPABILITY_PROJECTS_REGISTRY_V1,
    CAPABILITY_UNITY_MANAGE_V1, ProjectDiscoveryMode, ProjectSetFavoriteParams,
    ProjectUnityLaunchConfigSetParams, SettingsUpdate, SettingsUpdateParams,
    UnityInstallationRegisterParams, error_code,
};
use anyhow::{Context, Result, ensure};
use clap::{Args, ValueEnum};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[derive(Args)]
pub(super) struct Arguments {
    #[arg(long)]
    source: Source,
    #[arg(long)]
    source_root: PathBuf,
    #[arg(long)]
    category: Category,
    /// Apply the exact planDigest printed by a preceding preview on this source.
    #[arg(long)]
    apply_plan: Option<String>,
    /// Absolute directory for reviewed template archives, reused with the same plan on retry.
    #[arg(long)]
    staging_directory: Option<PathBuf>,
}

#[derive(Clone, Copy, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Source {
    Vcc,
    Alcom,
    Alcomd3Beta,
}

#[derive(Clone, Copy, ValueEnum)]
enum Category {
    Projects,
    Resources,
    Settings,
    Theme,
    Templates,
}

#[derive(Serialize)]
#[serde(
    tag = "category",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
enum Selection {
    Projects {
        projects: Vec<Project>,
        unity_installations: Vec<UnityInstallation>,
    },
    Resources {
        repositories: Vec<alcomd_import::Repository>,
        local_packages: Vec<String>,
    },
    Settings {
        patch: SettingsUpdate,
    },
    Theme {
        mode: AppearanceMode,
    },
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Merge {
    projects_confirmed: usize,
    existing_projects_unchanged: usize,
    unity_installations_confirmed: usize,
    repositories_confirmed: usize,
    local_packages_confirmed: usize,
    settings_changed: bool,
    theme_changed: bool,
}

pub(super) fn run(arguments: Arguments) -> Result<Value> {
    let (root, original) =
        alcomd_platform::resolve_plain_storage_directory(&arguments.source_root)?;
    if matches!(arguments.category, Category::Templates) {
        return crate::selection_templates::run(
            &root,
            &original,
            arguments.source,
            arguments.apply_plan.as_deref(),
            arguments.staging_directory.as_deref(),
        );
    }
    ensure!(
        arguments.staging_directory.is_none(),
        "staging directory is only used for template selection"
    );
    let selected = match arguments.category {
        Category::Projects => {
            ensure!(
                !matches!(arguments.source, Source::Alcomd3Beta),
                "Beta has no separate Projects selection"
            );
            let (projects, unity_installations) = crate::profile::selected_projects(&root)?;
            Selection::Projects {
                projects,
                unity_installations,
            }
        }
        Category::Resources => {
            ensure!(
                !matches!(arguments.source, Source::Alcomd3Beta),
                "Beta has no separate supported registry resource selection"
            );
            let (repositories, local_packages) = crate::profile::selected_resources(&root)?;
            Selection::Resources {
                repositories,
                local_packages,
            }
        }
        Category::Settings => Selection::Settings {
            patch: crate::profile::selected_settings(
                &root,
                matches!(arguments.source, Source::Alcomd3Beta),
            )?,
        },
        Category::Theme => {
            let beta = matches!(arguments.source, Source::Alcomd3Beta);
            let mode = match crate::profile::selected_theme(&root, beta)? {
                "system" => AppearanceMode::System,
                "dark" => AppearanceMode::Dark,
                "light" => AppearanceMode::Light,
                _ => anyhow::bail!("unsupported legacy theme"),
            };
            Selection::Theme { mode }
        }
        Category::Templates => unreachable!(),
    };
    let (_, current) = alcomd_platform::resolve_plain_storage_directory(&root)?;
    ensure!(original == current, "selected source was replaced");
    let plan = serde_json::to_vec(&(
        "alcomd-selective-import-v1",
        arguments.source,
        &root,
        original,
        &selected,
    ))?;
    let digest = crate::hex_digest(&Sha256::digest(&plan));
    let Some(approved) = arguments.apply_plan else {
        return Ok(preview(arguments.source, &digest, &selected));
    };
    ensure!(
        approved == digest,
        "source no longer matches the approved selection"
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let result = runtime.block_on(async {
        let config = ClientConfig::for_installed_core()
            .without_daemon_start()
            .with_client_name("alcomd-selective-import");
        let mut core = AlcomdClient::connect(config).await?;
        preflight(&mut core, &selected).await?;
        merge(&selected, &digest, async |step| {
            apply_step(&mut core, step).await
        })
        .await
    })?;
    Ok(json!({"planDigest": digest, "merged": result}))
}

fn preview(source: Source, digest: &str, selected: &Selection) -> Value {
    match selected {
        Selection::Projects {
            projects,
            unity_installations,
        } => json!({
            "source": source, "category": "projects", "planDigest": digest,
            "projects": projects.len(), "unityInstallations": unity_installations.len(),
            "favoriteProjects": projects.iter().filter(|project| project.favorite).count(),
            "projectsWithLaunchArguments": projects.iter().filter(|project| project.unity_launch_arguments.is_some()).count(),
        }),
        Selection::Resources {
            repositories,
            local_packages,
        } => json!({
            "source": source, "category": "resources", "planDigest": digest,
            "repositories": repositories.len(), "localPackages": local_packages.len(),
        }),
        Selection::Settings { patch } => {
            let document = serde_json::to_value(patch).expect("typed settings patch");
            let fields = document
                .as_object()
                .expect("settings groups")
                .keys()
                .collect::<Vec<_>>();
            json!({"source": source, "category": "settings", "planDigest": digest, "groups": fields})
        }
        Selection::Theme { mode } => {
            json!({ "source": source, "category": "theme", "planDigest": digest, "appearanceMode": mode })
        }
    }
}

async fn preflight(core: &mut AlcomdClient, selected: &Selection) -> Result<()> {
    if let Selection::Projects {
        projects,
        unity_installations,
    } = selected
    {
        ensure!(
            projects.is_empty() || core.supports_capability(CAPABILITY_PROJECTS_REGISTRY_V1),
            "Core lacks the project registry"
        );
        ensure!(
            (unity_installations.is_empty()
                && projects
                    .iter()
                    .all(|item| item.unity_launch_arguments.is_none()))
                || core.supports_capability(CAPABILITY_UNITY_MANAGE_V1),
            "Core lacks the Unity registry or launch settings",
        );
        for project in projects {
            core.project_inspect(project.path.clone(), ProjectDiscoveryMode::ExactRoot)
                .await?;
        }
        for unity in unity_installations {
            alcomd_platform::inspect_unity_installation(std::path::Path::new(
                &unity.executable_path,
            ))?;
        }
    }
    Ok(())
}

enum Step {
    RegisterProject {
        path: String,
        key: String,
    },
    Favorite {
        id: String,
        revision: u64,
        key: String,
    },
    LaunchArguments {
        id: String,
        arguments: Vec<String>,
        key: String,
    },
    RegisterUnity {
        path: String,
        key: String,
    },
    RegisterRepository {
        source: alcomd_protocol::RepositorySource,
        key: String,
    },
    EnrollLocalPackage {
        source_path: String,
        key: String,
    },
    Settings(Box<SettingsUpdate>, String),
    Theme(AppearanceMode, String),
}

#[derive(Debug)]
enum Reply {
    Project { id: String, revision: u64 },
    ExistingProject,
    Done,
    Theme(bool),
    Settings(bool),
}

async fn merge(
    selected: &Selection,
    digest: &str,
    mut call: impl AsyncFnMut(Step) -> Result<Reply>,
) -> Result<Merge> {
    let mut summary = Merge::default();
    match selected {
        Selection::Projects {
            projects,
            unity_installations,
        } => {
            for (index, project) in projects.iter().enumerate() {
                let key = format!("select:{digest}:project:{index}");
                match call(Step::RegisterProject {
                    path: project.path.clone(),
                    key: key.clone(),
                })
                .await?
                {
                    Reply::Project { id, revision } => {
                        if project.favorite {
                            call(Step::Favorite {
                                id: id.clone(),
                                revision,
                                key: format!("{key}:favorite"),
                            })
                            .await?;
                        }
                        if let Some(arguments) = &project.unity_launch_arguments {
                            call(Step::LaunchArguments {
                                id,
                                arguments: arguments.clone(),
                                key: format!("{key}:arguments"),
                            })
                            .await?;
                        }
                        summary.projects_confirmed += 1;
                    }
                    Reply::ExistingProject => summary.existing_projects_unchanged += 1,
                    _ => anyhow::bail!("unexpected project receipt"),
                }
            }
            for (index, unity) in unity_installations.iter().enumerate() {
                call(Step::RegisterUnity {
                    path: unity.executable_path.clone(),
                    key: format!("select:{digest}:unity:{index}"),
                })
                .await?;
                summary.unity_installations_confirmed += 1;
            }
        }
        Selection::Resources {
            repositories,
            local_packages,
        } => {
            for (index, record) in repositories.iter().enumerate() {
                let source = match &record.source {
                    alcomd_import::RepositorySource::Local { path } => {
                        alcomd_protocol::RepositorySource::Local { path: path.clone() }
                    }
                    alcomd_import::RepositorySource::Remote { url } => {
                        alcomd_protocol::RepositorySource::Remote { url: url.clone() }
                    }
                };
                call(Step::RegisterRepository {
                    source,
                    key: format!("select:{digest}:repo:{index}"),
                })
                .await?;
                summary.repositories_confirmed += 1;
            }
            for (index, source_path) in local_packages.iter().enumerate() {
                call(Step::EnrollLocalPackage {
                    source_path: source_path.clone(),
                    key: format!("select:{digest}:local-package:{index}"),
                })
                .await?;
                summary.local_packages_confirmed += 1;
            }
        }
        Selection::Settings { patch } => match call(Step::Settings(
            Box::new(patch.clone()),
            format!("select:{digest}:settings"),
        ))
        .await?
        {
            Reply::Settings(changed) => summary.settings_changed = changed,
            _ => anyhow::bail!("unexpected settings receipt"),
        },
        Selection::Theme { mode } => {
            match call(Step::Theme(*mode, format!("select:{digest}:theme"))).await? {
                Reply::Theme(changed) => summary.theme_changed = changed,
                _ => anyhow::bail!("unexpected settings receipt"),
            }
        }
    }
    Ok(summary)
}

async fn has_enrolled_source(core: &mut AlcomdClient, path: &str) -> Result<bool> {
    #[derive(serde::Deserialize)]
    struct PackageIdentity {
        name: String,
    }

    let (root, selected) = alcomd_platform::resolve_directory_identity(std::path::Path::new(path))?;
    let manifest = crate::profile::read_file(&root, "package.json", 1_048_576)?
        .context("selected package manifest is missing")?;
    let package = serde_json::from_slice::<PackageIdentity>(&manifest)?;
    let mut cursor = None;
    loop {
        let page = core
            .user_packages_list(alcomd_protocol::UserPackagesListParams {
                cursor,
                limit: None,
            })
            .await?;
        for record in page.user_packages {
            if record.package_id == package.name
                && alcomd_platform::resolve_directory_identity(std::path::Path::new(
                    &record.source_root_path,
                ))
                .is_ok_and(|(_, identity)| identity == selected)
            {
                return Ok(true);
            }
        }
        let Some(next) = page.next_cursor else {
            return Ok(false);
        };
        cursor = Some(next);
    }
}

async fn apply_step(core: &mut AlcomdClient, step: Step) -> Result<Reply> {
    match step {
        Step::RegisterProject { path, key } => match core
            .project_register(alcomd_protocol::ProjectRegisterParams {
                source: alcomd_protocol::ProjectSource::Absolute { path },
                idempotency_key: key,
            })
            .await
        {
            Ok(result) => Ok(Reply::Project {
                id: result
                    .project
                    .project_id
                    .context("project registration was not returned")?,
                revision: result
                    .project
                    .revision
                    .context("project revision was not returned")?,
            }),
            Err(ClientError::Remote(error))
                if error.code == error_code::PROJECT_ALREADY_REGISTERED =>
            {
                Ok(Reply::ExistingProject)
            }
            Err(error) => Err(error.into()),
        },
        Step::Favorite { id, revision, key } => {
            core.project_set_favorite(ProjectSetFavoriteParams {
                project_id: id,
                favorite: true,
                expected_revision: revision,
                idempotency_key: key,
            })
            .await?;
            Ok(Reply::Done)
        }
        Step::LaunchArguments { id, arguments, key } => {
            let current = core.unity_project_launch_config_get(id.clone()).await?;
            if current.config.revision == 0 {
                core.unity_project_launch_config_set(ProjectUnityLaunchConfigSetParams {
                    project_id: id,
                    arguments,
                    expected_revision: 0,
                    idempotency_key: key,
                })
                .await?;
            }
            Ok(Reply::Done)
        }
        Step::RegisterUnity { path, key } => {
            core.unity_installation_register(UnityInstallationRegisterParams {
                executable_path: path,
                idempotency_key: key,
            })
            .await?;
            Ok(Reply::Done)
        }
        Step::RegisterRepository { source, key } => {
            let mut cursor = None;
            loop {
                let page = core.repositories_list(cursor, None).await?;
                if page.repositories.iter().any(|row| row.source == source) {
                    return Ok(Reply::Done);
                }
                let Some(next) = page.next_cursor else { break };
                cursor = Some(next);
            }
            let preview_id = core
                .repository_inspect(source)
                .await?
                .preview_id
                .ok_or(ClientError::InvalidResponse)?;
            // A new inspection reviews a new preview. A receipt belongs to that exact request.
            let request_key = format!("{key}:{preview_id}");
            match core.repository_register(preview_id, request_key).await {
                Ok(_) => {}
                Err(ClientError::Remote(error))
                    if error.code == error_code::REPOSITORY_ALREADY_REGISTERED => {}
                Err(error) => return Err(error.into()),
            }
            Ok(Reply::Done)
        }
        Step::EnrollLocalPackage { source_path, key } => {
            match core
                .user_package_enroll(alcomd_protocol::UserPackageEnrollParams {
                    source_path: source_path.clone(),
                    idempotency_key: key,
                })
                .await
            {
                Ok(_) => {}
                Err(ClientError::Remote(error))
                    if error.code == "user_package_already_enrolled" =>
                {
                    if !has_enrolled_source(core, &source_path).await? {
                        return Err(ClientError::Remote(error).into());
                    }
                }
                Err(error) => return Err(error.into()),
            }
            Ok(Reply::Done)
        }
        Step::Settings(patch, key) => {
            let current = core.settings_get().await?;
            let expected = serde_json::to_value(&patch)?;
            if matches_object_fields(&expected, &serde_json::to_value(&current.settings)?) {
                return Ok(Reply::Settings(false));
            }
            core.settings_update(SettingsUpdateParams {
                idempotency_key: format!("{key}:{}", current.revision),
                expected_revision: current.revision,
                update: *patch,
            })
            .await?;
            Ok(Reply::Settings(true))
        }
        Step::Theme(mode, key) => {
            let current = core.settings_get().await?;
            if current.settings.appearance.mode == mode {
                return Ok(Reply::Theme(false));
            }
            core.settings_update(theme_update(
                current.revision,
                mode,
                format!("{key}:{}", current.revision),
            ))
            .await?;
            Ok(Reply::Theme(true))
        }
    }
}

fn matches_object_fields(expected: &Value, current: &Value) -> bool {
    match expected {
        Value::Object(fields) => fields.iter().all(|(key, value)| {
            current
                .get(key)
                .is_some_and(|saved| matches_object_fields(value, saved))
        }),
        _ => expected == current,
    }
}

fn theme_update(
    revision: u64,
    mode: AppearanceMode,
    idempotency_key: String,
) -> SettingsUpdateParams {
    SettingsUpdateParams {
        idempotency_key,
        expected_revision: revision,
        update: SettingsUpdate {
            appearance: Some(AppearanceSettingsUpdate {
                mode: Some(mode),
                ..Default::default()
            }),
            ..Default::default()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn resources_merge_only_identical_enrollments_and_preserve_their_revision() {
        use alcomd_protocol::{
            McpClientMutationParams, McpClientsCreateParams, UserPackageEnrollParams,
            UserPackagesListParams,
        };
        let root = std::env::temp_dir()
            .canonicalize()
            .expect("temp directory")
            .join(format!(
                "ac-s-{}",
                &uuid::Uuid::new_v4().simple().to_string()[..12]
            ));
        std::fs::create_dir(&root).expect("isolated selection root");
        let mut config = ClientConfig::default().without_daemon_start();
        let ipc = if cfg!(unix) {
            config = config.with_runtime_directory(root.join("run"));
            alcomd_platform::IpcConfig::isolated(root.join("run"))
        } else {
            alcomd_platform::IpcConfig::default()
        };
        let (stop, stopped) = tokio::sync::oneshot::channel();
        let daemon = alcomd_daemon::test_support::start_with_data_until(
            ipc,
            alcomd_platform::DataConfig::isolated(root.join("data")),
            async {
                let _ = stopped.await;
            },
        )
        .await
        .expect("bind isolated Core before connecting");
        let mut core = AlcomdClient::connect(config.clone())
            .await
            .expect("Core client");
        let package = |folder: &str, package_id: &str| {
            let path = root.join(folder);
            std::fs::create_dir(&path).expect("fixture source");
            std::fs::write(
                path.join("package.json"),
                json!({"name":package_id,"version":"1.0.0"}).to_string(),
            )
            .expect("fixture manifest");
            path.to_str().expect("source path").to_owned()
        };
        let existing = package("first", "example.first");
        let next = package("next", "example.next");
        let before = core
            .user_package_enroll(UserPackageEnrollParams {
                source_path: existing.clone(),
                idempotency_key: "previous-enrollment".into(),
            })
            .await
            .expect("already enrolled source")
            .user_package;
        std::fs::write(
            std::path::Path::new(&existing).join("package.json"),
            json!({"name":"example.first","version":"2.0.0"}).to_string(),
        )
        .expect("change source without refreshing its registration");
        let selected = Selection::Resources {
            repositories: Vec::new(),
            local_packages: vec![existing.clone(), next],
        };
        let merged = merge(&selected, "overlap", async |step| {
            apply_step(&mut core, step).await
        })
        .await
        .expect("merge overlapping resources");
        assert_eq!(merged.local_packages_confirmed, 2);
        let page = core
            .user_packages_list(UserPackagesListParams {
                cursor: None,
                limit: None,
            })
            .await
            .expect("enrollments");
        assert_eq!(page.user_packages.len(), 2);
        assert!(page.user_packages.contains(&before));
        assert!(
            page.user_packages
                .iter()
                .any(|record| record.package_id == "example.next")
        );

        let collision = package("duplicate-id", "example.first");
        let expect_error = |error: anyhow::Error, code| {
            assert!(
                matches!(error.downcast_ref::<ClientError>(), Some(ClientError::Remote(remote)) if remote.code == code),
                "{error:#}"
            );
        };
        expect_error(
            apply_step(
                &mut core,
                Step::EnrollLocalPackage {
                    source_path: collision,
                    key: "collision".into(),
                },
            )
            .await
            .expect_err("same package name in a different folder"),
            "user_package_already_enrolled",
        );
        std::fs::write(
            std::path::Path::new(&existing).join("package.json"),
            json!({"name":"example.renamed","version":"1.0.0"}).to_string(),
        )
        .expect("change package identity in the same directory");
        expect_error(
            apply_step(
                &mut core,
                Step::EnrollLocalPackage {
                    source_path: existing.clone(),
                    key: "changed-id".into(),
                },
            )
            .await
            .expect_err("changed package identity"),
            "user_package_source_changed",
        );
        std::fs::rename(&existing, root.join("original-first"))
            .expect("keep original directory identity");
        package("first", "example.first");
        expect_error(
            apply_step(
                &mut core,
                Step::EnrollLocalPackage {
                    source_path: existing.clone(),
                    key: "replacement".into(),
                },
            )
            .await
            .expect_err("the same path now refers to a different directory"),
            "user_package_source_changed",
        );

        let client = core
            .mcp_client_create(McpClientsCreateParams {
                display_name: "Selection test".into(),
                idempotency_key: "test-client".into(),
            })
            .await
            .expect("identity");
        let token = core
            .mcp_client_issue_credential(McpClientMutationParams {
                client_id: client.client_id.clone(),
                expected_revision: client.revision,
                idempotency_key: "test-token".into(),
            })
            .await
            .expect("credential");
        let mut delegated = AlcomdClient::connect(config.with_mcp_credential(token.credential))
            .await
            .expect("authenticated connection");
        core.mcp_client_revoke(McpClientMutationParams {
            client_id: client.client_id,
            expected_revision: token.revision,
            idempotency_key: "revoke".into(),
        })
        .await
        .expect("revoke identity");
        expect_error(
            apply_step(
                &mut delegated,
                Step::EnrollLocalPackage {
                    source_path: existing,
                    key: "revoked".into(),
                },
            )
            .await
            .expect_err("revoked connection"),
            "permission_denied",
        );
        drop(delegated);
        drop(core);
        stop.send(()).expect("request shutdown");
        daemon.await.expect("daemon joined").expect("shutdown");
        std::fs::remove_dir_all(root).expect("clean test directory");
    }

    #[tokio::test]
    async fn selection_keeps_existing_registrations_and_uses_frozen_keys_for_new_records() {
        let project = |path: &str| Project {
            path: path.to_owned(),
            favorite: true,
            registered_at_ms: None,
            last_modified_at_ms: None,
            unity_version: None,
            unity_revision: None,
            unity_executable_path: None,
            unity_launch_arguments: Some(vec![]),
        };
        let plan = Selection::Projects {
            projects: vec![
                project("already registered"),
                project("new selected project"),
            ],
            unity_installations: vec![UnityInstallation {
                executable_path: "selected Unity".into(),
                version: None,
                from_hub: false,
            }],
        };
        let mut keys = Vec::new();
        let outcome = merge(&plan, "fixture", async |step| {
            Ok(match step {
                Step::RegisterProject { path, key } => {
                    keys.push(key);
                    if path == "already registered" {
                        Reply::ExistingProject
                    } else {
                        Reply::Project {
                            id: "new identity".into(),
                            revision: 5,
                        }
                    }
                }
                Step::Favorite { id, revision, key } => {
                    assert_eq!(id, "new identity");
                    assert_eq!(revision, 5);
                    keys.push(key);
                    Reply::Done
                }
                Step::LaunchArguments { id, arguments, key } => {
                    assert_eq!(id, "new identity");
                    assert!(arguments.is_empty());
                    keys.push(key);
                    Reply::Done
                }
                Step::RegisterUnity { path, key } => {
                    assert_eq!(path, "selected Unity");
                    keys.push(key);
                    Reply::Done
                }
                Step::Theme(_, _)
                | Step::Settings(_, _)
                | Step::RegisterRepository { .. }
                | Step::EnrollLocalPackage { .. } => {
                    panic!("unselected account groups may not change")
                }
            })
        })
        .await
        .expect("selected merge");
        assert_eq!(outcome.projects_confirmed, 1);
        assert_eq!(outcome.existing_projects_unchanged, 1);
        assert_eq!(outcome.unity_installations_confirmed, 1);
        assert_eq!(
            keys,
            [
                "select:fixture:project:0",
                "select:fixture:project:1",
                "select:fixture:project:1:favorite",
                "select:fixture:project:1:arguments",
                "select:fixture:unity:0"
            ]
        );
    }

    #[test]
    fn theme_update_carries_no_unselected_settings() {
        assert_eq!(
            serde_json::to_value(theme_update(17, AppearanceMode::Dark, "theme".into()))
                .expect("typed update"),
            json!({"idempotencyKey":"theme","expectedRevision":17,"update":{"appearance":{"mode":"dark"}}})
        );
    }
}
