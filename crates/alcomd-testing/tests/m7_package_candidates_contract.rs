use alcomd_protocol::{ProjectCandidatePage, ProjectCandidateRequest};
use serde_json::Value;

const SCHEMA: &str = include_str!("../../../specs/rpc/m7-package-candidates.proposal.schema.json");
const VECTORS: &str =
    include_str!("../../../specs/rpc/m7-package-candidates.contract-vectors.json");

#[test]
fn approved_candidate_wire_shapes_round_trip_through_production_dtos() {
    let vectors: Value = serde_json::from_str(VECTORS).expect("candidate vectors");
    for case in vectors["shapeCases"].as_array().expect("shape cases") {
        if case["valid"] != true {
            continue;
        }
        let value = case["value"].clone();
        match case["definition"].as_str().expect("definition") {
            "Request" => {
                let request: ProjectCandidateRequest = serde_json::from_value(value)
                    .unwrap_or_else(|error| panic!("{}: {error}", case["id"]));
                let encoded = serde_json::to_value(request).expect("request encoding");
                // Optional/default field serialization need not reproduce omitted keys.
                for (name, expected) in case["value"].as_object().expect("request") {
                    assert_eq!(&encoded[name], expected, "{} / {name}", case["id"]);
                }
            }
            "Result" => {
                let result: ProjectCandidatePage = serde_json::from_value(value.clone())
                    .unwrap_or_else(|error| panic!("{}: {error}", case["id"]));
                assert_eq!(
                    serde_json::to_value(result).expect("result encoding"),
                    value
                );
            }
            other => panic!("unexpected definition {other}"),
        }
    }
}

#[test]
fn candidate_public_contract_is_additive_and_permission_names_are_reused() {
    let schema: Value = serde_json::from_str(SCHEMA).expect("candidate schema");
    assert_eq!(schema["x-alcomd-approval"], "owner-approved");
    assert_eq!(schema["x-alcomd-breaking-change"], false);
    assert_eq!(
        schema["x-alcomd-method"],
        alcomd_protocol::METHOD_PACKAGES_QUERY_PROJECT_CANDIDATES
    );
    assert_eq!(
        schema["x-alcomd-capability"],
        alcomd_protocol::CAPABILITY_PACKAGES_CANDIDATES_V1
    );
    assert_eq!(
        schema["x-alcomd-method-permissions"]["packages.queryProjectCandidates"],
        serde_json::json!([
            "projects.read",
            "repositories.read",
            "packages.read",
            "settings.read"
        ])
    );
    assert_eq!(
        schema["x-alcomd-new-errors"],
        serde_json::json!([
            "package_candidate_evidence_stale",
            "package_candidate_limit_exceeded"
        ])
    );
    let vectors: Value = serde_json::from_str(VECTORS).expect("candidate vectors");
    assert_eq!(
        vectors["semanticVectors"]
            .as_array()
            .expect("semantic vectors")
            .len(),
        39
    );
}

#[test]
fn candidate_request_rejects_unknown_fields_and_foreign_source_shape() {
    for params in [
        serde_json::json!({"projectId":"p","expectedRevision":1,"view":{"kind":"summary"},"refresh":true}),
        serde_json::json!({"projectId":"p","view":{"kind":"summary"}}),
        serde_json::json!({"projectId":"p","expectedRevision":1,"view":{"kind":"versions","packageId":"com.example","source":{"kind":"user_package","path":"private"}}}),
    ] {
        assert!(serde_json::from_value::<ProjectCandidateRequest>(params).is_err());
    }
}

#[test]
fn candidate_response_rejects_unknown_top_level_and_snapshot_fields() {
    let vectors: Value = serde_json::from_str(VECTORS).expect("candidate vectors");
    let page = vectors["shapeCases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["id"] == "summary-target")
        .expect("summary case")["value"]
        .clone();
    let mut unknown_top = page.clone();
    unknown_top["unexpected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<ProjectCandidatePage>(unknown_top).is_err());
    let mut unknown_snapshot = page;
    unknown_snapshot["snapshot"]["unexpected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<ProjectCandidatePage>(unknown_snapshot).is_err());
}
