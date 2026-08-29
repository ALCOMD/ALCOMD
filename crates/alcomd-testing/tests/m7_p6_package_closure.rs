use serde_json::Value;

const CONTRACT: &str =
    include_str!("../../../specs/rpc/m7-p6-package-closure.proposal.schema.json");
const VECTORS: &str =
    include_str!("../../../specs/rpc/m7-p6-package-closure.contract-vectors.json");
const STATE: &str =
    include_str!("../../../specs/storage/state-v12-migration.proposal.contract.json");

fn document(text: &str) -> Value {
    serde_json::from_str(text).expect("valid contract JSON")
}

#[test]
fn p6_methods_capabilities_errors_and_source_shapes_are_exact() {
    let contract = document(CONTRACT);
    assert_eq!(contract["x-alcomd-production-wiring-created"], true);
    assert_eq!(contract["x-alcomd-config-schema"], 2);
    assert_eq!(contract["x-alcomd-state-schema"], 12);
    assert_eq!(
        contract["properties"]["methods"]["const"],
        serde_json::json!([
            "packages.planReinstall",
            "packages.planBulk",
            "packages.userPackages.list",
            "packages.userPackages.get",
            "packages.userPackages.enroll",
            "packages.userPackages.refresh",
            "packages.userPackages.remove"
        ])
    );
    assert_eq!(
        contract["properties"]["capabilities"]["const"],
        serde_json::json!([
            "packages.plan.v1",
            "packages.plan.v2",
            "packages.apply.v1",
            "packages.user-packages.v1"
        ])
    );
    assert_eq!(
        contract["properties"]["stableErrors"]["const"],
        serde_json::json!([
            "package_not_installed",
            "package_intent_conflict",
            "user_package_not_found",
            "user_package_already_enrolled",
            "user_package_source_unavailable",
            "user_package_source_unsafe",
            "user_package_source_changed",
            "user_package_manifest_invalid",
            "user_package_limit_exceeded"
        ])
    );
    let repository_required = contract["$defs"]["repositorySourcePinV1"]["required"]
        .as_array()
        .expect("repository pin required fields");
    assert!(!repository_required.iter().any(|field| field == "kind"));
    assert!(
        contract["$defs"]["userPackageSourcePinV2"]["properties"]
            .get("artifactUrl")
            .is_none()
    );
}

#[test]
fn p6_vectors_and_state_contract_freeze_limits_and_atomic_scope() {
    let vectors = document(VECTORS);
    let state = document(STATE);
    let contract = document(CONTRACT);
    let serialized = serde_json::to_string(&vectors).expect("vectors");
    for evidence in [
        "unparseable-absent",
        "source-ambiguous",
        "duplicate-intent",
        "hardlink",
        "cache-miss-no-source-fallback",
    ] {
        assert!(serialized.contains(evidence), "missing vector {evidence}");
    }
    assert_eq!(state["to"], 12);
    assert_eq!(state["transaction"], "BEGIN IMMEDIATE");
    assert_eq!(
        state["tablesAdded"],
        serde_json::json!(["user_package_sources"])
    );
    assert_eq!(
        contract["properties"]["userPackageQuotas"]["const"],
        serde_json::json!({
            "maxEntries": 65536,
            "maxSingleRegularFileBytes": 1073741824_u64,
            "maxTotalRegularBytes": 4294967296_u64,
            "maxDepth": 64,
            "maxNormalizedPathUtf8Bytes": 1024,
            "maxFinalArchiveBytes": 1073741824_u64
        })
    );
}
