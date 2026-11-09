use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum SelectionKind {
    Directory,
    File,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct PickerLocation {
    pub directory: Option<PathBuf>,
    pub file_name: Option<String>,
}

/// A hint only: never creates a directory or changes the submitted path.
/// Relative input must not accidentally open the application's working directory.
pub(super) fn resolve_location(
    current_path: Option<&str>,
    home: Option<&Path>,
    kind: SelectionKind,
) -> PickerLocation {
    let current = current_path
        .map(Path::new)
        .filter(|path| path.is_absolute());
    let file_hint = current.filter(|path| {
        kind == SelectionKind::File
            && (!path.is_dir()
                || cfg!(target_os = "macos")
                    && path.extension().is_some_and(|extension| extension == "app"))
    });
    let file_name = file_hint
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .map(str::to_owned);
    let directory = current
        .and_then(|path| {
            if file_hint.is_some() {
                path.parent()
            } else {
                Some(path)
            }
        })
        .and_then(|path| path.ancestors().find(|ancestor| ancestor.is_dir()))
        .or_else(|| home.filter(|path| path.is_absolute() && path.is_dir()))
        .map(Path::to_path_buf);
    PickerLocation {
        directory,
        file_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("alcomd-picker-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&path).expect("owned fixture");
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let root = std::fs::canonicalize(std::env::temp_dir()).expect("owned temp root");
            let path = std::fs::canonicalize(&self.0).expect("owned fixture root");
            assert_eq!(path.parent(), Some(root.as_path()));
            assert!(
                path.file_name()
                    .expect("fixture name")
                    .to_string_lossy()
                    .starts_with("alcomd-picker-")
            );
            let _ = std::fs::remove_dir_all(path);
        }
    }

    #[test]
    fn existing_directory_takes_priority_and_does_not_become_a_file_name() {
        let fixture = Fixture::new();
        for kind in [SelectionKind::Directory, SelectionKind::File] {
            let location = resolve_location(fixture.0.to_str(), None, kind);
            assert_eq!(location.directory.as_deref(), Some(fixture.0.as_path()));
            assert_eq!(location.file_name, None);
        }
    }

    #[test]
    fn current_file_opens_its_parent_and_only_file_selection_prefills_the_name() {
        let fixture = Fixture::new();
        let file = fixture.0.join("模板 file.alcomdtemplate");
        std::fs::write(&file, b"owned fixture").expect("file");
        let location = resolve_location(file.to_str(), None, SelectionKind::File);
        assert_eq!(location.directory.as_deref(), Some(fixture.0.as_path()));
        assert_eq!(
            location.file_name.as_deref(),
            Some("模板 file.alcomdtemplate")
        );
        let folder = resolve_location(file.to_str(), None, SelectionKind::Directory);
        assert_eq!(folder.directory, location.directory);
        assert_eq!(folder.file_name, None);
    }

    #[test]
    fn missing_targets_use_the_nearest_existing_directory_without_creating_anything() {
        let fixture = Fixture::new();
        let parent = fixture.0.join("现有目录");
        std::fs::create_dir(&parent).expect("existing parent");
        let target = parent.join("not-created").join("nested").join("new.txt");
        let location = resolve_location(target.to_str(), None, SelectionKind::File);
        assert_eq!(location.directory, Some(parent.clone()));
        assert_eq!(location.file_name.as_deref(), Some("new.txt"));
        let folder = resolve_location(target.to_str(), None, SelectionKind::Directory);
        assert_eq!(folder.directory, Some(parent.clone()));
        assert_eq!(folder.file_name, None);
        assert!(!parent.join("not-created").exists());
    }

    #[test]
    fn empty_relative_and_url_hints_use_home_without_reading_the_working_directory() {
        let home = Fixture::new();
        for path in [
            None,
            Some(""),
            Some("relative/file.txt"),
            Some("https://example.invalid/index.json"),
        ] {
            let location = resolve_location(path, Some(&home.0), SelectionKind::File);
            assert_eq!(location.directory.as_deref(), Some(home.0.as_path()));
            assert_eq!(location.file_name, None);
        }
    }

    #[test]
    fn unavailable_home_leaves_the_system_default_instead_of_inventing_a_path() {
        let location = resolve_location(None, None, SelectionKind::Directory);
        assert_eq!(location.directory, None);
        assert_eq!(location.file_name, None);
    }
}
