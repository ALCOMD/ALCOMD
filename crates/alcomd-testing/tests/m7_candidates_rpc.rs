use std::collections::BTreeMap;
use std::fs;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use alcomd_client::{AlcomdClient, ClientConfig, ClientError};
use alcomd_platform::{DataConfig, IpcConfig};
use alcomd_protocol::ProjectCandidateRequest;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn request(project_id: &str, revision: u64, view: Value, limit: u32) -> ProjectCandidateRequest {
    serde_json::from_value(json!({
        "projectId":project_id, "expectedRevision":revision,"view":view,"limit":limit
    }))
    .expect("query request")
}
fn wire(value: impl serde::Serialize) -> Value {
    serde_json::to_value(value).expect("serialize response")
}
fn remote_code(error: ClientError) -> String {
    match error {
        ClientError::Remote(error) => error.code,
        other => panic!("expected remote error, got {other:?}"),
    }
}
fn fingerprints(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut result = BTreeMap::new();
    if !root.exists() {
        return result;
    }
    for entry in fs::read_dir(root).expect("read fixture directory") {
        let entry = entry.expect("fixture entry");
        let path = entry.path();
        if path.is_dir() {
            result.extend(fingerprints(&path));
        } else if path.extension().is_none_or(|ext| ext != "log")
            && !path
                .file_name()
                .expect("file name")
                .to_string_lossy()
                .ends_with("-shm")
        {
            result.insert(
                path.clone(),
                Sha256::digest(fs::read(path).expect("fixture bytes")).to_vec(),
            );
        }
    }
    result
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn candidate_rpc_registered_snapshots_zero_effects_paging_and_plan_authority() {
    let fixture = Fixture::new();
    let data = fixture.0.join("data");
    let project = fixture.0.join("Project");
    let runtime = fixture.0.join("runtime");
    for path in [
        &data,
        &runtime,
        &project.join("Packages"),
        &project.join("ProjectSettings"),
    ] {
        fs::create_dir_all(path).expect("fixture directory");
    }
    fs::write(
        project.join("ProjectSettings/ProjectVersion.txt"),
        "m_EditorVersion: 2022.3.22f1\n",
    )
    .expect("Unity version");
    fs::write(
        project.join("Packages/manifest.json"),
        "{\"dependencies\":{}}",
    )
    .expect("UPM manifest");
    fs::write(
        project.join("Packages/vpm-manifest.json"),
        json!({
            "dependencies":{"com.example.candidate":"1.9.0"},
            "locked":{"com.example.candidate":{"version":"1.9.0"}}
        })
        .to_string(),
    )
    .expect("VPM manifest");
    fs::create_dir_all(project.join("Packages/com.example.candidate"))
        .expect("installed package directory");
    fs::write(
        project.join("Packages/com.example.candidate/package.json"),
        json!({"name":"com.example.candidate","version":"1.9.0"}).to_string(),
    )
    .expect("installed manifest");
    let network = TcpListener::bind("127.0.0.1:0").expect("network tripwire");
    network.set_nonblocking(true).expect("nonblocking tripwire");
    let archive_url = format!(
        "https://{}/must-not-download.zip",
        network.local_addr().expect("tripwire address")
    );
    let mut versions = serde_json::Map::new();
    for (version, unity) in [
        ("1.9.0", "2022.3"),
        ("1.10.0", "2022.3"),
        ("2.0.0", "2023.1"),
    ] {
        versions.insert(
            version.to_owned(),
            json!({"name":"com.example.candidate","displayName":"Candidate","version":version,
            "unity":unity,"url":archive_url,"zipSHA256":"a".repeat(64),
            "author":{"name":"Candidate fixture","email":"fixture@example.invalid"},
                "vpmDependencies":{"com.example.missing":"3.0.0"}}),
        );
    }
    let repository_path = fixture.0.join("repository.json");
    fs::write(
        &repository_path,
        json!({"id":"candidate-fixture","name":"Candidate fixture",
        "packages":{"com.example.candidate":{"versions":versions}}})
        .to_string(),
    )
    .expect("repository");
    alcomd_vpm::parse_resolver_ready_repository(
        &fs::read(&repository_path).expect("repository bytes"),
        &alcomd_vpm::RepositoryPackageContext {
            repository_id: "fixture-repository".to_owned(),
            repository_revision: 1,
            priority: 1,
            source_identity: "fixture".to_owned(),
        },
    )
    .expect("fixture must be resolver ready before testing candidate semantics");
    let (ipc, config) = isolated_ipc(runtime);
    let shutdown = Arc::new(AtomicBool::new(false));
    let stop = shutdown.clone();
    let daemon_data = data.clone();
    let daemon = tokio::spawn(async move {
        alcomd_daemon::serve_with_data_until(ipc, DataConfig::isolated(daemon_data), async move {
            while !stop.load(Ordering::Acquire) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
    });
    let mut client = connect(config).await;
    let status = wire(client.system_status().await.expect("status"));
    assert!(
        status["capabilities"]
            .as_array()
            .expect("capabilities")
            .iter()
            .any(|c| c == "packages.candidates.v1")
    );
    let registered = client
        .project_register(
            project.to_string_lossy().into_owned(),
            "candidate-project".to_owned(),
        )
        .await
        .expect("register project");
    let project_id = registered.project.project_id.expect("project ID");
    let repository = client
        .repository_register(
            alcomd_protocol::RepositorySource::Local {
                path: repository_path.to_string_lossy().into_owned(),
            },
            "candidate-repository".to_owned(),
        )
        .await
        .expect("register repository");
    let repository_id = repository.repository.repository_id.expect("repository ID");
    let refreshed = client
        .repository_refresh(
            repository_id.clone(),
            repository.repository.revision.expect("repo revision"),
            "candidate-refresh".to_owned(),
        )
        .await
        .expect("normalize repository");
    let repository_revision = refreshed.repository.revision.expect("refreshed revision");
    let settings = client.settings_get().await.expect("settings");
    let before_events = wire(client.events_list(0, Some(200)).await.expect("events"));
    let before_operations = wire(
        client
            .operations_list(None, Some(200))
            .await
            .expect("operations"),
    );
    let before_data = fingerprints(&data);
    let before_project = fingerprints(&project);
    let summary_request = request(
        &project_id,
        1,
        json!({"kind":"summary","packageIds":["com.example.candidate"]}),
        64,
    );
    let summary = client
        .package_query_project_candidates(summary_request.clone())
        .await
        .expect("summary");
    let summary = wire(summary);
    assert_eq!(
        summary["items"][0]["latest"]["candidate"]["version"], "2.0.0",
        "{summary:#}"
    );
    assert_eq!(
        summary["items"][0]["projectLatest"]["candidate"]["version"],
        "1.10.0"
    );
    assert_eq!(
        summary["items"][0]["update"]["candidate"]["relation"],
        "newer"
    );
    assert_eq!(summary["catalogComplete"], true);
    let mut versions_request = request(
        &project_id,
        1,
        json!({"kind":"versions","packageId":"com.example.candidate"}),
        1,
    );
    versions_request.expected_snapshot = Some(
        summary["snapshot"]["fingerprint"]
            .as_str()
            .expect("fingerprint")
            .to_owned(),
    );
    let first = client
        .package_query_project_candidates(versions_request.clone())
        .await
        .expect("versions first page");
    assert!(first.next_cursor.is_some());
    assert!(
        first.catalog_complete,
        "page continuation is not catalog incompleteness"
    );
    versions_request.cursor = first.next_cursor;
    let second = wire(
        client
            .package_query_project_candidates(versions_request.clone())
            .await
            .expect("versions second page"),
    );
    assert_eq!(second["items"][0]["version"], "1.10.0");
    assert_eq!(
        wire(client.events_list(0, Some(200)).await.expect("events")),
        before_events
    );
    assert_eq!(
        wire(
            client
                .operations_list(None, Some(200))
                .await
                .expect("operations")
        ),
        before_operations
    );
    assert_eq!(
        fingerprints(&data),
        before_data,
        "query must not write business DB/cache/config"
    );
    assert_eq!(fingerprints(&project), before_project);
    assert!(matches!(network.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock));

    // Registered evidence is unchanged by an unrefreshed external repository edit.
    let mut external: Value =
        serde_json::from_slice(&fs::read(&repository_path).expect("repository bytes"))
            .expect("repository JSON");
    let mut added = external["packages"]["com.example.candidate"]["versions"]["2.0.0"].clone();
    added["version"] = json!("2.1.0");
    external["packages"]["com.example.candidate"]["versions"]["2.1.0"] = added;
    fs::write(&repository_path, external.to_string()).expect("external repository change");
    assert_eq!(
        wire(
            client
                .package_query_project_candidates(summary_request.clone())
                .await
                .expect("registered snapshot")
        ),
        summary
    );
    client
        .settings_update(alcomd_protocol::SettingsUpdateParams {
            expected_revision: settings.revision,
            update: alcomd_protocol::SettingsUpdate {
                packages: Some(alcomd_protocol::PackageSettingsUpdate {
                    show_prerelease: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            },
        })
        .await
        .expect("explicit config update");
    assert_eq!(
        remote_code(
            client
                .package_query_project_candidates(versions_request.clone())
                .await
                .expect_err("config stale")
        ),
        "package_candidate_evidence_stale"
    );
    let mut fresh = request(
        &project_id,
        1,
        json!({"kind":"versions","packageId":"com.example.candidate"}),
        1,
    );
    let page = client
        .package_query_project_candidates(fresh.clone())
        .await
        .expect("fresh page");
    fresh.cursor = page.next_cursor;
    client
        .repository_refresh(
            repository_id.clone(),
            repository_revision,
            "candidate-refresh-again".to_owned(),
        )
        .await
        .expect("refresh source");
    assert_eq!(
        remote_code(
            client
                .package_query_project_candidates(fresh)
                .await
                .expect_err("source stale")
        ),
        "package_candidate_evidence_stale"
    );
    let mut fresh = request(
        &project_id,
        1,
        json!({"kind":"versions","packageId":"com.example.candidate"}),
        1,
    );
    let page = client
        .package_query_project_candidates(fresh.clone())
        .await
        .expect("fresh page");
    fresh.cursor = page.next_cursor;
    // Explicit user intent creates a real existing remove Plan, without Apply or
    // package download. Candidate evidence does not make this Plan immune to staleness.
    let remove_before_project_change = client
        .package_plan_remove(alcomd_protocol::PackagePlanRemoveParams {
            project_id: project_id.clone(),
            expected_revision: 1,
            package_id: "com.example.candidate".to_owned(),
        })
        .await
        .expect("explicit remove Plan before Project change");
    fs::write(
        project.join("ProjectSettings/ProjectVersion.txt"),
        "m_EditorVersion: 2022.3.23f1\n",
    )
    .expect("external project edit");
    client
        .project_refresh(
            project_id.clone(),
            1,
            "candidate-project-refresh".to_owned(),
        )
        .await
        .expect("refresh project");
    assert_eq!(
        remote_code(
            client
                .package_query_project_candidates(fresh)
                .await
                .expect_err("project stale")
        ),
        "package_candidate_evidence_stale"
    );

    let project_after_refresh = fingerprints(&project);
    let stale_apply = client
        .package_apply_plan(alcomd_protocol::PackageApplyPlanParams {
            plan_id: remove_before_project_change.plan_id,
            expected_revision: remove_before_project_change.project_revision,
            idempotency_key: "candidate-old-remove-plan-apply".to_owned(),
        })
        .await;
    assert_eq!(
        remote_code(stale_apply.expect_err("Project change invalidates already-created Plan")),
        "plan_stale"
    );
    assert_eq!(fingerprints(&project), project_after_refresh);
    assert!(matches!(network.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock));

    // Candidate evidence never bypasses the existing Plan's project revision check.
    let stale_plan = client
        .package_plan_upgrade(alcomd_protocol::PackagePlanInstallParams {
            project_id: project_id.clone(),
            expected_revision: 1,
            package_id: "com.example.candidate".to_owned(),
            version_range: Some("=1.10.0".to_owned()),
            repository_id: None,
            source: Some(alcomd_protocol::PackageSourceSelector::Repository {
                repository_id: repository_id.clone(),
            }),
            include_prerelease: false,
        })
        .await;
    assert_eq!(
        remote_code(stale_plan.expect_err("old candidate Project revision cannot authorize Plan")),
        "revision_conflict"
    );

    // Direct eligibility is NOT full dependency solvability, and creates no implicit Plan.
    let new_summary = wire(
        client
            .package_query_project_candidates(request(
                &project_id,
                2,
                json!({"kind":"summary","packageIds":["com.example.candidate"]}),
                64,
            ))
            .await
            .expect("updated project summary"),
    );
    assert_eq!(new_summary["items"][0]["update"]["kind"], "target");
    let failed_plan = client
        .package_plan_upgrade(alcomd_protocol::PackagePlanInstallParams {
            project_id: project_id.clone(),
            expected_revision: 2,
            package_id: "com.example.candidate".to_owned(),
            version_range: Some("=1.10.0".to_owned()),
            repository_id: None,
            source: Some(alcomd_protocol::PackageSourceSelector::Repository {
                repository_id: repository_id.clone(),
            }),
            include_prerelease: false,
        })
        .await;
    assert_eq!(
        remote_code(failed_plan.expect_err("missing dependency must reject actual Plan")),
        "package_dependency_missing"
    );
    assert_eq!(
        wire(
            client
                .operations_list(None, Some(200))
                .await
                .expect("operations")
        ),
        before_operations
    );
    assert!(matches!(network.accept(), Err(e) if e.kind() == std::io::ErrorKind::WouldBlock));

    let ids: Vec<_> = (0..256)
        .map(|index| format!("com.{}.{index:03}", "x".repeat(120)))
        .collect();
    let overrides: Vec<_> = ids.iter().map(|id| json!({"packageId":id,"source":{"kind":"repository","repository_id":repository_id}})).collect();
    let oversized = request(
        &project_id,
        2,
        json!({"kind":"summary","packageIds":ids,"sources":overrides}),
        64,
    );
    assert!(
        serde_json::to_vec(&oversized)
            .expect("encoded request")
            .len()
            > 65536
    );
    assert_eq!(
        remote_code(
            client
                .package_query_project_candidates(oversized)
                .await
                .expect_err("byte quota")
        ),
        "package_candidate_limit_exceeded"
    );
    // Inventory changes invalidate cursors even when the added source has no matching package.
    let mut inventory_page = request(
        &project_id,
        2,
        json!({"kind":"versions","packageId":"com.example.candidate"}),
        1,
    );
    inventory_page.cursor = client
        .package_query_project_candidates(inventory_page.clone())
        .await
        .expect("inventory baseline")
        .next_cursor;
    let extra_path = fixture.0.join("additional-repository.json");
    fs::write(
        &extra_path,
        json!({"id":"additional","name":"Additional","packages":{}}).to_string(),
    )
    .expect("additional source");
    let extra = client
        .repository_register(
            alcomd_protocol::RepositorySource::Local {
                path: extra_path.to_string_lossy().into_owned(),
            },
            "candidate-add-source".to_owned(),
        )
        .await
        .expect("add source");
    assert_eq!(
        remote_code(
            client
                .package_query_project_candidates(inventory_page)
                .await
                .expect_err("added inventory stale")
        ),
        "package_candidate_evidence_stale"
    );
    let mut removal_page = request(
        &project_id,
        2,
        json!({"kind":"versions","packageId":"com.example.candidate"}),
        1,
    );
    removal_page.cursor = client
        .package_query_project_candidates(removal_page.clone())
        .await
        .expect("source removal baseline")
        .next_cursor;
    client
        .repository_unregister(
            extra.repository.repository_id.expect("extra ID"),
            extra.repository.revision.expect("extra revision"),
            "candidate-remove-source".to_owned(),
        )
        .await
        .expect("remove source");
    assert_eq!(
        remote_code(
            client
                .package_query_project_candidates(removal_page)
                .await
                .expect_err("removed inventory stale")
        ),
        "package_candidate_evidence_stale"
    );
    let incomplete_path = fixture.0.join("incomplete-repository.json");
    fs::write(&incomplete_path, json!({"id":"incomplete", "name":"Incomplete",
        "packages":{"com.example.other":{"versions":{"1.0.0":{"name":"com.example.other","version":"1.0.0"}}}}}).to_string()).expect("raw-only repository");
    client
        .repository_register(
            alcomd_protocol::RepositorySource::Local {
                path: incomplete_path.to_string_lossy().into_owned(),
            },
            "candidate-incomplete-source".to_owned(),
        )
        .await
        .expect("register raw-only source");
    let incomplete_summary = client
        .package_query_project_candidates(request(
            &project_id,
            2,
            json!({"kind":"summary","packageIds":["com.example.candidate"]}),
            64,
        ))
        .await
        .expect("local evidence with unrelated incomplete catalog");
    assert!(!incomplete_summary.catalog_complete);
    assert_eq!(
        wire(incomplete_summary)["items"][0]["update"]["kind"],
        "target",
        "unrelated incomplete source must not erase local evidence"
    );
    let remove_only = client
        .package_plan_bulk(alcomd_protocol::PackagePlanBulkParams {
            project_id,
            expected_revision: 2,
            intents: vec![alcomd_protocol::PackageBulkIntent::Remove {
                package_id: "com.example.candidate".to_owned(),
            }],
        })
        .await
        .expect_err("remove-only Bulk retains existing completeness requirement");
    assert_eq!(remote_code(remove_only), "repository_refresh_required");
    shutdown.store(true, Ordering::Release);
    drop(client);
    daemon.await.expect("daemon task").expect("daemon shutdown");
}

async fn connect(config: ClientConfig) -> AlcomdClient {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        match AlcomdClient::connect(config.clone()).await {
            Ok(client) => return client,
            Err(ClientError::StartTimeout) if tokio::time::Instant::now() < deadline => (),
            Err(error) => panic!("daemon connection: {error:?}"),
        }
    }
}
#[cfg(unix)]
fn isolated_ipc(runtime: PathBuf) -> (IpcConfig, ClientConfig) {
    (
        IpcConfig::isolated(runtime.clone()),
        ClientConfig::default()
            .without_daemon_start()
            .with_runtime_directory(runtime),
    )
}
#[cfg(windows)]
fn isolated_ipc(_runtime: PathBuf) -> (IpcConfig, ClientConfig) {
    (
        IpcConfig::default(),
        ClientConfig::default().without_daemon_start(),
    )
}
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("alcomd-candidates-rpc-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).expect("fixture");
        Self(root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
