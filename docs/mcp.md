# ALCOMD3 MCP Guide

Languages: English | [日本語](mcp/mcp.ja.md) | [简体中文](mcp/mcp.zh-CN.md) |
[繁體中文](mcp/mcp.zh-TW.md)

This document describes ALCOMD3 MCP setup, available tools, lifecycle behavior,
and troubleshooting.

ALCOMD3 implements the Streamable HTTP transport from MCP specification
`2025-11-25`. The GUI starts one local `alcomd3-mcp` server and exposes its MCP
endpoint only on `127.0.0.1`. `alcomd3-mcp` requests application data through
the separate private IPC endpoint exposed by the GUI.

## Quick Start

1. Start ALCOMD3 and open the MCP page from the sidebar.
2. Enable MCP.
3. Configure the client manually with the MCP Endpoint and Authorization Token
   shown on the page. On Windows, Codex, Claude Code, and Cursor users may
   instead choose the corresponding optional quick setup button.
4. For manual setup, add the URL as a Streamable HTTP server and send the token
   as `Authorization: Bearer <token>`.
5. Run a tool call while MCP remains enabled in ALCOMD3.

Use the endpoint shown by the GUI instead of guessing its port. See
[Enabling MCP and Client Configuration](#enabling-mcp-and-client-configuration)
for a configuration example and lifecycle details.

## Current Boundaries

- MCP is disabled by default. Users must enable it in the GUI before new tool
  calls may read or write ALCOMD3 data.
- When the GUI is running normally, it starts the local IPC endpoint. Enabling
  or disabling MCP gates tool data access; it does not stop the endpoint.
- Current tools include read-only project, repository, package, environment,
  activity log, and technical log tools, plus limited write tools: create a
  project, add an existing project, add a VPM repository, back up a registered
  project, copy a registered project, restore a project from a zip backup, and
  install/uninstall/reinstall one package in a registered project. Other write
  operations such as repository deletion, repository reorder, and project
  deletion are not exposed.
- The GUI starts and owns the local Streamable HTTP server. Closing the GUI also
  stops that server.
- If GUI startup fails or the endpoint remains unavailable, the tool call
  returns structured `alcomd3_unavailable` and marks the MCP tool result with
  `isError: true`.
- When MCP is disabled, new tool calls return structured `mcp_disabled`, do not
  stop the endpoint, do not panic, and mark the MCP tool result with
  `isError: true`. Existing project long tasks are cleanup exceptions:
  `tasks/get`, `tasks/result`, and `tasks/cancel` may still query or cancel the
  task.
- The bridge applies loose local rate limiting and concurrency protection to
  tool calls. When the limits are exceeded, it returns structured
  `rate_limited` and marks the MCP tool result with `isError: true`.
- The GUI MCP page highlights known tool calls while they run, and briefly keeps
  the highlight after completion or failure so fast calls remain visible.
- The GUI MCP page groups tools by read-only, write, and log usage, and keeps
  the exact MCP names. Hovering over a tool name shows the localized readable
  name.
- The GUI MCP page shows recently active clients, not a live connection list.
  Records with no recent activity are hidden automatically.
- While the Logs extension is enabled, MCP tool calls are written to the GUI's
  local activity log. Records include source, tool name, request id, client
  summary, started/completed/failed/cancelled state, and safely processed
  target/details so users can review what an Agent did from the GUI Activity
  page.
- The GUI project management page and MCP package tools share the backend
  GUI-visible package catalog. Pre-release filtering, yanked packages, hidden
  repositories, hidden local user packages, same-name package merge across
  sources, default/user repository priority, and Unity compatibility are all
  handled by the shared backend.
- Every public MCP tool must map to an existing GUI capability and enter
  business logic through shared backend services under `vrc-get-gui/src/backend/`.
  MCP dispatch is responsible only for enabled-state gating, argument parsing,
  task wrapping, error mapping, and activity logging; it should not add business
  capabilities the GUI does not have.
- Streamable HTTP requests require the bearer token generated for the local
  ALCOMD3 installation.
- The HTTP server validates `Host` and `Origin`, binds exactly to `127.0.0.1`,
  and never listens on a LAN or public address.
- The GUI internal IPC listens only on `127.0.0.1`; it never listens on a public
  network address.

Activity records do not save raw MCP params, token-like fields, HTTP header
values, URLs with query strings, or URL userinfo credentials. Local filesystem
paths keep their full value for diagnosing Unity, VPM, and non-ASCII path
issues; MCP access still requires enabling MCP in the GUI first.

## Architecture

```text
MCP Host / Client
        |
        | Streamable HTTP + bearer token
        v
alcomd3-mcp
        |
        | reads endpoint metadata
        v
ALCOMD3 data dir / mcp / endpoint.json
        |
        | localhost TCP, newline-delimited JSON
        v
ALCOMD3 GUI IPC server
```

The external MCP transport is Streamable HTTP. The GUI internal TCP IPC remains
a separate private channel between the server process and the desktop app; it
is not exposed directly to MCP clients.

## Enabling MCP and Client Configuration

1. Start ALCOMD3.
2. Open the MCP page from the sidebar.
3. Enable MCP to allow tools to read ALCOMD3 data.
4. Copy the MCP Endpoint and Authorization Token from the page.
5. In a client that supports Streamable HTTP MCP servers, add the endpoint URL
   and configure the token as a bearer `Authorization` header.

A generic configuration shape is shown below. Exact field names depend on the
MCP client:

```json
{
    "mcpServers": {
        "alcomd3": {
            "url": "http://127.0.0.1:51739/mcp",
            "headers": {
                "Authorization": "Bearer <token shown by ALCOMD3>"
            }
        }
    }
}
```

Manual configuration remains the default and does not modify the operating
system or an AI client configuration.

### Optional AI client quick setup on Windows

The MCP page includes separate quick setup buttons for Codex, Claude Code, and
Cursor on Windows. A client is only changed after its button is explicitly
clicked. Every button writes the current token to the current user's
`ALCOMD3_MCP_BEARER_TOKEN` environment variable, then adds or updates only the
selected client's ALCOMD3 MCP entry:

- Codex: `$CODEX_HOME/config.toml`, or `~/.codex/config.toml` when
  `CODEX_HOME` is unset:

```toml
[mcp_servers.alcomd3]
url = "http://127.0.0.1:51739/mcp"
bearer_token_env_var = "ALCOMD3_MCP_BEARER_TOKEN"
```

- Claude Code: `$CLAUDE_CONFIG_DIR/.claude.json`, or `~/.claude.json` when
  `CLAUDE_CONFIG_DIR` is unset:

```json
{
    "mcpServers": {
        "alcomd3": {
            "type": "http",
            "url": "http://127.0.0.1:51739/mcp",
            "headers": {
                "Authorization": "Bearer ${ALCOMD3_MCP_BEARER_TOKEN}"
            }
        }
    }
}
```

- Cursor: `~/.cursor/mcp.json`:

```json
{
    "mcpServers": {
        "alcomd3": {
            "type": "http",
            "url": "http://127.0.0.1:51739/mcp",
            "headers": {
                "Authorization": "Bearer ${env:ALCOMD3_MCP_BEARER_TOKEN}"
            }
        }
    }
}
```

Other settings and MCP servers in the selected client are preserved. If either
the environment variable or the selected client's `alcomd3` entry already has
a different value, ALCOMD3 asks for confirmation before replacing it. Fully
exit and restart the selected client after quick setup so it inherits the user
environment and reloads its MCP configuration.

Exact client fields vary. Always copy the current URL and token from the GUI.
The default port is `51739`; advanced users may change `mcpHttpPort` in
`gui-config.json` before starting ALCOMD3.

The endpoint is available only while ALCOMD3 is running. If MCP is disabled,
new tool calls return `mcp_disabled`; enable MCP in the GUI and retry. Already
started project long tasks can still be queried or cancelled through task
follow-up methods while the server remains running.

The external HTTP port and bearer token are stored as `mcpHttpPort` and
`mcpHttpToken` in `gui-config.json`. Treat the token as a local secret and do
not include it in logs, screenshots, or shared configuration. Replacing the
token invalidates existing client configuration.

## Package Locations

- Windows: `alcomd3-mcp.exe` is installed next to the main GUI executable.
- macOS: `alcomd3-mcp` is under `.app/Contents/MacOS/`.
- Linux: `alcomd3-mcp` is installed to `/usr/bin/alcomd3-mcp`.
- AppImage: `alcomd3-mcp` is inside AppDir `usr/bin/`.

`cargo xtask build-alcom` builds both the GUI main program and `alcomd3-mcp`.

## Endpoint Metadata

When the GUI is running normally, it writes endpoint metadata. The default path
is under the ALCOMD3 local data directory:

```text
ALCOMD3/mcp/endpoint.json
```

Tests and development can override the path with:

```text
ALCOMD3_MCP_ENDPOINT_FILE
```

Tests and development can also override the GUI executable the bridge starts:

```text
ALCOMD3_GUI_EXECUTABLE
```

Metadata format:

```json
{
    "protocolVersion": 2,
    "transport": "tcp",
    "host": "127.0.0.1",
    "port": 49152,
    "token": "opaque-random-token",
    "pid": 12345
}
```

This endpoint metadata `token` is used only for private IPC authentication
between the HTTP server process and GUI. It is different from the external HTTP
bearer token. Do not expose either value to remote systems.

## Internal IPC

Internal IPC uses newline-delimited JSON. Request and response shapes:

```text
IpcRequest {
    protocolVersion,
    token,
    requestId,
    client,
    method,
    params
}

IpcResponse {
    requestId,
    ok,
    result?,
    error?
}
```

The GUI validates `protocolVersion` and `token`. Validation failures return a
business error and do not run tool logic. After validation, if MCP is disabled
in the GUI, the GUI returns `mcp_disabled` for new tool data access and task
startup, and does not read or return project, repository, package, or similar
data. Existing project long-task methods `project_task_get`,
`project_task_list`, and `project_task_cancel` are exceptions so clients can
finish querying or cancelling already running tasks after MCP is disabled.

## Available Tools

All tools return JSON. Success responses include `ok: true`; business failures
include `ok: false` and `error: { code, message, data? }`, and the outer MCP
tool result includes `isError: true`.

| Tool | Arguments | Description |
| --- | --- | --- |
| `alcomd3_list_projects` | `{}` | Lists projects registered in ALCOMD3. |
| `alcomd3_get_project_details` | `{ "project_path": string }` | Gets details and installed package summary for a registered project. `project_path` must match a project registered in ALCOMD3. |
| `alcomd3_list_repositories` | `{}` | Lists current remote repositories and display settings. `repositories` includes official defaults, Curated defaults, and user repositories; `userRepositories` remains as a user-repository-only compatibility field. |
| `alcomd3_add_repository` | `{ "repository_url": string, "headers"?: object }` | Downloads and validates a VPM repository URL, adds it as a user repository, and clears package cache so later loads refresh. Success returns the added `repository` summary; activity logs store only a redacted URL and header count, not header values. |
| `alcomd3_get_package_details` | `{ "package_name": string, "version"?: string, "repository_id"?: string, "repository_url"?: string }` | Gets detailed metadata for a GUI-visible package. `package_name` is required; `version` and repository selection fields can narrow the result to one version or source. |
| `alcomd3_list_packages` | `{ "offset"?: number, "limit"?: number }` | Paginates lightweight summaries of GUI-default-visible packages. Within one source, each package name returns only the latest visible version, with `source.kind` set to `officialDefault`, `curatedDefault`, `userRepository`, or `localUser`. MCP does not server-side search; clients or Agents should filter returned rows. |
| `alcomd3_list_repository_packages` | `{ "repository_id"?: string, "repository_url"?: string, "offset"?: number, "limit"?: number }` | Paginates lightweight GUI-visible package summaries for one repository. Within that repository, each package name returns only the latest visible version. Call `alcomd3_list_repositories` first to get `id` or `url`, then pass one of those fields. |
| `alcomd3_get_environment_settings` | `{}` | Reads an environment settings summary, including added Unity installs, default Unity launch arguments, default project path, and backup path. |
| `alcomd3_search_activity_logs` | `{ "search"?: string, "sources"?: string[], "kinds"?: string[], "statuses"?: string[], "visibility"?: "important" \| "primary" \| "secondary" \| "technical" \| "all", "operations"?: string[], "tool_names"?: string[], "request_id"?: string, "target"?: string, "since"?: string, "until"?: string, "offset"?: number, "limit"?: number, "order"?: "newest" \| "oldest" }` | Paginates user-readable activity log summaries. By default it returns only important activity; `limit` defaults to 50 and maxes at 200. |
| `alcomd3_get_activity_log_entry` | `{ "id": string, "include_details"?: boolean }` | Reads one full activity log entry by id. Details do not include raw MCP params, URL query, or URL userinfo; local paths remain complete for diagnostics. |
| `alcomd3_summarize_activity_logs` | `alcomd3_search_activity_logs` arguments plus `{ "group_by"?: "source" \| "kind" \| "status" \| "operation" \| "tool_name" \| "client_name" \| "day" \| "hour" }` | Aggregates activity records by source, kind, status, operation, tool, client, or time to locate records before reading them. |
| `alcomd3_get_activity_log_context` | `{ "id": string, "before"?: number, "after"?: number, "include_details"?: boolean }` | Reads neighboring activity around an entry for operation-chain review; `before`/`after` max at 50. |
| `alcomd3_search_technical_logs` | `{ "search"?: string, "levels"?: string[], "targets"?: string[], "scope"?: "memory" \| "recent_files", "since"?: string, "until"?: string, "offset"?: number, "limit"?: number, "max_message_chars"?: number }` | Paginates technical log previews. Defaults to current process memory `error`/`warn`, `limit` defaults to 50 and maxes at 100, and message previews default to 300 characters. |
| `alcomd3_get_technical_log_entry` | `{ "id": string, "max_message_chars"?: number }` | Reads one technical log message by id; the message is redacted and capped at 4000 characters. |
| `alcomd3_summarize_technical_logs` | `alcomd3_search_technical_logs` arguments plus `{ "group_by"?: "level" \| "target" \| "file" \| "hour" }` | Aggregates technical logs by level, target, file, or hour to locate error hotspots. |
| `alcomd3_create_project` | `{ "project_name": string, "base_path"?: string, "template_id"?: string, "unity_version"?: string }` | Creates a Unity project, resolves project packages, and registers it in ALCOMD3. `project_name` is required; omitted `base_path` uses the GUI default project path; omitted `template_id` and `unity_version` use the current GUI template selection rules. Success returns `projectPath`, `templateId`, and `unityVersion`. |
| `alcomd3_add_existing_project` | `{ "project_path": string }` | Registers an existing Unity project directory in ALCOMD3. `project_path` must be an absolute path and load as a valid Unity project. Success returns `projectPath`. |
| `alcomd3_backup_project` | `{ "project_path": string, "backup_name"?: string, "exclude_vpm_packages"?: boolean }` | Creates a zip backup for a registered project using the current GUI backup directory and backup format. `exclude_vpm_packages` omits installed VPM package contents when `true` and defaults to `false`. Omitted `backup_name` generates the project-name-plus-timestamp default; a provided name overrides the archive file name without `.zip`. Success returns `backupPath`. |
| `alcomd3_copy_project` | `{ "source_project_path": string, "new_project_path": string }` | Copies a registered project to a new non-existing directory and registers the copied project. Success returns `projectPath`. |
| `alcomd3_restore_project_from_backup` | `{ "backup_path": string, "project_name"?: string }` | Restores a project from a zip backup into the GUI-configured default project directory and registers it. If `project_name` is omitted, the backup file name is used. Success returns `projectPath`. |
| `alcomd3_install_project_package` | `{ "project_path": string, "package_name": string, "version_selector": { "type": "latest_gui_visible" } \| { "type": "exact", "version": string }, "source"?: { "repository_id"?: string, "repository_url"?: string }, "allow_conflicts"?: boolean }` | Installs one GUI-visible package compatible with the project's Unity version into a registered project. `latest_gui_visible` uses the same visible package, source priority, and pre-release settings as the GUI backend; `exact` must still match a GUI-visible version. Conflicts or legacy file/folder deletion block by default. |
| `alcomd3_uninstall_project_package` | `{ "project_path": string, "package_name": string, "allow_conflicts"?: boolean }` | Uninstalls one installed package from a registered project. Conflicts or legacy file/folder deletion block by default. |
| `alcomd3_reinstall_project_package` | `{ "project_path": string, "package_name": string, "allow_conflicts"?: boolean }` | Reinstalls one installed package in a registered project. Conflicts or legacy file/folder deletion block by default. |

### Log Query Tools

Log tools are split into activity records and technical logs so an Agent does
not need to pull all logs into context to diagnose one issue.

- Activity records are user-readable, structured, and redacted operation
  history. `alcomd3_search_activity_logs` defaults `visibility` to `important`,
  returning writes, failures, cancellations, and important MCP/System behavior.
  Pass `secondary`, `technical`, or `all` explicitly when needed.
- Activity search results return summary fields only, including id, time,
  source, kind, status, operation, target, duration, and error summary. Call
  `alcomd3_get_activity_log_entry` for details or
  `alcomd3_get_activity_log_context` for surrounding activity.
- Technical logs are for diagnostics. By default they search current process
  memory for `error` and `warn`. Pass `"scope": "recent_files"` for recent
  files, or explicit `levels` for Info/Debug/Trace.
- Technical log tools do not return unlimited raw text. Search returns
  `messagePreview`; details are truncated by `max_message_chars` and redact
  token, secret, authorization, API key, `sk-` values, URL userinfo, query, and
  fragment.
- Log tools are themselves recorded as MCP read activity. Successful log reads
  are Secondary; failures remain visible as failed activity.

### Project Long Tasks

Tasks were introduced in MCP `2025-11-25` and are currently experimental.
Client support varies, and their protocol behavior may evolve in future MCP
versions.

`alcomd3_create_project`, `alcomd3_backup_project`, `alcomd3_copy_project`,
`alcomd3_restore_project_from_backup`, `alcomd3_install_project_package`,
`alcomd3_uninstall_project_package`, and `alcomd3_reinstall_project_package`
support MCP task-aware calls and declare `execution.taskSupport: "optional"` in
`tools/list`.

Clients that support Tasks can include `task: {}` in `tools/call` params:

- `tools/call` immediately returns a `CreateTaskResult` containing
  `task.taskId`.
- `tasks/get` queries `working`, `completed`, `failed`, `cancelled`, and similar
  states.
- `tasks/result` returns the original tool result shape after completion, such
  as `backupPath`, `projectPath`, or package-change summary.
- The tool result returned by `tasks/result` includes
  `_meta.io.modelcontextprotocol/related-task` with the matching `taskId`.
- `tasks/cancel` cancels the underlying GUI backend task and releases the
  project task lock of that type.
- If `alcomd3_create_project` is cancelled before formal registration, or if
  package resolve/apply fails, it cleans up the unregistered project directory
  created by MCP.
- If the user disables MCP while a task is running, new tool calls and new
  project task starts still return `mcp_disabled`; long tasks that already have
  a `taskId` can still be finished with `tasks/get`, `tasks/result`, and
  `tasks/cancel`.

If `_meta.progressToken` exists on `tools/call`, the bridge sends standard
`notifications/progress`. `tasks/get` `_meta` also includes
`alcomd3/projectProgress` for polling the latest progress snapshot:

```json
{
  "_meta": {
    "alcomd3/projectProgress": {
      "total": 120,
      "proceed": 42,
      "lastProceed": "Assets/example.prefab"
    }
  }
}
```

Without task-aware calls, these tools still run as normal synchronous
`tools/call` calls until they complete.

### Path Restrictions

`alcomd3_get_project_details`, `alcomd3_backup_project`,
`alcomd3_copy_project`, and project package install/uninstall/reinstall tools
only accept source project paths that are registered in the ALCOMD3 database.
MCP clients cannot use these tools to read or copy arbitrary local paths.

`alcomd3_get_environment_settings` returns ALCOMD3-saved local paths, such as
Unity executables, default project directory, and backup directory. It does not
start Unity, ask Unity Hub to refresh, or scan additional disks.

`alcomd3_backup_project` `backup_name` must be one legal file name, not a path,
and must omit the `.zip` extension, which is appended automatically. The archive
is always written to the GUI-configured backup directory, and an existing
archive is never overwritten.

`alcomd3_copy_project` `new_project_path` must be an absolute, non-existing
directory path and must not be inside the source project. The tool creates the
directory, copies project files, registers the new project, and cleans up the
new directory on failure. `alcomd3_restore_project_from_backup` `backup_path`
must be absolute and restores only into the GUI-configured default project
directory. `project_name` must be one legal folder name, not a path separator,
root path, or `..`. `alcomd3_create_project` applies the same single-folder-name
restriction to `project_name`; explicit `base_path` must be absolute. Omitted
`base_path` uses the GUI default project path. `alcomd3_add_existing_project`
`project_path` must be absolute and load as a Unity project.

### Package Visibility and Write Limits

`alcomd3_list_packages` and `alcomd3_list_repository_packages` use the same
package-state load path as the GUI package page, not the force-refresh path.
Results follow GUI pre-release, hidden repository, hidden local user package,
and yanked filters. MCP tool calls do not server-side search. Adding a
repository requires an explicit `alcomd3_add_repository` call; list tools never
implicitly add repositories or redesign repository refresh behavior.

The GUI project-management package table is generated by the backend from
same-name package merge logic. MCP package lists, package details, and project
package install selection use the same backend rules:

- When "Show pre-release packages" is off, GUI and MCP GUI-visible results do
  not include pre-release versions; MCP `latest_gui_visible` cannot select
  pre-release versions either. The underlying cache may still store
  pre-release data; it becomes visible only after the setting is enabled again.
- Yanked packages do not enter visible candidates. If the installed package
  version is currently yanked, the project package row keeps the yanked marker.
- Hidden repositories and hidden local user packages affect only visible
  candidates. Hidden sources may still appear as "existing source" information,
  but do not participate in latest-version selection.
- Same-name packages across sources are merged into one row on the project
  management page. Default repositories, local user packages, user
  repositories, and unregistered repositories are merged in backend order.
- Project package installation selects only GUI-visible candidates compatible
  with the project's Unity version.

`alcomd3_install_project_package`, `alcomd3_uninstall_project_package`, and
`alcomd3_reinstall_project_package` first generate pending project changes. If
the result contains dependency conflicts or legacy file/folder deletion and
`"allow_conflicts": true` was not passed, the tool returns
`project_package_conflicts` with change summary in `error.data.changes`; nothing
is applied to the project. Confirm and retry with `"allow_conflicts": true` to
continue apply.

Package list tools return only discovery-friendly summary fields: `name`,
`displayName`, `version`, and `source`. `totalCount` and paging fields are
computed from aggregated summary rows, not raw repository version lists. To
read description, keywords, dependencies, legacy packages, documentation URL,
changelog URL, or Unity version requirements, choose a candidate from the list
and call `alcomd3_get_package_details`.

Package list tools default `offset` to `0` and `limit` to `200`; `limit` maxes
at `1000`. Paging responses include `totalCount`, `offset`, `limit`,
`returnedCount`, `hasMore`, and `nextOffset`. To read a complete list, keep
requesting `nextOffset` while `hasMore` is `true`. Package tools no longer
return a `count` field.

## Lifecycle and Multi-Process Behavior

The GUI starts one `alcomd3-mcp` Streamable HTTP server after loading its local
configuration. All MCP clients connect to that shared local endpoint and create
independent MCP sessions.

ALCOMD3 lifecycle boundaries:

- When the GUI exits, it stops the HTTP server and private IPC listener, then
  deletes the private endpoint file.
- The endpoint URL and bearer token are stable for the local installation. A
  client can reconnect after the GUI restarts without changing configuration.
- The GUI client area shows recent MCP session activity; tool highlight
  indicates a currently handled call.
- When the GUI is unavailable, tool calls return structured
  `alcomd3_unavailable`.
- When the GUI is available but MCP is disabled, new tool calls return
  structured `mcp_disabled`; task follow-up methods for already started project
  long tasks can still query results or cancel tasks.
- After the GUI restarts, it binds the configured loopback port again and later
  client requests can reconnect. Whether tools return data still depends on the
  MCP enable switch.
- If the configured port is already in use, the MCP page shows the server as
  not running and the technical log records the startup error.

## Errors and Troubleshooting

### `mcp_disabled`

The MCP page is disabled. The endpoint may still show as running; this is
normal. Enable MCP and retry the tool. Already started project long tasks are
exceptions: clients may still use `tasks/get`, `tasks/result`, and
`tasks/cancel` to query or cancel them.

### `rate_limited`

The bridge received too many tool calls in a short period, or too many tool
calls are already running. Retry later.

### `ALCOMD3 is not running or the MCP IPC endpoint is unavailable`

Common causes:

- ALCOMD3 GUI is not running.
- The client URL does not match the MCP Endpoint currently shown by the GUI.
- The configured local port is already in use.

Steps:

1. Start ALCOMD3.
2. Confirm endpoint running on the MCP page.
3. Copy the MCP Endpoint and Authorization Token again and update the client.
4. Restart the MCP client.

For a supported client on Windows, choose its quick setup button again, confirm
replacement when requested, then fully exit and restart that client.

### HTTP `401 Unauthorized`

The bearer token is missing or does not match the token shown by ALCOMD3.
Update the client's `Authorization` header.

### HTTP `403 Forbidden`

The request carried a disallowed browser `Origin`. ALCOMD3 accepts native MCP
clients and same-loopback origins only, preventing DNS rebinding and cross-site
requests to the local server.

### `protocol mismatch`

The HTTP server and GUI internal IPC versions do not match. Restart ALCOMD3 and
confirm that only the current installation is running.

## Development Smoke Test

Build the bridge from the repository root:

```powershell
cargo build -p alcomd3-mcp
```

Run the focused HTTP lifecycle and security smoke tests:

```powershell
cargo test -p alcomd3-mcp
```

Expected result:

- `initialize` succeeds.
- `tools/list` returns the current MCP tools.
- `tools/call` returns a readable `ok: false` error and marks the MCP tool
  result with `isError: true`.
- Missing/incorrect bearer tokens return HTTP `401`.
- Disallowed origins return HTTP `403`.

## Related Source

- Bridge: `alcomd3-mcp/src/main.rs`
- Shared IPC protocol: `alcomd3-mcp-protocol/src/lib.rs`
- GUI shared backend services and MCP capability matrix: `vrc-get-gui/src/backend/`
- GUI IPC server and tool dispatch: `vrc-get-gui/src/mcp.rs`
- GUI Tauri commands: `vrc-get-gui/src/commands/mcp.rs`
- GUI MCP page: `vrc-get-gui/app/_main/mcp/index.tsx`
- Packaging logic: `xtask/src/build_alcom.rs`, `xtask/src/bundle_alcom*`

## References

- MCP Specification `2025-11-25`: <https://modelcontextprotocol.io/specification/2025-11-25>
- MCP Streamable HTTP transport: <https://modelcontextprotocol.io/specification/2025-11-25/basic/transports>
- MCP lifecycle: <https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle>
- MCP tools: <https://modelcontextprotocol.io/specification/2025-11-25/server/tools>
- MCP tasks: <https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/tasks>
- MCP progress: <https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/progress>
- MCP cancellation: <https://modelcontextprotocol.io/specification/2025-11-25/basic/utilities/cancellation>
