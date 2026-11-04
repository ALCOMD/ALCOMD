//! Capture actual tracing text and reviewed technical fields, never arbitrary payloads.

use alcomd_application::{DiagnosticLog, DiagnosticLogLevel};
use tracing::{
    Event, Subscriber,
    field::Visit,
    span::{Attributes, Id, Record},
};
use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};

#[derive(Clone)]
pub(crate) struct DiagnosticLogLayer {
    log: DiagnosticLog,
}

impl DiagnosticLogLayer {
    pub(crate) fn new(log: DiagnosticLog) -> Self {
        Self { log }
    }
}

pub(crate) fn is_core_target(target: &str) -> bool {
    target == "alcomd" || target.starts_with("alcomd::") || target.starts_with("alcomd_")
}

pub(crate) fn capture_metadata(metadata: &tracing::Metadata<'_>) -> bool {
    is_core_target(metadata.target())
        && (metadata.is_span() || *metadata.level() <= tracing::Level::DEBUG)
}

#[derive(Clone, Copy)]
struct ToastOverride(bool);

#[derive(Default)]
struct SafeFields {
    code: Option<String>,
    notification_code: Option<String>,
    diagnostic_id: Option<String>,
    gui_toast: Option<bool>,
    message: String,
    details: Vec<String>,
}

#[derive(Default)]
struct BoundedText(String);
impl std::fmt::Write for BoundedText {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        let mut end = value.len().min(4096usize.saturating_sub(self.0.len()));
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        self.0.push_str(&value[..end]);
        Ok(())
    }
}

fn technical_field(name: &str) -> bool {
    matches!(
        name,
        "path"
            | "reason"
            | "error"
            | "endpoint"
            | "product"
            | "rpc_version"
            | "operation_id"
            | "project_id"
            | "kind"
            | "phase"
    )
}

impl Visit for SafeFields {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            // Bound even transient copies; application validates the grammar.
            "code" if value.len() <= 96 => self.code = Some(value.to_owned()),
            "notification_code" if value.len() <= 96 => {
                self.notification_code = Some(value.to_owned())
            }
            "diagnostic_id" if value.len() <= 36 => self.diagnostic_id = Some(value.to_owned()),
            "message" => {
                self.message = value.chars().take(4096).collect();
            }
            name if technical_field(name) => {
                self.details.push(format!(
                    "{name}={}",
                    value.chars().take(4096).collect::<String>()
                ));
            }
            _ => {}
        }
    }

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        if field.name() == "gui_toast" {
            self.gui_toast = Some(value);
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() != "message" && !technical_field(field.name()) {
            return;
        }
        let mut text = BoundedText::default();
        let _ = std::fmt::write(&mut text, format_args!("{value:?}"));
        if field.name() == "message" {
            self.message = text.0;
        } else {
            self.details.push(format!("{}={}", field.name(), text.0));
        }
    }
}

impl<S> Layer<S> for DiagnosticLogLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(&self, attributes: &Attributes<'_>, id: &Id, context: Context<'_, S>) {
        let mut fields = SafeFields::default();
        attributes.record(&mut fields);
        if let Some(value) = fields.gui_toast
            && let Some(span) = context.span(id)
        {
            span.extensions_mut().insert(ToastOverride(value));
        }
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, context: Context<'_, S>) {
        let mut fields = SafeFields::default();
        values.record(&mut fields);
        if let Some(value) = fields.gui_toast
            && let Some(span) = context.span(id)
        {
            span.extensions_mut().replace(ToastOverride(value));
        }
    }

    fn on_event(&self, event: &Event<'_>, context: Context<'_, S>) {
        let mut fields = SafeFields::default();
        event.record(&mut fields);
        let mut inherited = None;
        if let Some(scope) = context.event_scope(event) {
            for span in scope.from_root() {
                if let Some(value) = span.extensions().get::<ToastOverride>() {
                    inherited = Some(value.0);
                }
            }
        }
        let level = match *event.metadata().level() {
            tracing::Level::TRACE => DiagnosticLogLevel::Trace,
            tracing::Level::DEBUG => DiagnosticLogLevel::Debug,
            tracing::Level::INFO => DiagnosticLogLevel::Info,
            tracing::Level::WARN => DiagnosticLogLevel::Warning,
            tracing::Level::ERROR => DiagnosticLogLevel::Error,
        };
        let mut message = fields.message;
        for detail in fields.details {
            message.push('\n');
            message.push_str(&detail);
        }
        self.log.record(
            level,
            fields
                .notification_code
                .as_deref()
                .or(fields.code.as_deref()),
            fields.diagnostic_id.as_deref(),
            fields.gui_toast.or(inherited),
            &message,
            event.metadata().target(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alcomd_application::AccessContext;
    use tracing_subscriber::prelude::*;

    #[tokio::test]
    async fn captures_real_text_and_reviewed_details_without_secret_fields() {
        let log = DiagnosticLog::new();
        let subscriber = tracing_subscriber::registry().with(DiagnosticLogLayer::new(log.clone()));
        tracing::subscriber::with_default(subscriber, || {
            tracing::trace!(gui_toast = true, "trace raw text");
            tracing::debug!("debug raw text");
            tracing::info!("info raw text");
            tracing::warn!(
                code = "unity_version_unverified",
                path = "C:\\Unity 项目",
                argv = "secret",
                "warning raw text"
            );
            tracing::error!(
                notification_code = "core.failure",
                code = "ignored.code",
                diagnostic_id = "00000000-0000-4000-8000-000000000123",
                env = "secret",
                "error raw text"
            );
        });
        let page = log
            .read(&AccessContext::local_owner(), None, None, None)
            .await
            .expect("local owner can read captured safe tracing records");
        assert_eq!(
            page.items.iter().map(|item| item.level).collect::<Vec<_>>(),
            [
                DiagnosticLogLevel::Trace,
                DiagnosticLogLevel::Debug,
                DiagnosticLogLevel::Info,
                DiagnosticLogLevel::Warning,
                DiagnosticLogLevel::Error
            ]
        );
        assert_eq!(
            page.items
                .iter()
                .map(|item| item.notify)
                .collect::<Vec<_>>(),
            [false, false, false, false, true]
        );
        assert_eq!(page.items[3].code, "unity_version_unverified");
        assert_eq!(page.items[4].code, "core.failure");
        assert_eq!(
            page.items[4].diagnostic_id.as_deref(),
            Some("00000000-0000-4000-8000-000000000123")
        );
        let debug = format!("{:?}", page.items);
        assert!(!debug.contains("secret"));
        assert_eq!(page.items[0].message, "trace raw text");
        assert!(
            page.items[3]
                .message
                .contains("warning raw text\npath=C:\\Unity 项目")
        );
        assert_eq!(page.items[4].message, "error raw text");
        assert!(
            page.items
                .iter()
                .all(|item| item.target.starts_with("alcomd"))
        );
    }

    #[tokio::test]
    async fn request_span_suppresses_duplicates_and_explicit_event_values_take_precedence() {
        let log = DiagnosticLog::new();
        let subscriber = tracing_subscriber::registry().with(DiagnosticLogLayer::new(log.clone()));
        tracing::subscriber::with_default(subscriber, || {
            let request = tracing::info_span!("rpc.request", gui_toast = false);
            let _entered = request.enter();
            tracing::error!(code = "request.failure", "handled by caller");
            tracing::error!(gui_toast = true, "explicit override");
            tracing::warn!(gui_toast = true, "explicit warning");
            let child = tracing::info_span!("child", gui_toast = true);
            let _child = child.enter();
            tracing::error!("nearest span overrides parent");
            tracing::error!(gui_toast = false, "event overrides child");
            child.record("gui_toast", false);
            tracing::error!("updated span stays silent");
        });
        let page = log
            .read(&AccessContext::local_owner(), None, None, None)
            .await
            .expect("local owner can read captured safe tracing records");
        assert_eq!(
            page.items
                .iter()
                .map(|item| item.notify)
                .collect::<Vec<_>>(),
            [false, true, true, true, false, false]
        );
        assert!(
            page.items
                .iter()
                .filter(|item| item.level == DiagnosticLogLevel::Error)
                .all(|item| item.diagnostic_id.is_some())
        );
    }

    #[tokio::test]
    async fn rejects_formatted_or_unsafe_codes_and_diagnostic_identifiers() {
        let log = DiagnosticLog::new();
        let subscriber = tracing_subscriber::registry().with(DiagnosticLogLayer::new(log.clone()));
        tracing::subscriber::with_default(subscriber, || {
            tracing::error!(code = "C:\\secret", diagnostic_id = "secret", "ignored");
            tracing::error!(code = ?"secret-debug-field", diagnostic_id = ?"secret-debug-id", "ignored");
        });
        let page = log
            .read(&AccessContext::local_owner(), None, None, None)
            .await
            .expect("local owner can read captured safe tracing records");
        assert_eq!(page.items.len(), 2);
        assert!(
            page.items
                .iter()
                .all(|item| item.code == "diagnostic.error" && item.diagnostic_id.is_some())
        );
        assert!(!format!("{:?}", page.items).contains("secret"));
    }

    #[tokio::test]
    async fn output_filter_cannot_disable_request_policy_and_dependency_events_are_excluded() {
        let log = DiagnosticLog::new();
        let subscriber = tracing_subscriber::registry()
            .with(
                tracing_subscriber::fmt::layer()
                    .with_writer(std::io::sink)
                    .with_filter(tracing_subscriber::EnvFilter::new("alcomd=error")),
            )
            .with(
                DiagnosticLogLayer::new(log.clone())
                    .with_filter(tracing_subscriber::filter::filter_fn(capture_metadata)),
            );
        tracing::subscriber::with_default(subscriber, || {
            let request = tracing::info_span!("rpc_request", gui_toast = false);
            let _entered = request.enter();
            tracing::error!("already handled by RPC");
            tracing::error!(target: "third_party", gui_toast = true, "never enters the Core buffer");
            tracing::debug!("debug projection remains available but silent");
            tracing::trace!("trace is not retained by default");
        });
        let page = log
            .read(&AccessContext::local_owner(), None, None, None)
            .await
            .expect("local owner can read captured safe tracing records");
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.items[0].level, DiagnosticLogLevel::Error);
        assert!(!page.items[0].notify);
        assert_eq!(page.items[1].level, DiagnosticLogLevel::Debug);
        assert!(!page.items[1].notify);
    }
}
