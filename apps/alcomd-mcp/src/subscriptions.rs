//! Frozen tools-list invalidation streams, with independent authenticated Core IO.
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use alcomd_client::{AlcomdClient, ClientConfig};
use alcomd_protocol::{CAPABILITY_MCP_TOOLS_NOTIFICATIONS_V1, McpToolsCatalogParams};
use rmcp::{ErrorData, ServerHandler, model::*, service::SubscriptionContext};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

pub(crate) const MAX_AGE: Duration = Duration::from_secs(600);
pub(crate) const ESTABLISH_TIME: Duration = Duration::from_secs(10);
pub(crate) const OUTPUT_TIME: Duration = Duration::from_secs(10);

#[derive(Default)]
pub(crate) struct Limits(Mutex<HashMap<u64, Weak<Semaphore>>>);
impl Limits {
    pub fn acquire(&self, authentication_context: u64) -> Result<OwnedSemaphorePermit, ErrorData> {
        let mut contexts = self.0.lock().map_err(|_| unavailable())?;
        contexts.retain(|_, weak| weak.strong_count() != 0);
        let semaphore = contexts
            .get(&authentication_context)
            .and_then(Weak::upgrade)
            .unwrap_or_else(|| {
                let semaphore = Arc::new(Semaphore::new(4));
                contexts.insert(authentication_context, Arc::downgrade(&semaphore));
                semaphore
            });
        semaphore
            .try_acquire_owned()
            .map_err(|_| ErrorData::invalid_params("Too many active subscriptions", None))
    }
}

pub(crate) fn accepted(requested: &SubscriptionFilter) -> Option<SubscriptionFilter> {
    (requested.tools_list_changed == Some(true))
        .then(|| SubscriptionFilter::builder().tools_list_changed().build())
}
fn unavailable() -> ErrorData {
    ErrorData::internal_error("The current authorized tool catalogue is unavailable", None)
}
pub(crate) async fn fingerprint(
    core: &mut AlcomdClient,
) -> Result<String, alcomd_client::ClientError> {
    core.mcp_tools_catalog(McpToolsCatalogParams {
        limit: Some(1),
        ..Default::default()
    })
    .await
    .map(|result| result.catalog_fingerprint)
}

pub(crate) struct Prepared {
    core: tokio::sync::Mutex<AlcomdClient>,
    initial: String,
    _permit: OwnedSemaphorePermit,
}
impl Prepared {
    pub fn new(core: AlcomdClient, initial: String, permit: OwnedSemaphorePermit) -> Self {
        Self {
            core: tokio::sync::Mutex::new(core),
            initial,
            _permit: permit,
        }
    }
    pub async fn connect(
        config: ClientConfig,
        permit: OwnedSemaphorePermit,
    ) -> Result<Self, ErrorData> {
        tokio::time::timeout(ESTABLISH_TIME, async move {
            let mut core = AlcomdClient::connect(config.without_daemon_start())
                .await
                .map_err(|_| unavailable())?;
            if !core.supports_capability(CAPABILITY_MCP_TOOLS_NOTIFICATIONS_V1) {
                return Err(super::unsupported_method());
            }
            let initial = fingerprint(&mut core).await.map_err(|_| unavailable())?;
            Ok(Self::new(core, initial, permit))
        })
        .await
        .map_err(|_| unavailable())?
    }
    pub async fn run(&self, context: SubscriptionContext) -> Result<(), ErrorData> {
        let mut previous = self.initial.clone();
        let deadline = tokio::time::Instant::now() + MAX_AGE;
        let mut ticks = tokio::time::interval(Duration::from_secs(1));
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                biased;
                () = context.cancelled() => return Ok(()),
                () = tokio::time::sleep_until(deadline) => return Ok(()),
                _ = ticks.tick() => {},
            }
            // First tick is immediate, closing the pre-ack snapshot race. A failed
            // read is never interpreted as an empty catalogue or a grant change.
            let current = tokio::select! {
                biased;
                () = context.cancelled() => return Ok(()),
                result = tokio::time::timeout_at(deadline.min(tokio::time::Instant::now() + ESTABLISH_TIME), async {
                    fingerprint(&mut *self.core.lock().await).await
                }) => result.map_err(|_| unavailable())?.map_err(|_| unavailable())?,
            };
            if current != previous {
                let sent = tokio::select! {
                    biased;
                    () = context.cancelled() => return Ok(()),
                    result = tokio::time::timeout_at(deadline.min(tokio::time::Instant::now() + OUTPUT_TIME), context.sink().notify_tool_list_changed()) => result,
                };
                if !matches!(sent, Ok(Ok(()))) {
                    context.request_context().ct.cancel();
                    return Err(unavailable());
                }
                previous = current;
            }
        }
    }
}

pub(crate) struct StdioHandler(pub Prepared, pub Option<Arc<super::observations::Request>>);
impl ServerHandler for StdioHandler {
    fn get_info(&self) -> ServerConfig {
        super::server_info(true, true)
    }
    fn accepted_subscription_filter(
        &self,
        requested: &SubscriptionFilter,
    ) -> Option<SubscriptionFilter> {
        accepted(requested)
    }
    async fn listen(&self, context: SubscriptionContext) -> Result<(), ErrorData> {
        let _observed_stream = self.1.as_ref().and_then(|r| r.stream());
        self.0.run(context).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filters_and_admission_are_narrow_and_released_on_drop() {
        let limits = Limits::default();
        let mut permits: Vec<_> = (0..4)
            .map(|_| limits.acquire(1).expect("four contexts"))
            .collect();
        assert!(limits.acquire(1).is_err());
        assert!(limits.acquire(2).is_ok());
        drop(permits.pop());
        assert!(limits.acquire(1).is_ok());
        assert!(accepted(&SubscriptionFilter::builder().prompts_list_changed().build()).is_none());
        let requested = SubscriptionFilter::builder()
            .tools_list_changed()
            .prompts_list_changed()
            .build();
        assert_eq!(
            serde_json::to_value(accepted(&requested)).expect("filter"),
            serde_json::json!({"toolsListChanged":true})
        );
    }
}
