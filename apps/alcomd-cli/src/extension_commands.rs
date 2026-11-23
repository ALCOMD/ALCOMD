#[path = "extension_management.rs"]
mod management;

use crate::{CliError, ExecutionOptions, confirm_frozen_plan, confirm_high_impact};
use alcomd_client::AlcomdClient;
use alcomd_protocol::{ExtensionCliCommandsInvokeParams, ExtensionCliCommandsListParams, PlanKind};
use clap::Subcommand;
use serde_json::{Value, json};

#[derive(Debug, Subcommand)]
pub(super) enum Command {
    #[command(flatten)]
    Management(management::Command),
    /// Read a page of enabled extensions' signed CLI commands.
    Commands {
        #[arg(long)]
        extension_id: Option<String>,
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Run one named command in its extension's sandbox.
    Run {
        extension_id: String,
        command: String,
        #[arg(long)]
        arguments: Option<String>,
        #[arg(long)]
        idempotency_key: Option<String>,
        #[arg(long, conflicts_with = "operation_id")]
        plan_id: Option<String>,
        #[arg(long)]
        operation_id: Option<String>,
        #[arg(long)]
        expected_revision: Option<u64>,
    },
}
impl Command {
    pub(super) fn supports_preview(&self) -> bool {
        match self {
            Self::Management(command) => command.supports_preview(),
            Self::Run { .. } => true,
            Self::Commands { .. } => false,
        }
    }

    pub(super) fn name(&self) -> &'static str {
        match self {
            Self::Management(command) => command.name(),
            Self::Commands { .. } => "extension commands",
            Self::Run { .. } => "extension run",
        }
    }
}

pub(super) async fn execute(
    client: &mut AlcomdClient,
    command: Command,
    options: ExecutionOptions,
) -> Result<Value, CliError> {
    let result = match command {
        Command::Management(command) => return management::execute(client, command, options).await,
        Command::Commands {
            extension_id,
            cursor,
            limit,
        } => serde_json::to_value(
            client
                .extension_cli_commands_list(ExtensionCliCommandsListParams {
                    extension_id,
                    cursor,
                    limit,
                })
                .await?,
        ),
        Command::Run {
            extension_id,
            command,
            arguments,
            idempotency_key,
            plan_id,
            operation_id,
            expected_revision,
        } => {
            let mut fields = match arguments {
                None => serde_json::Map::new(),
                Some(input) => serde_json::from_str(&input)
                    .map_err(|_| CliError::Usage("--arguments must be a JSON object"))?,
            };
            let mut cursor = None;
            let descriptor = loop {
                let page = client
                    .extension_cli_commands_list(ExtensionCliCommandsListParams {
                        extension_id: Some(extension_id.clone()),
                        cursor,
                        limit: Some(32),
                    })
                    .await?;
                if let Some(item) = page.commands.into_iter().find(|item| item.name == command) {
                    break item;
                }
                cursor = page.next_cursor;
                if cursor.is_none() {
                    return Err(CliError::Usage(
                        "this extension does not declare the requested command",
                    ));
                }
            };
            let effect = descriptor.effect.as_str();
            if options.dry_run && effect != "plan" {
                return Err(CliError::Usage("this command does not produce a plan"));
            }
            if matches!(effect, "plan" | "apply" | "cancel") {
                let reserved: &[&str] = match effect {
                    "apply" => &["idempotency_key", "plan_id", "expected_revision", "confirm"],
                    "cancel" => &["idempotency_key", "operation_id", "expected_revision"],
                    _ => &["idempotency_key"],
                };
                for &field in reserved {
                    if fields.contains_key(field) {
                        return Err(CliError::Usage("pass write controls as CLI options"));
                    }
                }
                let key = idempotency_key
                    .ok_or(CliError::Usage("this command requires --idempotency-key"))?;
                fields.insert("idempotency_key".into(), json!(key));
            } else if idempotency_key.is_some() {
                return Err(CliError::Usage(
                    "this command does not accept --idempotency-key",
                ));
            }
            if (effect != "apply" && plan_id.is_some())
                || (effect != "cancel" && operation_id.is_some())
                || (!matches!(effect, "apply" | "cancel") && expected_revision.is_some())
            {
                return Err(CliError::Usage(
                    "the supplied controls do not match the command's effect",
                ));
            }
            if effect == "apply" {
                let id = plan_id.ok_or(CliError::Usage("this command requires --plan-id"))?;
                let revision =
                    confirm_frozen_plan(client, PlanKind::Package, &id, expected_revision, options)
                        .await?
                        .ok_or(CliError::Usage("the selected plan has no revision"))?;
                fields.insert("plan_id".into(), json!(id));
                fields.insert("expected_revision".into(), json!(revision));
                fields.insert("confirm".into(), json!(true));
            } else if effect == "cancel" {
                let operation =
                    operation_id.ok_or(CliError::Usage("this command requires --operation-id"))?;
                let revision = expected_revision
                    .ok_or(CliError::Usage("this command requires --expected-revision"))?;
                confirm_high_impact(options)?;
                fields.insert("operation_id".into(), json!(operation));
                fields.insert("expected_revision".into(), json!(revision));
            }
            let response = client
                .extension_cli_commands_invoke(ExtensionCliCommandsInvokeParams {
                    extension_id,
                    name: command,
                    arguments: Value::Object(fields),
                })
                .await?;
            if let Some(code) = response.error {
                return Err(CliError::ExtensionCommandFailed {
                    code,
                    core_effect: response.core_effect,
                    replayed: response.replayed,
                });
            }
            serde_json::to_value(response)
        }
    };
    result.map_err(|_| CliError::Client(alcomd_client::ClientError::InvalidResponse))
}
