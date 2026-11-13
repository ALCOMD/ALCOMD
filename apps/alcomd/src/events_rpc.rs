//! Dedicated, bounded live Event delivery. Store work never waits on this socket.

use std::{io, time::Duration};

use alcomd_application as app;
use alcomd_protocol as rpc;
use alcomd_store::StateStoreHandle;
use serde::Serialize;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::{AccessContext, ConnectionState, DispatchAction, M2Application};

type Subscription = app::EventSubscription<StateStoreHandle>;
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);

pub(super) fn dispatch(
    request: rpc::RequestEnvelope,
    state: &ConnectionState,
    application: &M2Application,
    access: &AccessContext,
) -> DispatchAction {
    if let Some(action) =
        super::require_capability(&request.id, state, rpc::CAPABILITY_EVENTS_LIVE_V1)
    {
        return action;
    }
    let params: rpc::EventsSubscribeParams = match serde_json::from_value(request.params) {
        Ok(value) => value,
        Err(_) => return super::invalid(request.id),
    };
    let cursor = params.cursor.map(|cursor| app::EventsCursor {
        instance_id: cursor.instance_id,
        sequence: cursor.sequence,
        resources_revision: cursor.resources_revision,
    });
    match application.subscribe_events(
        access,
        cursor,
        params.limit.unwrap_or(rpc::DEFAULT_EVENT_BATCH_LIMIT),
        state.capabilities.contains(rpc::CAPABILITY_LOGS_READ_V1),
    ) {
        Ok(subscription) => {
            let mut action = super::success_action(
                request.id,
                rpc::EventsSubscribeResult {
                    subscription_id: subscription.subscription_id.clone(),
                    instance_id: subscription.instance_id.clone(),
                    baseline_cursor: rpc::EventsCursor {
                        instance_id: subscription.baseline_cursor.instance_id.clone(),
                        sequence: subscription.baseline_cursor.sequence,
                        resources_revision: subscription.baseline_cursor.resources_revision,
                    },
                    resync_required: subscription.resync_required,
                },
                None,
            );
            action.event_subscription = Some(subscription);
            action
        }
        Err(error) => super::application_error(request.id, error),
    }
}

pub(super) async fn serve(
    stream: &mut super::IpcStream,
    subscription: Subscription,
) -> io::Result<()> {
    let (reader, writer) = tokio::io::split(stream);
    deliver(reader, writer, subscription).await
}

async fn deliver<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    mut reader: R,
    mut writer: W,
    mut subscription: Subscription,
) -> io::Result<()> {
    let mut batch_id = 0_u64;
    let mut sent_through = subscription.baseline_cursor.sequence;
    loop {
        let mut byte = [0_u8; 1];
        // Only inspect upstream input between complete output frames. Cancelling
        // a partially written batch to append an error would corrupt framing.
        let next = tokio::select! {
            biased;
            read = reader.read(&mut byte) => match read? {
                0 => return Ok(()),
                _ => return close(&mut writer, &subscription, rpc::RpcError::invalid_request(), false).await,
            },
            page = subscription.next_page() => page,
        };
        let page = match next {
            Ok(page) => page,
            Err(error) => {
                let error = super::application_rpc_error(error);
                let retryable = error.code == "store_unavailable";
                return close(&mut writer, &subscription, error, retryable).await;
            }
        };
        let events = match page
            .events
            .into_iter()
            .map(super::event_to_rpc)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(events) => events,
            Err(error) => return close(&mut writer, &subscription, error, false).await,
        };
        batch_id = batch_id
            .checked_add(1)
            .ok_or_else(|| io::Error::other("event batch identity exhausted"))?;
        let invalidations = page
            .invalidated_scopes
            .iter()
            .map(|scope| match scope {
                app::EventInvalidation::Extensions => rpc::EventInvalidation::Extensions,
                app::EventInvalidation::OperationLog => rpc::EventInvalidation::OperationLog,
            })
            .collect::<Vec<_>>();
        let metadata = BatchMetadata {
            subscription_id: &subscription.subscription_id,
            instance_id: &subscription.instance_id,
            batch_id,
            previous_sequence: sent_through,
            resources_revision: page.resources_revision,
            invalidated_scopes: &invalidations,
        };
        let (frame, sent_sequence) = match batch_frame(&metadata, &events) {
            Ok(frame) => frame,
            Err(error) => return close(&mut writer, &subscription, error, false).await,
        };
        send(&mut writer, &frame).await?;
        // Only the prefix encoded and written above is acknowledged here. A
        // client resumes from its own accepted cursor, never this sent cursor.
        subscription
            .advance(sent_sequence, page.resources_revision)
            .map_err(|_| io::Error::other("invalid event delivery progress"))?;
        sent_through = sent_sequence;
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Batch<'a> {
    subscription_id: &'a str,
    instance_id: &'a str,
    batch_id: u64,
    events: &'a [rpc::Event],
    invalidated_scopes: &'a [rpc::EventInvalidation],
    next_cursor: rpc::EventsCursor,
}

struct BatchMetadata<'a> {
    subscription_id: &'a str,
    instance_id: &'a str,
    batch_id: u64,
    previous_sequence: u64,
    resources_revision: u64,
    invalidated_scopes: &'a [rpc::EventInvalidation],
}

fn encode_batch(
    metadata: &BatchMetadata<'_>,
    events: &[rpc::Event],
) -> Result<Vec<u8>, rpc::RpcError> {
    serde_json::to_vec(&rpc::NotificationEnvelope {
        method: rpc::NOTIFICATION_EVENTS_BATCH.to_owned(),
        params: Batch {
            subscription_id: metadata.subscription_id,
            instance_id: metadata.instance_id,
            batch_id: metadata.batch_id,
            events,
            invalidated_scopes: metadata.invalidated_scopes,
            next_cursor: rpc::EventsCursor {
                instance_id: metadata.instance_id.to_owned(),
                sequence: events
                    .last()
                    .map_or(metadata.previous_sequence, |last| last.sequence),
                resources_revision: metadata.resources_revision,
            },
        },
    })
    .map_err(|_| internal())
}

/// The full envelope, cursor, UTF-8 and escaped payload count toward the limit.
fn batch_frame(
    metadata: &BatchMetadata<'_>,
    events: &[rpc::Event],
) -> Result<(Vec<u8>, u64), rpc::RpcError> {
    if (events.is_empty() && metadata.invalidated_scopes.is_empty())
        || events.len() > rpc::MAX_EVENT_BATCH_LIMIT as usize
    {
        return Err(internal());
    }
    let mut lower = 0;
    let mut upper = events.len();
    let mut payload = encode_batch(metadata, events)?;
    if payload.len() <= rpc::MAX_FRAME_PAYLOAD_BYTES {
        lower = events.len();
    } else {
        while lower < upper {
            let middle = lower + (upper - lower).div_ceil(2);
            let encoded = encode_batch(metadata, &events[..middle])?;
            if encoded.len() <= rpc::MAX_FRAME_PAYLOAD_BYTES {
                lower = middle;
                payload = encoded;
            } else {
                upper = middle - 1;
            }
        }
    }
    if lower == 0 {
        if metadata.invalidated_scopes.is_empty() {
            return Err(internal());
        }
        payload = encode_batch(metadata, &[])?;
    }
    Ok((
        rpc::encode_frame(&payload).map_err(|_| internal())?,
        events[..lower]
            .last()
            .map_or(metadata.previous_sequence, |event| event.sequence),
    ))
}

async fn send<W: AsyncWrite + Unpin>(writer: &mut W, frame: &[u8]) -> io::Result<()> {
    tokio::time::timeout(WRITE_TIMEOUT, async {
        writer.write_all(frame).await?;
        writer.flush().await
    })
    .await
    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "event send timed out"))?
}

async fn close<W: AsyncWrite + Unpin>(
    writer: &mut W,
    subscription: &Subscription,
    error: rpc::RpcError,
    retryable: bool,
) -> io::Result<()> {
    let payload = serde_json::to_vec(&rpc::EventNotification::Closed(rpc::EventsClosed {
        subscription_id: subscription.subscription_id.clone(),
        instance_id: subscription.instance_id.clone(),
        error,
        retryable,
    }))
    .map_err(io::Error::other)?;
    let frame = rpc::encode_frame(&payload).map_err(io::Error::other)?;
    send(writer, &frame).await
}

fn internal() -> rpc::RpcError {
    rpc::RpcError::internal(app::OperationId::new().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(batch_id: u64) -> BatchMetadata<'static> {
        BatchMetadata {
            subscription_id: "subscription",
            instance_id: "instance",
            batch_id,
            previous_sequence: 0,
            resources_revision: 0,
            invalidated_scopes: &[],
        }
    }

    fn event(sequence: u64, bytes: usize) -> rpc::Event {
        rpc::Event {
            sequence,
            event_id: app::OperationId::new().to_string(),
            kind: "operation.progress".into(),
            aggregate_kind: "operation".into(),
            aggregate_id: app::OperationId::new().to_string(),
            aggregate_revision: sequence,
            occurred_at_ms: 1,
            payload: serde_json::json!({"detail":"\\\"".repeat(bytes)}),
        }
    }

    #[test]
    fn encoded_limit_splits_a_prefix_without_skipping_the_remaining_events() {
        let events = vec![event(2, 600_000), event(9, 600_000), event(12, 600_000)];
        let (frame, sent) = batch_frame(&metadata(1), &events).expect("first prefix");
        assert!(frame.len() - 4 <= rpc::MAX_FRAME_PAYLOAD_BYTES);
        let rpc::EventNotification::Batch(batch) =
            serde_json::from_slice(&frame[4..]).expect("notification")
        else {
            panic!("batch")
        };
        assert!(!batch.events.is_empty());
        assert!(batch.events.len() < events.len());
        assert_eq!(sent, batch.next_cursor.sequence);
        let remaining = events
            .into_iter()
            .filter(|event| event.sequence > sent)
            .collect::<Vec<_>>();
        let (_, next) = batch_frame(&metadata(2), &remaining).expect("remaining prefix");
        assert!(next > sent);
    }

    #[test]
    fn an_oversized_single_event_is_an_error_not_an_empty_or_truncated_batch() {
        assert!(batch_frame(&metadata(1), &[event(1, rpc::MAX_FRAME_PAYLOAD_BYTES)]).is_err());
        assert!(batch_frame(&metadata(1), &[]).is_err());
    }

    #[test]
    fn a_real_resource_invalidation_preserves_the_durable_event_cursor() {
        let scopes = [rpc::EventInvalidation::Extensions];
        let metadata = BatchMetadata {
            previous_sequence: 17,
            resources_revision: 4,
            invalidated_scopes: &scopes,
            ..metadata(1)
        };
        let (frame, sequence) = batch_frame(&metadata, &[]).expect("real resource invalidation");
        let rpc::EventNotification::Batch(batch) =
            serde_json::from_slice(&frame[4..]).expect("typed batch")
        else {
            panic!("batch")
        };
        assert!(batch.events.is_empty());
        assert_eq!(sequence, 17);
        assert_eq!(batch.next_cursor.sequence, 17);
        assert_eq!(batch.next_cursor.resources_revision, 4);
        assert_eq!(batch.invalidated_scopes, scopes);
    }

    fn application() -> M2Application {
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/m7-events-rpc-tests")
            .join(app::OperationId::new().to_string());
        std::fs::create_dir_all(&directory).expect("owned test artifact directory");
        M2Application::new(
            StateStoreHandle::open(directory.join("state.db")).expect("isolated store"),
        )
    }

    #[tokio::test]
    async fn permission_rejection_remains_an_id_bearing_response() {
        let application = application();
        let state = ConnectionState {
            mcp_hash: None,
            connection_id: "permission-test".into(),
            client_instance_id: Some("test".into()),
            actor: "Local IPC".to_owned(),
            handshake_complete: true,
            capabilities: [rpc::CAPABILITY_EVENTS_LIVE_V1.to_owned()].into(),
        };
        let access = app::AccessContext::new(app::PrincipalId::local_owner(), []);
        let action = dispatch(
            rpc::RequestEnvelope {
                id: "denied".into(),
                method: rpc::METHOD_EVENTS_SUBSCRIBE.into(),
                params: serde_json::json!({}),
            },
            &state,
            &application,
            &access,
        );
        assert_eq!(action.response["id"], "denied");
        assert_eq!(action.response["error"]["code"], "permission_denied");
        assert!(action.event_subscription.is_none());
        assert!(action.response.get("method").is_none());
    }

    #[tokio::test]
    async fn unexpected_subscription_input_closes_with_a_permanent_notification() {
        let application = application();
        let subscription = application
            .subscribe_events(&AccessContext::local_owner(), None, 100, false)
            .expect("subscription");
        let (mut client, server) = tokio::io::duplex(1024);
        let (reader, writer) = tokio::io::split(server);
        let delivering = tokio::spawn(deliver(reader, writer, subscription));
        client
            .write_all(b"X")
            .await
            .expect("upstream protocol violation");
        let mut prefix = [0; 4];
        client
            .read_exact(&mut prefix)
            .await
            .expect("notification prefix");
        let mut payload = vec![0; rpc::decode_frame_length(prefix).expect("bounded frame")];
        client
            .read_exact(&mut payload)
            .await
            .expect("notification body");
        let value: serde_json::Value = serde_json::from_slice(&payload).expect("JSON");
        assert!(value.get("id").is_none());
        let rpc::EventNotification::Closed(closed) =
            serde_json::from_value(value).expect("closed envelope")
        else {
            panic!("closed")
        };
        assert!(!closed.retryable);
        assert_eq!(closed.error.code, "invalid_request");
        delivering
            .await
            .expect("delivery task")
            .expect("clean close");
    }

    #[tokio::test]
    async fn a_proven_stalled_subscriber_does_not_gate_another_command_or_store_read() {
        let application = application();
        let access = AccessContext::local_owner();
        let subscription = application
            .subscribe_events(&AccessContext::local_owner(), None, 100, false)
            .expect("subscription");
        let (mut client, server) = tokio::io::duplex(16);
        let (reader, writer) = tokio::io::split(server);
        let delivering = tokio::spawn(deliver(reader, writer, subscription));
        application
            .state_check(
                &AccessContext::local_owner(),
                app::IdempotencyKey::parse("first-backpressure-task").expect("test key"),
            )
            .await
            .expect("first command");
        let mut first = [0; 1];
        tokio::time::timeout(Duration::from_secs(2), client.read_exact(&mut first))
            .await
            .expect("writer started")
            .expect("first frame byte");
        // The notification cannot fit in 16 bytes and no further bytes are read.
        assert!(!delivering.is_finished());
        let command = application.state_check(
            &access,
            app::IdempotencyKey::parse("independent-backpressure-task").expect("test key"),
        );
        let accepted = tokio::time::timeout(Duration::from_secs(2), command)
            .await
            .expect("command is not network gated")
            .expect("second command");
        tokio::time::timeout(
            Duration::from_secs(2),
            application.get_operation(&AccessContext::local_owner(), accepted.operation_id),
        )
        .await
        .expect("store read is not network gated")
        .expect("owned operation");
        delivering.abort();
        let _ = delivering.await;
    }
}
