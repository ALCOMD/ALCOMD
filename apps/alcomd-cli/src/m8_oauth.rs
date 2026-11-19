use super::*;
use alcomd_protocol::*;

#[derive(Debug, Subcommand)]
pub(super) enum OAuthCommand {
    Registration {
        #[command(subcommand)]
        command: RegistrationCommand,
    },
    Request {
        #[command(subcommand)]
        command: RequestCommand,
    },
}
#[derive(Debug, Subcommand)]
pub(super) enum RegistrationCommand {
    List {
        #[arg(long)]
        cursor: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },
    /// Bind an OAuth registration to an existing MCP client; does not grant authority.
    Set {
        client_id: String,
        #[arg(long,value_parser=clap::value_parser!(u16).range(1..))]
        port: u16,
        #[arg(long, required = true)]
        redirect_uri: Vec<String>,
        #[arg(long)]
        confidential: bool,
        #[arg(long)]
        allow_refresh: bool,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
        #[arg(long)]
        secret_stdout: bool,
    },
    Revoke {
        oauth_client_id: String,
        #[arg(long)]
        expected_revision: u64,
        #[arg(long)]
        idempotency_key: String,
    },
}
#[derive(Debug, Subcommand)]
pub(super) enum RequestCommand {
    Show {
        request_id: String,
    },
    Approve {
        request_id: String,
        #[arg(long)]
        expected_fingerprint: String,
        #[arg(long)]
        idempotency_key: String,
    },
    Deny {
        request_id: String,
        #[arg(long)]
        expected_fingerprint: String,
        #[arg(long)]
        idempotency_key: String,
    },
}
pub(super) fn secret_output(command: &OAuthCommand) -> bool {
    matches!(
        command,
        OAuthCommand::Registration {
            command: RegistrationCommand::Set {
                confidential: true,
                ..
            }
        }
    )
}
pub(super) fn name(command: &OAuthCommand) -> &'static str {
    match command {
        OAuthCommand::Registration { command } => match command {
            RegistrationCommand::List { .. } => "mcp oauth registration list",
            RegistrationCommand::Set { .. } => "mcp oauth registration set",
            RegistrationCommand::Revoke { .. } => "mcp oauth registration revoke",
        },
        OAuthCommand::Request { command } => match command {
            RequestCommand::Show { .. } => "mcp oauth request show",
            RequestCommand::Approve { .. } => "mcp oauth request approve",
            RequestCommand::Deny { .. } => "mcp oauth request deny",
        },
    }
}
pub(super) fn validate(
    command: &OAuthCommand,
    options: ExecutionOptions,
    terminal: bool,
) -> Result<(), CliError> {
    if options.dry_run || options.no_wait {
        return Err(CliError::Usage(
            "OAuth control commands do not support --dry-run or --no-wait",
        ));
    }
    if let OAuthCommand::Registration {
        command:
            RegistrationCommand::Set {
                confidential: true,
                secret_stdout,
                ..
            },
    } = command
        && (!terminal || options.output_mode != OutputMode::Human)
        && !secret_stdout
    {
        return Err(CliError::Usage(
            "OAuth client secret output to a pipe or JSON/NDJSON requires --secret-stdout; no request was sent",
        ));
    }
    Ok(())
}
macro_rules! as_json {
    ($value:expr) => {
        serde_json::to_value($value)
            .map_err(|_| CliError::Client(alcomd_client::ClientError::InvalidResponse))
    };
}
async fn review(
    client: &mut alcomd_client::AlcomdClient,
    id: String,
) -> Result<McpOAuthRequestReview, CliError> {
    Ok(client
        .oauth_request_get(McpOAuthRequestGetParams { request_id: id })
        .await?)
}
pub(super) async fn execute(
    client: &mut alcomd_client::AlcomdClient,
    command: OAuthCommand,
    options: ExecutionOptions,
) -> Result<serde_json::Value, CliError> {
    match command {
        OAuthCommand::Registration { command } => match command {
            RegistrationCommand::List { cursor, limit } => as_json!(
                client
                    .oauth_registrations_list(McpOAuthPageParams { cursor, limit })
                    .await?
            ),
            RegistrationCommand::Set {
                client_id,
                port,
                redirect_uri,
                confidential,
                allow_refresh,
                expected_revision,
                idempotency_key,
                ..
            } => {
                eprintln!(
                    "Bind OAuth to MCP client {} at https://localhost:{port}. ",
                    client_id.escape_debug()
                );
                if confidential {
                    eprintln!(
                        "A new client secret is returned once; the previous client secret cannot be used after this update."
                    );
                }
                if expected_revision != 0 {
                    eprintln!(
                        "All previous OAuth tokens for this registration become invalid. Accepted operations are unchanged."
                    );
                }
                confirm_high_impact(options)?;
                let issuer = format!("https://localhost:{port}");
                let result = client
                    .oauth_registration_set(McpOAuthRegistrationSetParams {
                        client_id,
                        resource: format!("{issuer}/mcp"),
                        issuer,
                        redirect_uris: redirect_uri,
                        confidential,
                        allow_refresh,
                        expected_revision,
                        idempotency_key,
                    })
                    .await
                    .map_err(|e| match e {
                        alcomd_client::ClientError::Remote(ref remote)
                            if !matches!(
                                remote.code.as_str(),
                                "store_unavailable" | "internal_error"
                            ) =>
                        {
                            CliError::Client(e)
                        }
                        _ if confidential => CliError::OAuthCredentialOutcomeUnknown,
                        _ => CliError::Client(e),
                    })?;
                as_json!(result)
            }
            RegistrationCommand::Revoke {
                oauth_client_id,
                expected_revision,
                idempotency_key,
            } => {
                eprintln!(
                    "Revoke OAuth registration {} and its token families. Accepted operations keep running.",
                    oauth_client_id.escape_debug()
                );
                confirm_high_impact(options)?;
                as_json!(
                    client
                        .oauth_registration_revoke(McpOAuthRegistrationRevokeParams {
                            oauth_client_id,
                            expected_revision,
                            idempotency_key
                        })
                        .await?
                )
            }
        },
        OAuthCommand::Request { command } => match command {
            RequestCommand::Show { request_id } => {
                let review = review(client, request_id).await?;
                as_json!(&review)
            }
            RequestCommand::Approve {
                request_id,
                expected_fingerprint,
                idempotency_key,
            } => {
                let reviewed = review(client, request_id.clone()).await?;
                if reviewed.approval_fingerprint != expected_fingerprint {
                    return Err(CliError::Usage(
                        "the authorization review changed; show it again before approving",
                    ));
                }
                eprintln!(
                    "Authorize {} ({}) to use all published MCP tools at {}. Callback: {}.",
                    reviewed.display_name.escape_debug(),
                    reviewed.oauth_client_id,
                    reviewed.resource,
                    reviewed.redirect_uri.escape_debug(),
                );
                confirm_high_impact(options)?;
                as_json!(
                    client
                        .oauth_request_decide(McpOAuthRequestDecideParams {
                            request_id,
                            approve: true,
                            expected_fingerprint,
                            idempotency_key
                        })
                        .await?
                )
            }
            RequestCommand::Deny {
                request_id,
                expected_fingerprint,
                idempotency_key,
            } => as_json!(
                client
                    .oauth_request_decide(McpOAuthRequestDecideParams {
                        request_id,
                        approve: false,
                        expected_fingerprint,
                        idempotency_key
                    })
                    .await?
            ),
        },
    }
}
pub(super) fn write_review(output: &mut impl Write, value: &serde_json::Value) -> io::Result<()> {
    let text = |key: &str| value[key].as_str().unwrap_or("").escape_debug().to_string();
    writeln!(
        output,
        "Request: {}\nClient: {} ({})\nResource: {}\nCallback: {}\nStatus: {}",
        text("requestId"),
        text("displayName"),
        text("oauthClientId"),
        text("resource"),
        text("redirectUri"),
        text("status")
    )?;
    writeln!(output, "Access: all published MCP tools")?;
    writeln!(
        output,
        "Review fingerprint: {}",
        text("approvalFingerprint")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn secret_preflight_reuses_stdout_consent_and_rejects_unsupported_modes() {
        let options = ExecutionOptions {
            output_mode: OutputMode::Json,
            yes: true,
            dry_run: false,
            no_wait: false,
        };
        let command = OAuthCommand::Registration {
            command: RegistrationCommand::Set {
                client_id: "client".into(),
                port: 5443,
                redirect_uri: vec!["http://localhost:1234/callback".into()],
                confidential: true,
                allow_refresh: true,
                expected_revision: 0,
                idempotency_key: "create".into(),
                secret_stdout: false,
            },
        };
        assert!(secret_output(&command));
        assert!(validate(&command, options, false).is_err());
        assert!(
            validate(
                &command,
                ExecutionOptions {
                    output_mode: OutputMode::Human,
                    ..options
                },
                true
            )
            .is_ok()
        );
        assert!(
            validate(
                &command,
                ExecutionOptions {
                    no_wait: true,
                    output_mode: OutputMode::Human,
                    ..options
                },
                true
            )
            .is_err()
        );
    }
    #[test]
    fn review_uses_labels_and_preserves_opaque_ids_without_terminal_controls() {
        let value = serde_json::json!({"requestId":"request","displayName":"Name\ncontrol","oauthClientId":"client","resource":"https://localhost:5443/mcp","redirectUri":"http://localhost:1234/callback","status":"pending","approvalFingerprint":"fingerprint","scopes":["mcp"]});
        let mut bytes = Vec::new();
        write_review(&mut bytes, &value).expect("human output");
        let text = String::from_utf8(bytes).expect("text");
        assert!(text.contains("all published MCP tools"));
        assert!(text.contains("Name\\ncontrol"));
        assert!(!text.contains("\"resourceKind\""));
    }
}
