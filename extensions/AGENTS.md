# Extension-specific instructions

- Extension UI uses semantic Portable UI documents delivered through Core/RPC and rendered by the GUI host. Packaged HTML/JavaScript, iframes, child WebViews, private GUI commands, and first-party-only renderer privileges are forbidden.
- Background logic runs as WASM/WASI through `alcomd-extension-host`.
- Native DLL, `.so`, and `.dylib` extensions are forbidden.
- Any new host capability requires a threat-model update, a public permission, Schema changes, and human approval.
- Extension IDs use reverse-DNS notation and never include the current brand generation.
