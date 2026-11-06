use alcomd_application::{
    ManifestState, ProjectObservation, ProjectTemplateRef, ProjectType, StoreErrorKind,
};

use super::*;

fn fixture() -> (crate::events_tests::Directory, Connection) {
    let directory = crate::events_tests::Directory::new();
    let connection = sqlite::initialize_connection(&directory.database()).expect("isolated store");
    (directory, connection)
}

fn declaration() -> ProjectTemplateDeclaration {
    ProjectTemplateDeclaration {
        id: "com.example.template".to_owned(),
        name: "Fixture".to_owned(),
        editor_version: Some("2022.3.22f1".to_owned()),
        archive_sha256: "ab".repeat(32),
        dependencies: Vec::new(),
        locked_versions: Default::default(),
    }
}

fn key(value: &str) -> IdempotencyKey {
    IdempotencyKey::parse(value).expect("fixture key")
}

fn save_request() -> ProjectTemplateJobRequest {
    ProjectTemplateJobRequest::Save {
        source: ProjectTemplateSource::File {
            source_file: "/fixture.alcomdtemplate".to_owned(),
        },
        expected_revision: None,
    }
}

#[test]
fn template_save_job_publishes_catalogue_and_terminal_events_and_replays() {
    let (_directory, mut connection) = fixture();
    let owner = PrincipalId::local_owner();
    let declaration = declaration();
    let accepted = accept(
        &mut connection,
        &owner,
        save_request(),
        &key("save"),
        "{}",
        1,
    )
    .expect("accept");
    let id = accepted.operation_id;
    begin_work(&mut connection, id, 2)
        .expect("start")
        .expect("work");
    connection
        .execute_batch(
            "CREATE TEMP TRIGGER reject_template_finish BEFORE INSERT ON events
         WHEN NEW.kind='operation.state_changed' BEGIN SELECT RAISE(ABORT,'fixture failure'); END;",
        )
        .expect("simulate terminal event failure");
    assert!(finish_save(&mut connection, id, declaration.clone(), 3).is_err());
    assert!(
        project_templates::list(&connection, &owner, 100, None)
            .expect("catalogue")
            .templates
            .is_empty()
    );
    assert_eq!(
        sqlite::load_owned_operation(&connection, &owner, id)
            .expect("visible")
            .state,
        OperationState::Running,
    );
    connection
        .execute_batch("DROP TRIGGER reject_template_finish")
        .expect("restore event writer");
    finish_save(&mut connection, id, declaration.clone(), 3).expect("commit");
    let operation = sqlite::load_owned_operation(&connection, &owner, id).expect("visible");
    assert_eq!(operation.state, OperationState::Succeeded);
    let result = serde_json::from_str::<serde_json::Value>(&operation.result_json.expect("result"))
        .expect("JSON result");
    assert_eq!(result["template"]["id"], declaration.id);
    assert_eq!(result["template"]["editorVersion"], "2022.3.22f1");
    assert!(result["template"].get("archiveSha256").is_none());
    let events = sqlite::list_events(&connection, &owner, 0, 100)
        .expect("events")
        .events;
    assert_eq!(events[events.len() - 2].kind, "project-template.changed");
    assert_eq!(events[events.len() - 1].kind, "operation.state_changed");
    let replayed = accept(
        &mut connection,
        &owner,
        save_request(),
        &key("save"),
        "{}",
        4,
    )
    .expect("same request replays");
    assert!(replayed.replayed);
    assert_eq!(replayed.operation_id, id);
    assert_eq!(
        accept(
            &mut connection,
            &owner,
            save_request(),
            &key("save"),
            r#"{"other":1}"#,
            5
        )
        .expect_err("changed request conflicts")
        .code(),
        ProjectCreateErrorCode::IdempotencyConflict
    );
    let duplicate = accept(
        &mut connection,
        &owner,
        save_request(),
        &key("duplicate"),
        "{}",
        5,
    )
    .expect("second task");
    begin_work(&mut connection, duplicate.operation_id, 6).expect("start");
    finish_save(&mut connection, duplicate.operation_id, declaration, 7).expect("reuse record");
    let events = sqlite::list_events(&connection, &owner, 0, 100)
        .expect("events")
        .events;
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == "project-template.changed")
            .count(),
        1
    );
}

#[test]
fn project_template_save_rechecks_the_source_revision_at_commit() {
    let (_directory, mut connection) = fixture();
    let owner = PrincipalId::local_owner();
    let project = m3::register_project(
        &mut connection,
        &owner,
        ProjectObservation {
            root_path: "/canonical/project".to_owned(),
            path_identity_key: vec![20],
            project_type: ProjectType::Unknown,
            unity_version: "2022.3.22f1".to_owned(),
            unity_revision: None,
            vpm_manifest: ManifestState::Missing,
            upm_manifest: ManifestState::Valid,
            direct_dependencies: Vec::new(),
            locked_dependencies: Vec::new(),
            issues: Vec::new(),
        },
        &key("register"),
        1,
    )
    .expect("register project")
    .value;
    let declaration = declaration();
    let request = ProjectTemplateJobRequest::Save {
        source: ProjectTemplateSource::Project {
            project_id: project.project_id,
            expected_project_revision: project.revision.get(),
            id: declaration.id.clone(),
            name: declaration.name.clone(),
            include_assets: false,
            lock_package_versions: false,
        },
        expected_revision: None,
    };
    let id = accept(&mut connection, &owner, request, &key("save"), "{}", 2)
        .expect("accept")
        .operation_id;
    begin_work(&mut connection, id, 3).expect("start");
    m3::set_project_favorite(
        &mut connection,
        &owner,
        project.project_id,
        true,
        project.revision,
        &key("favorite"),
        4,
    )
    .expect("revise registered project");
    assert_eq!(
        finish_save(&mut connection, id, declaration, 5)
            .expect_err("changed source cannot commit")
            .code(),
        ProjectCreateErrorCode::TemplateStale,
    );
    assert!(
        project_templates::list(&connection, &owner, 100, None)
            .expect("catalogue")
            .templates
            .is_empty()
    );
}

#[test]
fn template_cancelled_save_never_changes_catalogue() {
    let (_directory, mut connection) = fixture();
    let owner = PrincipalId::local_owner();
    let id = accept(
        &mut connection,
        &owner,
        save_request(),
        &key("save"),
        "{}",
        1,
    )
    .expect("accept")
    .operation_id;
    begin_work(&mut connection, id, 2).expect("start");
    let operation = sqlite::load_owned_operation(&connection, &owner, id).expect("visible");
    sqlite::cancel_operation(
        &mut connection,
        &owner,
        id,
        operation.revision,
        &key("cancel"),
        3,
    )
    .expect("cancellation requested");
    finish_save(&mut connection, id, declaration(), 4).expect("cancellation committed");
    let operation = sqlite::load_owned_operation(&connection, &owner, id).expect("visible");
    assert_eq!(operation.state, OperationState::Cancelled);
    assert!(
        project_templates::list(&connection, &owner, 100, None)
            .expect("catalogue")
            .templates
            .is_empty()
    );
}

#[test]
fn template_export_recovers_the_frozen_archive_and_fences_cancellation() {
    let (_directory, mut connection) = fixture();
    let owner = PrincipalId::local_owner();
    let declaration = declaration();
    let record = project_templates::save(&mut connection, &owner, declaration.clone(), None, 1)
        .expect("save fixture");
    let reference = ProjectTemplateRef {
        id: declaration.id.clone(),
        revision: record.revision,
    };
    let request = ProjectTemplateJobRequest::Export {
        reference,
        destination_file: "/out.alcomdtemplate".to_owned(),
    };
    let id = accept(
        &mut connection,
        &owner,
        request.clone(),
        &key("export"),
        "{}",
        2,
    )
    .expect("accept")
    .operation_id;
    begin_work(&mut connection, id, 3).expect("start");
    assert!(publish_intent(&mut connection, id, 4).expect("intent persisted"));
    project_templates::remove(&mut connection, &owner, &declaration.id, record.revision, 5)
        .expect("remove catalogue reference");
    assert_eq!(pending(&mut connection, 6).expect("recover"), [id]);
    let work = begin_work(&mut connection, id, 7)
        .expect("resume")
        .expect("work");
    assert!(work.publishing);
    assert_eq!(work.export_declaration, Some(declaration));
    let operation = sqlite::load_owned_operation(&connection, &owner, id).expect("visible");
    assert_eq!(
        sqlite::cancel_operation(
            &mut connection,
            &owner,
            id,
            operation.revision,
            &key("cancel"),
            8
        )
        .expect_err("publication intent is fenced")
        .kind(),
        StoreErrorKind::OperationNotCancellable
    );
    finish_export(&mut connection, id, 9).expect("complete");
    assert_eq!(
        sqlite::load_owned_operation(&connection, &owner, id)
            .expect("visible")
            .state,
        OperationState::Succeeded
    );
    assert!(
        accept(&mut connection, &owner, request, &key("export"), "{}", 10)
            .expect("replay after removal")
            .replayed
    );
}
