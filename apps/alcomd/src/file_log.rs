//! One bounded, nonblocking queue from accepted Core logs to rotating UTF-8 files.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
    mpsc,
};
use std::thread::{self, JoinHandle};

use alcomd_application::{DiagnosticLog, DiagnosticLogItem, DiagnosticLogLevel, DiagnosticLogSink};

const FILE_BYTES: u64 = 5 * 1024 * 1024;
const FILES: usize = 5;
const QUEUE: usize = 1024;

enum Message {
    Entry(DiagnosticLogItem),
    Finish,
}
struct Queue {
    sender: mpsc::SyncSender<Message>,
    dropped: Arc<AtomicU64>,
}
impl DiagnosticLogSink for Queue {
    fn accept(&self, item: &DiagnosticLogItem) {
        // A file failure must remain visible in memory without recursively trying
        // to write its own failure to the same broken file.
        if item.code.starts_with("diagnostic.log_file_") {
            return;
        }
        if self.sender.try_send(Message::Entry(item.clone())).is_err() {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub(crate) struct FileLog {
    sender: mpsc::SyncSender<Message>,
    thread: Option<JoinHandle<()>>,
}
impl FileLog {
    pub(crate) fn start(log: DiagnosticLog, directory: &Path) -> io::Result<Self> {
        let writer = RotatingFile::open(directory, FILE_BYTES, FILES)?;
        let (sender, receiver) = mpsc::sync_channel(QUEUE);
        let dropped = Arc::new(AtomicU64::new(0));
        let queue = Arc::new(Queue {
            sender: sender.clone(),
            dropped: dropped.clone(),
        });
        let worker_log = log.clone();
        let thread = thread::Builder::new()
            .name("technical-log".to_owned())
            .spawn(move || {
                run(receiver, writer, &worker_log, &dropped);
            })?;
        log.attach_sink(queue);
        Ok(Self {
            sender,
            thread: Some(thread),
        })
    }
}
impl Drop for FileLog {
    fn drop(&mut self) {
        let _ = self.sender.send(Message::Finish);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn file_error(log: &DiagnosticLog, message: &str) {
    eprintln!("{message}");
    log.record(
        DiagnosticLogLevel::Error,
        Some("diagnostic.log_file_failed"),
        None,
        None,
        message,
        "alcomd::logging",
    );
}

fn run(
    receiver: mpsc::Receiver<Message>,
    mut writer: RotatingFile,
    log: &DiagnosticLog,
    dropped: &AtomicU64,
) {
    let mut failed = false;
    let mut overflow_reported = false;
    while let Ok(message) = receiver.recv() {
        let skipped = dropped.swap(0, Ordering::Relaxed);
        if skipped > 0 && !overflow_reported {
            overflow_reported = true;
            log.record(DiagnosticLogLevel::Warning, Some("diagnostic.log_file_overflow"), None, Some(true),
            &format!("The technical log file writer fell behind. At least {skipped} records could not be written to the file; records remain subject to the bounded memory history."), "alcomd::logging");
        }
        match message {
            Message::Entry(entry) if !failed => {
                if let Err(error) = writer.append(&entry) {
                    failed = true;
                    file_error(
                        log,
                        &format!(
                            "Could not write the technical log file: {error}. New records remain available in memory."
                        ),
                    );
                }
            }
            Message::Entry(_) => {}
            Message::Finish => break,
        }
    }
    if !failed
        && let Some(file) = writer.file.as_mut()
        && let Err(error) = file.flush()
    {
        file_error(
            log,
            &format!("Could not flush the technical log file: {error}."),
        );
    }
}

struct RotatingFile {
    directory: PathBuf,
    file: Option<File>,
    bytes: u64,
    limit: u64,
    files: usize,
}
impl RotatingFile {
    fn path(&self, index: usize) -> PathBuf {
        let suffix = if index == 0 {
            String::new()
        } else {
            format!(".{index}")
        };
        self.directory.join(format!(
            "{}-technical{suffix}.log",
            alcomd_protocol::PRODUCT_FAMILY.to_ascii_lowercase()
        ))
    }
    fn open(directory: &Path, limit: u64, files: usize) -> io::Result<Self> {
        fs::create_dir_all(directory)?;
        if fs::symlink_metadata(directory)?.file_type().is_symlink() {
            return Err(io::Error::other("Log directory must not be a link"));
        }
        let mut writer = Self {
            directory: directory.canonicalize()?,
            file: None,
            bytes: 0,
            limit,
            files,
        };
        writer.check_files()?;
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(writer.path(0))?;
        writer.bytes = file.metadata()?.len();
        writer.file = Some(file);
        Ok(writer)
    }
    fn check_files(&self) -> io::Result<()> {
        for index in 0..self.files {
            match fs::symlink_metadata(self.path(index)) {
                Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
                    return Err(io::Error::other("Log file is not a regular file"));
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
                _ => {}
            }
        }
        Ok(())
    }
    fn rotate(&mut self) -> io::Result<()> {
        self.check_files()?;
        self.file.take();
        for index in (1..self.files).rev() {
            let source = self.path(index - 1);
            let destination = self.path(index);
            if source.exists() {
                if destination.exists() {
                    fs::remove_file(&destination)?;
                }
                fs::rename(source, destination)?;
            }
        }
        self.file = Some(
            OpenOptions::new()
                .create(true)
                .truncate(true)
                .write(true)
                .open(self.path(0))?,
        );
        self.bytes = 0;
        Ok(())
    }
    fn append(&mut self, entry: &DiagnosticLogItem) -> io::Result<()> {
        let mut bytes = serde_json::to_vec(&serde_json::json!({
            "instanceId": entry.instance_id, "sequence":entry.sequence, "occurredAtMs":entry.occurred_at_ms,
            "level":entry.level.as_str(), "target":entry.target, "message":entry.message,
            "code":entry.code, "diagnosticId":entry.diagnostic_id, "notify":entry.notify
        }))?;
        bytes.push(b'\n');
        if bytes.len() as u64 > self.limit {
            return Err(io::Error::other(
                "One log record exceeds the file size limit",
            ));
        }
        if self.bytes + bytes.len() as u64 > self.limit {
            self.rotate()?;
        }
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::other("Log file is closed"))?
            .write_all(&bytes)?;
        self.bytes += bytes.len() as u64;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alcomd_application::{AccessContext, OperationId};
    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            let path = std::env::temp_dir()
                .canonicalize()
                .expect("temporary root")
                .join(format!("alcomd-file-log-{}", OperationId::new()));
            fs::create_dir(&path).expect("isolated directory");
            Self(path)
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            let path = self.0.canonicalize().expect("fixture");
            assert_eq!(
                path.parent(),
                Some(
                    std::env::temp_dir()
                        .canonicalize()
                        .expect("temporary root")
                        .as_path()
                )
            );
            fs::remove_dir_all(path).expect("remove only isolated fixture");
        }
    }
    fn entry(log: &DiagnosticLog) -> DiagnosticLogItem {
        log.record(
            DiagnosticLogLevel::Info,
            Some("test.ready"),
            None,
            None,
            "已读取 Unity 安装。\npath=C:\\Unity 项目\\测试 🦀",
            "alcomd::unity",
        )
    }
    #[test]
    fn one_accepted_record_reaches_utf8_file_and_restart_appends_without_erasing_history() {
        let directory = Directory::new();
        for _ in 0..2 {
            let log = DiagnosticLog::new();
            let guard = FileLog::start(log.clone(), &directory.0).expect("writer");
            let item = entry(&log);
            drop(guard);
            let path = directory.0.join(format!(
                "{}-technical.log",
                alcomd_protocol::PRODUCT_FAMILY.to_ascii_lowercase()
            ));
            let text = fs::read_to_string(path).expect("UTF-8");
            let value: serde_json::Value =
                serde_json::from_str(text.lines().last().expect("entry")).expect("complete record");
            assert_eq!(value["message"], item.message);
            assert_eq!(value["sequence"], item.sequence);
            assert_eq!(value["instanceId"], item.instance_id);
            assert_eq!(value["target"], item.target);
        }
        let file = fs::read_dir(&directory.0)
            .expect("files")
            .next()
            .expect("one file")
            .expect("entry")
            .path();
        assert_eq!(
            fs::read_to_string(file).expect("history").lines().count(),
            2
        );
    }
    #[test]
    fn rotation_bounds_files_and_bytes_and_preserves_complete_utf8_records_and_foreign_files() {
        let directory = Directory::new();
        let foreign = directory.0.join("owner-notes.txt");
        fs::write(&foreign, "preserve").expect("foreign file");
        let log = DiagnosticLog::new();
        let mut item = entry(&log);
        let mut writer = RotatingFile::open(&directory.0, 1024, 3).expect("writer");
        for sequence in 1..=100 {
            item.sequence = sequence;
            writer.append(&item).expect("write/rotate");
        }
        drop(writer);
        let files = fs::read_dir(&directory.0)
            .expect("files")
            .map(|entry| entry.expect("file").path())
            .filter(|path| path.extension().is_some_and(|value| value == "log"))
            .collect::<Vec<_>>();
        assert_eq!(files.len(), 3);
        for path in files {
            assert!(fs::metadata(&path).expect("size").len() <= 1024);
            for line in fs::read_to_string(path).expect("UTF-8").lines() {
                let value: serde_json::Value = serde_json::from_str(line).expect("unbroken record");
                assert_eq!(value["message"], item.message);
            }
        }
        assert_eq!(fs::read_to_string(foreign).expect("preserved"), "preserve");
    }
    #[tokio::test]
    async fn write_failure_reports_once_in_memory_without_recursive_file_attempts() {
        let directory = Directory::new();
        let log = DiagnosticLog::new();
        let item = entry(&log);
        let writer = RotatingFile::open(&directory.0, 1, 3).expect("writer");
        let (sender, receiver) = mpsc::sync_channel(4);
        sender.send(Message::Entry(item.clone())).expect("first");
        sender.send(Message::Entry(item)).expect("second");
        sender.send(Message::Finish).expect("finish");
        run(receiver, writer, &log, &AtomicU64::new(0));
        let page = log
            .read(&AccessContext::local_owner(), None, None, None)
            .await
            .expect("memory log");
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[1].code, "diagnostic.log_file_failed");
        assert!(
            page.items[1]
                .message
                .contains("exceeds the file size limit")
        );
        assert!(page.items[1].notify);
    }
    #[test]
    fn a_full_file_queue_never_waits_and_internal_failure_does_not_enqueue_itself() {
        let log = DiagnosticLog::new();
        let mut item = entry(&log);
        let (sender, receiver) = mpsc::sync_channel(1);
        let dropped = Arc::new(AtomicU64::new(0));
        let queue = Queue {
            sender,
            dropped: dropped.clone(),
        };
        queue.accept(&item);
        queue.accept(&item);
        queue.accept(&item);
        assert_eq!(dropped.load(Ordering::Relaxed), 2);
        item.code = "diagnostic.log_file_failed".to_owned();
        queue.accept(&item);
        assert_eq!(dropped.load(Ordering::Relaxed), 2);
        assert!(matches!(receiver.try_recv(), Ok(Message::Entry(_))));
        assert!(receiver.try_recv().is_err());
    }
}
