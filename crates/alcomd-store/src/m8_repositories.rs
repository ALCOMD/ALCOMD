use alcomd_application as app;

impl app::McpRepositoryStore for super::StateStoreHandle {
    async fn mcp_repository_replay(
        &self,
        authority: app::McpRepositoryAuthority,
        command: app::McpRepositoryCommand,
        key: app::IdempotencyKey,
    ) -> Result<Option<app::McpRepositoryReceipt>, app::M3Error> {
        self.request_m3(move |db| super::m3::mcp_repository_replay(db, &authority, command, &key))
            .await
    }

    async fn mcp_repository_commit(
        &self,
        authority: app::McpRepositoryAuthority,
        command: app::McpRepositoryCommand,
        observation: Option<app::RepositoryReadOutcome>,
        key: app::IdempotencyKey,
        now_ms: u64,
    ) -> Result<app::McpRepositoryReceipt, app::M3Error> {
        self.request_m3(move |db| {
            super::m3::mcp_repository_commit(db, &authority, command, observation, &key, now_ms)
        })
        .await
    }
}
