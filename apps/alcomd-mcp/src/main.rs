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
            alcomd_mcp::serve_stdio(input, output).await?;
        }
        Command::Serve { port } => {
            anyhow::bail!(
                "MCP HTTP transport is unavailable until Core client authentication is implemented; no listener was started (requested port {port})"
            );
        }
    }
    Ok(())
}
