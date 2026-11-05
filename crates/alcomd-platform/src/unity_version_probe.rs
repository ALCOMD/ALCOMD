//! Explicit Unix Editor version inspection. Never used by list or launch queries.

use std::fs;
use std::io::{self, Read};
use std::os::fd::AsFd;
use std::os::unix::fs::DirBuilderExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const OUTPUT_LIMIT: usize = 64 * 1024;
const TIME_LIMIT: Duration = Duration::from_secs(10);
static CAPACITY: ProbeCapacity = ProbeCapacity(AtomicUsize::new(0));
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct ProbeCapacity(AtomicUsize);
struct Permit<'a>(&'a ProbeCapacity);

impl ProbeCapacity {
    fn reserve(&self) -> io::Result<Permit<'_>> {
        self.0
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < 2).then_some(count + 1)
            })
            .map(|_| Permit(self))
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "Unity version probe capacity exhausted",
                )
            })
    }
}

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        self.0.0.fetch_sub(1, Ordering::AcqRel);
    }
}

struct WorkingDirectory(PathBuf);

impl WorkingDirectory {
    fn create() -> io::Result<Self> {
        let root = fs::canonicalize(std::env::temp_dir())?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        let leaf = format!(
            "alcomd-editor-version-{}-{now}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        );
        let path = root.join(leaf);
        fs::DirBuilder::new().mode(0o700).create(&path)?;
        Ok(Self(path))
    }
}

impl Drop for WorkingDirectory {
    fn drop(&mut self) {
        // Only remove the empty directory we created. Unexpected probe artifacts
        // remain available for diagnosis rather than recursively deleting them.
        let _ = fs::remove_dir(&self.0);
    }
}

struct OwnedChild(Child);

impl Drop for OwnedChild {
    fn drop(&mut self) {
        // This handle belongs only to the -version child, never an existing Editor.
        if !matches!(self.0.try_wait(), Ok(Some(_))) {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

pub(super) fn probe(executable: &Path) -> io::Result<String> {
    probe_with_limit(executable, TIME_LIMIT)
}

fn probe_with_limit(executable: &Path, limit: Duration) -> io::Result<String> {
    let deadline = Instant::now() + limit;
    let _permit = CAPACITY.reserve()?;
    let directory = WorkingDirectory::create()?;
    let mut child = OwnedChild(
        Command::new(executable)
            .arg("-version")
            .current_dir(&directory.0)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
    );
    let mut stdout = child
        .0
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("version stdout missing"))?;
    let mut stderr = child
        .0
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("version stderr missing"))?;
    nonblocking(&stdout)?;
    nonblocking(&stderr)?;
    let mut output = Vec::new();
    let mut errors = Vec::new();
    let mut stdout_closed = false;
    let mut stderr_closed = false;
    loop {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Unity version probe timed out",
            ));
        }
        if !stdout_closed {
            stdout_closed = drain(&mut stdout, &mut output, deadline)?;
        }
        if !stderr_closed {
            stderr_closed = drain(&mut stderr, &mut errors, deadline)?;
        }
        if let Some(status) = child.0.try_wait()? {
            if !status.success() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Unity version probe exited unsuccessfully",
                ));
            }
            if stdout_closed && stderr_closed {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let output = String::from_utf8(output).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Unity version output is not UTF-8",
        )
    })?;
    let output = output.trim();
    if output.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unity version output is empty",
        ));
    }
    Ok(output.to_owned())
}

fn nonblocking(stream: &impl AsFd) -> io::Result<()> {
    let flags = rustix::fs::fcntl_getfl(stream).map_err(io::Error::from)?;
    rustix::fs::fcntl_setfl(stream, flags | rustix::fs::OFlags::NONBLOCK).map_err(io::Error::from)
}

fn drain(stream: &mut impl Read, bytes: &mut Vec<u8>, deadline: Instant) -> io::Result<bool> {
    let mut buffer = [0; 4096];
    loop {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "Unity version probe timed out",
            ));
        }
        match stream.read(&mut buffer) {
            Ok(0) => return Ok(true),
            Ok(count) => {
                if count > OUTPUT_LIMIT - bytes.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Unity version output exceeds 64 KiB",
                    ));
                }
                bytes.extend_from_slice(&buffer[..count]);
            }
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn script(root: &Path, name: &str, body: &str) -> PathBuf {
        let path = root.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("owned script");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).expect("script permissions");
        path
    }

    #[test]
    fn probe_uses_only_version_argv_and_empty_independent_cwd() {
        let root = WorkingDirectory::create().expect("fixture");
        let executable = script(
            &root.0,
            "Unity",
            "[ \"$#\" = 1 ] && [ \"$1\" = -version ] && [ -z \"$(ls -A)\" ] || exit 2\nprintf '2022.3.22f1 (887be4894c44)\\n'",
        );
        assert_eq!(
            probe(&executable).expect("version"),
            "2022.3.22f1 (887be4894c44)"
        );
        fs::remove_file(executable).expect("remove script");
    }

    #[test]
    fn probe_bounds_each_stream_and_reaps_its_child_on_timeout() {
        let root = WorkingDirectory::create().expect("fixture");
        for (name, body, kind) in [
            (
                "stdout",
                "head -c 65537 /dev/zero",
                io::ErrorKind::InvalidData,
            ),
            (
                "stderr",
                "head -c 65537 /dev/zero >&2",
                io::ErrorKind::InvalidData,
            ),
            ("nonzero", "exit 1", io::ErrorKind::InvalidData),
            ("blocked", "while :; do :; done", io::ErrorKind::TimedOut),
        ] {
            let executable = script(&root.0, name, body);
            let error = probe_with_limit(&executable, Duration::from_millis(200)).expect_err(name);
            assert_eq!(error.kind(), kind, "{name}");
            fs::remove_file(executable).expect("remove script");
        }
    }

    #[test]
    fn timed_out_child_is_no_longer_running() {
        let root = WorkingDirectory::create().expect("fixture");
        let pid_path = root.0.join("child.pid");
        let body = format!("echo $$ > '{}'\nwhile :; do :; done", pid_path.display());
        let executable = script(&root.0, "timeout", &body);
        assert_eq!(
            probe_with_limit(&executable, Duration::from_millis(200))
                .expect_err("timeout")
                .kind(),
            io::ErrorKind::TimedOut
        );
        let pid: i32 = fs::read_to_string(&pid_path)
            .expect("child pid")
            .trim()
            .parse()
            .expect("pid");
        let pid = rustix::process::Pid::from_raw(pid).expect("positive child pid");
        assert_eq!(
            rustix::process::test_kill_process(pid),
            Err(rustix::io::Errno::SRCH)
        );
        fs::remove_file(executable).expect("remove script");
        fs::remove_file(pid_path).expect("remove pid");
    }

    #[test]
    fn only_two_probe_processes_can_be_reserved() {
        let capacity = ProbeCapacity(AtomicUsize::new(0));
        let first = capacity.reserve().expect("first");
        let second = capacity.reserve().expect("second");
        assert_eq!(
            capacity.reserve().err().expect("capacity").kind(),
            io::ErrorKind::WouldBlock
        );
        drop(first);
        let replacement = capacity.reserve().expect("released slot");
        assert_eq!(capacity.0.load(Ordering::Acquire), 2);
        drop((second, replacement));
        assert_eq!(capacity.0.load(Ordering::Acquire), 0);
    }

    #[test]
    fn layout_validation_does_not_probe_and_explicit_inspection_binds_file_identity() {
        let root = WorkingDirectory::create().expect("fixture");
        let editor = root.0.join("Editor");
        let resources = editor.join("Data/Resources/PackageManager/Editor");
        fs::create_dir_all(&resources).expect("layout");
        let manifest = resources.join("manifest.json");
        fs::write(&manifest, r#"{"schemaVersion":4,"packages":{}}"#).expect("package catalog");
        let marker = root.0.join("probe-called");
        let body = format!("touch '{}'\nprintf '2022.3.22f1\\n'", marker.display());
        let executable = script(&editor, "Unity", &body);
        let validated = super::super::validate_unity_executable(&executable).expect("layout only");
        assert_eq!(validated.probed_version(), None);
        assert!(!marker.exists());
        let inspected =
            super::super::inspect_unity_installation(&executable).expect("explicit probe");
        assert_eq!(inspected.probed_version(), Some("2022.3.22f1"));
        assert!(marker.exists());
        let body = format!(
            "cp '{}' '{}.new'\nmv '{}.new' '{}'\nprintf '2022.3.22f1\\n'",
            executable.display(),
            executable.display(),
            executable.display(),
            executable.display()
        );
        fs::write(&executable, format!("#!/bin/sh\n{body}\n")).expect("owned replacement script");
        let error =
            super::super::inspect_unity_installation(&executable).expect_err("identity changed");
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        fs::remove_file(executable).expect("remove script");
        fs::remove_file(marker).expect("remove marker");
        fs::remove_file(manifest).expect("remove catalog");
        for path in [
            resources,
            editor.join("Data/Resources/PackageManager"),
            editor.join("Data/Resources"),
            editor.join("Data"),
            editor,
        ] {
            fs::remove_dir(path).expect("remove owned empty layout");
        }
    }
}
