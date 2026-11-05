use std::sync::{Arc, Mutex as SyncMutex};

use alcomd_client::{AlcomdClient, ClientConfig};
use alcomd_protocol::{DiagnosticLogReadParams, DiagnosticLogReadResult, RpcError};
use tauri::State;
use tauri::async_runtime::Mutex;

type AbortRead = Box<dyn Fn() + Send>;

#[tauri::command]
pub(crate) async fn gui_diagnostic_log_history(
    params: alcomd_protocol::DiagnosticLogHistoryParams,
) -> Result<alcomd_protocol::DiagnosticLogHistoryResult, RpcError> {
    // Saved-file paging must never wait behind the live log long read or the
    // normal command connection. This connection is disposable and read-only.
    let mut client = AlcomdClient::connect(ClientConfig::default().without_daemon_start())
        .await
        .map_err(super::client_error)?;
    client
        .diagnostic_log_history(params)
        .await
        .map_err(super::client_error)
}

#[tauri::command]
pub(crate) async fn gui_diagnostic_log_health()
-> Result<alcomd_protocol::DiagnosticLogHealth, RpcError> {
    let mut client = AlcomdClient::connect(ClientConfig::default().without_daemon_start())
        .await
        .map_err(super::client_error)?;
    client
        .diagnostic_log_health()
        .await
        .map_err(super::client_error)
}
#[derive(Default)]
struct PendingRead {
    sequence: u64,
    cancellations: usize,
    active: Option<(u64, AbortRead)>,
}

#[derive(Default)]
pub(crate) struct DiagnosticLogClient {
    client: Arc<Mutex<Option<AlcomdClient>>>,
    pending: SyncMutex<PendingRead>,
}

impl DiagnosticLogClient {
    pub(crate) async fn cancel(&self) {
        let abort = {
            let mut pending = self
                .pending
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            pending.cancellations += 1;
            pending.active.take()
        };
        if let Some((_, abort)) = abort {
            abort();
        }
        // Cancellation may interrupt a frame, so the connection cannot be reused.
        *self.client.lock().await = None;
        self.pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .cancellations -= 1;
    }
}

impl Drop for DiagnosticLogClient {
    fn drop(&mut self) {
        if let Some((_, abort)) = self
            .pending
            .get_mut()
            .unwrap_or_else(|error| error.into_inner())
            .active
            .take()
        {
            abort();
        }
    }
}

impl DiagnosticLogClient {
    async fn read(
        &self,
        params: DiagnosticLogReadParams,
    ) -> Result<DiagnosticLogReadResult, RpcError> {
        let client = Arc::clone(&self.client);
        let (sequence, task) = {
            let mut pending = self
                .pending
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if pending.active.is_some() || pending.cancellations != 0 {
                return Err(RpcError::invalid_request());
            }
            pending.sequence = pending.sequence.wrapping_add(1);
            let sequence = pending.sequence;
            let task = tauri::async_runtime::spawn(async move {
                let mut client = client.lock().await;
                if client.is_none() {
                    *client = Some(
                        AlcomdClient::connect(ClientConfig::default().without_daemon_start())
                            .await
                            .map_err(super::client_error)?,
                    );
                }
                let result = client
                    .as_mut()
                    .ok_or_else(super::daemon_unavailable)?
                    .diagnostic_log_read(params)
                    .await;
                super::finish_call(&mut client, result)
            });
            let abort = task.inner().abort_handle();
            pending.active = Some((sequence, Box::new(move || abort.abort())));
            (sequence, task)
        };
        let result = task.await.map_err(|_| super::daemon_unavailable());
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if pending
            .active
            .as_ref()
            .is_some_and(|(active, _)| *active == sequence)
        {
            pending.active = None;
        }
        result?
    }
}

#[tauri::command]
pub(crate) async fn gui_diagnostic_log_read(
    state: State<'_, DiagnosticLogClient>,
    params: DiagnosticLogReadParams,
) -> Result<DiagnosticLogReadResult, RpcError> {
    state.read(params).await
}

#[tauri::command]
pub(crate) async fn gui_diagnostic_log_cancel(
    state: State<'_, DiagnosticLogClient>,
) -> Result<(), RpcError> {
    state.cancel().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_new_read_is_rejected_until_all_connection_cancellations_finish() {
        use std::future::Future;
        use std::task::Poll;
        tauri::async_runtime::block_on(async {
            let state = DiagnosticLogClient::default();
            let held = state.client.lock().await;
            let mut first = Box::pin(state.cancel());
            let mut second = Box::pin(state.cancel());
            assert!(
                std::future::poll_fn(|context| Poll::Ready(
                    first.as_mut().poll(context).is_pending()
                ))
                .await
            );
            assert!(
                std::future::poll_fn(|context| Poll::Ready(
                    second.as_mut().poll(context).is_pending()
                ))
                .await
            );
            assert_eq!(
                state
                    .pending
                    .lock()
                    .expect("unpoisoned diagnostic test state")
                    .cancellations,
                2
            );
            let error = state
                .read(DiagnosticLogReadParams::default())
                .await
                .expect_err("read must be rejected while cancellation is pending");
            assert_eq!(error.code, "invalid_request");
            drop(held);
            first.await;
            assert_eq!(
                state
                    .pending
                    .lock()
                    .expect("unpoisoned diagnostic test state")
                    .cancellations,
                1
            );
            let error = state
                .read(DiagnosticLogReadParams::default())
                .await
                .expect_err("read must be rejected while cancellation is pending");
            assert_eq!(error.code, "invalid_request");
            second.await;
            assert_eq!(
                state
                    .pending
                    .lock()
                    .expect("unpoisoned diagnostic test state")
                    .cancellations,
                0
            );
        });
    }

    #[test]
    fn cancellation_releases_a_long_read_before_closing_its_connection() {
        let (complete, completion) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            tauri::async_runtime::block_on(async {
                let state = DiagnosticLogClient::default();
                let client = Arc::clone(&state.client);
                let (entered, mut received) = tauri::async_runtime::channel(1);
                let task = tauri::async_runtime::spawn(async move {
                    let _held_connection = client.lock().await;
                    entered
                        .send(())
                        .await
                        .expect("signal that the read owns the connection");
                    std::future::pending::<()>().await;
                });
                let abort = task.inner().abort_handle();
                state
                    .pending
                    .lock()
                    .expect("unpoisoned diagnostic test state")
                    .active = Some((1, Box::new(move || abort.abort())));
                received
                    .recv()
                    .await
                    .expect("long read reached its connection");
                state.cancel().await;
                assert!(task.await.is_err());
                assert!(
                    state
                        .pending
                        .lock()
                        .expect("unpoisoned diagnostic test state")
                        .active
                        .is_none()
                );
                assert!(state.client.lock().await.is_none());
            });
            complete.send(()).expect("report completed cancellation");
        });
        completion
            .recv_timeout(Duration::from_secs(3))
            .expect("cancel must not wait for the long read timeout");
    }
}
