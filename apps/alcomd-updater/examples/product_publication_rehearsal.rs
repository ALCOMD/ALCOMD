//! Offline harness for the signed publication rehearsal. Built with synthetic trust only.

use std::path::PathBuf;

use alcomd_updater::{
    Channel,
    distribution::{Target, format_digest},
    product_update::{UpdateFormat, UpdateRequest, UpdateStream, download_update},
};
use anyhow::ensure;
use clap::Parser;

const REHEARSAL_PUBLIC_KEY: &str =
    "bed7d2ab668da3efad613998f06f7abf7875f3a6b7677a9f3ce947d77d7760a6";

#[derive(Parser)]
struct Arguments {
    #[arg(long, value_enum)]
    channel: Channel,
    #[arg(long, value_enum)]
    target: Target,
    #[arg(long, value_enum)]
    format: UpdateFormat,
    #[arg(long)]
    installed_version: semver::Version,
    #[arg(long)]
    download_directory: PathBuf,
    #[arg(long)]
    replay_directory: PathBuf,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let arguments = Arguments::parse();
    ensure!(
        option_env!("ALCOMD_RELEASE_PUBLIC_KEY_HEX") == Some(REHEARSAL_PUBLIC_KEY),
        "publication rehearsal requires the synthetic public key"
    );
    let staged = download_update(
        &url::Url::parse("https://localhost/")?,
        &UpdateRequest {
            stream: UpdateStream {
                channel: arguments.channel,
                target: arguments.target,
                format: arguments.format,
            },
            installed_version: &arguments.installed_version,
            allowed_asset_hosts: &["localhost"],
            now: chrono::Utc::now(),
            previous: None,
        },
        &arguments.download_directory,
        &arguments.replay_directory,
    )
    .await?;
    let report = staged.map(|value| {
        let accepted = value.verified.acceptance();
        serde_json::json!({
            "version": value.verified.version().to_string(),
            "serial": accepted.serial,
            "manifestSha256": format_digest(&accepted.manifest_sha256),
        })
    });
    println!("{}", serde_json::to_string(&report)?);
    Ok(())
}
