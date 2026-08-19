use std::collections::BTreeSet;

use serde_json::Value;

const RESOLVER_READY_REPOSITORY: &str =
    include_str!("../fixtures/m4/repository-resolver-ready.json");
const KEY_MISMATCH_REPOSITORY: &str = include_str!("../fixtures/m4/repository-key-mismatch.json");
const VERSION_RANGE_VECTORS: &str = include_str!("../fixtures/m4/version-range-vectors.json");
const ARCHIVE_PATH_VECTORS: &str = include_str!("../fixtures/m4/archive-path-vectors.json");
const CHANGESET_GOLDEN: &str = include_str!("../fixtures/m4/changeset-golden.json");

#[test]
fn resolver_ready_fixture_has_all_m4_security_inputs() {
    let repository = json(RESOLVER_READY_REPOSITORY);
    let packages = repository["packages"].as_object().expect("packages object");
    for (package_key, package) in packages {
        let versions = package["versions"].as_object().expect("versions object");
        for (version_key, manifest) in versions {
            assert_eq!(manifest["name"], package_key.as_str());
            assert_eq!(manifest["version"], version_key.as_str());
            for required in ["displayName", "url", "zipSHA256"] {
                assert!(manifest[required].is_string(), "missing {required}");
            }
            assert!(manifest["author"]["name"].is_string());
            assert!(manifest["author"]["email"].is_string());
            let digest = manifest["zipSHA256"].as_str().expect("SHA-256 string");
            assert_eq!(digest.len(), 64);
            assert!(digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
        }
    }
}

#[test]
fn mismatch_fixture_proves_map_keys_are_not_advisory() {
    let repository = json(KEY_MISMATCH_REPOSITORY);
    let packages = repository["packages"].as_object().expect("packages object");
    let (package_key, package) = packages.iter().next().expect("one package");
    let versions = package["versions"].as_object().expect("versions object");
    let (version_key, manifest) = versions.iter().next().expect("one version");
    assert_ne!(manifest["name"], package_key.as_str());
    assert_ne!(manifest["version"], version_key.as_str());
}

#[test]
fn range_fixture_covers_every_frozen_semantic_family() {
    let fixture = json(VERSION_RANGE_VECTORS);
    assert_eq!(fixture["formatVersion"], 1);
    let categories = fixture["vectors"]
        .as_array()
        .expect("range vectors")
        .iter()
        .map(|vector| vector["category"].as_str().expect("category"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        categories,
        BTreeSet::from([
            "bare",
            "build-metadata",
            "caret",
            "comparator",
            "exact",
            "hyphen",
            "or",
            "prerelease",
            "tilde",
            "wildcard",
        ])
    );
    assert!(
        fixture["vectors"]
            .as_array()
            .expect("vectors")
            .iter()
            .any(|vector| vector["includePrerelease"] == true)
    );
    assert_eq!(
        fixture["intersections"]
            .as_array()
            .expect("intersections")
            .len(),
        2
    );
    assert!(fixture["loose"].as_array().expect("loose ranges").len() >= 3);
    assert!(fixture["invalid"].as_array().expect("invalid ranges").len() >= 2);
}

#[test]
fn archive_fixture_freezes_quotas_and_cross_platform_rejections() {
    let fixture = json(ARCHIVE_PATH_VECTORS);
    assert_eq!(fixture["limits"]["compressedBytes"], 1_073_741_824_u64);
    assert_eq!(fixture["limits"]["entries"], 65_536);
    assert_eq!(
        fixture["limits"]["entryUncompressedBytes"],
        1_073_741_824_u64
    );
    assert_eq!(
        fixture["limits"]["totalUncompressedBytes"],
        4_294_967_296_u64
    );
    assert_eq!(fixture["limits"]["depth"], 64);
    assert_eq!(fixture["limits"]["normalizedUtf8PathBytes"], 1_024);
    assert_eq!(fixture["limits"]["expansionRatio"], 1_000);

    let rejected = fixture["rejected"]
        .as_array()
        .expect("rejected paths")
        .iter()
        .map(|entry| entry["id"].as_str().expect("rejection id"))
        .collect::<BTreeSet<_>>();
    for required in [
        "parent",
        "absolute-unix",
        "drive",
        "unc",
        "device",
        "ads",
        "control",
        "windows-reserved",
        "trailing-dot",
        "trailing-space",
    ] {
        assert!(rejected.contains(required), "missing {required}");
    }
    let collisions = fixture["collisions"]
        .as_array()
        .expect("collisions")
        .iter()
        .map(|entry| entry["id"].as_str().expect("collision id"))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        collisions,
        BTreeSet::from([
            "case",
            "duplicate",
            "file-directory",
            "unicode-normalization"
        ])
    );
}

#[test]
fn changeset_golden_is_bounded_and_pins_the_complete_source() {
    let change_set = json(CHANGESET_GOLDEN);
    assert_eq!(change_set["formatVersion"], 1);
    let mutations = change_set["mutations"].as_array().expect("mutations");
    let edges = change_set["dependencyEdges"]
        .as_array()
        .expect("dependency edges");
    assert!(mutations.len() <= 1_024);
    assert!(edges.len() <= 4_096);
    let source = &mutations[0]["source"];
    for required in [
        "repositoryId",
        "repositoryRevision",
        "sourceIdentity",
        "manifestFingerprint",
        "packageId",
        "version",
        "artifactUrl",
        "archiveSha256",
    ] {
        assert!(!source[required].is_null(), "missing source pin {required}");
    }
    assert!(CHANGESET_GOLDEN.len() < 4 * 1024 * 1024);
    assert!(CHANGESET_GOLDEN.ends_with('\n'));
}

fn json(source: &str) -> Value {
    serde_json::from_str(source).expect("valid fixture JSON")
}
