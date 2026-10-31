//! Bounded, owned Unity Hub process dispatch. Location inspection stays read-only.

use std::io;
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{SyncSender, sync_channel};
use std::thread;

use crate::{UnityHubPathError, normalize_unity_hub_path};

const MAX_ACTIVE_REAPERS: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnityHubLaunchError {
    PathInvalid,
    PathChanged,
    Unavailable,
    Busy,
    OpenFailed,
}

/// One daemon-owned capacity bound; clones share all active child slots.
#[derive(Clone, Default)]
pub struct UnityHubProcessLauncher {
    active: Arc<AtomicUsize>,
}

struct ReaperPermit(Arc<AtomicUsize>);

impl Drop for ReaperPermit {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

impl UnityHubProcessLauncher {
    /// Returns after OS acceptance, not after Hub closes or creates a window.
    /// The application directly supplies only the resolved program path, with no
    /// shell command, arguments, project target, or inherited standard streams.
    /// Operating-system executable/interpreter semantics still apply.
    pub fn launch(&self, resolved_path: &str) -> Result<(), UnityHubLaunchError> {
        self.launch_with_worker(resolved_path, |work| {
            thread::Builder::new()
                .name("hub-process-reaper".to_owned())
                .spawn(work)
                .map(|_| ())
        })
    }

    fn reserve(&self) -> Result<ReaperPermit, UnityHubLaunchError> {
        self.active
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |active| {
                (active < MAX_ACTIVE_REAPERS).then_some(active + 1)
            })
            .map_err(|_| UnityHubLaunchError::Busy)?;
        Ok(ReaperPermit(Arc::clone(&self.active)))
    }

    fn launch_with_worker(
        &self,
        resolved_path: &str,
        start_worker: impl FnOnce(Box<dyn FnOnce() + Send>) -> io::Result<()>,
    ) -> Result<(), UnityHubLaunchError> {
        validate_resolved_path(resolved_path)?;
        let permit = self.reserve()?;
        let path = resolved_path.to_owned();
        let (accepted, result) = sync_channel(1);
        // The worker must exist before it can create any child. A creation error
        // drops its closure/permit without having launched an untracked process.
        start_worker(Box::new(move || dispatch_and_reap(path, accepted, permit)))
            .map_err(|_| UnityHubLaunchError::OpenFailed)?;
        result.recv().map_err(|_| UnityHubLaunchError::OpenFailed)?
    }
}

fn validate_resolved_path(path: &str) -> Result<(), UnityHubLaunchError> {
    let normalized = normalize_unity_hub_path(path).map_err(|error| match error {
        UnityHubPathError::Invalid => UnityHubLaunchError::PathInvalid,
        UnityHubPathError::Unavailable => UnityHubLaunchError::Unavailable,
    })?;
    if normalized != path {
        return Err(UnityHubLaunchError::PathChanged);
    }
    Ok(())
}

fn hub_command(path: &str) -> Command {
    let mut command = Command::new(path);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

fn dispatch_and_reap(
    path: String,
    accepted: SyncSender<Result<(), UnityHubLaunchError>>,
    _permit: ReaperPermit,
) {
    // Revalidate after reserving/establishing the worker. This is an inspection-time
    // usability check, not a vendor check or an immutable binary-identity guarantee.
    if let Err(error) = validate_resolved_path(&path) {
        let _ = accepted.send(Err(error));
        return;
    }
    match hub_command(&path).spawn() {
        Ok(child) => {
            let _ = accepted.send(Ok(()));
            // Receiver cancellation never skips reaping or kills a user Hub.
            wait_owned_child(child);
        }
        Err(_) => {
            let _ = accepted.send(Err(UnityHubLaunchError::OpenFailed));
        }
    }
}

fn wait_owned_child(mut child: Child) {
    loop {
        match child.wait() {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            _ => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;
    use std::time::{Duration, Instant};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct InvalidExecutable {
        root: PathBuf,
        path: String,
    }

    impl InvalidExecutable {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "alcomd-hub-open-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).expect("private fixture");
            let path = root.join("not-a-program.exe");
            #[cfg(windows)]
            fs::write(&path, b"not an executable image").expect("invalid executable fixture");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let interpreter = root.join("missing-interpreter");
                assert!(!interpreter.exists(), "fixture interpreter must be absent");
                // A plain executable text file can be accepted through libc's
                // ENOEXEC shell fallback on macOS. A recognized shebang whose
                // interpreter is missing instead fails synchronously with ENOENT.
                fs::write(
                    &path,
                    format!("#!{}\n", interpreter.to_str().expect("UTF-8 interpreter")),
                )
                .expect("missing-interpreter fixture");
                fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                    .expect("fixture execute bit");
            }
            Self {
                path: normalize_unity_hub_path(path.to_str().expect("UTF-8 fixture"))
                    .expect("usable file with an intentional spawn failure"),
                root,
            }
        }
    }

    impl Drop for InvalidExecutable {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn harmless_executable() -> String {
        #[cfg(windows)]
        let path = PathBuf::from(std::env::var_os("SystemRoot").expect("Windows SystemRoot"))
            .join("System32/whoami.exe");
        #[cfg(unix)]
        let path = PathBuf::from("/usr/bin/true");
        normalize_unity_hub_path(path.to_str().expect("UTF-8 system utility"))
            .expect("read-only system utility")
    }

    fn unsuccessful_executable() -> String {
        #[cfg(windows)]
        let path = PathBuf::from(std::env::var_os("SystemRoot").expect("Windows SystemRoot"))
            .join("System32/find.exe");
        #[cfg(unix)]
        let path = PathBuf::from("/usr/bin/false");
        normalize_unity_hub_path(path.to_str().expect("UTF-8 system utility"))
            .expect("read-only utility that exits unsuccessfully without arguments")
    }

    fn wait_for_idle(launcher: &UnityHubProcessLauncher) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while launcher.active.load(Ordering::Acquire) != 0 {
            assert!(Instant::now() < deadline, "owned child must be reaped");
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn launcher_reaps_its_owned_process_without_shell_or_arguments() {
        let launcher = UnityHubProcessLauncher::default();
        let path = harmless_executable();
        let command = hub_command(&path);
        assert_eq!(command.get_program(), std::ffi::OsStr::new(&path));
        assert_eq!(command.get_args().count(), 0);
        assert_eq!(launcher.launch(&path), Ok(()));
        wait_for_idle(&launcher);
    }

    #[test]
    fn closed_response_receiver_still_waits_and_releases_child_capacity() {
        let launcher = UnityHubProcessLauncher::default();
        let permit = launcher.reserve().expect("capacity");
        let (accepted, result) = sync_channel(1);
        drop(result);
        dispatch_and_reap(harmless_executable(), accepted, permit);
        assert_eq!(launcher.active.load(Ordering::Acquire), 0);
    }

    #[test]
    fn capacity_is_shared_by_clones_and_reserved_before_worker_creation() {
        let launcher = UnityHubProcessLauncher::default();
        let mut slots = Vec::new();
        for _ in 0..MAX_ACTIVE_REAPERS {
            slots.push(launcher.reserve().expect("bounded slot"));
        }
        let worker_called = AtomicBool::new(false);
        let cloned = launcher.clone();
        let result = cloned.launch_with_worker(&harmless_executable(), |_| {
            worker_called.store(true, Ordering::Release);
            Ok(())
        });
        assert_eq!(result, Err(UnityHubLaunchError::Busy));
        assert!(!worker_called.load(Ordering::Acquire));
        drop(slots.pop());
        assert!(cloned.reserve().is_ok());
        drop(slots);
        assert_eq!(launcher.active.load(Ordering::Acquire), 0);
    }

    #[test]
    fn worker_creation_failure_cannot_launch_and_releases_its_slot() {
        let launcher = UnityHubProcessLauncher::default();
        let result = launcher.launch_with_worker(&harmless_executable(), |work| {
            // Dropping work proves the launch closure is never entered.
            drop(work);
            Err(io::Error::other("isolated worker failure"))
        });
        assert_eq!(result, Err(UnityHubLaunchError::OpenFailed));
        assert_eq!(launcher.active.load(Ordering::Acquire), 0);
    }

    #[test]
    fn invalid_path_never_creates_a_worker() {
        let launcher = UnityHubProcessLauncher::default();
        assert_eq!(
            launcher.launch_with_worker("relative-path", |_| panic!("no worker for invalid path")),
            Err(UnityHubLaunchError::PathInvalid)
        );
        assert_eq!(launcher.active.load(Ordering::Acquire), 0);
    }

    #[test]
    fn failed_os_spawn_releases_its_reserved_slot() {
        let fixture = InvalidExecutable::new();
        let error = hub_command(&fixture.path)
            .spawn()
            .map(|mut child| child.wait().expect("reap an unexpectedly accepted fixture"))
            .expect_err("fixture must fail synchronously at spawn");
        assert!(error.raw_os_error().is_some(), "OS spawn error: {error}");
        #[cfg(unix)]
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        let launcher = UnityHubProcessLauncher::default();
        assert_eq!(
            launcher.launch(&fixture.path),
            Err(UnityHubLaunchError::OpenFailed)
        );
        wait_for_idle(&launcher);
    }

    #[test]
    fn nonzero_child_exit_is_still_an_accepted_and_reaped_launch() {
        let path = unsuccessful_executable();
        let mut child = hub_command(&path).spawn().expect("OS accepts the utility");
        assert!(
            !child
                .wait()
                .expect("reap the unsuccessful utility")
                .success(),
            "fixture exits unsuccessfully after OS acceptance"
        );
        let launcher = UnityHubProcessLauncher::default();
        assert_eq!(launcher.launch(&path), Ok(()));
        wait_for_idle(&launcher);
    }

    #[test]
    fn worker_revalidates_after_reservation_before_process_creation() {
        let fixture = InvalidExecutable::new();
        let launcher = UnityHubProcessLauncher::default();
        let result = launcher.launch_with_worker(&fixture.path, |work| {
            fs::remove_file(&fixture.path).expect("fixture removed before worker dispatch");
            work();
            Ok(())
        });
        assert_eq!(result, Err(UnityHubLaunchError::PathInvalid));
        assert_eq!(launcher.active.load(Ordering::Acquire), 0);
    }
}
