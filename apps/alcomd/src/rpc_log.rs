//! Safe request outcomes are recorded before their response is sent.

use alcomd_application::{DiagnosticLog, DiagnosticLogLevel};
use serde_json::Value;

pub(super) fn record(log: &DiagnosticLog, method: &str, response: &mut Value) {
    // Observing the log must never generate another log entry. Live-event
    // delivery and connection negotiation are transport, not business outcomes.
    if method.starts_with("diagnostics.log.")
        || method.starts_with("events.")
        || method == "system.hello"
        || method == "system.status"
    {
        return;
    }
    if let Some(error) = response.get_mut("error").and_then(Value::as_object_mut) {
        let code = error.get("code").and_then(Value::as_str);
        let diagnostic_id = error.get("diagnosticId").and_then(Value::as_str);
        // A request's caller decides whether its feedback needs a popup. The
        // record still exists for every rejected request, including CLI calls.
        let entry = log.record(DiagnosticLogLevel::Error, code, diagnostic_id, Some(false));
        if let Some(id) = entry.diagnostic_id {
            error.insert("diagnosticId".to_owned(), Value::String(id));
        }
    } else if response.get("result").is_some() {
        let code = format!("rpc.{}.succeeded", method.to_ascii_lowercase());
        log.record(DiagnosticLogLevel::Trace, Some(&code), None, Some(false));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alcomd_application::AccessContext;
    use serde_json::json;

    #[tokio::test]
    async fn errors_have_a_log_record_and_the_same_diagnostic_id_before_delivery() {
        let log = DiagnosticLog::new();
        let mut response = json!({"id":"request", "error":{"code":"unity_launch_state_uncertain", "message":"safe"}});
        record(&log, "unity.launch", &mut response);
        let page = log
            .read(&AccessContext::local_owner(), None, None, None)
            .await
            .expect("log");
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].code, "unity_launch_state_uncertain");
        assert_eq!(
            page.items[0].diagnostic_id.as_deref(),
            response["error"]["diagnosticId"].as_str()
        );
        assert!(!page.items[0].notify);
    }

    #[tokio::test]
    async fn results_are_safe_and_log_observation_never_logs_itself() {
        let log = DiagnosticLog::new();
        let mut response = json!({"id":"request", "result":{"privatePath":"not a log field"}});
        record(&log, "unity.projectLaunchConfig.get", &mut response);
        for method in [
            "diagnostics.log.read",
            "events.subscribe",
            "events.list",
            "system.hello",
            "system.status",
        ] {
            record(&log, method, &mut response);
        }
        let page = log
            .read(&AccessContext::local_owner(), None, None, None)
            .await
            .expect("log");
        assert_eq!(page.items.len(), 1);
        assert_eq!(
            page.items[0].code,
            "rpc.unity.projectlaunchconfig.get.succeeded"
        );
        assert_eq!(page.items[0].level, DiagnosticLogLevel::Trace);
        assert!(!page.items[0].notify);
    }
}
