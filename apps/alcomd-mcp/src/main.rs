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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arguments = Arguments::parse();

    match arguments.command.unwrap_or(Command::Stdio) {
        Command::Stdio => {
            let (input, output) = rmcp::transport::stdio_pipes();
            let core = match std::env::var("ALCOMD_MCP_CLIENT_CREDENTIAL") {
                Ok(secret) => Some(
                    alcomd_mcp::connect_core(alcomd_client::ClientConfig::default(), secret)
                        .await?,
                ),
                Err(std::env::VarError::NotPresent) => None,
                Err(_) => anyhow::bail!("MCP startup credential is invalid"),
            };
            alcomd_mcp::serve_stdio_with_client(input, output, core).await?;
        }
        Command::Serve { port } => {
            let server =
                alcomd_mcp::HttpServer::bind(port, alcomd_client::ClientConfig::default()).await?;
            eprintln!("http://{}/mcp", server.local_addr()?);
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
