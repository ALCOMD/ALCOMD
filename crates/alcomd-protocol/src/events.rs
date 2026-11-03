//! Dedicated live Event subscription contract. Notifications never carry a response ID.

use serde::{Deserialize, Serialize};

use crate::{Event, RpcError};

pub const METHOD_EVENTS_SUBSCRIBE: &str = "events.subscribe";
pub const CAPABILITY_EVENTS_LIVE_V1: &str = "events.live.v1";
pub const NOTIFICATION_EVENTS_BATCH: &str = "events.batch";
pub const NOTIFICATION_EVENTS_CLOSED: &str = "events.closed";
pub const DEFAULT_EVENT_BATCH_LIMIT: u32 = 100;
pub const MAX_EVENT_BATCH_LIMIT: u32 = 1000;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventsCursor {
    pub instance_id: String,
    pub sequence: u64,
    /// Disposable resource invalidation watermark; not a durable Event sequence.
    pub resources_revision: u64,
}

/// The single non-durable resource category required by current M6 writes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EventInvalidation {
    Extensions,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventsSubscribeParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<EventsCursor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventsSubscribeResult {
    pub subscription_id: String,
    pub instance_id: String,
    pub baseline_cursor: EventsCursor,
    pub resync_required: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventsBatch {
    pub subscription_id: String,
    pub instance_id: String,
    pub batch_id: u64,
    pub events: Vec<Event>,
    pub invalidated_scopes: Vec<EventInvalidation>,
    pub next_cursor: EventsCursor,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventsClosed {
    pub subscription_id: String,
    pub instance_id: String,
    pub error: RpcError,
    pub retryable: bool,
}

/// A distinct server notification envelope; an `id`, including null, is invalid.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NotificationEnvelope<T> {
    pub method: String,
    pub params: T,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", deny_unknown_fields)]
pub enum EventNotification {
    #[serde(rename = "events.batch")]
    Batch(EventsBatch),
    #[serde(rename = "events.closed")]
    Closed(EventsClosed),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn notifications_are_separate_from_id_bearing_responses() {
        let vectors: serde_json::Value = serde_json::from_str(include_str!(
            "../../../specs/rpc/m7-events-live.contract-vectors.json"
        ))
        .expect("vectors");
        let wire = vectors["batch"].clone();
        assert!(serde_json::from_value::<EventNotification>(wire.clone()).is_ok());
        assert!(serde_json::from_value::<crate::Response<EventsBatch>>(wire.clone()).is_err());
        for id in [json!(null), json!("request-1")] {
            let mut invalid = wire.clone();
            invalid["id"] = id;
            assert!(serde_json::from_value::<EventNotification>(invalid.clone()).is_err());
            assert!(
                serde_json::from_value::<NotificationEnvelope<serde_json::Value>>(invalid).is_err()
            );
        }
        let mut unknown = wire;
        unknown["method"] = json!("events.unknown");
        assert!(serde_json::from_value::<EventNotification>(unknown).is_err());
    }
}
