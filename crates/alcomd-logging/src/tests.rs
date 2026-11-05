use super::*;
use serde_json::json;
use std::fs;
use tracing_subscriber::prelude::*;

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let parent = std::env::temp_dir().canonicalize().expect("temporary root");
        let path = parent.join(format!("alcomd-new-logs-{}", Uuid::new_v4()));
        fs::create_dir(&path).expect("isolated source");
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let parent = std::env::temp_dir().canonicalize().expect("temporary root");
        if self.0.parent() == Some(parent.as_path())
            && self
                .0
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("alcomd-new-logs-"))
        {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

fn line(message: &str) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&json!({"timestamp":"2026-11-05T00:00:00Z","level":"INFO","target":"alcomd_test","fields":{"message":message}})).expect("JSON");
    bytes.push(b'\n');
    bytes
}
fn emit(source: &LogSource, message: &str) {
    let mut writer = source.writer().make_writer();
    writer.write_all(&line(message)).expect("producer");
}
fn settle(source: &LogSource) {
    assert!(
        wait_for_drain(source, Duration::from_secs(3)),
        "worker drained"
    );
    let deadline = Instant::now() + Duration::from_secs(3);
    while source.0.flushed.load(Ordering::Acquire) < source.0.processed.load(Ordering::Acquire)
        && Instant::now() < deadline
    {
        std::thread::yield_now();
    }
}

#[tokio::test]
async fn standard_formatter_preserves_path_fields_and_does_not_capture_skipped_secrets() {
    let directory = Directory::new();
    let runtime = LoggingRuntime::start(&directory.0, "core");
    let source = runtime.source();
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .json()
            .with_current_span(false)
            .with_span_list(true)
            .with_writer(source.writer()),
    );
    let path = "C:\\Users\\测试\\项目 A\\Packages\\vpm-manifest.json";
    #[tracing::instrument(skip_all, fields(path = %path))]
    fn fail(path: &str, secret: &str) {
        let _ = secret;
        tracing::error!(
            error = "invalid manifest\ncaused by: invalid version",
            "failed to inspect project"
        );
    }
    tracing::subscriber::with_default(subscriber, || fail(path, "DO-NOT-LOG-TOKEN"));
    settle(&source);
    let page = source
        .history(None, None, 100, LogQuery::default())
        .await
        .expect("history");
    assert_eq!(page.items.len(), 1);
    let record = &page.items[0].record;
    assert_eq!(record["level"], "ERROR");
    assert_eq!(record["fields"]["message"], "failed to inspect project");
    assert_eq!(record["spans"][0]["path"], path);
    assert!(!record.to_string().contains("DO-NOT-LOG-TOKEN"));
    let report = runtime.shutdown();
    assert!(report.drained && report.flushed && !report.timed_out);
}

#[tokio::test]
async fn live_and_history_share_identity_and_the_baseline_has_no_boundary_gap() {
    let directory = Directory::new();
    let runtime = LoggingRuntime::start(&directory.0, "core");
    let source = runtime.source();
    emit(&source, "before");
    settle(&source);
    let baseline = source
        .read(None, 100, false, LogQuery::default())
        .await
        .expect("baseline");
    emit(&source, "after");
    settle(&source);
    let history = source
        .history(
            None,
            Some(baseline.next_cursor.clone()),
            100,
            LogQuery::default(),
        )
        .await
        .expect("snapshot history");
    let live = source
        .read(Some(baseline.next_cursor), 100, false, LogQuery::default())
        .await
        .expect("new records");
    assert_eq!(history.items.len(), 1);
    assert_eq!(live.items.len(), 1);
    assert_eq!(history.items[0].record["fields"]["message"], "before");
    assert_eq!(live.items[0].record["fields"]["message"], "after");
    let all = source
        .history(None, None, 100, LogQuery::default())
        .await
        .expect("same source");
    assert_eq!(all.items[0], live.items[0]);
    assert_eq!(all.items[1], history.items[0]);
    runtime.shutdown();
}

#[tokio::test]
async fn idle_wait_does_not_return_empty_and_registration_precedes_the_check_wait_gap() {
    let directory = Directory::new();
    let runtime = LoggingRuntime::start(&directory.0, "core");
    let source = runtime.source();
    let baseline = source
        .read(None, 100, false, LogQuery::default())
        .await
        .expect("baseline");
    assert!(
        tokio::time::timeout(
            Duration::from_millis(30),
            source.read(
                Some(baseline.next_cursor.clone()),
                100,
                true,
                LogQuery::default()
            )
        )
        .await
        .is_err()
    );
    let ready = Arc::new(tokio::sync::Barrier::new(2));
    let release = Arc::new(tokio::sync::Barrier::new(2));
    *lock(&source.0.test.wait_gate) = Some((ready.clone(), release.clone()));
    let clone = source.clone();
    let task = tokio::spawn(async move {
        clone
            .read(Some(baseline.next_cursor), 100, true, LogQuery::default())
            .await
    });
    ready.wait().await;
    emit(&source, "committed between check and wait");
    settle(&source);
    release.wait().await;
    let page = tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .expect("no missed wakeup")
        .expect("task")
        .expect("page");
    assert_eq!(page.items.len(), 1);
    runtime.shutdown();
}

#[tokio::test]
async fn enqueue_is_not_readability_and_full_queue_is_bounded_and_nonblocking() {
    let directory = Directory::new();
    let runtime = LoggingRuntime::start(&directory.0, "core");
    let source = runtime.source();
    *lock(&source.0.test.paused) = true;
    let baseline = source
        .read(None, 100, false, LogQuery::default())
        .await
        .expect("baseline");
    let started = source.0.test.write_started.notified();
    tokio::pin!(started);
    started.as_mut().enable();
    emit(&source, "not yet readable");
    started.await;
    assert!(
        tokio::time::timeout(
            Duration::from_millis(30),
            source.read(
                Some(baseline.next_cursor.clone()),
                100,
                true,
                LogQuery::default()
            )
        )
        .await
        .is_err()
    );
    let template = line("");
    let padding = "x".repeat(RECORD_BYTES - template.len());
    let large = line(&padding);
    assert_eq!(large.len(), RECORD_BYTES);
    let started = Instant::now();
    for _ in 0..70 {
        let mut writer = source.writer().make_writer();
        writer.write_all(&large).expect("nonblocking producer");
    }
    assert!(started.elapsed() < Duration::from_secs(2));
    let health = source.health();
    assert!(health.queue_dropped > 0);
    assert!(health.has_gap);
    let pending = source.0.submitted.load(Ordering::Acquire) - health.queue_dropped;
    assert_eq!(
        pending,
        (QUEUE_RECORDS + 1) as u64,
        "64 queued and one writer-owned record"
    );
    let page = source
        .read(Some(baseline.next_cursor), 100, false, LogQuery::default())
        .await
        .expect("health independent of files");
    assert!(page.items.is_empty());
    assert!(page.health.queue_dropped > 0);
    *lock(&source.0.test.paused) = false;
    source.0.test.resume.notify_all();
    settle(&source);
    assert!(lock(&source.0.catalog).len() <= FILE_COUNT);
    assert!(
        lock(&source.0.catalog)
            .iter()
            .all(|segment| segment.readable <= FILE_BYTES)
    );
    runtime.shutdown();
}

#[tokio::test]
async fn oversize_and_writer_faults_have_separate_health_without_fabricating_records() {
    let directory = Directory::new();
    let runtime = LoggingRuntime::start(&directory.0, "core");
    let source = runtime.source();
    let baseline = source
        .read(None, 100, false, LogQuery::default())
        .await
        .expect("baseline");
    {
        let mut writer = source.writer().make_writer();
        writer
            .write_all(&vec![b'x'; RECORD_BYTES + 1])
            .expect("bounded reject");
    }
    assert_eq!(source.health().oversize_rejected, 1);
    source.0.test.fail_write.store(true, Ordering::Release);
    emit(&source, "failed write");
    settle(&source);
    assert_eq!(source.health().write_failed, 1);
    source.0.test.fail_write.store(false, Ordering::Release);
    source.0.test.force_rotation.store(true, Ordering::Release);
    source.0.test.fail_rotation.store(true, Ordering::Release);
    emit(&source, "failed rotation");
    settle(&source);
    assert_eq!(source.health().rotation_failed, 1);
    source.0.test.fail_rotation.store(false, Ordering::Release);
    source.0.test.fail_flush.store(true, Ordering::Release);
    emit(&source, "readable but flush failed");
    let deadline = Instant::now() + Duration::from_secs(1);
    while source.health().flush_failed == 0 && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert!(source.health().flush_failed > 0);
    let health = source
        .read(Some(baseline.next_cursor), 100, false, LogQuery::default())
        .await
        .expect("health without disk reads");
    let page = source
        .read(Some(health.next_cursor), 100, false, LogQuery::default())
        .await
        .expect("actual records");
    assert_eq!(page.items.len(), 1);
    assert_eq!(
        page.items[0].record["fields"]["message"],
        "readable but flush failed"
    );
    source.0.test.fail_flush.store(false, Ordering::Release);
    runtime.shutdown();
}

#[tokio::test]
async fn query_scans_unmatched_records_and_history_pagination_has_no_holes() {
    let directory = Directory::new();
    let runtime = LoggingRuntime::start(&directory.0, "core");
    let source = runtime.source();
    for i in 0..15 {
        emit(&source, &format!("record {i}"));
    }
    settle(&source);
    let mut all = Vec::new();
    let mut cursor = None;
    loop {
        let page = source
            .history(cursor, None, 4, LogQuery::default())
            .await
            .expect("page");
        all.extend(page.items);
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(all.len(), 15);
    for (i, item) in all.iter().enumerate() {
        assert_eq!(
            item.record["fields"]["message"],
            format!("record {}", 14 - i)
        );
    }
    let page = source
        .history(
            None,
            None,
            4,
            LogQuery {
                search: Some("absent".to_owned()),
                ..LogQuery::default()
            },
        )
        .await
        .expect("filtered scan");
    assert!(page.items.is_empty());
    assert!(!page.has_more);
    assert!(page.scanned_bytes > 0);
    runtime.shutdown();
}

#[tokio::test]
async fn restart_reads_previous_sessions_but_owns_only_five_segments_across_sessions() {
    let directory = Directory::new();
    let mut original_id = String::new();
    let mut first_cursor = String::new();
    let mut last = None;
    for n in 0..7 {
        let runtime = LoggingRuntime::start(&directory.0, "core");
        let source = runtime.source();
        if n == 0 {
            original_id = source.source_id().to_owned();
            first_cursor = source
                .read(None, 100, false, LogQuery::default())
                .await
                .expect("baseline")
                .next_cursor;
        }
        assert_eq!(source.source_id(), original_id);
        emit(&source, &format!("session {n}"));
        settle(&source);
        let page = source
            .history(None, None, 100, LogQuery::default())
            .await
            .expect("history");
        assert_eq!(page.items.len(), (n + 1).min(5));
        if n == 6 {
            assert_eq!(
                source
                    .read(Some(first_cursor.clone()), 100, false, LogQuery::default())
                    .await,
                Err(LogError::CursorExpired)
            );
        }
        last = Some(source);
        assert!(runtime.shutdown().drained);
    }
    assert_eq!(lock(&last.expect("source").0.catalog).len(), 5);
}

#[tokio::test]
async fn sealed_damage_and_tail_are_reported_and_live_identity_replacement_is_rejected() {
    let directory = Directory::new();
    let runtime = LoggingRuntime::start(&directory.0, "core");
    let source = runtime.source();
    emit(&source, "valid");
    settle(&source);
    let path = lock(&source.0.catalog)[0].path.clone();
    runtime.shutdown();
    {
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("owned test file");
        file.write_all(b"not json\n{\"unfinished\":")
            .expect("injected tail");
    }
    let runtime = LoggingRuntime::start(&directory.0, "core");
    let source = runtime.source();
    let page = source
        .history(None, None, 100, LogQuery::default())
        .await
        .expect("damaged history");
    assert_eq!(page.items.len(), 1);
    assert!(page.issues.iter().any(|i| i.kind == "damaged_record"));
    assert!(page.issues.iter().any(|i| i.kind == "incomplete_record"));
    fs::rename(&path, path.with_extension("moved")).expect("external move");
    fs::write(&path, line("replacement")).expect("different object");
    assert_eq!(
        source.history(None, None, 100, LogQuery::default()).await,
        Err(LogError::Unavailable)
    );
    runtime.shutdown();
}

#[test]
fn competing_writer_does_not_clean_or_own_another_active_source() {
    let directory = Directory::new();
    let first = LoggingRuntime::start(&directory.0, "gui");
    emit(&first.source(), "first");
    settle(&first.source());
    let second = LoggingRuntime::start(&directory.0, "gui");
    assert_eq!(second.source().health().state, "degraded");
    assert_eq!(first.source().health().state, "ready");
    assert_eq!(
        fs::read_dir(&directory.0)
            .expect("directory")
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|e| e == "jsonl"))
            .count(),
        1
    );
    drop(second);
    first.shutdown();
}

#[tokio::test]
async fn invalid_limits_query_and_cursors_fail_explicitly_without_polling() {
    let directory = Directory::new();
    let runtime = LoggingRuntime::start(&directory.0, "core");
    let source = runtime.source();
    assert_eq!(
        source.read(None, 0, false, LogQuery::default()).await,
        Err(LogError::InvalidInput)
    );
    assert_eq!(
        source
            .read(Some("null".to_owned()), 100, false, LogQuery::default())
            .await,
        Err(LogError::InvalidInput)
    );
    let long = LogQuery {
        search: Some("测".repeat(256)),
        ..LogQuery::default()
    };
    let baseline = source
        .read(None, 100, false, long.clone())
        .await
        .expect("bounded token");
    assert!(baseline.next_cursor.len() < 1024);
    assert!(
        source
            .read(Some(baseline.next_cursor), 100, false, long)
            .await
            .is_ok()
    );
    runtime.shutdown();
}

#[tokio::test]
async fn health_changes_bypass_a_busy_file_reader() {
    let directory = Directory::new();
    let runtime = LoggingRuntime::start(&directory.0, "core");
    let source = runtime.source();
    let baseline = source
        .read(None, 100, false, LogQuery::default())
        .await
        .expect("baseline");
    let permit = source
        .0
        .reader
        .clone()
        .acquire_owned()
        .await
        .expect("hold disk reader");
    let clone = source.clone();
    let reader = tokio::spawn(async move {
        clone
            .read(Some(baseline.next_cursor), 100, true, LogQuery::default())
            .await
    });
    tokio::task::yield_now().await;
    {
        let mut writer = source.writer().make_writer();
        writer
            .write_all(&vec![b'x'; RECORD_BYTES + 1])
            .expect("reject large record");
    }
    let page = tokio::time::timeout(Duration::from_secs(1), reader)
        .await
        .expect("health independent of read worker")
        .expect("reader")
        .expect("health page");
    assert_eq!(page.health.oversize_rejected, 1);
    assert_eq!(page.scanned_bytes, 0);
    assert!(page.items.is_empty());
    drop(permit);
    runtime.shutdown();
}

#[tokio::test]
async fn blocked_worker_has_bounded_shutdown_and_never_claims_a_flush() {
    let directory = Directory::new();
    let runtime = LoggingRuntime::start(&directory.0, "core");
    let source = runtime.source();
    *lock(&source.0.test.paused) = true;
    let started = source.0.test.write_started.notified();
    tokio::pin!(started);
    started.as_mut().enable();
    emit(&source, "blocked until after shutdown deadline");
    started.await;
    let start = Instant::now();
    let report = tokio::task::spawn_blocking(move || runtime.shutdown())
        .await
        .expect("bounded close");
    assert!(start.elapsed() < Duration::from_millis(2600));
    assert!(report.timed_out);
    assert!(!report.drained);
    assert!(!report.flushed);
    *lock(&source.0.test.paused) = false;
    source.0.test.resume.notify_all();
    assert!(wait_for_drain(&source, Duration::from_secs(1)));
}

#[tokio::test]
async fn initialization_failure_is_visible_without_a_log_file() {
    let directory = Directory::new();
    let occupied = directory.0.join("not-a-directory");
    fs::write(&occupied, b"owned fixture").expect("file");
    let runtime = LoggingRuntime::start(&occupied, "gui");
    let source = runtime.source();
    assert_eq!(source.health().state, "degraded");
    let page = source
        .read(None, 100, false, LogQuery::default())
        .await
        .expect("health despite no file");
    assert!(page.items.is_empty());
    assert!(page.health.has_gap);
    assert_eq!(
        page.health.last_failure.expect("reason").kind,
        "source_unavailable"
    );
    runtime.shutdown();
}

#[tokio::test]
async fn oversized_corrupt_lines_cross_scan_windows_without_promoting_fragments() {
    let directory = Directory::new();
    let runtime = LoggingRuntime::start(&directory.0, "core");
    let source = runtime.source();
    let baseline = source
        .read(None, 100, false, LogQuery::default())
        .await
        .expect("baseline");
    let path = lock(&source.0.catalog)[0].path.clone();
    runtime.shutdown();
    let mut bytes = line("before");
    bytes.extend(std::iter::repeat_n(b'x', SCAN_BYTES + 1000));
    bytes.extend(line("not a separate record"));
    bytes.extend(line("after"));
    fs::write(&path, bytes).expect("controlled corrupt segment");
    let runtime = LoggingRuntime::start(&directory.0, "core");
    let source = runtime.source();
    let mut cursor = baseline.next_cursor;
    let mut live = Vec::new();
    let mut issue = false;
    for _ in 0..12 {
        let page = source
            .read(Some(cursor.clone()), 100, false, LogQuery::default())
            .await
            .expect("scan progresses");
        assert!(page.scanned_bytes <= SCAN_BYTES as u64);
        issue |= !page.issues.is_empty();
        live.extend(page.items);
        let done = !page.has_more;
        if !done {
            assert_ne!(page.next_cursor, cursor);
        }
        cursor = page.next_cursor;
        if done {
            break;
        }
    }
    assert!(issue);
    assert_eq!(live.len(), 2);
    assert_eq!(live[0].record["fields"]["message"], "before");
    assert_eq!(live[1].record["fields"]["message"], "after");
    let mut cursor = None;
    let mut history = Vec::new();
    for _ in 0..12 {
        let page = source
            .history(cursor.clone(), None, 100, LogQuery::default())
            .await
            .expect("reverse scan progresses");
        assert!(page.scanned_bytes <= SCAN_BYTES as u64);
        history.extend(page.items);
        let done = !page.has_more;
        if !done {
            assert_ne!(page.next_cursor, cursor);
        }
        cursor = page.next_cursor;
        if done {
            break;
        }
    }
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].record["fields"]["message"], "after");
    assert_eq!(history[1].record["fields"]["message"], "before");
    runtime.shutdown();
}

#[tokio::test]
async fn search_matches_full_unescaped_unicode_paths_and_span_values() {
    let directory = Directory::new();
    let runtime = LoggingRuntime::start(&directory.0, "core");
    let source = runtime.source();
    let path = format!("C:\\项目 根目录\\{}file.json", "较长路径\\".repeat(130));
    assert!(path.chars().count() > 512 && path.len() < 4096);
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .json()
            .with_span_list(true)
            .with_writer(source.writer()),
    );
    tracing::subscriber::with_default(subscriber, || {
        tracing::info_span!("inspection", path = %path).in_scope(|| {
            tracing::info!(count = 42, "read completed");
        });
    });
    settle(&source);
    let query = LogQuery {
        search: Some(path.clone()),
        ..Default::default()
    };
    let page = source
        .history(None, None, 100, query)
        .await
        .expect("full path query");
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].record["spans"][0]["path"], path);
    let _ = runtime.shutdown();
}
