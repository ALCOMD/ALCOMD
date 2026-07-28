use crate::common::*;
use vrc_get_vpm::unity_project::pending_project_changes::RemoveReason;
use vrc_get_vpm::version::Version;

mod common;

#[test]
fn basic_remove() {
    block_on(async {
        let project = VirtualProjectBuilder::new()
            .add_dependency("com.anatawa12.gists", Version::new(1, 0, 0))
            .add_locked("com.anatawa12.gists", Version::new(1, 0, 0), &[])
            .build()
            .await
            .unwrap();

        let result = project
            .remove_request(&["com.anatawa12.gists"])
            .await
            .unwrap();

        assert_eq!(result.package_changes().len(), 1);
        assert_eq!(result.remove_legacy_folders().len(), 0);
        assert_eq!(result.remove_legacy_files().len(), 0);
        assert_eq!(result.conflicts().len(), 0);

        assert!(!result.affects_vrchat_project_type());
        assert_removed(&result, "com.anatawa12.gists", RemoveReason::Requested);
    })
}

#[test]
fn transitive_unused_remove() {
    block_on(async {
        let project = VirtualProjectBuilder::new()
            .add_dependency("com.vrchat.avatars", Version::new(1, 0, 0))
            .add_locked(
                "com.vrchat.avatars",
                Version::new(1, 0, 0),
                &[("com.vrchat.base", "1.0.0")],
            )
            .add_locked("com.vrchat.base", Version::new(1, 0, 0), &[])
            .build()
            .await
            .unwrap();

        let result = project
            .remove_request(&["com.vrchat.avatars"])
            .await
            .unwrap();

        assert_eq!(result.package_changes().len(), 2);
        assert_eq!(result.remove_legacy_folders().len(), 0);
        assert_eq!(result.remove_legacy_files().len(), 0);
        assert_eq!(result.conflicts().len(), 0);

        assert!(result.affects_vrchat_project_type());
        assert_removed(&result, "com.vrchat.avatars", RemoveReason::Requested);
        assert_removed(&result, "com.vrchat.base", RemoveReason::Unused);
    })
}

#[test]
fn worlds_remove_affects_project_type() {
    block_on(async {
        let project = VirtualProjectBuilder::new()
            .add_dependency("com.vrchat.worlds", Version::new(1, 0, 0))
            .add_locked("com.vrchat.worlds", Version::new(1, 0, 0), &[])
            .build()
            .await
            .unwrap();

        let result = project
            .remove_request(&["com.vrchat.worlds"])
            .await
            .unwrap();

        assert!(result.affects_vrchat_project_type());
        assert_removed(&result, "com.vrchat.worlds", RemoveReason::Requested);
    })
}

#[test]
fn do_not_remove_transitively_when_untouched() {
    block_on(async {
        let project = VirtualProjectBuilder::new()
            .add_dependency("com.vrchat.avatars", Version::new(1, 0, 0))
            .add_locked(
                "com.vrchat.avatars",
                Version::new(1, 0, 0),
                &[("com.vrchat.base", "1.0.0")],
            )
            .add_locked("com.vrchat.base", Version::new(1, 0, 0), &[])
            .add_locked(
                "com.anatawa12.untouched_library",
                Version::new(1, 0, 0),
                &[],
            )
            .build()
            .await
            .unwrap();

        let result = project
            .remove_request(&["com.vrchat.avatars"])
            .await
            .unwrap();

        assert_eq!(result.package_changes().len(), 2);
        assert_eq!(result.remove_legacy_folders().len(), 0);
        assert_eq!(result.remove_legacy_files().len(), 0);
        assert_eq!(result.conflicts().len(), 0);

        assert_removed(&result, "com.vrchat.avatars", RemoveReason::Requested);
        assert_removed(&result, "com.vrchat.base", RemoveReason::Unused);
    })
}

#[test]
fn remove_referenced_legacy_package() {
    block_on(async {
        let project = VirtualProjectBuilder::new()
            .add_dependency("com.anatawa12.user", Version::new(1, 0, 0))
            .add_dependency("com.anatawa12.legacy-package", Version::new(1, 0, 0))
            .add_locked(
                "com.anatawa12.user",
                Version::new(1, 0, 0),
                &[("com.anatawa12.legacy-package", "^1.0.0")],
            )
            .add_locked("com.anatawa12.legacy-package", Version::new(1, 0, 0), &[])
            .add_locked("com.anatawa12.package", Version::new(1, 1, 0), &[])
            .add_package_json(
                "com.anatawa12.package",
                r#"
            {
                "name": "com.anatawa12.package",
                "version": "1.1.0",
                "legacyPackages": ["com.anatawa12.legacy-package"]
            }
            "#,
            )
            .build()
            .await
            .unwrap();

        let result = project
            .remove_request(&["com.anatawa12.legacy-package"])
            .await
            .unwrap();

        assert_eq!(result.package_changes().len(), 1);
        assert_eq!(result.remove_legacy_folders().len(), 0);
        assert_eq!(result.remove_legacy_files().len(), 0);
        assert_eq!(result.conflicts().len(), 0);

        assert_removed(
            &result,
            "com.anatawa12.legacy-package",
            RemoveReason::Requested,
        );
    })
}
