use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use alcomd_application::{
    BackupCompression, BackupCreateOptions, BackupCreateRequest, BackupDestination, IdempotencyKey,
    M3RegistryStore, M5BackupErrorCode, M5BackupStore, ManifestState, McpIdentityStore,
    McpIdentityWrite, OperationState, PrincipalId, ProjectObservation, ProjectType, Revision,
    StateStore, ToolAuthentication, mcp_credential_hash, with_workflow_authentication,
};
use alcomd_store::StateStoreHandle;
use uuid::Uuid;

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("alcomd-backup-directories-{}", Uuid::new_v4()));
        fs::create_dir(&path).expect("isolated directory");
        Self(path)
    }

    fn database(&self) -> PathBuf {
        self.0.join("state.db")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        for _ in 0..20 {
            if fs::remove_dir_all(&self.0).is_ok() {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        panic!("isolated backup store directory could not be removed");
    }
}

fn key(value: &str) -> IdempotencyKey {
    IdempotencyKey::parse(value).expect("key")
}

#[tokio::test]
async fn mcp_backup_acceptance_uses_shared_project_and_checks_live_authority() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("isolated store");
    let owner = PrincipalId::local_owner();
    let project = store
        .register_project(
            owner.clone(),
            ProjectObservation {
                root_path: directory
                    .0
                    .join("owned project")
                    .to_string_lossy()
                    .into_owned(),
                path_identity_key: vec![7; 24],
                project_type: ProjectType::VpmStarter,
                unity_version: "2022.3.22f1".to_owned(),
                unity_revision: None,
                vpm_manifest: ManifestState::Valid,
                upm_manifest: ManifestState::Valid,
                direct_dependencies: Vec::new(),
                locked_dependencies: Vec::new(),
                issues: Vec::new(),
            },
            key("owner-register"),
            1,
        )
        .await
        .expect("native registration")
        .value;
    let mut clients = Vec::new();
    for name in ["first", "second"] {
        let client = store
            .mcp_write(
                owner.clone(),
                format!("create-{name}"),
                McpIdentityWrite::Create {
                    name: name.to_owned(),
                },
                2,
            )
            .await
            .expect("client")
            .client;
        let hash = mcp_credential_hash(name);
        let issued = store
            .mcp_write(
                owner.clone(),
                format!("issue-{name}"),
                McpIdentityWrite::Issue {
                    id: client.client_id,
                    expected: client.revision,
                    hash,
                },
                3,
            )
            .await
            .expect("issue")
            .client;
        clients.push((issued, ToolAuthentication::Credential(hash)));
    }
    let [(first, first_auth), (second, second_auth)] = clients.as_slice() else {
        panic!("two distinct clients");
    };
    let new_backup = || BackupCreateRequest {
        backup_id: Default::default(),
        project_id: project.project_id,
        expected_revision: project.revision,
        compression_mode: BackupCompression::Fast,
        exclude_vpm_packages: true,
        include_library: false,
        created_at_ms: 4,
        destination: None,
    };
    let accept = |caller: PrincipalId| {
        let request = new_backup();
        let options = BackupCreateOptions::from(&request);
        store.accept_backup_create(caller, request, options, key("same-key"))
    };
    for unauthorized in [
        accept(first.principal_id.clone()).await,
        with_workflow_authentication(second_auth.clone(), accept(first.principal_id.clone())).await,
    ] {
        assert_eq!(
            unauthorized
                .expect_err("missing or mismatched authority")
                .code(),
            M5BackupErrorCode::PermissionDenied
        );
    }
    let result =
        with_workflow_authentication(first_auth.clone(), accept(first.principal_id.clone()))
            .await
            .expect("authorized write");
    let replay =
        with_workflow_authentication(first_auth.clone(), accept(first.principal_id.clone()))
            .await
            .expect("authorized replay");
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, result.operation_id);
    assert_eq!(replay.backup_id, result.backup_id);
    let independent =
        with_workflow_authentication(second_auth.clone(), accept(second.principal_id.clone()))
            .await
            .expect("independent write");
    assert_ne!(independent.operation_id, result.operation_id);
    store
        .mcp_write(
            owner,
            "revoke-backup-write".to_owned(),
            McpIdentityWrite::Revoke {
                id: first.client_id,
                expected: first.revision,
            },
            5,
        )
        .await
        .expect("revoke");
    let denied =
        with_workflow_authentication(first_auth.clone(), accept(first.principal_id.clone()))
            .await
            .expect_err("live grants apply to existing credentials and replay");
    assert_eq!(denied.code(), M5BackupErrorCode::PermissionDenied);
    drop(store);
}

#[tokio::test]
async fn backup_destination_is_frozen_across_preference_change_replay_and_restart() {
    let directory = TestDirectory::new();
    let store = StateStoreHandle::open(directory.database()).expect("store");
    let owner = PrincipalId::local_owner();
    let project = store
        .register_project(
            owner.clone(),
            ProjectObservation {
                root_path: directory.0.join("project").to_string_lossy().into_owned(),
                path_identity_key: vec![1; 24],
                project_type: ProjectType::VpmStarter,
                unity_version: "2022.3.22f1".to_owned(),
                unity_revision: None,
                vpm_manifest: ManifestState::Valid,
                upm_manifest: ManifestState::Valid,
                direct_dependencies: Vec::new(),
                locked_dependencies: Vec::new(),
                issues: Vec::new(),
            },
            key("register"),
            1,
        )
        .await
        .expect("project")
        .value;
    let request = BackupCreateRequest {
        backup_id: Default::default(),
        project_id: project.project_id,
        expected_revision: project.revision,
        compression_mode: BackupCompression::Fast,
        exclude_vpm_packages: true,
        include_library: false,
        created_at_ms: 2,
        destination: Some(BackupDestination {
            directory: directory.0.join("first").to_string_lossy().into_owned(),
            identity: vec![2; 24],
        }),
    };
    assert!(
        store
            .replay_backup_create(
                owner.clone(),
                request.project_id,
                request.expected_revision,
                BackupCreateOptions::from(&request),
                key("create")
            )
            .await
            .expect("no replay yet")
            .is_none()
    );
    let accepted = store
        .accept_backup_create(
            owner.clone(),
            request.clone(),
            BackupCreateOptions::from(&request),
            key("create"),
        )
        .await
        .expect("accept");
    let mut replay_request = request.clone();
    replay_request.backup_id = Default::default();
    replay_request.created_at_ms = 3;
    replay_request.destination = Some(BackupDestination {
        directory: directory.0.join("second").to_string_lossy().into_owned(),
        identity: vec![3; 24],
    });
    let replayed = store
        .replay_backup_create(
            owner.clone(),
            replay_request.project_id,
            replay_request.expected_revision,
            BackupCreateOptions::from(&replay_request),
            key("create"),
        )
        .await
        .expect("replay query")
        .expect("existing outcome");
    assert!(replayed.replayed);
    assert!(!replayed.schedule);
    assert_eq!(replayed.operation_id, accepted.operation_id);
    assert_eq!(replayed.backup_id, accepted.backup_id);
    let accepted_replay = store
        .accept_backup_create(
            owner.clone(),
            replay_request.clone(),
            BackupCreateOptions::from(&replay_request),
            key("create"),
        )
        .await
        .expect("transactional replay");
    assert_eq!(accepted_replay, replayed);
    replay_request.compression_mode = BackupCompression::Maximum;
    replay_request.exclude_vpm_packages = false;
    replay_request.include_library = true;
    let inherited_replay = store
        .replay_backup_create(
            owner.clone(),
            replay_request.project_id,
            replay_request.expected_revision,
            BackupCreateOptions::default(),
            key("create"),
        )
        .await
        .expect("omitted defaults reuse accepted selection")
        .expect("existing operation");
    assert_eq!(inherited_replay, replayed);
    assert_eq!(
        store
            .replay_backup_create(
                owner.clone(),
                replay_request.project_id,
                replay_request.expected_revision,
                BackupCreateOptions {
                    include_library: Some(true),
                    ..BackupCreateOptions::default()
                },
                key("create")
            )
            .await
            .expect_err("explicit cache selection changed")
            .code(),
        M5BackupErrorCode::IdempotencyConflict
    );
    replay_request.exclude_vpm_packages = true;
    replay_request.include_library = false;
    replay_request.compression_mode = BackupCompression::Maximum;
    assert_eq!(
        store
            .replay_backup_create(
                owner,
                replay_request.project_id,
                replay_request.expected_revision,
                BackupCreateOptions::from(&replay_request),
                key("create")
            )
            .await
            .expect_err("different user intent")
            .code(),
        M5BackupErrorCode::IdempotencyConflict,
    );
    let connection =
        rusqlite::Connection::open(directory.database()).expect("read fingerprint evidence");
    let fingerprint: String = connection.query_row("SELECT request_fingerprint FROM idempotency_records WHERE method='backups.create' AND idempotency_key='create'", [], |row| row.get(0)).expect("stored fingerprint");
    assert_eq!(
        fingerprint,
        format!(
            "{{\"compressionMode\":\"fast\",\"excludeVpmPackages\":true,\"expectedRevision\":1,\"projectId\":\"{}\"}}",
            project.project_id
        )
    );
    drop(connection);
    drop(store);
    let reopened = StateStoreHandle::open(directory.database()).expect("reopened store");
    let recovered = reopened
        .begin_backup_create(accepted.operation_id, 4)
        .await
        .expect("stored operation");
    assert_eq!(recovered.request, request);
    let cancellation = reopened
        .accept_backup_create(
            PrincipalId::local_owner(),
            request.clone(),
            BackupCreateOptions::from(&request),
            key("cancel-create"),
        )
        .await
        .expect("accept cancellable operation");
    let (cancelled, _) = reopened
        .cancel_operation(
            PrincipalId::local_owner(),
            cancellation.operation_id,
            Revision::INITIAL,
            key("cancel"),
            5,
        )
        .await
        .expect("queued cancellation");
    assert_eq!(cancelled.state, OperationState::Cancelled);
    assert!(
        reopened
            .cancellation_requested(cancellation.operation_id)
            .await
            .expect("queued cancellation remains requested")
    );
    let cleanup_request = reopened
        .begin_backup_create(cancellation.operation_id, 6)
        .await
        .expect("read already cancelled queued request without restarting it");
    assert_eq!(cleanup_request.request.destination, request.destination);
    assert_eq!(
        reopened
            .get_operation(PrincipalId::local_owner(), cancellation.operation_id)
            .await
            .expect("queued terminal state preserved"),
        cancelled,
    );
    let final_state = reopened
        .finish_cancelled(cancellation.operation_id, 7)
        .await
        .expect("already cancelled finish is read-only");
    assert_eq!(final_state, cancelled);

    let running_cancellation = reopened
        .accept_backup_create(
            PrincipalId::local_owner(),
            request.clone(),
            BackupCreateOptions::from(&request),
            key("running-cancel-create"),
        )
        .await
        .expect("accept running cancellation fixture");
    reopened
        .begin_backup_create(running_cancellation.operation_id, 8)
        .await
        .expect("start queued fixture");
    let running = reopened
        .get_operation(
            PrincipalId::local_owner(),
            running_cancellation.operation_id,
        )
        .await
        .expect("running operation");
    assert_eq!(running.state, OperationState::Running);
    let (cancelling, _) = reopened
        .cancel_operation(
            PrincipalId::local_owner(),
            running_cancellation.operation_id,
            running.revision,
            key("running-cancel"),
            9,
        )
        .await
        .expect("running cancellation");
    assert_eq!(cancelling.state, OperationState::Cancelling);
    let cleanup_request = reopened
        .begin_backup_create(running_cancellation.operation_id, 10)
        .await
        .expect("read cancelling request for partial cleanup");
    assert_eq!(cleanup_request.request.destination, request.destination);
    assert_eq!(
        reopened
            .get_operation(
                PrincipalId::local_owner(),
                running_cancellation.operation_id
            )
            .await
            .expect("cancelling state and revision preserved"),
        cancelling,
    );
    let finished = reopened
        .finish_cancelled(running_cancellation.operation_id, 11)
        .await
        .expect("finish running cancellation");
    assert_eq!(finished.state, OperationState::Cancelled);
    assert_eq!(
        finished.revision,
        cancelling
            .revision
            .checked_next()
            .expect("terminal revision")
    );
    drop(reopened);
}

#[test]
fn retained_backup_request_without_destination_still_decodes_as_legacy() {
    let request: BackupCreateRequest = serde_json::from_value(serde_json::json!({
        "backupId": "00000000-0000-4000-8000-000000000001",
        "projectId": "00000000-0000-4000-8000-000000000002",
        "expectedRevision": 1,
        "compressionMode": "fast",
        "excludeVpmPackages": true,
        "createdAtMs": 1,
    }))
    .expect("retained request");
    assert!(request.destination.is_none());
    assert!(!request.include_library);
    assert!(
        serde_json::to_value(request)
            .expect("request")
            .get("destination")
            .is_none()
    );
}
