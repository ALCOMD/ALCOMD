use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use alcomd_extensions::{
    HOST_PROTOCOL_VERSION, HostMessage, HostMessageBody, HostStableError, RuntimeLimits,
    read_host_message, write_host_message,
};
use anyhow::{Context, Result, bail};
use clap::Parser;
use serde_json::{Value, json};
use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Config, Engine, Store, StoreLimits, StoreLimitsBuilder};

wasmtime::component::bindgen!({
    path: "../../specs/extensions/wit/extension-v1",
    world: "extension-v1",
    imports: { default: async },
    exports: { default: async },
});

use crate::alcomd::extension::{host_data, host_projects, types};
use crate::exports::alcomd::extension::guest_ui;

#[derive(Debug, Parser)]
#[command(name = "alcomd-extension-host", version, about)]
struct Arguments {
    #[arg(long)]
    extension: String,
    #[arg(long)]
    component: PathBuf,
}

struct HostState {
    limits: StoreLimits,
    protocol: ProtocolChannel,
}

struct ProtocolChannel {
    input: std::io::Stdin,
    output: std::io::Stdout,
    daemon_epoch: String,
    instance_id: String,
    lifecycle_generation: u64,
    lease_id: String,
    inbound_sequence: u64,
    outbound_sequence: u64,
    next_call: u64,
    failed: bool,
    call_times: VecDeque<Instant>,
    call_tokens: f64,
    token_updated_at: Instant,
    call_window_ms: u64,
    calls_per_window: usize,
    call_burst: f64,
    wit_input_bytes: usize,
    wit_output_bytes: usize,
    current_invocation_context_id: Option<String>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if run().await.is_err() {
        eprintln!("extension host failed");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let arguments = Arguments::parse();
    validate_component_path(&arguments.component)?;
    let mut input = std::io::stdin();
    let mut output = std::io::stdout();
    let bootstrap = read_host_message(&mut input).map_err(|_| anyhow::anyhow!("protocol"))?;
    let HostMessageBody::Bootstrap {
        nonce,
        lease_id,
        extension_id,
        api_world: _,
        limits,
    } = &bootstrap.body
    else {
        bail!("protocol");
    };
    if extension_id != &arguments.extension || limits != &RuntimeLimits::default() {
        bail!("protocol");
    }
    write_host_message(
        &mut output,
        &HostMessage {
            protocol_version: HOST_PROTOCOL_VERSION,
            daemon_epoch: bootstrap.daemon_epoch.clone(),
            instance_id: bootstrap.instance_id.clone(),
            lifecycle_generation: bootstrap.lifecycle_generation,
            sequence: 1,
            body: HostMessageBody::Ready {
                nonce: nonce.clone(),
            },
        },
    )
    .map_err(|_| anyhow::anyhow!("protocol"))?;

    let engine = create_engine()?;
    let component = Component::from_file(&engine, &arguments.component)
        .map_err(|_| anyhow::anyhow!("component"))?;
    let store_limits = StoreLimitsBuilder::new()
        .memory_size(usize::try_from(limits.linear_memory_bytes).context("limit")?)
        .table_elements(usize::try_from(limits.table_elements).context("limit")?)
        // The Host creates exactly one top-level Component. This separate
        // bound limits the core instances that Component may instantiate.
        .instances(16)
        .memories(1)
        .tables(1)
        .trap_on_grow_failure(true)
        .build();
    let protocol = ProtocolChannel {
        input,
        output,
        daemon_epoch: bootstrap.daemon_epoch,
        instance_id: bootstrap.instance_id,
        lifecycle_generation: bootstrap.lifecycle_generation,
        lease_id: lease_id.clone(),
        inbound_sequence: bootstrap.sequence,
        outbound_sequence: 1,
        next_call: 1,
        failed: false,
        call_times: VecDeque::new(),
        call_tokens: f64::from(limits.host_call_burst),
        token_updated_at: Instant::now(),
        call_window_ms: limits.host_call_window_ms,
        calls_per_window: usize::try_from(limits.host_calls_per_window).context("limit")?,
        call_burst: f64::from(limits.host_call_burst),
        wit_input_bytes: usize::try_from(limits.wit_input_bytes).context("limit")?,
        wit_output_bytes: usize::try_from(limits.wit_output_bytes).context("limit")?,
        current_invocation_context_id: None,
    };
    let mut store = Store::new(
        &engine,
        HostState {
            limits: store_limits,
            protocol,
        },
    );
    store.limiter(|state| &mut state.limits);
    store
        .set_fuel(limits.fuel_per_guest_call)
        .map_err(|_| anyhow::anyhow!("fuel"))?;
    store.epoch_deadline_trap();
    store.set_epoch_deadline(
        limits
            .activate_timeout_ms
            .div_ceil(limits.epoch_tick_ms)
            .max(1),
    );
    let ticker_running = Arc::new(AtomicBool::new(true));
    let ticker = start_epoch_ticker(
        engine.clone(),
        Arc::clone(&ticker_running),
        limits.epoch_tick_ms,
    );
    let mut linker = Linker::new(&engine);
    ExtensionV1::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)
        .map_err(|_| anyhow::anyhow!("link"))?;
    let bindings = ExtensionV1::instantiate_async(&mut store, &component, &linker)
        .await
        .map_err(|_| anyhow::anyhow!("instantiate"))?;
    let result = event_loop(&bindings, &mut store, limits).await;
    ticker_running.store(false, Ordering::Release);
    let _ = ticker.join();
    result
}

async fn event_loop(
    bindings: &ExtensionV1,
    store: &mut Store<HostState>,
    limits: &RuntimeLimits,
) -> Result<()> {
    loop {
        let message = read_host_message(&mut store.data_mut().protocol.input)
            .map_err(|_| anyhow::anyhow!("protocol"))?;
        store.data_mut().protocol.validate_inbound(&message)?;
        match message.body {
            HostMessageBody::InvokeExport {
                request_id,
                invocation_context_id,
                export,
                input,
            } => {
                if store
                    .data()
                    .protocol
                    .current_invocation_context_id
                    .is_some()
                {
                    bail!("protocol");
                }
                store.data_mut().protocol.current_invocation_context_id =
                    Some(invocation_context_id);
                prepare_call(store, limits, export.as_str())?;
                let result = match export.as_str() {
                    "activate" => invoke_activate(bindings, store, &input).await,
                    "deactivate" => invoke_deactivate(bindings, store, &input).await,
                    "ui.open" => invoke_ui_open(bindings, store, &input).await,
                    "ui.refresh" => invoke_ui_refresh(bindings, store, &input).await,
                    "ui.dispatch" => invoke_ui_dispatch(bindings, store, &input).await,
                    "ui.close" => invoke_ui_close(bindings, store, &input).await,
                    _ => bail!("protocol"),
                };
                if store.data().protocol.failed {
                    bail!("protocol");
                }
                let body = match result {
                    Ok(value) => HostMessageBody::ExportResult {
                        request_id,
                        result: Some(value),
                        error: None,
                    },
                    Err(error) => HostMessageBody::ExportResult {
                        request_id,
                        result: None,
                        error: Some(error),
                    },
                };
                store.data_mut().protocol.current_invocation_context_id = None;
                store.data_mut().protocol.send(body)?;
            }
            HostMessageBody::RevokeLease { lease_id } => {
                if lease_id != store.data().protocol.lease_id {
                    bail!("protocol");
                }
            }
            HostMessageBody::Shutdown => return Ok(()),
            _ => bail!("protocol"),
        }
    }
}

fn prepare_call(store: &mut Store<HostState>, limits: &RuntimeLimits, export: &str) -> Result<()> {
    store
        .set_fuel(limits.fuel_per_guest_call)
        .map_err(|_| anyhow::anyhow!("fuel"))?;
    let timeout = if export == "activate" {
        limits.activate_timeout_ms
    } else if export == "deactivate" {
        limits.deactivate_timeout_ms
    } else {
        limits.wall_timeout_ms
    };
    store.set_epoch_deadline(timeout.div_ceil(limits.epoch_tick_ms).max(1));
    Ok(())
}

async fn invoke_activate(
    bindings: &ExtensionV1,
    store: &mut Store<HostState>,
    input: &Value,
) -> Result<Value, HostStableError> {
    let context = types::ActivationContext {
        extension_id: required_input_string(input, "extensionId")?,
        instance_id: required_input_string(input, "instanceId")?,
        api_major: input
            .get("apiMajor")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .ok_or_else(resource_error)?,
        lifecycle_generation: input
            .get("lifecycleGeneration")
            .and_then(Value::as_u64)
            .ok_or_else(resource_error)?,
        kind: match input.get("kind").and_then(Value::as_str) {
            Some("background") => types::ActivationKind::Background,
            Some("interactive_ui") => types::ActivationKind::InteractiveUi,
            _ => return Err(resource_error()),
        },
    };
    match bindings
        .alcomd_extension_guest_lifecycle()
        .call_activate(store, &context)
        .await
    {
        Ok(Ok(())) => Ok(json!({})),
        Ok(Err(error)) => Err(wit_error(error)),
        Err(error) => Err(trap_error(&error)),
    }
}

async fn invoke_deactivate(
    bindings: &ExtensionV1,
    store: &mut Store<HostState>,
    input: &Value,
) -> Result<Value, HostStableError> {
    let reason = match input.get("reason").and_then(Value::as_str) {
        Some("disabled") => types::StopReason::Disabled,
        Some("permission_revoked") => types::StopReason::PermissionRevoked,
        Some("lease_expired") => types::StopReason::LeaseExpired,
        Some("daemon_shutdown") => types::StopReason::DaemonShutdown,
        Some("uninstalling") => types::StopReason::Uninstalling,
        Some("interactive_ui_idle") => types::StopReason::InteractiveUiIdle,
        _ => return Err(resource_error()),
    };
    match bindings
        .alcomd_extension_guest_lifecycle()
        .call_deactivate(store, reason)
        .await
    {
        Ok(Ok(())) => Ok(json!({})),
        Ok(Err(error)) => Err(wit_error(error)),
        Err(error) => Err(trap_error(&error)),
    }
}

async fn invoke_ui_open(
    bindings: &ExtensionV1,
    store: &mut Store<HostState>,
    input: &Value,
) -> Result<Value, HostStableError> {
    let session_id = required_input_string(input, "sessionId")?;
    let locale = required_input_string(input, "locale")?;
    bindings
        .alcomd_extension_guest_ui()
        .call_open(store, &session_id, &locale)
        .await
        .map(|document| ui_document(&document))
        .map_err(|error| trap_error(&error))
}

async fn invoke_ui_refresh(
    bindings: &ExtensionV1,
    store: &mut Store<HostState>,
    input: &Value,
) -> Result<Value, HostStableError> {
    let session_id = required_input_string(input, "sessionId")?;
    bindings
        .alcomd_extension_guest_ui()
        .call_refresh(store, &session_id)
        .await
        .map(|document| ui_document(&document))
        .map_err(|error| trap_error(&error))
}

async fn invoke_ui_dispatch(
    bindings: &ExtensionV1,
    store: &mut Store<HostState>,
    input: &Value,
) -> Result<Value, HostStableError> {
    let session_id = required_input_string(input, "sessionId")?;
    let action = input
        .get("action")
        .ok_or_else(resource_error)
        .and_then(ui_action)?;
    bindings
        .alcomd_extension_guest_ui()
        .call_dispatch(store, &session_id, &action)
        .await
        .map(|document| ui_document(&document))
        .map_err(|error| trap_error(&error))
}

async fn invoke_ui_close(
    bindings: &ExtensionV1,
    store: &mut Store<HostState>,
    input: &Value,
) -> Result<Value, HostStableError> {
    let session_id = required_input_string(input, "sessionId")?;
    bindings
        .alcomd_extension_guest_ui()
        .call_close(store, &session_id)
        .await
        .map(|()| json!({}))
        .map_err(|error| trap_error(&error))
}

fn ui_document(document: &guest_ui::UiDocument) -> Value {
    json!({
        "protocol": "portable-v1",
        "title": document.title,
        "nodes": document.nodes.iter().map(ui_node).collect::<Vec<_>>()
    })
}

fn ui_node(node: &guest_ui::UiNode) -> Value {
    let (kind, payload) = match &node.payload {
        guest_ui::NodePayload::Page(value) => ("page", json!({"title": value.title})),
        guest_ui::NodePayload::Section(value) => ("section", json!({"label": value.label})),
        guest_ui::NodePayload::Stack(value) => (
            "stack",
            json!({"orientation": match value.orientation {
                guest_ui::StackOrientation::Vertical => "vertical",
                guest_ui::StackOrientation::Horizontal => "horizontal",
            }}),
        ),
        guest_ui::NodePayload::Group(value) => ("group", optional_label(&value.label)),
        guest_ui::NodePayload::Form(value) => (
            "form",
            json!({
                "submitActionId": value.submit_action_id,
                "submitLabel": value.submit_label,
                "disabled": value.disabled
            }),
        ),
        guest_ui::NodePayload::List(value) => ("list", optional_label(&value.label)),
        guest_ui::NodePayload::ListItem(value) => ("list-item", optional_label(&value.label)),
        guest_ui::NodePayload::Text(value) => (
            "text",
            json!({"text": value.text, "tone": ui_tone(value.tone)}),
        ),
        guest_ui::NodePayload::Status(value) => (
            "status",
            json!({"label": value.label, "tone": ui_tone(value.tone)}),
        ),
        guest_ui::NodePayload::KeyValue(value) => (
            "key-value",
            json!({"label": value.label, "value": value.value}),
        ),
        guest_ui::NodePayload::Progress(value) => (
            "progress",
            json!({"label": value.label, "value": match value.value {
                guest_ui::ProgressValue::Indeterminate => json!({"mode": "indeterminate"}),
                guest_ui::ProgressValue::BasisPoints(value) => {
                    json!({"mode": "determinate", "basisPoints": value})
                }
            }}),
        ),
        guest_ui::NodePayload::Divider => ("divider", json!({})),
        guest_ui::NodePayload::Button(value) => (
            "button",
            json!({
                "label": value.label,
                "actionId": value.action_id,
                "disabled": value.disabled
            }),
        ),
        guest_ui::NodePayload::Switch(value) => (
            "switch",
            field_payload(
                json!({
                    "fieldId": value.field_id,
                    "label": value.label,
                    "initialValue": value.initial_value,
                    "disabled": value.disabled,
                    "readOnly": value.read_only
                }),
                value.validation.as_ref(),
            ),
        ),
        guest_ui::NodePayload::TextField(value) => (
            "text-field",
            field_payload(
                json!({
                    "fieldId": value.field_id,
                    "label": value.label,
                    "initialValue": value.initial_value,
                    "required": value.required,
                    "minLength": value.min_length,
                    "maxLength": value.max_length,
                    "multiline": value.multiline,
                    "disabled": value.disabled,
                    "readOnly": value.read_only
                }),
                value.validation.as_ref(),
            ),
        ),
        guest_ui::NodePayload::IntegerField(value) => (
            "integer-field",
            field_payload(
                json!({
                    "fieldId": value.field_id,
                    "label": value.label,
                    "initialValue": value.initial_value,
                    "required": value.required,
                    "minimum": value.minimum,
                    "maximum": value.maximum,
                    "disabled": value.disabled,
                    "readOnly": value.read_only
                }),
                value.validation.as_ref(),
            ),
        ),
        guest_ui::NodePayload::Select(value) => (
            "select",
            field_payload(
                json!({
                    "fieldId": value.field_id,
                    "label": value.label,
                    "initialOptionId": value.initial_option_id,
                    "required": value.required,
                    "options": value.options.iter().map(|option| json!({
                        "optionId": option.option_id,
                        "label": option.label
                    })).collect::<Vec<_>>(),
                    "disabled": value.disabled,
                    "readOnly": value.read_only
                }),
                value.validation.as_ref(),
            ),
        ),
    };
    let mut encoded = serde_json::Map::new();
    encoded.insert("kind".to_owned(), Value::String(kind.to_owned()));
    encoded.insert("nodeId".to_owned(), Value::String(node.node_id.clone()));
    if let Some(parent_id) = &node.parent_id {
        encoded.insert("parentId".to_owned(), Value::String(parent_id.clone()));
    }
    encoded.insert("order".to_owned(), Value::from(node.order));
    encoded.insert("payload".to_owned(), payload);
    Value::Object(encoded)
}

fn optional_label(label: &Option<String>) -> Value {
    label
        .as_ref()
        .map_or_else(|| json!({}), |label| json!({"label": label}))
}

fn ui_tone(tone: guest_ui::Tone) -> &'static str {
    match tone {
        guest_ui::Tone::Neutral => "neutral",
        guest_ui::Tone::Info => "info",
        guest_ui::Tone::Success => "success",
        guest_ui::Tone::Warning => "warning",
        guest_ui::Tone::Danger => "danger",
    }
}

fn field_payload(mut value: Value, validation: Option<&guest_ui::FieldValidation>) -> Value {
    if let (Some(object), Some(validation)) = (value.as_object_mut(), validation) {
        object.insert(
            "validation".to_owned(),
            match validation {
                guest_ui::FieldValidation::Valid => json!({"state": "valid"}),
                guest_ui::FieldValidation::Invalid(message) => {
                    json!({"state": "invalid", "message": message})
                }
            },
        );
    }
    value
}

fn ui_action(value: &Value) -> Result<guest_ui::UiAction, HostStableError> {
    match value.get("kind").and_then(Value::as_str) {
        Some("activate") => {
            required_input_string(value, "actionId").map(guest_ui::UiAction::Activate)
        }
        Some("submit-form") => {
            let action_id = required_input_string(value, "actionId")?;
            let values = value
                .get("values")
                .and_then(Value::as_array)
                .ok_or_else(resource_error)?
                .iter()
                .map(ui_submitted_field)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(guest_ui::UiAction::SubmitForm(guest_ui::FormSubmission {
                action_id,
                values,
            }))
        }
        _ => Err(resource_error()),
    }
}

fn ui_submitted_field(value: &Value) -> Result<guest_ui::SubmittedField, HostStableError> {
    let field_id = required_input_string(value, "fieldId")?;
    let value = value.get("value").ok_or_else(resource_error)?;
    let value = match value.get("kind").and_then(Value::as_str) {
        Some("boolean") => value
            .get("value")
            .and_then(Value::as_bool)
            .map(guest_ui::FieldValue::Boolean),
        Some("text") => value
            .get("value")
            .and_then(Value::as_str)
            .map(|value| guest_ui::FieldValue::Text(value.to_owned())),
        Some("integer") => value
            .get("value")
            .and_then(Value::as_i64)
            .map(guest_ui::FieldValue::Integer),
        Some("selection") => value
            .get("value")
            .and_then(Value::as_str)
            .map(|value| guest_ui::FieldValue::Selection(value.to_owned())),
        _ => None,
    }
    .ok_or_else(resource_error)?;
    Ok(guest_ui::SubmittedField { field_id, value })
}

impl host_projects::Host for HostState {
    async fn get_summary(
        &mut self,
        id: String,
    ) -> Result<types::ProjectSummary, types::ExtensionError> {
        let result = self
            .protocol
            .capability("host-projects.get-summary", json!({"projectId": id}))?;
        parse_project_summary(result)
    }
}

impl types::Host for HostState {}

impl host_data::Host for HostState {
    async fn get(
        &mut self,
        key: String,
    ) -> Result<Option<types::DataValue>, types::ExtensionError> {
        let result = self
            .protocol
            .capability("host-data.get", json!({"key": key}))?;
        parse_data_value(result)
    }

    async fn set(
        &mut self,
        key: String,
        value: Vec<u8>,
        expected_key_revision: Option<u64>,
    ) -> Result<types::DataWriteResult, types::ExtensionError> {
        let result = self.protocol.capability(
            "host-data.set",
            json!({
                "key": key,
                "value": value,
                "expectedKeyRevision": expected_key_revision
            }),
        )?;
        parse_data_write(result)
    }

    async fn delete(
        &mut self,
        key: String,
        expected_key_revision: u64,
    ) -> Result<types::DataWriteResult, types::ExtensionError> {
        let result = self.protocol.capability(
            "host-data.delete",
            json!({"key": key, "expectedKeyRevision": expected_key_revision}),
        )?;
        parse_data_write(result)
    }
}

impl ProtocolChannel {
    fn validate_inbound(&mut self, message: &HostMessage) -> Result<()> {
        if message.protocol_version != HOST_PROTOCOL_VERSION
            || message.daemon_epoch != self.daemon_epoch
            || message.instance_id != self.instance_id
            || message.lifecycle_generation != self.lifecycle_generation
            || message.sequence != self.inbound_sequence + 1
        {
            bail!("protocol");
        }
        self.inbound_sequence = message.sequence;
        Ok(())
    }

    fn send(&mut self, body: HostMessageBody) -> Result<()> {
        self.outbound_sequence = self.outbound_sequence.checked_add(1).context("sequence")?;
        write_host_message(
            &mut self.output,
            &HostMessage {
                protocol_version: HOST_PROTOCOL_VERSION,
                daemon_epoch: self.daemon_epoch.clone(),
                instance_id: self.instance_id.clone(),
                lifecycle_generation: self.lifecycle_generation,
                sequence: self.outbound_sequence,
                body,
            },
        )
        .map_err(|_| anyhow::anyhow!("protocol"))
    }

    fn capability(
        &mut self,
        capability: &str,
        input: Value,
    ) -> Result<Value, types::ExtensionError> {
        if serde_json::to_vec(&input).map_or(true, |value| value.len() > self.wit_input_bytes)
            || !self.consume_call_budget()
        {
            return Err(resource_wit_error());
        }
        self.capability_inner(capability, input)
            .map_err(|_| {
                self.failed = true;
                internal_wit_error()
            })
            .and_then(|value| {
                if serde_json::to_vec(&value)
                    .map_or(true, |value| value.len() > self.wit_output_bytes)
                {
                    Err(resource_wit_error())
                } else {
                    Ok(value)
                }
            })
    }

    fn consume_call_budget(&mut self) -> bool {
        let now = Instant::now();
        let window = std::time::Duration::from_millis(self.call_window_ms);
        while self
            .call_times
            .front()
            .is_some_and(|value| now.duration_since(*value) >= window)
        {
            self.call_times.pop_front();
        }
        if self.call_times.len() >= self.calls_per_window {
            return false;
        }
        let refill_per_ms = self.calls_per_window as f64 / self.call_window_ms as f64;
        self.call_tokens = (self.call_tokens
            + now.duration_since(self.token_updated_at).as_millis() as f64 * refill_per_ms)
            .min(self.call_burst);
        self.token_updated_at = now;
        if self.call_tokens < 1.0 {
            return false;
        }
        self.call_tokens -= 1.0;
        self.call_times.push_back(now);
        true
    }

    fn capability_inner(&mut self, capability: &str, input: Value) -> Result<Value> {
        let call_id = format!("call-{}", self.next_call);
        self.next_call = self.next_call.checked_add(1).context("call sequence")?;
        self.send(HostMessageBody::CapabilityCall {
            call_id: call_id.clone(),
            invocation_context_id: self
                .current_invocation_context_id
                .clone()
                .context("invocation context")?,
            lease_id: self.lease_id.clone(),
            capability: capability.to_owned(),
            input,
        })?;
        let message =
            read_host_message(&mut self.input).map_err(|_| anyhow::anyhow!("protocol"))?;
        self.validate_inbound(&message)?;
        let HostMessageBody::CapabilityResult {
            call_id: response_id,
            result,
            error,
        } = message.body
        else {
            bail!("protocol");
        };
        if response_id != call_id {
            bail!("protocol");
        }
        match (result, error) {
            (Some(value), None) => Ok(value),
            (None, Some(error)) => Ok(json!({"error": error})),
            _ => bail!("protocol"),
        }
    }
}

fn parse_project_summary(value: Value) -> Result<types::ProjectSummary, types::ExtensionError> {
    if let Some(error) = value.get("error") {
        return Err(parse_wit_error(error));
    }
    let value = value.get("summary").ok_or_else(internal_wit_error)?;
    let kind = match value.get("kind").and_then(Value::as_str) {
        Some("vpm") => types::ProjectKind::Vpm,
        Some("upm") => types::ProjectKind::Upm,
        Some("unknown") => types::ProjectKind::Unknown,
        _ => return Err(internal_wit_error()),
    };
    Ok(types::ProjectSummary {
        project_id: required_string(value, "projectId")?,
        display_name: required_string(value, "displayName")?,
        kind,
        unity_version: value
            .get("unityVersion")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        revision: value
            .get("revision")
            .and_then(Value::as_u64)
            .ok_or_else(internal_wit_error)?,
    })
}

fn parse_data_value(value: Value) -> Result<Option<types::DataValue>, types::ExtensionError> {
    if let Some(error) = value.get("error") {
        return Err(parse_wit_error(error));
    }
    let Some(value) = value.get("value") else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    Ok(Some(types::DataValue {
        bytes: byte_array(value.get("bytes").ok_or_else(internal_wit_error)?)?,
        key_revision: value
            .get("keyRevision")
            .and_then(Value::as_u64)
            .ok_or_else(internal_wit_error)?,
        namespace_revision: value
            .get("namespaceRevision")
            .and_then(Value::as_u64)
            .ok_or_else(internal_wit_error)?,
    }))
}

fn parse_data_write(value: Value) -> Result<types::DataWriteResult, types::ExtensionError> {
    if let Some(error) = value.get("error") {
        return Err(parse_wit_error(error));
    }
    Ok(types::DataWriteResult {
        key_revision: value
            .get("keyRevision")
            .and_then(Value::as_u64)
            .ok_or_else(internal_wit_error)?,
        namespace_revision: value
            .get("namespaceRevision")
            .and_then(Value::as_u64)
            .ok_or_else(internal_wit_error)?,
    })
}

fn required_string(value: &Value, field: &str) -> Result<String, types::ExtensionError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(internal_wit_error)
}

fn required_input_string(value: &Value, field: &str) -> Result<String, HostStableError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(resource_error)
}

fn byte_array(value: &Value) -> Result<Vec<u8>, types::ExtensionError> {
    value
        .as_array()
        .ok_or_else(internal_wit_error)?
        .iter()
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| u8::try_from(value).ok())
                .ok_or_else(internal_wit_error)
        })
        .collect()
}

fn parse_wit_error(value: &Value) -> types::ExtensionError {
    let code = match value.get("code").and_then(Value::as_str) {
        Some("extension_permission_denied") => types::ErrorCode::PermissionDenied,
        Some("extension_scope_denied") => types::ErrorCode::ScopeDenied,
        Some("extension_instance_stale") => types::ErrorCode::LeaseRevoked,
        Some("extension_resource_limit") => types::ErrorCode::ResourceLimit,
        Some("project_not_found") => types::ErrorCode::ProjectNotFound,
        Some("revision_conflict") => types::ErrorCode::DataRevisionConflict,
        Some("extension_data_quota_exceeded") => types::ErrorCode::DataQuotaExceeded,
        Some("cancelled") => types::ErrorCode::LeaseRevoked,
        _ => types::ErrorCode::InternalError,
    };
    types::ExtensionError {
        code,
        diagnostic_id: value
            .get("diagnosticId")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
    }
}

fn wit_error(error: types::ExtensionError) -> HostStableError {
    let internal = error.code == types::ErrorCode::InternalError;
    HostStableError {
        code: match error.code {
            types::ErrorCode::PermissionDenied => "extension_permission_denied",
            types::ErrorCode::ScopeDenied => "extension_scope_denied",
            types::ErrorCode::LeaseExpired | types::ErrorCode::LeaseRevoked => {
                "extension_instance_stale"
            }
            types::ErrorCode::ProjectNotFound => "project_not_found",
            types::ErrorCode::DataRevisionConflict => "revision_conflict",
            types::ErrorCode::DataQuotaExceeded => "extension_data_quota_exceeded",
            types::ErrorCode::ResourceLimit => "extension_resource_limit",
            _ => "internal_error",
        }
        .to_owned(),
        diagnostic_id: error.diagnostic_id.or_else(|| internal.then(diagnostic_id)),
    }
}

fn internal_wit_error() -> types::ExtensionError {
    types::ExtensionError {
        code: types::ErrorCode::InternalError,
        diagnostic_id: Some(diagnostic_id()),
    }
}

fn resource_wit_error() -> types::ExtensionError {
    types::ExtensionError {
        code: types::ErrorCode::ResourceLimit,
        diagnostic_id: None,
    }
}

fn resource_error() -> HostStableError {
    HostStableError {
        code: "extension_resource_limit".to_owned(),
        diagnostic_id: None,
    }
}

fn crashed_error() -> HostStableError {
    HostStableError {
        code: "extension_crashed".to_owned(),
        diagnostic_id: None,
    }
}

fn trap_error(error: &wasmtime::Error) -> HostStableError {
    if error.downcast_ref::<wasmtime::Trap>().is_some_and(|trap| {
        matches!(
            trap,
            wasmtime::Trap::OutOfFuel
                | wasmtime::Trap::Interrupt
                | wasmtime::Trap::MemoryOutOfBounds
                | wasmtime::Trap::TableOutOfBounds
                | wasmtime::Trap::AllocationTooLarge
                | wasmtime::Trap::StackOverflow
        )
    }) {
        resource_error()
    } else {
        crashed_error()
    }
}

fn diagnostic_id() -> String {
    alcomd_application::OperationId::new().to_string()
}

fn create_engine() -> Result<Engine> {
    let mut config = Config::new();
    config
        .wasm_component_model(true)
        .concurrency_support(false)
        .consume_fuel(true)
        .epoch_interruption(true);
    Engine::new(&config).map_err(|_| anyhow::anyhow!("engine"))
}

fn start_epoch_ticker(
    engine: Engine,
    running: Arc<AtomicBool>,
    tick_ms: u64,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        while running.load(Ordering::Acquire) {
            std::thread::sleep(std::time::Duration::from_millis(tick_ms));
            engine.increment_epoch();
        }
    })
}

fn validate_component_path(path: &PathBuf) -> Result<()> {
    if !path.is_absolute()
        || path.file_name().and_then(|value| value.to_str()) != Some("extension.wasm")
    {
        bail!("component");
    }
    let metadata = std::fs::symlink_metadata(path).context("component")?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        bail!("component");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn protocol_channel() -> ProtocolChannel {
        ProtocolChannel {
            input: std::io::stdin(),
            output: std::io::stdout(),
            daemon_epoch: "00000000-0000-4000-8000-000000000001".to_owned(),
            instance_id: "00000000-0000-4000-8000-000000000002".to_owned(),
            lifecycle_generation: 7,
            lease_id: "00000000-0000-4000-8000-000000000003".to_owned(),
            inbound_sequence: 1,
            outbound_sequence: 1,
            next_call: 1,
            failed: false,
            call_times: VecDeque::new(),
            call_tokens: 1.0,
            token_updated_at: Instant::now(),
            call_window_ms: 60_000,
            calls_per_window: 2,
            call_burst: 1.0,
            wit_input_bytes: 256 * 1024,
            wit_output_bytes: 256 * 1024,
            current_invocation_context_id: None,
        }
    }

    fn inbound_message() -> HostMessage {
        HostMessage {
            protocol_version: HOST_PROTOCOL_VERSION,
            daemon_epoch: "00000000-0000-4000-8000-000000000001".to_owned(),
            instance_id: "00000000-0000-4000-8000-000000000002".to_owned(),
            lifecycle_generation: 7,
            sequence: 2,
            body: HostMessageBody::CapabilityResult {
                call_id: "call-1".to_owned(),
                result: Some(json!({"value": null})),
                error: None,
            },
        }
    }

    #[test]
    fn inbound_authority_and_sequence_mismatch_fail_closed() {
        let mut channel = protocol_channel();
        let valid = inbound_message();
        assert!(channel.validate_inbound(&valid).is_ok());
        assert!(
            channel.validate_inbound(&valid).is_err(),
            "duplicate sequence"
        );

        let mut wrong_epoch = inbound_message();
        wrong_epoch.daemon_epoch = "00000000-0000-4000-8000-000000000009".to_owned();
        let mut wrong_instance = inbound_message();
        wrong_instance.instance_id = "00000000-0000-4000-8000-000000000009".to_owned();
        let mut wrong_generation = inbound_message();
        wrong_generation.lifecycle_generation = 8;
        for invalid in [wrong_epoch, wrong_instance, wrong_generation] {
            let mut channel = protocol_channel();
            assert!(channel.validate_inbound(&invalid).is_err());
        }
    }

    #[test]
    fn host_call_token_and_window_limits_are_enforced_before_routing() {
        let mut channel = protocol_channel();
        assert!(channel.consume_call_budget());
        assert!(!channel.consume_call_budget());
        channel.call_times.push_back(Instant::now());
        assert!(!channel.consume_call_budget());
    }
}
