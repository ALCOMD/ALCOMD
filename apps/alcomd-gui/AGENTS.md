# GUI-specific instructions

- The GUI is a client and extension UI container, not a business-logic host.
- Do not add package, project, repository, MCP, Discord, or migration logic to Tauri commands.
- Tauri commands must remain thin system/UI adapters.
- MCP management UI and Discord UI must be implemented through the public extension contribution system.
- Use Material Design 3 and reusable `@alcomd/ui` primitives.
- Persist authoritative settings through ALCOMD RPC, not localStorage.
- localStorage may only contain disposable view state.
- Use `displayPath` for filesystem path labels and full-path tooltips. Use `PathField` when an editable field displays a path supplied by Core or a picker. Keep the original value for untouched submissions, identities, and filesystem actions. URLs have their own controls.
