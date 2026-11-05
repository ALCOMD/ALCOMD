//! Process-local structured log storage. Business state and RPC authorization live elsewhere.
//!
//! Standard tracing-subscriber JSON is accepted unchanged. Only complete, readable
//! lines receive positions; health changes have their own wakeup path.

mod reader;
mod types;
mod writer;

pub use types::*;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, Instant};
use tokio::sync::{Notify, Semaphore};
use tracing_appender::non_blocking::{ErrorCounter, NonBlocking, WorkerGuard};
use tracing_subscriber::fmt::MakeWriter;
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct Segment {
    pub id: String,
    pub session: String,
    pub ordinal: u64,
    pub path: PathBuf,
    pub identity: Vec<u8>,
    pub readable: u64,
    pub sealed: bool,
}

pub(crate) struct Shared {
    directory: PathBuf,
    source_id: String,
    session_id: String,
    health: Mutex<LogHealth>,
    catalog: Mutex<Vec<Segment>>,
    queue: Mutex<Option<(NonBlocking, ErrorCounter)>>,
    changed: Notify,
    health_changed: Notify,
    reader: Arc<Semaphore>,
    closing: AtomicBool,
    submitted: AtomicU64,
    processed: AtomicU64,
    flushed: AtomicU64,
    flush_succeeded: AtomicBool,
    worker_closed: (Mutex<bool>, Condvar),
    #[cfg(test)]
    test: TestControl,
}

#[cfg(test)]
#[derive(Default)]
struct TestControl {
    paused: Mutex<bool>,
    resume: Condvar,
    write_started: Notify,
    fail_write: AtomicBool,
    fail_rotation: AtomicBool,
    fail_flush: AtomicBool,
    force_rotation: AtomicBool,
    wait_gate: Mutex<Option<(Arc<tokio::sync::Barrier>, Arc<tokio::sync::Barrier>)>>,
}

pub(crate) fn lock<T>(value: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    value
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Shared {
    pub(crate) fn observe_read_issues(&self, issues: &[ReadIssue]) {
        if issues.is_empty() {
            return;
        }
        let mut health = lock(&self.health);
        if health.has_gap {
            return;
        }
        health.has_gap = true;
        health.state = "degraded".to_owned();
        health.revision = health.revision.saturating_add(1);
        health.last_failure = Some(LogFailure {
            kind: "retained_data_damaged".to_owned(),
            message: "Some retained log records could not be read completely.".to_owned(),
        });
        drop(health);
        self.changed.notify_waiters();
        self.health_changed.notify_waiters();
    }

    pub(crate) fn failure(&self, kind: &str, message: impl Into<String>) {
        let mut health = lock(&self.health);
        match kind {
            "oversize_rejected" => {
                health.oversize_rejected = health.oversize_rejected.saturating_add(1)
            }
            "write_failed" => health.write_failed = health.write_failed.saturating_add(1),
            "rotation_failed" => health.rotation_failed = health.rotation_failed.saturating_add(1),
            "flush_failed" => health.flush_failed = health.flush_failed.saturating_add(1),
            _ => {}
        }
        health.state = "degraded".to_owned();
        health.has_gap = true;
        health.revision = health.revision.saturating_add(1);
        health.last_failure = Some(LogFailure {
            kind: kind.to_owned(),
            message: message.into().chars().take(4096).collect(),
        });
        drop(health);
        self.changed.notify_waiters();
        self.health_changed.notify_waiters();
    }

    fn observe_drops(&self, count: u64) {
        let mut health = lock(&self.health);
        if count <= health.queue_dropped {
            return;
        }
        health.queue_dropped = count;
        health.state = "degraded".to_owned();
        health.has_gap = true;
        health.revision = health.revision.saturating_add(1);
        health.last_failure = Some(LogFailure {
            kind: "queue_dropped".to_owned(),
            message: "The bounded logging queue rejected records.".to_owned(),
        });
        drop(health);
        self.changed.notify_waiters();
        self.health_changed.notify_waiters();
    }
}

/// Retain this guard until business producers have stopped.
pub struct LoggingRuntime {
    source: LogSource,
    guard: Option<WorkerGuard>,
}

impl LoggingRuntime {
    /// Opens only this controlled source. Initialization failures remain queryable.
    pub fn start(directory: impl AsRef<Path>, source_kind: &str) -> Self {
        let source_id = Uuid::new_v4().to_string();
        let session_id = Uuid::new_v4().to_string();
        let mut shared = Shared {
            directory: directory.as_ref().to_path_buf(),
            source_id: source_id.clone(),
            session_id: session_id.clone(),
            health: Mutex::new(LogHealth {
                source_id,
                session_id,
                revision: 1,
                state: "ready".to_owned(),
                queue_dropped: 0,
                oversize_rejected: 0,
                write_failed: 0,
                rotation_failed: 0,
                flush_failed: 0,
                has_gap: false,
                last_failure: None,
            }),
            catalog: Mutex::new(Vec::new()),
            queue: Mutex::new(None),
            changed: Notify::new(),
            health_changed: Notify::new(),
            reader: Arc::new(Semaphore::new(1)),
            closing: AtomicBool::new(false),
            submitted: AtomicU64::new(0),
            processed: AtomicU64::new(0),
            flushed: AtomicU64::new(0),
            flush_succeeded: AtomicBool::new(false),
            worker_closed: (Mutex::new(false), Condvar::new()),
            #[cfg(test)]
            test: TestControl::default(),
        };
        // The lease and source identity are established before any retention action.
        let opened = writer::open_source(&shared.directory, source_kind);
        if let Ok(opened) = &opened {
            shared.source_id.clone_from(&opened.source_id);
            lock(&shared.health).source_id.clone_from(&opened.source_id);
        }
        let shared = Arc::new(shared);
        let source = LogSource(shared.clone());
        let guard = match opened {
            Ok(opened) => {
                *lock(&shared.catalog) = opened.segments.clone();
                match writer::SegmentWriter::new(shared.clone(), opened) {
                    Ok(writer) => {
                        let (queue, guard) =
                            tracing_appender::non_blocking::NonBlockingBuilder::default()
                                .buffered_lines_limit(QUEUE_RECORDS)
                                .lossy(true)
                                .thread_name("alcomd-log-writer")
                                .finish(writer);
                        let counter = queue.error_counter();
                        *lock(&shared.queue) = Some((queue, counter));
                        Some(guard)
                    }
                    Err(error) => {
                        shared.failure("rotation_failed", error.to_string());
                        None
                    }
                }
            }
            Err(error) => {
                shared.failure("source_unavailable", error.to_string());
                None
            }
        };
        Self { source, guard }
    }

    pub fn source(&self) -> LogSource {
        self.source.clone()
    }

    /// Bounded shutdown; blocking work is isolated on a dedicated close thread.
    pub fn shutdown(mut self) -> ShutdownReport {
        self.close()
    }

    fn close(&mut self) -> ShutdownReport {
        let deadline = Instant::now() + Duration::from_secs(2);
        {
            let _queue = lock(&self.source.0.queue);
            self.source.0.closing.store(true, Ordering::Release);
        }
        let mut timed_out = false;
        if let Some(guard) = self.guard.take() {
            let (send, recv) = std::sync::mpsc::sync_channel(1);
            match std::thread::Builder::new()
                .name("alcomd-log-close".to_owned())
                .spawn(move || {
                    drop(guard);
                    let _ = send.send(());
                }) {
                Ok(_) => {
                    timed_out = recv
                        .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                        .is_err();
                    if !timed_out {
                        let (closed, changed) = &self.source.0.worker_closed;
                        let closed = lock(closed);
                        let result = changed.wait_timeout_while(
                            closed,
                            deadline.saturating_duration_since(Instant::now()),
                            |closed| !*closed,
                        );
                        timed_out = result.map_or(true, |(_, result)| result.timed_out());
                    }
                }
                Err(error) => {
                    self.source.0.failure("flush_failed", error.to_string());
                    timed_out = true;
                }
            }
        }
        let submitted = self.source.0.submitted.load(Ordering::Acquire);
        let processed = self.source.0.processed.load(Ordering::Acquire);
        let drops = self.source.health().queue_dropped;
        let drained = processed.saturating_add(drops) >= submitted;
        let flushed = drained
            && !timed_out
            && *lock(&self.source.0.worker_closed.0)
            && self.source.0.flush_succeeded.load(Ordering::Acquire)
            && self.source.0.flushed.load(Ordering::Acquire) >= processed;
        if !drained || !flushed || timed_out {
            self.source.0.failure(
                "flush_failed",
                "Logging shutdown did not confirm a complete drain and flush.",
            );
            timed_out |= !drained;
        }
        {
            let mut health = lock(&self.source.0.health);
            health.state = "closed".to_owned();
            health.revision = health.revision.saturating_add(1);
        }
        self.source.0.changed.notify_waiters();
        self.source.0.health_changed.notify_waiters();
        ShutdownReport {
            drained,
            flushed,
            timed_out,
            health: self.source.health(),
        }
    }
}

impl Drop for LoggingRuntime {
    fn drop(&mut self) {
        if self.guard.is_some() {
            let _ = self.close();
        }
    }
}

#[derive(Clone)]
pub struct LogSource(Arc<Shared>);

impl LogSource {
    pub fn source_id(&self) -> &str {
        &self.0.source_id
    }
    pub fn session_id(&self) -> &str {
        &self.0.session_id
    }
    pub fn writer(&self) -> LogMakeWriter {
        LogMakeWriter(self.clone())
    }
    pub fn health(&self) -> LogHealth {
        lock(&self.0.health).clone()
    }

    pub async fn read(
        &self,
        cursor: Option<String>,
        limit: u32,
        follow: bool,
        query: LogQuery,
    ) -> Result<ReadPage, LogError> {
        reader::validate(limit, &query)?;
        loop {
            // Register first; notify_waiters otherwise leaves a check/wait gap.
            let changed = self.0.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let health_changed = self.0.health_changed.notified();
            tokio::pin!(health_changed);
            health_changed.as_mut().enable();
            if cursor.is_none() {
                return reader::read(&self.0, None, limit, &query);
            }
            if let Some(page) = reader::health_page(
                &self.0,
                cursor.as_deref().ok_or(LogError::InvalidInput)?,
                &query,
            )? {
                return Ok(page);
            }
            let permit = tokio::select! {
                value = self.0.reader.clone().acquire_owned() => value.map_err(|_| LogError::Unavailable)?,
                () = &mut health_changed => continue,
            };
            let source = self.clone();
            let position = cursor.clone();
            let filter = query.clone();
            let task = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                reader::read(&source.0, position.as_deref(), limit, &filter)
            });
            let page = tokio::select! {
                value = task => value.map_err(|_| LogError::Unavailable)??,
                () = &mut health_changed => continue,
            };
            if !follow
                || page.baseline
                || Some(&page.next_cursor) != cursor.as_ref()
                || page.has_more
                || page.health.state == "closed"
            {
                return Ok(page);
            }
            #[cfg(test)]
            {
                let gate = lock(&self.0.test.wait_gate).take();
                if let Some((ready, release)) = gate {
                    ready.wait().await;
                    release.wait().await;
                }
            }
            changed.await;
        }
    }

    pub async fn history(
        &self,
        cursor: Option<String>,
        before: Option<String>,
        limit: u32,
        query: LogQuery,
    ) -> Result<HistoryPage, LogError> {
        reader::validate(limit, &query)?;
        let permit = self
            .0
            .reader
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| LogError::Unavailable)?;
        let source = self.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            reader::history(
                &source.0,
                cursor.as_deref(),
                before.as_deref(),
                limit,
                &query,
            )
        })
        .await
        .map_err(|_| LogError::Unavailable)?
    }
}

#[derive(Clone)]
pub struct LogMakeWriter(LogSource);

pub struct EventWriter {
    source: LogSource,
    bytes: Vec<u8>,
    oversized: bool,
}

impl<'a> MakeWriter<'a> for LogMakeWriter {
    type Writer = EventWriter;
    fn make_writer(&'a self) -> Self::Writer {
        EventWriter {
            source: self.0.clone(),
            bytes: Vec::new(),
            oversized: false,
        }
    }
}

impl Write for EventWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len()) > RECORD_BYTES {
            self.oversized = true;
            self.bytes.clear();
        } else if !self.oversized {
            self.bytes.extend_from_slice(bytes);
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for EventWriter {
    fn drop(&mut self) {
        let shared = &self.source.0;
        if self.oversized {
            shared.failure(
                "oversize_rejected",
                "A complete encoded log record exceeded 512KiB.",
            );
            return;
        }
        if self.bytes.is_empty() || shared.closing.load(Ordering::Acquire) {
            return;
        }
        if !self.bytes.ends_with(b"\n")
            || self.bytes[..self.bytes.len() - 1].contains(&b'\n')
            || serde_json::from_slice::<serde_json::Value>(&self.bytes).is_err()
        {
            shared.failure(
                "write_failed",
                "The formatter did not produce one complete JSON line.",
            );
            return;
        }
        let mut queue = lock(&shared.queue);
        if shared.closing.load(Ordering::Acquire) {
            return;
        }
        if let Some((queue, counter)) = queue.as_mut() {
            shared.submitted.fetch_add(1, Ordering::AcqRel);
            let _ = queue.write_all(&self.bytes);
            shared.observe_drops(counter.dropped_lines() as u64);
        } else {
            shared.failure("write_failed", "The log source has no writable backend.");
        }
    }
}

/// Waits for accepted records without polling business data. Useful before taking snapshots.
#[cfg(test)]
fn wait_for_drain(source: &LogSource, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if source
            .0
            .processed
            .load(Ordering::Acquire)
            .saturating_add(source.health().queue_dropped)
            >= source.0.submitted.load(Ordering::Acquire)
        {
            return true;
        }
        std::thread::yield_now();
    }
    false
}

#[cfg(test)]
mod tests;
