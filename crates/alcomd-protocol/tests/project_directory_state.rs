use alcomd_protocol::{ProjectDirectoryState, ProjectSnapshot};
use serde_json::json;

#[test]
fn directory_state_keeps_unobserved_snapshots_and_accepts_only_the_approved_states() {
    let snapshot = json!({
        "projectId": null,
        "rootPath": "C:/Projects/Example",
        "projectType": "avatars",
        "unityVersion": "2022.3.22f1",
        "unityRevision": null,
        "vpmManifest": "valid",
        "upmManifest": "valid",
        "directDependencies": [],
        "lockedDependencies": [],
        "issues": [],
        "revision": null
    });
    let unobserved: ProjectSnapshot =
        serde_json::from_value(snapshot.clone()).expect("retained snapshot");
    assert_eq!(unobserved.directory_state, None);
    assert!(
        serde_json::to_value(unobserved)
            .expect("serialize")
            .get("directoryState")
            .is_none()
    );
    for (wire, state) in [
        ("available", ProjectDirectoryState::Available),
        ("missing", ProjectDirectoryState::Missing),
        ("unavailable", ProjectDirectoryState::Unavailable),
    ] {
        let mut value = snapshot.clone();
        value["directoryState"] = json!(wire);
        assert_eq!(
            serde_json::from_value::<ProjectSnapshot>(value)
                .expect("approved state")
                .directory_state,
            Some(state)
        );
    }
    for invalid in [json!(null), json!("unknown"), json!(false)] {
        let mut value = snapshot.clone();
        value["directoryState"] = invalid;
        assert!(serde_json::from_value::<ProjectSnapshot>(value).is_err());
    }
}
