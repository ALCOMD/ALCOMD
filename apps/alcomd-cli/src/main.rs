use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

/// Command-line client for ALCOMD.
#[derive(Debug, Parser)]
#[command(name = "alcomd-cli", version, about)]
struct Arguments {
    /// Emit machine-readable JSON.
    #[arg(long, global = true)]
    json: bool,

    /// Do not start the daemon when its endpoint is absent.
    #[arg(long, global = true)]
    no_start_daemon: bool,

    /// Override the private Unix runtime directory for isolated testing.
    #[arg(long, global = true, hide = true)]
    runtime_dir: Option<PathBuf>,

    /// Override the daemon data directory for isolated testing.
    #[arg(long, global = true, hide = true)]
    data_dir: Option<PathBuf>,

    /// Override the daemon executable for isolated testing.
    #[arg(long, global = true, hide = true)]
    daemon_path: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Inspect or control core components.
    System {
        #[command(subcommand)]
        command: SystemCommand,
    },
    /// Inspect and manage only the ALCOMD project registry.
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Inspect and manage normalized VPM repository metadata.
    Repository {
        #[command(subcommand)]
        command: RepositoryCommand,
    },
    /// Plan and apply the minimal M4 VPM package transaction slice.
    Package {
        #[command(subcommand)]
        command: PackageCommand,
    },
    /// Inspect and manage native ALCOMD Template bundles.
    Template {
        #[command(subcommand)]
        command: TemplateCommand,
    },
}

#[derive(Debug, Subcommand)]
enum SystemCommand {
    /// Query the running per-user daemon.
    Status,
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    Create {
        #[arg(long)]
        template: String,
        target_parent: PathBuf,
        target_leaf: String,
        #[arg(long)]
        expected_template_revision: u64,
        #[arg(long)]
        idempotency_key: String,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        no_wait: bool,
    },
    Inspect {
        path: PathBuf,
        #[arg(long)]
        search_parents: bool,
    },
    List {
        #[arg(long)]
        limit: Option<u32>,
    },
    Get {
        project_id: String,
    },
    Register {
        path: PathBuf,
        #[arg(long)]
        idempotency_key: String,
    },
    Refresh {
        project_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
    Unregister {
        project_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
}

#[derive(Debug, Subcommand)]
enum RepositoryCommand {
    Inspect {
        source: String,
    },
    List {
        #[arg(long)]
        limit: Option<u32>,
    },
    Get {
        repository_id: String,
    },
    Packages {
        repository_id: String,
        #[arg(long)]
        limit: Option<u32>,
    },
    Register {
        source: String,
        #[arg(long)]
        idempotency_key: String,
    },
    Refresh {
        repository_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
    Unregister {
        repository_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
}

#[derive(Debug, Subcommand)]
enum PackageCommand {
    PlanInstall {
        project_id: String,
        package_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        version_range: Option<String>,
        #[arg(long)]
        repository_id: Option<String>,
        #[arg(long)]
        include_prerelease: bool,
    },
    PlanRemove {
        project_id: String,
        package_id: String,
        #[arg(long)]
        expected_revision: u64,
    },
    PlanUpgrade {
        project_id: String,
        package_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        version_range: Option<String>,
        #[arg(long)]
        repository_id: Option<String>,
        #[arg(long)]
        include_prerelease: bool,
    },
    PlanDowngrade {
        project_id: String,
        package_id: String,
        version: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        repository_id: Option<String>,
    },
    PlanResolve {
        project_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        include_prerelease: bool,
    },
    ApplyPlan {
        plan_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
}

#[derive(Debug, Subcommand)]
enum TemplateCommand {
    List {
        #[arg(long)]
        limit: Option<u32>,
    },
    Get {
        template_id: String,
    },
    Inspect {
        bundle_path: PathBuf,
    },
    Import {
        bundle_path: PathBuf,
        #[arg(long)]
        override_existing: bool,
        #[arg(long, default_value_t = 0)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        no_wait: bool,
    },
    Derive {
        project_id: String,
        template_id: String,
        #[arg(long)]
        expected_project_revision: u64,
        #[arg(long)]
        template_version: String,
        #[arg(long)]
        display_name: String,
        #[arg(long)]
        description: Option<String>,
        #[arg(long)]
        idempotency_key: String,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        no_wait: bool,
    },
    Export {
        template_id: String,
        target_path: PathBuf,
        #[arg(long)]
        expected_revision: u64,
    },
    Favorite {
        template_id: String,
        favorite: bool,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
    Remove {
        template_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
        #[arg(long)]
        yes: bool,
    },
}

#[tokio::main]
async fn main() -> ExitCode {
    let arguments = Arguments::parse();

    let mut config = alcomd_client::ClientConfig::default();
    if arguments.no_start_daemon {
        config = config.without_daemon_start();
    }
    if let Some(path) = arguments.runtime_dir {
        config = config.with_runtime_directory(path);
    }
    if let Some(path) = arguments.data_dir {
        config = config.with_data_directory(path);
    }
    if let Some(path) = arguments.daemon_path {
        config = config.with_daemon_path(path);
    }

    match execute(config, arguments.command).await {
        Ok(value) => {
            print_result(arguments.json, &value);
            ExitCode::SUCCESS
        }
        Err(error) => {
            print_error(arguments.json, &error);
            ExitCode::FAILURE
        }
    }
}

async fn execute(
    config: alcomd_client::ClientConfig,
    command: Command,
) -> Result<serde_json::Value, alcomd_client::ClientError> {
    let mut client = alcomd_client::AlcomdClient::connect(config).await?;
    let value = match command {
        Command::System {
            command: SystemCommand::Status,
        } => serde_json::to_value(client.system_status().await?),
        Command::Project { command } => match command {
            ProjectCommand::Create {
                template,
                target_parent,
                target_leaf,
                expected_template_revision,
                idempotency_key,
                yes,
                dry_run,
                no_wait,
            } => {
                let target_parent =
                    absolute_path(target_parent).map_err(alcomd_client::ClientError::Transport)?;
                let plan = client
                    .template_plan_create_project(
                        alcomd_protocol::TemplatePlanCreateProjectParams {
                            template_id: template,
                            expected_template_revision,
                            target_parent,
                            target_leaf,
                        },
                    )
                    .await?;
                if dry_run {
                    serde_json::to_value(plan)
                } else {
                    confirm_high_impact(yes)?;
                    let accepted = client
                        .template_apply_create_project(alcomd_protocol::TemplateApplyPlanParams {
                            plan_id: plan.plan_id,
                            idempotency_key,
                        })
                        .await?;
                    if no_wait {
                        serde_json::to_value(accepted)
                    } else {
                        serde_json::to_value(
                            wait_for_operation(&mut client, accepted.operation_id).await?,
                        )
                    }
                }
            }
            ProjectCommand::Inspect {
                path,
                search_parents,
            } => {
                let path = absolute_path(path).map_err(alcomd_client::ClientError::Transport)?;
                serde_json::to_value(
                    client
                        .project_inspect(
                            path,
                            if search_parents {
                                alcomd_protocol::ProjectDiscoveryMode::SearchParents
                            } else {
                                alcomd_protocol::ProjectDiscoveryMode::ExactRoot
                            },
                        )
                        .await?,
                )
            }
            ProjectCommand::List { limit } => {
                serde_json::to_value(client.projects_list(None, limit).await?)
            }
            ProjectCommand::Get { project_id } => {
                serde_json::to_value(client.project_get(project_id).await?)
            }
            ProjectCommand::Register {
                path,
                idempotency_key,
            } => {
                let path = absolute_path(path).map_err(alcomd_client::ClientError::Transport)?;
                serde_json::to_value(client.project_register(path, idempotency_key).await?)
            }
            ProjectCommand::Refresh {
                project_id,
                expected_revision,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .project_refresh(project_id, expected_revision, idempotency_key)
                    .await?,
            ),
            ProjectCommand::Unregister {
                project_id,
                expected_revision,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .project_unregister(project_id, expected_revision, idempotency_key)
                    .await?,
            ),
        },
        Command::Repository { command } => match command {
            RepositoryCommand::Inspect { source } => serde_json::to_value(
                client
                    .repository_inspect(repository_source(source)?)
                    .await?,
            ),
            RepositoryCommand::List { limit } => {
                serde_json::to_value(client.repositories_list(None, limit).await?)
            }
            RepositoryCommand::Get { repository_id } => {
                serde_json::to_value(client.repository_get(repository_id).await?)
            }
            RepositoryCommand::Packages {
                repository_id,
                limit,
            } => serde_json::to_value(
                client
                    .repository_packages(repository_id, None, limit)
                    .await?,
            ),
            RepositoryCommand::Register {
                source,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .repository_register(repository_source(source)?, idempotency_key)
                    .await?,
            ),
            RepositoryCommand::Refresh {
                repository_id,
                expected_revision,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .repository_refresh(repository_id, expected_revision, idempotency_key)
                    .await?,
            ),
            RepositoryCommand::Unregister {
                repository_id,
                expected_revision,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .repository_unregister(repository_id, expected_revision, idempotency_key)
                    .await?,
            ),
        },
        Command::Package { command } => match command {
            PackageCommand::PlanInstall {
                project_id,
                package_id,
                expected_revision,
                version_range,
                repository_id,
                include_prerelease,
            } => serde_json::to_value(
                client
                    .package_plan_install(alcomd_protocol::PackagePlanInstallParams {
                        project_id,
                        expected_revision,
                        package_id,
                        version_range,
                        repository_id,
                        include_prerelease,
                    })
                    .await?,
            ),
            PackageCommand::PlanRemove {
                project_id,
                package_id,
                expected_revision,
            } => serde_json::to_value(
                client
                    .package_plan_remove(alcomd_protocol::PackagePlanRemoveParams {
                        project_id,
                        expected_revision,
                        package_id,
                    })
                    .await?,
            ),
            PackageCommand::PlanUpgrade {
                project_id,
                package_id,
                expected_revision,
                version_range,
                repository_id,
                include_prerelease,
            } => serde_json::to_value(
                client
                    .package_plan_upgrade(alcomd_protocol::PackagePlanUpgradeParams {
                        project_id,
                        expected_revision,
                        package_id,
                        version_range,
                        repository_id,
                        include_prerelease,
                    })
                    .await?,
            ),
            PackageCommand::PlanDowngrade {
                project_id,
                package_id,
                version,
                expected_revision,
                repository_id,
            } => serde_json::to_value(
                client
                    .package_plan_downgrade(alcomd_protocol::PackagePlanDowngradeParams {
                        project_id,
                        expected_revision,
                        package_id,
                        version,
                        repository_id,
                    })
                    .await?,
            ),
            PackageCommand::PlanResolve {
                project_id,
                expected_revision,
                include_prerelease,
            } => serde_json::to_value(
                client
                    .package_plan_resolve(alcomd_protocol::PackagePlanResolveParams {
                        project_id,
                        expected_revision,
                        include_prerelease,
                    })
                    .await?,
            ),
            PackageCommand::ApplyPlan {
                plan_id,
                expected_revision,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .package_apply_plan(alcomd_protocol::PackageApplyPlanParams {
                        plan_id,
                        expected_revision,
                        idempotency_key,
                    })
                    .await?,
            ),
        },
        Command::Template { command } => match command {
            TemplateCommand::List { limit } => serde_json::to_value(
                client
                    .templates_list(alcomd_protocol::TemplatesListParams {
                        cursor: None,
                        limit,
                    })
                    .await?,
            ),
            TemplateCommand::Get { template_id } => {
                serde_json::to_value(client.template_get(template_id).await?)
            }
            TemplateCommand::Inspect { bundle_path } => {
                let bundle_path =
                    absolute_path(bundle_path).map_err(alcomd_client::ClientError::Transport)?;
                serde_json::to_value(client.template_inspect_bundle(bundle_path).await?)
            }
            TemplateCommand::Import {
                bundle_path,
                override_existing,
                expected_revision,
                idempotency_key,
                yes,
                dry_run,
                no_wait,
            } => {
                let bundle_path =
                    absolute_path(bundle_path).map_err(alcomd_client::ClientError::Transport)?;
                let plan = client
                    .template_plan_import(alcomd_protocol::TemplatePlanImportParams {
                        bundle_path,
                        override_existing,
                        expected_revision,
                    })
                    .await?;
                if dry_run {
                    serde_json::to_value(plan)
                } else {
                    confirm_high_impact(yes)?;
                    let accepted = client
                        .template_apply_import(alcomd_protocol::TemplateApplyPlanParams {
                            plan_id: plan.plan_id,
                            idempotency_key,
                        })
                        .await?;
                    if no_wait {
                        serde_json::to_value(accepted)
                    } else {
                        serde_json::to_value(
                            wait_for_operation(&mut client, accepted.operation_id).await?,
                        )
                    }
                }
            }
            TemplateCommand::Derive {
                project_id,
                template_id,
                expected_project_revision,
                template_version,
                display_name,
                description,
                idempotency_key,
                yes,
                dry_run,
                no_wait,
            } => {
                let plan = client
                    .template_plan_derive(alcomd_protocol::TemplatePlanDeriveParams {
                        project_id,
                        expected_project_revision,
                        template_id,
                        template_version,
                        display_name,
                        description,
                    })
                    .await?;
                if dry_run {
                    serde_json::to_value(plan)
                } else {
                    confirm_high_impact(yes)?;
                    let accepted = client
                        .template_apply_derive(alcomd_protocol::TemplateApplyPlanParams {
                            plan_id: plan.plan_id,
                            idempotency_key,
                        })
                        .await?;
                    if no_wait {
                        serde_json::to_value(accepted)
                    } else {
                        serde_json::to_value(
                            wait_for_operation(&mut client, accepted.operation_id).await?,
                        )
                    }
                }
            }
            TemplateCommand::Export {
                template_id,
                target_path,
                expected_revision,
            } => {
                let target_path =
                    absolute_path(target_path).map_err(alcomd_client::ClientError::Transport)?;
                serde_json::to_value(
                    client
                        .template_export(alcomd_protocol::TemplateExportParams {
                            template_id,
                            expected_revision,
                            target_path,
                        })
                        .await?,
                )
            }
            TemplateCommand::Favorite {
                template_id,
                favorite,
                expected_revision,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .template_set_favorite(alcomd_protocol::TemplateSetFavoriteParams {
                        template_id,
                        favorite,
                        expected_revision,
                        idempotency_key,
                    })
                    .await?,
            ),
            TemplateCommand::Remove {
                template_id,
                expected_revision,
                idempotency_key,
                yes,
            } => {
                confirm_high_impact(yes)?;
                serde_json::to_value(
                    client
                        .template_remove(alcomd_protocol::TemplateRemoveParams {
                            template_id,
                            expected_revision,
                            idempotency_key,
                        })
                        .await?,
                )
            }
        },
    }
    .map_err(|_| alcomd_client::ClientError::InvalidResponse)?;
    Ok(value)
}

fn print_result(as_json: bool, value: &serde_json::Value) {
    if as_json {
        println!("{value}");
    } else if let (Some(product), Some(version), Some(state), Some(rpc_version)) = (
        value.get("product").and_then(serde_json::Value::as_str),
        value
            .get("daemonVersion")
            .and_then(serde_json::Value::as_str),
        value.get("state").and_then(serde_json::Value::as_str),
        value.get("rpcVersion").and_then(serde_json::Value::as_u64),
    ) {
        println!("{product} daemon {version}: {state} (RPC v{rpc_version})");
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(value).expect("approved result DTO must serialize")
        );
    }
}

fn absolute_path(path: PathBuf) -> std::io::Result<String> {
    let path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()?.join(path)
    };
    path.to_str().map(str::to_owned).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "path encoding is unsupported",
        )
    })
}

fn confirm_high_impact(yes: bool) -> Result<(), alcomd_client::ClientError> {
    if yes {
        return Ok(());
    }
    use std::io::{IsTerminal, Write};
    if !std::io::stdin().is_terminal() || !std::io::stderr().is_terminal() {
        return Err(alcomd_client::ClientError::Transport(
            std::io::Error::other("confirmation_required: pass --yes in non-interactive use"),
        ));
    }
    eprint!("Apply this high-impact Template operation? [y/N] ");
    std::io::stderr()
        .flush()
        .map_err(alcomd_client::ClientError::Transport)?;
    let mut response = String::new();
    let read = std::io::stdin()
        .read_line(&mut response)
        .map_err(alcomd_client::ClientError::Transport)?;
    if read == 0 || !matches!(response.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        return Err(alcomd_client::ClientError::Transport(
            std::io::Error::other("confirmation_required"),
        ));
    }
    Ok(())
}

async fn wait_for_operation(
    client: &mut alcomd_client::AlcomdClient,
    operation_id: String,
) -> Result<alcomd_protocol::Operation, alcomd_client::ClientError> {
    loop {
        let operation = client.operation_get(operation_id.clone()).await?;
        if matches!(
            operation.state,
            alcomd_protocol::OperationState::Succeeded
                | alcomd_protocol::OperationState::Failed
                | alcomd_protocol::OperationState::Cancelled
        ) {
            return Ok(operation);
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

fn repository_source(
    source: String,
) -> Result<alcomd_protocol::RepositorySource, alcomd_client::ClientError> {
    if source.starts_with("http://") || source.starts_with("https://") {
        Ok(alcomd_protocol::RepositorySource::Remote { url: source })
    } else {
        let path =
            absolute_path(PathBuf::from(source)).map_err(alcomd_client::ClientError::Transport)?;
        Ok(alcomd_protocol::RepositorySource::Local { path })
    }
}

fn print_error(as_json: bool, error: &alcomd_client::ClientError) {
    if as_json {
        let code = match error {
            alcomd_client::ClientError::Remote(remote) => remote.code.as_str(),
            alcomd_client::ClientError::InvalidResponse => "invalid_response",
            alcomd_client::ClientError::StartTimeout => "daemon_start_timeout",
            alcomd_client::ClientError::Transport(_)
            | alcomd_client::ClientError::StartDaemon(_)
            | alcomd_client::ClientError::DaemonPathUnavailable => "daemon_unavailable",
        };
        eprintln!("{{\"error\":{{\"code\":{}}}}}", serde_json::json!(code));
    } else {
        eprintln!("error: {error}");
    }
}
