use super::{CliError, ExecutionOptions, OutputMode, confirm_high_impact};
use alcomd_client::AlcomdClient;
use alcomd_protocol::*;
use clap::Subcommand;
use std::io::IsTerminal;

#[derive(Debug, Subcommand)]
pub(super) enum Command {
    Create {
        /// JSON action as defined by the client management request contract.
        #[arg(long,value_parser=parse_action)]
        action: ClientManagementAction,
        #[arg(long)]
        idempotency_key: String,
    },
    Get {
        request_id: String,
    },
    List {
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },
    Approve {
        request_id: String,
        #[arg(long)]
        idempotency_key: String,
        /// Allow a one-time secret to be written to a pipe or structured output.
        #[arg(long)]
        secret_stdout: bool,
    },
    Deny {
        request_id: String,
        #[arg(long)]
        idempotency_key: String,
    },
}

impl Command {
    pub(super) fn name(&self) -> &'static str {
        match self {
            Self::Create { .. } => "mcp client request create",
            Self::Get { .. } => "mcp client request get",
            Self::List { .. } => "mcp client request list",
            Self::Approve { .. } => "mcp client request approve",
            Self::Deny { .. } => "mcp client request deny",
        }
    }
}
fn parse_action(text: &str) -> Result<ClientManagementAction, String> {
    serde_json::from_str(text).map_err(|error| error.to_string())
}

pub(super) async fn lookup(
    client: &mut AlcomdClient,
    id: &str,
) -> Result<McpClientRecord, CliError> {
    let mut cursor = None;
    loop {
        let page = client
            .mcp_clients_list(McpClientsListParams {
                cursor,
                limit: None,
            })
            .await?;
        if let Some(item) = page.items.into_iter().find(|item| item.client_id == id) {
            return Ok(item);
        }
        cursor = page.next_cursor;
        if cursor.is_none() {
            return Err(CliError::Usage("Client not found"));
        }
    }
}

pub(super) async fn execute(
    client: &mut AlcomdClient,
    command: Command,
    options: ExecutionOptions,
) -> Result<serde_json::Value, CliError> {
    let result = match command {
        Command::List { cursor, limit } => serde_json::to_value(
            client
                .mcp_client_request_list(McpClientsListParams { cursor, limit })
                .await?,
        ),
        Command::Get { request_id } => serde_json::to_value(
            client
                .mcp_client_request_get(McpClientRequestGetParams { request_id })
                .await?,
        ),
        Command::Create {
            action,
            idempotency_key,
        } => {
            eprintln!("Submit management request {}", serde_json::json!(&action));
            confirm_high_impact(options)?;
            serde_json::to_value(
                client
                    .mcp_client_request_create(McpClientRequestCreateParams {
                        action,
                        idempotency_key,
                    })
                    .await?,
            )
        }
        decision => {
            let (request_id, idempotency_key, approve, secret_stdout) = match decision {
                Command::Approve {
                    request_id,
                    idempotency_key,
                    secret_stdout,
                } => (request_id, idempotency_key, true, secret_stdout),
                Command::Deny {
                    request_id,
                    idempotency_key,
                } => (request_id, idempotency_key, false, false),
                _ => unreachable!(),
            };
            let review = client
                .mcp_client_request_get(McpClientRequestGetParams {
                    request_id: request_id.clone(),
                })
                .await?;
            let returns_secret = matches!(
                review.action,
                ClientManagementAction::IssueCredential { .. }
                    | ClientManagementAction::SetOAuthRegistration {
                        confidential: true,
                        ..
                    }
            );
            if approve
                && returns_secret
                && !secret_stdout
                && (!std::io::stdout().is_terminal() || options.output_mode != OutputMode::Human)
            {
                return Err(CliError::Usage(
                    "Use --secret-stdout to allow a one-time credential to be written to this output. No decision was sent.",
                ));
            }
            eprintln!(
                "{} {} from {}: {} (expires at {} ms).",
                if approve { "Approve" } else { "Deny" },
                review.request_id,
                review.requester.escape_debug(),
                serde_json::json!(&review.action),
                review.expires_at_ms
            );
            confirm_high_impact(options)?;
            serde_json::to_value(
                client
                    .mcp_client_request_decide(McpClientRequestDecideParams {
                        request_id,
                        approve,
                        expected_fingerprint: review.fingerprint,
                        idempotency_key,
                    })
                    .await?,
            )
        }
    };
    result.map_err(|_| CliError::Client(alcomd_client::ClientError::InvalidResponse))
}
