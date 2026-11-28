use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use alcomd_platform::{
    open_owned_installation_record, open_plain_file, resolve_plain_storage_directory,
    sync_directory, validate_installation_record_parent,
};
use anyhow::{Context, ensure};
use serde::{Deserialize, Serialize};

use super::{
    AcceptedUpdate, UpdateRequest, UpdateStream, VerifiedUpdate, check_replay, verify_update,
};
use crate::Channel;

const RECORD_LIMIT: u64 = 4096;
static NEXT_PARTIAL: AtomicU64 = AtomicU64::new(0);

pub(super) fn verify_download_replay(
    directory: &Path,
    accepted: AcceptedUpdate,
) -> anyhow::Result<()> {
    let journal = ReplayJournal::open(directory, accepted.stream)?;
    check_replay(accepted, journal.previous()?)
}

/// Verifies an offline update and durably accepts its replay receipt in an existing directory.
pub fn verify_and_record_update(
    directory: &Path,
    body: &[u8],
    request: &UpdateRequest<'_>,
    asset_path: &Path,
) -> anyhow::Result<VerifiedUpdate> {
    verify_and_record(directory, body, request, asset_path, RecordScope::User)
}

#[cfg(windows)]
pub(super) fn verify_and_record_machine_update(
    directory: &Path,
    body: &[u8],
    request: &UpdateRequest<'_>,
    asset_path: &Path,
) -> anyhow::Result<VerifiedUpdate> {
    verify_and_record(directory, body, request, asset_path, RecordScope::Machine)
}

fn verify_and_record(
    directory: &Path,
    body: &[u8],
    request: &UpdateRequest<'_>,
    asset_path: &Path,
    scope: RecordScope,
) -> anyhow::Result<VerifiedUpdate> {
    ensure!(
        request.previous.is_none(),
        "a stored replay receipt cannot be combined with a caller supplied receipt"
    );
    let verified = verify_update(body, request)?;
    verified.asset().verify_file(asset_path)?;
    let journal = ReplayJournal::open_with_scope(directory, request.stream, scope)?;
    let previous = journal.previous()?;
    check_replay(verified.acceptance(), previous)?;
    if previous != Some(verified.acceptance()) {
        journal.accept(verified.acceptance())?;
    }
    Ok(verified)
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Record {
    schema: u32,
    accepted: AcceptedUpdate,
}

pub(super) struct ReplayJournal {
    directory: PathBuf,
    identity: Vec<u8>,
    stem: String,
    stream: UpdateStream,
    scope: RecordScope,
    _lock: File,
}

impl ReplayJournal {
    pub(super) fn open(directory: &Path, stream: UpdateStream) -> anyhow::Result<Self> {
        Self::open_with_scope(directory, stream, RecordScope::User)
    }

    fn open_with_scope(
        directory: &Path,
        stream: UpdateStream,
        scope: RecordScope,
    ) -> anyhow::Result<Self> {
        let (directory, identity) = resolve_plain_storage_directory(directory)?;
        ensure!(
            stream.target == stream.format.target(),
            "invalid update replay stream"
        );
        let channel = match stream.channel {
            Channel::Stable => "stable",
            Channel::Beta => "beta",
        };
        let stem = format!("update-{channel}-{}", stream.format.feed_name());
        let lock_path = directory.join(format!("{stem}.lock"));
        scope.validate_parent(&lock_path)?;
        match scope.create(&lock_path) {
            Ok(created) => drop(created),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error).context("cannot open product update replay lock"),
        }
        let (_, lock_identity) = open_plain_file(&lock_path)?;
        let lock = scope.open(&lock_path)?;
        lock.try_lock().context("product update stream is busy")?;
        ensure!(
            open_plain_file(&lock_path)?.1 == lock_identity
                && resolve_plain_storage_directory(&directory)?.1 == identity,
            "product update replay journal changed while locking"
        );
        Ok(Self {
            directory,
            identity,
            stem,
            stream,
            scope,
            _lock: lock,
        })
    }

    pub(super) fn previous(&self) -> anyhow::Result<Option<AcceptedUpdate>> {
        self.ensure_directory()?;
        let path = self.record_path();
        match self.scope.open(&path) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take(RECORD_LIMIT + 1).read_to_end(&mut bytes)?;
                ensure!(
                    bytes.len() as u64 <= RECORD_LIMIT,
                    "product update replay record exceeds its size limit"
                );
                let record: Record = serde_json::from_slice(&bytes)
                    .map_err(|_| anyhow::anyhow!("invalid product update replay record"))?;
                ensure!(
                    record.schema == 1 && record.accepted.stream == self.stream,
                    "invalid product update replay record"
                );
                check_replay(record.accepted, None)?;
                Ok(Some(record.accepted))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).context("cannot inspect product update replay record"),
        }
    }

    pub(super) fn accept(&self, accepted: AcceptedUpdate) -> anyhow::Result<()> {
        let previous = self.previous()?;
        check_replay(accepted, previous)?;
        self.ensure_directory()?;
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let partial = self.directory.join(format!(
            "{}.{}-{stamp}-{}.partial",
            self.stem,
            std::process::id(),
            NEXT_PARTIAL.fetch_add(1, Ordering::Relaxed)
        ));
        let bytes = serde_json::to_vec(&Record {
            schema: 1,
            accepted,
        })?;
        let mut file = self.scope.create(&partial)?;
        let result = (|| {
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            self.ensure_directory()?;
            fs::rename(&partial, self.record_path())?;
            sync_directory(&self.directory)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&partial);
        }
        result
    }

    fn record_path(&self) -> PathBuf {
        self.directory.join(format!("{}-v1.json", self.stem))
    }

    fn ensure_directory(&self) -> anyhow::Result<()> {
        ensure!(
            resolve_plain_storage_directory(&self.directory)?.1 == self.identity,
            "product update replay journal directory changed"
        );
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum RecordScope {
    User,
    #[cfg(windows)]
    Machine,
}

impl RecordScope {
    fn validate_parent(self, path: &Path) -> io::Result<()> {
        match self {
            Self::User => validate_installation_record_parent(path),
            #[cfg(windows)]
            Self::Machine => {
                alcomd_platform::validate_windows_machine_installation_record_parent(path)
            }
        }
    }

    fn open(self, path: &Path) -> io::Result<File> {
        match self {
            Self::User => open_owned_installation_record(path),
            #[cfg(windows)]
            Self::Machine => alcomd_platform::open_windows_machine_installation_record(path),
        }
    }

    fn create(self, path: &Path) -> io::Result<File> {
        match self {
            Self::User => create_private_file(path),
            #[cfg(windows)]
            Self::Machine => alcomd_platform::create_windows_machine_installation_record(path),
        }
    }
}

fn create_private_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}
