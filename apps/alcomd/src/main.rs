use anyhow::Context;
use clap::Parser;
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

mod log_capture;

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
    let diagnostic_log = alcomd_application::DiagnosticLog::new();
    init_tracing(diagnostic_log.clone());
    let arguments = Arguments::parse();
    let config = arguments
        .runtime_dir
        .map(alcomd_platform::IpcConfig::isolated)
        .unwrap_or_default();
    let data = arguments
        .data_dir
        .map(alcomd_platform::DataConfig::isolated)
        .unwrap_or_default();
    let endpoint = alcomd_platform::endpoint_display(&config)
        .context("failed to resolve the per-user RPC endpoint")?;
    info!(
        product = alcomd_protocol::PRODUCT_FAMILY,
        rpc_version = alcomd_protocol::RPC_VERSION,
        endpoint,
        "starting per-user daemon"
    );
    alcomd::serve_with_data_and_diagnostics_until(config, data, diagnostic_log, async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to wait for Ctrl+C");
    })
    .await
    .context("daemon stopped")?;
    info!("shutdown requested");
    Ok(())
}

fn init_tracing(diagnostic_log: alcomd_application::DiagnosticLog) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("alcomd=info"));
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_filter(filter))
        .with(
            log_capture::DiagnosticLogLayer::new(diagnostic_log).with_filter(
                tracing_subscriber::filter::filter_fn(|metadata| {
                    log_capture::is_core_target(metadata.target())
                }),
            ),
        )
        .init();
}
