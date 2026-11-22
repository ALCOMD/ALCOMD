//! Kernel-derived attribution for display. Missing evidence never changes IPC authority.

use std::path::Path;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcessIdentity {
    pub pid: u32,
    pub creation: u64,
    pub image: Vec<u8>,
    pub name: String,
    pub shared_runtime: bool,
}

#[derive(Clone, Debug, Default)]
pub struct ClientPeer {
    pub process: Option<ProcessIdentity>,
    pub bridge_parent: Option<ProcessIdentity>,
}

pub fn peer_pid(stream: &crate::IpcStream) -> Option<u32> {
    #[cfg(unix)]
    let pid = stream.peer_cred().ok()?.pid()?.try_into().ok()?;
    #[cfg(windows)]
    let pid = crate::windows::client_process_id(stream)?;
    (pid != 0).then_some(pid)
}

fn identity(pid: Pid) -> Option<(ProcessIdentity, Option<Pid>)> {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing()
            .without_tasks()
            .with_exe(UpdateKind::Always),
    );
    let process = system.process(pid)?;
    let executable = process.exe()?;
    let name = executable.file_name()?.to_str()?.to_owned();
    #[cfg(windows)]
    let (creation, image) = crate::windows::client_process_image(pid.as_u32(), executable)?;
    #[cfg(target_os = "linux")]
    let (creation, image) = {
        let proc = std::path::PathBuf::from("/proc").join(pid.as_u32().to_string());
        let stat = std::fs::read_to_string(proc.join("stat")).ok()?;
        let creation = stat
            .rsplit_once(')')?
            .1
            .split_whitespace()
            .nth(19)?
            .parse()
            .ok()?;
        let image = crate::file_identity_key(&proc.join("exe")).ok()?;
        if crate::file_identity_key(executable).ok()? != image {
            return None;
        }
        (creation, image)
    };
    #[cfg(target_os = "macos")]
    let (creation, image) = (
        darwin_creation(pid.as_u32())?,
        crate::file_identity_key(executable).ok()?,
    );
    // These executables identify an interpreter or bridge, not the program it hosts.
    let stem = Path::new(&name).file_stem()?.to_str()?.to_ascii_lowercase();
    let shared_runtime = matches!(
        stem.as_str(),
        "alcomd-sdk-bridge"
            | "node"
            | "deno"
            | "bun"
            | "electron"
            | "java"
            | "javaw"
            | "dotnet"
            | "ruby"
            | "pwsh"
            | "powershell"
            | "sh"
            | "bash"
            | "zsh"
    ) || stem
        .strip_prefix("python")
        .is_some_and(|suffix| suffix.bytes().all(|b| b.is_ascii_digit() || b == b'.'));
    Some((
        ProcessIdentity {
            pid: pid.as_u32(),
            creation,
            image,
            name,
            shared_runtime,
        },
        process.parent(),
    ))
}

#[cfg(target_os = "macos")]
#[path = "client_peer_darwin.rs"]
mod darwin;
#[cfg(target_os = "macos")]
use darwin::darwin_creation;

pub fn observe(pid: Option<u32>) -> ClientPeer {
    let Some((process, ppid)) = pid.and_then(|pid| identity(Pid::from_u32(pid))) else {
        return ClientPeer::default();
    };
    let is_bridge = Path::new(&process.name)
        .file_stem()
        .is_some_and(|name| name == "alcomd-sdk-bridge");
    let bridge_parent = if is_bridge {
        ppid.and_then(identity)
            .map(|(parent, _)| parent)
            .filter(|parent| parent.creation <= process.creation)
    } else {
        None
    };
    ClientPeer {
        process: Some(process),
        bridge_parent,
    }
}
