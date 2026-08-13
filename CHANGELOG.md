# Changelog

All notable changes to ALCOMD3 will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
This English file is the canonical record. GitHub Release bodies combine its
target version entry with the structurally matched Japanese and Simplified
Chinese entries, in that order after English.
Stable entries describe the final net changes since the previous stable release;
prerelease entries describe changes since the immediately previous published
release, whether stable or prerelease.

During normal development, add each notable user-facing change to `Unreleased`
in the same change or pull request. Use the appropriate `Added`, `Changed`,
`Deprecated`, `Removed`, `Fixed`, or `Security` category and omit empty
categories. This file is a curated release record, not a commit or pull-request
log.

For a beta release, move the applicable `Unreleased` changes into the new dated
version entry. For a stable release, curate the final net changes since the
previous stable release from the intervening prerelease entries and
`Unreleased`; intentional overlap with those prerelease entries is expected.
Leave a fresh `Unreleased` section after every release.
Before release validation, synchronize the target version entry in
`CHANGELOG/CHANGELOG.ja.md` and `CHANGELOG/CHANGELOG.zh-CN.md`; their date,
category order, and bullet counts must match this canonical entry.

## [Unreleased]

### Added

- Added per-item controls to the Discord extension so users can choose whether to show the Unity project folder name, Unity version, and open-editor count, and can append optional custom text to the activity. When enabled, the Unity version appears in the activity title.

### Changed

- Simplified preference changes so switches and selectors update immediately without a saving state, explicit rollback, or error notification; after each write, the interface rereads the persisted value, while failures remain visible in the logs.

### Fixed

- Persisted the MCP tool-name display and log-view preferences in `gui-config.json` and stopped relying on WebView local or session storage for user settings.
- Reserved the final 1% of project package-operation progress for backend finalization, so the progress display reaches 100% only after the operation completes.
- Kept selected project packages selected when the package-change confirmation dialog is cancelled.
- Corrected the Discord preview to show no activity while Unity is not running and to mirror the text and elapsed-time format actually published to Discord; removed the redundant full-path notice and the ineffective session-duration switch, and now always publishes Unity's real start time instead of allowing Discord to substitute one.
- Fixed Unity asset import worker processes being counted as additional open editors in Discord activity.

## [3.3.0-beta.2] - 2026-08-12

### Added

- Added a built-in Discord extension, enabled by default, with a dedicated live Unity status and preview page. A separate persisted sharing switch remains off by default and controls Discord publishing. It can publish the current project folder name, Unity version, session duration, and open-editor count with Unity artwork and an ALCOMD3 badge through the local Discord desktop client without exposing full project paths. When multiple editors are open, a newly started or foreground Unity editor becomes primary and remains primary while another app is in the foreground; platforms without foreground detection use the most recently started editor.

### Fixed

- Displayed the invalid computer-name notice as a high-severity red error because this condition causes VRCSDK uploads to fail.
- Aligned all seven GUI locales on the same keys, runtime values, confirmation semantics, compatibility-warning severity, and user-facing feature coverage, and added automated consistency checks to prevent structural localization drift.
- Kept repository-list imports running after individual repository download failures, committed the successful repositories, and added per-repository progress with retry for unfinished entries.
- Counted failed package operations and repository downloads as processed in progress bars so finished operations reach 100%, moved repository progress to the concurrent preview download, reused that result for final saving instead of downloading twice, started the standard package refresh independently after repository additions finish, and kept cancellations at their actual progress.

## [3.3.0-beta.1] - 2026-08-10

### Added

- Added persistent user-defined display names for repositories.
- Added a persistent MCP tool-name display switch between call names and localized names, with the alternate name shown on hover.

### Changed

- Embedded the MCP HTTP server, all 33 tools, direct business dispatch, and shared task management in the GUI process while preserving the existing loopback URL and bearer-token client configuration.
- Upgraded MCP to RMCP 3.1.2 with explicit `2026-07-28` sessionless requests and experimental extension Tasks, while retaining ordinary `2025-11-25` tool-call compatibility for legacy sessions.
- Standardized MCP package repository selection on repository IDs; repository URLs remain limited to adding and removing repositories.
- Selected desktop shortcut creation by default for new Windows installations while preserving the previous choice during upgrades.
- Added an optional Windows uninstall choice to remove settings, caches, and other local application data without deleting projects or backups stored in Documents.
- Made the MCP tool and project card lists responsive up to three columns based on their container width, with more room required before switching to three columns.

### Fixed

- Kept activity-log table columns stable and prevented layout shifts when auxiliary records are toggled.

### Removed

- Removed the standalone `alcomd3-mcp` executable, its private IPC protocol and endpoint metadata, and the obsolete core Tasks `tasks/list` and `tasks/result` compatibility paths; Windows upgrades and uninstall still clean up the historical helper file.

## [3.2.0] - 2026-08-09

### Added

- Added MCP discovery and management for project templates through the shared
  resource-management backend.
- Added this standardized changelog as the canonical record of notable changes
  between releases.

### Changed

- Simplified MCP repository and template payloads and aligned repository
  management on URL identity.
- Simplified the Unity launch-state labels.
- Extended Unity editor focusing to macOS and improved process matching and
  Windows editor-readiness caching.
- Consolidated GitHub Release descriptions into this changelog and moved localized
  updater summaries to structured release metadata.

### Fixed

- Stopped issue templates and in-app report links from requesting the obsolete
  `vrc-get-gui` label.
- Preserved user-configured package paths when saving environment settings on
  Windows.
- Detected and focused Unity editors when any matching Unity process is active.
- Localized package-operation errors and prevented long repository fields from
  overflowing their interface.

### Removed

- Removed the standalone versioned release-description directory and its
  duplicate content.

## [3.1.0] - 2026-08-01

### Added

- Added a bearer-token-protected, loopback-only MCP Streamable HTTP endpoint
  with optional client setup.
- Added Unity project opening and ready-state tracking, duplicate-launch
  prevention, and editor focusing.
- Added MCP template editing for VPM dependencies and UnityPackage attachment
  references.

### Changed

- Kept extension pages accessible when sidebar entries are hidden and made
  Projects, Resources, and Settings permanently available in the sidebar,
  repairing existing configurations that hid them.
- Allowed the built-in MCP extension to be disabled, revoking access, stopping
  endpoints, removing its sidebar entry, and cancelling application-owned MCP
  project tasks.
- Displayed the main window and MCP tools without waiting for background
  extension or endpoint startup work.
- Kept project lists visible during refreshes and refreshed project types and
  saved repository names promptly.
- Preserved unrelated client settings during optional MCP setup and required
  clients configured for the former stdio transport to switch to the protected
  endpoint and token.

### Security

- Kept MCP disabled by default and validated bearer authentication, host, and
  origin information on the local endpoint.

## [3.1.0-beta.3] - 2026-08-01

### Added

- Added Unity project opening and ready-state tracking, duplicate-launch
  prevention, and editor focusing.

### Changed

- Kept project lists visible during refreshes, refreshed saved repository names,
  and clarified built-in extension behavior.

## [3.1.0-beta.2] - 2026-07-28

### Added

- Allowed the built-in MCP extension to be disabled, revoking access, stopping
  endpoints, removing its sidebar entry, and cancelling application-owned MCP
  project tasks.

### Changed

- Displayed the main window and MCP tools while endpoint startup continues in
  the background.
- Refreshed project types immediately after project creation or VRChat SDK
  package changes.

## [3.1.0-beta.1] - 2026-07-28

### Added

- Added a bearer-token-protected MCP Streamable HTTP endpoint and optional
  client setup for Codex, Claude Code, and Cursor while preserving unrelated
  client settings.

### Changed

- Grouped endpoint details and client setup in the MCP configuration dialog.
- Refreshed project types immediately after project creation or VRChat SDK
  package changes.
- Required MCP clients configured for the former stdio transport to switch to
  the protected endpoint and token.

### Security

- Kept MCP disabled by default and validated bearer authentication, host, and
  origin information on the loopback-only endpoint.

## [3.0.1-beta.1] - 2026-07-27

### Added

- Added an Open button for installed extensions whose sidebar entries are
  hidden.

### Changed

- Kept Projects, Resources, and Settings visible in the sidebar and made
  sidebar visibility controls more predictable, repairing existing
  configurations that hid permanent entries.

## [3.0.0] - 2026-07-26

### Added

- Published the first public ALCOMD3 application release for Windows x64,
  macOS Apple Silicon, and Linux x86_64.

### Security

- Used ALCOMD3-owned update endpoints, an embedded public key, and signed update
  payloads.

[Unreleased]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.3.0-beta.2...HEAD
[3.3.0-beta.2]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.3.0-beta.1...v3.3.0-beta.2
[3.3.0-beta.1]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.2.0...v3.3.0-beta.1
[3.2.0]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.1.0...v3.2.0
[3.1.0]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.0.0...v3.1.0
[3.1.0-beta.3]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.1.0-beta.2...v3.1.0-beta.3
[3.1.0-beta.2]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.1.0-beta.1...v3.1.0-beta.2
[3.1.0-beta.1]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.0.1-beta.1...v3.1.0-beta.1
[3.0.1-beta.1]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.0.0...v3.0.1-beta.1
[3.0.0]: https://github.com/ALCOMD3/ALCOMD3/releases/tag/v3.0.0
