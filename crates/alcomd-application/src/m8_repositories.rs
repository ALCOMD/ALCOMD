//! Authorized access to the shared repository registry from an MCP client.
use super::*;

#[derive(Clone)]
pub struct McpRepositoryAuthority {
    pub caller: PrincipalId,
    pub authentication: ToolAuthentication,
}

#[derive(Clone, Copy)]
pub enum McpRepositoryCommand {
    Register {
        preview_id: OperationId,
    },
    Remove {
        id: RepositoryId,
        expected: Revision,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRepositoryEntry {
    pub repository_id: RepositoryId,
    pub display_name: Option<String>,
    pub revision: Revision,
    pub source_kind: McpRepositorySourceKind,
    pub builtin: bool,
    pub available: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpRepositorySourceKind {
    Remote,
    Local,
}

impl From<RepositoryRecord> for McpRepositoryEntry {
    fn from(record: RepositoryRecord) -> Self {
        Self {
            repository_id: record.repository_id,
            display_name: record.display_name.or(record.observation.name),
            revision: record.revision,
            source_kind: if matches!(record.observation.source, RepositorySource::Remote { .. }) {
                McpRepositorySourceKind::Remote
            } else {
                McpRepositorySourceKind::Local
            },
            builtin: builtin_repository(record.repository_id).is_some(),
            available: record.resolver_indexed,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpRepositoryReceipt {
    pub repository: Option<McpRepositoryEntry>,
    pub repository_id: RepositoryId,
    pub revision: Revision,
    pub replayed: bool,
}

pub trait McpRepositoryStore {
    fn mcp_repository_replay(
        &self,
        authority: McpRepositoryAuthority,
        command: McpRepositoryCommand,
        key: IdempotencyKey,
    ) -> impl Future<Output = Result<Option<McpRepositoryReceipt>, M3Error>> + Send;

    fn mcp_repository_commit(
        &self,
        authority: McpRepositoryAuthority,
        command: McpRepositoryCommand,
        observation: Option<RepositoryReadOutcome>,
        key: IdempotencyKey,
        now_ms: u64,
    ) -> impl Future<Output = Result<McpRepositoryReceipt, M3Error>> + Send;
}

pub(super) struct McpRepositoryPreview {
    id: OperationId,
    caller: PrincipalId,
    authentication_binding: String,
    expires_at: Instant,
    observation: RepositoryObservation,
}

/// Compare the observed source and contents independently of fetch time and validators.
pub fn same_repository_evidence(
    left: &RepositoryObservation,
    right: &RepositoryObservation,
) -> bool {
    left.source == right.source
        && left.source_identity_key == right.source_identity_key
        && left.declared_id == right.declared_id
        && left.name == right.name
        && left.declared_url == right.declared_url
        && left.issues == right.issues
        && left.packages == right.packages
}

impl<S: M3RegistryStore + McpRepositoryStore, R: M3ReadAdapter> M3Application<S, R> {
    pub async fn mcp_repositories_list(
        &self,
        access: &AccessContext,
        cursor: Option<RepositoryCursor>,
        limit: u32,
    ) -> Result<RepositoryPage, M3Error> {
        require_m3(access, Permission::RepositoriesRead)?;
        validate_m3_limit(limit)?;
        self.store
            .list_repositories(PrincipalId::local_owner(), cursor, limit)
            .await
    }

    pub async fn mcp_repository_inspect(
        &self,
        access: &AccessContext,
        authentication: &ToolAuthentication,
        url: String,
    ) -> Result<(Option<OperationId>, Option<String>, usize), M3Error> {
        require_m3(access, Permission::RepositoriesRead)?;
        let observation = self
            .read_repository_preview(RepositorySource::Remote { url }, None)
            .await?;
        let name = observation.name.clone();
        let version_count = observation.packages.len();
        if access.require(Permission::RepositoriesManage).is_err() {
            return Ok((None, name, version_count));
        }
        let id = OperationId::new();
        let mut previews = self.mcp_repository_previews.lock().await;
        let now = Instant::now();
        previews.retain(|preview| preview.expires_at > now);
        if previews.len() == REPOSITORY_PREVIEW_LIMIT {
            previews.pop_front();
        }
        previews.push_back(McpRepositoryPreview {
            id,
            caller: access.principal().clone(),
            authentication_binding: authentication.binding(),
            expires_at: now + REPOSITORY_PREVIEW_TTL,
            observation,
        });
        Ok((Some(id), name, version_count))
    }

    pub async fn mcp_repository_write(
        &self,
        access: &AccessContext,
        authentication: ToolAuthentication,
        command: McpRepositoryCommand,
        key: IdempotencyKey,
    ) -> Result<McpRepositoryReceipt, M3Error> {
        require_m3(access, Permission::RepositoriesManage)?;
        if let McpRepositoryCommand::Remove { id, .. } = command
            && !can_remove_repository(access, id)
        {
            return Err(M3Error::new(M3ErrorCode::PermissionDenied));
        }
        let authority = McpRepositoryAuthority {
            caller: access.principal().clone(),
            authentication,
        };
        let _registration = if matches!(command, McpRepositoryCommand::Register { .. }) {
            Some(self.repository_import.lock().await)
        } else {
            None
        };
        let _repository = match command {
            McpRepositoryCommand::Remove { id, .. } => {
                Some(self.locks.acquire(vec![ResourceKey::Repository(id)]).await)
            }
            McpRepositoryCommand::Register { .. } => None,
        };
        if let Some(replayed) = self
            .store
            .mcp_repository_replay(authority.clone(), command, key.clone())
            .await?
        {
            return Ok(replayed);
        }
        let owner = PrincipalId::local_owner();
        let (observation, old_credential) = match command {
            McpRepositoryCommand::Register { preview_id } => {
                let binding = authority.authentication.binding();
                let (expires_at, preview) = {
                    let now = Instant::now();
                    self.mcp_repository_previews
                        .lock()
                        .await
                        .iter()
                        .find(|preview| {
                            preview.id == preview_id
                                && preview.expires_at > now
                                && preview.caller == authority.caller
                                && preview.authentication_binding == binding
                        })
                        .map(|preview| (preview.expires_at, preview.observation.clone()))
                        .ok_or_else(|| M3Error::new(M3ErrorCode::RevisionConflict))?
                };
                let fresh = self
                    .read_repository_preview(preview.source.clone(), None)
                    .await?;
                if Instant::now() >= expires_at || !same_repository_evidence(&preview, &fresh) {
                    return Err(M3Error::new(M3ErrorCode::RevisionConflict));
                }
                (Some(RepositoryReadOutcome::Fresh(fresh)), None)
            }
            McpRepositoryCommand::Remove { id, .. } => {
                let credential = match self.store.repository_credential_reference(owner, id).await {
                    Ok(reference) => reference,
                    Err(error) if error.code() == M3ErrorCode::RepositoryNotRegistered => None,
                    Err(error) => return Err(error),
                };
                (None, credential)
            }
        };
        let result = self
            .store
            .mcp_repository_commit(authority, command, observation, key, m3_time_ms()?)
            .await?;
        if let Some(reference) = old_credential.filter(|_| !result.replayed) {
            self.discard_credential_reference(reference).await;
        }
        Ok(result)
    }
}
