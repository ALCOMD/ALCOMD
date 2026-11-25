use std::path::PathBuf;

use alcomd_updater::{
    Channel, decode_hex,
    distribution::Target,
    product_update::{
        AcceptedUpdate, MAX_UPDATE_RESPONSE_BYTES, UpdateFormat, UpdateRequest, UpdateStream,
        verify_update,
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
            let verified = verify_update(
                &envelope,
                &UpdateRequest {
                    stream,
                    installed_version: &installed_version,
                    allowed_asset_hosts: &hosts,
                    now: chrono::Utc::now(),
                    previous,
                },
            )?;
            verified.asset().verify_file(&std::path::absolute(asset)?)?;
            println!(
                "Verified product update {} (serial {})",
                verified.version(),
                verified.acceptance().serial
            );
        }
    }
    Ok(())
}
