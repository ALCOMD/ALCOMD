#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct McpToolCapability {
    pub(crate) tool_name: &'static str,
    pub(crate) ipc_method: &'static str,
    pub(crate) gui_capability: &'static str,
    pub(crate) read_only: bool,
    pub(crate) destructive: bool,
}

pub(crate) const MCP_TOOL_CAPABILITIES: &[McpToolCapability] = &[
    McpToolCapability {
        tool_name: "alcomd3_list_projects",
        ipc_method: "list_projects",
        gui_capability: "environment.projects.list",
        read_only: true,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_list_templates",
        ipc_method: "list_templates",
        gui_capability: "environment.templates.list",
        read_only: true,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_get_template",
        ipc_method: "get_template",
        gui_capability: "environment.templates.details.read",
        read_only: true,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_create_template",
        ipc_method: "create_template",
        gui_capability: "environment.templates.create",
        read_only: false,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_edit_template",
        ipc_method: "edit_template",
        gui_capability: "environment.templates.edit",
        read_only: false,
        destructive: true,
    },
    McpToolCapability {
        tool_name: "alcomd3_set_template_package",
        ipc_method: "set_template_package",
        gui_capability: "environment.templates.packages.set",
        read_only: false,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_remove_template_package",
        ipc_method: "remove_template_package",
        gui_capability: "environment.templates.packages.remove",
        read_only: false,
        destructive: true,
    },
    McpToolCapability {
        tool_name: "alcomd3_set_template_unitypackage",
        ipc_method: "set_template_unitypackage",
        gui_capability: "environment.templates.unitypackages.set",
        read_only: false,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_remove_template_unitypackage",
        ipc_method: "remove_template_unitypackage",
        gui_capability: "environment.templates.unitypackages.remove",
        read_only: false,
        destructive: true,
    },
    McpToolCapability {
        tool_name: "alcomd3_remove_template",
        ipc_method: "remove_template",
        gui_capability: "environment.templates.remove",
        read_only: false,
        destructive: true,
    },
    McpToolCapability {
        tool_name: "alcomd3_get_project_details",
        ipc_method: "get_project_details",
        gui_capability: "project.details.read",
        read_only: true,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_list_repositories",
        ipc_method: "list_repositories",
        gui_capability: "environment.repositories.list",
        read_only: true,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_add_repository",
        ipc_method: "add_repository",
        gui_capability: "environment.repositories.add",
        read_only: false,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_remove_repository",
        ipc_method: "remove_repository",
        gui_capability: "environment.repositories.remove",
        read_only: false,
        destructive: true,
    },
    McpToolCapability {
        tool_name: "alcomd3_get_package_details",
        ipc_method: "get_package_details",
        gui_capability: "packages.details.read",
        read_only: true,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_list_packages",
        ipc_method: "list_packages",
        gui_capability: "packages.visible.list",
        read_only: true,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_list_repository_packages",
        ipc_method: "list_repository_packages",
        gui_capability: "packages.repository.list",
        read_only: true,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_get_environment_settings",
        ipc_method: "get_environment_settings",
        gui_capability: "environment.settings.read",
        read_only: true,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_search_activity_logs",
        ipc_method: "search_activity_logs",
        gui_capability: "logs.activity.search",
        read_only: true,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_get_activity_log_entry",
        ipc_method: "get_activity_log_entry",
        gui_capability: "logs.activity.entry.read",
        read_only: true,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_summarize_activity_logs",
        ipc_method: "summarize_activity_logs",
        gui_capability: "logs.activity.summarize",
        read_only: true,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_get_activity_log_context",
        ipc_method: "get_activity_log_context",
        gui_capability: "logs.activity.context.read",
        read_only: true,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_search_technical_logs",
        ipc_method: "search_technical_logs",
        gui_capability: "logs.technical.search",
        read_only: true,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_get_technical_log_entry",
        ipc_method: "get_technical_log_entry",
        gui_capability: "logs.technical.entry.read",
        read_only: true,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_summarize_technical_logs",
        ipc_method: "summarize_technical_logs",
        gui_capability: "logs.technical.summarize",
        read_only: true,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_create_project",
        ipc_method: "create_project",
        gui_capability: "project.create",
        read_only: false,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_add_existing_project",
        ipc_method: "add_existing_project",
        gui_capability: "project.add",
        read_only: false,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_backup_project",
        ipc_method: "backup_project",
        gui_capability: "project.backup.create",
        read_only: false,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_copy_project",
        ipc_method: "copy_project",
        gui_capability: "project.copy.create",
        read_only: false,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_restore_project_from_backup",
        ipc_method: "restore_project_from_backup",
        gui_capability: "project.backup.restore",
        read_only: false,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_install_project_package",
        ipc_method: "install_project_package",
        gui_capability: "project.packages.install",
        read_only: false,
        destructive: false,
    },
    McpToolCapability {
        tool_name: "alcomd3_uninstall_project_package",
        ipc_method: "uninstall_project_package",
        gui_capability: "project.packages.uninstall",
        read_only: false,
        destructive: true,
    },
    McpToolCapability {
        tool_name: "alcomd3_reinstall_project_package",
        ipc_method: "reinstall_project_package",
        gui_capability: "project.packages.reinstall",
        read_only: false,
        destructive: false,
    },
];

pub(crate) fn mcp_tool_capability_for_method(method: &str) -> Option<&'static McpToolCapability> {
    MCP_TOOL_CAPABILITIES
        .iter()
        .find(|capability| capability.ipc_method == method)
}

pub(crate) fn mcp_tool_capability_for_tool_name(
    tool_name: &str,
) -> Option<&'static McpToolCapability> {
    MCP_TOOL_CAPABILITIES
        .iter()
        .find(|capability| capability.tool_name == tool_name)
}
