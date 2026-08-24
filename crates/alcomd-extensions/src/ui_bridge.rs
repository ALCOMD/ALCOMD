use std::collections::{HashSet, VecDeque};

use alcomd_application::OperationId;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_MESSAGE_BYTES: usize = 262_144;
const MAX_REQUESTS_PER_WINDOW: usize = 30;
const RATE_WINDOW_MS: u64 = 60_000;
const BURST: f64 = 10.0;
const TOKEN_REFILL_PER_MS: f64 = 1.0 / 2_000.0;
const MAX_CONCURRENT: usize = 8;
const MAX_PENDING: usize = 64;
const IDLE_MS: u64 = 300_000;
const LIFETIME_MS: u64 = 3_600_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionUiOrigin {
    pub extension_id: String,
    pub package_digest: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UiBridgeBinding {
    pub origin: ExtensionUiOrigin,
    pub instance_id: String,
    pub principal_id: String,
    pub grant_revision: u64,
    pub lifecycle_generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BridgeRequest {
    pub bridge_version: u32,
    pub session_id: String,
    pub sequence: u64,
    pub request_id: String,
    pub method: String,
    pub params: Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BridgeAdmission {
    Ready,
    Queued,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BridgeError {
    Closed,
    OriginMismatch,
    InvalidEnvelope,
    Replay,
    RequestCollision,
    MessageTooLarge,
    RateLimited,
    PendingLimit,
    MethodDenied,
}

pub struct UiBridgeSession {
    session_id: String,
    binding: UiBridgeBinding,
    next_sequence: u64,
    created_at_ms: u64,
    last_activity_ms: u64,
    request_times: VecDeque<u64>,
    tokens: f64,
    token_updated_at_ms: u64,
    seen_request_ids: HashSet<String>,
    active: HashSet<String>,
    queued: VecDeque<String>,
    closed: bool,
}

impl UiBridgeSession {
    #[must_use]
    pub fn new(binding: UiBridgeBinding, now_ms: u64) -> Self {
        Self {
            session_id: OperationId::new().to_string(),
            binding,
            next_sequence: 1,
            created_at_ms: now_ms,
            last_activity_ms: now_ms,
            request_times: VecDeque::new(),
            tokens: BURST,
            token_updated_at_ms: now_ms,
            seen_request_ids: HashSet::new(),
            active: HashSet::new(),
            queued: VecDeque::new(),
            closed: false,
        }
    }

    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn accept_headless(
        &mut self,
        origin: &ExtensionUiOrigin,
        encoded: &[u8],
        now_ms: u64,
    ) -> Result<BridgeAdmission, BridgeError> {
        self.ensure_live(now_ms)?;
        if origin != &self.binding.origin {
            self.close();
            return Err(BridgeError::OriginMismatch);
        }
        if encoded.len() > MAX_MESSAGE_BYTES {
            self.close();
            return Err(BridgeError::MessageTooLarge);
        }
        let request: BridgeRequest =
            serde_json::from_slice(encoded).map_err(|_| BridgeError::InvalidEnvelope)?;
        if request.bridge_version != 1
            || request.session_id != self.session_id
            || request.sequence != self.next_sequence
            || request.sequence == 0
            || !valid_request_id(&request.request_id)
            || !valid_method(&request.method)
            || !request.params.is_object()
        {
            self.close();
            return Err(if request.sequence != self.next_sequence {
                BridgeError::Replay
            } else {
                BridgeError::InvalidEnvelope
            });
        }
        if request.method != "headless.test.ping" {
            self.close();
            return Err(BridgeError::MethodDenied);
        }
        if self.seen_request_ids.contains(&request.request_id) {
            self.close();
            return Err(BridgeError::RequestCollision);
        }
        self.consume_rate(now_ms)?;
        if self.active.len() + self.queued.len() >= MAX_PENDING {
            return Err(BridgeError::PendingLimit);
        }
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(BridgeError::InvalidEnvelope)?;
        self.last_activity_ms = now_ms;
        self.seen_request_ids.insert(request.request_id.clone());
        if self.active.len() < MAX_CONCURRENT {
            self.active.insert(request.request_id);
            Ok(BridgeAdmission::Ready)
        } else {
            self.queued.push_back(request.request_id);
            Ok(BridgeAdmission::Queued)
        }
    }

    pub fn complete(&mut self, request_id: &str, now_ms: u64) -> Result<(), BridgeError> {
        self.ensure_live(now_ms)?;
        if !self.active.remove(request_id) {
            return Err(BridgeError::InvalidEnvelope);
        }
        if let Some(next) = self.queued.pop_front() {
            self.active.insert(next);
        }
        self.last_activity_ms = now_ms;
        Ok(())
    }

    pub fn revoke(&mut self) {
        self.close();
    }

    fn consume_rate(&mut self, now_ms: u64) -> Result<(), BridgeError> {
        while self
            .request_times
            .front()
            .is_some_and(|value| now_ms.saturating_sub(*value) >= RATE_WINDOW_MS)
        {
            self.request_times.pop_front();
        }
        if self.request_times.len() >= MAX_REQUESTS_PER_WINDOW {
            return Err(BridgeError::RateLimited);
        }
        let elapsed = now_ms.saturating_sub(self.token_updated_at_ms);
        self.tokens = (self.tokens + elapsed as f64 * TOKEN_REFILL_PER_MS).min(BURST);
        self.token_updated_at_ms = now_ms;
        if self.tokens < 1.0 {
            return Err(BridgeError::RateLimited);
        }
        self.tokens -= 1.0;
        self.request_times.push_back(now_ms);
        Ok(())
    }

    fn ensure_live(&mut self, now_ms: u64) -> Result<(), BridgeError> {
        if self.closed
            || now_ms.saturating_sub(self.last_activity_ms) >= IDLE_MS
            || now_ms.saturating_sub(self.created_at_ms) >= LIFETIME_MS
        {
            self.close();
            return Err(BridgeError::Closed);
        }
        Ok(())
    }

    fn close(&mut self) {
        self.closed = true;
        self.active.clear();
        self.queued.clear();
    }
}

fn valid_request_id(value: &str) -> bool {
    (1..=64).contains(&value.len()) && value.bytes().all(|byte| (b'!'..=b'~').contains(&byte))
}

fn valid_method(value: &str) -> bool {
    (3..=128).contains(&value.len())
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (index > 0 && matches!(byte, b'.' | b'-'))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin(extension_id: &str, digest: u8) -> ExtensionUiOrigin {
        ExtensionUiOrigin {
            extension_id: extension_id.to_owned(),
            package_digest: [digest; 32],
        }
    }

    fn session(now_ms: u64) -> UiBridgeSession {
        UiBridgeSession::new(
            UiBridgeBinding {
                origin: origin("dev.example.fixture", 7),
                instance_id: "instance".to_owned(),
                principal_id: "principal".to_owned(),
                grant_revision: 1,
                lifecycle_generation: 1,
            },
            now_ms,
        )
    }

    fn request(
        session: &UiBridgeSession,
        sequence: u64,
        request_id: &str,
        method: &str,
    ) -> Vec<u8> {
        serde_json::to_vec(&BridgeRequest {
            bridge_version: 1,
            session_id: session.session_id().to_owned(),
            sequence,
            request_id: request_id.to_owned(),
            method: method.to_owned(),
            params: serde_json::json!({}),
        })
        .expect("serialize request")
    }

    #[test]
    fn logical_origin_replay_collision_and_private_methods_fail_closed() {
        let expected_origin = origin("dev.example.fixture", 7);
        let mut bridge = session(1_000);
        let wrong = request(&bridge, 1, "one", "headless.test.ping");
        assert_eq!(
            bridge.accept_headless(&origin("dev.example.other", 7), &wrong, 1_001),
            Err(BridgeError::OriginMismatch)
        );

        let mut bridge = session(2_000);
        let first = request(&bridge, 1, "same", "headless.test.ping");
        assert_eq!(
            bridge.accept_headless(&expected_origin, &first, 2_001),
            Ok(BridgeAdmission::Ready)
        );
        let replay = request(&bridge, 1, "other", "headless.test.ping");
        assert_eq!(
            bridge.accept_headless(&expected_origin, &replay, 2_002),
            Err(BridgeError::Replay)
        );

        let mut bridge = session(2_500);
        let first = request(&bridge, 1, "same", "headless.test.ping");
        bridge
            .accept_headless(&expected_origin, &first, 2_501)
            .expect("first request");
        let collision = request(&bridge, 2, "same", "headless.test.ping");
        assert_eq!(
            bridge.accept_headless(&expected_origin, &collision, 2_502),
            Err(BridgeError::RequestCollision)
        );

        let mut bridge = session(3_000);
        let private = request(&bridge, 1, "private", "tauri.invoke");
        assert_eq!(
            bridge.accept_headless(&expected_origin, &private, 3_001),
            Err(BridgeError::MethodDenied)
        );
    }

    #[test]
    fn size_rate_concurrency_revocation_and_expiry_are_bounded() {
        let expected_origin = origin("dev.example.fixture", 7);
        let mut bridge = session(10_000);
        assert_eq!(
            bridge.accept_headless(&expected_origin, &vec![b' '; MAX_MESSAGE_BYTES + 1], 10_001),
            Err(BridgeError::MessageTooLarge)
        );

        let mut bridge = session(20_000);
        for sequence in 1..=8 {
            let payload = request(
                &bridge,
                sequence,
                &format!("request-{sequence}"),
                "headless.test.ping",
            );
            assert_eq!(
                bridge.accept_headless(&expected_origin, &payload, 20_000 + sequence * 2_000),
                Ok(BridgeAdmission::Ready)
            );
        }
        let queued = request(&bridge, 9, "queued", "headless.test.ping");
        assert_eq!(
            bridge.accept_headless(&expected_origin, &queued, 38_000),
            Ok(BridgeAdmission::Queued)
        );
        bridge.complete("request-1", 39_000).expect("complete");
        bridge.revoke();
        assert_eq!(
            bridge.complete("request-2", 39_001),
            Err(BridgeError::Closed)
        );

        let mut expired = session(100);
        let payload = request(&expired, 1, "late", "headless.test.ping");
        assert_eq!(
            expired.accept_headless(&expected_origin, &payload, 300_100),
            Err(BridgeError::Closed)
        );

        let mut flooded = session(500_000);
        for sequence in 1..=10 {
            let payload = request(
                &flooded,
                sequence,
                &format!("burst-{sequence}"),
                "headless.test.ping",
            );
            flooded
                .accept_headless(&expected_origin, &payload, 500_001)
                .expect("within burst");
        }
        let payload = request(&flooded, 11, "burst-11", "headless.test.ping");
        assert_eq!(
            flooded.accept_headless(&expected_origin, &payload, 500_001),
            Err(BridgeError::RateLimited)
        );
    }
}
