use anyhow::Context;
use clap::Parser;
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let arguments = Arguments::parse();
    let config = arguments
        .runtime_dir
        .map(alcomd_platform::IpcConfig::isolated)
        .unwrap_or_default();
    let data = arguments
        .data_dir
        .map(alcomd_platform::DataConfig::isolated)
        .unwrap_or_default();
    alcomd::serve_with_data_until(config, data, async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to wait for Ctrl+C");
    })
    .await
    .context("daemon stopped")?;
    Ok(())
}
