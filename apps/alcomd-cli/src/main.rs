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
}

#[derive(Debug, Subcommand)]
enum SystemCommand {
    /// Query the running per-user daemon.
    Status,
}

#[derive(Debug, Subcommand)]
enum ProjectCommand {
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
