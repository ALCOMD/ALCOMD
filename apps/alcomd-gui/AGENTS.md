# GUI-specific instructions

- The GUI is a client and extension UI container, not a business-logic host.
- Do not add package, project, repository, MCP, Discord, or migration logic to Tauri commands.
- Tauri commands must remain thin system/UI adapters.
- MCP management UI and Discord UI must be implemented through the public extension contribution system.
- Use Material Design 3 and reusable `@alcomd/ui` primitives.
- Persist authoritative settings through ALCOMD RPC, not localStorage.
- localStorage may only contain disposable view state.
