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
}

#[derive(Debug, Subcommand)]
enum SystemCommand {
    /// Query the running per-user daemon.
    Status,
}

#[tokio::main]
async fn main() -> ExitCode {
    let arguments = Arguments::parse();

    let Command::System { command } = arguments.command;
    let SystemCommand::Status = command;
    let mut config = alcomd_client::ClientConfig::default();
    if arguments.no_start_daemon {
        config = config.without_daemon_start();
    }
    if let Some(path) = arguments.runtime_dir {
        config = config.with_runtime_directory(path);
    }
    if let Some(path) = arguments.daemon_path {
        config = config.with_daemon_path(path);
    }

    match query_status(config).await {
        Ok(status) => {
            print_status(arguments.json, &status);
            ExitCode::SUCCESS
        }
        Err(error) => {
            print_error(arguments.json, &error);
            ExitCode::FAILURE
        }
    }
}

async fn query_status(
    config: alcomd_client::ClientConfig,
) -> Result<alcomd_protocol::SystemStatusResult, alcomd_client::ClientError> {
    let mut client = alcomd_client::AlcomdClient::connect(config).await?;
    client.system_status().await
}

fn print_status(as_json: bool, status: &alcomd_protocol::SystemStatusResult) {
    if as_json {
        println!(
            "{}",
            serde_json::to_string(status).expect("approved status DTO must serialize")
        );
    } else {
        println!(
            "{} daemon {}: {} (RPC v{})",
            status.product, status.daemon_version, status.state, status.rpc_version
        );
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
