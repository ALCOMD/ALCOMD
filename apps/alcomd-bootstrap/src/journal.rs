use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, ensure};
use serde::{Serialize, de::DeserializeOwned};

const RECORD_LIMIT: u64 = 64 * 1024;

/// An isolated workspace with a process-lifetime lock and durable immutable records.
pub(super) struct Journal {
    root: PathBuf,
    identity: Vec<u8>,
    _lock: File,
}

impl Journal {
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let (path, parent_identity) = isolated_child(path)?;
        match fs::create_dir(&path) {
            Ok(()) => {
                let parent = path.parent().context("workspace parent")?;
                ensure!(
                    alcomd_platform::resolve_plain_storage_directory(parent)?.1 == parent_identity,
                    "migration workspace parent changed"
                );
                alcomd_platform::sync_directory(parent)?;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error).context("cannot create migration workspace"),
        }
        let (root, identity) = alcomd_platform::resolve_plain_storage_directory(&path)?;
        let lock_path = root.join("transaction.lock");
        if !lock_path.try_exists()? {
            ensure!(
                fs::read_dir(&root)?.next().is_none(),
                "workspace is not a migration workspace"
            );
        }
        let lock = match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(lock) => lock,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                // Locking an existing file must not resolve another directory through a link.
                let (_, identity) = alcomd_platform::open_plain_file(&lock_path)?;
                let lock = OpenOptions::new().read(true).write(true).open(&lock_path)?;
                ensure!(
                    alcomd_platform::file_identity_key(&lock_path)? == identity,
                    "migration lock changed"
                );
                lock
            }
            Err(error) => return Err(error).context("cannot open migration lock"),
        };
        lock.try_lock()
            .context("another bridge is using this migration workspace")?;
        Ok(Self {
            root,
            identity,
            _lock: lock,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn check(&self) -> anyhow::Result<()> {
        ensure!(
            alcomd_platform::resolve_plain_storage_directory(&self.root)?.1 == self.identity,
            "migration workspace changed"
        );
        Ok(())
    }

    pub fn read<T: DeserializeOwned>(&self, name: &str) -> anyhow::Result<Option<T>> {
        self.check()?;
        let path = self.root.join(name);
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                let value = alcomd_updater::read_limited(&path, RECORD_LIMIT)?;
                Ok(Some(
                    serde_json::from_slice(&value).context("migration journal is damaged")?,
                ))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).context("cannot read migration journal"),
        }
    }

    pub fn record<T: Serialize>(&self, name: &str, value: &T) -> anyhow::Result<()> {
        self.record_with(name, value, File::write_all)
    }

    pub(super) fn record_with<T: Serialize>(
        &self,
        name: &str,
        value: &T,
        write_record: impl FnOnce(&mut File, &[u8]) -> io::Result<()>,
    ) -> anyhow::Result<()> {
        self.check()?;
        let bytes = serde_json::to_vec(value)?;
        ensure!(
            u64::try_from(bytes.len())? <= RECORD_LIMIT,
            "migration record too large"
        );
        let temporary = self.root.join(format!("{name}.partial"));
        if fs::symlink_metadata(&temporary).is_ok() {
            alcomd_platform::open_plain_file(&temporary)
                .context("unsafe partial journal record")?;
            fs::remove_file(&temporary)?;
        }
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        write_record(&mut file, &bytes)?;
        file.sync_all()?;
        drop(file);
        self.check()?;
        alcomd_platform::publish_path_no_replace(&temporary, &self.root.join(name))?;
        alcomd_platform::sync_directory(&self.root)?;
        Ok(())
    }
}

pub(super) fn isolated_child(path: &Path) -> anyhow::Result<(PathBuf, Vec<u8>)> {
    ensure!(path.is_absolute(), "migration path must be absolute");
    let leaf = path
        .file_name()
        .and_then(|name| name.to_str())
        .context("migration path has no Unicode file name")?;
    ensure!(
        leaf.len() <= 255
            && !leaf.ends_with([' ', '.'])
            && !leaf.chars().any(|c| c.is_control()
                || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')),
        "unsafe migration path"
    );
    let (parent, identity) = alcomd_platform::resolve_plain_storage_directory(
        path.parent().context("migration path has no parent")?,
    )?;
    Ok((parent.join(leaf), identity))
}
