use super::*;

struct Fixture(PathBuf);

impl Fixture {
    fn create() -> Self {
        let root = std::env::temp_dir().join(format!("alcomd-migration-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).expect("isolated test fixture");
        Self(fs::canonicalize(root).expect("canonical fixture root"))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("test migration cleanup: {error}");
        }
    }
}

fn test_steps(fixture: &Fixture) -> Steps {
    let source = fixture.0.join("source");
    let destination = fixture.0.join("v4");
    let recovery = fixture.0.join("recovery-fixture");
    let workspace = fixture.0.join("workspace");
    fs::create_dir(&source).expect("synthetic v3 directory");
    fs::write(source.join("previous"), b"unchanged input").expect("synthetic source");
    fs::write(&recovery, b"unchanged executable").expect("synthetic recovery program");
    let source_identity = alcomd_platform::resolve_plain_storage_directory(&source)
        .expect("fixture source")
        .1;
    let recovery_file = file_evidence(&recovery, EXE_LIMIT).expect("fixture recovery identity");
    let destination_parent_identity = isolated_child(&destination)
        .expect("fixture target parent")
        .1;
    let plan = Plan {
        workspace: workspace.clone(),
        source_root: source,
        recovery_executable: recovery,
        source_identity,
        recovery_file,
        destination,
        destination_parent_identity,
        payload_root: fixture.0.join("payload"),
        payload_identity: vec![7, 1],
        payload_manifest_sha256: [4; 32],
        target_version: "4.0.0".to_owned(),
        channel: Channel::Stable,
        install_scope: InstallScope::WindowsCurrentUser,
        credential_transfer_id: None,
        discord_consent_digest: None,
    };
    let journal = Journal::open(&workspace).expect("journal open");
    journal.record(PLAN_RECORD, &plan).expect("journal plan");
    Steps {
        journal,
        plan,
        migration_reader: PathBuf::new(),
        daemon: PathBuf::new(),
    }
}

fn stub_state(stage: &Path) -> StateEvidence {
    fs::create_dir_all(stage.join("config")).expect("native data structure");
    fs::write(stage.join("state.db"), b"test bytes").expect("test state");
    fs::write(stage.join("config/settings.toml"), b"test config").expect("test settings");
    state_evidence(stage).expect("evidence for publication mechanics")
}

fn stub_export() -> ExportEvidence {
    ExportEvidence {
        manifest_sha256: [20; 32],
        records: BTreeMap::from([("projects".to_owned(), 1)]),
    }
}

#[test]
fn journal_detects_concurrent_writes_and_missing_steps() {
    let fixture = Fixture::create();
    let steps = test_steps(&fixture);
    assert!(Journal::open(&steps.plan.workspace).is_err());
    assert!(
        steps.journal.record(PLAN_RECORD, &steps.plan).is_err(),
        "plan is immutable"
    );
    steps
        .journal
        .record(&phase_file(2), &Phase::ExportValidated(stub_export()))
        .expect("inject interrupted journal hole");
    assert!(
        latest_phase(&steps.journal).is_err(),
        "must not skip a persistent phase"
    );
}

#[test]
fn pre_commit_rollback_removes_only_owned_stages_and_preserves_old_state() {
    let fixture = Fixture::create();
    let steps = test_steps(&fixture);
    let export = steps.reset_stage(EXPORT_DIR).expect("owned export stage");
    let native = steps.reset_stage(STATE_DIR).expect("owned native stage");
    fs::write(export.join("transient"), b"disposable export").expect("fake failed export");
    stub_state(&native.join(DATA_DIR));
    steps
        .journal
        .record(&phase_file(1), &Phase::Frozen)
        .expect("record freeze");
    rollback(&steps.journal, &steps.plan).expect("rollback owned outputs");
    assert!(!steps.plan.destination.exists());
    assert!(!export.exists() && !native.exists());
    assert_eq!(
        fs::read(steps.plan.source_root.join("previous")).expect("original v3 input"),
        b"unchanged input"
    );
    assert_eq!(
        fs::read(&steps.plan.recovery_executable).expect("original recovery"),
        b"unchanged executable"
    );
    assert!(matches!(
        latest_phase(&steps.journal).expect("journal"),
        Some(Phase::RolledBack)
    ));
}

#[test]
fn healthy_publication_recovers_after_interrupt_between_rename_and_receipt() {
    let fixture = Fixture::create();
    let steps = test_steps(&fixture);
    let stage = steps
        .reset_stage(STATE_DIR)
        .expect("owned stage")
        .join(DATA_DIR);
    let evidence = stub_state(&stage);
    for phase in [
        Phase::Frozen,
        Phase::ExportValidated(stub_export()),
        Phase::NativeStaged,
        Phase::NativeChecked(evidence.clone()),
        Phase::Healthy(evidence.clone()),
        Phase::CommitIntent(evidence.clone()),
    ] {
        steps
            .journal
            .record(&phase_file(phase.index()), &phase)
            .expect("persist phase");
    }
    alcomd_platform::publish_path_no_replace(&stage, &steps.plan.destination)
        .expect("simulate interruption after native publication");
    let workspace = steps.plan.workspace.clone();
    let mut steps = steps;
    drop(steps.journal);
    assert!(
        rollback_v3(&workspace)
            .expect_err("commit intent forbids rollback")
            .to_string()
            .contains("commit point"),
    );
    steps.journal = Journal::open(&workspace).expect("resume interrupted journal");
    let mut phase = latest_phase(&steps.journal).expect("read durable intent");
    assert_eq!(
        steps
            .run(&mut phase, None)
            .expect("recover committed native publication"),
        MigrationOutcome::Committed
    );
    assert!(matches!(
        latest_phase(&steps.journal).expect("journal"),
        Some(Phase::Committed)
    ));
    assert!(steps.plan.source_root.join("previous").is_file());
    assert_eq!(
        state_evidence(&steps.plan.destination).expect("committed evidence"),
        evidence
    );
    assert!(!steps.journal.root().join(STATE_DIR).exists());
}

#[test]
fn substituted_target_and_missing_gui_health_block_migration() {
    let fixture = Fixture::create();
    let steps = test_steps(&fixture);
    let stage = steps
        .reset_stage(STATE_DIR)
        .expect("owned stage")
        .join(DATA_DIR);
    let evidence = stub_state(&stage);
    stub_state(&steps.plan.destination);
    assert!(
        steps.commit(&evidence).is_err(),
        "one file content match cannot authorize adoption"
    );
    assert!(steps.plan.destination.join("state.db").is_file());
    assert!(stage.join("state.db").is_file());
    let export = stub_export();
    let expected = HealthChallenge {
        schema: 1,
        product_version: "4.0.0".to_owned(),
        install_scope: InstallScope::WindowsCurrentUser,
        payload_manifest_sha256: [7; 32],
        export_manifest_sha256: export.manifest_sha256,
        native_content_sha256: evidence.content_sha256,
        records: export.records.clone(),
    };
    let mut health = HealthProof {
        schema: 1,
        product_version: "4.0.0".to_owned(),
        install_scope: InstallScope::WindowsCurrentUser,
        payload_manifest_sha256: expected.payload_manifest_sha256,
        test_evidence_sha256: [10; 32],
        core_rpc: true,
        cli: true,
        gui: false,
        mcp: true,
        first_party_extensions: true,
    };
    assert!(!health.matches(&expected));
    health.gui = true;
    assert!(health.matches(&expected));
    health.payload_manifest_sha256[0] ^= 1;
    assert!(!health.matches(&expected));
}

#[test]
fn credential_cleanup_rejects_references_from_a_different_namespace() {
    let fixture = Fixture::create();
    let transfer_id = Uuid::new_v4();
    let reference = migration_credential_reference(transfer_id, 0);
    let mut state = serde_json::json!({
        "schema": 1,
        "sourceVersion": "3.4.0",
        "projects": [],
        "unityInstallations": [],
        "repositories": [{
            "source": {"kind": "remote", "url": "https://example.invalid/repository.json"},
            "hidden": false,
            "hasCredentials": true,
            "credentialRef": reference,
        }],
        "localPackages": [],
        "templates": [],
        "settings": {
            "locale": "en", "appearanceMode": "system", "appearanceMotion": "system",
            "fullscreen": false, "handleVccLinks": false, "showPrerelease": false,
            "hideLocalUserPackages": false, "hideOfficialRepository": false,
            "hideCuratedRepository": false, "backupCompression": "fast", "favoriteTemplateIds": [],
        },
        "extensions": [],
        "discord": {
            "enabled": false, "projectName": false, "unityVersion": false,
            "editorCount": false, "customText": "",
        },
    });
    fs::write(fixture.0.join("state.json"), state.to_string()).expect("isolated reference fixture");
    assert_eq!(
        staged_credential_references(&fixture.0, transfer_id).expect("owned reference"),
        vec![reference],
    );
    let unrelated = migration_credential_reference(Uuid::new_v4(), 0);
    state["repositories"][0]["credentialRef"] = serde_json::json!(unrelated);
    fs::write(fixture.0.join("state.json"), state.to_string())
        .expect("substituted reference fixture");
    assert!(staged_credential_references(&fixture.0, transfer_id).is_err());
}
