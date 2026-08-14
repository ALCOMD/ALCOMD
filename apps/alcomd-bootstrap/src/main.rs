use clap::{Parser, Subcommand};

/// External installer, updater, migration, and cleanup coordinator.
#[derive(Debug, Parser)]
#[command(name = "alcomd-bootstrap", version, about)]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Validate that the scaffold bootstrap binary can run.
    Doctor,
}

fn main() {
    let arguments = Arguments::parse();
    let Command::Doctor = arguments.command;
    println!("alcomd-bootstrap scaffold is available; no system changes were made");
}
