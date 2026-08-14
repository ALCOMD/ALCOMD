use clap::{Parser, Subcommand};
use serde_json::json;

/// Command-line client for ALCOMD.
#[derive(Debug, Parser)]
#[command(name = "alcomd-cli", version, about)]
struct Arguments {
    /// Emit machine-readable JSON.
    #[arg(long, global = true)]
    json: bool,

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
    /// Report scaffold status without connecting to a daemon.
    Status,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arguments = Arguments::parse();

    let Command::System { command } = arguments.command;
    let SystemCommand::Status = command;
    print_status(arguments.json);

    Ok(())
}

fn print_status(as_json: bool) {
    let value = json!({
        "product": alcomd_protocol::PRODUCT_FAMILY,
        "component": "alcomd-cli",
        "version": env!("CARGO_PKG_VERSION"),
        "status": "scaffold",
        "daemonConnected": false,
        "nextMilestone": "M1"
    });

    if as_json {
        println!("{value}");
    } else {
        println!("ALCOMD CLI scaffold {}", env!("CARGO_PKG_VERSION"));
        println!("Daemon connection: not implemented (planned for M1)");
    }
}
