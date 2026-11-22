use alcomd_application::*;
use alcomd_store::StateStoreHandle;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Default)]
struct RecordedDesktop {
    calls: Mutex<Vec<(String, Option<PathBuf>)>>,
    failure: Option<DesktopError>,
}

impl DesktopEnvironment for RecordedDesktop {
    async fn prepare(
        &self,
        _: &AccessContext,
        action: &DesktopAction,
    ) -> Result<Option<DesktopProject>, DesktopError> {
        Ok(action.project_id().map(|_| DesktopProject {
            revision: Revision::INITIAL,
            root: PathBuf::from("fixture"),
        }))
    }
    async fn perform(
        &self,
        action: &DesktopAction,
        root: Option<&Path>,
    ) -> Result<(), DesktopError> {
        self.calls
            .lock()
            .expect("isolated desktop fixture")
            .push((action.method().into(), root.map(Path::to_owned)));
        self.failure.map_or(Ok(()), Err)
    }
}

fn key(value: &str) -> IdempotencyKey {
    IdempotencyKey::parse(value).expect("isolated desktop fixture")
}

#[tokio::test]
async fn desktop_dispatch_replays_success_failure_and_uncertain_crash_without_another_effect() {
    let root = std::env::temp_dir().join(format!("alcomd-desktop-{}", OperationId::new()));
    std::fs::create_dir(&root).expect("isolated desktop fixture");
    let path = root.join("state.sqlite3");
    let store = StateStoreHandle::open(path.clone()).expect("isolated desktop fixture");
    let access = AccessContext::local_owner();
    let project = store
        .register_project(
            access.principal().clone(),
            ProjectObservation {
                root_path: root.to_str().expect("isolated desktop fixture").to_owned(),
                path_identity_key: vec![2; 24],
                project_type: ProjectType::Unknown,
                unity_version: "2022.3.22f1".into(),
                unity_revision: None,
                vpm_manifest: ManifestState::Missing,
                upm_manifest: ManifestState::Missing,
                direct_dependencies: vec![],
                locked_dependencies: vec![],
                issues: vec![],
            },
            key("register"),
            1,
        )
        .await
        .expect("isolated desktop fixture")
        .value;
    let desktop = RecordedDesktop::default();
    let focus = DesktopAction::Focus {
        project_id: project.project_id,
        expected_revision: project.revision,
    };
    let dir = DesktopAction::Directory {
        project_id: project.project_id,
    };
    for action in [focus.clone(), dir, DesktopAction::Download] {
        for _ in 0..2 {
            run_desktop_action(&access, &store, &desktop, action.clone(), "same-key".into())
                .await
                .expect("isolated desktop fixture");
        }
    }
    assert_eq!(
        desktop
            .calls
            .lock()
            .expect("isolated desktop fixture")
            .len(),
        3
    );
    let stale = DesktopAction::Focus {
        project_id: project.project_id,
        expected_revision: project
            .revision
            .checked_next()
            .expect("isolated desktop fixture"),
    };
    assert_eq!(
        run_desktop_action(&access, &store, &desktop, stale.clone(), "same-key".into()).await,
        Err(DesktopError::IdempotencyConflict)
    );
    assert_eq!(
        run_desktop_action(&access, &store, &desktop, stale, "new-key".into()).await,
        Err(DesktopError::ResourceRevisionConflict)
    );
    let no_desktop = RecordedDesktop {
        failure: Some(DesktopError::DesktopUnavailable),
        ..Default::default()
    };
    for _ in 0..2 {
        assert_eq!(
            run_desktop_action(
                &access,
                &store,
                &no_desktop,
                DesktopAction::Download,
                "headless".into()
            )
            .await,
            Err(DesktopError::DesktopUnavailable)
        );
    }
    assert_eq!(
        no_desktop
            .calls
            .lock()
            .expect("isolated desktop fixture")
            .len(),
        1
    );
    let interrupted = DesktopRequest {
        owner: access.principal().clone(),
        action: DesktopAction::Download,
        key: key("lost-at-effect"),
    };
    let DesktopAcceptance::Accepted(id) = store
        .desktop_accept(interrupted.clone(), None, 4)
        .await
        .expect("isolated desktop fixture")
    else {
        panic!("expected new operation")
    };
    assert_eq!(
        store
            .desktop_replay(interrupted)
            .await
            .expect("isolated desktop fixture"),
        Some(Err(DesktopError::DesktopResultUnknown))
    );
    drop(store);
    let store = StateStoreHandle::open(path).expect("isolated desktop fixture");
    assert!(
        !StateStore::recover(&store, 5)
            .await
            .expect("isolated desktop fixture")
            .contains(&id)
    );
    assert_eq!(
        run_desktop_action(
            &access,
            &store,
            &desktop,
            DesktopAction::Download,
            "lost-at-effect".into()
        )
        .await,
        Err(DesktopError::DesktopResultUnknown)
    );
    run_desktop_action(&access, &store, &desktop, focus, "same-key".into())
        .await
        .expect("isolated desktop fixture");
    assert_eq!(
        desktop
            .calls
            .lock()
            .expect("isolated desktop fixture")
            .len(),
        3
    );
    drop(store);
    std::fs::remove_dir_all(root).expect("isolated desktop fixture");
}

#[tokio::test]
async fn revoked_delegation_cannot_accept_prepared_desktop_action_or_replay_a_receipt() {
    let root = std::env::temp_dir().join(format!("alcomd-desktop-revoke-{}", OperationId::new()));
    std::fs::create_dir(&root).expect("isolated desktop fixture");
    let store = StateStoreHandle::open(root.join("state.db")).expect("isolated desktop fixture");
    let owner = PrincipalId::local_owner();
    let client = store
        .mcp_write(
            owner.clone(),
            "create".into(),
            McpIdentityWrite::Create { name: "AI".into() },
            1,
        )
        .await
        .expect("isolated desktop fixture")
        .client;
    let hash = mcp_credential_hash("test credential");
    let client = store
        .mcp_write(
            owner.clone(),
            "issue".into(),
            McpIdentityWrite::Issue {
                id: client.client_id,
                expected: client.revision,
                hash,
            },
            2,
        )
        .await
        .expect("isolated desktop fixture")
        .client;
    let auth = ToolAuthentication::Credential(hash);
    let access = mcp_access(&client).expect("isolated desktop fixture");
    let desktop = RecordedDesktop::default();
    with_workflow_authentication(
        auth.clone(),
        run_desktop_action(
            &access,
            &store,
            &desktop,
            DesktopAction::Download,
            "done".into(),
        ),
    )
    .await
    .expect("isolated desktop fixture");
    let pending = DesktopRequest {
        owner: access.principal().clone(),
        action: DesktopAction::Download,
        key: key("new"),
    };
    assert_eq!(
        with_workflow_authentication(auth.clone(), store.desktop_replay(pending.clone()))
            .await
            .expect("isolated desktop fixture"),
        None
    );
    store
        .mcp_write(
            owner,
            "revoke".into(),
            McpIdentityWrite::Revoke {
                id: client.client_id,
                expected: client.revision,
            },
            3,
        )
        .await
        .expect("isolated desktop fixture");
    assert!(matches!(
        with_workflow_authentication(auth.clone(), store.desktop_accept(pending, None, 4)).await,
        Err(DesktopError::PermissionDenied)
    ));
    assert_eq!(
        with_workflow_authentication(
            auth,
            run_desktop_action(
                &access,
                &store,
                &desktop,
                DesktopAction::Download,
                "done".into()
            )
        )
        .await,
        Err(DesktopError::PermissionDenied)
    );
    assert_eq!(
        desktop
            .calls
            .lock()
            .expect("isolated desktop fixture")
            .len(),
        1
    );
    drop(store);
    std::fs::remove_dir_all(root).expect("isolated desktop fixture");
}
