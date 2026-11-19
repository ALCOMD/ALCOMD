use std::time::Duration;

use crate::{M2Application, M6ExtensionApplication};

pub(super) const STARTUP_GRACE: Duration = Duration::from_secs(5);
pub(super) const DISCONNECTED_GRACE: Duration = Duration::from_secs(1);

/// This future is only polled with no connection handlers, and replaced when
/// the last handler ends. It never infers completion from one client's view.
pub(super) async fn wait(
    application: &M2Application,
    extensions: &M6ExtensionApplication,
    grace: Duration,
) {
    tokio::time::sleep(grace).await;
    let mut reported_error = false;
    loop {
        let active_work = match application.has_unfinished_operations().await {
            Ok(false) => extensions
                .has_scoped_background_work()
                .await
                .map_err(|error| error.to_string()),
            Ok(true) => Ok(true),
            Err(error) => Err(error.to_string()),
        };
        match active_work {
            Ok(false) => return,
            Ok(true) => reported_error = false,
            Err(error) if !reported_error => {
                eprintln!("Core idle shutdown could not read active work: {error}");
                reported_error = true;
            }
            Err(_) => {}
        }
        tokio::time::sleep(DISCONNECTED_GRACE).await;
    }
}
