use alcomd_client::AlcomdClient;
use alcomd_protocol::{McpClientMutationParams, McpClientsCreateParams};

pub async fn issue_actor(
    owner: &mut AlcomdClient,
    name: &str,
) -> alcomd_protocol::McpClientCredentialResult {
    let record = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: name.to_owned(),
            idempotency_key: format!("create-{name}"),
        })
        .await
        .expect("create MCP client");
    owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: record.client_id,
            expected_revision: record.revision,
            idempotency_key: format!("issue-{name}"),
        })
        .await
        .expect("issue credential")
}
