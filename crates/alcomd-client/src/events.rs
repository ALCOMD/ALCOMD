use alcomd_platform::IpcStream;
use alcomd_protocol::{
    EventNotification, EventsBatch, EventsSubscribeParams, EventsSubscribeResult,
    MAX_EVENT_BATCH_LIMIT, METHOD_EVENTS_SUBSCRIBE,
};

use super::{AlcomdClient, ClientConfig, ClientError};
use tokio::io::{AsyncRead, AsyncReadExt};

/// One dedicated authenticated stream. The ordinary RPC client remains request/response only.
pub struct EventSubscription {
    stream: IpcStream,
    state: SubscriptionState,
    usable: bool,
}

impl EventSubscription {
    /// Connects, negotiates and receives the correlated ready response within five seconds.
    pub async fn connect(
        config: ClientConfig,
        params: EventsSubscribeParams,
    ) -> Result<Self, ClientError> {
        let connection = async {
            let mut client = AlcomdClient::connect(config).await?;
            let ready: EventsSubscribeResult =
                client.call(METHOD_EVENTS_SUBSCRIBE, params.clone()).await?;
            let state = SubscriptionState::new(ready, &params)?;
            Ok(Self {
                stream: client.stream,
                state,
                usable: true,
            })
        };
        tokio::time::timeout(std::time::Duration::from_secs(5), connection)
            .await
            .unwrap_or(Err(ClientError::ReadTimeout))
    }

    #[must_use]
    pub fn ready(&self) -> &EventsSubscribeResult {
        &self.state.ready
    }

    /// Waits for data indefinitely while healthy. A received cursor is not a consumer ACK.
    pub async fn next_notification(&mut self) -> Result<EventNotification, ClientError> {
        if !self.usable {
            return Err(ClientError::InvalidResponse);
        }
        self.usable = false;
        let payload =
            read_stream_frame(&mut self.stream, std::time::Duration::from_secs(10)).await?;
        let notification: EventNotification =
            serde_json::from_slice(&payload).map_err(|_| ClientError::InvalidResponse)?;
        self.state.accept(&notification)?;
        self.usable = matches!(&notification, EventNotification::Batch(_));
        Ok(notification)
    }
}

async fn read_stream_frame<R: AsyncRead + Unpin>(
    reader: &mut R,
    remainder_timeout: std::time::Duration,
) -> Result<Vec<u8>, ClientError> {
    // An idle stream has no deadline. Once a frame starts, a stalled peer must
    // not retain the subscriber forever with a partial prefix or payload.
    let mut prefix = [0_u8; 4];
    reader
        .read_exact(&mut prefix[..1])
        .await
        .map_err(ClientError::Transport)?;
    let remainder = async {
        reader
            .read_exact(&mut prefix[1..])
            .await
            .map_err(ClientError::Transport)?;
        let length = alcomd_protocol::decode_frame_length(prefix)
            .map_err(|_| ClientError::InvalidResponse)?;
        let mut payload = vec![0; length];
        reader
            .read_exact(&mut payload)
            .await
            .map_err(ClientError::Transport)?;
        Ok(payload)
    };
    tokio::time::timeout(remainder_timeout, remainder)
        .await
        .unwrap_or(Err(ClientError::ReadTimeout))
}

struct SubscriptionState {
    ready: EventsSubscribeResult,
    received_sequence: u64,
    received_resources_revision: u64,
    batch_id: u64,
    limit: usize,
}

impl SubscriptionState {
    fn new(
        ready: EventsSubscribeResult,
        params: &EventsSubscribeParams,
    ) -> Result<Self, ClientError> {
        let limit = params
            .limit
            .unwrap_or(alcomd_protocol::DEFAULT_EVENT_BATCH_LIMIT);
        if !(1..=MAX_EVENT_BATCH_LIMIT).contains(&limit)
            || !valid_identity(&ready.subscription_id)
            || !valid_identity(&ready.instance_id)
            || ready.baseline_cursor.instance_id != ready.instance_id
            || ready.baseline_cursor.sequence > i64::MAX as u64
            || ready.baseline_cursor.resources_revision > i64::MAX as u64
            || (!ready.resync_required && params.cursor.as_ref() != Some(&ready.baseline_cursor))
        {
            return Err(ClientError::InvalidResponse);
        }
        Ok(Self {
            received_sequence: ready.baseline_cursor.sequence,
            received_resources_revision: ready.baseline_cursor.resources_revision,
            ready,
            batch_id: 0,
            limit: limit as usize,
        })
    }

    fn accept(&mut self, message: &EventNotification) -> Result<(), ClientError> {
        match message {
            EventNotification::Batch(batch) => self.accept_batch(batch),
            EventNotification::Closed(closed) => {
                if closed.subscription_id != self.ready.subscription_id
                    || closed.instance_id != self.ready.instance_id
                    || closed.error.code.is_empty()
                    || closed.error.message.is_empty()
                    || (closed.retryable
                        && !ClientError::Remote(closed.error.clone()).is_retryable())
                {
                    return Err(ClientError::InvalidResponse);
                }
                Ok(())
            }
        }
    }

    fn accept_batch(&mut self, batch: &EventsBatch) -> Result<(), ClientError> {
        if batch.subscription_id != self.ready.subscription_id
            || batch.instance_id != self.ready.instance_id
            || batch.next_cursor.instance_id != self.ready.instance_id
            || batch.batch_id
                != self
                    .batch_id
                    .checked_add(1)
                    .ok_or(ClientError::InvalidResponse)?
            || batch.batch_id > 9_007_199_254_740_991
            || batch.events.len() > self.limit
            || batch.invalidated_scopes.len() > 2
            || (batch.invalidated_scopes.len() == 2
                && batch.invalidated_scopes[0] == batch.invalidated_scopes[1])
            || batch.next_cursor.resources_revision < self.received_resources_revision
            || batch.next_cursor.resources_revision > i64::MAX as u64
            || (batch.next_cursor.resources_revision > self.received_resources_revision
                && batch.invalidated_scopes.is_empty())
            || (!batch.invalidated_scopes.is_empty()
                && batch.next_cursor.resources_revision <= self.received_resources_revision)
            || (batch.events.is_empty() && batch.invalidated_scopes.is_empty())
        {
            return Err(ClientError::InvalidResponse);
        }
        let mut sequence = self.received_sequence;
        for event in &batch.events {
            if event.sequence <= sequence
                || event.sequence > i64::MAX as u64
                || !valid_identity(&event.event_id)
                || !valid_identity(&event.aggregate_id)
                || event.kind.is_empty()
                || event.kind.len() > 128
                || !matches!(
                    event.aggregate_kind.as_str(),
                    "operation"
                        | "project"
                        | "repository"
                        | "unity-installation"
                        | "project-editor-preference"
                        | "project-unity-launch-config"
                        | "template"
                        | "user-package"
                )
                || event.aggregate_revision == 0
                || event.aggregate_revision > i64::MAX as u64
                || event.occurred_at_ms > i64::MAX as u64
                || !event.payload.is_object()
            {
                return Err(ClientError::InvalidResponse);
            }
            sequence = event.sequence;
        }
        if sequence != batch.next_cursor.sequence {
            return Err(ClientError::InvalidResponse);
        }
        self.received_sequence = sequence;
        self.received_resources_revision = batch.next_cursor.resources_revision;
        self.batch_id = batch.batch_id;
        Ok(())
    }
}

fn valid_identity(value: &str) -> bool {
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
    use serde_json::{Value, json};

    fn vectors() -> Value {
        serde_json::from_str(include_str!(
            "../../../specs/rpc/m7-events-live.contract-vectors.json"
        ))
        .expect("vectors")
    }
    fn state() -> SubscriptionState {
        SubscriptionState::new(
            serde_json::from_value(vectors()["ready"]["result"].clone()).expect("ready"),
            &EventsSubscribeParams::default(),
        )
        .expect("new state")
    }
    #[test]
    fn owner_sequence_gaps_are_valid_but_duplicates_and_regressions_are_not() {
        let message: EventNotification =
            serde_json::from_value(vectors()["batch"].clone()).expect("batch");
        let mut state = state();
        state
            .accept(&message)
            .expect("40 to 43 is a valid owner gap");
        assert!(state.accept(&message).is_err());
        assert_eq!(state.received_sequence, 43);
    }

    #[test]
    fn resource_only_batches_advance_a_separate_watermark_without_fake_events() {
        let message: EventNotification = serde_json::from_value(vectors()["resourceBatch"].clone())
            .expect("resource invalidation");
        let mut state = state();
        state
            .accept(&message)
            .expect("extension invalidation without durable events");
        assert_eq!(state.received_sequence, 40);
        assert_eq!(state.received_resources_revision, 1);
        let mut duplicate = vectors()["resourceBatch"].clone();
        duplicate["params"]["batchId"] = json!(2);
        assert!(
            state
                .accept(&serde_json::from_value(duplicate).expect("typed duplicate invalidation"))
                .is_err()
        );
        let mut data = vectors()["batch"].clone();
        data["params"]["batchId"] = json!(2);
        data["params"]["nextCursor"]["resourcesRevision"] = json!(1);
        state
            .accept(&serde_json::from_value(data).expect("data after invalidation"))
            .expect("durable sequence advances independently");
        assert_eq!(state.received_sequence, 43);
        assert_eq!(state.received_resources_revision, 1);
    }

    #[test]
    fn lost_resource_invalidation_requires_resync_and_resource_watermarks_never_regress() {
        let mut ready: EventsSubscribeResult =
            serde_json::from_value(vectors()["ready"]["result"].clone()).expect("ready");
        let old_cursor = ready.baseline_cursor.clone();
        ready.baseline_cursor.resources_revision = 1;
        ready.resync_required = false;
        let params = EventsSubscribeParams {
            cursor: Some(old_cursor),
            limit: None,
        };
        assert!(SubscriptionState::new(ready.clone(), &params).is_err());
        ready.resync_required = true;
        let mut subscription_state =
            SubscriptionState::new(ready, &params).expect("explicit resource resync");
        let data: EventNotification =
            serde_json::from_value(vectors()["batch"].clone()).expect("older data cursor");
        assert!(
            subscription_state.accept(&data).is_err(),
            "resource revision must not go backwards"
        );
        let mut lost_scope = vectors()["batch"].clone();
        lost_scope["params"]["nextCursor"]["resourcesRevision"] = json!(2);
        assert!(
            subscription_state
                .accept(&serde_json::from_value(lost_scope).expect("typed missing invalidation"))
                .is_err()
        );
        let mut unknown_scope = vectors()["resourceBatch"].clone();
        unknown_scope["params"]["invalidatedScopes"] = json!(["projects"]);
        assert!(serde_json::from_value::<EventNotification>(unknown_scope).is_err());
        let mut empty = vectors()["resourceBatch"].clone();
        empty["params"]["invalidatedScopes"] = json!([]);
        assert!(
            state()
                .accept(&serde_json::from_value(empty).expect("typed empty batch"))
                .is_err()
        );
        let mut mixed = vectors()["batch"].clone();
        mixed["params"]["invalidatedScopes"] = json!(["extensions"]);
        assert!(
            state()
                .accept(&serde_json::from_value(mixed.clone()).expect("typed unchanged scope"))
                .is_err(),
            "a mixed batch must advance the invalidated resource watermark too"
        );
        mixed["params"]["nextCursor"]["resourcesRevision"] = json!(1);
        state()
            .accept(&serde_json::from_value(mixed).expect("typed mixed batch"))
            .expect("durable events and resource invalidation may share a batch");
    }
    #[test]
    fn stream_identity_order_cursor_and_nonempty_bound_are_validated() {
        for (field, value) in [
            ("subscriptionId", json!("different")),
            ("instanceId", json!("different")),
            ("batchId", json!(2)),
            ("events", json!([])),
            (
                "nextCursor",
                json!({"instanceId":"00000000-0000-4000-8000-000000000002","sequence":44,"resourcesRevision":0}),
            ),
        ] {
            let mut wire = vectors()["batch"].clone();
            wire["params"][field] = value;
            let message: EventNotification =
                serde_json::from_value(wire).expect("typed malformed frame");
            assert!(state().accept(&message).is_err(), "{field}");
        }
        for id in [json!(null), json!("request-1")] {
            let mut wire = vectors()["batch"].clone();
            wire["id"] = id;
            assert!(serde_json::from_value::<EventNotification>(wire).is_err());
        }
    }

    #[test]
    fn durable_event_aggregates_follow_the_canonical_schema_without_extension_events() {
        for kind in [
            "operation",
            "project",
            "repository",
            "unity-installation",
            "project-editor-preference",
            "project-unity-launch-config",
            "template",
            "user-package",
        ] {
            let mut wire = vectors()["batch"].clone();
            wire["params"]["events"][0]["aggregateKind"] = json!(kind);
            state()
                .accept(&serde_json::from_value(wire).expect("typed durable aggregate"))
                .expect("current canonical aggregate kind");
        }
        for (field, value) in [
            ("aggregateKind", "extension"),
            ("aggregateKind", "unknown"),
            ("aggregateId", "not-a-resource-uuid"),
        ] {
            let mut wire = vectors()["batch"].clone();
            wire["params"]["events"][0][field] = json!(value);
            assert!(
                state()
                    .accept(&serde_json::from_value(wire).expect("typed invalid aggregate"))
                    .is_err(),
                "{field} {value}"
            );
        }
    }

    #[test]
    fn explicit_permanent_close_and_unknown_frames_never_become_empty_data() {
        let closed: EventNotification =
            serde_json::from_value(vectors()["closed"].clone()).expect("closed");
        state().accept(&closed).expect("explicit close");
        let mut invalid = vectors()["closed"].clone();
        invalid["params"]["retryable"] = json!(true);
        assert!(
            state()
                .accept(&serde_json::from_value(invalid).expect("typed close"))
                .is_err()
        );
        let mut unknown = vectors()["batch"].clone();
        unknown["method"] = json!("events.unknown");
        assert!(serde_json::from_value::<EventNotification>(unknown).is_err());
        assert!(!ClientError::InvalidResponse.is_retryable());
        assert!(
            !ClientError::Remote(alcomd_protocol::RpcError::permission_denied()).is_retryable()
        );
        assert!(ClientError::ReadTimeout.is_retryable());
    }
    #[test]
    fn reads_are_bounded_and_writes_never_acquire_read_retry_deadlines() {
        for method in [
            "operations.get",
            "repositories.packages",
            "packages.queryProjectCandidates",
            "unity.writerState",
            "unity.launchOptions",
        ] {
            assert_eq!(
                super::super::read_deadline(method),
                Some(std::time::Duration::from_secs(10)),
                "{method}"
            );
        }
        assert_eq!(super::super::read_deadline("state.check"), None);
        assert_eq!(super::super::read_deadline("operations.cancel"), None);
    }

    #[test]
    fn ready_and_rejected_requests_require_the_original_response_id() {
        let ready = serde_json::to_vec(&vectors()["ready"]).expect("ready response");
        let request_id = vectors()["ready"]["id"]
            .as_str()
            .expect("request id")
            .to_owned();
        super::super::decode_response::<EventsSubscribeResult>(&ready, &request_id)
            .expect("correlated ready");
        assert!(matches!(
            super::super::decode_response::<EventsSubscribeResult>(&ready, "different"),
            Err(ClientError::InvalidResponse)
        ));
        for returned_id in [json!(null), json!("different"), json!(request_id)] {
            let payload = serde_json::to_vec(&json!({
                "id": returned_id,
                "error": alcomd_protocol::RpcError::permission_denied()
            }))
            .expect("error response");
            let result =
                super::super::decode_response::<EventsSubscribeResult>(&payload, &request_id);
            if returned_id == request_id {
                assert!(matches!(result, Err(ClientError::Remote(_))));
            } else {
                assert!(matches!(result, Err(ClientError::InvalidResponse)));
            }
        }
    }

    #[tokio::test]
    async fn idle_streams_wait_but_started_frames_time_out_without_becoming_empty_data() {
        use std::time::Duration;
        use tokio::io::AsyncWriteExt;
        let (mut sender, mut reader) = tokio::io::duplex(32);
        assert!(
            tokio::time::timeout(
                Duration::from_millis(15),
                read_stream_frame(&mut reader, Duration::from_millis(1))
            )
            .await
            .is_err()
        );
        sender.write_all(&[8]).await.expect("first byte");
        assert!(matches!(
            read_stream_frame(&mut reader, Duration::from_millis(1)).await,
            Err(ClientError::ReadTimeout)
        ));
        let (mut sender, mut reader) = tokio::io::duplex(32);
        sender
            .write_all(&((alcomd_protocol::MAX_FRAME_PAYLOAD_BYTES + 1) as u32).to_le_bytes())
            .await
            .expect("oversize prefix");
        assert!(matches!(
            read_stream_frame(&mut reader, Duration::from_millis(10)).await,
            Err(ClientError::InvalidResponse)
        ));
    }
}
