#![forbid(unsafe_code)]
use crate::bindings::alcomd::extension::types;
use crate::bindings::alcomd::mcp_management::clients as host;
use crate::bindings::exports::alcomd::extension::{guest_lifecycle, guest_ui as ui};
use std::cell::RefCell;
use std::collections::BTreeMap;

pub struct Management;
thread_local! { static SESSIONS: RefCell<BTreeMap<String, View>> = const { RefCell::new(BTreeMap::new()) }; }
#[derive(Default)]
struct View {
    clients: Vec<host::ClientSummary>,
    selected: Option<host::ClientSummary>,
    client_cursor: Option<String>,
    next_clients: Option<String>,
    grant_cursor: Option<String>,
    next_grants: Option<String>,
    grants: Vec<host::Grant>,
    projects: Vec<host::ProjectSummary>,
    project_cursor: Option<String>,
    next_projects: Option<String>,
    confirmation: Option<Confirmation>,
    message: Option<(String, ui::Tone)>,
    unknown_write: bool,
    can_manage: bool,
    counter: u64,
}
enum Confirmation {
    Revoke,
    Remove(host::Grant),
    Add(host::Grant),
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
            let document = view.document();
            sessions.insert(session_id, view);
            document
        })
    }
    fn refresh(session_id: String) -> ui::UiDocument {
        with_view(&session_id, |view| {
            view.confirmation = None;
            view.message = None;
            // Refresh authoritative metadata first, including the selected client's revision.
            view.client_cursor = None;
            if view.load_clients() {
                view.unknown_write = false;
                if view.selected.is_some() {
                    view.grant_cursor = None;
                    view.load_grants();
                }
            }
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
                self.fail(error);
                false
            }
        }
    }
    fn load_grants(&mut self) {
        let Some(selected) = &self.selected else {
            return;
        };
        match host::grants(
            &selected.client_id,
            selected.revision,
            self.grant_cursor.as_deref(),
            Some(64),
        ) {
            Ok(page) => {
                self.grants = page.items;
                self.next_grants = page.next_cursor;
            }
            Err(error) => {
                self.grants.clear();
                self.next_grants = None;
                self.fail(error);
            }
        }
    }
    fn load_projects(&mut self) {
        match host::projects(self.project_cursor.as_deref(), Some(64)) {
            Ok(page) => {
                self.projects = page.items;
                self.next_projects = page.next_cursor;
            }
            Err(error)
                if matches!(
                    error.code,
                    host::ErrorCode::PermissionDenied | host::ErrorCode::ScopeDenied
                ) =>
            {
                self.projects.clear();
                self.next_projects = None;
            }
            Err(error) => self.fail(error),
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
        if self.unknown_write && !matches!(id.as_str(), "back" | "cancel") {
            return; // Only the host's explicit Refresh clears an unknown result.
        }
        if !self.unknown_write {
            self.message = None;
        }
        match id.as_str() {
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
            "grants-next" => {
                self.grant_cursor = self.next_grants.clone();
                self.load_grants();
            }
            "grants-first" => {
                self.grant_cursor = None;
                self.load_grants();
            }
            "projects-next" => {
                self.project_cursor = self.next_projects.clone();
                self.load_projects();
            }
            "projects-first" => {
                self.project_cursor = None;
                self.load_projects();
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
                        self.grant_cursor = None;
                        self.load_grants();
                        self.load_projects();
                    }
                    Err(error) => self.fail(error),
                }
            }
            "add" if self.can_manage => {
                let permission = field(&fields, "permission");
                let selected_scope = field(&fields, "project");
                let resource = selected_scope
                    .strip_prefix("project-")
                    .unwrap_or(&selected_scope)
                    .to_owned();
                let project = matches!(permission.as_str(), "projects.read" | "packages.manage");
                if project && !self.projects.iter().any(|p| p.project_id == resource)
                    || !project && resource != "global"
                {
                    self.message = Some((
                        "请选择与权限对应的项目或全局范围。".into(),
                        ui::Tone::Danger,
                    ));
                    return;
                }
                self.confirmation = Some(Confirmation::Add(host::Grant {
                    permission,
                    resource_kind: if project { "project" } else { "global" }.into(),
                    resource_id: if project { resource } else { "*".into() },
                }));
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
                    Confirmation::Add(grant) => {
                        host::change_grants(&client.client_id, client.revision, &[grant], &[], &key)
                    }
                    Confirmation::Remove(grant) => {
                        host::change_grants(&client.client_id, client.revision, &[], &[grant], &key)
                    }
                };
                match result {
                    Ok(client) => {
                        self.selected = Some(client);
                        self.grant_cursor = None;
                        self.load_grants();
                    }
                    Err(error) => self.fail(error),
                }
            }
            _ if id.starts_with("client-") => {
                if let Some(client) = self.clients.iter().find(|c| c.client_id == id[7..]) {
                    self.selected = Some(client.clone());
                    self.confirmation = None;
                    self.grant_cursor = None;
                    self.load_grants();
                    self.project_cursor = None;
                    self.load_projects();
                }
            }
            _ if id.starts_with("remove-") && self.can_manage => {
                if let Ok(index) = id[7..].parse::<usize>()
                    && let Some(grant) = self.grants.get(index)
                {
                    self.confirmation = Some(Confirmation::Remove(grant.clone()));
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
                    Confirmation::Add(grant) => doc.key("允许", &self.grant_label(grant)),
                    Confirmation::Remove(grant) => doc.key("撤销授权", &self.grant_label(grant)),
                }
                doc.button("确认", "confirm", self.unknown_write);
                doc.button("取消", "cancel", false);
            } else {
                let list = doc.node(
                    None,
                    ui::NodePayload::List(ui::ListNode {
                        label: Some("已允许的权限".into()),
                    }),
                );
                for (index, grant) in self.grants.iter().enumerate() {
                    let row = doc.node(
                        Some(&list),
                        ui::NodePayload::ListItem(ui::ListItemNode {
                            label: Some(self.grant_label(grant)),
                        }),
                    );
                    if self.can_manage && client.revoked_at_ms.is_none() {
                        doc.node(
                            Some(&row),
                            button("撤销此授权", &format!("remove-{index}"), self.unknown_write),
                        );
                    }
                }
                if self.grant_cursor.is_some() {
                    doc.button("授权第一页", "grants-first", false);
                }
                if self.next_grants.is_some() {
                    doc.button("下一页授权", "grants-next", false);
                }
                if self.can_manage && client.revoked_at_ms.is_none() && !self.unknown_write {
                    self.add_form(&mut doc);
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
        let scope = if grant.resource_kind == "global" {
            "全局".into()
        } else {
            self.projects
                .iter()
                .find(|p| p.project_id == grant.resource_id)
                .map_or_else(
                    || format!("项目 {}", grant.resource_id),
                    |p| format!("{} ({})", p.name, p.project_id),
                )
        };
        format!("{} · {scope}", permission_label(&grant.permission))
    }
    fn add_form(&self, doc: &mut Document) {
        let form = doc.node(
            None,
            ui::NodePayload::Form(ui::FormNode {
                submit_action_id: "add".into(),
                submit_label: "审阅新增授权".into(),
                disabled: false,
            }),
        );
        let permissions = [
            "projects.read",
            "packages.manage",
            "state.check",
            "operations.read",
            "operations.cancel",
            "events.read",
            "repositories.read",
            "packages.read",
        ];
        doc.node(
            Some(&form),
            ui::NodePayload::Select(ui::SelectNode {
                field_id: "permission".into(),
                label: "权限".into(),
                initial_option_id: Some("state.check".into()),
                required: true,
                options: permissions
                    .iter()
                    .map(|p| ui::SelectOption {
                        option_id: (*p).into(),
                        label: permission_label(p).into(),
                    })
                    .collect(),
                disabled: false,
                read_only: false,
                validation: None,
            }),
        );
        let mut options = vec![ui::SelectOption {
            option_id: "global".into(),
            label: "全局（非项目权限）".into(),
        }];
        options.extend(self.projects.iter().map(|p| ui::SelectOption {
            option_id: format!("project-{}", p.project_id),
            label: format!("{} ({})", p.name, p.project_id),
        }));
        doc.node(
            Some(&form),
            ui::NodePayload::Select(ui::SelectNode {
                field_id: "project".into(),
                label: "范围".into(),
                initial_option_id: Some("global".into()),
                required: true,
                options,
                disabled: false,
                read_only: false,
                validation: None,
            }),
        );
        if self.project_cursor.is_some() {
            doc.button("项目第一页", "projects-first", false);
        }
        if self.next_projects.is_some() {
            doc.button("下一页项目", "projects-next", false);
        }
    }
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
