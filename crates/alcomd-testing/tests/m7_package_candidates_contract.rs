use alcomd_protocol::{
    CandidateEvidence, CandidateProvider, CandidateSource, PackageSourceSelector,
    ProjectCandidatePage, ProjectCandidateRequest,
};
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

#[test]
fn candidate_sources_keep_snake_case_independently_of_p6_selectors() {
    for (kind, snake_field, camel_field, id) in [
        (
            "repository",
            "repository_id",
            "repositoryId",
            "11111111-1111-4111-8111-111111111111",
        ),
        (
            "user_package",
            "user_package_id",
            "userPackageId",
            "22222222-2222-4222-8222-222222222222",
        ),
    ] {
        let snake = serde_json::json!({"kind": kind, snake_field: id});
        let camel = serde_json::json!({"kind": kind, camel_field: id});
        let candidate: CandidateSource =
            serde_json::from_value(snake.clone()).expect("candidate snake_case source");
        assert_eq!(
            serde_json::to_value(candidate).expect("candidate encoding"),
            snake
        );
        assert!(serde_json::from_value::<CandidateSource>(camel.clone()).is_err());
        let mut mixed = snake.clone();
        mixed[camel_field] = serde_json::json!(id);
        assert!(serde_json::from_value::<CandidateSource>(mixed).is_err());

        let selector: PackageSourceSelector =
            serde_json::from_value(camel.clone()).expect("P6 camelCase selector");
        assert_eq!(serde_json::to_value(selector).expect("P6 encoding"), camel);
        assert!(serde_json::from_value::<PackageSourceSelector>(snake.clone()).is_err());

        for view in [
            serde_json::json!({
                "kind": "versions",
                "packageId": "com.example.package",
                "source": snake,
            }),
            serde_json::json!({
                "kind": "summary",
                "sources": [{"packageId": "com.example.package", "source": snake}],
            }),
        ] {
            let request = serde_json::json!({
                "projectId": "33333333-3333-4333-8333-333333333333",
                "expectedRevision": 7,
                "view": view,
            });
            let decoded: ProjectCandidateRequest =
                serde_json::from_value(request.clone()).expect("nested candidate source");
            assert_eq!(
                serde_json::to_value(decoded).expect("request encoding"),
                request
            );
            let mut foreign = request;
            if view["kind"] == "versions" {
                foreign["view"]["source"] = camel.clone();
            } else {
                foreign["view"]["sources"][0]["source"] = camel.clone();
            }
            assert!(serde_json::from_value::<ProjectCandidateRequest>(foreign).is_err());
        }

        let provider = serde_json::json!({"source": snake, "sourceRevision": 3});
        let decoded: CandidateProvider =
            serde_json::from_value(provider.clone()).expect("candidate provider source");
        assert_eq!(
            serde_json::to_value(decoded).expect("provider encoding"),
            provider
        );
        let mut foreign_provider = provider;
        foreign_provider["source"] = camel.clone();
        assert!(serde_json::from_value::<CandidateProvider>(foreign_provider).is_err());

        let evidence = serde_json::json!({
            "version": "1.0.0",
            "source": snake,
            "sourceRevision": 3,
            "classification": "stable",
            "relation": "newer",
            "unity": "compatible",
            "eligibility": "eligible",
            "reasons": [],
        });
        let decoded: CandidateEvidence =
            serde_json::from_value(evidence.clone()).expect("candidate evidence source");
        assert_eq!(
            serde_json::to_value(decoded).expect("evidence encoding"),
            evidence
        );
        let mut foreign_evidence = evidence;
        foreign_evidence["source"] = camel;
        assert!(serde_json::from_value::<CandidateEvidence>(foreign_evidence).is_err());
    }
}
