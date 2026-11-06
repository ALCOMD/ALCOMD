use std::io::{self, Read, Write};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

fn system_helper() -> io::Result<PathBuf> {
    let path = std::env::var_os("PATH").ok_or(io::ErrorKind::NotFound)?;
    std::env::split_paths(&path)
        .filter(|directory| directory.is_absolute())
        .filter_map(|directory| directory.join("secret-tool").canonicalize().ok())
        .find(|binary| {
            binary.is_file()
                && binary.ancestors().all(|entry| {
                    entry
                        .metadata()
                        .is_ok_and(|metadata| metadata.uid() == 0 && metadata.mode() & 0o022 == 0)
                })
        })
        .ok_or_else(|| io::ErrorKind::NotFound.into())
}

fn command(action: &str, reference: &str) -> io::Result<Command> {
    let mut command = Command::new(system_helper()?);
    command.arg(action);
    if action == "store" {
        command.args(["--label", super::SERVICE]);
    }
    command
        .args(["service", super::SERVICE, "reference", reference])
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .stdin(Stdio::null())
        .env_remove("LD_PRELOAD")
        .env_remove("LD_LIBRARY_PATH")
        .env_remove("LD_AUDIT")
        .env_remove("GIO_EXTRA_MODULES");
    Ok(command)
}

fn wait(process: &mut Child) -> io::Result<ExitStatus> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = process.try_wait()? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            let _ = process.kill();
            let _ = process.wait();
            return Err(io::ErrorKind::TimedOut.into());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub(super) fn write(reference: &str, value: &[u8]) -> io::Result<()> {
    let mut process = command("store", reference)?.stdin(Stdio::piped()).spawn()?;
    let written = process
        .stdin
        .take()
        .ok_or(io::ErrorKind::BrokenPipe)?
        .write_all(value);
    let status = wait(&mut process)?;
    written?;
    if !status.success() {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(())
}

pub(super) fn read(reference: &str) -> io::Result<Vec<u8>> {
    let mut process = command("lookup", reference)?
        .stdout(Stdio::piped())
        .spawn()?;
    if !wait(&mut process)?.success() {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let mut bytes = Vec::new();
    process
        .stdout
        .take()
        .ok_or(io::ErrorKind::BrokenPipe)?
        .take((super::MAX_BYTES + 2) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.last() == Some(&b'\n') {
        bytes.pop();
    }
    Ok(bytes)
}

pub(super) fn delete(reference: &str) -> io::Result<()> {
    let mut process = command("clear", reference)?.spawn()?;
    let status = wait(&mut process)?;
    if !status.success() {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    Ok(())
}
