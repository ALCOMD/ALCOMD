use anyhow::Context;
use clap::Parser;
use tracing::info;
use tracing_subscriber::EnvFilter;

/// ALCOMD core daemon scaffold.
#[derive(Debug, Parser)]
#[command(name = "alcomd", version, about)]
struct Arguments {
    /// Print the scaffold health response and exit.
    #[arg(long)]
    once: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let arguments = Arguments::parse();

    if arguments.once {
        let response = alcomd_protocol::HelloResponse::scaffold();
        println!("{}", serde_json::to_string_pretty(&response)?);
        return Ok(());
    }

    info!(
        product = alcomd_protocol::PRODUCT_FAMILY,
        rpc_version = alcomd_protocol::RPC_VERSION,
        "starting scaffold daemon; IPC is intentionally not implemented in M0"
    );

    tokio::signal::ctrl_c()
        .await
        .context("failed to wait for Ctrl+C")?;
    info!("shutdown requested");
    Ok(())
}

fn init_tracing() {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("alcomd=info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
