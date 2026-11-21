use std::path::PathBuf;

use alcomd_bootstrap::{
    HealthReport, InstallScope, MigrationOutcome, MigrationRequest, migrate_v3, rollback_v3,
};
use alcomd_updater::Channel;
use clap::{Args, Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(version, about = "ALCOMD product migration coordinator")]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Migrate published v3 state or continue an interrupted migration.
    MigrateV3(Box<MigrateArguments>),
    /// Discard uncommitted outputs of an existing migration.
    RollbackV3 {
        #[arg(long)]
        work_directory: PathBuf,
    },
}

#[derive(Debug, Args)]
struct MigrateArguments {
    #[arg(long)]
    source_root: PathBuf,
    #[arg(long)]
    recovery_executable: PathBuf,
    #[arg(long)]
    work_directory: PathBuf,
    #[arg(long)]
    destination: PathBuf,
    #[arg(long)]
    staged_payload: PathBuf,
    #[arg(long, value_enum)]
    channel: Channel,
    #[arg(long, value_enum)]
    install_scope: InstallScope,
    #[arg(long, requires = "health_signature")]
    health_report: Option<PathBuf>,
    #[arg(long, requires = "health_report")]
    health_signature: Option<PathBuf>,
    /// Store this user's repository authentication in this user's credential vault.
    #[arg(long)]
    transfer_credentials: bool,
    /// Consent to migrate the enabled Discord integration for this exact package digest.
    #[arg(long)]
    discord_consent_digest: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let arguments = match Arguments::parse().command {
        Command::MigrateV3(arguments) => arguments,
        Command::RollbackV3 { work_directory } => {
            rollback_v3(&work_directory)?;
            println!("Uncommitted migration staging discarded");
            return Ok(());
        }
    };
    let MigrateArguments {
        source_root,
        recovery_executable,
        work_directory,
        destination,
        staged_payload,
        channel,
        install_scope,
        health_report,
        health_signature,
        transfer_credentials,
        discord_consent_digest,
    } = *arguments;
    let outcome = migrate_v3(&MigrationRequest {
        source_root,
        recovery_executable,
        work_directory,
        destination,
        staged_payload,
        channel,
        install_scope,
        transfer_credentials,
        discord_consent_digest,
        health_report: health_report
            .zip(health_signature)
            .map(|(report, signature)| HealthReport { report, signature }),
    })?;
    match outcome {
        MigrationOutcome::AwaitingHealth(challenge) => {
            println!("{}", serde_json::to_string(&challenge)?)
        }
        MigrationOutcome::Committed => println!("Native v4 state committed"),
        MigrationOutcome::AlreadyCommitted => println!("Native v4 state was already committed"),
    }
    Ok(())
}
