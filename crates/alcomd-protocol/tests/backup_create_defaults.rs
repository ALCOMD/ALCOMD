use alcomd_protocol::{BackupCompression, BackupCreateParams};
use serde_json::json;

#[test]
fn omitted_backup_choices_inherit_but_explicit_null_and_invalid_values_are_rejected() {
    let base = json!({
        "projectId": "00000000-0000-4000-8000-000000000001",
        "expectedRevision": 1,
        "idempotencyKey": "backup-defaults"
    });
    let inherited: BackupCreateParams =
        serde_json::from_value(base.clone()).expect("inherit defaults");
    assert_eq!(inherited.compression_mode, None);
    assert_eq!(inherited.exclude_vpm_packages, None);
    assert_eq!(inherited.include_library, None);
    assert_eq!(
        serde_json::to_value(inherited).expect("serialize omitted options"),
        base
    );
    for (field, invalid) in [
        ("compressionMode", json!(null)),
        ("compressionMode", json!("balanced")),
        ("excludeVpmPackages", json!(null)),
        ("excludeVpmPackages", json!("false")),
        ("includeLibrary", json!(null)),
        ("includeLibrary", json!(0)),
        ("unknown", json!(false)),
    ] {
        let mut value = base.clone();
        value[field] = invalid;
        assert!(
            serde_json::from_value::<BackupCreateParams>(value).is_err(),
            "{field}"
        );
    }
    let mut explicit = base;
    explicit["compressionMode"] = json!("maximum");
    explicit["excludeVpmPackages"] = json!(false);
    explicit["includeLibrary"] = json!(true);
    let selected: BackupCreateParams = serde_json::from_value(explicit).expect("explicit choices");
    assert_eq!(selected.compression_mode, Some(BackupCompression::Maximum));
    assert_eq!(selected.exclude_vpm_packages, Some(false));
    assert_eq!(selected.include_library, Some(true));
}

#[test]
fn backup_compression_accepts_exactly_the_six_approved_wire_choices() {
    for (wire, expected) in [
        ("store", BackupCompression::Store),
        ("fast", BackupCompression::Fast),
        ("level3", BackupCompression::Level3),
        ("level5", BackupCompression::Level5),
        ("level7", BackupCompression::Level7),
        ("maximum", BackupCompression::Maximum),
    ] {
        let value = json!({
            "projectId": "00000000-0000-4000-8000-000000000001",
            "expectedRevision": 1,
            "compressionMode": wire,
            "idempotencyKey": "six-compression-choices"
        });
        let request: BackupCreateParams = serde_json::from_value(value.clone()).expect(wire);
        assert_eq!(request.compression_mode, Some(expected));
        assert_eq!(
            serde_json::to_value(request).expect("wire roundtrip"),
            value
        );
    }
    for invalid in [
        json!("level2"),
        json!("level4"),
        json!("level6"),
        json!("level8"),
        json!("level9"),
        json!("level_3"),
        json!("level-3"),
        json!("3"),
        json!(3),
        json!(false),
        json!(null),
    ] {
        assert!(
            serde_json::from_value::<BackupCompression>(invalid.clone()).is_err(),
            "{invalid}"
        );
    }
}
