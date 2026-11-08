use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicU64, Ordering};

use alcomd_client::AlcomdClient;
use tauri::async_runtime::{Mutex, Receiver, Sender, channel};

static CONNECTION_GENERATION: AtomicU64 = AtomicU64::new(0);

pub(crate) fn invalidate_connections() {
    CONNECTION_GENERATION.fetch_add(1, Ordering::AcqRel);
}

struct CachedClient {
    generation: u64,
    client: Option<AlcomdClient>,
}

/// Each slot owns a sequential RPC connection. Only acquisition uses the queue
/// mutex; an in-flight request never holds a lock shared with other connections.
pub(crate) struct ClientPool {
    available: Sender<CachedClient>,
    idle: Mutex<Receiver<CachedClient>>,
}

impl ClientPool {
    pub(crate) fn new(capacity: usize) -> Self {
        let (available, idle) = channel(capacity);
        for _ in 0..capacity {
            assert!(
                available
                    .try_send(CachedClient {
                        generation: CONNECTION_GENERATION.load(Ordering::Acquire),
                        client: None,
                    })
                    .is_ok(),
                "new RPC pool has capacity"
            );
        }
        Self {
            available,
            idle: Mutex::new(idle),
        }
    }

    pub(crate) async fn lock(&self) -> ClientLease {
        let cached = self
            .idle
            .lock()
            .await
            .recv()
            .await
            .expect("the pool retains its sender");
        let generation = CONNECTION_GENERATION.load(Ordering::Acquire);
        ClientLease {
            generation,
            client: (cached.generation == generation)
                .then_some(cached.client)
                .flatten(),
            available: self.available.clone(),
        }
    }
}

pub(crate) struct ClientLease {
    generation: u64,
    client: Option<AlcomdClient>,
    available: Sender<CachedClient>,
}

impl Deref for ClientLease {
    type Target = Option<AlcomdClient>;
    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl DerefMut for ClientLease {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.client
    }
}

impl Drop for ClientLease {
    fn drop(&mut self) {
        // A cancelled request can leave a partial frame or an unread response.
        // Return an empty slot in that case so the next request reconnects.
        let generation = CONNECTION_GENERATION.load(Ordering::Acquire);
        let client = self
            .client
            .take()
            .filter(|client| self.generation == generation && client.is_usable());
        let _ = self.available.try_send(CachedClient { generation, client });
    }
}

#[cfg(test)]
mod tests {
    use super::ClientPool;
    use std::future::{Future, poll_fn};
    use std::pin::pin;
    use std::task::Poll;

    #[test]
    fn saturated_work_does_not_block_page_reads_or_control_requests() {
        tauri::async_runtime::block_on(async {
            let state = crate::GuiClientState::default();
            let mut work = Vec::new();
            for _ in 0..4 {
                work.push(state.work.lock().await);
            }
            let _repository_preview = state.repository.lock().await;
            let _extension_dispatch = state.extension_ui.lock().await;
            let mut queued_work = pin!(state.work.lock());
            assert!(
                poll_fn(|cx| Poll::Ready(queued_work.as_mut().poll(cx)))
                    .await
                    .is_pending()
            );
            // These are the same lanes used by projects/settings reads and by
            // status/operation cancellation in the actual command adapters.
            let mut read = pin!(state.reads.lock());
            let read = poll_fn(|cx| Poll::Ready(read.as_mut().poll(cx))).await;
            assert!(
                read.is_ready(),
                "page navigation must not queue behind a plan"
            );
            let mut control = pin!(state.control.lock());
            assert!(
                poll_fn(|cx| Poll::Ready(control.as_mut().poll(cx)))
                    .await
                    .is_ready()
            );
            drop(work.pop());
            assert!(
                poll_fn(|cx| Poll::Ready(queued_work.as_mut().poll(cx)))
                    .await
                    .is_ready()
            );
        });
    }

    #[test]
    fn a_busy_connection_does_not_block_another_slot_and_drop_returns_capacity() {
        tauri::async_runtime::block_on(async {
            let pool = ClientPool::new(2);
            let first = pool.lock().await;
            let second = pool.lock().await;
            let mut waiting = pin!(pool.lock());
            assert!(
                poll_fn(|cx| Poll::Ready(waiting.as_mut().poll(cx)))
                    .await
                    .is_pending()
            );
            drop(second);
            let next = poll_fn(|cx| Poll::Ready(waiting.as_mut().poll(cx))).await;
            assert!(
                next.is_ready(),
                "use the released slot without waiting for the first"
            );
            drop(next);
            drop(first);
            let _one = pool.lock().await;
            let _two = pool.lock().await;
        });
    }

    #[test]
    fn abandoning_a_queued_request_does_not_lose_the_connection_slot() {
        tauri::async_runtime::block_on(async {
            let pool = ClientPool::new(1);
            let active = pool.lock().await;
            {
                let mut queued = pin!(pool.lock());
                assert!(
                    poll_fn(|cx| Poll::Ready(queued.as_mut().poll(cx)))
                        .await
                        .is_pending()
                );
            }
            drop(active);
            let _next = pool.lock().await;
        });
    }

    #[test]
    fn reconnect_invalidates_idle_and_in_flight_connections_in_every_lane() {
        tauri::async_runtime::block_on(async {
            let reads = ClientPool::new(1);
            let work = ClientPool::new(1);
            let old_work = work.lock().await;
            super::invalidate_connections();
            let next_read = reads.lock().await;
            assert_ne!(next_read.generation, old_work.generation);
            drop(old_work);
            let next_work = work.lock().await;
            assert_eq!(next_read.generation, next_work.generation);
        });
    }
}
