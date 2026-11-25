use std::path::PathBuf;

use alcomd_updater::{
    Channel, decode_hex,
    distribution::Target,
    product_update::{
        AcceptedUpdate, MAX_UPDATE_RESPONSE_BYTES, UpdateFormat, UpdateRequest, UpdateStream,
        download_update, product_update_origin, verify_and_record_update, verify_update,
    },
    read_limited, verify_staged_payload,
};
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
    /// Verify a signed v4 update response and its local asset without contacting a server.
    VerifyProductUpdate {
        #[arg(long)]
        envelope: PathBuf,
        #[arg(long)]
        asset: PathBuf,
        #[arg(long, value_enum)]
        channel: Channel,
        #[arg(long, value_enum)]
        target: Target,
        #[arg(long, value_enum)]
        format: UpdateFormat,
        #[arg(long)]
        installed_version: semver::Version,
        #[arg(long, required = true)]
        allow_host: Vec<String>,
        #[arg(long, requires = "previous_manifest_sha256")]
        previous_serial: Option<u64>,
        #[arg(long, requires = "previous_serial")]
        previous_manifest_sha256: Option<String>,
        #[arg(long, conflicts_with_all = ["previous_serial", "previous_manifest_sha256"])]
        replay_directory: Option<PathBuf>,
    },
    /// Download a verified v4 update into a trusted existing directory.
    DownloadProductUpdate {
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
    },
}

fn main() -> anyhow::Result<()> {
    let Arguments { command } = Arguments::parse();
    match command {
        Command::VerifyStaging { staging, channel } => {
            let verified = verify_staged_payload(&staging, channel)?;
            println!("Verified staged product {}", verified.version());
        }
        Command::VerifyProductUpdate {
            envelope,
            asset,
            channel,
            target,
            format,
            installed_version,
            allow_host,
            previous_serial,
            previous_manifest_sha256,
            replay_directory,
        } => {
            let stream = UpdateStream {
                channel,
                target,
                format,
            };
            let previous =
                if let Some((serial, digest)) = previous_serial.zip(previous_manifest_sha256) {
                    Some(AcceptedUpdate {
                        stream,
                        serial,
                        manifest_sha256: decode_hex(&digest)?,
                    })
                } else {
                    None
                };
            let hosts: Vec<&str> = allow_host.iter().map(String::as_str).collect();
            let envelope = read_limited(
                &std::path::absolute(envelope)?,
                MAX_UPDATE_RESPONSE_BYTES as u64,
            )?;
            let request = UpdateRequest {
                stream,
                installed_version: &installed_version,
                allowed_asset_hosts: &hosts,
                now: chrono::Utc::now(),
                previous,
            };
            let asset = std::path::absolute(asset)?;
            let verified = if let Some(directory) = replay_directory {
                verify_and_record_update(
                    &std::path::absolute(directory)?,
                    &envelope,
                    &request,
                    &asset,
                )?
            } else {
                let verified = verify_update(&envelope, &request)?;
                verified.asset().verify_file(&asset)?;
                verified
            };
            println!(
                "Verified product update {} (serial {})",
                verified.version(),
                verified.acceptance().serial
            );
        }
        Command::DownloadProductUpdate {
            channel,
            target,
            format,
            installed_version,
            download_directory,
            replay_directory,
        } => {
            let origin = product_update_origin()?;
            let origin_host = origin.host_str().ok_or_else(|| {
                anyhow::anyhow!("embedded product update origin does not have a host")
            })?;
            let request = UpdateRequest {
                stream: UpdateStream {
                    channel,
                    target,
                    format,
                },
                installed_version: &installed_version,
                allowed_asset_hosts: &[origin_host],
                now: chrono::Utc::now(),
                previous: None,
            };
            let download_directory = std::path::absolute(download_directory)?;
            let replay_directory = std::path::absolute(replay_directory)?;
            let network = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            let staged = network.block_on(download_update(
                &origin,
                &request,
                &download_directory,
                &replay_directory,
            ))?;
            if let Some(staged) = staged {
                println!(
                    "Staged product update {} (serial {})",
                    staged.verified.version(),
                    staged.verified.acceptance().serial
                );
            } else {
                println!("No product update available");
            }
        }
    }
    Ok(())
}
