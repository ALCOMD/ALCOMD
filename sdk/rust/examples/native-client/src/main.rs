use std::process::ExitCode;

use alcomd_client::{AlcomdClient, ClientConfig, EventSubscription};
use alcomd_platform::{DataConfig, core_installation_registration};
use alcomd_protocol::EventsSubscribeParams;
use serde_json::json;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    if let Err(error) = run().await {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let command = std::env::args().nth(1).unwrap_or_else(|| "probe".into());
    if command == "registration-path" {
        println!(
            "{}",
            core_installation_registration(&DataConfig::default())?.display()
        );
        return Ok(());
    }
    let config = ClientConfig::for_installed_core().with_client_name("native-sdk-example");
    let config = match command.as_str() {
        "probe" => config,
        "probe-running" => config.without_daemon_start(),
        _ => return Err("unknown native SDK example command".into()),
    };
    let mut client = AlcomdClient::connect(config.clone()).await?;
    let status = client.system_status().await?;
    let page = client.projects_list(None, Some(1)).await?;
    let events = EventSubscription::connect(config, EventsSubscribeParams::default()).await?;
    println!(
        "{}",
        json!({
            "hello": client.hello_result(),
            "status": status,
            "instanceId": events.ready().instance_id,
            "projectCount": page.projects.len(),
        })
    );
    Ok(())
}
