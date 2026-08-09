# ALCOMD3 MCP tool reference

[简体中文](tools.zh-CN.md) | [繁體中文](tools.zh-TW.md) | [日本語](tools.ja.md)

This reference documents the 33 MCP tools currently exposed by ALCOMD3. Use it when
building an Agent, reviewing a call, or troubleshooting a result. For connection,
authentication, lifecycle, and client setup, see the [MCP guide](../mcp.md).

## How to read this reference

- Input names use `snake_case`; output names usually use `camelCase`.
- A required field must be present. Optional fields may be omitted; their defaults are
  stated in the Meaning column.
- A tool with no input fields still receives an empty object: `{}`.
- `string / null` means the field is present but may contain `null`. The When column
  describes fields that are conditionally present.
- Runtime `tools/list` exposes every tool's `inputSchema`. At present,
  `alcomd3_list_repositories` also exposes a strict `outputSchema`.
- Results use MCP `structuredContent`. Most successful results contain `ok: true`.
  `alcomd3_get_activity_log_entry` and `alcomd3_get_technical_log_entry` return their
  detail object directly and do not contain `ok`.

Business failures have this shape and the outer MCP tool result has `isError: true`:

```json
{
    "ok": false,
    "error": {
        "code": "error_code",
        "message": "Actionable message",
        "data": {}
    }
}
```

`error.data` appears only when structured context is useful. See the
[MCP Tools specification](https://modelcontextprotocol.io/specification/2025-11-25/server/tools#error-handling)
for the distinction between schema, business, and protocol errors.

## Quick index

| Area | Tool | Behavior | Purpose |
| --- | --- | --- | --- |
| Projects | `alcomd3_list_projects` | Read-only | List registered projects. |
| Templates | `alcomd3_list_templates` | Read-only | List available environment-level templates. |
| Templates | `alcomd3_get_template` | Read-only | Read one environment-level template. |
| Templates | `alcomd3_create_template` | Write | Create a derived template. |
| Templates | `alcomd3_edit_template` | Destructive write | Edit a derived template by replacing its definition. |
| Templates | `alcomd3_set_template_package` | Idempotent write | Set one direct VPM dependency. |
| Templates | `alcomd3_remove_template_package` | Destructive write | Remove one direct VPM dependency. |
| Templates | `alcomd3_set_template_unitypackage` | Idempotent write | Set one UnityPackage attachment reference. |
| Templates | `alcomd3_remove_template_unitypackage` | Destructive write | Remove one UnityPackage attachment reference. |
| Templates | `alcomd3_remove_template` | Destructive write | Move a removable template to trash. |
| Projects | `alcomd3_get_project_details` | Read-only | Read a registered project. |
| Repositories | `alcomd3_list_repositories` | Read-only | List remote repositories and package visibility. |
| Repositories | `alcomd3_add_repository` | Network write | Add a remote VPM repository. |
| Repositories | `alcomd3_remove_repository` | Destructive write | Remove a user repository by URL. |
| Packages | `alcomd3_get_package_details` | Read-only | Read visible package metadata. |
| Packages | `alcomd3_list_packages` | Read-only | Page through all GUI-visible packages. |
| Packages | `alcomd3_list_repository_packages` | Read-only | Page through one repository's packages. |
| Environment | `alcomd3_get_environment_settings` | Read-only | Read Unity installations and default paths. |
| Activity | `alcomd3_search_activity_logs` | Read-only | Filter activity summaries. |
| Activity | `alcomd3_get_activity_log_entry` | Read-only | Read one activity entry. |
| Activity | `alcomd3_summarize_activity_logs` | Read-only | Aggregate activity records. |
| Activity | `alcomd3_get_activity_log_context` | Read-only | Read activity around an entry. |
| Technical logs | `alcomd3_search_technical_logs` | Read-only | Filter technical log previews. |
| Technical logs | `alcomd3_get_technical_log_entry` | Read-only | Read one technical log entry. |
| Technical logs | `alcomd3_summarize_technical_logs` | Read-only | Aggregate technical logs. |
| Projects | `alcomd3_create_project` | Long-running write | Create and register a Unity project. |
| Projects | `alcomd3_add_existing_project` | Write | Register an existing Unity project. |
| Projects | `alcomd3_backup_project` | Long-running write | Create a project zip backup. |
| Projects | `alcomd3_copy_project` | Long-running write | Copy and register a project. |
| Projects | `alcomd3_restore_project_from_backup` | Long-running write | Restore and register a project. |
| Project packages | `alcomd3_install_project_package` | Long-running write | Install a VPM package. |
| Project packages | `alcomd3_uninstall_project_package` | Destructive long task | Uninstall a package. |
| Project packages | `alcomd3_reinstall_project_package` | Long-running write | Reinstall a package. |

“Long-running” tools declare `execution.taskSupport: "optional"`. A client may use MCP
Tasks or a normal synchronous `tools/call`. See [Project long tasks](../mcp.md#project-long-tasks).

## Projects and templates

### `alcomd3_list_projects`

Lists projects registered in the ALCOMD3 database. It does not scan unregistered folders.

**Input:** No fields; pass `{}`.

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `projects` | [`ProjectSummary[]`](#projectsummary) | Always | Registered project summaries; records that cannot produce a valid path summary are skipped. |

### `alcomd3_list_templates`

Lists the environment-level templates currently available for creating projects. These are not
template data owned by a registered project. Source-file paths are not exposed.

**Input:** No fields; pass `{}`.

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `templates` | [`TemplateSummary[]`](#templatesummary) | Always | Template summaries and capability flags. |

### `alcomd3_get_template`

Reads one environment-level template selected by its stable ID. This does not inspect a registered
project. The ID selects the resource; this is still a read-only call.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `template_id` | `string` | Yes | A template `id` returned by `alcomd3_list_templates`; it must not be blank after trimming. |

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `template` | [`TemplateDetails`](#templatedetails) | Always | Summary and readable definition, without its storage path. |

### `alcomd3_create_template`

Creates a derived template. The backend generates and persists its stable ID.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `display_name` | `string` | Yes | User-visible template name. |
| `base_template_id` | `string` | Yes | Existing template whose `usableAsBase` flag is `true`. |
| `unity_version_range` | `string` | Yes | Parseable Unity version range. |
| `vpm_dependencies` | `object<string, string>` | Yes | Complete map of VPM package names to version ranges. |
| `unitypackage_paths` | `string[]` | Yes | Existing absolute regular-file paths ending in `.unitypackage`; an empty array is allowed. |

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `template` | [`TemplateDetails`](#templatedetails) | Always | Persisted definition and generated ID. |

Attachments are referenced, not copied. Invalid dependencies, ranges, paths, self-reference,
and base-template cycles are rejected.

### `alcomd3_edit_template`

Replaces the complete editable definition of a derived template while preserving its ID and
storage location.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `template_id` | `string` | Yes | Derived template to edit. |
| `display_name` | `string` | Yes | Replacement display name. |
| `base_template_id` | `string` | Yes | Replacement base template ID. |
| `unity_version_range` | `string` | Yes | Replacement Unity version range. |
| `vpm_dependencies` | `object<string, string>` | Yes | Replacement complete dependency map. |
| `unitypackage_paths` | `string[]` | Yes | Replacement complete attachment-path list. |

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `template` | [`TemplateDetails`](#templatedetails) | Always | Complete updated definition. |

Built-in and project-archive templates are not field-editable. The tool is marked destructive
because it uses replacement semantics.

### `alcomd3_set_template_package`

Sets one direct VPM dependency on a derived template. It stores a package-name/version-range
declaration; it does not select a repository, resolve dependencies, or install files.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `template_id` | `string` | Yes | Editable derived template ID. |
| `package_name` | `string` | Yes | Valid complete VPM package name. |
| `version_range` | `string` | Yes | Parseable VPM version range to add or replace. |

**Success output:** `ok: true` and the complete updated [`TemplateDetails`](#templatedetails) in
`template`. Repeating the same package and range is a no-op.

### `alcomd3_remove_template_package`

Removes one direct VPM dependency declaration from a derived template. It does not modify any
existing project.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `template_id` | `string` | Yes | Editable derived template ID. |
| `package_name` | `string` | Yes | Existing direct dependency to remove. |

**Success output:** `ok: true` and the complete updated [`TemplateDetails`](#templatedetails) in
`template`. A missing dependency returns `template_package_not_found`.

### `alcomd3_set_template_unitypackage`

Sets one UnityPackage attachment reference on a derived template.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `template_id` | `string` | Yes | Editable derived template ID. |
| `unitypackage_path` | `string` | Yes | Existing absolute regular-file path ending in `.unitypackage`. |

The path is canonicalized and referenced without copying the file. Repeating the same canonical
path is a no-op.

**Success output:** `ok: true` and the complete updated [`TemplateDetails`](#templatedetails) in
`template`.

### `alcomd3_remove_template_unitypackage`

Removes one UnityPackage attachment reference from a derived template. Copy the path from
`alcomd3_get_template`; the referenced file is never deleted and need not still exist.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `template_id` | `string` | Yes | Editable derived template ID. |
| `unitypackage_path` | `string` | Yes | Existing attachment path in the template definition. |

**Success output:** `ok: true` and the complete updated [`TemplateDetails`](#templatedetails) in
`template`. A missing reference returns `template_unitypackage_not_found`.

### `alcomd3_remove_template`

Moves a removable template to the system trash. Built-in templates cannot be removed and
referenced attachments are not deleted.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `template_id` | `string` | Yes | Template to remove. |

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `template` | [`RemovedTemplate`](#removedtemplate) | Always | Identity, name, and kind of the removed template. |

### `alcomd3_get_project_details`

Reads Unity and installed-package information for a registered project. It cannot read an
arbitrary unregistered directory.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `project_path` | `string` | Yes | Path that exactly matches a project registered in ALCOMD3. |

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `project` | [`ProjectDetails`](#projectdetails) | Always | Unity version, resolve state, and installed packages. |

### `alcomd3_create_project`

Creates a Unity project, resolves packages, and registers it. Supports an optional MCP Task.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `project_name` | `string` | Yes | One valid directory name, not a path, root, or `..`. |
| `base_path` | `string` | No | Absolute parent directory; defaults to the GUI project directory. |
| `template_id` | `string` | No | Template ID; omission follows the GUI's current template-selection rules. |
| `unity_version` | `string` | No | Unity version; omission follows the GUI's current template-selection rules. |

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `projectPath` | `string` | Always | Absolute new-project path. |
| `templateId` | `string` | Always | Template actually used. |
| `unityVersion` | `string` | Always | Unity version actually selected. |

### `alcomd3_add_existing_project`

Registers an existing Unity project without copying it.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `project_path` | `string` | Yes | Absolute path to a valid Unity project directory. |

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `projectPath` | `string` | Always | Path actually registered. |

### `alcomd3_backup_project`

Creates a zip backup for a registered project. Supports an optional MCP Task.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `project_path` | `string` | Yes | Registered project path. |
| `backup_name` | `string` | No | One valid filename without `.zip`; omission generates a name. Paths are rejected. |
| `exclude_vpm_packages` | `boolean` | No | Exclude installed VPM package contents when `true`; defaults to `false`. |

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `backupPath` | `string` | Always | Absolute path to the created zip. |

Backups are written to the GUI-configured backup directory and never overwrite a file.

### `alcomd3_copy_project`

Copies a registered project and registers the copy. Supports an optional MCP Task.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `source_project_path` | `string` | Yes | Registered source-project path. |
| `new_project_path` | `string` | Yes | Absolute non-existing destination that is not inside the source. |

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `projectPath` | `string` | Always | Path of the copied and registered project. |

### `alcomd3_restore_project_from_backup`

Restores a zip backup into the GUI default project directory and registers it. Supports an
optional MCP Task.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `backup_path` | `string` | Yes | Absolute path to an ALCOMD3 zip backup. |
| `project_name` | `string` | No | One valid restored-directory name; defaults to the backup filename. |

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `projectPath` | `string` | Always | Path of the restored and registered project. |

## Repositories, packages, and environment

### `alcomd3_list_repositories`

Lists all supported remote repositories and global package-visibility settings. Local
repositories are unsupported.

**Input:** No fields; pass `{}`.

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `repositories` | [`RepositorySummary[]`](#repositorysummary) | Always | Canonical, deduplicated array of Official, Curated, and user repositories. |
| `packageVisibility` | `object` | Always | Global package display settings. |
| `packageVisibility.hideLocalUserPackages` | `boolean` | Always | Whether local user packages are hidden. |
| `packageVisibility.showPrereleasePackages` | `boolean` | Always | Whether prerelease packages are shown. |

Use a returned `id` for package reads and a returned user-repository `url` for removal.

### `alcomd3_add_repository`

Downloads, validates, and adds a remote VPM repository, then clears package cache.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `repository_url` | `string` | Yes | Valid remote VPM repository URL and the identity used for later removal. |
| `headers` | `object<string, string>` | No | HTTP headers used to download the repository; defaults to an empty map. |

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `repository` | [`RepositoryMutationSummary`](#repositorymutationsummary) | Always | Added user-repository summary. |

An existing stored URL or declared repository ID is rejected as a duplicate. Activity logs
store only a redacted URL and header count, never header values.

### `alcomd3_remove_repository`

Removes one user repository by exact stored URL and clears package cache. Default repositories
cannot be removed.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `repository_url` | `string` | Yes | User-repository `url` from `alcomd3_list_repositories`; IDs are not accepted. |

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `repository` | [`RepositoryMutationSummary`](#repositorymutationsummary) | Always | Removed repository summary. |

### `alcomd3_list_packages`

Pages through package summaries visible under the same rules as the GUI package list. It does
not provide server-side text search.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `offset` | `integer >= 0` | No | Starting item offset; defaults to `0`. |
| `limit` | `integer >= 0` | No | Requested page size; defaults to `200` and is clamped to `1..=1000`. |

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `totalCount` | `integer` | Always | Total summaries after visibility filtering and source grouping. |
| `offset` | `integer` | Always | Offset used for this page. |
| `limit` | `integer` | Always | Effective page size. |
| `returnedCount` | `integer` | Always | Number returned in this page. |
| `hasMore` | `boolean` | Always | Whether another page exists. |
| `nextOffset` | `integer / null` | Always | Next offset, or `null` at the end. |
| `packages` | [`PackageSummary[]`](#packagesummary) | Always | Package summaries in this page. |

### `alcomd3_list_repository_packages`

Pages through GUI-visible package summaries from one remote repository.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `repository_id` | `string` | Yes | Repository `id` from `alcomd3_list_repositories`; URLs are not accepted. |
| `offset` | `integer >= 0` | No | Starting offset; defaults to `0`. |
| `limit` | `integer >= 0` | No | Page size; defaults to `200` and is clamped to `1..=1000`. |

**Success output:** Same pagination fields as `alcomd3_list_packages`, plus:

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `repository` | [`PackageRepositorySummary`](#packagerepositorysummary) | Always | Repository being read. |
| `packages` | [`PackageSummary[]`](#packagesummary) | Always | Current page from only that repository. |

### `alcomd3_get_package_details`

Reads detailed metadata for a GUI-visible package. Omitting filters may return multiple sources
or versions.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `package_name` | `string` | Yes | Full, nonblank VPM package identifier. |
| `version` | `string` | No | Exact version string. |
| `repository_id` | `string` | No | Restrict results to a remote repository ID; URLs are not accepted. |

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `packages` | [`PackageDetails[]`](#packagedetails) | Always | All matching GUI-visible package details; contains at least one item. |

### `alcomd3_get_environment_settings`

Reads stored Unity installations, launch arguments, and default paths. It does not start Unity
or scan additional disks.

**Input:** No fields; pass `{}`.

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `unityInstallations` | [`UnityInstallation[]`](#unityinstallation) | Always | Registered Unity installations. |
| `unityLaunchArguments` | `object` | Always | Configured, default, and effective Unity arguments. |
| `unityLaunchArguments.configured` | `string[] / null` | Always | User configuration, or `null` when unset. |
| `unityLaunchArguments.builtinDefault` | `string[]` | Always | Built-in ALCOMD3 defaults. |
| `unityLaunchArguments.effective` | `string[]` | Always | Arguments currently in effect. |
| `unityLaunchArguments.usesBuiltinDefault` | `boolean` | Always | Whether the built-in default is active. |
| `paths` | `object` | Always | Default directory settings. |
| `paths.defaultProjectPath` | `string` | Always | Default project directory. |
| `paths.projectBackupPath` | `string` | Always | Project backup directory. |

## Activity records

### Common activity filters

`alcomd3_search_activity_logs` and `alcomd3_summarize_activity_logs` share these inputs:

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `search` | `string` | No | Case-insensitive contains match across operation, summary, target, tool, and client names. |
| `sources` | `string[]` | No | Allowed values: `gui`, `mcp`, `deep_link`, `system`. |
| `kinds` | `string[]` | No | Allowed values: `read`, `write`, `passive`, `open`, `maintenance`. |
| `statuses` | `string[]` | No | Allowed values: `started`, `succeeded`, `failed`, `cancelled`, `info`. |
| `visibility` | `string` | No | `important`, `primary`, `secondary`, `technical`, or `all`; defaults to `important`. |
| `operations` | `string[]` | No | Restrict to internal operation identifiers. |
| `tool_names` | `string[]` | No | Restrict to MCP tool names. |
| `request_id` | `string` | No | Restrict to one MCP request ID. |
| `target` | `string` | No | Restrict to an operation target. |
| `since` | `RFC 3339 string` | No | Inclusive earliest time. |
| `until` | `RFC 3339 string` | No | Inclusive latest time; cannot precede `since`. |
| `offset` | `integer >= 0` | No | Page offset; defaults to `0`. |
| `limit` | `integer >= 0` | No | Page size; defaults to `50` and is clamped to `1..=200`. |
| `order` | `string` | No | `newest` or `oldest`; defaults to `newest`. |

### `alcomd3_search_activity_logs`

Pages through user-readable activity summaries using the common filters.

**Input:** Any [common activity filter](#common-activity-filters); all are optional, so `{}` is valid.

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `totalCount` | `integer` | Always | Total matching activities. |
| `offset`, `limit`, `returnedCount`, `hasMore`, `nextOffset` | pagination fields | Always | Current page state, with the same meaning as package pagination. |
| `entries` | [`ActivityEntrySummary[]`](#activityentrysummary) | Always | Activity summaries in this page. |

### `alcomd3_get_activity_log_entry`

Reads a complete activity entry selected by an ID from search or summary results.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `id` | `string` | Yes | Activity entry ID. |
| `include_details` | `boolean` | No | Include `details`; defaults to `true`. `false` returns an empty array. |

**Success output:** Returns [`ActivityEntry`](#activityentry) directly, without an `ok` wrapper.

### `alcomd3_summarize_activity_logs`

Aggregates matching activity records so a client can locate a useful range before reading it.

**Input:** Common activity filters, plus:

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `group_by` | `string` | No | `source`, `kind`, `status`, `operation`, `tool_name`, `client_name`, `day`, or `hour`; defaults to `source`. |

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `groupBy` | `string` | Always | Effective grouping dimension. |
| `totalCount` | `integer` | Always | Total matching activities. |
| `totalGroupCount` | `integer` | Always | Number of groups before pagination. |
| `offset`, `limit`, `returnedCount`, `hasMore`, `nextOffset` | pagination fields | Always | Group-page state. |
| `groups` | [`ActivitySummaryGroup[]`](#activitysummarygroup) | Always | Groups in this page. |

### `alcomd3_get_activity_log_context`

Reads an activity and neighboring records without fetching the complete log.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `id` | `string` | Yes | Center activity entry ID. |
| `before` | `integer >= 0` | No | Earlier entries; defaults to `5`, maximum `50`. |
| `after` | `integer >= 0` | No | Later entries; defaults to `5`, maximum `50`. |
| `include_details` | `boolean` | No | Include details in all three sets; defaults to `false`. |

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `entry` | [`ActivityEntry`](#activityentry) | Always | Center entry. |
| `before` | [`ActivityEntry[]`](#activityentry) | Always | Entries before the center. |
| `after` | [`ActivityEntry[]`](#activityentry) | Always | Entries after the center. |

## Technical logs

### Common technical-log filters

`alcomd3_search_technical_logs` and `alcomd3_summarize_technical_logs` share:

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `search` | `string` | No | Case-insensitive contains match across target and message. |
| `levels` | `string[]` | No | `error`, `warn`, `info`, `debug`, `trace`; defaults to `error` and `warn`. |
| `targets` | `string[]` | No | Case-insensitive target contains matches. |
| `scope` | `string` | No | `memory` or `recent_files`; defaults to `memory`. |
| `since` | `RFC 3339 string` | No | Inclusive earliest time. |
| `until` | `RFC 3339 string` | No | Inclusive latest time; cannot precede `since`. |
| `offset` | `integer >= 0` | No | Page offset; defaults to `0`. |
| `limit` | `integer >= 0` | No | Page size; defaults to `50` and is clamped to `1..=100`. |
| `max_message_chars` | `integer >= 0` | No | Search-preview limit; default and maximum are `300`. Summaries omit message text. |

### `alcomd3_search_technical_logs`

Pages through redacted, bounded technical-log previews.

**Input:** Any [common technical-log filter](#common-technical-log-filters); all are optional.

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `totalCount` | `integer` | Always | Total matching entries. |
| `offset`, `limit`, `returnedCount`, `hasMore`, `nextOffset` | pagination fields | Always | Current page state. |
| `entries` | [`TechnicalLogEntrySummary[]`](#technicallogentrysummary) | Always | Previews in this page. |

### `alcomd3_get_technical_log_entry`

Reads one search result; its message is redacted before being truncated.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `id` | `string` | Yes | Technical-log ID returned by search. |
| `max_message_chars` | `integer >= 0` | No | Maximum message length; default and maximum are `4000`. |

**Success output:** Returns [`TechnicalLogEntryDetails`](#technicallogentrydetails) directly,
without an `ok` wrapper.

### `alcomd3_summarize_technical_logs`

Aggregates matching technical logs.

**Input:** Common technical-log filters, plus:

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `group_by` | `string` | No | `level`, `target`, `file`, or `hour`; defaults to `level`. |

**Success output:**

| Field | Type | When | Meaning |
| --- | --- | --- | --- |
| `ok` | `boolean` | Always | `true` on success. |
| `groupBy` | `string` | Always | Effective grouping dimension. |
| `totalCount` | `integer` | Always | Total matching entries. |
| `totalGroupCount` | `integer` | Always | Number of groups before pagination. |
| `offset`, `limit`, `returnedCount`, `hasMore`, `nextOffset` | pagination fields | Always | Group-page state. |
| `groups` | [`TechnicalLogSummaryGroup[]`](#technicallogsummarygroup) | Always | Groups in this page. |

## Project package writes

### `alcomd3_install_project_package`

Installs one package from GUI-visible candidates compatible with the project's Unity version.
Supports an optional MCP Task.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `project_path` | `string` | Yes | Registered project path. |
| `package_name` | `string` | Yes | Valid full VPM package identifier. |
| `version_selector` | `object` | Yes | `{"type":"latest_gui_visible"}` or `{"type":"exact","version":"x.y.z"}`; exact versions must still be visible and compatible. |
| `source` | `object` | No | Optional remote-repository selector. |
| `source.repository_id` | `string` | Yes when `source` is present | Repository `id` returned by `alcomd3_list_repositories`; URLs are not accepted. |
| `allow_conflicts` | `boolean` | No | Allow dependency conflicts or legacy file/directory deletion; defaults to `false`. |

**Success output:** [`ProjectPackageChangeResult`](#projectpackagechangeresult).

### `alcomd3_uninstall_project_package`

Uninstalls one installed package. Supports an optional MCP Task and is marked destructive.

**Input:**

| Field | Type | Required | Meaning |
| --- | --- | --- | --- |
| `project_path` | `string` | Yes | Registered project path. |
| `package_name` | `string` | Yes | Valid VPM package currently installed in the project. |
| `allow_conflicts` | `boolean` | No | Allow conflicts or legacy deletion; defaults to `false`. |

**Success output:** [`ProjectPackageChangeResult`](#projectpackagechangeresult).

### `alcomd3_reinstall_project_package`

Reinstalls one installed package. Supports an optional MCP Task.

**Input:** Same as `alcomd3_uninstall_project_package`.

**Success output:** [`ProjectPackageChangeResult`](#projectpackagechangeresult).

All three tools first calculate pending changes. If authorization is needed while
`allow_conflicts` is `false`, they return `project_package_conflicts` with the same
[`PendingChanges`](#pendingchanges) shape in `error.data.changes` and do not modify the project.

## Shared output types

### `ProjectSummary`

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | `string / null` | Project display name. |
| `path` | `string` | Registered path. |
| `projectType` | `string` | Backend-detected project type. |
| `unity` | `string / null` | Unity version. |
| `unityRevision` | `string / null` | Unity revision. |
| `lastModified` | `integer / null` | Last-modified Unix time in milliseconds. |
| `createdAt` | `integer / null` | Creation Unix time in milliseconds. |
| `favorite` | `boolean` | Whether the project is a favorite. |
| `exists` | `boolean` | Whether the registered directory currently exists. |

### `TemplateSummary`

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | `string` | Original VPM repository name. |
| `displayName` | `string` | Non-empty display name; initialized from `name` and user-editable. |
| `id` | `string` | Stable management ID. |
| `unityVersions` | `string[]` | Unity versions available for project creation. |
| `updateDate` | `string / null` | Template update date. |
| `hasUnityPackages` | `boolean` | Whether the template references Unity packages. |
| `hasProjectArchive` | `boolean` | Whether it contains a project archive. |
| `available` | `boolean` | Whether it is currently usable. |
| `kind` | `builtIn / derived / projectArchive` | Template kind. |
| `editable` | `boolean` | Whether fields can be edited. |
| `removable` | `boolean` | Whether it can be removed. |
| `usableAsBase` | `boolean` | Whether a derived template may use it as a base. |

### `TemplateDetails`

Includes all [`TemplateSummary`](#templatesummary) fields plus:

| Field | Type | Meaning |
| --- | --- | --- |
| `baseTemplateId` | `string / null` | Base ID for a derived template. |
| `unityVersionRange` | `string / null` | Unity version range for a derived template. |
| `vpmDependencies` | `object<string, string>` | Package-name to version-range map. |
| `unityPackagePaths` | `string[]` | Referenced absolute attachment paths. |

### `RemovedTemplate`

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | `string` | Removed template ID. |
| `displayName` | `string` | Removed template name. |
| `kind` | `builtIn / derived / projectArchive` | Removed template kind. |

### `ProjectDetails`

| Field | Type | Meaning |
| --- | --- | --- |
| `path` | `string` | Project path. |
| `unity.major` | `integer` | Unity major version. |
| `unity.minor` | `integer` | Unity minor version. |
| `unity.version` | `string` | Complete Unity version. |
| `unity.revision` | `string / null` | Unity revision. |
| `shouldResolve` | `boolean` | Whether packages need resolution. |
| `installedPackages` | `object[]` | Installed packages. |
| `installedPackages[].id` | `string` | Package ID in project dependencies. |
| `installedPackages[].package` | [`PackageDetails`](#packagedetails) | Installed manifest summary without `source`. |

### `RepositorySummary`

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | `string` | Repository identity for package reads; falls back to URL if no ID is declared. |
| `url` | `string` | Remote URL; use this to remove a user repository. |
| `displayName` | `string` | Display name. |
| `kind` | `officialDefault / curatedDefault / user` | Sole repository classification field. |
| `hidden` | `boolean` | Whether GUI package visibility currently hides it. |

### `RepositoryMutationSummary`

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | `string / null` | Declared repository ID, or `null`. |
| `url` | `string` | Added or removed URL. |
| `name` | `string / null` | Original VPM repository name. |
| `displayName` | `string` | Non-empty display name at mutation time. |
| `kind` | `user` | Always a user repository. |

### `PackageRepositorySummary`

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | `string / null` | Cached repository ID. |
| `url` | `string / null` | Cached repository URL. |
| `name` | `string` | Original VPM repository name. |
| `displayName` | `string` | Non-empty repository display name. |
| `kind` | `officialDefault / curatedDefault / user` | Repository classification. |

### `PackageSummary`

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | `string` | Full VPM package identifier. |
| `displayName` | `string / null` | Display name. |
| `version` | `string` | Version. |
| `source` | [`PackageSource`](#packagesource) | Package source. |

### `PackageSource`

A remote source has these fields:

| Field | Type | Meaning |
| --- | --- | --- |
| `type` | `remote` | Indicates a package from a remote repository. |
| `kind` | `officialDefault / curatedDefault / userRepository` | Remote repository classification. |
| `id` | `string / null` | Declared repository ID. |
| `name` | `string` | Original VPM repository name. |
| `displayName` | `string` | Non-empty repository display name. |
| `url` | `string / null` | Repository URL. |

A local-user source has these fields:

| Field | Type | Meaning |
| --- | --- | --- |
| `type` | `localUser` | Indicates a package from a registered local-user package directory. |
| `kind` | `localUser` | Fixed local-user classification. |
| `isLocalUserPackage` | `true` | Explicit local-user-package marker. |

### `PackageDetails`

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | `string` | Full package identifier. |
| `displayName` | `string / null` | Display name. |
| `description` | `string / null` | Description. |
| `version` | `string` | Version. |
| `unity` | `object / null` | Unity requirement; contains `major` and `minor` when present. |
| `keywords` | `string[]` | Keywords. |
| `aliases` | `string[]` | Package aliases. |
| `vpmDependencies` | `string[]` | Dependency package IDs without version ranges. |
| `legacyPackages` | `string[]` | Superseded legacy packages. |
| `changelogUrl` | `string / null` | Changelog URL. |
| `documentationUrl` | `string / null` | Documentation URL. |
| `isYanked` | `boolean` | Whether this version is yanked. |
| `source` | [`PackageSource`](#packagesource) | Present in package-detail results; omitted from installed manifest summaries. |

### `ActivityEntrySummary`

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | `string` | Activity ID for detail and context tools. |
| `startedAt` | `RFC 3339 string` | Start time. |
| `finishedAt` | `RFC 3339 string / null` | Completion time, or `null` while unfinished. |
| `source` | `string` | Source such as `Gui`, `Mcp`, `DeepLink`, or `System`. |
| `kind` | `string` | Behavior such as `Read`, `Write`, or `Maintenance`. |
| `status` | `string` | Current or final state such as `Started`, `Succeeded`, or `Failed`. |
| `importance` | `string` | Visibility level: `Primary`, `Secondary`, or `Technical`. |
| `operation` | `string` | Stable internal operation identifier. |
| `summary` | `string` | Short user-readable activity description. |
| `target` | `string / null` | Resource or path being acted on. |
| `durationMs` | `integer / null` | Completed activity duration in milliseconds. |
| `requestId` | `string / null` | Associated MCP request ID. |
| `toolName` | `string / null` | Associated MCP tool name. |
| `clientName` | `string / null` | Calling MCP client name. |
| `detailCount` | `integer` | Number of key/value details in the complete entry. |
| `hasError` | `boolean` | Whether the complete entry contains an error. |
| `errorSummary` | `string / null` | Redacted and truncated error summary. |

### `ActivityEntry`

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | `string` | Activity ID. |
| `source` | `string` | Current values are `Gui`, `Mcp`, `DeepLink`, and `System`. |
| `kind` | `string` | Current values are `Read`, `Write`, `Passive`, `Open`, and `Maintenance`. |
| `status` | `string` | Current values are `Started`, `Succeeded`, `Failed`, `Cancelled`, and `Info`. |
| `importance` | `string` | `Primary`, `Secondary`, or `Technical`. |
| `operation` | `string` | Stable internal operation identifier. |
| `summary` | `string` | User-readable activity description. |
| `target` | `string / null` | Operation target. |
| `details` | [`ActivityDetail[]`](#activitydetail) | Redacted structured details; empty when `include_details` is `false`. |
| `requestId` | `string / null` | Associated MCP request ID. |
| `toolName` | `string / null` | Associated MCP tool name. |
| `clientName` | `string / null` | MCP client name. |
| `startedAt` | `RFC 3339 string` | Start time. |
| `finishedAt` | `RFC 3339 string / null` | Completion time. |
| `durationMs` | `integer / null` | Duration in milliseconds. |
| `error` | `string / null` | Redacted complete error text. |

Filter enums use lowercase `snake_case`; the table lists current serialized output enum values.

### `ActivityDetail`

| Field | Type | Meaning |
| --- | --- | --- |
| `key` | `string` | Detail key. |
| `value` | `string` | Redacted detail value. |

### `ActivitySummaryGroup`

| Field | Type | Meaning |
| --- | --- | --- |
| `key` | `string` | Grouping key. |
| `count` | `integer` | Entry count. |
| `failedCount` | `integer` | Failure count. |
| `cancelledCount` | `integer` | Cancellation count. |
| `latestEntryId` | `string / null` | Latest entry ID in the group. |
| `latestStartedAt` | `string / null` | Latest start time in the group. |

### `TechnicalLogEntrySummary`

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | `string` | Technical-log ID for the detail tool. |
| `time` | `RFC 3339 string` | Log time. |
| `level` | `string` | Current values are `Error`, `Warn`, `Info`, `Debug`, and `Trace`. |
| `target` | `string` | Rust target that emitted the entry. |
| `messagePreview` | `string` | Redacted message truncated to the search limit. |
| `truncated` | `boolean` | Whether the message was truncated. |
| `source` | `memory / file` | Current process memory or a recent log file. |
| `fileName` | `string / null` | Source filename; `null` for memory entries. |
| `lineNumber` | `integer / null` | Source-file line; `null` for memory entries. |

### `TechnicalLogEntryDetails`

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | `string` | Technical-log ID. |
| `time` | `RFC 3339 string` | Log time. |
| `level` | `string` | `Error`, `Warn`, `Info`, `Debug`, or `Trace`. |
| `target` | `string` | Rust target that emitted the entry. |
| `message` | `string` | Redacted message truncated to the requested detail limit. |
| `truncated` | `boolean` | Whether the message was truncated. |
| `source` | `memory / file` | Log source. |
| `fileName` | `string / null` | Source filename; `null` for memory entries. |
| `lineNumber` | `integer / null` | Source-file line; `null` for memory entries. |

### `TechnicalLogSummaryGroup`

| Field | Type | Meaning |
| --- | --- | --- |
| `key` | `string` | Grouping key. |
| `count` | `integer` | Log count. |
| `errorCount` | `integer` | Error count. |
| `warnCount` | `integer` | Warning count. |
| `latestEntryId` | `string / null` | Latest log ID in the group. |
| `latestTime` | `string / null` | Latest time in the group. |

### `ProjectPackageChangeResult`

| Field | Type | Meaning |
| --- | --- | --- |
| `ok` | `boolean` | `true` on success. |
| `operation` | `install / uninstall / reinstall` | Operation performed. |
| `projectPath` | `string` | Modified project path. |
| `packageName` | `string` | Target package identifier. |
| `changes` | [`PendingChanges`](#pendingchanges) | Applied-change summary. |

### `PendingChanges`

| Field | Type | Meaning |
| --- | --- | --- |
| `changes_version` | `integer` | Backend change-snapshot version. |
| `package_changes` | `[string, PackageChange][]` | Package-name and install/remove-change tuples. |
| `remove_legacy_files` | `string[]` | Legacy files to remove. |
| `remove_legacy_folders` | `string[]` | Legacy directories to remove. |
| `conflicts` | `[string, ConflictInfo][]` | Package-name and conflict-detail tuples. |

`PackageChange` is `{ "InstallNew": PackageInfo }` or
`{ "Remove": "Requested" / "Legacy" / "Unused" }`. The remove values mean explicitly
requested, superseded by another package, or no longer needed as a dependency.

### `PackageInfo`

| Field | Type | Meaning |
| --- | --- | --- |
| `name` | `string` | Full package identifier to install. |
| `display_name` | `string / null` | Package display name. |
| `description` | `string / null` | Package description. |
| `keywords` | `string[]` | Combined package aliases and keywords. |
| `version` | `object` | Structured SemVer with `major`, `minor`, `patch`, `pre`, and `build`. |
| `unity` | `[integer, integer] / null` | Required Unity major and minor versions. |
| `changelog_url` | `string / null` | Changelog URL. |
| `documentation_url` | `string / null` | Documentation URL. |
| `vpm_dependencies` | `string[]` | Dependency package identifiers. |
| `legacy_packages` | `string[]` | Superseded legacy packages. |
| `is_yanked` | `boolean` | Whether the version is yanked. |

### `ConflictInfo`

| Field | Type | Meaning |
| --- | --- | --- |
| `packages` | `string[]` | Package identifiers that conflict with the target change. |
| `unity_conflict` | `boolean` | Whether a Unity-version conflict exists. |
| `unlocked_names` | `string[]` | Package identifiers that must be unlocked while applying the change. |

### `UnityInstallation`

| Field | Type | Meaning |
| --- | --- | --- |
| `path` | `string` | Unity executable path. |
| `version` | `string` | Complete registered Unity version. |
| `loadedFromHub` | `boolean` | Whether this installation was loaded from a Unity Hub record. |
