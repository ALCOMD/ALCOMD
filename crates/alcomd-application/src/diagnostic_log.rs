//! Bounded, ephemeral and redacted Core technical-log projection.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::Notify;
use tokio::time::Instant;

use crate::{AccessContext, OperationId, Permission, PrincipalId};

pub const DIAGNOSTIC_LOG_CAPACITY: usize = 1024;
const MAX_SAFE_SEQUENCE: u64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticLogLevel {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
}

impl DiagnosticLogLevel {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "trace",
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    const fn fallback_code(self) -> &'static str {
        match self {
            Self::Trace => "diagnostic.trace",
            Self::Debug => "diagnostic.debug",
            Self::Info => "diagnostic.info",
            Self::Warning => "diagnostic.warning",
            Self::Error => "diagnostic.error",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticLogCursor {
    pub instance_id: String,
    pub sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticLogItem {
    pub instance_id: String,
    pub sequence: u64,
    pub occurred_at_ms: u64,
    pub level: DiagnosticLogLevel,
    pub code: String,
    pub diagnostic_id: Option<String>,
    pub notify: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticLogPage {
    pub items: Vec<DiagnosticLogItem>,
    pub next_cursor: DiagnosticLogCursor,
    pub reset: bool,
    pub gap: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiagnosticLogError {
    InvalidInput,
    PermissionDenied,
}

struct LogState {
    instance_id: String,
    sequence: u64,
    items: VecDeque<DiagnosticLogItem>,
}

struct LogInner {
    state: Mutex<LogState>,
    changed: Notify,
}

/// One shared in-memory stream for a Core lifetime, independent of GUI visits.
#[derive(Clone)]
pub struct DiagnosticLog(Arc<LogInner>);

impl Default for DiagnosticLog {
    fn default() -> Self {
        Self::new()
    }
}

impl DiagnosticLog {
    #[must_use]
    pub fn new() -> Self {
        Self(Arc::new(LogInner {
            state: Mutex::new(LogState {
                instance_id: new_identity(),
                sequence: 0,
                items: VecDeque::with_capacity(DIAGNOSTIC_LOG_CAPACITY),
            }),
            changed: Notify::new(),
        }))
    }

    /// Accept only bounded machine fields. Raw technical text has no input slot.
    pub fn record(
        &self,
        level: DiagnosticLogLevel,
        code: Option<&str>,
        diagnostic_id: Option<&str>,
        notify: Option<bool>,
    ) -> DiagnosticLogItem {
        let code = code
            .filter(|value| safe_code(value))
            .unwrap_or(level.fallback_code());
        let diagnostic_id = diagnostic_id
            .filter(|value| safe_identity(value))
            .map(str::to_owned)
            .or_else(|| (level == DiagnosticLogLevel::Error).then(new_identity));
        let notify = match level {
            DiagnosticLogLevel::Error => notify.unwrap_or(true),
            DiagnosticLogLevel::Warning => notify.unwrap_or(false),
            DiagnosticLogLevel::Trace | DiagnosticLogLevel::Debug | DiagnosticLogLevel::Info => {
                false
            }
        };
        let mut state = self
            .0
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Never alias a cursor or expose an integer JavaScript cannot represent.
        if state.sequence == MAX_SAFE_SEQUENCE {
            state.instance_id = new_identity();
            state.sequence = 0;
            state.items.clear();
        }
        state.sequence += 1;
        let item = DiagnosticLogItem {
            instance_id: state.instance_id.clone(),
            sequence: state.sequence,
            occurred_at_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
                .min(u128::from(MAX_SAFE_SEQUENCE)) as u64,
            level,
            code: code.to_owned(),
            diagnostic_id,
            notify,
        };
        if state.items.len() == DIAGNOSTIC_LOG_CAPACITY {
            state.items.pop_front();
        }
        state.items.push_back(item.clone());
        drop(state);
        self.0.changed.notify_waiters();
        item
    }

    /// Initial/reset/gap reads return recent history and a tail cursor. Normal
    /// incremental reads page in order; a gap never pretends continuity.
    pub async fn read(
        &self,
        access: &AccessContext,
        cursor: Option<DiagnosticLogCursor>,
        limit: Option<u32>,
        wait_ms: Option<u32>,
    ) -> Result<DiagnosticLogPage, DiagnosticLogError> {
        if access.principal() != &PrincipalId::local_owner()
            || access.require(Permission::DiagnosticsRead).is_err()
        {
            return Err(DiagnosticLogError::PermissionDenied);
        }
        let limit = limit.unwrap_or(100);
        let wait_ms = wait_ms.unwrap_or(0);
        if !(1..=200).contains(&limit)
            || wait_ms > 20_000
            || cursor.as_ref().is_some_and(|value| {
                !safe_identity(&value.instance_id) || value.sequence > MAX_SAFE_SEQUENCE
            })
        {
            return Err(DiagnosticLogError::InvalidInput);
        }
        let deadline = Instant::now() + Duration::from_millis(u64::from(wait_ms));
        loop {
            // Register before inspecting the buffer; a record between inspection
            // and awaiting must wake this read, including concurrent readers.
            let changed = self.0.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let page = self.snapshot(cursor.as_ref(), limit as usize)?;
            if cursor.is_none() || page.reset || page.gap || !page.items.is_empty() || wait_ms == 0
            {
                return Ok(page);
            }
            if tokio::time::timeout_at(deadline, changed).await.is_err() {
                return self.snapshot(cursor.as_ref(), limit as usize);
            }
        }
    }

    fn snapshot(
        &self,
        cursor: Option<&DiagnosticLogCursor>,
        limit: usize,
    ) -> Result<DiagnosticLogPage, DiagnosticLogError> {
        let state = self
            .0
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let reset = cursor.is_some_and(|value| value.instance_id != state.instance_id);
        let tail = DiagnosticLogCursor {
            instance_id: state.instance_id.clone(),
            sequence: state.sequence,
        };
        if !reset && cursor.is_some_and(|value| value.sequence > state.sequence) {
            return Err(DiagnosticLogError::InvalidInput);
        }
        let gap = !reset
            && cursor.is_some_and(|cursor| {
                state
                    .items
                    .front()
                    .is_some_and(|first| cursor.sequence < first.sequence.saturating_sub(1))
            });
        if cursor.is_none() || reset || gap {
            return Ok(DiagnosticLogPage {
                items: state
                    .items
                    .iter()
                    .skip(state.items.len().saturating_sub(limit))
                    .cloned()
                    .collect(),
                next_cursor: tail,
                reset,
                gap,
            });
        }
        let cursor = cursor.expect("initial reads returned above");
        let items: Vec<_> = state
            .items
            .iter()
            .filter(|item| item.sequence > cursor.sequence)
            .take(limit)
            .cloned()
            .collect();
        let sequence = items.last().map_or(cursor.sequence, |item| item.sequence);
        Ok(DiagnosticLogPage {
            items,
            next_cursor: DiagnosticLogCursor {
                instance_id: state.instance_id.clone(),
                sequence,
            },
            reset: false,
            gap: false,
        })
    }
}

fn new_identity() -> String {
    OperationId::new().to_string()
}

fn safe_code(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 96
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(byte))
}

fn safe_identity(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::task::Poll;

    #[tokio::test]
    async fn history_is_bounded_and_incremental_reads_do_not_skip_pages() {
        let log = DiagnosticLog::new();
        for _ in 0..250 {
            log.record(DiagnosticLogLevel::Info, Some("core.ready"), None, None);
        }
        let initial = log
            .read(&AccessContext::local_owner(), None, Some(2), None)
            .await
            .expect("local owner can read the bounded diagnostic log");
        assert_eq!(
            initial
                .items
                .iter()
                .map(|item| item.sequence)
                .collect::<Vec<_>>(),
            [249, 250]
        );
        assert_eq!(initial.next_cursor.sequence, 250);
        assert!(!initial.reset && !initial.gap);
        for _ in 0..5 {
            log.record(DiagnosticLogLevel::Info, None, None, None);
        }
        let first = log
            .read(
                &AccessContext::local_owner(),
                Some(initial.next_cursor),
                Some(2),
                None,
            )
            .await
            .expect("local owner can read the bounded diagnostic log");
        assert_eq!(
            first
                .items
                .iter()
                .map(|item| item.sequence)
                .collect::<Vec<_>>(),
            [251, 252]
        );
        let second = log
            .read(
                &AccessContext::local_owner(),
                Some(first.next_cursor),
                Some(2),
                None,
            )
            .await
            .expect("local owner can read the bounded diagnostic log");
        assert_eq!(
            second
                .items
                .iter()
                .map(|item| item.sequence)
                .collect::<Vec<_>>(),
            [253, 254]
        );
        assert_eq!(second.next_cursor.sequence, 254);
    }

    #[tokio::test]
    async fn overflow_and_new_core_have_distinct_gap_and_reset_results() {
        let log = DiagnosticLog::new();
        let cursor = log
            .read(&AccessContext::local_owner(), None, None, None)
            .await
            .expect("local owner can read the bounded diagnostic log")
            .next_cursor;
        for _ in 0..1027 {
            log.record(DiagnosticLogLevel::Info, None, None, None);
        }
        assert_eq!(
            log.0
                .state
                .lock()
                .expect("log mutex remains unpoisoned")
                .items
                .len(),
            DIAGNOSTIC_LOG_CAPACITY
        );
        let page = log
            .read(&AccessContext::local_owner(), Some(cursor), Some(2), None)
            .await
            .expect("local owner can read the bounded diagnostic log");
        assert!(page.gap && !page.reset);
        assert_eq!(
            page.items
                .iter()
                .map(|item| item.sequence)
                .collect::<Vec<_>>(),
            [1026, 1027]
        );
        assert_eq!(page.next_cursor.sequence, 1027);
        let next = log
            .read(
                &AccessContext::local_owner(),
                Some(page.next_cursor.clone()),
                Some(2),
                None,
            )
            .await
            .expect("local owner can read the bounded diagnostic log");
        assert!(!next.gap && !next.reset);
        assert!(next.items.is_empty());
        let restarted = DiagnosticLog::new();
        restarted.record(DiagnosticLogLevel::Warning, None, None, None);
        let reset = restarted
            .read(
                &AccessContext::local_owner(),
                Some(page.next_cursor),
                Some(1),
                Some(20_000),
            )
            .await
            .expect("local owner can read the bounded diagnostic log");
        assert!(reset.reset && !reset.gap);
        assert_eq!(reset.items.len(), 1);
    }

    #[tokio::test]
    async fn permission_and_input_bounds_are_checked_before_waiting() {
        let log = DiagnosticLog::new();
        let denied = AccessContext::new(PrincipalId::local_owner(), []);
        assert_eq!(
            log.read(&denied, None, None, None).await,
            Err(DiagnosticLogError::PermissionDenied)
        );
        let external = AccessContext::new(
            PrincipalId::parse("client:external").expect("external principal fixture is valid"),
            [Permission::DiagnosticsRead],
        );
        assert_eq!(
            log.read(&external, None, None, None).await,
            Err(DiagnosticLogError::PermissionDenied)
        );
        for (limit, wait) in [(0, 0), (201, 0), (1, 20_001)] {
            assert_eq!(
                log.read(&AccessContext::local_owner(), None, Some(limit), Some(wait))
                    .await,
                Err(DiagnosticLogError::InvalidInput)
            );
        }
        let mut cursor = log
            .read(&AccessContext::local_owner(), None, None, None)
            .await
            .expect("local owner can read the bounded diagnostic log")
            .next_cursor;
        cursor.sequence = 1;
        assert_eq!(
            log.read(&AccessContext::local_owner(), Some(cursor), None, None)
                .await,
            Err(DiagnosticLogError::InvalidInput)
        );
    }

    #[test]
    fn unsafe_fields_never_enter_the_buffer_and_error_identity_is_available() {
        let log = DiagnosticLog::new();
        for code in [
            "C:\\private\\token",
            "https://secret.example",
            "unsafe\ncode",
            "",
            &"x".repeat(97),
        ] {
            let item = log.record(DiagnosticLogLevel::Error, Some(code), Some("secret"), None);
            assert_eq!(item.code, "diagnostic.error");
            assert!(safe_identity(
                item.diagnostic_id
                    .as_deref()
                    .expect("Error records receive a diagnostic identity")
            ));
            assert!(item.notify);
        }
        let id = new_identity();
        let item = log.record(
            DiagnosticLogLevel::Warning,
            Some("unity_version_unverified"),
            Some(&id),
            Some(true),
        );
        assert_eq!(item.code, "unity_version_unverified");
        assert_eq!(item.diagnostic_id.as_deref(), Some(id.as_str()));
        assert!(item.notify);
    }

    #[test]
    fn level_defaults_and_overrides_keep_low_levels_silent() {
        let log = DiagnosticLog::new();
        for level in [
            DiagnosticLogLevel::Trace,
            DiagnosticLogLevel::Debug,
            DiagnosticLogLevel::Info,
        ] {
            assert!(!log.record(level, None, None, Some(true)).notify);
        }
        assert!(
            !log.record(DiagnosticLogLevel::Warning, None, None, None)
                .notify
        );
        assert!(
            log.record(DiagnosticLogLevel::Error, None, None, None)
                .notify
        );
        assert!(
            !log.record(DiagnosticLogLevel::Error, None, None, Some(false))
                .notify
        );
    }

    #[tokio::test]
    async fn waiting_read_is_woken_without_lost_notifications_and_can_be_cancelled() {
        let log = DiagnosticLog::new();
        let access = AccessContext::local_owner();
        let cursor = log
            .read(&access, None, None, None)
            .await
            .expect("local owner can read the bounded diagnostic log")
            .next_cursor;
        let mut first = Box::pin(log.read(&access, Some(cursor.clone()), None, Some(20_000)));
        let mut second = Box::pin(log.read(&access, Some(cursor.clone()), None, Some(20_000)));
        std::future::poll_fn(|cx| {
            assert!(first.as_mut().poll(cx).is_pending());
            assert!(second.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        log.record(DiagnosticLogLevel::Error, None, None, None);
        let a = tokio::time::timeout(Duration::from_secs(1), first)
            .await
            .expect("first waiting reader wakes after a record")
            .expect("first resumed owner read succeeds");
        let b = tokio::time::timeout(Duration::from_secs(1), second)
            .await
            .expect("second waiting reader also wakes after the record")
            .expect("second resumed owner read succeeds");
        assert_eq!(a.items, b.items);
        let mut cancelled =
            Box::pin(log.read(&access, Some(a.next_cursor.clone()), None, Some(20_000)));
        std::future::poll_fn(|cx| {
            assert!(cancelled.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        })
        .await;
        drop(cancelled);
        let item = log.record(DiagnosticLogLevel::Warning, None, None, None);
        let after = log
            .read(&access, Some(a.next_cursor), None, None)
            .await
            .expect("local owner can read the bounded diagnostic log");
        assert_eq!(after.items, [item]);
    }

    #[tokio::test]
    async fn empty_wait_times_out_without_advancing_the_cursor() {
        let log = DiagnosticLog::new();
        let access = AccessContext::local_owner();
        let cursor = log
            .read(&access, None, None, None)
            .await
            .expect("local owner can read the bounded diagnostic log")
            .next_cursor;
        let page = log
            .read(&access, Some(cursor.clone()), None, Some(1))
            .await
            .expect("local owner can read the bounded diagnostic log");
        assert_eq!(page.next_cursor, cursor);
        assert!(page.items.is_empty() && !page.reset && !page.gap);
    }
}
