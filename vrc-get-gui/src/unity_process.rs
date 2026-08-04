use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use sysinfo::{Process, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

pub(crate) fn find_unity_process_ids_for_project(project_path: &Path) -> Vec<u32> {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::OnlyIfNotSet)
            .with_exe(UpdateKind::OnlyIfNotSet)
            .without_tasks(),
    );

    system
        .processes()
        .values()
        .filter_map(|process| {
            if !is_unity_process(process) {
                return None;
            }

            let process_project_path = project_path_from_command(process.cmd())?;
            paths_match(&process_project_path, project_path).then(|| process.pid().as_u32())
        })
        .collect()
}

fn is_unity_process(process: &Process) -> bool {
    process
        .exe()
        .and_then(Path::file_stem)
        .is_some_and(is_unity_name)
        || Path::new(process.name())
            .file_stem()
            .is_some_and(is_unity_name)
        || process
            .cmd()
            .first()
            .and_then(|path| Path::new(path).file_stem())
            .is_some_and(is_unity_name)
}

fn is_unity_name(name: &OsStr) -> bool {
    name.to_string_lossy().eq_ignore_ascii_case("Unity")
}

fn project_path_from_command(command: &[OsString]) -> Option<PathBuf> {
    command.iter().enumerate().find_map(|(index, argument)| {
        let argument = argument.to_string_lossy();

        if argument.eq_ignore_ascii_case("-projectPath") {
            return command.get(index + 1).map(|path| {
                let path = path.to_string_lossy();
                PathBuf::from(path.trim_matches('"'))
            });
        }

        let (name, path) = argument.split_once('=')?;
        name.eq_ignore_ascii_case("-projectPath")
            .then(|| PathBuf::from(path.trim_matches('"')))
    })
}

fn paths_match(left: &Path, right: &Path) -> bool {
    comparable_path(left) == comparable_path(right)
}

fn comparable_path(path: &Path) -> String {
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path));
    let normalized = canonical.to_string_lossy().replace('/', "\\");
    let normalized = if let Some(path) = normalized.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{path}")
    } else {
        normalized
            .strip_prefix(r"\\?\")
            .unwrap_or(&normalized)
            .to_string()
    };

    normalized.trim_end_matches('\\').to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(arguments: &[&str]) -> Vec<OsString> {
        arguments.iter().map(OsString::from).collect()
    }

    #[test]
    fn reads_separate_project_path_argument() {
        let command = command(&[
            r"C:\Unity\Editor\Unity.exe",
            "-projectPath",
            r#""C:\Projects\Avatar""#,
        ]);

        assert_eq!(
            project_path_from_command(&command),
            Some(PathBuf::from(r"C:\Projects\Avatar"))
        );
    }

    #[test]
    fn reads_equals_project_path_argument_case_insensitively() {
        let command = command(&[
            r"C:\Unity\Editor\Unity.exe",
            r"-PROJECTPATH=C:\Projects\Avatar",
        ]);

        assert_eq!(
            project_path_from_command(&command),
            Some(PathBuf::from(r"C:\Projects\Avatar"))
        );
    }

    #[test]
    fn matches_windows_project_paths_case_insensitively() {
        assert!(paths_match(
            Path::new(r"C:\Projects\Avatar"),
            Path::new(r"c:/projects/avatar/")
        ));
    }

    #[test]
    fn ignores_commands_without_project_path() {
        let command = command(&[r"C:\Unity\Editor\Unity.exe", "-batchmode"]);

        assert_eq!(project_path_from_command(&command), None);
    }
}
