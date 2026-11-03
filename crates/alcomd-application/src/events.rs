//! Live delivery is driven by committed watermarks, never by periodic reads.

use tokio::sync::watch;

use crate::{
    AccessContext, Application, ApplicationError, EventPage, EventRecord, OperationId, Permission,
    PrincipalId, StateStore, StoreError, StoreErrorKind,
};

/// A disposable identity and durable sequence used to resume one Core instance.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventsCursor {
    pub instance_id: String,
    pub sequence: u64,
    pub resources_revision: u64,
}

/// The only in-memory state needed to wake durable event readers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventCommitWatermark {
    pub instance_id: String,
    pub sequence: u64,
    pub resources_revision: u64,
    pub available: bool,
}

/// Fixed resource invalidations for state without a durable event aggregate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventInvalidation {
    Extensions,
}

#[derive(Clone, Debug)]
pub struct LiveEventPage {
    pub events: Vec<EventRecord>,
    pub next_sequence: u64,
    pub resources_revision: u64,
    pub invalidated_scopes: Vec<EventInvalidation>,
}

/// One owned live subscription. Pages remain durable until explicitly advanced.
pub struct EventSubscription<S: StateStore> {
    pub subscription_id: String,
    pub instance_id: String,
    pub baseline_cursor: EventsCursor,
    pub resync_required: bool,
    store: S,
    owner: PrincipalId,
    commits: watch::Receiver<EventCommitWatermark>,
    sequence: u64,
    offered_through: u64,
    resources_revision: u64,
    offered_resources_revision: u64,
    limit: u32,
}

impl<S: StateStore> Application<S> {
    /// Registers the wakeup receiver before inspecting the committed watermark.
    pub fn subscribe_events(
        &self,
        access: &AccessContext,
        cursor: Option<EventsCursor>,
        limit: u32,
    ) -> Result<EventSubscription<S>, ApplicationError> {
        access.require(Permission::EventsRead)?;
        if access.principal() != &PrincipalId::local_owner() {
            return Err(ApplicationError::PermissionDenied);
        }
        if !(1..=1_000).contains(&limit)
            || cursor.as_ref().is_some_and(|value| {
                OperationId::parse(&value.instance_id).is_err()
                    || value.sequence > i64::MAX as u64
                    || value.resources_revision > i64::MAX as u64
            })
        {
            return Err(ApplicationError::InvalidInput);
        }
        let mut commits = self.store.event_commits();
        let watermark = commits.borrow_and_update().clone();
        if !watermark.available || commits.has_changed().is_err() {
            return Err(unavailable());
        }
        let resync_required = cursor.as_ref().is_none_or(|value| {
            value.instance_id != watermark.instance_id
                || value.sequence > watermark.sequence
                || value.resources_revision != watermark.resources_revision
        });
        let sequence = if resync_required {
            watermark.sequence
        } else {
            cursor.as_ref().expect("validated cursor").sequence
        };
        Ok(EventSubscription {
            subscription_id: OperationId::new().to_string(),
            instance_id: watermark.instance_id.clone(),
            baseline_cursor: EventsCursor {
                instance_id: watermark.instance_id,
                sequence,
                resources_revision: watermark.resources_revision,
            },
            resync_required,
            store: self.store.clone(),
            owner: access.principal().clone(),
            commits,
            sequence,
            offered_through: sequence,
            resources_revision: watermark.resources_revision,
            offered_resources_revision: watermark.resources_revision,
            limit,
        })
    }
}

impl<S: StateStore> EventSubscription<S> {
    /// Reads committed data, then waits without retaining a database transaction.
    /// A commit racing with the read is retained by `watch::changed`.
    pub async fn next_page(&mut self) -> Result<LiveEventPage, ApplicationError> {
        let store = &self.store;
        let owner = &self.owner;
        let sequence = self.sequence;
        let limit = self.limit;
        let page = read_committed_page(
            &mut self.commits,
            &self.instance_id,
            self.resources_revision,
            || store.list_events(owner.clone(), sequence, limit),
        )
        .await?;
        self.offered_through = page.next_sequence;
        self.offered_resources_revision = page.resources_revision;
        Ok(page)
    }

    /// Advances only a prefix that the transport actually sent successfully.
    pub fn advance(
        &mut self,
        sequence: u64,
        resources_revision: u64,
    ) -> Result<(), ApplicationError> {
        if sequence < self.sequence
            || sequence > self.offered_through
            || resources_revision < self.resources_revision
            || resources_revision > self.offered_resources_revision
            || (sequence == self.sequence && resources_revision == self.resources_revision)
        {
            return Err(ApplicationError::InvalidInput);
        }
        self.sequence = sequence;
        self.resources_revision = resources_revision;
        Ok(())
    }
}

async fn read_committed_page<F, R>(
    commits: &mut watch::Receiver<EventCommitWatermark>,
    instance_id: &str,
    resources_revision: u64,
    mut read: R,
) -> Result<LiveEventPage, ApplicationError>
where
    F: std::future::Future<Output = Result<EventPage, StoreError>>,
    R: FnMut() -> F,
{
    loop {
        let watermark = commits.borrow_and_update().clone();
        if !watermark.available || watermark.instance_id != instance_id {
            return Err(unavailable());
        }
        let page = read().await?;
        if !page.events.is_empty() || watermark.resources_revision != resources_revision {
            return Ok(LiveEventPage {
                events: page.events,
                next_sequence: page.next_sequence,
                resources_revision: watermark.resources_revision,
                invalidated_scopes: if watermark.resources_revision != resources_revision {
                    vec![EventInvalidation::Extensions]
                } else {
                    vec![]
                },
            });
        }
        // The receiver was marked before the read. A commit racing with an
        // empty result remains unseen and wakes this await immediately.
        commits.changed().await.map_err(|_| unavailable())?;
    }
}

fn unavailable() -> ApplicationError {
    StoreError::new(StoreErrorKind::Unavailable).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EventRecord, Revision};
    use std::future::{Future, poll_fn, ready};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::task::Poll;

    fn event(sequence: u64) -> EventRecord {
        EventRecord {
            sequence,
            event_id: OperationId::new().to_string(),
            kind: "operation.created".to_owned(),
            aggregate_kind: "operation".to_owned(),
            aggregate_id: OperationId::new().to_string(),
            aggregate_revision: Revision::INITIAL,
            occurred_at_ms: 0,
            payload_json: "{}".to_owned(),
        }
    }

    #[tokio::test]
    async fn commit_between_empty_read_and_wait_is_retained() {
        let identity = OperationId::new().to_string();
        let (sender, mut receiver) = watch::channel(EventCommitWatermark {
            instance_id: identity.clone(),
            sequence: 0,
            resources_revision: 0,
            available: true,
        });
        let reads = AtomicUsize::new(0);
        let result = read_committed_page(&mut receiver, &identity, 0, || {
            if reads.fetch_add(1, Ordering::SeqCst) == 0 {
                // Exact race: the read saw empty; commit happens before its
                // caller can begin awaiting the change signal.
                sender.send_modify(|watermark| watermark.sequence = 1);
                ready(Ok(EventPage {
                    events: vec![],
                    next_sequence: 0,
                }))
            } else {
                ready(Ok(EventPage {
                    events: vec![event(1)],
                    next_sequence: 1,
                }))
            }
        })
        .await
        .expect("racing committed event");
        assert_eq!(result.events.len(), 1);
        assert_eq!(reads.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn resource_commit_between_empty_read_and_wait_is_retained() {
        let identity = OperationId::new().to_string();
        let (sender, mut receiver) = watch::channel(EventCommitWatermark {
            instance_id: identity.clone(),
            sequence: 0,
            resources_revision: 0,
            available: true,
        });
        let reads = AtomicUsize::new(0);
        let page = read_committed_page(&mut receiver, &identity, 0, || {
            if reads.fetch_add(1, Ordering::SeqCst) == 0 {
                sender.send_modify(|watermark| watermark.resources_revision = 1);
            }
            ready(Ok(EventPage {
                events: vec![],
                next_sequence: 0,
            }))
        })
        .await
        .expect("resource commit retained");
        assert!(page.events.is_empty());
        assert_eq!(page.resources_revision, 1);
        assert_eq!(page.invalidated_scopes, vec![EventInvalidation::Extensions]);
        assert_eq!(reads.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn unrelated_owner_wakeup_does_not_poll_or_report_a_gap() {
        let identity = OperationId::new().to_string();
        let (sender, mut receiver) = watch::channel(EventCommitWatermark {
            instance_id: identity.clone(),
            sequence: 0,
            resources_revision: 0,
            available: true,
        });
        let reads = AtomicUsize::new(0);
        let future = read_committed_page(&mut receiver, &identity, 0, || {
            let sequence = sender.borrow().sequence;
            reads.fetch_add(1, Ordering::SeqCst);
            ready(Ok(EventPage {
                events: if sequence == 20 {
                    vec![event(20)]
                } else {
                    vec![]
                },
                next_sequence: sequence,
            }))
        });
        tokio::pin!(future);
        assert!(poll_fn(|context| Poll::Ready(future.as_mut().poll(context).is_pending())).await);
        assert_eq!(reads.load(Ordering::SeqCst), 1);
        sender.send_modify(|value| value.sequence = 10);
        assert!(poll_fn(|context| Poll::Ready(future.as_mut().poll(context).is_pending())).await);
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        assert!(poll_fn(|context| Poll::Ready(future.as_mut().poll(context).is_pending())).await);
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        sender.send_modify(|value| value.sequence = 20);
        assert_eq!(future.await.expect("owned event").events[0].sequence, 20);
    }
}
