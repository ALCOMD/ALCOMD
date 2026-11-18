#![forbid(unsafe_code)]
use crate::bindings::alcomd::extension::types;
use crate::bindings::alcomd::mcp_management::clients as host;
use crate::bindings::alcomd::mcp_runtime::observations as runtime;
use crate::bindings::alcomd::mcp_transport::observations as transport;
use crate::bindings::exports::alcomd::extension::{guest_lifecycle, guest_ui as ui};
use std::cell::RefCell;
use std::collections::BTreeMap;

const MCP_PERMISSIONS: [&str; 9] = [
    "projects.read",
    "packages.manage",
    "state.check",
    "operations.read",
    "operations.cancel",
    "events.read",
    "repositories.read",
    "repositories.manage",
    "packages.read",
];

pub struct Management;
thread_local! { static SESSIONS: RefCell<BTreeMap<String, View>> = const { RefCell::new(BTreeMap::new()) }; }
#[derive(Default)]
struct View {
    clients: Vec<host::ClientSummary>,
    selected: Option<host::ClientSummary>,
    client_cursor: Option<String>,
    next_clients: Option<String>,
    grants: Vec<host::Grant>,
    grants_loaded: bool,
    confirmation: Option<Confirmation>,
    message: Option<(String, ui::Tone)>,
    unknown_write: bool,
    can_manage: bool,
    activity: bool,
    runtime: Option<runtime::RuntimeSnapshot>,
    runtime_message: Option<String>,
    stream_snapshot: Option<transport::TransportSnapshot>,
    stream_message: Option<String>,
    counter: u64,
}
enum Confirmation {
    Revoke,
    Change {
        add: Vec<host::Grant>,
        remove: Vec<host::Grant>,
    },
}

impl guest_lifecycle::Guest for Management {
    fn activate(_: types::ActivationContext) -> Result<(), types::ExtensionError> {
        Ok(())
    }
    fn deactivate(_: types::StopReason) -> Result<(), types::ExtensionError> {
        SESSIONS.with(|sessions| sessions.borrow_mut().clear());
        Ok(())
    }
}
impl ui::Guest for Management {
    fn open(session_id: String, _: String) -> ui::UiDocument {
        SESSIONS.with(|sessions| {
            let mut sessions = sessions.borrow_mut();
            if sessions.len() >= 32 {
                return error_document("打开的扩展页面过多，请先关闭其他页面。");
            }
            let mut view = View::default();
            view.load_clients();
            view.load_runtime();
            let document = view.document();
            sessions.insert(session_id, view);
            document
        })
    }
    fn refresh(session_id: String) -> ui::UiDocument {
        with_view(&session_id, |view| {
            view.confirmation = None;
            view.message = None;
            if view.activity && !view.unknown_write {
                view.load_runtime();
                return;
            }
            // Refresh authoritative metadata first, including the selected client's revision.
            view.client_cursor = None;
            if view.load_clients() {
                view.unknown_write = false;
                if view.selected.is_some() {
                    view.load_grants();
                }
            }
            view.load_runtime();
        })
    }
    fn dispatch(session_id: String, action: ui::UiAction) -> ui::UiDocument {
        with_view(&session_id, |view| view.act(&session_id, action))
    }
    fn close(session_id: String) {
        SESSIONS.with(|sessions| {
            sessions.borrow_mut().remove(&session_id);
        });
    }
}
fn with_view(id: &str, action: impl FnOnce(&mut View)) -> ui::UiDocument {
    SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let Some(view) = sessions.get_mut(id) else {
            return error_document("页面会话已结束，请关闭后重新打开。");
        };
        action(view);
        view.document()
    })
}
impl View {
    fn fail(&mut self, error: host::ManagementError) {
        let retained_unknown = self.unknown_write;
        self.unknown_write |= matches!(error.code, host::ErrorCode::WriteOutcomeUnknown);
        let text =
            if retained_unknown && !matches!(error.code, host::ErrorCode::WriteOutcomeUnknown) {
                format!("之前的写入结果仍未确认。{}", error_text(error.code))
            } else {
                error_text(error.code).into()
            };
        self.message = Some((text, ui::Tone::Danger));
        if matches!(error.code, host::ErrorCode::PermissionDenied) {
            self.can_manage = false;
        }
    }
    fn load_runtime(&mut self) {
        self.runtime = None;
        self.runtime_message = None;
        match runtime::snapshot(None, Some(25)) {
            Ok(snapshot) => self.runtime = Some(snapshot),
            Err(error) if matches!(error.code, host::ErrorCode::PermissionDenied) => {}
            Err(error) => self.runtime_message = Some(error_text(error.code).into()),
        }
        self.stream_snapshot = None;
        self.stream_message = None;
        match transport::snapshot(None, Some(25)) {
            Ok(snapshot) if snapshot.can_read_subscription_streams => {
                self.stream_snapshot = Some(snapshot);
            }
            Ok(_) => {}
            Err(error) if matches!(error.code, host::ErrorCode::PermissionDenied) => {}
            Err(error) => self.stream_message = Some(error_text(error.code).into()),
        }
    }
    fn activity_visible(&self) -> bool {
        self.runtime.is_some()
            || self.runtime_message.is_some()
            || self.stream_snapshot.is_some()
            || self.stream_message.is_some()
    }
    fn runtime_document(&self, doc: &mut Document) {
        doc.button("返回客户端", "clients", false);
        if let Some(error) = &self.runtime_message {
            doc.status(error, ui::Tone::Danger);
        }
        if let Some(snapshot) = &self.runtime {
            if snapshot.coverage_incomplete {
                doc.status("部分连接未被跟踪，活动列表不完整。", ui::Tone::Warning);
            }
            if snapshot.can_read_connections
                && let Some(page) = &snapshot.connections
            {
                let list = doc.node(
                    None,
                    ui::NodePayload::List(ui::ListNode {
                        label: Some(format!("Core 连接 · {}", page.total_tracked)),
                    }),
                );
                for row in &page.items {
                    let label = format!(
                        "{} · {}",
                        self.runtime_client(&row.client_id),
                        if row.active_call_count > 0 {
                            "正在调用"
                        } else {
                            "Core 连接存在"
                        }
                    );
                    doc.node(
                        Some(&list),
                        ui::NodePayload::ListItem(ui::ListItemNode { label: Some(label) }),
                    );
                }
                if page.items.is_empty() {
                    doc.status("当前没有已跟踪的 Core 连接。", ui::Tone::Neutral);
                }
                if page.truncated {
                    doc.status(
                        "仅显示部分 Core 连接，可刷新查看当前快照。",
                        ui::Tone::Warning,
                    );
                }
            }
            if snapshot.can_read_requests
                && let Some(page) = &snapshot.requests
            {
                let list = doc.node(
                    None,
                    ui::NodePayload::List(ui::ListNode {
                        label: Some(format!("Core 调用 · {}", page.total_tracked)),
                    }),
                );
                for row in &page.items {
                    let elapsed = snapshot.observed_at_ms.saturating_sub(row.started_at_ms) / 1000;
                    let label = format!(
                        "{} · {} · {} 秒",
                        self.runtime_client(&row.client_id),
                        runtime_method_label(&row.method),
                        elapsed
                    );
                    doc.node(
                        Some(&list),
                        ui::NodePayload::ListItem(ui::ListItemNode { label: Some(label) }),
                    );
                }
                if page.items.is_empty() {
                    doc.status("当前没有已跟踪的 Core 调用。", ui::Tone::Neutral);
                }
                if page.truncated {
                    doc.status(
                        "仅显示部分 Core 调用，可刷新查看当前快照。",
                        ui::Tone::Warning,
                    );
                }
            }
        }
        self.stream_document(doc);
        if !self.activity_visible() {
            doc.status("尚未授权查看活动。", ui::Tone::Neutral);
        }
        doc.button("刷新活动", "runtime-refresh", false);
    }
    fn stream_document(&self, doc: &mut Document) {
        if let Some(error) = &self.stream_message {
            doc.status(&format!("无法读取订阅流。{error}"), ui::Tone::Danger);
        }
        let Some(snapshot) = &self.stream_snapshot else {
            return;
        };
        match snapshot.status {
            transport::Availability::Unavailable => {
                doc.status("当前没有可用的 MCP 订阅流报告。", ui::Tone::Neutral);
                return;
            }
            transport::Availability::Stale => {
                doc.status("MCP 订阅流报告已过期，请刷新。", ui::Tone::Warning);
                return;
            }
            transport::Availability::Available => {}
        }
        let Some(page) = &snapshot.subscription_streams else {
            return;
        };
        if snapshot.coverage_incomplete {
            doc.status("部分 MCP 订阅流未被跟踪。", ui::Tone::Warning);
        }
        if page.items.is_empty() {
            if !snapshot.coverage_incomplete {
                doc.status("当前没有已跟踪的 MCP 订阅流。", ui::Tone::Neutral);
            }
            return;
        }
        let list = doc.node(
            None,
            ui::NodePayload::List(ui::ListNode {
                label: Some(format!("MCP 订阅流（适配器上报） · {}", page.total_tracked)),
            }),
        );
        for stream in &page.items {
            let client = stream
                .client_id
                .as_deref()
                .map_or_else(|| "未关联客户端".into(), |id| self.runtime_client(id));
            let elapsed = snapshot
                .observed_at_ms
                .saturating_sub(stream.first_observed_at_ms)
                / 1000;
            let kind = if stream.tools_list_changed {
                "工具列表变更"
            } else {
                "订阅"
            };
            let label = format!("{client} · {kind} · {elapsed} 秒");
            doc.node(
                Some(&list),
                ui::NodePayload::ListItem(ui::ListItemNode { label: Some(label) }),
            );
        }
        if page.truncated {
            doc.status("仅显示部分已跟踪的 MCP 订阅流。", ui::Tone::Warning);
        }
    }
    fn runtime_client(&self, id: &str) -> String {
        self.clients
            .iter()
            .find(|client| client.client_id == id)
            .map(|client| client.display_name.clone())
            .unwrap_or_else(|| format!("客户端 {}", id.chars().take(8).collect::<String>()))
    }
    fn load_clients(&mut self) -> bool {
        self.can_manage = false;
        match host::list(self.client_cursor.as_deref(), Some(32)) {
            Ok(page) => {
                if let Some(selected) = &self.selected {
                    let selected_id = selected.client_id.clone();
                    let mut found = page
                        .items
                        .iter()
                        .find(|item| item.client_id == selected_id)
                        .cloned();
                    let mut cursor = page.next_cursor.clone();
                    while found.is_none() && cursor.is_some() {
                        match host::list(cursor.as_deref(), Some(32)) {
                            Ok(next) => {
                                found = next
                                    .items
                                    .into_iter()
                                    .find(|item| item.client_id == selected_id);
                                cursor = next.next_cursor;
                            }
                            Err(error) => {
                                self.grants.clear();
                                self.fail(error);
                                return false;
                            }
                        }
                    }
                    if found.is_none() && self.client_cursor.is_some() {
                        // A selected record can be on an earlier page; restart the explicit refresh.
                        self.client_cursor = None;
                        return self.load_clients();
                    }
                    self.selected = found;
                }
                self.clients = page.items;
                self.next_clients = page.next_cursor;
                self.can_manage = page.can_manage;
                true
            }
            Err(error) => {
                self.clients.clear();
                self.next_clients = None;
                if matches!(error.code, host::ErrorCode::PermissionDenied) {
                    self.selected = None;
                    self.grants.clear();
                    self.grants_loaded = false;
                    if self.unknown_write {
                        self.fail(error);
                    } else {
                        self.message = Some((
                            "尚未授权读取 MCP 客户端。请在扩展管理中授权后重新打开页面。".into(),
                            ui::Tone::Neutral,
                        ));
                    }
                } else {
                    self.fail(error);
                }
                false
            }
        }
    }
    fn load_grants(&mut self) {
        self.grants_loaded = false;
        self.grants.clear();
        let Some(selected) = &self.selected else {
            return;
        };
        let id = selected.client_id.clone();
        let revision = selected.revision;
        let mut cursor = None;
        loop {
            match host::grants(&id, revision, cursor.as_deref(), Some(64)) {
                Ok(page) if page.revision == revision => {
                    self.grants.extend(page.items);
                    cursor = page.next_cursor;
                    if cursor.is_none() {
                        self.grants_loaded = true;
                        break;
                    }
                }
                Ok(_) => {
                    self.grants.clear();
                    self.message = Some(("记录已变化，请刷新后再操作。".into(), ui::Tone::Danger));
                    break;
                }
                Err(error) => {
                    self.grants.clear();
                    self.fail(error);
                    break;
                }
            }
        }
    }
    fn key(&mut self, id: &str) -> String {
        self.counter = self.counter.saturating_add(1);
        format!("mcp-ui:{id}:{}", self.counter)
    }
    fn act(&mut self, session_id: &str, action: ui::UiAction) {
        let (id, fields) = match action {
            ui::UiAction::Activate(id) => (id, vec![]),
            ui::UiAction::SubmitForm(form) => (form.action_id, form.values),
        };
        if self.unknown_write
            && !matches!(
                id.as_str(),
                "back" | "cancel" | "activity" | "clients" | "runtime-refresh"
            )
        {
            return; // Only the host's explicit Refresh clears an unknown result.
        }
        if !self.unknown_write {
            self.message = None;
        }
        match id.as_str() {
            "activity" => {
                self.activity = true;
                self.load_runtime();
            }
            "clients" => {
                self.activity = false;
            }
            "runtime-refresh" => self.load_runtime(),
            "back" => {
                self.selected = None;
                self.confirmation = None;
                self.client_cursor = None;
                self.load_clients();
            }
            "clients-next" => {
                self.client_cursor = self.next_clients.clone();
                self.load_clients();
            }
            "clients-first" => {
                self.client_cursor = None;
                self.load_clients();
            }
            "cancel" => {
                self.confirmation = None;
            }
            "create" if self.can_manage => {
                let name = field(&fields, "name");
                let key = self.key(session_id);
                match host::create(&name, &key) {
                    Ok(client) => {
                        self.selected = Some(client);
                        self.load_grants();
                    }
                    Err(error) => self.fail(error),
                }
            }
            "permissions" if self.can_manage && self.grants_loaded => {
                let Some((add, remove)) = permission_changes(&self.grants, &fields) else {
                    self.message = Some((
                        "权限选项不完整，请刷新页面后重试。".into(),
                        ui::Tone::Danger,
                    ));
                    return;
                };
                if add.is_empty() && remove.is_empty() {
                    self.message = Some(("权限未变化。".into(), ui::Tone::Neutral));
                    return;
                }
                self.confirmation = Some(Confirmation::Change { add, remove });
            }
            "revoke" if self.can_manage => self.confirmation = Some(Confirmation::Revoke),
            "confirm" if self.can_manage => {
                let Some(confirmation) = self.confirmation.take() else {
                    return;
                };
                let Some(client) = self.selected.clone() else {
                    return;
                };
                let key = self.key(session_id);
                let result = match confirmation {
                    Confirmation::Revoke => host::revoke(&client.client_id, client.revision, &key),
                    Confirmation::Change { add, remove } => {
                        host::change_grants(&client.client_id, client.revision, &add, &remove, &key)
                    }
                };
                match result {
                    Ok(client) => {
                        self.selected = Some(client);
                        self.load_grants();
                    }
                    Err(error) => self.fail(error),
                }
            }
            _ if id.starts_with("client-") => {
                if let Some(client) = self.clients.iter().find(|c| c.client_id == id[7..]) {
                    self.selected = Some(client.clone());
                    self.confirmation = None;
                    self.load_grants();
                }
            }
            _ => self.message = Some(("当前操作已不可用，请刷新页面。".into(), ui::Tone::Warning)),
        }
    }
    fn document(&self) -> ui::UiDocument {
        let mut doc = Document::new();
        if let Some((message, tone)) = &self.message {
            doc.status(message, *tone);
        }
        if self.activity {
            self.runtime_document(&mut doc);
            return doc.finish();
        }
        if self.selected.is_none() && self.activity_visible() {
            doc.button("活动", "activity", false);
        }
        if let Some(client) = &self.selected {
            doc.button("返回客户端列表", "back", false);
            doc.key("客户端", &client.display_name);
            doc.key("ID", &client.client_id);
            doc.key(
                "状态",
                if client.revoked_at_ms.is_some() {
                    "已撤销"
                } else {
                    "已登记"
                },
            );
            doc.key("授权数量", &client.grant_count.to_string());
            if let Some(confirmation) = &self.confirmation {
                match confirmation {
                    Confirmation::Revoke => doc.status(
                        "撤销后，此客户端的后续请求将被拒绝。已接受的业务任务不会回滚。",
                        ui::Tone::Warning,
                    ),
                    Confirmation::Change { add, remove } => {
                        for grant in add {
                            doc.key("允许", permission_label(&grant.permission));
                        }
                        for grant in remove {
                            doc.key("关闭", permission_label(&grant.permission));
                        }
                        if add
                            .iter()
                            .any(|grant| grant.permission == "packages.manage")
                        {
                            doc.status("允许修改所有现有及以后新建项目的包。实际修改仍需审阅并确认原计划。",ui::Tone::Warning);
                        }
                    }
                }
                doc.button("确认", "confirm", self.unknown_write);
                doc.button("取消", "cancel", false);
            } else {
                let editable = self.can_manage
                    && self.grants_loaded
                    && client.revoked_at_ms.is_none()
                    && !self.unknown_write;
                if !editable {
                    let list = doc.node(
                        None,
                        ui::NodePayload::List(ui::ListNode {
                            label: Some("已允许的权限".into()),
                        }),
                    );
                    for grant in &self.grants {
                        doc.node(
                            Some(&list),
                            ui::NodePayload::ListItem(ui::ListItemNode {
                                label: Some(self.grant_label(grant)),
                            }),
                        );
                    }
                }
                if editable {
                    self.permissions_form(&mut doc);
                    doc.button("撤销客户端", "revoke", false);
                }
            }
        } else {
            let list = doc.node(
                None,
                ui::NodePayload::List(ui::ListNode {
                    label: Some("客户端".into()),
                }),
            );
            for client in &self.clients {
                let row = doc.node(
                    Some(&list),
                    ui::NodePayload::ListItem(ui::ListItemNode {
                        label: Some(format!(
                            "{} · {}",
                            client.display_name,
                            if client.revoked_at_ms.is_some() {
                                "已撤销"
                            } else {
                                "已登记"
                            }
                        )),
                    }),
                );
                doc.node(
                    Some(&row),
                    button("查看权限", &format!("client-{}", client.client_id), false),
                );
            }
            if self.clients.is_empty() && self.message.is_none() {
                doc.status("尚无客户端。", ui::Tone::Neutral);
            }
            if self.client_cursor.is_some() {
                doc.button("第一页", "clients-first", false);
            }
            if self.next_clients.is_some() {
                doc.button("下一页", "clients-next", false);
            }
            if self.can_manage && !self.unknown_write {
                let form = doc.node(
                    None,
                    ui::NodePayload::Form(ui::FormNode {
                        submit_action_id: "create".into(),
                        submit_label: "创建客户端".into(),
                        disabled: false,
                    }),
                );
                doc.node(
                    Some(&form),
                    ui::NodePayload::TextField(ui::TextFieldNode {
                        field_id: "name".into(),
                        label: "客户端名称".into(),
                        initial_value: String::new(),
                        required: true,
                        min_length: 1,
                        max_length: 128,
                        multiline: false,
                        disabled: false,
                        read_only: false,
                        validation: None,
                    }),
                );
            }
        }
        doc.finish()
    }
    fn grant_label(&self, grant: &host::Grant) -> String {
        permission_label(&grant.permission).into()
    }
    fn permissions_form(&self, doc: &mut Document) {
        let form = doc.node(
            None,
            ui::NodePayload::Form(ui::FormNode {
                submit_action_id: "permissions".into(),
                submit_label: "审阅权限修改".into(),
                disabled: false,
            }),
        );
        for permission in MCP_PERMISSIONS {
            doc.node(
                Some(&form),
                ui::NodePayload::Switch(ui::SwitchNode {
                    field_id: permission.into(),
                    label: permission_label(permission).into(),
                    initial_value: self.grants.iter().any(|grant| {
                        grant.permission == permission
                            && grant.resource_kind == "global"
                            && grant.resource_id == "*"
                    }),
                    disabled: false,
                    read_only: false,
                    validation: None,
                }),
            );
        }
    }
}
fn permission_changes(
    grants: &[host::Grant],
    fields: &[ui::SubmittedField],
) -> Option<(Vec<host::Grant>, Vec<host::Grant>)> {
    if fields.len() != MCP_PERMISSIONS.len() {
        return None;
    }
    let mut add = Vec::new();
    let mut remove = Vec::new();
    for permission in MCP_PERMISSIONS {
        let mut matching = fields.iter().filter(|field| field.field_id == permission);
        let ui::FieldValue::Boolean(enabled) = matching.next()?.value else {
            return None;
        };
        if matching.next().is_some() {
            return None;
        }
        let allowed = grants.iter().any(|grant| {
            grant.permission == permission
                && grant.resource_kind == "global"
                && grant.resource_id == "*"
        });
        if enabled != allowed {
            let grant = host::Grant {
                permission: permission.into(),
                resource_kind: "global".into(),
                resource_id: "*".into(),
            };
            if enabled {
                add.push(grant);
            } else {
                remove.push(grant);
            }
        }
    }
    Some((add, remove))
}
fn field(fields: &[ui::SubmittedField], id: &str) -> String {
    fields
        .iter()
        .find(|f| f.field_id == id)
        .and_then(|f| match &f.value {
            ui::FieldValue::Text(v) | ui::FieldValue::Selection(v) => Some(v.clone()),
            _ => None,
        })
        .unwrap_or_default()
}
fn permission_label(permission: &str) -> &str {
    match permission {
        "projects.read" => "读取项目",
        "packages.manage" => "修改项目包",
        "state.check" => "检查服务状态",
        "operations.read" => "查看任务",
        "operations.cancel" => "取消任务",
        "events.read" => "读取事件",
        "repositories.read" => "读取仓库",
        "repositories.manage" => "添加、刷新和移除仓库",
        "packages.read" => "读取包信息",
        _ => "不支持的权限",
    }
}
fn error_text(code: host::ErrorCode) -> &'static str {
    match code {
        host::ErrorCode::PermissionDenied => "没有执行此操作的权限，请检查客户端和扩展授权。",
        host::ErrorCode::ScopeDenied => "此项目或扩展不在允许的范围内。",
        host::ErrorCode::LeaseExpired | host::ErrorCode::LeaseRevoked => {
            "扩展授权或运行状态已变化，请关闭后重新打开。"
        }
        host::ErrorCode::RevisionConflict => "记录已变化，请刷新后再操作。",
        host::ErrorCode::IdempotencyConflict => "操作记录冲突，请刷新核对当前结果。",
        host::ErrorCode::InvalidArgument => {
            "操作内容无效，请检查名称、权限及范围；重复授权不会再次添加。"
        }
        host::ErrorCode::ResourceLimit => "已达到数量或大小限制，请减少本次内容。",
        host::ErrorCode::Cancelled => "请求已停止。已接受的修改可能仍然有效，请刷新核对。",
        host::ErrorCode::WriteOutcomeUnknown => {
            "结果可能已生效，请使用刷新核对后再操作。不会自动重试。"
        }
        host::ErrorCode::Unavailable => "暂时无法读取服务，请稍后刷新。",
        host::ErrorCode::InternalError => "操作未能完成，请刷新核对；如仍失败请查看诊断记录。",
    }
}
struct Document {
    nodes: Vec<ui::UiNode>,
}
impl Document {
    fn new() -> Self {
        Self {
            nodes: vec![ui::UiNode {
                node_id: "page".into(),
                parent_id: None,
                order: 0,
                payload: ui::NodePayload::Page(ui::PageNode {
                    title: "MCP 客户端".into(),
                }),
            }],
        }
    }
    fn node(&mut self, parent: Option<&str>, payload: ui::NodePayload) -> String {
        let id = format!("n{}", self.nodes.len());
        let parent = parent.unwrap_or("page");
        let order = self
            .nodes
            .iter()
            .filter(|node| node.parent_id.as_deref() == Some(parent))
            .count() as u32;
        self.nodes.push(ui::UiNode {
            node_id: id.clone(),
            parent_id: Some(parent.into()),
            order,
            payload,
        });
        id
    }
    fn status(&mut self, text: &str, tone: ui::Tone) {
        self.node(
            None,
            ui::NodePayload::Status(ui::StatusNode {
                label: text.into(),
                tone,
            }),
        );
    }
    fn key(&mut self, label: &str, value: &str) {
        self.node(
            None,
            ui::NodePayload::KeyValue(ui::KeyValueNode {
                label: label.into(),
                value: value.into(),
            }),
        );
    }
    fn button(&mut self, label: &str, action: &str, disabled: bool) {
        self.node(None, button(label, action, disabled));
    }
    fn finish(self) -> ui::UiDocument {
        ui::UiDocument {
            title: "MCP 客户端".into(),
            nodes: self.nodes,
        }
    }
}
fn button(label: &str, action: &str, disabled: bool) -> ui::NodePayload {
    ui::NodePayload::Button(ui::ButtonNode {
        label: label.into(),
        action_id: action.into(),
        disabled,
    })
}
fn error_document(message: &str) -> ui::UiDocument {
    let mut doc = Document::new();
    doc.status(message, ui::Tone::Danger);
    doc.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_form_includes_repository_management_and_has_no_project_selector() {
        let view = View {
            selected: Some(selected()),
            can_manage: true,
            grants_loaded: true,
            grants: vec![host::Grant {
                permission: "projects.read".into(),
                resource_kind: "global".into(),
                resource_id: "*".into(),
            }],
            ..View::default()
        };
        let doc = view.document();
        let fields = doc
            .nodes
            .iter()
            .filter_map(|node| match &node.payload {
                ui::NodePayload::Switch(field) => Some(ui::SubmittedField {
                    field_id: field.field_id.clone(),
                    value: ui::FieldValue::Boolean(field.initial_value),
                }),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(fields.len(), 9);
        assert!(
            !doc.nodes
                .iter()
                .any(|node| matches!(node.payload, ui::NodePayload::Select(_)))
        );
        let (add, remove) = permission_changes(&view.grants, &fields).expect("unchanged draft");
        assert!(add.is_empty() && remove.is_empty());
        let mut changed = fields;
        changed
            .iter_mut()
            .find(|field| field.field_id == "projects.read")
            .expect("read")
            .value = ui::FieldValue::Boolean(false);
        changed
            .iter_mut()
            .find(|field| field.field_id == "packages.manage")
            .expect("write")
            .value = ui::FieldValue::Boolean(true);
        let repository = changed
            .iter_mut()
            .find(|field| field.field_id == "repositories.manage")
            .expect("repository management");
        assert!(matches!(repository.value, ui::FieldValue::Boolean(false)));
        repository.value = ui::FieldValue::Boolean(true);
        let (add, remove) = permission_changes(&view.grants, &changed).expect("delta");
        assert_eq!(add.len(), 2);
        assert_eq!(remove.len(), 1);
        assert_eq!(add[0].permission, "packages.manage");
        assert_eq!(add[1].permission, "repositories.manage");
        assert_eq!(remove[0].permission, "projects.read");
        assert!(
            add.iter()
                .chain(remove.iter())
                .all(|grant| grant.resource_kind == "global" && grant.resource_id == "*")
        );
        changed.pop();
        assert!(
            permission_changes(&view.grants, &changed).is_none(),
            "missing field never silently revokes a permission"
        );
    }

    #[test]
    fn incomplete_authoritative_grant_read_never_publishes_false_off_switches() {
        let doc = View {
            selected: Some(selected()),
            can_manage: true,
            grants_loaded: false,
            ..View::default()
        }
        .document();
        assert!(!doc.nodes.iter().any(|node| matches!(
            node.payload,
            ui::NodePayload::Form(_) | ui::NodePayload::Switch(_)
        )));
    }
    fn selected() -> host::ClientSummary {
        host::ClientSummary {
            client_id: "00000000-0000-4000-8000-000000000001".into(),
            display_name: "隔离客户端".into(),
            revision: 1,
            created_at_ms: 1,
            revoked_at_ms: None,
            grant_count: 0,
        }
    }
    #[test]
    fn readonly_view_never_publishes_mutating_actions() {
        let view = View {
            clients: vec![selected()],
            ..View::default()
        };
        let doc = view.document();
        assert!(
            !doc.nodes
                .iter()
                .any(|node| matches!(&node.payload, ui::NodePayload::Form(_)))
        );
        assert!(doc.nodes.iter().any(|node| matches!(&node.payload,ui::NodePayload::Button(button) if button.label=="查看权限")));
    }
    #[test]
    fn unknown_write_keeps_risk_visible_and_disables_confirmation() {
        let view = View {
            selected: Some(selected()),
            can_manage: true,
            unknown_write: true,
            confirmation: Some(Confirmation::Revoke),
            message: Some((
                error_text(host::ErrorCode::WriteOutcomeUnknown).into(),
                ui::Tone::Danger,
            )),
            ..View::default()
        };
        let doc = view.document();
        assert!(doc.nodes.iter().any(|node| matches!(&node.payload,ui::NodePayload::Status(status) if status.label.contains("可能已生效"))));
        assert!(doc.nodes.iter().any(|node| matches!(&node.payload,ui::NodePayload::Button(button) if button.action_id=="confirm"&&button.disabled)));
        assert!(
            !doc.nodes
                .iter()
                .any(|node| matches!(&node.payload, ui::NodePayload::Form(_)))
        );
    }
    #[test]
    fn revoked_client_has_no_write_form_or_revoke_action() {
        let mut client = selected();
        client.revoked_at_ms = Some(2);
        let doc = View {
            selected: Some(client),
            can_manage: true,
            ..View::default()
        }
        .document();
        assert!(
            !doc.nodes
                .iter()
                .any(|node| matches!(&node.payload, ui::NodePayload::Form(_)))
        );
        assert!(!doc.nodes.iter().any(|node|matches!(&node.payload,ui::NodePayload::Button(button) if button.action_id=="revoke")));
    }
}

fn runtime_method_label(method: &str) -> &'static str {
    match method {
        "mcp.tools.catalog" => "读取工具目录",
        "extensions.mcpTools.call" => "调用扩展工具",
        "system.status" => "读取 Core 状态",
        "projects.get" => "读取项目",
        "projects.list" => "读取项目列表",
        "state.check" => "检查状态",
        "operations.get" => "读取任务",
        "operations.list" => "读取任务列表",
        "operations.cancel" => "请求取消任务",
        "events.list" => "读取事件",
        "packages.planRemove" => "生成卸载计划",
        "packages.applyPlan" => "应用包变更计划",
        _ => "Core 调用",
    }
}

#[cfg(test)]
mod runtime_tests {
    use super::*;

    #[test]
    fn stream_report_distinguishes_stale_incomplete_and_empty() {
        let mut snapshot = transport::TransportSnapshot {
            instance_id: "core".into(),
            observed_at_ms: 5000,
            status: transport::Availability::Stale,
            can_read_connections: false,
            can_read_requests: false,
            can_read_subscription_streams: true,
            coverage_incomplete: true,
            observers: vec![],
            connections: None,
            requests: None,
            subscription_streams: Some(transport::StreamPage {
                items: vec![],
                total_tracked: 0,
                truncated: false,
            }),
        };
        let document = |snapshot| {
            View {
                activity: true,
                stream_snapshot: Some(snapshot),
                ..Default::default()
            }
            .document()
        };
        let has_status = |doc: &ui::UiDocument, text| {
            doc.nodes.iter().any(|node| {
            matches!(&node.payload, ui::NodePayload::Status(status) if status.label == text)
        })
        };
        let empty = "当前没有已跟踪的 MCP 订阅流。";
        let stale = document(snapshot.clone());
        assert!(has_status(&stale, "MCP 订阅流报告已过期，请刷新。"));
        assert!(!has_status(&stale, empty));
        snapshot.status = transport::Availability::Available;
        let incomplete = document(snapshot.clone());
        assert!(has_status(&incomplete, "部分 MCP 订阅流未被跟踪。"));
        assert!(!has_status(&incomplete, empty));
        snapshot.coverage_incomplete = false;
        assert!(has_status(&document(snapshot), empty));
    }

    #[test]
    fn activity_masks_ungranted_groups_and_uses_core_call_labels() {
        let view = View {
            activity: true,
            runtime: Some(runtime::RuntimeSnapshot {
                instance_id: "core".into(),
                observed_at_ms: 5000,
                can_read_connections: false,
                can_read_requests: true,
                coverage_incomplete: false,
                connections: Some(runtime::ConnectionPage {
                    items: vec![runtime::ConnectionRow {
                        connection_id: "ungranted-connection".into(),
                        client_id: "ungranted-client".into(),
                        connected_at_ms: 1,
                        active_call_count: 0,
                    }],
                    total_tracked: 1,
                    truncated: false,
                }),
                requests: Some(runtime::RequestPage {
                    items: vec![runtime::RequestRow {
                        call_id: "call".into(),
                        connection_id: "connection".into(),
                        client_id: "00000000-0000-4000-8000-000000000001".into(),
                        method: "projects.list".into(),
                        started_at_ms: 1000,
                    }],
                    total_tracked: 1,
                    truncated: false,
                }),
            }),
            ..Default::default()
        };
        let document = view.document();
        let labels: Vec<_> = document
            .nodes
            .iter()
            .filter_map(|node| match &node.payload {
                ui::NodePayload::List(node) => node.label.clone(),
                ui::NodePayload::ListItem(node) => node.label.clone(),
                _ => None,
            })
            .collect();
        assert!(labels.iter().any(|label| label.contains("Core 调用 · 1")));
        assert!(
            labels
                .iter()
                .any(|label| label.contains("读取项目列表") && label.contains("4 秒"))
        );
        assert!(
            !labels
                .iter()
                .any(|label| label.contains("Core 连接") || label.contains("ungranted"))
        );
    }
    #[test]
    fn activity_read_failure_is_not_reported_as_zero_and_keeps_unknown_write_risk() {
        let view = View {
            activity: true,
            runtime_message: Some("活动读取失败".into()),
            unknown_write: true,
            message: Some(("之前写入结果未知".into(), ui::Tone::Warning)),
            ..Default::default()
        };
        let document = view.document();
        let labels: Vec<_> = document
            .nodes
            .iter()
            .filter_map(|node| match &node.payload {
                ui::NodePayload::Status(node) => Some(node.label.as_str()),
                _ => None,
            })
            .collect();
        assert!(labels.contains(&"活动读取失败"));
        assert!(labels.contains(&"之前写入结果未知"));
        assert!(!labels.iter().any(|text| text.contains("当前没有")));
        assert!(
            !document
                .nodes
                .iter()
                .any(|node| matches!(&node.payload, ui::NodePayload::Form(_)))
        );
    }
}
