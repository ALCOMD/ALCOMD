//! Minimal read-only process observation for the M5 Unity writer gate.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

const PROCESS_EVIDENCE_LIMIT: usize = 32_768;
const PROCESS_NAME_BYTE_LIMIT: usize = 4_096;
const EXECUTABLE_PATH_BYTE_LIMIT: usize = 32_768;
const ARGUMENT_COUNT_LIMIT: usize = 256;
const ARGUMENT_ITEM_BYTE_LIMIT: usize = 4_096;
const ARGUMENT_TOTAL_BYTE_LIMIT: usize = 65_536;

/// One owned, short-lived process observation with no third-party types.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessEvidence {
    pid: u32,
    start_time: u64,
    name: OsString,
    executable: Option<PathBuf>,
    arguments: Option<Vec<OsString>>,
}

impl ProcessEvidence {
    /// Numeric PID observed in this snapshot; it is not a persistent identity.
    #[must_use]
    pub const fn pid(&self) -> u32 {
        self.pid
    }

    /// Process start time observed with the PID to mitigate PID reuse.
    #[must_use]
    pub const fn start_time(&self) -> u64 {
        self.start_time
    }

    /// Platform process name used only for conservative candidate detection.
    #[must_use]
    pub fn name(&self) -> &OsStr {
        &self.name
    }

    /// Executable path when the operating system made it available.
    #[must_use]
    pub fn executable(&self) -> Option<&Path> {
        self.executable.as_deref()
    }

    /// Owned argv items when the operating system made a non-empty vector available.
    #[must_use]
    pub fn arguments(&self) -> Option<&[OsString]> {
        self.arguments.as_deref()
    }
}

/// One complete platform enumeration attempt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessSnapshot {
    processes: Vec<ProcessEvidence>,
}

impl ProcessSnapshot {
    /// Returns the observations owned by this snapshot.
    #[must_use]
    pub fn processes(&self) -> &[ProcessEvidence] {
        &self.processes
    }
}

/// Performs the exact minimal sysinfo refresh approved for M5.
///
/// A new `System` is used for every call so concurrent callers share no mutable
/// process state. CPU, memory, disk, cwd, root, environment and user identity
/// are never requested. Missing exe/cmd values remain missing.
#[must_use]
pub fn observe_processes() -> ProcessSnapshot {
    let mut system = System::new();
    let refresh = ProcessRefreshKind::nothing()
        .without_tasks()
        .with_exe(UpdateKind::OnlyIfNotSet)
        .with_cmd(UpdateKind::OnlyIfNotSet);
    let _ = system.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh);
    let mut processes = system
        .processes()
        .iter()
        .map(|(pid, process)| {
            let executable = process
                .exe()
                .filter(|path| !path.as_os_str().is_empty())
                .filter(|path| {
                    path.as_os_str().as_encoded_bytes().len() <= EXECUTABLE_PATH_BYTE_LIMIT
                })
                .map(Path::to_path_buf);
            let arguments = bounded_arguments(process.cmd());
            let name = if process.name().as_encoded_bytes().len() <= PROCESS_NAME_BYTE_LIMIT {
                process.name().to_os_string()
            } else {
                OsString::new()
            };
            ProcessEvidence {
                pid: pid.as_u32(),
                start_time: process.start_time(),
                name,
                executable,
                arguments,
            }
        })
        .collect::<Vec<_>>();
    processes.sort_by_key(|process| (process.pid, process.start_time));
    processes.truncate(PROCESS_EVIDENCE_LIMIT);
    ProcessSnapshot { processes }
}

fn bounded_arguments(arguments: &[OsString]) -> Option<Vec<OsString>> {
    if arguments.is_empty() || arguments.len() > ARGUMENT_COUNT_LIMIT {
        return None;
    }
    let mut total = 0_usize;
    for argument in arguments {
        let size = argument.as_encoded_bytes().len();
        if size > ARGUMENT_ITEM_BYTE_LIMIT {
            return None;
        }
        total = total.checked_add(size)?;
        if total > ARGUMENT_TOTAL_BYTE_LIMIT {
            return None;
        }
    }
    Some(arguments.to_vec())
}

#[cfg(test)]
mod tests {
    use super::observe_processes;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    #[test]
    fn current_process_has_stable_pid_start_and_no_shared_refresh_state() {
        let pid = std::process::id();
        let first = observe_processes();
        let current = first
            .processes()
            .iter()
            .find(|process| process.pid() == pid)
            .expect("current process");
        assert!(current.start_time() > 0);
        assert!(current.executable().is_some());
        let start_time = current.start_time();

        let second = observe_processes();
        let current = second
            .processes()
            .iter()
            .find(|process| process.pid() == pid)
            .expect("current process after second snapshot");
        assert_eq!(current.start_time(), start_time);
    }

    #[test]
    fn parallel_observations_do_not_share_mutable_state_or_panic() {
        let workers = (0..8)
            .map(|_| std::thread::spawn(observe_processes))
            .collect::<Vec<_>>();
        for worker in workers {
            assert!(
                !worker
                    .join()
                    .expect("observation thread")
                    .processes()
                    .is_empty()
            );
        }
    }

    #[test]
    fn short_lived_child_exposes_pid_start_and_project_selector_arguments() {
        let mut child = child_process();
        let pid = child.id();
        let deadline = Instant::now() + Duration::from_secs(3);
        let observed = loop {
            if let Some(process) = observe_processes()
                .processes()
                .iter()
                .find(|process| process.pid() == pid)
                .cloned()
            {
                break process;
            }
            assert!(Instant::now() < deadline, "child process was not observed");
            std::thread::sleep(Duration::from_millis(25));
        };
        assert!(observed.start_time() > 0);
        if let Some(executable) = observed.executable() {
            assert!(executable.is_absolute());
        }
        let arguments = observed.arguments().expect("child argv");
        assert!(arguments.iter().any(|value| value == "-projectPath"));
        assert!(
            arguments
                .iter()
                .any(|value| value == "alcomd-process-fixture")
        );

        child.kill().expect("stop test child");
        child.wait().expect("reap test child");
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let absent = observe_processes()
                .processes()
                .iter()
                .all(|process| process.pid() != pid);
            if absent {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "exited child remained observable"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    #[cfg(windows)]
    fn child_process() -> std::process::Child {
        let executable =
            std::path::PathBuf::from(std::env::var_os("WINDIR").expect("Windows directory"))
                .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        Command::new(executable)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Start-Sleep -Seconds 10",
                "-projectPath",
                "alcomd-process-fixture",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn test child")
    }

    #[cfg(unix)]
    fn child_process() -> std::process::Child {
        Command::new("sh")
            .args([
                "-c",
                "sleep 10",
                "alcomd-probe",
                "-projectPath",
                "alcomd-process-fixture",
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn test child")
    }
}
