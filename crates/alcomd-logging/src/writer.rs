use crate::{FILE_BYTES, FILE_COUNT, Segment, Shared, lock};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::{Arc, atomic::Ordering};
use uuid::Uuid;

pub(crate) struct OpenedSource {
    pub source_id: String,
    pub segments: Vec<Segment>,
    lease: File,
}

pub(crate) fn regular(path: &Path) -> io::Result<fs::Metadata> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() || is_reparse(&metadata) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "The controlled log entry is not a regular, unlinked file.",
        ));
    }
    #[cfg(windows)]
    if alcomd_platform::file_link_count(path)? != 1 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "The controlled log file has unexpected hard links.",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "The controlled log file has unexpected hard links.",
            ));
        }
    }
    Ok(metadata)
}

fn is_reparse(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        let _ = metadata;
        false
    }
}

fn create(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).read(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

pub(crate) fn open_source(directory: &Path, source_kind: &str) -> io::Result<OpenedSource> {
    if !directory.is_absolute()
        || source_kind.is_empty()
        || source_kind.len() > 64
        || !source_kind
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid controlled log source.",
        ));
    }
    fs::create_dir_all(directory)?;
    let metadata = fs::symlink_metadata(directory)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() || is_reparse(&metadata) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "The log directory must not be a link.",
        ));
    }
    let lease_path = directory.join("writer.lock");
    let lease = match create(&lease_path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            regular(&lease_path)?;
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(&lease_path)?
        }
        Err(error) => return Err(error),
    };
    lease.try_lock().map_err(|_| {
        io::Error::new(
            io::ErrorKind::WouldBlock,
            "The log source is already owned by another writer.",
        )
    })?;
    let source_path = directory.join("source-id");
    let source_id = match create(&source_path) {
        Ok(mut file) => {
            let id = Uuid::new_v4().to_string();
            file.write_all(id.as_bytes())?;
            file.flush()?;
            id
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let metadata = regular(&source_path)?;
            if metadata.len() != 36 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid log source identity.",
                ));
            }
            let mut id = String::new();
            File::open(&source_path)?.take(37).read_to_string(&mut id)?;
            if Uuid::parse_str(&id).is_err() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Invalid log source identity.",
                ));
            }
            id
        }
        Err(error) => return Err(error),
    };
    let mut segments = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some((ordinal, session, id)) = parse_name(&name) else {
            continue;
        };
        let path = entry.path();
        let metadata = regular(&path)?;
        if metadata.len() > FILE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "A log segment exceeds its size bound.",
            ));
        }
        segments.push(Segment {
            id,
            session,
            ordinal,
            identity: alcomd_platform::file_identity_key(&path)?,
            path,
            readable: metadata.len(),
            sealed: true,
        });
    }
    segments.sort_by_key(|segment| segment.ordinal);
    Ok(OpenedSource {
        source_id,
        segments,
        lease,
    })
}

fn parse_name(name: &str) -> Option<(u64, String, String)> {
    let suffix = name.strip_prefix("segment-")?.strip_suffix(".jsonl")?;
    if suffix.len() != 94 {
        return None;
    }
    let ordinal = suffix.get(..20)?.parse().ok()?;
    if suffix.get(20..21)? != "-" || suffix.get(57..58)? != "-" {
        return None;
    }
    let session = suffix.get(21..57)?;
    let id = suffix.get(58..94)?;
    Uuid::parse_str(session).ok()?;
    Uuid::parse_str(id).ok()?;
    Some((ordinal, session.to_owned(), id.to_owned()))
}

pub(crate) struct SegmentWriter {
    shared: Arc<Shared>,
    _lease: File,
    current: File,
    current_id: String,
    bytes: u64,
    poisoned: bool,
}

impl SegmentWriter {
    pub fn new(shared: Arc<Shared>, opened: OpenedSource) -> io::Result<Self> {
        let (current, id) = allocate(&shared)?;
        Ok(Self {
            shared,
            _lease: opened.lease,
            current,
            current_id: id,
            bytes: 0,
            poisoned: false,
        })
    }

    fn rotate(&mut self) -> io::Result<()> {
        #[cfg(test)]
        if self.shared.test.fail_rotation.load(Ordering::Acquire) {
            return Err(io::Error::other("injected rotation failure"));
        }
        self.flush()?;
        {
            if let Some(old) = lock(&self.shared.catalog)
                .iter_mut()
                .find(|segment| segment.id == self.current_id)
            {
                old.sealed = true;
            }
        }
        let (file, id) = allocate(&self.shared)?;
        self.current = file;
        self.current_id = id;
        self.bytes = 0;
        self.poisoned = false;
        Ok(())
    }
}

fn allocate(shared: &Shared) -> io::Result<(File, String)> {
    // This mutex never covers network, ACKs, or frontend work.
    let mut catalog = lock(&shared.catalog).clone();
    let ordinal = catalog.last().map_or(Ok(1), |segment| {
        segment
            .ordinal
            .checked_add(1)
            .ok_or_else(|| io::Error::other("Log segment ordinal exhausted."))
    })?;
    while catalog.len() >= FILE_COUNT {
        if !catalog[0].sealed {
            return Err(io::Error::other(
                "No sealed log segment can be retained out.",
            ));
        }
        let oldest = &catalog[0];
        regular(&oldest.path)?;
        if alcomd_platform::file_identity_key(&oldest.path)? != oldest.identity {
            return Err(io::Error::other("The old log segment identity changed."));
        }
        fs::remove_file(&oldest.path)?;
        lock(&shared.catalog).retain(|segment| segment.id != oldest.id);
        catalog.remove(0);
    }
    let id = Uuid::new_v4().to_string();
    let path = shared.directory.join(format!(
        "segment-{ordinal:020}-{}-{id}.jsonl",
        shared.session_id
    ));
    let file = create(&path)?;
    let identity = alcomd_platform::file_identity_key(&path)?;
    lock(&shared.catalog).push(Segment {
        id: id.clone(),
        session: shared.session_id.clone(),
        ordinal,
        path,
        identity,
        readable: 0,
        sealed: false,
    });
    Ok((file, id))
}

impl Write for SegmentWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        #[cfg(test)]
        {
            self.shared.test.write_started.notify_one();
            let paused = lock(&self.shared.test.paused);
            let _paused = self
                .shared
                .test
                .resume
                .wait_while(paused, |paused| *paused)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if self.shared.test.fail_write.load(Ordering::Acquire) {
                self.shared.processed.fetch_add(1, Ordering::AcqRel);
                self.shared
                    .failure("write_failed", "injected write failure");
                return Err(io::Error::other("injected write failure"));
            }
        }
        #[cfg(test)]
        let forced = self
            .shared
            .test
            .force_rotation
            .swap(false, Ordering::AcqRel);
        #[cfg(not(test))]
        let forced = false;
        if (forced || self.poisoned || self.bytes.saturating_add(bytes.len() as u64) > FILE_BYTES)
            && let Err(error) = self.rotate()
        {
            self.shared.processed.fetch_add(1, Ordering::AcqRel);
            self.shared.failure("rotation_failed", error.to_string());
            return Err(error);
        }
        let result = self.current.write_all(bytes);
        self.shared.processed.fetch_add(1, Ordering::AcqRel);
        match result {
            Ok(()) => {
                self.bytes += bytes.len() as u64;
                // File has no userspace buffering. Complete bytes are now readable,
                // although power-loss durability is deliberately not promised.
                if let Some(segment) = lock(&self.shared.catalog)
                    .iter_mut()
                    .find(|segment| segment.id == self.current_id)
                {
                    segment.readable = self.bytes;
                }
                self.shared.changed.notify_waiters();
                Ok(bytes.len())
            }
            Err(error) => {
                self.poisoned = true;
                self.shared.failure("write_failed", error.to_string());
                Err(error)
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        #[cfg(test)]
        if self.shared.test.fail_flush.load(Ordering::Acquire) {
            self.shared.flush_succeeded.store(false, Ordering::Release);
            self.shared
                .failure("flush_failed", "injected flush failure");
            return Err(io::Error::other("injected flush failure"));
        }
        match self.current.flush() {
            Ok(()) => {
                self.shared.flush_succeeded.store(true, Ordering::Release);
                self.shared.flushed.store(
                    self.shared.processed.load(Ordering::Acquire),
                    Ordering::Release,
                );
                Ok(())
            }
            Err(error) => {
                self.shared.flush_succeeded.store(false, Ordering::Release);
                self.shared.failure("flush_failed", error.to_string());
                Err(error)
            }
        }
    }
}

impl Drop for SegmentWriter {
    fn drop(&mut self) {
        // Release writer ownership before acknowledging actual worker completion.
        let _ = self._lease.unlock();
        let (closed, changed) = &self.shared.worker_closed;
        *lock(closed) = true;
        changed.notify_all();
    }
}

pub(crate) fn read_segment(
    segment: &Segment,
    start: u64,
    length: usize,
) -> Result<Vec<u8>, crate::LogError> {
    use std::io::{Seek, SeekFrom};
    let metadata = regular(&segment.path).map_err(|_| crate::LogError::Unavailable)?;
    if alcomd_platform::file_identity_key(&segment.path)
        .map_err(|_| crate::LogError::Unavailable)?
        != segment.identity
    {
        return Err(crate::LogError::Unavailable);
    }
    if metadata.len() < segment.readable {
        return Err(crate::LogError::CursorExpired);
    }
    let mut file = File::open(&segment.path).map_err(|_| crate::LogError::Unavailable)?;
    file.seek(SeekFrom::Start(start))
        .map_err(|_| crate::LogError::Unavailable)?;
    let mut bytes = Vec::with_capacity(length);
    file.take(length as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| crate::LogError::Unavailable)?;
    Ok(bytes)
}
