use alcomd_protocol::{
    BackupCompression, BackupCreateParams, CAPABILITY_BACKUPS_CREATE_V2, METHOD_BACKUPS_CREATE,
    OperationAccepted,
};
use alcomd_vpm::ArchiveLimits;
use serde_json::json;

#[test]
fn backup_create_wire_contract_has_optional_absolute_destination_and_operation_receipt() {
    assert_eq!(METHOD_BACKUPS_CREATE, "backups.create");
    assert_eq!(CAPABILITY_BACKUPS_CREATE_V2, "backups.create.v2");
    let request = json!({
        "projectId": "abc",
        "expectedRevision": 1,
        "compressionMode": "level7",
        "excludeVpmPackages": true,
        "includeLibrary": true,
        "destinationPath": "/external folder/项目.zip",
        "idempotencyKey": "retry-backup",
    });
    let params: BackupCreateParams = serde_json::from_value(request.clone()).expect("wire request");
    assert_eq!(params.compression_mode, Some(BackupCompression::Level7));
    assert_eq!(
        serde_json::to_value(params).expect("valid isolated fixture or result"),
        request
    );
    let mut default = request;
    default
        .as_object_mut()
        .expect("valid isolated fixture or result")
        .remove("destinationPath");
    let params: BackupCreateParams =
        serde_json::from_value(default.clone()).expect("default destination");
    assert_eq!(params.destination_path, None);
    assert_eq!(
        serde_json::to_value(params).expect("valid isolated fixture or result"),
        default
    );
    assert_eq!(
        serde_json::to_value(OperationAccepted {
            operation_id: "operation".to_owned(),
            replayed: true
        })
        .expect("valid isolated fixture or result"),
        json!({"operationId": "operation", "replayed": true}),
    );
}

#[test]
fn project_zip_keeps_approved_archive_quotas() {
    let limits = ArchiveLimits::backup();
    assert_eq!(limits.archive_bytes, 68_719_476_736);
    assert_eq!(limits.entries, 500_000);
    assert_eq!(limits.entry_bytes, 34_359_738_368);
    assert_eq!(limits.total_uncompressed_bytes, 137_438_953_472);
    assert_eq!(limits.path_depth, 128);
    assert_eq!(limits.normalized_path_bytes, 1_024);
    assert_eq!(limits.expansion_ratio, 10_000);
}
