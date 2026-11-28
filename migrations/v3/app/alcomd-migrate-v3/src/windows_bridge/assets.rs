use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

use alcomd_platform::{
    open_owned_installation_record, open_plain_file, open_windows_machine_installation_record,
    publish_path_no_replace, resolve_plain_storage_directory, sync_directory,
};
use anyhow::{Context, ensure};
use sha2::{Digest, Sha256};

pub(super) struct Bundle {
    pub version: &'static str,
    pub channel: &'static str,
    pub install_directory: &'static str,
    pub source_commit: &'static str,
    pub product_manifest: &'static str,
    pub bridge_manifest: &'static str,
    pub bootstrap: &'static str,
    pub daemon: &'static str,
    pub migration_reader: &'static str,
    pub discord_sha256: &'static str,
    pub files: &'static [Component],
    pub runtime: &'static [ProgramFile],
}

pub(super) struct Component {
    pub path: &'static str,
    pub sha256: &'static str,
    pub bytes: &'static [u8],
}

pub(super) struct ProgramFile {
    pub path: &'static str,
    pub sha256: &'static str,
}

#[cfg(not(test))]
pub(super) fn embedded() -> Option<Bundle> {
    Some(include!(env!("ALCOMD_WINDOWS_BRIDGE_EMBED")))
}

#[cfg(test)]
pub(super) fn embedded() -> Option<Bundle> {
    None
}

impl Bundle {
    pub(super) fn populate(&self, root: &Path) -> anyhow::Result<()> {
        let mut expected = BTreeSet::new();
        for component in self.files {
            let path = Path::new(component.path);
            ensure!(
                valid_relative(component.path) && expected.insert(path.to_owned()),
                "invalid or duplicate bridge component path"
            );
            let destination = root.join(path);
            let parent = destination
                .parent()
                .context("bridge component has no parent")?;
            super::prepare_dir(parent)?;
            match destination.symlink_metadata() {
                Ok(_) => {
                    let _guard = open_component(&destination, component.sha256, true)?;
                }
                Err(error) if error.kind() == ErrorKind::NotFound => {
                    publish_bytes(&destination, component.bytes)?;
                    let _guard = open_component(&destination, component.sha256, true)?;
                }
                Err(error) => return Err(error.into()),
            }
        }
        verify_file_set(root, &expected)
    }

    pub(super) fn verify_program(&self, program_stage: &Path, machine: bool) -> anyhow::Result<()> {
        let (stage, _) = resolve_plain_storage_directory(program_stage)?;
        let mut expected = BTreeSet::new();
        for file in self.runtime {
            ensure!(
                valid_relative(file.path) && expected.insert(PathBuf::from(file.path)),
                "invalid or duplicate staged product component"
            );
            let path = stage.join(file.path);
            resolve_plain_storage_directory(path.parent().context("missing file parent")?)?;
            let guard = if machine {
                open_windows_machine_installation_record(&path)?
            } else {
                open_owned_installation_record(&path)?
            };
            ensure!(
                hash(&guard)? == file.sha256,
                "staged Windows program changed"
            );
        }
        verify_file_set(&stage, &expected)
    }

    pub(super) fn lock_files(&self, root: &Path) -> anyhow::Result<Vec<File>> {
        self.files
            .iter()
            .map(|component| self.hold(root, component.path))
            .collect()
    }

    pub(super) fn hold(&self, root: &Path, path: &str) -> anyhow::Result<File> {
        let component = self
            .files
            .iter()
            .find(|component| component.path == path)
            .context("missing signed bridge component")?;
        open_component(&root.join(path), component.sha256, true)
    }
}

fn valid_relative(value: &str) -> bool {
    value.len() <= 255
        && value.split('/').count() <= 4
        && value.split('/').all(|component| {
            !component.is_empty()
                && !component.ends_with('.')
                && component
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        })
}

fn verify_file_set(root: &Path, expected: &BTreeSet<PathBuf>) -> anyhow::Result<()> {
    let mut files = BTreeSet::new();
    collect(root, root, &mut files, 0, expected.len())?;
    ensure!(&files == expected, "bridge directory has unlisted files");
    Ok(())
}

fn collect(
    root: &Path,
    path: &Path,
    output: &mut BTreeSet<PathBuf>,
    depth: usize,
    max_files: usize,
) -> anyhow::Result<()> {
    ensure!(depth < 5, "bridge directory has excess depth");
    resolve_plain_storage_directory(path)?;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect(root, &path, output, depth + 1, max_files)?;
        } else {
            open_plain_file(&path)?;
            ensure!(
                output.len() < max_files && output.insert(path.strip_prefix(root)?.to_owned()),
                "bridge directory has excess files"
            );
        }
    }
    Ok(())
}

fn open_component(path: &Path, expected: &str, user: bool) -> anyhow::Result<File> {
    let file = if user {
        open_owned_installation_record(path)?
    } else {
        open_plain_file(path)?.0
    };
    ensure!(hash(&file)? == expected, "staged bridge component changed");
    Ok(file)
}

pub(super) fn hash(file: &File) -> anyhow::Result<String> {
    let length = file.metadata()?.len();
    ensure!(
        length <= 1024 * 1024 * 1024,
        "bridge file exceeds its limit"
    );
    let mut read = file.take(length + 1);
    let mut sha256 = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    let mut bytes = 0;
    loop {
        let size = read.read(&mut buffer)?;
        if size == 0 {
            break;
        }
        bytes += size as u64;
        sha256.update(&buffer[..size]);
    }
    ensure!(bytes == length, "bridge file size changed");
    Ok(format_digest(&sha256.finalize().into()))
}

pub(super) fn format_digest(digest: &[u8; 32]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(super) fn publish_bytes(destination: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = destination.parent().context("record has no parent")?;
    let (parent, identity) = resolve_plain_storage_directory(parent)?;
    let temporary = parent.join(format!(".bridge-new-{}", uuid::Uuid::new_v4()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        ensure!(
            resolve_plain_storage_directory(&parent)?.1 == identity,
            "bridge publication parent changed"
        );
        publish_path_no_replace(&temporary, destination)?;
        sync_directory(&parent)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}
