# Extension MCP Provider fixture

Disposable signed-package test source for the approved provider/core WIT. This is not the MCP management extension or a formal MCP/Discord product. The lifecycle activate deliberately fails: a tool-only call must not invoke background/UI activation. Negative tools exercise invalid output, trap, quota exhaustion, undeclared Host use and a second persistent action.

Build with the frozen Rust toolchain and `wasm32-unknown-unknown`, then use existing `cargo xtask component <module> <component>` and the normal signed-package installation flow. No separate publisher approval is required; signature verification and explicit capability grants remain mandatory. No production bypass or direct Core/database access exists in the guest.
