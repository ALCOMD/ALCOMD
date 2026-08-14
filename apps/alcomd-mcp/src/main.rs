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

fn main() {
    let arguments = Arguments::parse();

    match arguments.command.unwrap_or(Command::Stdio) {
        Command::Stdio => {
            eprintln!(
                "alcomd-mcp scaffold: MCP 2026-07-28 transport is planned for M8; no protocol data was written"
            );
        }
        Command::Serve { port } => {
            eprintln!(
                "alcomd-mcp scaffold: HTTP transport is not implemented; requested port {port}"
            );
        }
    }
}
