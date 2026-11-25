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
    #[cfg(windows)]
    /// Initialize the current user's Core and publish its installed runtime registration.
    RegisterInstalledCore(InstallCoreArguments),
    #[cfg(windows)]
    /// Remove the current user's registration when it belongs to this installed runtime.
    UnregisterInstalledCore(InstallCoreArguments),
    #[cfg(windows)]
    #[command(hide = true)]
    VerifyInstalledUninstall {
        #[arg(long)]
        source_sha: String,
    },
    #[cfg(windows)]
    /// Prepare a verified update for a current-user Windows installation.
    StageInstalledUpdate(UpdateArguments),
    #[cfg(windows)]
    /// Install a verified update in the original Windows installation scope.
    ApplyInstalledUpdate(UpdateArguments),
    #[cfg(windows)]
    /// Recover an interrupted installed Windows update.
    ResumeInstalledUpdate,
    #[cfg(windows)]
    #[command(hide = true)]
    ApplyMachineUpdateWorker {
        #[arg(long)]
        transfer: String,
        #[arg(long)]
        requester_pid: u32,
        #[arg(long, value_enum)]
        channel: Channel,
    },
    #[cfg(windows)]
    #[command(hide = true)]
    ResumeMachineUpdateWorker,
    #[cfg(any(windows, target_os = "linux"))]
    #[command(hide = true)]
    StartInstalledCore,
    #[cfg(target_os = "linux")]
    #[command(hide = true)]
    StartPortable {
        #[arg(last = true)]
        arguments: Vec<std::ffi::OsString>,
    },
    #[cfg(target_os = "linux")]
    #[command(hide = true)]
    ApplyPortableUpdate {
        #[command(flatten)]
        update: UpdateArguments,
        #[arg(long)]
        desktop: bool,
    },
    #[cfg(target_os = "linux")]
    #[command(hide = true)]
    ResumePortableUpdate {
        #[arg(long)]
        desktop: bool,
    },
}

#[cfg(windows)]
#[derive(Debug, Args)]
struct InstallCoreArguments {
    #[arg(long)]
    source_sha: String,
    #[arg(long, hide = true)]
    data_dir: Option<PathBuf>,
}

#[cfg(any(windows, target_os = "linux"))]
#[derive(Debug, Args)]
struct UpdateArguments {
    #[arg(long)]
    envelope: PathBuf,
    #[arg(long)]
    asset: PathBuf,
    #[arg(long)]
    replay_directory: PathBuf,
    #[arg(long, value_enum)]
    channel: Channel,
}

#[cfg(any(windows, target_os = "linux"))]
impl UpdateArguments {
    fn absolute_paths(&self) -> std::io::Result<[PathBuf; 3]> {
        Ok([
            std::path::absolute(&self.envelope)?,
            std::path::absolute(&self.asset)?,
            std::path::absolute(&self.replay_directory)?,
        ])
    }
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
        #[cfg(windows)]
        Command::RegisterInstalledCore(arguments) => {
            return alcomd_bootstrap::register_installed_core(
                arguments.source_sha,
                arguments.data_dir.as_deref(),
            );
        }
        #[cfg(windows)]
        Command::UnregisterInstalledCore(arguments) => {
            return alcomd_bootstrap::unregister_installed_core(
                arguments.source_sha,
                arguments.data_dir.as_deref(),
            );
        }
        #[cfg(windows)]
        Command::VerifyInstalledUninstall { source_sha } => {
            return alcomd_bootstrap::verify_installed_machine_uninstall(source_sha);
        }
        #[cfg(windows)]
        Command::ApplyMachineUpdateWorker {
            transfer,
            requester_pid,
            channel,
        } => {
            return alcomd_bootstrap::apply_machine_update_worker(
                &transfer,
                requester_pid,
                channel,
            );
        }
        #[cfg(windows)]
        Command::ResumeMachineUpdateWorker => {
            return alcomd_bootstrap::resume_machine_update_worker();
        }
        #[cfg(windows)]
        Command::StageInstalledUpdate(arguments) => {
            let [envelope, asset, replay_directory] = arguments.absolute_paths()?;
            let staged = alcomd_bootstrap::stage_installed_update(
                &envelope,
                &asset,
                &replay_directory,
                arguments.channel,
            )?;
            println!("Verified update staged at {}", staged.display());
            return Ok(());
        }
        #[cfg(windows)]
        Command::ApplyInstalledUpdate(arguments) => {
            let [envelope, asset, replay_directory] = arguments.absolute_paths()?;
            alcomd_bootstrap::apply_installed_update(
                &envelope,
                &asset,
                &replay_directory,
                arguments.channel,
            )?;
            println!("Windows product update applied");
            return Ok(());
        }
        #[cfg(windows)]
        Command::ResumeInstalledUpdate => {
            alcomd_bootstrap::resume_installed_update()?;
            println!("Windows product update resumed");
            return Ok(());
        }
        #[cfg(any(windows, target_os = "linux"))]
        Command::StartInstalledCore => return alcomd_bootstrap::start_installed_core(),
        #[cfg(target_os = "linux")]
        Command::StartPortable { arguments } => {
            return alcomd_bootstrap::start_portable(&arguments);
        }
        #[cfg(target_os = "linux")]
        Command::ApplyPortableUpdate { update, desktop } => {
            let [envelope, asset, replay_directory] = update.absolute_paths()?;
            return alcomd_bootstrap::apply_portable_update(
                &envelope,
                &asset,
                &replay_directory,
                update.channel,
                desktop,
            );
        }
        #[cfg(target_os = "linux")]
        Command::ResumePortableUpdate { desktop } => {
            return alcomd_bootstrap::resume_portable_update(desktop);
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
