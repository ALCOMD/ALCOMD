use clap::{Parser, Subcommand};

/// MCP protocol adapter for ALCOMD.
#[derive(Debug, Parser)]
#[command(name = "alcomd-mcp", version, about)]
struct Arguments {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run over standard input and output.
    Stdio,
    /// Run the stateless Streamable HTTP adapter.
    Serve {
        /// Loopback port. Zero means dynamically assigned.
        #[arg(long, default_value_t = 0)]
        port: u16,
    },
    /// Run the explicit HTTPS OAuth profile. Trust the supplied certificate in each client.
    ServeOauth {
        #[arg(long, value_parser = clap::value_parser!(u16).range(1..))]
        port: u16,
        #[arg(long)]
        tls_cert: std::path::PathBuf,
        #[arg(long)]
        tls_key: std::path::PathBuf,
        /// Connect only to an already running Core (required for isolated manual runs).
        #[arg(long)]
        no_start_daemon: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arguments = Arguments::parse();

    match arguments.command.unwrap_or(Command::Stdio) {
        Command::Stdio => {
            let (input, output) = rmcp::transport::stdio_pipes();
            let (core, configuration) = match std::env::var("ALCOMD_MCP_CLIENT_CREDENTIAL") {
                Ok(secret) => {
                    let config =
                        alcomd_client::ClientConfig::default().with_mcp_credential(secret.clone());
                    let core =
                        alcomd_mcp::connect_core(alcomd_client::ClientConfig::default(), secret)
                            .await?;
                    (Some(core), Some(config))
                }
                Err(std::env::VarError::NotPresent) => (None, None),
                Err(_) => anyhow::bail!("MCP startup credential is invalid"),
            };
            alcomd_mcp::serve_stdio_with_client(input, output, core, configuration).await?;
        }
        Command::Serve { port } => {
            let server =
                alcomd_mcp::HttpServer::bind(port, alcomd_client::ClientConfig::default()).await?;
            eprintln!("http://{}/mcp", server.local_addr()?);
            server.run_until(shutdown()).await?;
        }
        Command::ServeOauth {
            port,
            tls_cert,
            tls_key,
            no_start_daemon,
        } => {
            let config = if no_start_daemon {
                alcomd_client::ClientConfig::default().without_daemon_start()
            } else {
                alcomd_client::ClientConfig::default()
            };
            let server =
                alcomd_mcp::HttpServer::bind_oauth(port, config, tls_cert, tls_key).await?;
            eprintln!("https://localhost:{port}/mcp");
            server.run_until(shutdown()).await?;
        }
    }
    Ok(())
}

async fn shutdown() {
    #[cfg(unix)]
    {
        if let Ok(mut terminate) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}
