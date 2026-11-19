use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn frozen_sdk_catalog_install_and_reinstall_real_core() {
    let _serial = REAL_CORE_LOCK.lock().await;
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/m8-business-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).expect("owned evidence root");
    let root = root.canonicalize().expect("fixture root");
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        IpcConfig::default(),
        DataConfig::isolated(root.join("data")),
        std::future::pending(),
    )
    .await
    .expect("exclusive real Core bind");
    let mut owner = AlcomdClient::connect(ClientConfig::default().without_daemon_start())
        .await
        .expect("native test owner");
    let fixture = root.join("Project");
    project(&fixture, false);
    let project_record = owner
        .project_register(
            fixture.to_string_lossy().into_owned(),
            "catalog-project".into(),
        )
        .await
        .expect("register project")
        .project;
    let project_id = project_record.project_id.expect("project ID");
    let revision = project_record.revision.expect("registered revision");
    let installed = fixture.join("Packages/com.example.catalog");
    let mut packages = Vec::new();
    for (name, version, dependencies) in [
        (
            "com.example.catalog",
            "1.0.0",
            json!({"com.example.dependency":"1.0.0"}),
        ),
        ("com.example.dependency", "1.0.0", json!({})),
    ] {
        let source = root.join(format!("{name}-{version}"));
        std::fs::create_dir(&source).expect("isolated user source");
        std::fs::write(source.join("package.json"), json!({
            "name":name, "version":version, "displayName":"Catalog fixture", "description":"Delegated package", "vpmDependencies":dependencies,
        }).to_string()).expect("source manifest");
        std::fs::write(source.join("fixture.txt"), version).expect("package content");
        packages.push(
            owner
                .user_package_enroll(UserPackageEnrollParams {
                    source_path: source.to_string_lossy().into_owned(),
                    idempotency_key: format!("enroll-{name}-{version}"),
                })
                .await
                .expect("Core enrolls package")
                .user_package,
        );
    }
    let a = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Catalog A".into(),
            idempotency_key: "catalog-a".into(),
        })
        .await
        .expect("client A");
    let b = owner
        .mcp_client_create(McpClientsCreateParams {
            display_name: "Catalog B".into(),
            idempotency_key: "catalog-b".into(),
        })
        .await
        .expect("client B");
    let ca = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: a.client_id.clone(),
            expected_revision: a.revision,
            idempotency_key: "issue-catalog-a".into(),
        })
        .await
        .expect("credential A");
    let cb = owner
        .mcp_client_issue_credential(McpClientMutationParams {
            client_id: b.client_id,
            expected_revision: b.revision,
            idempotency_key: "issue-catalog-b".into(),
        })
        .await
        .expect("credential B");
    let sa = sdk(&ca.credential, None).await;
    let sb = sdk(&cb.credential, None).await;
    let mut trace = Vec::new();
    let first = data(
        call(&sa, &mut trace, "search_packages", json!({"limit":1}))
            .await
            .expect("global search"),
    );
    assert_eq!(first["items"][0]["package_id"], "com.example.catalog");
    assert_eq!(first["items"][0]["version"], "1.0.0");
    let next = json!({"cursor":first["next_cursor"],"limit":1});
    let second = data(
        call(&sa, &mut trace, "search_packages", next.clone())
            .await
            .expect("second page"),
    );
    assert_eq!(second["items"][0]["package_id"], "com.example.dependency");
    error(
        call(&sb, &mut trace, "search_packages", next)
            .await
            .expect("foreign cursor"),
        "revision_conflict",
    );
    let details = data(call(&sa, &mut trace, "get_package", json!({"package_id":"com.example.catalog", "project_id":project_id, "expected_revision":revision})).await.expect("versions"));
    assert_eq!(details["items"].as_array().expect("versions").len(), 1);
    assert_eq!(
        details["items"][0]["dependencies"]["com.example.dependency"],
        "1.0.0"
    );
    assert_eq!(
        details["items"][0]["compatibility"]["eligibility"],
        "eligible"
    );
    let selector = json!({"kind":"user_package","user_package_id":packages[0].user_package_id});
    let plan = data(call(&sa, &mut trace, "plan_package_install", json!({
        "project_id":project_id, "expected_revision":revision, "package_id":"com.example.catalog",
        "source":selector, "version_selector":{"kind":"exact", "version":"1.0.0"},
    })).await.expect("exact install plan"));
    assert!(!installed.exists());
    let public = plan.to_string();
    assert!(
        !public.contains("artifact_url")
            && !public.contains("source_identity")
            && !public.contains("source_root_path")
    );
    assert_eq!(plan["plan"]["action"], "install");
    assert_eq!(
        plan["sources"]
            .as_array()
            .expect("two pinned sources")
            .len(),
        2
    );
    let apply = json!({"plan_id":plan["plan"]["plan_id"],"expected_revision":revision,"idempotency_key":"catalog-install","confirm":true});
    error(
        call(&sb, &mut trace, "apply_package_changes", apply.clone())
            .await
            .expect("foreign plan"),
        "plan_not_found",
    );
    let result = data(
        call(&sa, &mut trace, "apply_package_changes", apply.clone())
            .await
            .expect("apply original install"),
    );
    let operation = result["operation_id"].as_str().expect("operation");
    let installed_state = terminal(&sa, &mut trace, operation).await;
    assert_eq!(installed_state["state"], "succeeded", "{installed_state}");
    assert_eq!(
        std::fs::read_to_string(installed.join("fixture.txt")).expect("published fixture"),
        "1.0.0"
    );
    let replay = data(
        call(&sa, &mut trace, "apply_package_changes", apply)
            .await
            .expect("same key recovery"),
    );
    assert_eq!(replay["operation_id"], operation);
    assert_eq!(replay["replayed"], true);
    let revision = owner
        .project_get(project_id.clone())
        .await
        .expect("updated project")
        .project
        .revision
        .expect("updated revision");
    std::fs::remove_file(installed.join("fixture.txt"))
        .expect("simulate disposable missing package file");
    let plan = data(call(&sa, &mut trace, "plan_package_reinstall", json!({
        "project_id":project_id, "expected_revision":revision, "package_id":"com.example.catalog", "source":selector,
    })).await.expect("reinstall original version"));
    let result = data(call(&sa, &mut trace, "apply_package_changes", json!({
        "plan_id":plan["plan"]["plan_id"],"expected_revision":revision,"idempotency_key":"catalog-reinstall","confirm":true,
    })).await.expect("apply original reinstall"));
    let reinstalled = terminal(
        &sa,
        &mut trace,
        result["operation_id"].as_str().expect("operation"),
    )
    .await;
    assert_eq!(reinstalled["state"], "succeeded", "{reinstalled}");
    assert_eq!(
        std::fs::read_to_string(installed.join("fixture.txt")).expect("reinstalled fixture"),
        "1.0.0"
    );
    use sha2::{Digest, Sha256};
    use std::io::{Cursor, Write};
    let mut manifest = json!({
        "name":"com.example.catalog", "version":"2.0.0", "vpmDependencies":{"com.example.dependency":"1.0.0"},
    });
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default();
    archive
        .start_file("package.json", options)
        .expect("version manifest entry");
    archive
        .write_all(manifest.to_string().as_bytes())
        .expect("version manifest");
    archive
        .start_file("fixture.txt", options)
        .expect("payload entry");
    archive.write_all(b"2.0.0").expect("version payload");
    let bytes = archive.finish().expect("archive").into_inner();
    let digest = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let cache = root.join("data/package-cache/sha256").join(&digest[..2]);
    std::fs::create_dir_all(&cache).expect("owned cache");
    std::fs::write(cache.join(format!("{digest}.zip")), bytes)
        .expect("digest-verified archive fixture");
    manifest["url"] = json!("https://network-must-not-be-used.invalid/catalog.zip");
    manifest["zipSHA256"] = json!(digest);
    let repository = root.join("upgrade-repository.json");
    std::fs::write(&repository, json!({"name":"Upgrade fixture","packages":{"com.example.catalog":{"versions":{"2.0.0":manifest}}}}).to_string()).expect("index");
    let newer = owner
        .repository_register(
            RepositorySource::Local {
                path: repository.to_string_lossy().into_owned(),
            },
            "register-upgrade-source".into(),
        )
        .await
        .expect("register second version")
        .repository
        .repository_id
        .expect("repository ID");
    for (action, version) in [("upgrade", "2.0.0"), ("downgrade", "1.0.0")] {
        let revision = owner
            .project_get(project_id.clone())
            .await
            .expect("project")
            .project
            .revision
            .expect("revision");
        let (source, selected) = if action == "upgrade" {
            (
                alcomd_protocol::PackageSourceSelector::Repository {
                    repository_id: newer.clone(),
                },
                json!({"kind":"repository","repository_id":newer}),
            )
        } else {
            (
                alcomd_protocol::PackageSourceSelector::UserPackage {
                    user_package_id: packages[0].user_package_id.clone(),
                },
                selector.clone(),
            )
        };
        let source = Some(source);
        let native = if action == "upgrade" {
            owner
                .package_plan_upgrade(alcomd_protocol::PackagePlanInstallParams {
                    project_id: project_id.clone(),
                    expected_revision: revision,
                    package_id: "com.example.catalog".into(),
                    version_range: Some("^2.0.0".into()),
                    repository_id: None,
                    source,
                    include_prerelease: false,
                    allow_unity_incompatible: false,
                })
                .await
        } else {
            owner
                .package_plan_downgrade(alcomd_protocol::PackagePlanDowngradeParams {
                    project_id: project_id.clone(),
                    expected_revision: revision,
                    package_id: "com.example.catalog".into(),
                    version: version.into(),
                    repository_id: None,
                    source,
                    allow_unity_incompatible: false,
                })
                .await
        }
        .expect("original native package use case");
        let intent = if action == "upgrade" {
            json!({"kind":action,"version_selector":{"kind":"range","range":"^2.0.0"},"include_prerelease":false})
        } else {
            json!({"kind":action,"version":version})
        };
        let plan = data(call(&sa,&mut trace,"plan_package_changes",json!({
            "project_id":project_id,"expected_revision":revision,"package_id":"com.example.catalog",
            "source":selected,"intent":intent,
        })).await.expect("same Core plan through frozen SDK"));
        assert_eq!(
            plan["plan"]["change_set_fingerprint"],
            native.change_set_fingerprint
        );
        let accepted = data(call(&sa,&mut trace,"apply_package_changes",json!({
            "plan_id":plan["plan"]["plan_id"],"expected_revision":revision,"confirm":true,"idempotency_key":action,
        })).await.expect("apply the reviewed plan"));
        let state = terminal(
            &sa,
            &mut trace,
            accepted["operation_id"].as_str().expect("operation"),
        )
        .await;
        assert_eq!(state["state"], "succeeded", "{state}");
        assert_eq!(
            std::fs::read_to_string(installed.join("fixture.txt")).expect("published version"),
            version
        );
    }
    let revision = owner
        .project_get(project_id.clone())
        .await
        .expect("project")
        .project
        .revision
        .expect("revision");
    let bulk = data(call(&sa,&mut trace,"plan_package_changes",json!({
        "project_id":project_id,"expected_revision":revision,"intent":{"kind":"bulk","intents":[
            {"kind":"remove","package_id":"com.example.fixture"},
            {"kind":"reinstall","package_id":"com.example.catalog","source":selector},
        ]},
    })).await.expect("same Core bulk plan"));
    let accepted = data(call(&sa,&mut trace,"apply_package_changes",json!({
        "plan_id":bulk["plan"]["plan_id"],"expected_revision":revision,"confirm":true,"idempotency_key":"catalog-bulk",
    })).await.expect("apply reviewed bulk plan"));
    let state = terminal(
        &sa,
        &mut trace,
        accepted["operation_id"].as_str().expect("operation"),
    )
    .await;
    assert_eq!(state["state"], "succeeded", "{state}");
    assert!(!fixture.join("Packages/com.example.fixture").exists());
    let revision = owner
        .project_get(project_id.clone())
        .await
        .expect("project")
        .project
        .revision
        .expect("revision");
    let native = owner
        .package_plan_resolve(alcomd_protocol::PackagePlanResolveParams {
            project_id: project_id.clone(),
            expected_revision: revision,
            include_prerelease: false,
        })
        .await
        .expect("native resolve");
    let resolved = data(call(&sa,&mut trace,"plan_package_changes",json!({
        "project_id":project_id,"expected_revision":revision,"intent":{"kind":"resolve","include_prerelease":false},
    })).await.expect("same resolver"));
    assert_eq!(
        resolved["plan"]["change_set_fingerprint"],
        native.change_set_fingerprint
    );
    let first = data(
        call(&sa, &mut trace, "list_operations", json!({"limit":1}))
            .await
            .expect("own tasks"),
    );
    assert_eq!(
        first["operations"][0]["operation_id"],
        accepted["operation_id"]
    );
    let next = data(
        call(
            &sa,
            &mut trace,
            "list_operations",
            json!({"limit":1,"cursor":first["next_cursor"]}),
        )
        .await
        .expect("next owned task"),
    );
    assert_ne!(
        next["operations"][0]["operation_id"],
        accepted["operation_id"]
    );
    assert_eq!(
        data(
            call(&sb, &mut trace, "list_operations", json!({}))
                .await
                .expect("other principal")
        )
        .get("operations"),
        Some(&json!([]))
    );
    owner
        .mcp_client_revoke(McpClientMutationParams {
            client_id: a.client_id,
            expected_revision: ca.revision,
            idempotency_key: "catalog-revoke".into(),
        })
        .await
        .expect("revoke connection");
    error(
        call(
            &sa,
            &mut trace,
            "get_package",
            json!({"package_id":"com.example.catalog"}),
        )
        .await
        .expect("revoked while connected"),
        "permission_denied",
    );
    std::fs::write(
        root.join("catalog-flow.json"),
        serde_json::to_vec_pretty(&trace).expect("evidence"),
    )
    .expect("save trace");
    sa.cancel().await.expect("stop SDK A");
    sb.cancel().await.expect("stop SDK B");
    drop(owner);
    drop(daemon);
}
