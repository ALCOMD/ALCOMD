#[cfg(windows)]
use super::wait_for_shutdown;
use super::{TestDirectory, connect_with_retry, isolated_ipc, wait_for_terminal};
use alcomd_client::{AlcomdClient, ClientError};
use alcomd_platform::DataConfig;
use alcomd_protocol::{
    ExtensionDesiredState, ExtensionEnableParams, ExtensionInstallApplyParams,
    ExtensionInstallConsent, ExtensionLifecycleParams, ExtensionPermissionsListParams,
    ExtensionPlanInstallParams, ExtensionRuntimeState, ExtensionSourceKind, ExtensionUiCloseParams,
    ExtensionUiOpenParams, OperationState,
};
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const ID: &str = "com.vrclearn.alcomd-discord-presence";

async fn install(client: &mut AlcomdClient, package: &Path) {
    let plan = client
        .extension_plan_install(ExtensionPlanInstallParams {
            source_kind: ExtensionSourceKind::LocalOwnerSelected,
            package_path: package.to_str().expect("package path").into(),
            expected_revision: 0,
        })
        .await
        .expect("plan for signed development package")
        .plan;
    let operation = client
        .extension_apply_install(ExtensionInstallApplyParams {
            consent: ExtensionInstallConsent {
                plan_fingerprint: plan.plan_fingerprint,
                installation_terms_accepted: false,
                required_permissions_accepted: false,
            },
            plan_id: plan.plan_id,
            idempotency_key: "m9-discord-install".into(),
        })
        .await
        .expect("accepted package install");
    assert_eq!(
        wait_for_terminal(client, &operation.operation_id).await,
        OperationState::Succeeded
    );
}

async fn permissions(client: &mut AlcomdClient) -> alcomd_protocol::ExtensionPermissionsListResult {
    client
        .extension_permissions_list(ExtensionPermissionsListParams {
            extension_id: ID.into(),
            cursor: None,
            limit: Some(64),
        })
        .await
        .expect("current grants")
}

#[cfg(windows)]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires authorization to display generic activity on a signed-in Windows Discord client and ALCOMD_M9_UNITY_EDITOR"]
async fn discord_publishes_and_clears_generic_activity_for_real_unity() {
    use std::time::Duration;

    let _serial = super::RPC_TEST_LOCK.lock().await;
    let package = PathBuf::from(std::env::var_os("ALCOMD_M9_DISCORD_PACKAGE").expect("archive"));
    let editor = PathBuf::from(std::env::var_os("ALCOMD_M9_UNITY_EDITOR").expect("Unity Editor"));
    let root = TestDirectory::new();
    let runtime = root.path().join("runtime");
    std::fs::create_dir(&runtime).expect("owned runtime directory");
    let (ipc, config) = isolated_ipc(runtime);
    let shutdown = Arc::new(AtomicBool::new(false));
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc,
        DataConfig::isolated(root.path().join("data")),
        wait_for_shutdown(Arc::clone(&shutdown)),
    )
    .await
    .expect("isolated Core");
    let mut client = connect_with_retry(config).await;
    let version = client
        .unity_installation_register(alcomd_protocol::UnityInstallationRegisterParams {
            executable_path: editor.to_str().expect("editor path").into(),
            idempotency_key: "m9-live-unity-editor".into(),
        })
        .await
        .expect("register the real Editor through Core")
        .installation
        .unity_version;
    let mut projects = Vec::new();
    for name in ["Synthetic-A", "Synthetic-B"] {
        let project = root.path().join(name);
        for folder in ["Assets", "Packages", "ProjectSettings"] {
            std::fs::create_dir_all(project.join(folder)).expect("temporary Unity project folders");
        }
        std::fs::write(
            project.join("Packages/manifest.json"),
            "{\"dependencies\":{}}\n",
        )
        .expect("empty Unity package manifest");
        std::fs::write(
            project.join("ProjectSettings/ProjectVersion.txt"),
            format!("m_EditorVersion: {version}\n"),
        )
        .expect("fixture Unity version");
        client
            .project_register(
                project.to_str().expect("project path").into(),
                format!("m9-live-{name}"),
            )
            .await
            .expect("register synthetic Unity project");
        projects.push(project);
    }
    install(&mut client, &package).await;
    let receipt = client.extension_get(ID.into()).await.expect("installed");
    client
        .extension_enable_with_consent(ExtensionEnableParams {
            extension_id: ID.into(),
            expected_revision: receipt.extension.revision,
            idempotency_key: "m9-live-discord-enable".into(),
            activation_package_digest: Some(
                receipt.activation_consent.expect("consent").package_digest,
            ),
        })
        .await
        .expect("enable without a GUI or Unity process");
    wait_for_discord_status(&mut client, "Connected", Duration::from_secs(50)).await;
    let first = start_unity(&editor, &projects[0]);
    wait_for_discord_status(&mut client, "Showing activity", Duration::from_secs(90)).await;
    let second = start_unity(&editor, &projects[1]);
    drop(first);
    tokio::time::sleep(Duration::from_secs(8)).await;
    wait_for_discord_status(&mut client, "Showing activity", Duration::from_secs(90)).await;
    drop(second);
    wait_for_discord_status(&mut client, "Connected", Duration::from_secs(35)).await;

    let restarted = start_unity(&editor, &projects[0]);
    wait_for_discord_status(&mut client, "Showing activity", Duration::from_secs(90)).await;
    let current = client
        .extension_get(ID.into())
        .await
        .expect("still running")
        .extension;
    client
        .extension_disable(ExtensionLifecycleParams {
            extension_id: ID.into(),
            expected_revision: current.revision,
            idempotency_key: "m9-live-discord-disable".into(),
        })
        .await
        .expect("stop after clearing");
    drop(restarted);
    drop(client);
    shutdown.store(true, Ordering::Release);
    daemon.await.expect("daemon task").expect("clean shutdown");
}

#[cfg(windows)]
struct TestEditor(std::process::Child);

#[cfg(windows)]
impl Drop for TestEditor {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[cfg(windows)]
fn start_unity(executable: &Path, project: &Path) -> TestEditor {
    TestEditor(
        std::process::Command::new(executable)
            .arg("-projectPath")
            .arg(project)
            .arg("-logFile")
            .arg(project.join("Editor.log"))
            .spawn()
            .expect("start real Unity Editor for synthetic project"),
    )
}

#[cfg(windows)]
async fn wait_for_discord_status(
    client: &mut AlcomdClient,
    expected: &str,
    duration: std::time::Duration,
) {
    use std::time::Duration;
    let deadline = tokio::time::Instant::now() + duration;
    loop {
        let opened = client
            .extension_ui_open(ExtensionUiOpenParams {
                extension_id: ID.into(),
                locale: "en-US".into(),
            })
            .await
            .expect("read guest status");
        client
            .extension_ui_close(ExtensionUiCloseParams {
                session_id: opened.session.session_id,
            })
            .await
            .expect("close view to leave background lease alone");
        let document = serde_json::to_value(&opened.snapshot.document).expect("guest document");
        let nodes = document["nodes"].as_array().expect("nodes");
        let status = nodes
            .iter()
            .find(|node| node["nodeId"] == "connection")
            .expect("status");
        if status["payload"]["label"] == expected {
            if expected == "Showing activity" {
                let details = nodes
                    .iter()
                    .find(|node| node["nodeId"] == "details")
                    .expect("preview");
                assert_eq!(details["payload"]["text"], "Editing in Unity");
                assert!(nodes.iter().all(|node| node["nodeId"] != "state"));
            }
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "expected {expected}, got {}",
            status["payload"]["label"]
        );
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires the independently built M9 development archive in ALCOMD_M9_DISCORD_PACKAGE"]
async fn signed_discord_package_uses_scoped_activation_and_portable_ui() {
    use std::time::Duration;
    let _serial = super::RPC_TEST_LOCK.lock().await;
    let package = PathBuf::from(std::env::var_os("ALCOMD_M9_DISCORD_PACKAGE").expect("archive"));
    let root = TestDirectory::new();
    let runtime = root.path().join("runtime");
    std::fs::create_dir(&runtime).expect("owned runtime directory");
    let (ipc, config) = isolated_ipc(runtime);
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc,
        DataConfig::isolated(root.path().join("data")),
        std::future::pending(),
    )
    .await
    .expect("isolated Core");
    let mut client = connect_with_retry(config.clone()).await;
    install(&mut client, &package).await;
    let receipt = client.extension_get(ID.into()).await.expect("installed");
    assert_eq!(
        receipt.extension.desired_state,
        ExtensionDesiredState::InstalledDisabled
    );
    assert!(permissions(&mut client).await.grants.is_empty());
    let consent = receipt
        .activation_consent
        .expect("verified consent preview");
    assert_eq!(consent.targets.len(), 1);
    assert!(consent.targets[0].starts_with("Discord desktop (discord"));
    assert_eq!(consent.package_digest, receipt.extension.package_digest);

    let expected_revision = receipt.extension.revision;
    let mut enable = ExtensionEnableParams {
        extension_id: ID.into(),
        expected_revision,
        idempotency_key: "m9-discord-enable".into(),
        activation_package_digest: None,
    };
    assert!(matches!(
        client.extension_enable_with_consent(enable.clone()).await,
        Err(ClientError::Remote(error)) if error.code == "permission_denied"
    ));
    enable.activation_package_digest = Some("0".repeat(64));
    assert!(matches!(
        client.extension_enable_with_consent(enable.clone()).await,
        Err(ClientError::Remote(error)) if error.code == "revision_conflict"
    ));
    enable.activation_package_digest = Some(consent.package_digest);
    enable.expected_revision = expected_revision + 1;
    assert!(matches!(
        client.extension_enable_with_consent(enable.clone()).await,
        Err(ClientError::Remote(error)) if error.code == "revision_conflict"
    ));
    assert!(permissions(&mut client).await.grants.is_empty());

    enable.expected_revision = expected_revision;
    let active = client
        .extension_enable_with_consent(enable.clone())
        .await
        .expect("single consent");
    assert_eq!(
        active.extension.desired_state,
        ExtensionDesiredState::Enabled
    );
    assert_eq!(
        active.extension.runtime_state,
        ExtensionRuntimeState::Running
    );
    let granted = permissions(&mut client).await;
    assert_eq!(granted.grants.len(), 4);
    assert!(
        granted
            .grants
            .iter()
            .any(|grant| grant.permission == "unity.sessions.read")
    );
    let scoped = granted
        .grants
        .iter()
        .find(|grant| grant.permission == "local-ipc.connect")
        .expect("scoped consent");
    assert_eq!(scoped.resource_kind, "LocalIpcTarget");
    assert_eq!(scoped.resource_id.len(), 64);

    let opened = client
        .extension_ui_open(ExtensionUiOpenParams {
            extension_id: ID.into(),
            locale: "en-US".into(),
        })
        .await
        .expect("Portable UI");
    let document = opened.snapshot.document;
    document.validate().expect("Portable UI schema");
    assert_eq!(document.title, "ALCOMD Discord Presence");
    let serialized = serde_json::to_value(&document).expect("document");
    let nodes = serialized["nodes"].as_array().expect("nodes");
    for field in ["project", "version", "count"] {
        let control = nodes
            .iter()
            .find(|node| node["nodeId"] == field)
            .expect("privacy toggle");
        assert_eq!(control["payload"]["initialValue"], false);
    }
    let mut fields = ["project", "version", "count"]
        .into_iter()
        .map(|id| alcomd_protocol::UiSubmittedField {
            field_id: id.into(),
            value: alcomd_protocol::UiFieldValue::Boolean {
                value: id == "count",
            },
        })
        .collect::<Vec<_>>();
    fields.push(alcomd_protocol::UiSubmittedField {
        field_id: "custom".into(),
        value: alcomd_protocol::UiFieldValue::Text {
            value: "Local settings fixture".into(),
        },
    });
    let saved = client
        .extension_ui_dispatch(alcomd_protocol::ExtensionUiDispatchParams {
            session_id: opened.session.session_id.clone(),
            expected_snapshot_revision: opened.snapshot.snapshot_revision,
            sequence: 1,
            request_id: uuid::Uuid::new_v4().to_string(),
            action: alcomd_protocol::UiAction::SubmitForm {
                action_id: "save-settings".into(),
                values: fields,
            },
        })
        .await
        .expect("persist settings through Portable UI");
    let snapshot = serde_json::to_value(saved.snapshot.document).expect("saved document");
    let nodes = snapshot["nodes"].as_array().expect("nodes");
    assert!(
        nodes
            .iter()
            .any(|node| node["nodeId"] == "feedback" && node["payload"]["label"] == "Saved")
    );
    assert!(
        nodes
            .iter()
            .any(|node| node["nodeId"] == "count" && node["payload"]["initialValue"] == true)
    );
    assert!(nodes.iter().any(|node| node["nodeId"] == "custom"
        && node["payload"]["initialValue"] == "Local settings fixture"));
    client
        .extension_ui_close(ExtensionUiCloseParams {
            session_id: opened.session.session_id,
        })
        .await
        .expect("close extension view");
    drop(client);
    tokio::time::sleep(Duration::from_millis(2300)).await;
    assert!(
        !daemon.is_finished(),
        "authorized background work keeps Core alive without clients"
    );
    let mut client = connect_with_retry(config).await;
    assert_eq!(
        client
            .extension_get(ID.into())
            .await
            .expect("background record")
            .extension
            .runtime_state,
        ExtensionRuntimeState::Running
    );

    client
        .extension_revoke_grant(alcomd_protocol::ExtensionGrantParams {
            extension_id: ID.into(),
            permission: scoped.permission.clone(),
            resource_kind: scoped.resource_kind.clone(),
            resource_id: scoped.resource_id.clone(),
            expected_grant_revision: granted.grant_revision,
            idempotency_key: "m9-revoke-target".into(),
        })
        .await
        .expect("revoke signed target");
    let revoked = client
        .extension_enable_with_consent(enable)
        .await
        .expect("replay cannot restore consent");
    assert_eq!(
        revoked.extension.desired_state,
        ExtensionDesiredState::InstalledDisabled
    );
    assert_eq!(
        revoked.extension.runtime_state,
        ExtensionRuntimeState::Stopped
    );
    assert!(
        permissions(&mut client)
            .await
            .grants
            .iter()
            .all(|grant| grant.permission != "local-ipc.connect")
    );

    let enabled = client
        .extension_enable_with_consent(ExtensionEnableParams {
            extension_id: ID.into(),
            expected_revision: revoked.extension.revision,
            idempotency_key: "m9-fresh-consent".into(),
            activation_package_digest: Some(revoked.extension.package_digest),
        })
        .await
        .expect("fresh consent")
        .extension;
    let disabled = client
        .extension_disable(ExtensionLifecycleParams {
            extension_id: ID.into(),
            expected_revision: enabled.revision,
            idempotency_key: "m9-discord-disable".into(),
        })
        .await
        .expect("normal disable")
        .extension;
    assert_eq!(
        disabled.desired_state,
        ExtensionDesiredState::InstalledDisabled
    );
    assert_eq!(disabled.runtime_state, ExtensionRuntimeState::Stopped);

    drop(client);
    tokio::time::timeout(Duration::from_secs(8), daemon)
        .await
        .expect("idle exit after disabling the last scoped background extension")
        .expect("daemon task")
        .expect("clean shutdown");
}
