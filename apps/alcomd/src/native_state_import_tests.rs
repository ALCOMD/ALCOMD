use super::*;
use serde_json::json;
use std::fs;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = fs::canonicalize(std::env::temp_dir()).expect("temporary directory");
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).expect("fixture entropy");
        let root = root.join(format!(
            "alcomd-native-import-{:032x}",
            u128::from_ne_bytes(random)
        ));
        fs::create_dir(&root).expect("fresh fixture");
        Self(root)
    }

    fn bundle(&self) -> PathBuf {
        let project = self.0.join("Source Project");
        for subtree in ["Assets", "Packages", "ProjectSettings"] {
            fs::create_dir_all(project.join(subtree)).expect("fixture project directory");
        }
        fs::write(
            project.join("ProjectSettings/ProjectVersion.txt"),
            "m_EditorVersion: 2022.3.22f1\n",
        )
        .expect("fixture Unity version");
        fs::write(
            project.join("Packages/manifest.json"),
            br#"{"dependencies":{}}"#,
        )
        .expect("fixture Unity package manifest");
        let state = json!({
            "schema": 1, "sourceVersion": "3.4.0",
            "projects": [{"path": project, "favorite": true, "registeredAtMs": 1234, "unityLaunchArguments": ["-logFile", "Launcher Output.log"]}],
            "unityInstallations": [],
            "repositories": [{"source": {"kind": "remote", "url": "https://example.invalid/vpm.json"},
                "declaredId": "com.example.fixture", "displayName": "Offline fixture", "hidden": true, "hasCredentials": false}],
            "localPackages": [], "templates": [], "extensions": [],
            "discord": {"enabled": false, "projectName": true, "unityVersion": true, "editorCount": true, "customText": ""},
            "settings": {"locale": "zh-CN", "appearanceMode": "dark", "appearanceMotion": "reduced", "fullscreen": false,
                "handleVccLinks": true, "showPrerelease": false, "hideLocalUserPackages": false,
                "hideOfficialRepository": true, "hideCuratedRepository": false, "backupCompression": "fast",
                "favoriteTemplateIds": []}
        });
        let typed =
            serde_json::from_value::<NativeState>(state.clone()).expect("valid native fixture");
        let bytes = serde_json::to_vec(&state).expect("encode fixture state");
        let manifest = json!({
            "schema": 1, "sourceVersion": "3.4.0", "createdAt": "2026-11-21T00:00:00Z",
                "records": typed.counts(), "files": [{"path": "state.json", "size": bytes.len(), "sha256": digest_hex(&bytes)}]
        });
        let bundle = self.0.join("bundle");
        fs::create_dir(&bundle).expect("fixture bundle root");
        fs::write(bundle.join("state.json"), bytes).expect("write fixture state");
        fs::write(
            bundle.join("manifest.json"),
            serde_json::to_vec(&manifest).expect("encode fixture manifest"),
        )
        .expect("write fixture manifest");
        bundle
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn restores_into_a_new_state_and_reconciles_after_reopen() {
    let fixture = Fixture::new();
    let bundle = fixture.bundle();
    let destination = fixture.0.join("native state");
    let imported = import_native_state(&bundle, &destination, None)
        .await
        .expect("import native state");
    assert_eq!(
        imported,
        check_native_state(&bundle, &destination, None)
            .await
            .expect("reconcile reopened state")
    );
    assert_eq!(imported["dataSchema"], 46);
    assert_eq!(imported["configSchema"], 13);
    let settings =
        fs::read_to_string(destination.join("config/settings.toml")).expect("imported settings");
    assert!(settings.contains("show_prerelease = false"));
    assert!(settings.contains("prefer_prerelease = false"));
    assert_eq!(imported["records"]["projects"], 1);
    assert_eq!(imported["records"]["repositories"], 1);
    assert!(
        import_native_state(&bundle, &destination, None)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn rejects_corrupt_bundle_before_creating_a_stage() {
    let fixture = Fixture::new();
    let bundle = fixture.bundle();
    fs::write(bundle.join("state.json"), b"{}").expect("corrupt the fixture state");
    let destination = fixture.0.join("native state");
    assert!(
        import_native_state(&bundle, &destination, None)
            .await
            .is_err()
    );
    assert!(!destination.exists());
}

#[test]
fn detects_nonempty_unrecognized_state_directory_without_modification() {
    let root = Fixture::new();
    let file = root.0.join("unrecognized.txt");
    fs::write(&file, b"preserved").expect("fixture sentinel");
    assert!(crate::preflight_data_layout(&root.0.join("state.db")).is_err());
    assert_eq!(fs::read(file).expect("preserved sentinel"), b"preserved");
    assert_eq!(
        fs::read_dir(&root.0).expect("preserved directory").count(),
        1
    );
}
