use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use alcomd_application::{
    AccessContext, IdempotencyKey, M3RegistryStore, M5TemplateApplication, M5TemplateError,
    M5TemplateWriterGate, ManifestState, OperationState, PrincipalId, ProjectId,
    ProjectObservation, ProjectType, StateStore, TemplateId, TemplateSourceKind, UnityWriterState,
    UnityWriterStateKind,
};
use alcomd_store::StateStoreHandle;
use alcomd_vpm::TemplateEngine;
use uuid::Uuid;

#[derive(Clone)]
struct FixedWriter(UnityWriterStateKind);

impl M5TemplateWriterGate for FixedWriter {
    async fn observe_template_source(
        &self,
        _: &AccessContext,
        project_id: ProjectId,
    ) -> Result<UnityWriterState, M5TemplateError> {
        Ok(UnityWriterState {
            project_id,
            state: self.0,
            evidence: Vec::new(),
            checked_at_ms: 1,
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn template_registry_derive_export_remove_and_import_are_durable() {
    let fixture = TestDirectory::new();
    let project_root = fixture.path().join("Project");
    create_project(&project_root);
    let store = StateStoreHandle::open(fixture.path().join("state.db")).expect("open store");
    let engine = TemplateEngine::new(fixture.path().join("template-store")).expect("engine");
    let application = M5TemplateApplication::new(
        store.clone(),
        engine.clone(),
        FixedWriter(UnityWriterStateKind::NotObserved),
    );
    let access = AccessContext::local_owner();

    let builtins = engine
        .materialize_builtins(&fixture.path().join("builtin-staging"))
        .expect("materialize builtins");
    application
        .ensure_builtins(builtins)
        .await
        .expect("register builtins");
    assert_eq!(
        application
            .list(&access, None, 100)
            .await
            .expect("list")
            .templates
            .len(),
        3
    );

    let project = store
        .register_project(
            PrincipalId::local_owner(),
            project_observation(&project_root),
            key("template-project"),
            1,
        )
        .await
        .expect("register project")
        .value;
    let template_id = TemplateId::new();
    let plan = application
        .plan_derive(
            &access,
            project.project_id,
            project.revision,
            template_id,
            "1".to_owned(),
            "Derived fixture".to_owned(),
            Some("Synthetic M5 derive fixture".to_owned()),
        )
        .await
        .expect("plan derive");
    let apply = application
        .apply_derive(&access, plan.plan_id, key("derive"))
        .await
        .expect("apply derive");
    wait_for_success(&store, apply.operation_id).await;

    let derived = application
        .get(&access, template_id)
        .await
        .expect("get derived");
    assert_eq!(derived.source_kind, TemplateSourceKind::User);
    assert_eq!(derived.provenance, "derived");
    let exported = fixture.path().join("derived.alcomdtemplate");
    application
        .export(
            &access,
            template_id,
            derived.revision,
            exported.to_string_lossy().into_owned(),
        )
        .await
        .expect("export derived bundle");
    assert!(exported.is_file());

    let (removed, replayed) = application
        .remove(
            &access,
            template_id,
            derived.revision,
            key("remove-derived"),
        )
        .await
        .expect("remove derived registry binding");
    assert!(removed);
    assert!(!replayed);

    let import_plan = application
        .plan_import(
            &access,
            exported.to_string_lossy().into_owned(),
            false,
            None,
        )
        .await
        .expect("plan import");
    let imported = application
        .apply_import(&access, import_plan.plan_id, key("import-derived"))
        .await
        .expect("apply import");
    wait_for_success(&store, imported.operation_id).await;
    let record = application
        .get(&access, template_id)
        .await
        .expect("get import");
    assert_eq!(record.bundle_sha256, derived.bundle_sha256);
    assert_eq!(record.provenance, "derived");

    let (favorite, replayed) = application
        .set_favorite(
            &access,
            template_id,
            true,
            record.revision,
            key("favorite-import"),
        )
        .await
        .expect("favorite imported template");
    assert!(favorite.favorite);
    assert!(!replayed);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn template_create_project_publishes_then_registers_one_durable_project() {
    let fixture = TestDirectory::new();
    let store = StateStoreHandle::open(fixture.path().join("state.db")).expect("open store");
    let engine = TemplateEngine::new(fixture.path().join("template-store")).expect("engine");
    let application = M5TemplateApplication::new(
        store.clone(),
        engine.clone(),
        FixedWriter(UnityWriterStateKind::NotObserved),
    );
    let access = AccessContext::local_owner();
    let builtins = engine
        .materialize_builtins(&fixture.path().join("builtin-create-staging"))
        .expect("materialize builtins");
    let blank = builtins
        .iter()
        .find(|record| record.template_id.to_string().ends_with("b001"))
        .expect("blank builtin")
        .clone();
    application
        .ensure_builtins(builtins)
        .await
        .expect("register builtins");

    let plan = application
        .plan_create_project(
            &access,
            blank.template_id,
            blank.revision,
            fixture.path().to_string_lossy().into_owned(),
            "CreatedFromTemplate".to_owned(),
        )
        .await
        .expect("plan create project");
    let accepted = application
        .apply_create_project(&access, plan.plan_id, key("create-project"))
        .await
        .expect("apply create project");
    wait_for_success(&store, accepted.operation_id).await;
    let replay = application
        .apply_create_project(&access, plan.plan_id, key("create-project"))
        .await
        .expect("replay create project");
    assert!(replay.replayed);
    assert!(!replay.schedule);
    assert_eq!(replay.operation_id, accepted.operation_id);

    let projects = store
        .list_projects(PrincipalId::local_owner(), None, 10)
        .await
        .expect("list projects");
    assert_eq!(projects.projects.len(), 1);
    let created = &projects.projects[0];
    assert!(Path::new(&created.observation.root_path).ends_with(Path::new("CreatedFromTemplate")));
    let target = PathBuf::from(&created.observation.root_path);
    assert!(target.join("Packages/vpm-manifest.json").is_file());
    assert!(
        target
            .join("Library/ALCOMD/create-project-evidence.json")
            .is_file()
    );
    assert_eq!(
        fs::read_to_string(target.join("Packages/manifest.json")).expect("UPM manifest"),
        "{\n    \"dependencies\": {}\n}\n"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn template_derive_rejects_confirmed_writer_and_changed_project_snapshot() {
    let fixture = TestDirectory::new();
    let project_root = fixture.path().join("Project");
    create_project(&project_root);
    let store = StateStoreHandle::open(fixture.path().join("state.db")).expect("open store");
    let engine = TemplateEngine::new(fixture.path().join("template-store")).expect("engine");
    let access = AccessContext::local_owner();
    let project = store
        .register_project(
            PrincipalId::local_owner(),
            project_observation(&project_root),
            key("template-project"),
            1,
        )
        .await
        .expect("register project")
        .value;

    let confirmed = M5TemplateApplication::new(
        store.clone(),
        engine.clone(),
        FixedWriter(UnityWriterStateKind::RunningConfirmed),
    );
    let error = confirmed
        .plan_derive(
            &access,
            project.project_id,
            project.revision,
            TemplateId::new(),
            "1".to_owned(),
            "Blocked".to_owned(),
            None,
        )
        .await
        .expect_err("confirmed Unity writer must reject derive planning");
    assert_eq!(
        alcomd_application::template_error_name(error.code()),
        "unity_project_running"
    );

    let application = M5TemplateApplication::new(
        store.clone(),
        engine,
        FixedWriter(UnityWriterStateKind::NotObserved),
    );
    let template_id = TemplateId::new();
    let plan = application
        .plan_derive(
            &access,
            project.project_id,
            project.revision,
            template_id,
            "1".to_owned(),
            "Changed".to_owned(),
            None,
        )
        .await
        .expect("plan derive before source change");
    fs::write(
        project_root.join("Assets/source.txt"),
        b"changed after plan",
    )
    .expect("change source after plan");
    let apply = application
        .apply_derive(&access, plan.plan_id, key("derive-changed"))
        .await
        .expect("accept stale derive operation");
    wait_for_failure(
        &store,
        apply.operation_id,
        "project_changed_during_template_create",
    )
    .await;
    assert!(application.get(&access, template_id).await.is_err());
}

async fn wait_for_success(store: &StateStoreHandle, operation_id: alcomd_application::OperationId) {
    for _ in 0..200 {
        let operation = store
            .get_operation(PrincipalId::local_owner(), operation_id)
            .await
            .expect("read Template operation");
        if operation.state.is_terminal() {
            assert_eq!(
                operation.state,
                OperationState::Succeeded,
                "Template operation failed with {:?}",
                operation.error_code
            );
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("Template operation did not complete");
}

async fn wait_for_failure(
    store: &StateStoreHandle,
    operation_id: alcomd_application::OperationId,
    expected_code: &str,
) {
    for _ in 0..200 {
        let operation = store
            .get_operation(PrincipalId::local_owner(), operation_id)
            .await
            .expect("read failed Template operation");
        if operation.state.is_terminal() {
            assert_eq!(operation.state, OperationState::Failed);
            assert_eq!(operation.error_code.as_deref(), Some(expected_code));
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("Template operation did not fail");
}

fn project_observation(root: &Path) -> ProjectObservation {
    ProjectObservation {
        root_path: root.to_string_lossy().into_owned(),
        path_identity_key: alcomd_platform::file_identity_key(root).expect("project identity"),
        project_type: ProjectType::Unknown,
        unity_version: "2022.3.22f1".to_owned(),
        unity_revision: None,
        vpm_manifest: ManifestState::Valid,
        upm_manifest: ManifestState::Valid,
        direct_dependencies: Vec::new(),
        locked_dependencies: Vec::new(),
        issues: Vec::new(),
        observed_at_ms: 1,
    }
}

fn create_project(root: &Path) {
    fs::create_dir_all(root.join("Assets")).expect("Assets");
    fs::create_dir_all(root.join("ProjectSettings")).expect("ProjectSettings");
    fs::create_dir_all(root.join("Packages")).expect("Packages");
    fs::write(root.join("Assets/source.txt"), b"source").expect("asset");
    fs::write(
        root.join("ProjectSettings/ProjectVersion.txt"),
        b"m_EditorVersion: 2022.3.22f1\n",
    )
    .expect("ProjectVersion");
    fs::write(
        root.join("Packages/manifest.json"),
        b"{\"dependencies\":{}}",
    )
    .expect("UPM manifest");
    fs::write(
        root.join("Packages/vpm-manifest.json"),
        b"{\"dependencies\":{},\"locked\":{}}",
    )
    .expect("VPM manifest");
}

fn key(value: &str) -> IdempotencyKey {
    IdempotencyKey::parse(value).expect("valid idempotency key")
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("alcomd-m5-template-{}", Uuid::new_v4()));
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
