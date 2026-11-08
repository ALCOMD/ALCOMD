//! On-demand background repository maintenance, separate from read-only queries.
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::*;

pub const REPOSITORY_REFRESH_AGE: Duration = Duration::from_secs(5 * 60);

pub(super) fn repository_refresh_due(refreshed_at_ms: u64, now_ms: u64) -> bool {
    refreshed_at_ms == 0
        || now_ms < refreshed_at_ms
        || now_ms.saturating_sub(refreshed_at_ms) >= REPOSITORY_REFRESH_AGE.as_millis() as u64
}

#[derive(Default)]
struct RefreshAttempts(HashMap<RepositoryId, Instant>);

impl RefreshAttempts {
    fn claim(&mut self, id: RepositoryId, now: Instant) -> bool {
        if self
            .0
            .get(&id)
            .is_some_and(|last| now.saturating_duration_since(*last) < REPOSITORY_REFRESH_AGE)
        {
            return false;
        }
        self.0.insert(id, now);
        true
    }
}

#[derive(Clone)]
pub struct RepositoryAutoRefresh<S: M3RegistryStore, R: M3ReadAdapter> {
    registry: M3Application<S, R>,
    config: M7OfficialApplication,
    attempts: Arc<Mutex<RefreshAttempts>>,
    running: Arc<Mutex<()>>,
}

impl<S: M3RegistryStore, R: M3ReadAdapter> RepositoryAutoRefresh<S, R> {
    pub fn new(registry: M3Application<S, R>, config: M7OfficialApplication) -> Self {
        Self {
            registry,
            config,
            attempts: Arc::new(Mutex::new(RefreshAttempts::default())),
            running: Arc::new(Mutex::new(())),
        }
    }

    /// Core-owned maintenance authority is never supplied by a querying client.
    /// Concurrent hints coalesce; failures are retried only after the same age budget.
    pub async fn refresh_due(
        &self,
    ) -> Result<Vec<(RepositoryId, Result<bool, M3ErrorCode>)>, M3Error>
    where
        S: OperationLogStore,
    {
        let Ok(_running) = self.running.try_lock() else {
            return Ok(Vec::new());
        };
        let access =
            AccessContext::local_owner().with_actor("Core repository auto refresh".to_owned());
        let now_ms = m3_time_ms()?;
        let now = Instant::now();
        let mut records = Vec::new();
        let mut cursor = None;
        loop {
            let page = self
                .registry
                .list_repositories(&access, cursor, 100)
                .await?;
            records.extend(page.repositories);
            if records.len() > REPOSITORY_LIST_MAX_ENTRIES {
                return Err(M3Error::new(M3ErrorCode::RepositoryDocumentTooLarge));
            }
            cursor = page.next_cursor;
            if cursor.is_none() {
                break;
            }
        }
        let settings = self
            .config
            .candidate_settings()
            .await
            .map_err(|_| M3Error::new(M3ErrorCode::Internal))?;
        let hidden = settings.settings.packages.hidden_repository_ids;
        let present = records
            .iter()
            .map(|record| record.repository_id)
            .collect::<HashSet<_>>();
        let mut attempts = self.attempts.lock().await;
        attempts.0.retain(|id, _| present.contains(id));
        let jobs = records
            .into_iter()
            .filter(|record| {
                !hidden
                    .iter()
                    .any(|id| id == &record.repository_id.to_string())
                    && (!record.resolver_indexed
                        || repository_refresh_due(record.observation.refreshed_at_ms, now_ms))
                    && attempts.claim(record.repository_id, now)
            })
            .map(|record| record.repository_id)
            .collect::<Vec<_>>();
        drop(attempts);
        let mut pending = jobs.into_iter();
        let mut running = tokio::task::JoinSet::new();
        let mut outcomes = Vec::new();
        loop {
            while running.len() < DOWNLOAD_CONCURRENCY {
                let Some(id) = pending.next() else {
                    break;
                };
                let registry = self.registry.clone();
                let access = access.clone();
                running.spawn(async move {
                    let key = IdempotencyKey::parse(OperationId::new().to_string())
                        .map_err(|_| M3ErrorCode::Internal);
                    let result = match key {
                        Ok(key) => with_log_caller(
                            registry.store.clone(),
                            &access,
                            registry.refresh_repository_if_stale(&access, id, key),
                        )
                        .await
                        .map_err(|error| error.code()),
                        Err(code) => Err(code),
                    };
                    (id, result)
                });
            }
            match running.join_next().await {
                Some(Ok(outcome)) => outcomes.push(outcome),
                Some(Err(_)) => return Err(M3Error::new(M3ErrorCode::Internal)),
                None => break,
            }
        }
        Ok(outcomes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_age_and_clock_rollback_are_bounded() {
        assert!(!repository_refresh_due(1_000, 300_999));
        assert!(repository_refresh_due(1_000, 301_000));
        assert!(repository_refresh_due(0, 1));
        assert!(repository_refresh_due(5_000, 1_000));
    }

    #[test]
    fn repeated_hints_and_failed_attempts_share_a_five_minute_cooldown() {
        let id = RepositoryId::parse("11111111-1111-4111-8111-111111111111").expect("ID");
        let now = Instant::now();
        let mut attempts = RefreshAttempts::default();
        assert!(attempts.claim(id, now));
        assert!(!attempts.claim(id, now + Duration::from_secs(299)));
        assert!(attempts.claim(id, now + REPOSITORY_REFRESH_AGE));
        assert!(!attempts.claim(id, now + REPOSITORY_REFRESH_AGE));
    }
}
