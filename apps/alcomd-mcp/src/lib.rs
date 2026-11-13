//! Independent MCP transport. Business tools require Core-authenticated clients.

use std::borrow::Cow;

use rmcp::{
    ErrorData, RoleServer, Service,
    model::{
        CacheScope, ClientNotification, ClientRequest, DiscoverResult, ErrorCode, Implementation,
        ListToolsResult, ProtocolVersion, ServerCapabilities, ServerConfig, ServerResult,
    },
    service::{NotificationContext, RequestContext, serve_directly},
};
use tokio::io::{AsyncRead, AsyncWrite};

mod stdio_transport;
mod tools;

/// The sole frozen MCP revision. No legacy handshake or session is supported.
pub const MCP_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion::V_2026_07_28;

/// Run bounded NDJSON over the caller's streams; never connects to Core.
pub async fn serve_stdio<R, W>(input: R, output: W) -> anyhow::Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    serve_stdio_with_client(input, output, None).await
}

/// Connect through the existing typed client; authentication failure never retries as owner.
pub async fn connect_core(
    config: alcomd_client::ClientConfig,
    secret: String,
) -> anyhow::Result<alcomd_client::AlcomdClient> {
    alcomd_client::AlcomdClient::connect(
        config
            .with_client_name("alcomd-mcp")
            .with_mcp_credential(secret),
    )
    .await
    .map_err(|error| {
        let message=match error {
            alcomd_client::ClientError::Remote(ref error) if matches!(error.code.as_str(),"permission_denied"|"authorization_required")=>"Core rejected the MCP client credentials",
            alcomd_client::ClientError::Remote(_)=>"Core rejected the MCP client connection; verify that Core supports client authentication",
            _=>"MCP Core connection is unavailable",
        };
        anyhow::anyhow!(message)
    })
}
pub async fn serve_stdio_with_client<R, W>(
    input: R,
    output: W,
    client: Option<alcomd_client::AlcomdClient>,
) -> anyhow::Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let bound = client.is_some();
    let transport = stdio_transport::StdioTransport::new(input, output);
    // SDK's handshake entry also accepts legacy pre-init ping. Start its normal
    // dispatcher directly and validate every request here, without session state.
    serve_directly(
        ProtocolService {
            core: tokio::sync::Mutex::new(client),
            bound,
        },
        transport,
        None,
    )
    .waiting()
    .await?;
    Ok(())
}

struct ProtocolService {
    core: tokio::sync::Mutex<Option<alcomd_client::AlcomdClient>>,
    bound: bool,
}

impl Service<RoleServer> for ProtocolService {
    async fn handle_request(
        &self,
        request: ClientRequest,
        context: RequestContext<RoleServer>,
    ) -> Result<ServerResult, ErrorData> {
        if matches!(request, ClientRequest::InitializeRequest(_)) {
            return Err(unsupported_method());
        }
        let missing = context.meta.missing_required_keys(&MCP_PROTOCOL_VERSION);
        if !missing.is_empty() {
            return Err(ErrorData::invalid_params(
                format!(
                    "Missing or malformed request metadata: {}",
                    missing.join(", ")
                ),
                None,
            ));
        }
        let requested = context
            .meta
            .protocol_version()
            .ok_or_else(|| ErrorData::invalid_params("Missing request protocol version", None))?;
        if requested != MCP_PROTOCOL_VERSION {
            return Err(ErrorData::unsupported_protocol_version(
                requested,
                &[MCP_PROTOCOL_VERSION],
            ));
        }
        if let ClientRequest::CallToolRequest(request) = request {
            if !self.bound {
                return Err(unsupported_method());
            }
            // SDK cancellation signals the context; it does not drop this
            // handler automatically. Drop only the protocol exchange. The
            // typed client quarantines an interrupted IPC stream, while Core
            // retains and executes any Operation it has already accepted.
            let invoke = async {
                let mut guard = self.core.lock().await;
                let core = guard.as_mut().ok_or_else(unsupported_method)?;
                tools::execute(core, request.params)
                    .await
                    .map(ServerResult::CallToolResult)
            };
            return tokio::select! {
                biased;
                () = context.ct.cancelled() => Err(ErrorData::internal_error("MCP request cancelled", None)),
                result = invoke => result,
            };
        }
        // Bound connections are revalidated by Core on every discovery too;
        // rotation/revocation never becomes a stale successful protocol response.
        if let Some(core) = self.core.lock().await.as_mut() {
            core.system_status().await.map_err(|error| {
                let message = match error {
                    alcomd_client::ClientError::Remote(ref error)
                        if matches!(
                            error.code.as_str(),
                            "permission_denied" | "authorization_required"
                        ) =>
                    {
                        "Core client authorization is no longer valid"
                    }
                    _ => "Core connection is unavailable",
                };
                ErrorData::internal_error(message, None)
            })?;
        }
        match request {
            ClientRequest::DiscoverRequest(_) => Ok(ServerResult::DiscoverResult(
                DiscoverResult::from_server_info(vec![MCP_PROTOCOL_VERSION], self.get_info()),
            )),
            ClientRequest::ListToolsRequest(request) if self.bound => {
                if request.params.and_then(|p| p.cursor).is_some() {
                    return Err(ErrorData::invalid_params(
                        "This tool list has no continuation cursor.",
                        None,
                    ));
                }
                Ok(ServerResult::ListToolsResult(
                    ListToolsResult::with_all_items(tools::definitions()?)
                        .with_ttl_ms(0)
                        .with_cache_scope(CacheScope::Private),
                ))
            }
            // Do not return empty business results or advertise unimplemented
            // capabilities while Core identity/tool contracts remain pending.
            _ => Err(unsupported_method()),
        }
    }

    async fn handle_notification(
        &self,
        _notification: ClientNotification,
        _context: NotificationContext<RoleServer>,
    ) -> Result<(), ErrorData> {
        // No subscriptions or side effects exist at this stage. Request
        // cancellation is handled by the SDK, never as Core Operation cancel.
        Ok(())
    }

    fn get_info(&self) -> ServerConfig {
        let capabilities = if self.bound {
            ServerCapabilities::builder().enable_tools().build()
        } else {
            ServerCapabilities::default()
        };
        ServerConfig::new(capabilities)
            .with_protocol_version(MCP_PROTOCOL_VERSION)
            .with_server_info(Implementation::new(
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
            ))
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Owned(vec![MCP_PROTOCOL_VERSION])
    }
}

fn unsupported_method() -> ErrorData {
    ErrorData::new(ErrorCode::METHOD_NOT_FOUND, "Method not supported", None)
}
