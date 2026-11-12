//! Independent MCP transport. Business tools require Core-authenticated clients.

use std::borrow::Cow;

use rmcp::{
    ErrorData, RoleServer, Service,
    model::{
        ClientNotification, ClientRequest, DiscoverResult, ErrorCode, Implementation,
        ProtocolVersion, ServerCapabilities, ServerConfig, ServerResult,
    },
    service::{NotificationContext, RequestContext, serve_directly},
};
use tokio::io::{AsyncRead, AsyncWrite};

mod stdio_transport;

/// The sole frozen MCP revision. No legacy handshake or session is supported.
pub const MCP_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion::V_2026_07_28;

/// Run bounded NDJSON over the caller's streams; never connects to Core.
pub async fn serve_stdio<R, W>(input: R, output: W) -> anyhow::Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let transport = stdio_transport::StdioTransport::new(input, output);
    // SDK's handshake entry also accepts legacy pre-init ping. Start its normal
    // dispatcher directly and validate every request here, without session state.
    serve_directly(ProtocolService, transport, None)
        .waiting()
        .await?;
    Ok(())
}

struct ProtocolService;

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
        match request {
            ClientRequest::DiscoverRequest(_) => Ok(ServerResult::DiscoverResult(
                DiscoverResult::from_server_info(vec![MCP_PROTOCOL_VERSION], self.get_info()),
            )),
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
        ServerConfig::new(ServerCapabilities::default())
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
