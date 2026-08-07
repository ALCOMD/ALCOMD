# Changelog

All notable changes to ALCOMD3 will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
GitHub Release bodies are generated from the corresponding version entry.

## [Unreleased]

### Added

- Added MCP discovery and management for project templates through the shared
  resource-management backend.
- Added this standardized changelog as the canonical record of notable changes
  between releases.

### Changed

- Simplified MCP repository and template payloads and aligned repository
  management on URL identity.
- Simplified the Unity launch-state labels.
- Consolidated GitHub Release descriptions into this changelog and moved localized
  updater summaries to structured release metadata.

### Fixed

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
  Projects, Resources, and Settings permanently available in the sidebar.
- Displayed the main window and MCP tools without waiting for background
  extension or endpoint startup work.
- Kept project lists visible during refreshes and refreshed project types and
  saved repository names promptly.

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
  client setup for Codex, Claude Code, and Cursor.

### Changed

- Grouped endpoint details and client setup in the MCP configuration dialog.
- Refreshed project types immediately after project creation or VRChat SDK
  package changes.

### Security

- Kept MCP disabled by default and validated bearer authentication, host, and
  origin information on the loopback-only endpoint.

## [3.0.1-beta.1] - 2026-07-27

### Added

- Added an Open button for installed extensions whose sidebar entries are
  hidden.

### Changed

- Kept Projects, Resources, and Settings visible in the sidebar and made
  sidebar visibility controls more predictable.

## [3.0.0] - 2026-07-26

### Added

- Published the first public ALCOMD3 application release for Windows x64,
  macOS Apple Silicon, and Linux x86_64.

### Security

- Used ALCOMD3-owned update endpoints, an embedded public key, and signed update
  payloads.

[Unreleased]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.1.0...HEAD
[3.1.0]: https://github.com/ALCOMD3/ALCOMD3/releases/tag/v3.1.0
[3.1.0-beta.3]: https://github.com/ALCOMD3/ALCOMD3/releases/tag/v3.1.0-beta.3
[3.1.0-beta.2]: https://github.com/ALCOMD3/ALCOMD3/releases/tag/v3.1.0-beta.2
[3.1.0-beta.1]: https://github.com/ALCOMD3/ALCOMD3/releases/tag/v3.1.0-beta.1
[3.0.1-beta.1]: https://github.com/ALCOMD3/ALCOMD3/releases/tag/v3.0.1-beta.1
[3.0.0]: https://github.com/ALCOMD3/ALCOMD3/releases/tag/v3.0.0
