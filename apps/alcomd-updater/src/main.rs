use std::path::PathBuf;

use alcomd_updater::{Channel, verify_staged_payload};
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(version, about = "ALCOMD product update verification")]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Verify a signed bridge payload for this platform and channel.
    VerifyStaging {
        #[arg(long)]
        staging: PathBuf,
        #[arg(long, value_enum)]
        channel: Channel,
    },
}

fn main() -> anyhow::Result<()> {
    let Arguments { command } = Arguments::parse();
    let Command::VerifyStaging { staging, channel } = command;
    let verify = verify_staged_payload(&staging, channel)?;
    println!("Verified staged product {}", verify.version());
    Ok(())
}
