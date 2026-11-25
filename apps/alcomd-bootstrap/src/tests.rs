use super::*;

pub(super) struct Fixture(pub(super) PathBuf);

impl Fixture {
    pub(super) fn create() -> Self {
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

fn test_steps(root: &Path) -> Steps {
    let source = root.join("source");
    let destination = root.join("v4");
    let recovery = root.join("recovery-fixture");
    let workspace = root.join("workspace");
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
        payload_root: root.join("payload"),
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

fn checkpoints(evidence: &StateEvidence) -> [Phase; 7] {
    [
        Phase::Frozen,
        Phase::ExportValidated(stub_export()),
        Phase::NativeStaged,
        Phase::NativeChecked(evidence.clone()),
        Phase::Healthy(evidence.clone()),
        Phase::CommitIntent(evidence.clone()),
        Phase::Committed,
    ]
}

#[test]
fn journal_detects_concurrent_writes_and_missing_steps() {
    let fixture = Fixture::create();
    let steps = test_steps(&fixture.0);
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
fn partial_writes_preserve_every_checkpoint_until_retry() {
    for kind in [io::ErrorKind::StorageFull, io::ErrorKind::PermissionDenied] {
        let fixture = Fixture::create();
        let steps = test_steps(&fixture.0);
        let evidence = stub_state(&fixture.0.join("test-state"));
        let mut journal = steps.journal;
        let mut previous = None;
        for next in checkpoints(&evidence) {
            let name = phase_file(next.index());
            let failure = journal
                .record_with(&name, &next, |file, bytes| {
                    std::io::Write::write_all(file, &bytes[..bytes.len() / 2])?;
                    Err(kind.into())
                })
                .expect_err("partial write must fail without publishing the record");
            assert_eq!(
                failure.downcast_ref::<io::Error>().map(io::Error::kind),
                Some(kind)
            );
            let partial = journal.root().join(format!("{name}.partial"));
            assert!(partial.metadata().expect("partial write survived").len() > 0);
            assert!(!journal.root().join(&name).exists());

            drop(journal);
            journal = Journal::open(&steps.plan.workspace).expect("restart after incomplete write");
            assert_eq!(latest_phase(&journal).expect("restart journal"), previous);
            journal
                .record(&name, &next)
                .expect("retry after restoring writes");
            assert!(!partial.exists());
            previous = Some(next);
            assert_eq!(latest_phase(&journal).expect("retry checkpoint"), previous);
        }
        assert_eq!(
            fs::read(fixture.0.join("source/previous")).expect("v3 source"),
            b"unchanged input"
        );
        assert_eq!(
            file_evidence(&steps.plan.recovery_executable, EXE_LIMIT).expect("v3 executable"),
            steps.plan.recovery_file
        );
    }
}

#[cfg(windows)]
#[test]
fn denied_partial_record_preserves_the_checkpoint_until_permissions_are_restored() {
    let fixture = Fixture::create();
    let steps = test_steps(&fixture.0);
    steps
        .journal
        .record(&phase_file(Phase::Frozen.index()), &Phase::Frozen)
        .expect("persist first checkpoint");
    let next = Phase::ExportValidated(stub_export());
    let name = phase_file(next.index());
    let partial = steps.journal.root().join(format!("{name}.partial"));
    fs::write(&partial, b"interrupted checkpoint bytes").expect("create owned partial record");
    fn change_acl(path: &Path, operation: &str, entry: &str) {
        let output = std::process::Command::new("icacls.exe")
            .arg(path)
            .args([operation, entry])
            .output()
            .expect("apply ACL to sandboxed fixture file");
        assert!(output.status.success(), "icacls {operation} failed");
    }
    change_acl(&partial, "/deny", "*S-1-1-0:(RD)");
    let result = steps.journal.record(&name, &next);
    change_acl(&partial, "/remove:d", "*S-1-1-0");
    let error = result.expect_err("Windows must reject reading the denied record");
    assert_eq!(
        error.downcast_ref::<io::Error>().map(io::Error::kind),
        Some(io::ErrorKind::PermissionDenied)
    );
    assert_eq!(
        fs::read(&partial).expect("previous partial bytes"),
        b"interrupted checkpoint bytes"
    );
    assert!(matches!(
        latest_phase(&steps.journal).expect("unchanged journal"),
        Some(Phase::Frozen)
    ));
    steps
        .journal
        .record(&name, &next)
        .expect("retry after restoring permissions");
    assert!(!partial.exists());
}

#[test]
fn pre_commit_rollback_removes_only_owned_stages_and_preserves_old_state() {
    let fixture = Fixture::create();
    let steps = test_steps(&fixture.0);
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
    let steps = test_steps(&fixture.0);
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
fn process_termination_at_each_durable_phase_preserves_the_recovery_source() {
    for phase_index in 1..=Phase::Committed.index() {
        let fixture = Fixture::create();
        let mut child = CheckpointProcess(
            std::process::Command::new(std::env::current_exe().expect("test executable"))
                .args([
                    "--ignored",
                    "--exact",
                    "tests::persist_checkpoint_until_terminated",
                ])
                .env("ALCOMD_BOOTSTRAP_TEST_ROOT", &fixture.0)
                .env("ALCOMD_BOOTSTRAP_TEST_PHASE", phase_index.to_string())
                .spawn()
                .expect("start isolated checkpoint process"),
        );
        let workspace = fixture.0.join("workspace");
        let ready = fixture.0.join("checkpoint-ready");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while !ready.is_file() {
            assert!(
                child
                    .0
                    .try_wait()
                    .expect("checkpoint process status")
                    .is_none()
            );
            assert!(
                std::time::Instant::now() < deadline,
                "phase {phase_index} not reached"
            );
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        child.0.kill().expect("terminate checkpoint process");
        assert!(
            !child
                .0
                .wait()
                .expect("terminated checkpoint process")
                .success()
        );

        let journal = Journal::open(&workspace).expect("restart after process termination");
        let mut phase = latest_phase(&journal).expect("checkpoint persisted before termination");
        assert_eq!(phase.as_ref().map(Phase::index), Some(phase_index));
        let plan: Plan = journal
            .read(PLAN_RECORD)
            .expect("reopen plan")
            .expect("persisted plan");
        if !matches!(phase, Some(Phase::CommitIntent(_) | Phase::Committed)) {
            drop(journal);
            rollback_v3(&workspace).expect("roll back from persisted pre-commit phase");
            rollback_v3(&workspace).expect("rollback repeat is a no-op");
            assert!(!plan.destination.exists());
        } else {
            drop(journal);
            assert!(
                rollback_v3(&workspace).is_err(),
                "recover forward at the commit point"
            );
            let mut steps = Steps {
                journal: Journal::open(&workspace).expect("restart after commit intent"),
                plan: plan.clone(),
                migration_reader: PathBuf::new(),
                daemon: PathBuf::new(),
            };
            assert_eq!(
                steps.run(&mut phase, None).expect("recover forward"),
                MigrationOutcome::Committed
            );
            assert!(matches!(
                latest_phase(&steps.journal).expect("read final checkpoint"),
                Some(Phase::Committed)
            ));
            assert_eq!(
                fs::read(plan.destination.join("state.db")).expect("published native state"),
                b"test bytes"
            );
        }
        assert!(!workspace.join(EXPORT_DIR).exists() && !workspace.join(STATE_DIR).exists());
        assert_eq!(
            fs::read(plan.source_root.join("previous")).expect("read original data"),
            b"unchanged input"
        );
        assert_eq!(
            file_evidence(&plan.recovery_executable, EXE_LIMIT).expect("read recovery executable"),
            plan.recovery_file
        );
    }
}

struct CheckpointProcess(std::process::Child);

impl Drop for CheckpointProcess {
    fn drop(&mut self) {
        if matches!(self.0.try_wait(), Ok(None)) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}

#[test]
#[ignore = "invoked by the process termination test with a private fixture"]
fn persist_checkpoint_until_terminated() {
    let root = std::env::var_os("ALCOMD_BOOTSTRAP_TEST_ROOT").expect("private fixture root");
    let root = PathBuf::from(root);
    let phase_index: u8 = std::env::var("ALCOMD_BOOTSTRAP_TEST_PHASE")
        .expect("checkpoint index")
        .parse()
        .expect("numeric checkpoint index");
    let steps = test_steps(&root);
    let export = steps.reset_stage(EXPORT_DIR).expect("owned export staging");
    fs::write(export.join("test-export"), b"disposable export").expect("test export");
    let stage = steps
        .reset_stage(STATE_DIR)
        .expect("owned native staging")
        .join(DATA_DIR);
    let evidence = stub_state(&stage);
    for phase in checkpoints(&evidence) {
        let index = phase.index();
        if matches!(phase, Phase::Committed) {
            assert!(matches!(
                steps.commit(&evidence).expect("publish native staging"),
                Phase::Committed
            ));
        }
        steps
            .journal
            .record(&phase_file(index), &phase)
            .expect("persist checkpoint");
        if index == phase_index {
            fs::write(root.join("checkpoint-ready"), b"").expect("notify parent to terminate");
            loop {
                std::thread::park();
            }
        }
    }
    panic!("requested checkpoint was not reached");
}

#[test]
fn substituted_target_and_missing_gui_health_block_migration() {
    let fixture = Fixture::create();
    let steps = test_steps(&fixture.0);
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
