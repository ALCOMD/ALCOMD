use super::{
    AccessContext, ConnectionState, DispatchAction, ProjectCandidateApplication, error_action,
    require_capability, success_action,
};
use alcomd_application as app;
use alcomd_protocol as rpc;

pub(super) fn oversized_candidate_frame(method: &str, payload: &[u8]) -> bool {
    method == rpc::METHOD_PACKAGES_QUERY_PROJECT_CANDIDATES && payload.len() > 64 * 1024
}
fn response_fits(response: &serde_json::Value) -> bool {
    serde_json::to_vec(response).is_ok_and(|bytes| bytes.len() <= 1024 * 1024)
}

pub(super) async fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    application: &ProjectCandidateApplication,
    access: &AccessContext,
) -> DispatchAction {
    if let Some(action) =
        require_capability(&request.id, state, rpc::CAPABILITY_PACKAGES_CANDIDATES_V1)
    {
        return action;
    }
    if serde_json::to_vec(&request).map_or(true, |bytes| bytes.len() > 64 * 1024) {
        return failure(request.id, app::CandidateQueryError::LimitExceeded);
    }
    let params: rpc::ProjectCandidateRequest = match serde_json::from_value(request.params) {
        Ok(value) => value,
        Err(_) => return super::invalid(request.id),
    };
    // Explicit public DTO boundary; both closed shapes are contract-tested independently.
    let params: app::ProjectCandidateRequest =
        match serde_json::to_value(params).and_then(serde_json::from_value) {
            Ok(value) => value,
            Err(_) => return super::invalid(request.id),
        };
    match application.query(access, params).await {
        Ok(page) => {
            let page: rpc::ProjectCandidatePage =
                match serde_json::to_value(page).and_then(serde_json::from_value) {
                    Ok(value) => value,
                    Err(_) => return failure(request.id, app::CandidateQueryError::Unavailable),
                };
            let action = success_action(request.id.clone(), page, None);
            // Bound the complete successful envelope, reserving its actual ID/metadata bytes.
            if !response_fits(&action.response) {
                return failure(request.id, app::CandidateQueryError::LimitExceeded);
            }
            action
        }
        Err(error) => failure(request.id, error),
    }
}

pub(super) fn failure(id: String, error: app::CandidateQueryError) -> DispatchAction {
    let error = match error {
        app::CandidateQueryError::InvalidInput => rpc::RpcError::invalid_request(),
        app::CandidateQueryError::PermissionDenied => rpc::RpcError::permission_denied(),
        app::CandidateQueryError::RevisionConflict => rpc::RpcError::revision_conflict(),
        app::CandidateQueryError::Unavailable => {
            rpc::RpcError::internal(app::OperationId::new().to_string())
        }
        other => {
            let (code, message) = match other {
                app::CandidateQueryError::EvidenceStale => (
                    rpc::error_code::PACKAGE_CANDIDATE_EVIDENCE_STALE,
                    "Package candidate evidence changed; reload the snapshot.",
                ),
                app::CandidateQueryError::LimitExceeded => (
                    rpc::error_code::PACKAGE_CANDIDATE_LIMIT_EXCEEDED,
                    "Package candidate query exceeds its bounded quota.",
                ),
                app::CandidateQueryError::ProjectNotRegistered => {
                    ("project_not_registered", "The project is not registered.")
                }
                app::CandidateQueryError::ProjectManifestInvalid => (
                    "project_manifest_invalid",
                    "The project observation is invalid.",
                ),
                _ => unreachable!(),
            };
            rpc::RpcError {
                code: code.to_owned(),
                message: message.to_owned(),
                diagnostic_id: None,
                data: None,
            }
        }
    };
    error_action(Some(id), error, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_whitespace_and_escape_bytes_cannot_bypass_candidate_request_cap() {
        let compact = br#"{"id":"1","method":"packages.queryProjectCandidates","params":{}}"#;
        let mut padded = vec![b' '; 64 * 1024];
        padded.extend(compact);
        let parsed: serde_json::Value = serde_json::from_slice(&padded).expect("valid padded JSON");
        assert!(serde_json::to_vec(&parsed).expect("serialize").len() < 100);
        assert!(oversized_candidate_frame(
            rpc::METHOD_PACKAGES_QUERY_PROJECT_CANDIDATES,
            &padded
        ));
        let escaped = format!("{{\"text\":\"{}\"}}", "\\u0061".repeat(12_000));
        let parsed: serde_json::Value = serde_json::from_str(&escaped).expect("valid escaped JSON");
        assert!(serde_json::to_vec(&parsed).expect("serialize").len() < 64 * 1024);
        assert!(oversized_candidate_frame(
            rpc::METHOD_PACKAGES_QUERY_PROJECT_CANDIDATES,
            escaped.as_bytes()
        ));
        assert!(!oversized_candidate_frame("projects.get", &padded));
    }
    #[test]
    fn actual_response_envelope_and_json_escaping_count_toward_limit() {
        let result = "a".repeat(1024 * 1024 - 10);
        assert!(serde_json::to_vec(&result).expect("serialize result").len() < 1024 * 1024);
        let action = success_action("i".repeat(64), result, None);
        assert!(!response_fits(&action.response));
        let action = success_action("1".into(), "\\".repeat(600_000), None);
        assert!(!response_fits(&action.response));
        assert!(response_fits(
            &success_action("1".into(), vec![1, 2, 3], None).response
        ));
    }
}
