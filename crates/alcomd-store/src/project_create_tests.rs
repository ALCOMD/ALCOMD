use alcomd_application::{
    ManifestState, PackageChangeSet, ProjectTemplateDependency, ProjectTemplateRef, ProjectType,
    StoreErrorKind,
};

use super::*;

fn declaration(
    id: &str,
    dependencies: Vec<ProjectTemplateDependency>,
) -> ProjectTemplateDeclaration {
    ProjectTemplateDeclaration {
        id: id.to_owned(),
        name: "同名模板".to_owned(),
        editor_version: Some("2022.3.22f1".to_owned()),
        archive_sha256: "ab".repeat(32),
        dependencies,
        locked_versions: Default::default(),
    }
}

fn input(template: Option<ProjectTemplateRef>) -> ProjectCreateInput {
    ProjectCreateInput {
        name: "Project".to_owned(),
        target_parent: "/fixture".to_owned(),
        unity_version: "2022.3.22f1".to_owned(),
        template,
    }
}

fn acceptance(owner: &PrincipalId, input: ProjectCreateInput) -> ProjectCreateAcceptance {
    ProjectCreateAcceptance {
        owner: owner.clone(),
        input,
        parent_identity: vec![1],
        canonical_parent: "/canonical".to_owned(),
        fingerprint: "{}".to_owned(),
        key: IdempotencyKey::parse("fixture-create").expect("fixture key"),
        allow_package_changes: true,
    }
}

#[test]
fn catalogue_identity_lock_and_frozen_task_are_independent_of_display_names() {
    let dir = crate::events_tests::Directory::new();
    let mut connection = sqlite::initialize_connection(&dir.database()).expect("isolated store");
    let owner = PrincipalId::local_owner();
    let builtin = declaration("com.example.builtin", Vec::new());
    project_templates::seed(&mut connection, &owner, vec![builtin.clone()]).expect("seed");
    let locked = project_templates::save(&mut connection, &owner, builtin.clone(), None, 1)
        .expect("same ID and content returns original builtin");
    assert!(locked.locked);
    assert_eq!(
        project_templates::remove(&mut connection, &owner, &builtin.id, locked.revision, 1)
            .expect_err("builtin cannot be removed")
            .code(),
        ProjectCreateErrorCode::TemplateLocked
    );
    let mut conflict = builtin;
    conflict.name = "Different".to_owned();
    assert_eq!(
        project_templates::save(&mut connection, &owner, conflict, None, 2)
            .expect_err("same ID, different content")
            .code(),
        ProjectCreateErrorCode::TemplateConflict
    );
    let mut user = declaration(
        "com.example.user",
        vec![ProjectTemplateDependency {
            package_id: "com.example.package".to_owned(),
            version_range: "^1.0.0".to_owned(),
        }],
    );
    user.archive_sha256 = "cd".repeat(32);
    let frozen_digest = user.archive_sha256.clone();
    let saved = project_templates::save(&mut connection, &owner, user.clone(), None, 3)
        .expect("same display name is allowed");
    let reference = ProjectTemplateRef {
        id: user.id.clone(),
        revision: saved.revision,
    };
    let mut rejected = acceptance(&owner, input(Some(reference.clone())));
    rejected.allow_package_changes = false;
    assert_eq!(
        accept(&mut connection, rejected, 10)
            .expect_err("package permissions are required")
            .code(),
        ProjectCreateErrorCode::PermissionDenied
    );
    let mut incompatible = acceptance(
        &owner,
        input(Some(ProjectTemplateRef {
            id: locked.declaration.id.clone(),
            revision: locked.revision,
        })),
    );
    incompatible.input.unity_version = "6000.0.1f1".to_owned();
    assert_eq!(
        accept(&mut connection, incompatible, 10)
            .expect_err("a dependency-free declaration still restricts Unity")
            .code(),
        ProjectCreateErrorCode::DependencyUnsatisfied
    );

    let mut old = declaration("com.example.old", Vec::new());
    old.editor_version = None;
    let old = project_templates::save(&mut connection, &owner, old, None, 9)
        .expect("existing template record without a version");
    let old_reference = ProjectTemplateRef {
        id: old.declaration.id.clone(),
        revision: old.revision,
    };
    assert_eq!(
        accept(
            &mut connection,
            acceptance(&owner, input(Some(old_reference))),
            10
        )
        .expect_err("a template without a declared version cannot create a project")
        .code(),
        ProjectCreateErrorCode::DependencyUnsatisfied
    );
    project_templates::remove(
        &mut connection,
        &owner,
        &old.declaration.id,
        old.revision,
        10,
    )
    .expect("retire fixture record before checking the catalogue");

    let accepted = accept(
        &mut connection,
        acceptance(&owner, input(Some(reference.clone()))),
        10,
    )
    .expect("freeze revision");
    let deletion = project_templates::remove(&mut connection, &owner, &user.id, saved.revision, 20)
        .expect("remove selected declaration");
    let mut replacement = user.clone();
    replacement.dependencies.clear();
    replacement.archive_sha256 = "ef".repeat(32);
    let recreated = project_templates::save(&mut connection, &owner, replacement, None, 21)
        .expect("reuse public ID");
    assert!(recreated.revision > deletion && deletion > saved.revision);
    assert!(
        project_templates::archive_references(&connection)
            .expect("read frozen template archive references")
            .contains(&frozen_digest)
    );
    let work = begin_work(&mut connection, accepted.operation_id, 23)
        .expect("start accepted work")
        .expect("pending work");
    assert_eq!(work.declaration, Some(user));
    let pinned = ProjectCreatePackages {
        change_set: PackageChangeSet {
            unity_compatibility_overrides: Vec::new(),
            removal_conflicts: Vec::new(),
            absent_removal_packages: Vec::new(),
            format_version: 1,
            mutations: Vec::new(),
            dependency_edges: Vec::new(),
            legacy_removals: Vec::new(),
            legacy_packages: Vec::new(),
            vpm_manifest_sha256: [0; 32],
        },
        source_set: Vec::new(),
    };
    save_packages(&mut connection, accepted.operation_id, pinned.clone(), 24)
        .expect("persist frozen package evidence");
    assert_eq!(
        pending(&mut connection, 25).expect("recover"),
        [accepted.operation_id]
    );
    let work = begin_work(&mut connection, accepted.operation_id, 26)
        .expect("resume")
        .expect("pending task");
    assert_eq!(work.packages, Some(pinned));
    let replayed = accept(
        &mut connection,
        acceptance(&owner, input(Some(reference.clone()))),
        27,
    )
    .expect("a replay bypasses live revision lookup");
    assert!(replayed.replayed);
    assert_eq!(replayed.operation_id, accepted.operation_id);
    let mut new_attempt = acceptance(&owner, input(Some(reference)));
    new_attempt.key = IdempotencyKey::parse("fixture-stale").expect("new key");
    assert_eq!(
        accept(&mut connection, new_attempt, 28)
            .expect_err("a new attempt rejects stale revision")
            .code(),
        ProjectCreateErrorCode::TemplateStale
    );

    let first = project_templates::list(&connection, &owner, 1, None).expect("first page");
    assert_eq!(first.templates, vec![locked]);
    let second =
        project_templates::list(&connection, &owner, 1, first.next_cursor).expect("second page");
    assert_eq!(second.templates, vec![recreated]);
    assert!(second.next_cursor.is_none());
    let events = sqlite::list_events(&connection, &owner, 0, 100).expect("catalogue events");
    assert_eq!(
        events
            .events
            .iter()
            .filter(|event| event.kind == "project-template.changed")
            .count(),
        5
    );
    fail(
        &mut connection,
        accepted.operation_id,
        "internal_error",
        &OperationId::new().to_string(),
        None,
        31,
    )
    .expect("finish failed create");
    assert!(
        !project_templates::archive_references(&connection)
            .expect("read remaining template archive references")
            .contains(&frozen_digest)
    );
}

#[test]
fn reseeding_replaces_retired_builtins_and_preserves_user_templates() {
    let dir = crate::events_tests::Directory::new();
    let mut connection = sqlite::initialize_connection(&dir.database()).expect("isolated store");
    let owner = PrincipalId::local_owner();
    let old = declaration("com.example.builtin", Vec::new());
    project_templates::seed(&mut connection, &owner, vec![old.clone()]).expect("seed old ID");
    let user = project_templates::save(
        &mut connection,
        &owner,
        declaration("com.example.user", Vec::new()),
        None,
        1,
    )
    .expect("save user template");
    let mut new = old;
    new.id.push_str(".2022");
    project_templates::seed(&mut connection, &owner, vec![new.clone()]).expect("seed yearly ID");
    let page = project_templates::list(&connection, &owner, 100, None).expect("updated catalogue");
    assert_eq!(page.templates.len(), 2);
    assert_eq!(page.templates[0].declaration, new);
    assert!(page.templates[0].locked);
    assert_eq!(page.templates[1], user);
    project_templates::seed(&mut connection, &owner, vec![new]).expect("repeat seeding");
    assert_eq!(
        project_templates::list(&connection, &owner, 100, None).expect("read reseeded catalogue"),
        page
    );
}

#[test]
fn publish_intent_fences_cancel_and_restart_commits_registration_with_task() {
    let dir = crate::events_tests::Directory::new();
    let mut connection = sqlite::initialize_connection(&dir.database()).expect("isolated store");
    let owner = PrincipalId::local_owner();
    let created = accept(&mut connection, acceptance(&owner, input(None)), 10).expect("accept");
    let operation_id = created.operation_id;
    begin_work(&mut connection, operation_id, 11).expect("start");
    assert!(publish_intent(&mut connection, operation_id, 12).expect("publish allowed"));
    let revision = sqlite::load_owned_operation(&connection, &owner, operation_id)
        .expect("visible")
        .revision;
    let cancel_key = IdempotencyKey::parse("cancel-fixture").expect("cancel key");
    assert_eq!(
        sqlite::cancel_operation(
            &mut connection,
            &owner,
            operation_id,
            revision,
            &cancel_key,
            13
        )
        .expect_err("publish intent cannot be cancelled")
        .kind(),
        StoreErrorKind::OperationNotCancellable
    );
    drop(connection);
    let mut connection = sqlite::initialize_connection(&dir.database()).expect("reopen");
    assert_eq!(
        pending(&mut connection, 14).expect("recover"),
        [operation_id]
    );
    let recovered = begin_work(&mut connection, operation_id, 15)
        .expect("resume")
        .expect("pending");
    assert!(recovered.publishing);
    assert_eq!(recovered.project_id, created.project_id);

    let observation = ProjectObservation {
        root_path: "/canonical/Project".to_owned(),
        path_identity_key: vec![20],
        project_type: ProjectType::Unknown,
        unity_version: "2022.3.22f1".to_owned(),
        unity_revision: None,
        vpm_manifest: ManifestState::Missing,
        upm_manifest: ManifestState::Valid,
        direct_dependencies: Vec::new(),
        locked_dependencies: Vec::new(),
        issues: Vec::new(),
    };
    connection
        .execute_batch(
            "CREATE TEMP TRIGGER fail_finish BEFORE INSERT ON events
        WHEN NEW.kind='operation.state_changed' BEGIN SELECT RAISE(ABORT,'fixture failure'); END;",
        )
        .expect("inject final commit failure");
    assert!(finish(&mut connection, operation_id, observation.clone(), 16).is_err());
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM projects", [], |row| row
                .get::<_, i64>(0))
            .expect("rolled back"),
        0
    );
    assert_eq!(
        sqlite::load_owned_operation(&connection, &owner, operation_id)
            .expect("still pending")
            .state,
        OperationState::Running
    );
    connection
        .execute_batch("DROP TRIGGER fail_finish")
        .expect("restore store");
    finish(&mut connection, operation_id, observation, 17).expect("complete registration");
    let final_operation =
        sqlite::load_owned_operation(&connection, &owner, operation_id).expect("completed task");
    assert_eq!(final_operation.state, OperationState::Succeeded);
    assert_eq!(
        connection
            .query_row("SELECT project_id FROM projects", [], |row| row
                .get::<_, String>(0))
            .expect("project exists"),
        created.project_id.to_string()
    );
    let events = sqlite::list_events(&connection, &owner, 0, 100).expect("events");
    assert_eq!(
        events.events[events.events.len() - 2].kind,
        "project.registered"
    );
    assert_eq!(
        events
            .events
            .last()
            .expect("terminal event")
            .aggregate_revision,
        final_operation.revision
    );
}

#[test]
fn accepted_cancel_survives_restart_and_wins_against_pre_publish_failure() {
    let dir = crate::events_tests::Directory::new();
    let mut connection = sqlite::initialize_connection(&dir.database()).expect("isolated store");
    let owner = PrincipalId::local_owner();
    let operation = accept(&mut connection, acceptance(&owner, input(None)), 10)
        .expect("accept")
        .operation_id;
    begin_work(&mut connection, operation, 11).expect("start");
    let revision = sqlite::load_owned_operation(&connection, &owner, operation)
        .expect("visible")
        .revision;
    let key = IdempotencyKey::parse("cancel-fixture").expect("cancel key");
    sqlite::cancel_operation(&mut connection, &owner, operation, revision, &key, 12)
        .expect("pre-publish cancel");
    drop(connection);

    let mut connection = sqlite::initialize_connection(&dir.database()).expect("reopen");
    assert_eq!(pending(&mut connection, 13).expect("recover"), [operation]);
    let work = begin_work(&mut connection, operation, 14)
        .expect("start recovery")
        .expect("pending cancellation");
    assert!(work.cancel_requested);
    assert!(!publish_intent(&mut connection, operation, 15).expect("observe cancellation"));
    fail(
        &mut connection,
        operation,
        "internal_error",
        &Uuid::new_v4().to_string(),
        None,
        16,
    )
    .expect("finalize prior cancellation");
    let complete = sqlite::load_owned_operation(&connection, &owner, operation).expect("complete");
    assert_eq!(complete.state, OperationState::Cancelled);
    assert!(complete.error_code.is_none());
}
