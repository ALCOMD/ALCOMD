use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

/// ALCOMD per-user core daemon.
#[derive(Debug, Parser)]
#[command(name = "alcomd", version, about)]
struct Arguments {
    /// Override the private Unix runtime directory for isolated testing.
    #[arg(long, hide = true)]
    runtime_dir: Option<PathBuf>,
    /// Override the state data directory for isolated testing.
    #[arg(long, hide = true)]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[command(hide = true)]
    ImportNativeState(NativeStage),
    #[command(hide = true)]
    CheckNativeState(NativeStage),
    #[cfg(any(windows, target_os = "linux"))]
    #[command(hide = true)]
    CheckInstallationData,
    #[cfg(any(windows, target_os = "linux"))]
    #[command(hide = true)]
    RegisterInstallation {
        #[arg(long)]
        serve: bool,
    },
}

#[derive(Debug, Args)]
struct NativeStage {
    #[arg(long)]
    bundle: PathBuf,
    #[arg(long)]
    data_dir: PathBuf,
    #[arg(long, requires_all = ["discord_sha256", "publish_data_dir"])]
    discord_package: Option<PathBuf>,
    #[arg(long, requires = "discord_package")]
    discord_sha256: Option<String>,
    #[arg(long, requires = "discord_package")]
    publish_data_dir: Option<PathBuf>,
    #[arg(long, requires = "discord_package")]
    discord_consent_digest: Option<String>,
}

impl NativeStage {
    fn discord_options(&self) -> Option<alcomd::DiscordImportOptions<'_>> {
        Some(alcomd::DiscordImportOptions {
            package: self.discord_package.as_deref()?,
            package_sha256: self.discord_sha256.as_deref()?,
            publish_data_dir: self.publish_data_dir.as_deref()?,
            consent_digest: self.discord_consent_digest.as_deref(),
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arguments = Arguments::parse();
    #[cfg(windows)]
    let _installation = alcomd_platform::hold_windows_installation(&std::env::current_exe()?)
        .context("installed product is unavailable or undergoing maintenance")?;
    if let Some(command) = arguments.command {
        let receipt = match command {
            Command::ImportNativeState(stage) => {
                alcomd::import_native_state(&stage.bundle, &stage.data_dir, stage.discord_options())
                    .await?
            }
            Command::CheckNativeState(stage) => {
                alcomd::check_native_state(&stage.bundle, &stage.data_dir, stage.discord_options())
                    .await?
            }
            #[cfg(any(windows, target_os = "linux"))]
            Command::CheckInstallationData => {
                alcomd::check_installation_data(&alcomd_platform::DataConfig::default())?;
                return Ok(());
            }
            #[cfg(any(windows, target_os = "linux"))]
            Command::RegisterInstallation { serve } => {
                anyhow::ensure!(
                    arguments.runtime_dir.is_none(),
                    "invalid installation endpoint"
                );
                let data = arguments
                    .data_dir
                    .map(alcomd_platform::DataConfig::isolated)
                    .unwrap_or_default();
                alcomd::register_installation(data, serve.then(shutdown_signal)).await?;
                return Ok(());
            }
        };
        println!("{}", serde_json::to_string(&receipt)?);
        return Ok(());
    }
    let config = arguments
        .runtime_dir
        .map(alcomd_platform::IpcConfig::isolated)
        .unwrap_or_default();
    let data = arguments
        .data_dir
        .map(alcomd_platform::DataConfig::isolated)
        .unwrap_or_default();
    alcomd::serve_with_data_until(config, data, shutdown_signal())
        .await
        .context("daemon stopped")?;
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to wait for Ctrl+C");
}
