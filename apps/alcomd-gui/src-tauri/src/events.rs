//! Thin, single-consumer subscription bridge. It never queries business state.

use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use alcomd_client::{ClientConfig, EventSubscription};
use alcomd_protocol::{
    EventNotification, EventsBatch, EventsClosed, EventsCursor, EventsSubscribeParams,
    EventsSubscribeResult, RpcError,
};
use tauri::ipc::{Channel, InvokeResponseBody, IpcResponse};
use tauri::{Manager, State, Window, WindowEvent};

const ACK_TIMEOUT: Duration = Duration::from_secs(10);
type Task = tauri::async_runtime::JoinHandle<()>;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Identity {
    consumer_id: String,
    generation: u64,
}

struct Consumer {
    identity: Identity,
    channel: Option<Channel<Message>>,
    subscription_id: Option<String>,
    pending_batch: Option<u64>,
    accepted_batch: u64,
}

#[derive(Default)]
struct Inner {
    consumer: Option<Consumer>,
    task: Option<Task>,
}

#[derive(Default)]
struct Shared {
    inner: Mutex<Inner>,
    changed: Condvar,
}

#[derive(Default)]
pub(crate) struct EventsClient {
    shared: Arc<Shared>,
}

enum Payload {
    Ready(EventsSubscribeResult),
    Batch(EventsBatch),
    Closed(EventsClosed),
    Error { error: RpcError, retryable: bool },
}

pub(crate) struct Message {
    identity: Identity,
    payload: Payload,
}

// Tauri already serializes the strongly typed payloads through IpcResponse.
// Only this adapter envelope is assembled here; no new serde dependency or
// arbitrary RPC forwarding is needed in the native GUI.
impl IpcResponse for Message {
    fn body(self) -> tauri::Result<InvokeResponseBody> {
        fn json(value: impl IpcResponse) -> tauri::Result<String> {
            match value.body()? {
                InvokeResponseBody::Json(value) => Ok(value),
                InvokeResponseBody::Raw(_) => Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "event bridge requires JSON",
                )
                .into()),
            }
        }
        let consumer = json(self.identity.consumer_id)?;
        let (kind, name, data, extra) = match self.payload {
            Payload::Ready(value) => ("ready", "ready", json(value)?, String::new()),
            Payload::Batch(value) => ("batch", "batch", json(value)?, String::new()),
            Payload::Closed(value) => ("closed", "closed", json(value)?, String::new()),
            Payload::Error { error, retryable } => (
                "error",
                "error",
                json(error)?,
                format!(",\"retryable\":{retryable}"),
            ),
        };
        Ok(InvokeResponseBody::Json(format!(
            "{{\"type\":\"{kind}\",\"consumerId\":{consumer},\"generation\":{},\"{name}\":{data}{extra}}}",
            self.identity.generation
        )))
    }
}

fn lock(shared: &Shared) -> std::sync::MutexGuard<'_, Inner> {
    shared
        .inner
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn current(inner: &Inner, identity: &Identity) -> bool {
    inner
        .consumer
        .as_ref()
        .is_some_and(|consumer| &consumer.identity == identity)
}

impl Shared {
    fn stop(&self, identity: Option<&Identity>) -> Option<Task> {
        let mut inner = lock(self);
        if identity.is_some_and(|identity| !current(&inner, identity)) {
            return None;
        }
        inner.consumer = None;
        let task = inner.task.take();
        self.changed.notify_all();
        task
    }

    fn acknowledge(
        &self,
        identity: &Identity,
        subscription_id: &str,
        batch_id: u64,
    ) -> Result<(), RpcError> {
        let mut inner = lock(self);
        if !current(&inner, identity) {
            return Ok(());
        }
        let consumer = inner.consumer.as_mut().expect("current consumer");
        if batch_id == 0 || consumer.subscription_id.as_deref() != Some(subscription_id) {
            return Err(RpcError::invalid_request());
        }
        if batch_id == consumer.accepted_batch {
            return Ok(());
        }
        if consumer.pending_batch != Some(batch_id) {
            return Err(RpcError::invalid_request());
        }
        consumer.pending_batch = None;
        consumer.accepted_batch = batch_id;
        self.changed.notify_all();
        Ok(())
    }
}

struct Cleanup {
    shared: Arc<Shared>,
    identity: Identity,
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        self.shared.stop(Some(&self.identity));
    }
}

impl Drop for EventsClient {
    fn drop(&mut self) {
        if let Some(task) = self.shared.stop(None) {
            task.abort();
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Acknowledged {
    Accepted,
    Cancelled,
    TimedOut,
}

fn wait_ack(
    shared: &Shared,
    identity: &Identity,
    batch_id: u64,
    timeout: Duration,
) -> Acknowledged {
    let inner = lock(shared);
    let (inner, result) = shared
        .changed
        .wait_timeout_while(inner, timeout, |inner| {
            inner.consumer.as_ref().is_some_and(|consumer| {
                &consumer.identity == identity && consumer.pending_batch == Some(batch_id)
            })
        })
        .unwrap_or_else(|error| error.into_inner());
    match inner.consumer.as_ref() {
        Some(consumer) if &consumer.identity == identity && consumer.accepted_batch >= batch_id => {
            Acknowledged::Accepted
        }
        Some(consumer) if &consumer.identity == identity && result.timed_out() => {
            Acknowledged::TimedOut
        }
        _ => Acknowledged::Cancelled,
    }
}

fn send(
    shared: &Shared,
    identity: &Identity,
    channel: &Channel<Message>,
    payload: Payload,
) -> bool {
    if !current(&lock(shared), identity) {
        return false;
    }
    channel
        .send(Message {
            identity: identity.clone(),
            payload,
        })
        .is_ok()
}

async fn run(
    shared: Arc<Shared>,
    identity: Identity,
    cursor: Option<EventsCursor>,
    channel: Channel<Message>,
) {
    let _cleanup = Cleanup {
        shared: Arc::clone(&shared),
        identity: identity.clone(),
    };
    let mut subscription = match EventSubscription::connect(
        ClientConfig::default().without_daemon_start(),
        EventsSubscribeParams {
            cursor,
            limit: None,
        },
    )
    .await
    {
        Ok(value) => value,
        Err(error) => {
            let retryable = error.is_retryable();
            send(
                &shared,
                &identity,
                &channel,
                Payload::Error {
                    error: bridge_error(error),
                    retryable,
                },
            );
            return;
        }
    };
    {
        let mut inner = lock(&shared);
        if !current(&inner, &identity) {
            return;
        }
        inner
            .consumer
            .as_mut()
            .expect("current consumer")
            .subscription_id = Some(subscription.ready().subscription_id.clone());
    }
    if !send(
        &shared,
        &identity,
        &channel,
        Payload::Ready(subscription.ready().clone()),
    ) {
        return;
    }
    loop {
        match subscription.next_notification().await {
            Ok(EventNotification::Batch(batch)) => {
                let batch_id = batch.batch_id;
                {
                    let mut inner = lock(&shared);
                    if !current(&inner, &identity) {
                        return;
                    }
                    inner
                        .consumer
                        .as_mut()
                        .expect("current consumer")
                        .pending_batch = Some(batch_id);
                }
                if !send(&shared, &identity, &channel, Payload::Batch(batch)) {
                    return;
                }
                let waiting = Arc::clone(&shared);
                let expected = identity.clone();
                // At most one bounded blocking ACK wait exists for this GUI.
                // Cancellation wakes it immediately, including window shutdown.
                match tauri::async_runtime::spawn_blocking(move || {
                    wait_ack(&waiting, &expected, batch_id, ACK_TIMEOUT)
                })
                .await
                {
                    Ok(Acknowledged::Accepted) => {}
                    Ok(Acknowledged::Cancelled) => return,
                    _ => {
                        send(
                            &shared,
                            &identity,
                            &channel,
                            Payload::Error {
                                error: RpcError::extension("event_consumer_timeout"),
                                retryable: true,
                            },
                        );
                        return;
                    }
                }
            }
            Ok(EventNotification::Closed(closed)) => {
                send(&shared, &identity, &channel, Payload::Closed(closed));
                return;
            }
            Err(error) => {
                let retryable = error.is_retryable();
                send(
                    &shared,
                    &identity,
                    &channel,
                    Payload::Error {
                        error: bridge_error(error),
                        retryable,
                    },
                );
                return;
            }
        }
    }
}

fn bridge_error(error: alcomd_client::ClientError) -> RpcError {
    match error {
        alcomd_client::ClientError::InvalidResponse => RpcError::extension("event_protocol_error"),
        alcomd_client::ClientError::ReadTimeout => RpcError::extension("event_transport_timeout"),
        error => super::client_error(error),
    }
}

#[tauri::command]
pub(crate) async fn gui_events_start(
    state: State<'_, EventsClient>,
    consumer_id: String,
    generation: u64,
    cursor: Option<EventsCursor>,
    channel: Channel<Message>,
) -> Result<(), RpcError> {
    if consumer_id.is_empty()
        || consumer_id.len() > 128
        || generation == 0
        || generation > 9_007_199_254_740_991
    {
        return Err(RpcError::invalid_request());
    }
    let identity = Identity {
        consumer_id,
        generation,
    };
    let (old, replaced) = {
        let mut inner = lock(&state.shared);
        if inner.consumer.as_ref().is_some_and(|consumer| {
            consumer.identity.consumer_id == identity.consumer_id
                && consumer.identity.generation > generation
        }) {
            return Ok(());
        }
        if current(&inner, &identity) {
            return Err(RpcError::invalid_request());
        }
        let old = inner.task.take();
        let replaced = inner.consumer.as_mut().and_then(|consumer| {
            consumer
                .channel
                .take()
                .map(|channel| (consumer.identity.clone(), channel))
        });
        inner.consumer = Some(Consumer {
            identity: identity.clone(),
            channel: Some(channel.clone()),
            subscription_id: None,
            pending_batch: None,
            accepted_batch: 0,
        });
        state.shared.changed.notify_all();
        (old, replaced)
    };
    // A delayed start from a discarded WebView cannot silently strand the
    // currently visible consumer. Its controller can re-establish its lease.
    if let Some((identity, channel)) = replaced {
        let _ = channel.send(Message {
            identity,
            payload: Payload::Error {
                error: RpcError::extension("event_subscription_replaced"),
                retryable: true,
            },
        });
    }
    if let Some(old) = old {
        old.abort();
        let _ = old.await;
    }
    let task = tauri::async_runtime::spawn(run(
        Arc::clone(&state.shared),
        identity.clone(),
        cursor,
        channel,
    ));
    let unused = {
        let mut inner = lock(&state.shared);
        if current(&inner, &identity) {
            inner.task = Some(task);
            None
        } else {
            Some(task)
        }
    };
    if let Some(task) = unused {
        task.abort();
        let _ = task.await;
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn gui_events_ack(
    state: State<'_, EventsClient>,
    consumer_id: String,
    generation: u64,
    subscription_id: String,
    batch_id: u64,
) -> Result<(), RpcError> {
    let identity = Identity {
        consumer_id,
        generation,
    };
    let result = state
        .shared
        .acknowledge(&identity, &subscription_id, batch_id);
    if result.is_err()
        && let Some(task) = state.shared.stop(Some(&identity))
    {
        task.abort();
    }
    result
}

#[tauri::command]
pub(crate) async fn gui_events_cancel(
    state: State<'_, EventsClient>,
    consumer_id: String,
    generation: u64,
) -> Result<(), RpcError> {
    if let Some(task) = state.shared.stop(Some(&Identity {
        consumer_id,
        generation,
    })) {
        task.abort();
        let _ = task.await;
    }
    Ok(())
}

pub(crate) fn on_window_event(window: &Window, event: &WindowEvent) {
    if window.label() == "main"
        && matches!(event, WindowEvent::Destroyed)
        && let Some(state) = window.try_state::<EventsClient>()
        && let Some(task) = state.shared.stop(None)
    {
        task.abort();
    }
}

pub(crate) fn on_page_load(
    webview: &tauri::Webview,
    payload: &tauri::webview::PageLoadPayload<'_>,
) {
    if webview.label() == "main"
        && matches!(payload.event(), tauri::webview::PageLoadEvent::Started)
        && let Some(state) = webview.try_state::<EventsClient>()
        && let Some(task) = state.shared.stop(None)
    {
        task.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pending() -> (Arc<Shared>, Identity) {
        let shared = Arc::new(Shared::default());
        let identity = Identity {
            consumer_id: "webview".into(),
            generation: 2,
        };
        lock(&shared).consumer = Some(Consumer {
            identity: identity.clone(),
            channel: None,
            subscription_id: Some("subscription".into()),
            pending_batch: Some(1),
            accepted_batch: 0,
        });
        (shared, identity)
    }

    #[test]
    fn stale_ack_and_cancel_cannot_release_a_new_generation() {
        let (shared, identity) = pending();
        let old = Identity {
            generation: 1,
            ..identity.clone()
        };
        assert!(shared.acknowledge(&identity, "subscription", 0).is_err());
        assert!(shared.acknowledge(&old, "subscription", 1).is_ok());
        assert!(shared.stop(Some(&old)).is_none());
        assert_eq!(
            lock(&shared)
                .consumer
                .as_ref()
                .expect("current consumer")
                .pending_batch,
            Some(1)
        );
        assert!(shared.acknowledge(&identity, "subscription", 1).is_ok());
        assert_eq!(
            wait_ack(&shared, &identity, 1, Duration::ZERO),
            Acknowledged::Accepted
        );
        assert!(shared.acknowledge(&identity, "subscription", 1).is_ok());
        assert!(shared.acknowledge(&identity, "subscription", 3).is_err());
    }

    #[test]
    fn stalled_consumers_time_out_and_cancellation_wakes_waiters() {
        let (shared, identity) = pending();
        assert_eq!(
            wait_ack(&shared, &identity, 1, Duration::from_millis(2)),
            Acknowledged::TimedOut
        );
        let waiting = Arc::clone(&shared);
        let expected = identity.clone();
        let waiter =
            std::thread::spawn(move || wait_ack(&waiting, &expected, 1, Duration::from_secs(10)));
        shared.stop(Some(&identity));
        assert_eq!(
            waiter.join().expect("acknowledgement waiter"),
            Acknowledged::Cancelled
        );
    }
}
