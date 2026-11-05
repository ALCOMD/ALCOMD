use anyhow::Context;
use clap::Parser;
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

use tracing::Instrument;

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
    let database = alcomd_platform::state_database_path(&data)
        .context("failed to resolve Core data directory")?;
    let directory = database
        .parent()
        .context("state path has no parent")?
        .join("logs")
        .join("core");
    let logging = alcomd_logging::LoggingRuntime::start(directory, "core");
    let log_source = logging.source();
    init_tracing(log_source.clone());
    let process_span = tracing::info_span!("process",log_source="core",session_id=log_source.session_id(),state_path=%database.display());
    let endpoint = alcomd_platform::endpoint_display(&config)
        .context("failed to resolve the per-user RPC endpoint")?;
    info!(
        code = "core.started",
        product = alcomd_protocol::PRODUCT_FAMILY,
        rpc_version = alcomd_protocol::RPC_VERSION,
        endpoint,
        "starting per-user daemon"
    );
    let result = alcomd::serve_with_data_and_logs_until(config, data, log_source, async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to wait for Ctrl+C");
        info!("shutdown requested");
    })
    .instrument(process_span.clone())
    .await;
    process_span.in_scope(|| {
        if let Err(error) = &result {
            alcomd_application::record_technical_failure(
                error,
                Some(&database.to_string_lossy()),
                None,
                "Core daemon stopped unexpectedly",
            );
        } else {
            info!("Core daemon stopped");
        }
    });
    let _ = tokio::task::spawn_blocking(move || logging.shutdown()).await;
    result.context("daemon stopped")?;
    Ok(())
}

fn init_tracing(source: alcomd_logging::LogSource) {
    let filter=EnvFilter::try_from_default_env().unwrap_or_else(|_|EnvFilter::new("warn,alcomd=info,alcomd_application=info,alcomd_vpm=info,alcomd_store=info,alcomd_platform=info,alcomd_extensions=info"));
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_current_span(false)
                .with_span_list(true)
                .with_writer(source.writer())
                .with_filter(filter),
        )
        .init();
}
