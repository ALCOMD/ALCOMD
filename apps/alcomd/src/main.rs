use anyhow::Context;
use clap::Parser;
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::EnvFilter;

/// ALCOMD per-user core daemon.
#[derive(Debug, Parser)]
#[command(name = "alcomd", version, about)]
struct Arguments {
    /// Override the private Unix runtime directory for isolated testing.
    #[arg(long, hide = true)]
    runtime_dir: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let arguments = Arguments::parse();
    let config = arguments
        .runtime_dir
        .map(alcomd_platform::IpcConfig::isolated)
        .unwrap_or_default();
    let endpoint = alcomd_platform::endpoint_display(&config)
        .context("failed to resolve the per-user RPC endpoint")?;
    info!(
        product = alcomd_protocol::PRODUCT_FAMILY,
        rpc_version = alcomd_protocol::RPC_VERSION,
        endpoint,
        "starting per-user daemon"
    );
    alcomd::serve_until(config, async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to wait for Ctrl+C");
    })
    .await
    .context("daemon stopped")?;
    info!("shutdown requested");
    Ok(())
}

fn init_tracing() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("alcomd=info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
