use clap::{Parser, Subcommand, ValueEnum};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug)]
enum CliError {
    Client(alcomd_client::ClientError),
    ConfirmationRequired,
    OperationFailed(Box<alcomd_protocol::Operation>),
    OperationDetached(String),
    LocalIo(std::io::Error),
}

impl From<alcomd_client::ClientError> for CliError {
    fn from(error: alcomd_client::ClientError) -> Self {
        Self::Client(error)
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Client(error) => error.fmt(formatter),
            Self::ConfirmationRequired => formatter.write_str("confirmation is required"),
            Self::OperationFailed(_) => formatter.write_str("operation did not succeed"),
            Self::OperationDetached(_) => formatter.write_str("operation follow was interrupted"),
            Self::LocalIo(_) => formatter.write_str("local CLI I/O failed"),
        }
    }
}

impl std::error::Error for CliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Client(error) => Some(error),
            Self::LocalIo(error) => Some(error),
            Self::ConfirmationRequired | Self::OperationFailed(_) | Self::OperationDetached(_) => {
                None
            }
        }
    }
}

/// Command-line client for ALCOMD.
#[derive(Debug, Parser)]
#[command(name = "alcomd-cli", version, about)]
struct Arguments {
    /// Emit machine-readable JSON.
    #[arg(long, global = true, conflicts_with = "ndjson")]
    json: bool,

    /// Emit newline-delimited machine-readable JSON.
    #[arg(long, global = true, conflicts_with = "json")]
    ndjson: bool,

    /// Suppress non-error human diagnostics and progress.
    #[arg(long, global = true)]
    quiet: bool,

    /// Confirm a high-impact operation without prompting.
    #[arg(long, global = true)]
    yes: bool,

    /// Create and return an immutable Plan without applying it.
    #[arg(long, global = true)]
    dry_run: bool,

    /// Return after an Operation is accepted instead of following it.
    #[arg(long, global = true)]
    no_wait: bool,

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
    /// Inspect, follow, or cooperatively cancel daemon Operations.
    #[command(visible_alias = "operations")]
    Operation {
        #[command(subcommand)]
        command: OperationCommand,
    },
    /// Inspect and manage only the ALCOMD project registry.
    #[command(visible_alias = "projects")]
    Project {
        #[command(subcommand)]
        command: ProjectCommand,
    },
    /// Inspect and manage normalized VPM repository metadata.
    #[command(visible_alias = "repo")]
    Repository {
        #[command(subcommand)]
        command: RepositoryCommand,
    },
    /// Plan and apply the minimal M4 VPM package transaction slice.
    #[command(visible_alias = "packages")]
    Package {
        #[command(subcommand)]
        command: PackageCommand,
    },
    /// Inspect and manage Unity Editor installations and project launches.
    Unity {
        #[command(subcommand)]
        command: UnityCommand,
    },
    /// Inspect and manage native ALCOMD Template bundles.
    #[command(visible_alias = "templates")]
    Template {
        #[command(subcommand)]
        command: TemplateCommand,
    },
    /// List, inspect, and create managed native Backup archives.
    Backup {
        #[command(subcommand)]
        command: BackupCommand,
    },
    /// Generate static shell completion without connecting to the daemon.
    Completion { shell: CompletionShell },
}

#[derive(Debug, Subcommand)]
enum SystemCommand {
    /// Query the running per-user daemon.
    Status,
}

#[derive(Debug, Subcommand)]
enum OperationCommand {
    List {
        #[arg(long)]
        cursor_created_at_ms: Option<u64>,
        #[arg(long, requires = "cursor_created_at_ms")]
        cursor_operation_id: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },
    #[command(visible_alias = "info")]
    Get {
        operation_id: String,
    },
    Follow {
        operation_id: String,
    },
    Cancel {
        operation_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
    Copy {
        project_id: String,
        target_parent: PathBuf,
        target_leaf: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        plan_idempotency_key: String,
        #[arg(long)]
        idempotency_key: String,
    },
    Create {
        #[arg(long)]
        template: String,
        target_parent: PathBuf,
        target_leaf: String,
        #[arg(long)]
        expected_template_revision: u64,
        #[arg(long)]
        idempotency_key: String,
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
    #[command(visible_alias = "info")]
    Get { project_id: String },
    #[command(visible_alias = "add")]
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
    #[command(visible_alias = "remove")]
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
    #[command(visible_alias = "info")]
    Get {
        repository_id: String,
    },
    Packages {
        repository_id: String,
        #[arg(long)]
        limit: Option<u32>,
    },
    #[command(visible_alias = "add")]
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
    #[command(visible_alias = "remove")]
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
    #[command(name = "install", visible_alias = "i", alias = "plan-install")]
    Install {
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
        #[arg(long)]
        idempotency_key: String,
    },
    #[command(name = "remove", visible_alias = "rm", alias = "plan-remove")]
    Remove {
        project_id: String,
        package_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
    #[command(name = "upgrade", alias = "plan-upgrade")]
    Upgrade {
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
        #[arg(long)]
        idempotency_key: String,
    },
    #[command(name = "downgrade", alias = "plan-downgrade")]
    Downgrade {
        project_id: String,
        package_id: String,
        version: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        repository_id: Option<String>,
        #[arg(long)]
        idempotency_key: String,
    },
    #[command(name = "resolve", alias = "plan-resolve")]
    Resolve {
        project_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        include_prerelease: bool,
        #[arg(long)]
        idempotency_key: String,
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
enum UnityCommand {
    List {
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },
    #[command(visible_alias = "info")]
    Get {
        installation_id: String,
    },
    Refresh {
        #[arg(long)]
        idempotency_key: String,
    },
    #[command(visible_alias = "add")]
    Register {
        executable_path: PathBuf,
        #[arg(long)]
        idempotency_key: String,
    },
    Remove {
        installation_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
    ProjectGet {
        project_id: String,
    },
    ProjectSetEditor {
        project_id: String,
        installation_id: String,
        #[arg(long, default_value_t = 0)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
    ProjectSetArgs {
        project_id: String,
        #[arg(long = "argument", allow_hyphen_values = true)]
        arguments: Vec<String>,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
    WriterState {
        project_id: String,
    },
    Launch {
        project_id: String,
        #[arg(long)]
        expected_project_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
    LaunchStatus {
        launch_id: String,
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
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum BackupCompression {
    Store,
    Fast,
    Maximum,
}

#[derive(Debug, Subcommand)]
enum BackupCommand {
    List {
        #[arg(long)]
        project_id: Option<String>,
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },
    #[command(visible_alias = "info")]
    Get { backup_id: String },
    Create {
        project_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long, value_enum, default_value = "fast")]
        compression: BackupCompression,
        #[arg(long)]
        exclude_vpm_packages: bool,
        #[arg(long)]
        idempotency_key: String,
    },
    Restore {
        backup_id: String,
        target_parent: PathBuf,
        target_leaf: String,
        #[arg(long)]
        idempotency_key: String,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CompletionShell {
    Bash,
    Zsh,
    #[value(name = "powershell")]
    PowerShell,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutputMode {
    Human,
    Json,
    Ndjson,
}

#[derive(Clone, Copy, Debug)]
struct ExecutionOptions {
    output_mode: OutputMode,
    yes: bool,
    dry_run: bool,
    no_wait: bool,
}

#[tokio::main]
async fn main() -> ExitCode {
    let arguments = Arguments::parse();
    let output_mode = if arguments.json {
        OutputMode::Json
    } else if arguments.ndjson {
        OutputMode::Ndjson
    } else {
        OutputMode::Human
    };
    let options = ExecutionOptions {
        output_mode,
        yes: arguments.yes,
        dry_run: arguments.dry_run,
        no_wait: arguments.no_wait,
    };
    let command_name = arguments.command.name();

    if let Command::Completion { shell } = &arguments.command {
        return write_completion(*shell);
    }

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

    match execute(config, arguments.command, options).await {
        Ok(value) => output_exit(print_result(output_mode, command_name, &value)),
        Err(CliError::OperationDetached(operation_id)) => {
            let result = serde_json::json!({
                "operationId": operation_id,
                "detached": true,
            });
            let _ = print_result(output_mode, command_name, &result);
            ExitCode::from(130)
        }
        Err(error) => output_error_exit(print_error(output_mode, &error)),
    }
}

async fn execute(
    config: alcomd_client::ClientConfig,
    command: Command,
    options: ExecutionOptions,
) -> Result<serde_json::Value, CliError> {
    let mut client = alcomd_client::AlcomdClient::connect(config).await?;
    let value = match command {
        Command::System {
            command: SystemCommand::Status,
        } => serde_json::to_value(client.system_status().await?),
        Command::Operation { command } => match command {
            OperationCommand::List {
                cursor_created_at_ms,
                cursor_operation_id,
                limit,
            } => {
                let cursor = match (cursor_created_at_ms, cursor_operation_id) {
                    (Some(created_at_ms), Some(operation_id)) => {
                        Some(alcomd_protocol::OperationsListCursor {
                            created_at_ms,
                            operation_id,
                        })
                    }
                    _ => None,
                };
                serde_json::to_value(client.operations_list(cursor, limit).await?)
            }
            OperationCommand::Get { operation_id } => {
                serde_json::to_value(client.operation_get(operation_id).await?)
            }
            OperationCommand::Follow { operation_id } => serde_json::to_value(
                wait_for_operation(&mut client, operation_id, options.output_mode).await?,
            ),
            OperationCommand::Cancel {
                operation_id,
                expected_revision,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .operation_cancel(operation_id, expected_revision, idempotency_key)
                    .await?,
            ),
        },
        Command::Project { command } => match command {
            ProjectCommand::Copy {
                project_id,
                target_parent,
                target_leaf,
                expected_revision,
                plan_idempotency_key,
                idempotency_key,
            } => {
                let target_parent =
                    absolute_path(target_parent).map_err(alcomd_client::ClientError::Transport)?;
                let plan = client
                    .project_plan_copy(alcomd_protocol::ProjectsPlanCopyParams {
                        source_project_id: project_id,
                        expected_revision,
                        target_parent_path: target_parent,
                        target_leaf,
                        idempotency_key: plan_idempotency_key,
                    })
                    .await?;
                if options.dry_run {
                    serde_json::to_value(plan)
                } else {
                    confirm_high_impact(options)?;
                    let accepted = client
                        .project_apply_copy(alcomd_protocol::ProjectsApplyCopyParams {
                            plan_id: plan.plan.plan_id,
                            expected_revision,
                            idempotency_key,
                        })
                        .await?;
                    if options.no_wait {
                        serde_json::to_value(accepted)
                    } else {
                        serde_json::to_value(
                            wait_for_operation(
                                &mut client,
                                accepted.operation_id,
                                options.output_mode,
                            )
                            .await?,
                        )
                    }
                }
            }
            ProjectCommand::Create {
                template,
                target_parent,
                target_leaf,
                expected_template_revision,
                idempotency_key,
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
                if options.dry_run {
                    serde_json::to_value(plan)
                } else {
                    confirm_high_impact(options)?;
                    let accepted = client
                        .template_apply_create_project(alcomd_protocol::TemplateApplyPlanParams {
                            plan_id: plan.plan_id,
                            idempotency_key,
                        })
                        .await?;
                    if options.no_wait {
                        serde_json::to_value(accepted)
                    } else {
                        serde_json::to_value(
                            wait_for_operation(
                                &mut client,
                                accepted.operation_id,
                                options.output_mode,
                            )
                            .await?,
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
            PackageCommand::Install {
                project_id,
                package_id,
                expected_revision,
                version_range,
                repository_id,
                include_prerelease,
                idempotency_key,
            } => {
                let plan = client
                    .package_plan_install(alcomd_protocol::PackagePlanInstallParams {
                        project_id,
                        expected_revision,
                        package_id,
                        version_range,
                        repository_id,
                        source: None,
                        include_prerelease,
                    })
                    .await?;
                Ok(apply_or_return_package_plan(
                    &mut client,
                    plan,
                    expected_revision,
                    idempotency_key,
                    options,
                )
                .await?)
            }
            PackageCommand::Remove {
                project_id,
                package_id,
                expected_revision,
                idempotency_key,
            } => {
                let plan = client
                    .package_plan_remove(alcomd_protocol::PackagePlanRemoveParams {
                        project_id,
                        expected_revision,
                        package_id,
                    })
                    .await?;
                Ok(apply_or_return_package_plan(
                    &mut client,
                    plan,
                    expected_revision,
                    idempotency_key,
                    options,
                )
                .await?)
            }
            PackageCommand::Upgrade {
                project_id,
                package_id,
                expected_revision,
                version_range,
                repository_id,
                include_prerelease,
                idempotency_key,
            } => {
                let plan = client
                    .package_plan_upgrade(alcomd_protocol::PackagePlanUpgradeParams {
                        project_id,
                        expected_revision,
                        package_id,
                        version_range,
                        repository_id,
                        source: None,
                        include_prerelease,
                    })
                    .await?;
                Ok(apply_or_return_package_plan(
                    &mut client,
                    plan,
                    expected_revision,
                    idempotency_key,
                    options,
                )
                .await?)
            }
            PackageCommand::Downgrade {
                project_id,
                package_id,
                version,
                expected_revision,
                repository_id,
                idempotency_key,
            } => {
                let plan = client
                    .package_plan_downgrade(alcomd_protocol::PackagePlanDowngradeParams {
                        project_id,
                        expected_revision,
                        package_id,
                        version,
                        repository_id,
                        source: None,
                    })
                    .await?;
                Ok(apply_or_return_package_plan(
                    &mut client,
                    plan,
                    expected_revision,
                    idempotency_key,
                    options,
                )
                .await?)
            }
            PackageCommand::Resolve {
                project_id,
                expected_revision,
                include_prerelease,
                idempotency_key,
            } => {
                let plan = client
                    .package_plan_resolve(alcomd_protocol::PackagePlanResolveParams {
                        project_id,
                        expected_revision,
                        include_prerelease,
                    })
                    .await?;
                Ok(apply_or_return_package_plan(
                    &mut client,
                    plan,
                    expected_revision,
                    idempotency_key,
                    options,
                )
                .await?)
            }
            PackageCommand::ApplyPlan {
                plan_id,
                expected_revision,
                idempotency_key,
            } => {
                confirm_high_impact(options)?;
                let accepted = client
                    .package_apply_plan(alcomd_protocol::PackageApplyPlanParams {
                        plan_id,
                        expected_revision,
                        idempotency_key,
                    })
                    .await?;
                if options.no_wait {
                    serde_json::to_value(accepted)
                } else {
                    serde_json::to_value(
                        wait_for_operation(&mut client, accepted.operation_id, options.output_mode)
                            .await?,
                    )
                }
            }
        },
        Command::Unity { command } => match command {
            UnityCommand::List { cursor, limit } => serde_json::to_value(
                client
                    .unity_installations_list(alcomd_protocol::UnityInstallationsListParams {
                        cursor,
                        limit,
                    })
                    .await?,
            ),
            UnityCommand::Get { installation_id } => {
                serde_json::to_value(client.unity_installation_get(installation_id).await?)
            }
            UnityCommand::Refresh { idempotency_key } => {
                serde_json::to_value(client.unity_installations_refresh(idempotency_key).await?)
            }
            UnityCommand::Register {
                executable_path,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .unity_installation_register(alcomd_protocol::UnityInstallationRegisterParams {
                        executable_path: absolute_path(executable_path)
                            .map_err(CliError::LocalIo)?,
                        idempotency_key,
                    })
                    .await?,
            ),
            UnityCommand::Remove {
                installation_id,
                expected_revision,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .unity_installation_remove(alcomd_protocol::UnityInstallationRemoveParams {
                        installation_id,
                        expected_revision,
                        idempotency_key,
                    })
                    .await?,
            ),
            UnityCommand::ProjectGet { project_id } => {
                serde_json::to_value(client.unity_project_editor_get(project_id).await?)
            }
            UnityCommand::ProjectSetEditor {
                project_id,
                installation_id,
                expected_revision,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .unity_project_editor_set(alcomd_protocol::ProjectEditorSetParams {
                        project_id,
                        installation_id,
                        arguments: Vec::new(),
                        expected_revision,
                        idempotency_key,
                    })
                    .await?,
            ),
            UnityCommand::ProjectSetArgs {
                project_id,
                arguments,
                expected_revision,
                idempotency_key,
            } => {
                let current = client.unity_project_editor_get(project_id.clone()).await?;
                serde_json::to_value(
                    client
                        .unity_project_editor_set(alcomd_protocol::ProjectEditorSetParams {
                            project_id,
                            installation_id: current.preference.installation_id,
                            arguments,
                            expected_revision,
                            idempotency_key,
                        })
                        .await?,
                )
            }
            UnityCommand::WriterState { project_id } => {
                serde_json::to_value(client.unity_writer_state(project_id).await?)
            }
            UnityCommand::Launch {
                project_id,
                expected_project_revision,
                idempotency_key,
            } => serde_json::to_value(
                client
                    .unity_launch(alcomd_protocol::UnityLaunchParams {
                        project_id,
                        expected_project_revision,
                        idempotency_key,
                    })
                    .await?,
            ),
            UnityCommand::LaunchStatus { launch_id } => {
                serde_json::to_value(client.unity_launch_status(launch_id).await?)
            }
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
                if options.dry_run {
                    serde_json::to_value(plan)
                } else {
                    confirm_high_impact(options)?;
                    let accepted = client
                        .template_apply_import(alcomd_protocol::TemplateApplyPlanParams {
                            plan_id: plan.plan_id,
                            idempotency_key,
                        })
                        .await?;
                    if options.no_wait {
                        serde_json::to_value(accepted)
                    } else {
                        serde_json::to_value(
                            wait_for_operation(
                                &mut client,
                                accepted.operation_id,
                                options.output_mode,
                            )
                            .await?,
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
                if options.dry_run {
                    serde_json::to_value(plan)
                } else {
                    confirm_high_impact(options)?;
                    let accepted = client
                        .template_apply_derive(alcomd_protocol::TemplateApplyPlanParams {
                            plan_id: plan.plan_id,
                            idempotency_key,
                        })
                        .await?;
                    if options.no_wait {
                        serde_json::to_value(accepted)
                    } else {
                        serde_json::to_value(
                            wait_for_operation(
                                &mut client,
                                accepted.operation_id,
                                options.output_mode,
                            )
                            .await?,
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
            } => {
                confirm_high_impact(options)?;
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
        Command::Backup { command } => match command {
            BackupCommand::List {
                project_id,
                cursor,
                limit,
            } => serde_json::to_value(
                client
                    .backups_list(alcomd_protocol::BackupsListParams {
                        project_id,
                        cursor,
                        limit,
                    })
                    .await?,
            ),
            BackupCommand::Get { backup_id } => {
                serde_json::to_value(client.backup_get(backup_id).await?)
            }
            BackupCommand::Create {
                project_id,
                expected_revision,
                compression,
                exclude_vpm_packages,
                idempotency_key,
            } => {
                let accepted = client
                    .backup_create(alcomd_protocol::BackupCreateParams {
                        project_id,
                        expected_revision,
                        compression_mode: match compression {
                            BackupCompression::Store => alcomd_protocol::BackupCompression::Store,
                            BackupCompression::Fast => alcomd_protocol::BackupCompression::Fast,
                            BackupCompression::Maximum => {
                                alcomd_protocol::BackupCompression::Maximum
                            }
                        },
                        exclude_vpm_packages,
                        idempotency_key,
                    })
                    .await?;
                if options.no_wait {
                    serde_json::to_value(accepted)
                } else {
                    serde_json::to_value(
                        wait_for_operation(&mut client, accepted.operation_id, options.output_mode)
                            .await?,
                    )
                }
            }
            BackupCommand::Restore {
                backup_id,
                target_parent,
                target_leaf,
                idempotency_key,
            } => {
                let plan = client
                    .backup_plan_restore(alcomd_protocol::BackupPlanRestoreParams {
                        backup_id,
                        target_parent: absolute_path(target_parent).map_err(CliError::LocalIo)?,
                        target_leaf,
                    })
                    .await?;
                if options.dry_run {
                    serde_json::to_value(plan)
                } else {
                    confirm_high_impact(options)?;
                    let accepted = client
                        .backup_apply_restore(alcomd_protocol::BackupApplyRestoreParams {
                            plan_id: plan.plan_id,
                            idempotency_key,
                        })
                        .await?;
                    if options.no_wait {
                        serde_json::to_value(accepted)
                    } else {
                        serde_json::to_value(
                            wait_for_operation(
                                &mut client,
                                accepted.operation_id,
                                options.output_mode,
                            )
                            .await?,
                        )
                    }
                }
            }
        },
        Command::Completion { .. } => unreachable!("completion is handled before RPC setup"),
    }
    .map_err(|_| CliError::Client(alcomd_client::ClientError::InvalidResponse))?;
    Ok(value)
}

impl Command {
    fn name(&self) -> &'static str {
        match self {
            Self::System { .. } => "system status",
            Self::Operation { command } => match command {
                OperationCommand::List { .. } => "operation list",
                OperationCommand::Get { .. } => "operation get",
                OperationCommand::Follow { .. } => "operation follow",
                OperationCommand::Cancel { .. } => "operation cancel",
            },
            Self::Project { command } => match command {
                ProjectCommand::Copy { .. } => "project copy",
                ProjectCommand::Create { .. } => "project create",
                ProjectCommand::Inspect { .. } => "project inspect",
                ProjectCommand::List { .. } => "project list",
                ProjectCommand::Get { .. } => "project get",
                ProjectCommand::Register { .. } => "project register",
                ProjectCommand::Refresh { .. } => "project refresh",
                ProjectCommand::Unregister { .. } => "project unregister",
            },
            Self::Repository { command } => match command {
                RepositoryCommand::Inspect { .. } => "repository inspect",
                RepositoryCommand::List { .. } => "repository list",
                RepositoryCommand::Get { .. } => "repository get",
                RepositoryCommand::Packages { .. } => "repository packages",
                RepositoryCommand::Register { .. } => "repository register",
                RepositoryCommand::Refresh { .. } => "repository refresh",
                RepositoryCommand::Unregister { .. } => "repository unregister",
            },
            Self::Package { command } => match command {
                PackageCommand::Install { .. } => "package install",
                PackageCommand::Remove { .. } => "package remove",
                PackageCommand::Upgrade { .. } => "package upgrade",
                PackageCommand::Downgrade { .. } => "package downgrade",
                PackageCommand::Resolve { .. } => "package resolve",
                PackageCommand::ApplyPlan { .. } => "package apply-plan",
            },
            Self::Unity { command } => match command {
                UnityCommand::List { .. } => "unity list",
                UnityCommand::Get { .. } => "unity get",
                UnityCommand::Refresh { .. } => "unity refresh",
                UnityCommand::Register { .. } => "unity register",
                UnityCommand::Remove { .. } => "unity remove",
                UnityCommand::ProjectGet { .. } => "unity project-get",
                UnityCommand::ProjectSetEditor { .. } => "unity project-set-editor",
                UnityCommand::ProjectSetArgs { .. } => "unity project-set-args",
                UnityCommand::WriterState { .. } => "unity writer-state",
                UnityCommand::Launch { .. } => "unity launch",
                UnityCommand::LaunchStatus { .. } => "unity launch-status",
            },
            Self::Template { command } => match command {
                TemplateCommand::List { .. } => "template list",
                TemplateCommand::Get { .. } => "template get",
                TemplateCommand::Inspect { .. } => "template inspect",
                TemplateCommand::Import { .. } => "template import",
                TemplateCommand::Derive { .. } => "template derive",
                TemplateCommand::Export { .. } => "template export",
                TemplateCommand::Favorite { .. } => "template favorite",
                TemplateCommand::Remove { .. } => "template remove",
            },
            Self::Backup { command } => match command {
                BackupCommand::List { .. } => "backup list",
                BackupCommand::Get { .. } => "backup get",
                BackupCommand::Create { .. } => "backup create",
                BackupCommand::Restore { .. } => "backup restore",
            },
            Self::Completion { .. } => "completion",
        }
    }
}

async fn apply_or_return_package_plan(
    client: &mut alcomd_client::AlcomdClient,
    plan: alcomd_protocol::PackagePlan,
    expected_revision: u64,
    idempotency_key: String,
    options: ExecutionOptions,
) -> Result<serde_json::Value, CliError> {
    if options.dry_run {
        return serde_json::to_value(plan)
            .map_err(|_| CliError::Client(alcomd_client::ClientError::InvalidResponse));
    }
    confirm_high_impact(options)?;
    let accepted = client
        .package_apply_plan(alcomd_protocol::PackageApplyPlanParams {
            plan_id: plan.plan_id,
            expected_revision,
            idempotency_key,
        })
        .await?;
    if options.no_wait {
        serde_json::to_value(accepted)
    } else {
        serde_json::to_value(
            wait_for_operation(client, accepted.operation_id, options.output_mode).await?,
        )
    }
    .map_err(|_| CliError::Client(alcomd_client::ClientError::InvalidResponse))
}

fn print_result(
    output_mode: OutputMode,
    command: &str,
    value: &serde_json::Value,
) -> io::Result<()> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    if matches!(output_mode, OutputMode::Json | OutputMode::Ndjson) {
        let document = serde_json::json!({
            "type": "result",
            "command": command,
            "result": value,
        });
        writeln!(output, "{document}")
    } else if let (Some(product), Some(version), Some(state), Some(rpc_version)) = (
        value.get("product").and_then(serde_json::Value::as_str),
        value
            .get("daemonVersion")
            .and_then(serde_json::Value::as_str),
        value.get("state").and_then(serde_json::Value::as_str),
        value.get("rpcVersion").and_then(serde_json::Value::as_u64),
    ) {
        writeln!(
            output,
            "{product} daemon {version}: {state} (RPC v{rpc_version})"
        )
    } else {
        writeln!(
            output,
            "{}",
            serde_json::to_string_pretty(value).expect("approved result DTO must serialize")
        )
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

fn confirm_high_impact(options: ExecutionOptions) -> Result<(), CliError> {
    if options.yes {
        return Ok(());
    }
    use std::io::IsTerminal;
    if options.output_mode != OutputMode::Human
        || !std::io::stdin().is_terminal()
        || !std::io::stderr().is_terminal()
    {
        return Err(CliError::ConfirmationRequired);
    }
    eprint!("Apply this high-impact operation? [y/N] ");
    std::io::stderr().flush().map_err(CliError::LocalIo)?;
    let mut response = String::new();
    let read = std::io::stdin()
        .read_line(&mut response)
        .map_err(CliError::LocalIo)?;
    if read == 0 || !matches!(response.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        return Err(CliError::ConfirmationRequired);
    }
    Ok(())
}

async fn wait_for_operation(
    client: &mut alcomd_client::AlcomdClient,
    operation_id: String,
    output_mode: OutputMode,
) -> Result<alcomd_protocol::Operation, CliError> {
    let mut emitted_revision = None;
    loop {
        let operation = tokio::select! {
            operation = client.operation_get(operation_id.clone()) => operation?,
            interrupted = tokio::signal::ctrl_c() => {
                interrupted.map_err(CliError::LocalIo)?;
                return Err(CliError::OperationDetached(operation_id));
            }
        };
        if output_mode == OutputMode::Ndjson && emitted_revision != Some(operation.revision) {
            let stdout = io::stdout();
            let mut output = stdout.lock();
            writeln!(
                output,
                "{}",
                serde_json::json!({
                    "type": "operation",
                    "operationId": operation.operation_id,
                    "operation": operation,
                })
            )
            .map_err(CliError::LocalIo)?;
            if let Some(progress) = operation.progress {
                writeln!(
                    output,
                    "{}",
                    serde_json::json!({
                        "type": "progress",
                        "operationId": operation.operation_id,
                        "progress": progress,
                    })
                )
                .map_err(CliError::LocalIo)?;
            }
            emitted_revision = Some(operation.revision);
        }
        match operation.state {
            alcomd_protocol::OperationState::Succeeded => return Ok(operation),
            alcomd_protocol::OperationState::Failed
            | alcomd_protocol::OperationState::Cancelled => {
                return Err(CliError::OperationFailed(Box::new(operation)));
            }
            _ => {}
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

fn print_error(output_mode: OutputMode, error: &CliError) -> io::Result<()> {
    if output_mode == OutputMode::Ndjson
        && let CliError::OperationFailed(operation) = error
    {
        let stdout = io::stdout();
        let mut output = stdout.lock();
        let terminal_error = alcomd_protocol::RpcError {
            code: operation
                .error_code
                .clone()
                .unwrap_or_else(|| "operation_cancelled".to_owned()),
            message: "The operation did not succeed.".to_owned(),
            diagnostic_id: operation.diagnostic_id.clone(),
            data: None,
        };
        let document = serde_json::json!({
            "type": "error",
            "operationId": operation.operation_id,
            "error": terminal_error,
        });
        return writeln!(output, "{document}");
    }
    let stderr = io::stderr();
    let mut output = stderr.lock();
    if matches!(output_mode, OutputMode::Json | OutputMode::Ndjson) {
        let (code, message) = match error {
            CliError::ConfirmationRequired => ("confirmation_required", "confirmation is required"),
            CliError::OperationFailed(operation) => (
                operation
                    .error_code
                    .as_deref()
                    .unwrap_or("operation_cancelled"),
                "the operation did not succeed",
            ),
            CliError::OperationDetached(_) => {
                ("operation_detached", "operation follow was interrupted")
            }
            CliError::LocalIo(_) => ("local_io_error", "local CLI I/O failed"),
            CliError::Client(alcomd_client::ClientError::Remote(remote)) => {
                let document = serde_json::json!({
                    "type": "error",
                    "error": remote,
                });
                return writeln!(output, "{document}");
            }
            CliError::Client(alcomd_client::ClientError::InvalidResponse) => (
                "invalid_response",
                "the daemon returned an invalid RPC response",
            ),
            CliError::Client(alcomd_client::ClientError::StartTimeout) => (
                "daemon_start_timeout",
                "the daemon did not become ready within five seconds",
            ),
            CliError::Client(
                alcomd_client::ClientError::Transport(_)
                | alcomd_client::ClientError::StartDaemon(_)
                | alcomd_client::ClientError::DaemonPathUnavailable,
            ) => ("daemon_unavailable", "the daemon is unavailable"),
        };
        let document = serde_json::json!({
            "type": "error",
            "error": {
                "code": code,
                "message": message,
            },
        });
        writeln!(output, "{document}")
    } else {
        writeln!(output, "error: {error}")
    }
}

fn output_exit(result: io::Result<()>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

fn output_error_exit(result: io::Result<()>) -> ExitCode {
    match result {
        Ok(()) | Err(_) => ExitCode::FAILURE,
    }
}

fn write_completion(shell: CompletionShell) -> ExitCode {
    const COMMANDS: &str =
        "system operation project repository package unity template backup completion";
    let script = match shell {
        CompletionShell::Bash => format!("complete -W '{COMMANDS}' alcomd-cli\n"),
        CompletionShell::Zsh => {
            format!("#compdef alcomd-cli\n_arguments '1:command:({COMMANDS})'\n")
        }
        CompletionShell::PowerShell => format!(
            "Register-ArgumentCompleter -Native -CommandName alcomd-cli -ScriptBlock {{ param($wordToComplete) '{COMMANDS}'.Split(' ') | Where-Object {{ $_ -like \"$wordToComplete*\" }} }}\n"
        ),
    };
    output_exit(io::stdout().lock().write_all(script.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_commands_publish_exact_surface_and_info_alias() {
        let list = Arguments::try_parse_from([
            "alcomd-cli",
            "backup",
            "list",
            "--project-id",
            "00000000-0000-4000-8000-000000000001",
            "--limit",
            "10",
        ])
        .expect("backup list");
        assert!(matches!(
            list.command,
            Command::Backup {
                command: BackupCommand::List {
                    limit: Some(10),
                    ..
                }
            }
        ));
        let info = Arguments::try_parse_from([
            "alcomd-cli",
            "backup",
            "info",
            "00000000-0000-4000-8000-000000000002",
        ])
        .expect("info alias");
        assert!(matches!(
            info.command,
            Command::Backup {
                command: BackupCommand::Get { .. }
            }
        ));
        let create = Arguments::try_parse_from([
            "alcomd-cli",
            "backup",
            "create",
            "00000000-0000-4000-8000-000000000001",
            "--expected-revision",
            "1",
            "--compression",
            "maximum",
            "--exclude-vpm-packages",
            "--idempotency-key",
            "fixture",
            "--no-wait",
        ])
        .expect("backup create");
        assert!(matches!(
            create.command,
            Command::Backup {
                command: BackupCommand::Create {
                    compression: BackupCompression::Maximum,
                    exclude_vpm_packages: true,
                    ..
                }
            }
        ));
        assert!(create.no_wait);
        let restore = Arguments::try_parse_from([
            "alcomd-cli",
            "backup",
            "restore",
            "00000000-0000-4000-8000-000000000002",
            ".",
            "RestoredProject",
            "--idempotency-key",
            "restore-fixture",
            "--yes",
            "--no-wait",
        ])
        .expect("backup restore");
        assert!(matches!(
            restore.command,
            Command::Backup {
                command: BackupCommand::Restore { .. }
            }
        ));
        assert!(restore.yes);
        assert!(!restore.dry_run);
        assert!(restore.no_wait);
        assert!(
            Arguments::try_parse_from(["alcomd-cli", "--json", "--ndjson", "system", "status"])
                .is_err()
        );
    }

    #[test]
    fn core_catalog_groups_and_aliases_are_reachable() {
        assert!(Arguments::try_parse_from(["alcomd-cli", "operations", "list"]).is_ok());
        assert!(
            Arguments::try_parse_from([
                "alcomd-cli",
                "package",
                "i",
                "project",
                "package",
                "--expected-revision",
                "1",
                "--idempotency-key",
                "key",
                "--dry-run",
            ])
            .is_ok()
        );
        assert!(Arguments::try_parse_from(["alcomd-cli", "unity", "list"]).is_ok());
        assert!(Arguments::try_parse_from(["alcomd-cli", "completion", "powershell"]).is_ok());
    }
}
