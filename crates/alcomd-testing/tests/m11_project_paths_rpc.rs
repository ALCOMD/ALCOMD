#[path = "support/mcp_actor.rs"]
mod mcp_actor;

use std::{fs, path::Path, path::PathBuf, time::Duration};

use alcomd_client::{AlcomdClient, ClientConfig, ClientError};
use alcomd_platform::{DataConfig, IpcConfig};
use alcomd_protocol::{
    McpClientMutationParams, OperationState, ProjectRegisterParams, ProjectSource, ProjectTarget,
    ProjectTemplateRef, ProjectsApplyCopyParams, ProjectsCreateParams, ProjectsPlanCopyParams,
};
use tokio::sync::oneshot;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mcp_project_paths_are_direct_and_copy_rechecks_its_persisted_destination() {
    let fixture = Fixture::new();
    let configured = fixture.0.join("configured");
    let external = fixture.0.join("外部 路径");
    let runtime = fixture.0.join("runtime");
    let state = fixture.0.join("state");
    for path in [&configured, &external, &runtime, &state] {
        fs::create_dir(path).expect("isolated path");
    }
    #[cfg(unix)]
    let (ipc, config) = (
        IpcConfig::isolated(runtime.clone()),
        ClientConfig::default()
            .without_daemon_start()
            .with_runtime_directory(runtime),
    );
    #[cfg(windows)]
    let (ipc, config) = (
        IpcConfig::default(),
        ClientConfig::default().without_daemon_start(),
    );

    let (stop, stopped) = oneshot::channel();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc.clone(),
        DataConfig::isolated(state.clone()),
        async {
            let _ = stopped.await;
        },
    )
    .await
    .expect("bind an isolated data daemon");
    let mut owner = AlcomdClient::connect(config.clone())
        .await
        .expect("owner IPC");
    let settings = owner.settings_get().await.expect("read settings");
    owner
        .settings_update(
            serde_json::from_value(serde_json::json!({
                "expectedRevision": settings.revision,
                "update": {
                    "paths": {"projectDirectory": configured},
                    "projects": {"autoDiscover": false}
                }
            }))
            .expect("settings update"),
        )
        .await
        .expect("configured projects root");
    let issued = mcp_actor::issue_actor(&mut owner, "first").await;
    let other = mcp_actor::issue_actor(&mut owner, "second").await;
    let actor_config = config.clone().with_mcp_credential(issued.credential);
    let mut actor = AlcomdClient::connect(actor_config.clone())
        .await
        .expect("actor IPC");
    let mut second = AlcomdClient::connect(config.clone().with_mcp_credential(other.credential))
        .await
        .expect("second actor IPC");

    let source = external.join("源 工程");
    write_project(&source);
    let register = ProjectRegisterParams {
        source: ProjectSource::Absolute {
            path: text(&source),
        },
        idempotency_key: "register-absolute".into(),
    };
    for path in [
        "relative/source".to_owned(),
        text(&external.join("..").join("configured")),
    ] {
        let error = actor
            .project_register(ProjectRegisterParams {
                source: ProjectSource::Absolute { path },
                ..register.clone()
            })
            .await
            .expect_err("require an unambiguous absolute path");
        assert_code(error, "invalid_request");
    }
    let project = actor
        .project_register(register.clone())
        .await
        .expect("register absolute");
    assert!(
        serde_json::to_string(&project)
            .expect("public response")
            .contains("外部 路径")
    );
    let moved_source = external.join("offline source");
    fs::rename(&source, &moved_source).expect("temporarily remove the source after acceptance");
    let replayed = actor
        .project_register(register.clone())
        .await
        .expect("read the original registration without probing its source");
    assert!(replayed.replayed);
    assert_eq!(replayed.project, project.project);
    assert_code(
        actor
            .project_register(ProjectRegisterParams {
                source: ProjectSource::Absolute {
                    path: text(&moved_source),
                },
                ..register
            })
            .await
            .expect_err("same key with a changed source"),
        "idempotency_conflict",
    );
    fs::rename(&moved_source, &source).expect("restore fixture source");
    assert!(
        owner
            .project_get(project.project.project_id.clone().expect("registered ID"))
            .await
            .is_ok()
    );

    write_project(&configured.join("Default project"));
    assert!(
        actor
            .project_register(ProjectRegisterParams {
                source: ProjectSource::Default {
                    relative_path: "Default project".into()
                },
                idempotency_key: "register-default".into(),
            })
            .await
            .is_ok()
    );

    let templates = actor
        .project_templates_list(Default::default())
        .await
        .expect("templates");
    let blank = templates
        .templates
        .iter()
        .find(|template| template.locked && template.editor_version.is_none())
        .expect("built-in blank template");
    let create_path = external.join("新 工程");
    let create = ProjectsCreateParams {
        target: ProjectTarget::Absolute {
            parent_path: text(&external),
            name: "新 工程".into(),
        },
        unity_version: "2022.3.22f1".into(),
        template: Some(ProjectTemplateRef {
            id: blank.id.clone(),
            revision: blank.revision,
        }),
        idempotency_key: "create-absolute".into(),
    };
    let created = actor
        .projects_create(create.clone())
        .await
        .expect("create absolute");
    succeeded(&mut actor, &created.operation_id).await;
    assert!(
        create_path
            .join("ProjectSettings/ProjectVersion.txt")
            .exists()
    );
    assert_eq!(
        actor
            .projects_create(create.clone())
            .await
            .expect("create replay")
            .operation_id,
        created.operation_id
    );
    assert!(owner.project_get(created.project_id).await.is_ok());
    let occupied = actor
        .projects_create(ProjectsCreateParams {
            idempotency_key: "create-over-existing".into(),
            ..create
        })
        .await
        .expect_err("must not replace existing project");
    assert_code(occupied, "project_create_target_exists");

    let plan_copy = ProjectsPlanCopyParams {
        source_project_id: project.project.project_id.expect("registered ID"),
        expected_revision: project.project.revision.expect("registered revision"),
        target: ProjectTarget::Absolute {
            parent_path: text(&external),
            name: "已 复制".into(),
        },
        idempotency_key: "plan-absolute-copy".into(),
    };
    let plan = actor
        .project_plan_copy(plan_copy.clone())
        .await
        .expect("plan")
        .plan;
    assert!(
        serde_json::to_string(&plan)
            .expect("public response")
            .contains("外部 路径")
    );
    assert_eq!(
        actor
            .project_plan_copy(plan_copy.clone())
            .await
            .expect("plan replay")
            .plan
            .plan_id,
        plan.plan_id
    );
    let lookup = alcomd_protocol::PlansGetParams {
        kind: alcomd_protocol::PlanKind::ProjectCopy,
        plan_id: plan.plan_id.clone(),
    };
    let saved = actor
        .get_plan(lookup.clone())
        .await
        .expect("read own copy plan");
    let alcomd_protocol::PlansGetResult::ProjectCopy { plan: original } = &saved else {
        panic!("expected a copy plan");
    };
    assert_eq!(original.plan_id, plan.plan_id);
    assert!(second.get_plan(lookup.clone()).await.is_err());
    assert!(owner.get_plan(lookup.clone()).await.is_err());
    assert!(
        actor
            .get_plan(alcomd_protocol::PlansGetParams {
                kind: alcomd_protocol::PlanKind::Package,
                plan_id: plan.plan_id.clone(),
            })
            .await
            .is_err()
    );
    let apply = ProjectsApplyCopyParams {
        plan_id: plan.plan_id,
        expected_revision: plan.source_project_revision,
        idempotency_key: "apply-absolute-copy".into(),
    };
    assert!(
        second.project_apply_copy(apply.clone()).await.is_err(),
        "plan owner binding"
    );

    let movable = external.join("replaceable parent");
    fs::create_dir(&movable).expect("replaceable parent");
    let target = movable.join("never created");
    let changing_request = ProjectsPlanCopyParams {
        target: ProjectTarget::Absolute {
            parent_path: text(&movable),
            name: "never created".into(),
        },
        idempotency_key: "parent-change-plan".into(),
        ..plan_copy.clone()
    };
    let changed = actor
        .project_plan_copy(changing_request.clone())
        .await
        .expect("freeze parent identity")
        .plan;
    let original = external.join("previous parent");
    fs::rename(&movable, &original).expect("move original parent");
    let replay = actor
        .project_plan_copy(changing_request.clone())
        .await
        .expect("replay with the original parent offline");
    assert!(replay.replayed);
    assert_eq!(
        serde_json::to_value(&replay.plan).expect("replayed plan"),
        serde_json::to_value(&changed).expect("original plan")
    );
    assert_code(
        actor
            .project_plan_copy(ProjectsPlanCopyParams {
                target: ProjectTarget::Default {
                    name: "never created".into(),
                },
                ..changing_request
            })
            .await
            .expect_err("changed destination with same key"),
        "idempotency_conflict",
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&original, &movable).expect("link to original parent");
        assert_code(
            actor
                .project_apply_copy(ProjectsApplyCopyParams {
                    plan_id: changed.plan_id.clone(),
                    expected_revision: changed.source_project_revision,
                    idempotency_key: "linked-parent".into(),
                })
                .await
                .expect_err("reject a link even to the original parent"),
            "project_copy_target_unsafe",
        );
        fs::remove_file(&movable).expect("remove test link");
    }
    fs::create_dir(&movable).expect("replace parent");
    assert_code(
        actor
            .project_apply_copy(ProjectsApplyCopyParams {
                plan_id: changed.plan_id,
                expected_revision: changed.source_project_revision,
                idempotency_key: "parent-change-apply".into(),
            })
            .await
            .expect_err("reject changed parent identity"),
        "project_copy_plan_stale",
    );
    assert!(!target.exists() && !original.join("never created").exists());

    drop(second);
    drop(actor);
    drop(owner);
    stop.send(()).expect("stop");
    daemon.await.expect("join").expect("shutdown");
    let (stop, stopped) = oneshot::channel();
    let daemon = alcomd_daemon::test_support::start_with_data_until(
        ipc,
        DataConfig::isolated(state),
        async {
            let _ = stopped.await;
        },
    )
    .await
    .expect("reopen durable plan");
    let mut reader = AlcomdClient::connect(actor_config.clone())
        .await
        .expect("plan reader IPC");
    let mut actor = AlcomdClient::connect(actor_config)
        .await
        .expect("reconnect actor");
    assert_eq!(
        actor
            .get_plan(lookup.clone())
            .await
            .expect("reopen original plan"),
        saved
    );
    let copied = actor
        .project_apply_copy(apply.clone())
        .await
        .expect("apply persisted plan");
    succeeded(&mut actor, &copied.operation_id).await;
    assert_eq!(
        fs::read(external.join("已 复制/Assets/source.bin")).expect("copied content"),
        b"fixture source"
    );
    let replay = actor
        .project_apply_copy(apply.clone())
        .await
        .expect("apply replay");
    assert!(replay.replayed);
    assert_eq!(replay.operation_id, copied.operation_id);
    let mut owner = AlcomdClient::connect(config)
        .await
        .expect("owner reconnect");
    let delete = actor
        .project_plan_delete_directory(alcomd_protocol::ProjectsPlanDeleteDirectoryParams {
            project_id: copied.target_project_id.clone(),
            expected_revision: 1,
            idempotency_key: "plan-delete-copy".into(),
        })
        .await
        .expect("plan deletion of a shared project")
        .plan;
    let apply_delete = alcomd_protocol::ProjectsApplyDeleteDirectoryParams {
        plan_id: delete.plan_id,
        expected_revision: 1,
        idempotency_key: "apply-delete-copy".into(),
    };
    assert!(
        owner
            .project_apply_delete_directory(apply_delete.clone())
            .await
            .is_err()
    );
    let deleted = actor
        .project_apply_delete_directory(apply_delete)
        .await
        .expect("apply own delete plan");
    succeeded(&mut actor, &deleted.operation_id).await;
    assert!(!external.join("已 复制").exists());
    assert!(owner.project_get(copied.target_project_id).await.is_err());
    owner
        .mcp_client_revoke(McpClientMutationParams {
            client_id: issued.client_id,
            expected_revision: issued.revision,
            idempotency_key: "revoke-copy".into(),
        })
        .await
        .expect("revoke actor");
    assert_code(
        reader
            .get_plan(lookup)
            .await
            .expect_err("revoked plan read"),
        "permission_denied",
    );
    assert_code(
        actor
            .project_apply_copy(apply)
            .await
            .expect_err("recheck authorization"),
        "permission_denied",
    );
    stop.send(()).expect("stop");
    daemon.await.expect("join").expect("shutdown");
}

async fn succeeded(client: &mut AlcomdClient, id: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let result = client
            .operation_get(id.to_owned())
            .await
            .expect("operation");
        match result.state {
            OperationState::Succeeded => return,
            OperationState::Failed | OperationState::Cancelled => panic!("{result:?}"),
            _ => {}
        }
        assert!(tokio::time::Instant::now() < deadline, "{result:?}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn assert_code(error: ClientError, expected: &str) {
    assert!(
        matches!(error, ClientError::Remote(ref failure) if failure.code == expected),
        "expected {expected}, got {error:?}"
    );
}

fn write_project(path: &Path) {
    for directory in ["Assets", "Packages", "ProjectSettings"] {
        fs::create_dir_all(path.join(directory)).expect("Unity directory");
    }
    fs::write(path.join("Assets/source.bin"), b"fixture source").expect("content");
    fs::write(
        path.join("Packages/manifest.json"),
        br#"{"dependencies":{}}"#,
    )
    .expect("manifest");
    fs::write(
        path.join("ProjectSettings/ProjectVersion.txt"),
        "m_EditorVersion: 2022.3.22f1\n",
    )
    .expect("Unity version");
}

fn text(path: &Path) -> String {
    path.to_str().expect("Unicode fixture path").to_owned()
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        #[cfg(target_os = "macos")]
        let base = PathBuf::from("/private/tmp");
        #[cfg(not(target_os = "macos"))]
        let base = std::env::temp_dir();
        // Keep the Unix IPC socket name within the native sockaddr bound.
        let unique = uuid::Uuid::new_v4().simple().to_string();
        let path = base.join(format!("a11-{}", &unique[..12]));
        fs::create_dir(&path).expect("fixture root");
        Self(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
