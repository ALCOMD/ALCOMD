//! Explicit project evidence shared by candidate and Unity migration fingerprints.

use serde::Serialize;

use crate::{
    DependencyIdentity, ManifestState, ProjectId, ProjectObservation, ProjectRecord, ProjectType,
    ReadIssue, Revision,
};

/// Frozen fingerprint projection, independent of presentation metadata added to project records.
///
/// Field names and declaration order preserve the existing serialized authority bytes. In
/// particular, favorite and registration time remain part of that authority; activity time does
/// not. This is an internal application projection, not a public RPC or persistence DTO.
#[derive(Serialize)]
pub struct ProjectAuthorityFingerprint<'a> {
    project_id: ProjectId,
    observation: ProjectObservationAuthority<'a>,
    revision: Revision,
    registered_at_ms: u64,
    favorite: bool,
}

impl<'a> From<&'a ProjectRecord> for ProjectAuthorityFingerprint<'a> {
    fn from(project: &'a ProjectRecord) -> Self {
        Self {
            project_id: project.project_id,
            observation: ProjectObservationAuthority::from(&project.observation),
            revision: project.revision,
            registered_at_ms: project.registered_at_ms,
            favorite: project.favorite,
        }
    }
}

#[derive(Serialize)]
struct ProjectObservationAuthority<'a> {
    root_path: &'a str,
    path_identity_key: &'a [u8],
    project_type: ProjectType,
    unity_version: &'a str,
    unity_revision: Option<&'a str>,
    vpm_manifest: ManifestState,
    upm_manifest: ManifestState,
    direct_dependencies: &'a [DependencyIdentity],
    locked_dependencies: &'a [DependencyIdentity],
    issues: &'a [ReadIssue],
}

impl<'a> From<&'a ProjectObservation> for ProjectObservationAuthority<'a> {
    fn from(observation: &'a ProjectObservation) -> Self {
        Self {
            root_path: &observation.root_path,
            path_identity_key: &observation.path_identity_key,
            project_type: observation.project_type,
            unity_version: &observation.unity_version,
            unity_revision: observation.unity_revision.as_deref(),
            vpm_manifest: observation.vpm_manifest,
            upm_manifest: observation.upm_manifest,
            direct_dependencies: &observation.direct_dependencies,
            locked_dependencies: &observation.locked_dependencies,
            issues: &observation.issues,
        }
    }
}
