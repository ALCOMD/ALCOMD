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
        self.bundle_with_templates(Vec::new())
    }

    fn bundle_with_templates(&self, templates: Vec<serde_json::Value>) -> PathBuf {
        self.bundle_at("bundle", templates, Some(6789))
    }

    fn bundle_at(
        &self,
        folder: &str,
        templates: Vec<serde_json::Value>,
        activity: Option<u64>,
    ) -> PathBuf {
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
            "projects": [{"path": project, "favorite": true, "registeredAtMs": 1234, "lastModifiedAtMs": activity, "unityLaunchArguments": ["-logFile", "Launcher Output.log"]}],
            "unityInstallations": [],
            "repositories": [{"source": {"kind": "remote", "url": "https://example.invalid/vpm.json"},
                "declaredId": "com.example.fixture", "displayName": "Offline fixture", "hidden": true, "hasCredentials": false}],
            "localPackages": [], "templates": templates, "extensions": [],
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
        let bundle = self.0.join(folder);
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

fn manual_template(id: &str, base: &str, package: &str) -> serde_json::Value {
    json!({
        "id": id, "name": id, "source": {
            "kind": "manual", "baseTemplate": base, "unityRange": "2022.3.22f1",
            "dependencies": {(package): "1.0.0"}, "unityPackages": []
        }
    })
}

#[tokio::test]
async fn imports_derived_templates_as_independent_archives_regardless_of_bundle_order() {
    let fixture = Fixture::new();
    let child = "dev.example.child";
    let parent = "dev.example.parent";
    let bundle = fixture.bundle_with_templates(vec![
        manual_template(child, parent, "dev.example.extra"),
        manual_template(parent, app::BLANK_PROJECT_TEMPLATE_ID, "dev.example.shared"),
    ]);
    let original = fs::read(bundle.join("state.json")).expect("original transfer");
    let destination = fixture.0.join("native");
    let receipt = import_native_state(&bundle, &destination, None)
        .await
        .expect("import chain");
    assert_eq!(
        receipt,
        check_native_state(&bundle, &destination, None)
            .await
            .expect("reopen")
    );
    let data = alcomd_platform::DataConfig::isolated(destination);
    let store = alcomd_store::StateStoreHandle::open(
        alcomd_platform::state_database_path(&data).expect("state path"),
    )
    .expect("staged registry");
    let record = store
        .project_template_get(app::PrincipalId::local_owner(), child.into())
        .await
        .expect("derived archive");
    assert_eq!(
        record.declaration.editor_version.as_deref(),
        Some("2022.3.22f1")
    );
    let packages: BTreeSet<_> = record
        .declaration
        .dependencies
        .iter()
        .map(|dep| dep.package_id.as_str())
        .collect();
    assert_eq!(
        packages,
        BTreeSet::from(["dev.example.extra", "dev.example.shared"])
    );
    let base = store
        .project_template_get(app::PrincipalId::local_owner(), parent.into())
        .await
        .expect("base archive");
    assert_ne!(
        record.declaration.archive_sha256,
        base.declaration.archive_sha256
    );
    assert_eq!(
        fs::read(bundle.join("state.json")).expect("transfer bytes"),
        original
    );
}

#[tokio::test]
async fn rejects_duplicate_or_cyclic_templates_before_creating_a_stage() {
    let a = "dev.example.a";
    let b = "dev.example.b";
    for (templates, expected) in [
        (
            vec![manual_template(a, "", "dev.example.one")],
            "requires a base",
        ),
        (vec![manual_template(a, a, "dev.example.one")], "cyclic"),
        (
            vec![
                manual_template(a, b, "dev.example.one"),
                manual_template(b, a, "dev.example.two"),
            ],
            "cyclic",
        ),
        (
            vec![
                manual_template(a, app::BLANK_PROJECT_TEMPLATE_ID, "dev.example.one"),
                manual_template(a, app::BLANK_PROJECT_TEMPLATE_ID, "dev.example.two"),
            ],
            "duplicate",
        ),
    ] {
        let fixture = Fixture::new();
        let bundle = fixture.bundle_with_templates(templates);
        let original = fs::read(bundle.join("state.json")).expect("source transfer");
        let manifest = fs::read(bundle.join("manifest.json")).expect("source manifest");
        let destination = fixture.0.join("native");
        let error = import_native_state(&bundle, &destination, None)
            .await
            .expect_err("invalid dependency graph");
        assert!(error.to_string().contains(expected), "{error}");
        assert!(!destination.exists());
        assert_eq!(
            fs::read(bundle.join("state.json")).expect("preserved state"),
            original
        );
        assert_eq!(
            fs::read(bundle.join("manifest.json")).expect("preserved manifest"),
            manifest
        );
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
    assert_eq!(imported["dataSchema"], 1);
    assert_eq!(imported["configSchema"], 1);
    let settings =
        fs::read_to_string(destination.join("config/settings.toml")).expect("imported settings");
    assert!(settings.contains("show_prerelease = false"));
    assert!(settings.contains("prefer_prerelease = false"));
    assert_eq!(imported["records"]["projects"], 1);
    let reopened = alcomd_store::StateStoreHandle::open(destination.join("state.db"))
        .expect("reopen the staged project registry");
    let records = reopened
        .list_projects(app::PrincipalId::local_owner(), None, 2)
        .await
        .expect("read stored project times");
    assert_eq!(records.projects[0].registered_at_ms, 1234);
    assert_eq!(records.projects[0].last_modified_at_ms, Some(6789));
    drop(reopened);
    let without_activity = fixture.bundle_at("unknown-activity", Vec::new(), None);
    let mismatch = check_native_state(&without_activity, &destination, None)
        .await
        .expect_err("activity differs");
    assert!(
        mismatch
            .to_string()
            .contains("project activity time differs")
    );
    let unknown = fixture.0.join("state without activity");
    import_native_state(&without_activity, &unknown, None)
        .await
        .expect("preserve unknown activity");
    check_native_state(&without_activity, &unknown, None)
        .await
        .expect("reconcile unknown activity");
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
