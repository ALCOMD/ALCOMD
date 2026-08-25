# Extension-specific instructions

- First-party and third-party extensions must use the same public manifest, permissions, Portable UI, Extension API, data API, and host capabilities.
- Extension UI is a semantic Portable UI document returned through Core/RPC. Do not add packaged HTML/JavaScript, WebView assets, private GUI commands, or first-party-only renderer authority.
- Do not add first-party-only hidden IPC, private Tauri commands, direct SQLite access, or direct project writes.
- UI runs in a sandboxed iframe or isolated WebView.
- Background logic runs as WASM/WASI through `alcomd-extension-host`.
- Native DLL, `.so`, and `.dylib` extensions are forbidden.
- Any new host capability requires a threat-model update, a public permission, Schema changes, and human approval.
- Extension IDs use reverse-DNS notation and never include the current brand generation.
