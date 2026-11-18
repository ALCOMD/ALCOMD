use alcomd_protocol as rpc;

use crate::{AlcomdClient, ClientError};

impl AlcomdClient {
    pub async fn mcp_repositories_list(
        &mut self,
        params: rpc::McpRepositoriesListParams,
    ) -> Result<rpc::McpRepositoriesListResult, ClientError> {
        self.call(rpc::METHOD_MCP_REPOSITORIES_LIST, params).await
    }

    pub async fn mcp_repository_inspect(
        &mut self,
        params: rpc::McpRepositoryInspectParams,
    ) -> Result<rpc::McpRepositoryInspectResult, ClientError> {
        self.call(rpc::METHOD_MCP_REPOSITORIES_INSPECT, params)
            .await
    }

    pub async fn mcp_repository_register(
        &mut self,
        params: rpc::McpRepositoryRegisterParams,
    ) -> Result<rpc::McpRepositoryWriteResult, ClientError> {
        self.call(rpc::METHOD_MCP_REPOSITORIES_REGISTER, params)
            .await
    }

    pub async fn mcp_repository_refresh(
        &mut self,
        params: rpc::RepositoryMutationParams,
    ) -> Result<rpc::McpRepositoryWriteResult, ClientError> {
        self.call(rpc::METHOD_MCP_REPOSITORIES_REFRESH, params)
            .await
    }

    pub async fn mcp_repository_remove(
        &mut self,
        params: rpc::McpRepositoryRemoveParams,
    ) -> Result<rpc::McpRepositoryWriteResult, ClientError> {
        self.call(rpc::METHOD_MCP_REPOSITORIES_REMOVE, params).await
    }
}
