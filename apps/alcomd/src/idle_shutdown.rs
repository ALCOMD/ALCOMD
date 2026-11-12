use std::time::Duration;

use crate::M2Application;

pub(super) const STARTUP_GRACE: Duration = Duration::from_secs(5);
pub(super) const DISCONNECTED_GRACE: Duration = Duration::from_secs(1);

/// This future is only polled with no connection handlers, and replaced when
/// the last handler ends. It never infers completion from one client's view.
pub(super) async fn wait(application: &M2Application, grace: Duration) {
    tokio::time::sleep(grace).await;
    let mut reported_error = false;
    loop {
        match application.has_unfinished_operations().await {
            Ok(false) => return,
            Ok(true) => reported_error = false,
            Err(error) if !reported_error => {
                eprintln!("Core idle shutdown could not read unfinished tasks: {error}");
                reported_error = true;
            }
            Err(_) => {}
        }
        tokio::time::sleep(DISCONNECTED_GRACE).await;
    }
}
