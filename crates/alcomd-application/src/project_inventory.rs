use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstalledEvidence {
    Matched,
    Missing,
    VersionMismatch,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInventoryEntry {
    pub package_id: String,
    pub declared_range: Option<String>,
    pub locked_version: Option<String>,
    pub observed_version: Option<String>,
    pub state: InstalledEvidence,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectResolutionEvidence {
    Needed,
    NoIssueObserved,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInventory {
    pub entries: Vec<ProjectInventoryEntry>,
    pub resolution: ProjectResolutionEvidence,
    pub reason: String,
}

pub trait ProjectInventoryReader: M3ReadAdapter {
    fn read_project_inventory(
        &self,
        registered: &ProjectObservation,
    ) -> impl std::future::Future<Output = Result<ProjectInventory, CandidateQueryError>> + Send;
}

impl<S: M3RegistryStore, R: ProjectInventoryReader> M3Application<S, R> {
    pub async fn project_inventory(
        &self,
        access: &AccessContext,
        id: ProjectId,
        expected_revision: u64,
    ) -> Result<ProjectInventory, CandidateQueryError> {
        use CandidateQueryError as Error;
        for permission in [Permission::ProjectsRead, Permission::PackagesRead] {
            access
                .require(permission)
                .map_err(|_| Error::PermissionDenied)?;
        }
        let registered =
            self.get_project(access, id)
                .await
                .map_err(|error| match error.code() {
                    M3ErrorCode::PermissionDenied => Error::PermissionDenied,
                    M3ErrorCode::ProjectNotFound | M3ErrorCode::ProjectNotRegistered => {
                        Error::EvidenceStale
                    }
                    _ => Error::Unavailable,
                })?;
        if registered.project.revision.get() != expected_revision {
            return Err(Error::RevisionConflict);
        }
        let inventory = self
            .reader
            .read_project_inventory(&registered.project.observation)
            .await?;
        let current = self
            .get_project(access, id)
            .await
            .map_err(|_| Error::EvidenceStale)?;
        if current.project.revision != registered.project.revision {
            return Err(Error::EvidenceStale);
        }
        Ok(inventory)
    }
}
