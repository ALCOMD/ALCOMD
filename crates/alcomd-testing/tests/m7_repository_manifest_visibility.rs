//! Repository consumption must not use optional presentation fields as install authority.
use std::{
    collections::BTreeSet,
    path::Path,
    sync::{Arc, Mutex},
};

use alcomd_application::*;
use alcomd_store::StateStoreHandle;
use alcomd_vpm::{ProjectCandidateEngine, VpmReader};
use serde_json::{Value, json};

fn key(value: impl Into<String>) -> IdempotencyKey {
    IdempotencyKey::parse(value).expect("test key")
}

fn settings() -> ConfigPackageSettings {
    ConfigPackageSettings {
        show_prerelease: true,
        hidden_repository_ids: Vec::new(),
        hidden_repository_packages: Vec::new(),
        hide_local_user_packages: false,
    }
}

async fn index_document(root: &Path, path: &Path) -> Value {
    let reader = VpmReader::new().expect("reader");
    let RepositoryReadOutcome::Fresh(observation) = reader
        .inspect_repository(
            RepositorySource::Local {
                path: path.to_string_lossy().into_owned(),
            },
            None,
        )
        .await
        .expect("bounded real repository parse")
    else {
        panic!("fresh local document")
    };
    let rejected = observation
        .packages
        .iter()
        .filter(|row| row.resolver.is_none())
        .map(|row| format!("{}@{}", row.package_id, row.version))
        .collect::<Vec<_>>();
    let ready = observation
        .packages
        .iter()
        .filter(|row| row.resolver.is_some())
        .map(|row| (row.package_id.clone(), row.version.clone()))
        .collect::<BTreeSet<_>>();
    let issues = observation.issues.clone();
    let store = StateStoreHandle::open(root.join(format!("{}.db", uuid::Uuid::new_v4())))
        .expect("isolated store");
    let owner = PrincipalId::local_owner();
    store
        .register_repository(owner.clone(), observation, key("repository"), 1)
        .await
        .expect("persist metadata");
    let settings = settings();
    let catalog = store
        .resolver_catalog(owner.clone(), false, settings.clone())
        .await
        .expect("resolver catalog");
    assert!(catalog.complete);
    assert_eq!(
        alcomd_vpm::candidates_from_catalog(&catalog)
            .expect("resolver candidates")
            .len(),
        ready.len()
    );
    let project_path = root.join("Project");
    std::fs::create_dir_all(project_path.join("Packages")).expect("packages");
    std::fs::create_dir_all(project_path.join("ProjectSettings")).expect("settings");
    std::fs::write(
        project_path.join("ProjectSettings/ProjectVersion.txt"),
        "m_EditorVersion: 2022.3.22f1\n",
    )
    .expect("Unity version");
    std::fs::write(
        project_path.join("Packages/manifest.json"),
        r#"{"dependencies":{}}"#,
    )
    .expect("UPM");
    std::fs::write(
        project_path.join("Packages/vpm-manifest.json"),
        r#"{"dependencies":{},"locked":{}}"#,
    )
    .expect("VPM");
    let project = reader
        .inspect_project(
            project_path.to_string_lossy().into_owned(),
            ProjectDiscoveryMode::ExactRoot,
        )
        .await
        .expect("project");
    let registered = store
        .register_project(owner.clone(), project, key("project"), 1)
        .await
        .expect("register project")
        .value;
    let snapshot = store
        .candidate_snapshot(owner, registered.project_id, None, settings.clone())
        .await
        .expect("management snapshot");
    let actual = snapshot
        .entries
        .iter()
        .map(|row| (row.package_id.clone(), row.version.clone()))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual, ready,
        "every indexed version reaches package management"
    );
    assert!(snapshot.entries.iter().all(|row| row.metadata_ready));
    let input = CandidateEvidenceInput {
        project: snapshot.project.observation,
        entries: snapshot.entries,
        source_metadata: snapshot.source_metadata,
        settings,
        installed_presentations: Default::default(),
    };
    let summary = ProjectCandidateEngine.summarize(&input, None, &[]);
    let package_ids = ready
        .iter()
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    let visible_ids = input
        .entries
        .iter()
        .filter(|entry| {
            !entry.yanked
                && !entry
                    .presentation
                    .as_ref()
                    .is_some_and(|metadata| metadata.yanked)
        })
        .map(|entry| entry.package_id.clone())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        summary
            .iter()
            .map(|row| row.package_id.clone())
            .collect::<BTreeSet<_>>(),
        visible_ids,
        "uninstalled withdrawn packages stay hidden"
    );
    json!({"file":path.file_name().expect("file").to_string_lossy(),"readyVersions":ready.len(),"readyPackages":package_ids.len(),"rejected":rejected,"issues":issues})
}

#[tokio::test]
async fn real_public_repository_packages_reach_management_and_resolver() {
    let root = std::env::temp_dir().join(format!(
        "alcomd-manifest-visibility-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).expect("fixture directory");
    for (name, bytes, count) in [
        (
            "nadena",
            include_bytes!("../fixtures/m7/repository-visibility/nadena.json").as_slice(),
            4,
        ),
        (
            "lilxyzw",
            include_bytes!("../fixtures/m7/repository-visibility/lilxyzw.json").as_slice(),
            11,
        ),
        (
            "vrm-converter",
            include_bytes!("../fixtures/m7/repository-visibility/vrm-converter.json").as_slice(),
            1,
        ),
    ] {
        let path = root.join(format!("{name}.json"));
        std::fs::write(&path, bytes).expect("frozen public data");
        let result = index_document(&root, &path).await;
        assert_eq!(result["readyPackages"], count);
        assert_eq!(result["readyVersions"], count);
        assert_eq!(result["rejected"], json!([]));
    }
}

#[tokio::test]
async fn missing_and_invalid_presentation_fields_do_not_hide_valid_minimal_packages() {
    let root =
        std::env::temp_dir().join(format!("alcomd-minimal-manifest-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("directory");
    let path = root.join("minimal.json");
    std::fs::write(&path, json!({"packages":{
        "com.example.minimal":{"versions":{"1.0.0":{"name":"com.example.minimal","version":"1.0.0","url":"https://example.invalid/minimal.zip"}}},
        "com.example.presentation":{"versions":{"1.0.0":{"name":"com.example.presentation","version":"1.0.0","url":"https://example.invalid/presentation.zip","displayName":null,"description":false,"author":"Author <dev@example.invalid>"}}}
    }}).to_string()).expect("fixture");
    let result = index_document(&root, &path).await;
    assert_eq!(result["readyPackages"], 2);
    assert_eq!(result["rejected"], json!([]));
    assert!(
        result["issues"]
            .as_array()
            .expect("issues")
            .iter()
            .all(|issue| issue["code"] != "resolver_metadata_invalid")
    );
}

#[tokio::test]
#[ignore = "requires an explicitly supplied directory of bounded repository snapshots"]
async fn audit_public_repository_snapshots() {
    let corpus = std::env::var_os("ALCOMD_REPOSITORY_AUDIT_DIR").expect("explicit audit directory");
    let corpus = Path::new(&corpus);
    let root = std::env::temp_dir().join(format!("alcomd-manifest-audit-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("audit directory");
    let mut files = std::fs::read_dir(corpus)
        .expect("corpus")
        .map(|entry| entry.expect("file").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect::<Vec<_>>();
    files.sort();
    assert!(!files.is_empty());
    let mut results = Vec::new();
    for path in files {
        results.push(index_document(&root, &path).await);
    }
    std::fs::write(
        corpus
            .parent()
            .expect("evidence directory")
            .join("audit-result.json"),
        serde_json::to_vec_pretty(&results).expect("results"),
    )
    .expect("audit evidence");
}

#[derive(Clone)]
struct ValidatorReader {
    fresh: RepositoryObservation,
    seen: Arc<Mutex<Vec<Option<RepositoryValidators>>>>,
}
impl M3ReadAdapter for ValidatorReader {
    fn normalize_remote_repository_url(&self, url: &str) -> Result<String, M3Error> {
        Ok(url.to_owned())
    }
    async fn inspect_project(
        &self,
        _: String,
        _: ProjectDiscoveryMode,
    ) -> Result<ProjectObservation, M3Error> {
        panic!("no project inspection")
    }
    async fn inspect_repository(
        &self,
        _: RepositorySource,
        validators: Option<RepositoryValidators>,
    ) -> Result<RepositoryReadOutcome, M3Error> {
        self.seen
            .lock()
            .expect("observations")
            .push(validators.clone());
        Ok(match validators {
            Some(validators) => RepositoryReadOutcome::NotModified(validators),
            None => RepositoryReadOutcome::Fresh(self.fresh.clone()),
        })
    }
}

#[tokio::test]
async fn partial_indexes_are_reparsed_while_healthy_indexes_keep_conditional_requests() {
    let root =
        std::env::temp_dir().join(format!("alcomd-manifest-reparse-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("directory");
    let path = root.join("repository.json");
    std::fs::write(
        &path,
        include_bytes!("../fixtures/m7/repository-visibility/lilxyzw.json"),
    )
    .expect("fixture");
    let RepositoryReadOutcome::Fresh(mut fresh) = VpmReader::new()
        .expect("reader")
        .inspect_repository(
            RepositorySource::Local {
                path: path.to_string_lossy().into_owned(),
            },
            None,
        )
        .await
        .expect("fixture")
    else {
        panic!("fresh")
    };
    fresh.validators.etag = Some("unchanged-public-source".into());
    let mut old = fresh.clone();
    for row in &mut old.packages {
        row.resolver = None;
    }
    old.issues.push(ReadIssue {
        code: "resolver_metadata_invalid".into(),
        component: "version".into(),
        item: "jp.lilxyzw.liltoon@2.4.1".into(),
        line: None,
        column: None,
    });
    let store = StateStoreHandle::open(root.join("state.db")).expect("store");
    let access = AccessContext::local_owner();
    let original = store
        .register_repository(access.principal().clone(), old, key("old"), 1)
        .await
        .expect("old snapshot")
        .value;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let app = M3Application::new(
        store.clone(),
        ValidatorReader {
            fresh,
            seen: seen.clone(),
        },
    );
    let updated = app
        .refresh_repository(
            &access,
            original.repository_id,
            original.revision,
            key("repair"),
        )
        .await
        .expect("repair")
        .value;
    assert_eq!(
        store
            .resolver_catalog(access.principal().clone(), false, settings())
            .await
            .expect("catalog")
            .entries
            .len(),
        11
    );
    app.refresh_repository(
        &access,
        updated.repository_id,
        updated.revision,
        key("conditional"),
    )
    .await
    .expect("healthy conditional refresh");
    let seen = seen.lock().expect("requests");
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0], None);
    assert_eq!(
        seen[1].as_ref().and_then(|value| value.etag.as_deref()),
        Some("unchanged-public-source")
    );
}
