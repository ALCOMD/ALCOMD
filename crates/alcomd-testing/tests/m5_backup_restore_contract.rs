use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use alcomd_application::{BackupCancellation, M5BackupAdapter, M5BackupErrorCode, OperationId};
use alcomd_protocol::{BackupRestoreParams, CAPABILITY_BACKUPS_RESTORE_V2, METHOD_BACKUPS_RESTORE};
use alcomd_vpm::BackupEngine;
use serde_json::json;
use zip::write::SimpleFileOptions;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("alcomd-ordinary-zip-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("data")).expect("valid isolated fixture or result");
        Self(fs::canonicalize(root).expect("valid isolated fixture or result"))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("test fixture cleanup: {error}");
        }
    }
}

fn zip_project(path: &Path, prefix: &str, contents: &[u8]) {
    let mut zip =
        zip::ZipWriter::new(File::create(path).expect("valid isolated fixture or result"));
    for (name, bytes) in [
        (
            "ProjectSettings/ProjectVersion.txt",
            b"m_EditorVersion: 2022.3.22f1\n".as_slice(),
        ),
        ("Packages/manifest.json", b"{}".as_slice()),
        ("Assets/文 件.txt", contents),
    ] {
        zip.start_file(
            format!("{prefix}{name}"),
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
        )
        .expect("valid isolated fixture or result");
        zip.write_all(bytes)
            .expect("valid isolated fixture or result");
    }
    zip.finish().expect("valid isolated fixture or result");
}

#[test]
fn restore_wire_contract_uses_the_selected_archive_and_project_destination() {
    assert_eq!(METHOD_BACKUPS_RESTORE, "backups.restore");
    assert_eq!(CAPABILITY_BACKUPS_RESTORE_V2, "backups.restore.v2");
    let request = json!({
        "archivePath": "/other drive/项目.zip",
        "destinationPath": "/work/还 原",
        "idempotencyKey": "restore-once",
    });
    let value: BackupRestoreParams = serde_json::from_value(request.clone()).expect("file request");
    assert_eq!(value.project_name, None);
    assert_eq!(
        serde_json::to_value(value).expect("valid isolated fixture or result"),
        request
    );
}

#[tokio::test]
async fn restore_accepts_an_independent_zip_and_rejects_a_wrapped_or_changed_source() {
    let fixture = Fixture::new();
    let engine = BackupEngine::new(fixture.0.join("data/backups"))
        .expect("valid isolated fixture or result");
    let source = fixture.0.join("外部 文件.ZIP");
    let wrapped = fixture.0.join("Wrapped.zip");
    let target = fixture.0.join("还 原");
    zip_project(&source, "", b"data");
    zip_project(&wrapped, "Folder/", b"data");
    assert_eq!(
        engine
            .prepare_restore(
                wrapped
                    .to_str()
                    .expect("valid isolated fixture or result")
                    .to_owned(),
                target.clone()
            )
            .await
            .expect_err("operation must reject invalid input")
            .code(),
        M5BackupErrorCode::BackupInvalidArchive,
    );
    let input = source
        .to_str()
        .expect("valid isolated fixture or result")
        .to_owned();
    let request = engine
        .prepare_restore(input.clone(), target.clone())
        .await
        .expect("valid isolated fixture or result");

    // Changing bytes on the same path and at the same length also invalidates the accepted task.
    zip_project(&source, "", b"edit");
    let operation = OperationId::new();
    assert_eq!(
        engine
            .stage_restore(operation, request, BackupCancellation::default())
            .await
            .expect_err("operation must reject invalid input")
            .code(),
        M5BackupErrorCode::BackupSourceChanged,
    );
    assert!(!target.exists());

    let request = engine
        .prepare_restore(input, target.clone())
        .await
        .expect("valid isolated fixture or result");
    let operation = OperationId::new();
    let staged = engine
        .stage_restore(operation, request.clone(), BackupCancellation::default())
        .await
        .expect("valid isolated fixture or result");
    fs::create_dir(&target).expect("valid isolated fixture or result");
    fs::write(target.join("retain me"), b"existing").expect("valid isolated fixture or result");
    assert_eq!(
        engine
            .publish_restore(staged)
            .await
            .expect_err("operation must reject invalid input")
            .code(),
        M5BackupErrorCode::BackupRestoreTargetExists
    );
    assert_eq!(
        fs::read(target.join("retain me")).expect("valid isolated fixture or result"),
        b"existing"
    );
    engine
        .discard_staged_restore(operation, request)
        .await
        .expect("valid isolated fixture or result");
    assert!(
        !fixture
            .0
            .join(format!(".alcomd-restore-{operation}"))
            .exists()
    );
}

#[tokio::test]
async fn restore_reads_payload_after_acceptance_and_never_publishes_corrupt_contents() {
    let fixture = Fixture::new();
    let engine = BackupEngine::new(fixture.0.join("data/backups")).expect("isolated engine");
    let source = fixture.0.join("Malformed.zip");
    let target = fixture.0.join("Not published");
    let payload = b"entry payload unique within this archive";
    zip_project(&source, "", payload);
    let mut bytes = fs::read(&source).expect("fixture ZIP");
    let offset = bytes
        .windows(payload.len())
        .position(|window| window == payload)
        .expect("fixture ZIP stores uncompressed bytes");
    bytes[offset] ^= 1;
    fs::write(&source, bytes).expect("corrupt fixture payload with intact directory");
    let request = engine
        .prepare_restore(
            source.to_str().expect("source path").to_owned(),
            target.clone(),
        )
        .await
        .expect("bounded admission only reads authority and ZIP structure");
    let operation = OperationId::new();
    let failure = engine
        .stage_restore(operation, request, BackupCancellation::default())
        .await
        .expect_err("task must reject damaged file bytes");
    assert_eq!(failure.code(), M5BackupErrorCode::BackupIntegrityMismatch);
    assert!(!target.exists());
    assert!(
        !fixture
            .0
            .join(format!(".alcomd-restore-{operation}"))
            .exists()
    );
}

#[tokio::test]
async fn stale_restore_stage_cannot_remove_an_unowned_directory() {
    let fixture = Fixture::new();
    let engine = BackupEngine::new(fixture.0.join("data/backups")).expect("isolated engine");
    let source = fixture.0.join("Approved.zip");
    let target = fixture.0.join("Absent");
    zip_project(&source, "", b"intact");
    let request = engine
        .prepare_restore(source.to_str().expect("source path").to_owned(), target)
        .await
        .expect("valid project archive");
    let operation = OperationId::new();
    let staged = engine
        .stage_restore(operation, request.clone(), BackupCancellation::default())
        .await
        .expect("owned stage ready");
    fs::rename(&staged.staging_root, fixture.0.join("Previous stage"))
        .expect("replace test staging directory");
    fs::create_dir(&staged.staging_root).expect("unowned replacement");
    let unrelated = staged.staging_root.join("Unrelated data");
    fs::write(&unrelated, b"retain user data").expect("unrelated fixture file");
    assert_eq!(
        engine
            .discard_staged_restore(operation, request)
            .await
            .expect_err("reject replaced staging object")
            .code(),
        M5BackupErrorCode::BackupRestoreRecoveryRequired,
    );
    assert_eq!(
        fs::read(unrelated).expect("unowned data preserved"),
        b"retain user data"
    );
}
