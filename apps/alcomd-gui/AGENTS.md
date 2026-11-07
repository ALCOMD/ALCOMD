# GUI-specific instructions

- Keep Tauri commands limited to system/UI adapters and typed RPC bridging.
- Use Material Design 3. Reuse an existing shared component that fits the behavior, including app-level components built on `@alcomd/ui` primitives.
- Store only disposable view state in localStorage. Persist authoritative settings through ALCOMD RPC.
- Use `displayPath` for filesystem path labels and full-path tooltips. Use `PathField` when an editable field displays a path supplied by Core or a picker. Keep the original value for untouched submissions, identities, and filesystem actions. URLs have their own controls.
