//! Only committed task terminal states become logs; progress is not log traffic.

use alcomd_application::{
    AccessContext, DiagnosticLog, DiagnosticLogLevel, EventRecord, EventSubscription, OperationId,
    OperationRecord, OperationState,
};
use alcomd_store::StateStoreHandle;

pub(super) async fn follow(
    application: super::M2Application,
    log: DiagnosticLog,
    mut subscription: EventSubscription<StateStoreHandle>,
) -> Result<(), alcomd_application::ApplicationError> {
    let access = AccessContext::local_owner();
    loop {
        let page = subscription.next_page().await?;
        for event in page.events.iter().filter(|event| is_terminal_event(event)) {
            let id = OperationId::parse(&event.aggregate_id)
                .map_err(|_| alcomd_application::ApplicationError::InvalidInput)?;
            let operation = application.get_operation(&access, id).await?;
            record(&log, &operation);
        }
        subscription.advance(page.next_sequence, page.resources_revision)?;
    }
}

fn is_terminal_event(event: &EventRecord) -> bool {
    event.aggregate_kind == "operation"
        && matches!(
            event.kind.as_str(),
            "operation.completed"
                | "operation.succeeded"
                | "operation.failed"
                | "operation.cancelled"
        )
}

fn record(log: &DiagnosticLog, operation: &OperationRecord) {
    match operation.state {
        OperationState::Failed => {
            log.record(
                DiagnosticLogLevel::Error,
                operation.error_code.as_deref().or(Some("operation.failed")),
                operation.diagnostic_id.as_deref(),
                None,
            );
        }
        OperationState::Succeeded | OperationState::Cancelled => {
            let suffix = if operation.state == OperationState::Succeeded {
                "succeeded"
            } else {
                "cancelled"
            };
            let code = format!("operation.{}.{suffix}", operation.kind);
            log.record(DiagnosticLogLevel::Info, Some(&code), None, Some(false));
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alcomd_application::{PrincipalId, Revision};

    #[tokio::test]
    async fn committed_task_completion_reaches_the_log_without_a_waiting_command_caller() {
        let root = std::env::temp_dir().canonicalize().expect("temporary root");
        let directory = root.join(format!("alcomd-operation-log-{}", OperationId::new()));
        std::fs::create_dir(&directory).expect("isolated directory");
        let store =
            StateStoreHandle::open(directory.join("state.sqlite3")).expect("isolated store");
        let app = super::super::M2Application::new(store.clone());
        let access = AccessContext::local_owner();
        let subscription = app
            .subscribe_events(&access, None, 100)
            .expect("subscribe before commit");
        let log = DiagnosticLog::new();
        let task = tokio::spawn(follow(app.clone(), log.clone(), subscription));
        let key = alcomd_application::IdempotencyKey::parse("task-log-test").expect("key");
        app.state_check(&access, key.clone())
            .await
            .expect("accept task");
        // The command has returned; terminal delivery is driven by the Store's
        // committed events, rather than a caller waiting on the task result.
        let baseline = log.read(&access, None, None, None).await.expect("baseline");
        let page = if baseline.items.is_empty() {
            log.read(&access, Some(baseline.next_cursor), None, Some(5_000))
                .await
                .expect("committed result")
        } else {
            baseline
        };
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].code, "operation.state.check.succeeded");
        app.state_check(&access, key)
            .await
            .expect("idempotent replay");
        assert_eq!(
            log.read(&access, None, None, None)
                .await
                .expect("no replay log")
                .items
                .len(),
            1
        );
        task.abort();
        assert!(task.await.expect_err("cancel follower").is_cancelled());
        drop(app);
        drop(store);
        let actual = directory.canonicalize().expect("fixture path");
        assert_eq!(actual.parent(), Some(root.as_path()));
        std::fs::remove_dir_all(actual).expect("remove only the isolated fixture");
    }

    #[tokio::test]
    async fn terminal_results_are_recorded_but_progress_does_not_generate_notifications() {
        let log = DiagnosticLog::new();
        let mut operation = OperationRecord {
            operation_id: OperationId::new(),
            kind: "state.check".to_owned(),
            state: OperationState::Running,
            revision: Revision::INITIAL,
            owner: PrincipalId::local_owner(),
            cancel_requested: false,
            created_at_ms: 1,
            updated_at_ms: 2,
            started_at_ms: Some(2),
            completed_at_ms: None,
            result_json: None,
            error_code: None,
            diagnostic_id: None,
            progress_phase: None,
        };
        record(&log, &operation);
        operation.state = OperationState::Succeeded;
        record(&log, &operation);
        operation.state = OperationState::Failed;
        operation.error_code = Some("backup_source_changed".to_owned());
        operation.diagnostic_id = Some(OperationId::new().to_string());
        record(&log, &operation);
        let page = log
            .read(&AccessContext::local_owner(), None, None, None)
            .await
            .expect("logs");
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0].code, "operation.state.check.succeeded");
        assert!(!page.items[0].notify);
        assert_eq!(page.items[1].code, "backup_source_changed");
        assert_eq!(page.items[1].diagnostic_id, operation.diagnostic_id);
        assert!(page.items[1].notify);
    }
}
