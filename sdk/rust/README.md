# ALCOMD native Rust SDK

Use `alcomd-client` with the wire DTOs in `alcomd-protocol`. [The native method catalogue](../../specs/rpc/native-sdk-v1.methods.json)
maps each supported RPC method to its Rust function, capability, request schema and response schema.

```rust
use alcomd_client::{AlcomdClient, ClientConfig};

let mut client = AlcomdClient::connect(
    ClientConfig::for_installed_core().with_client_name("my-desktop-app"),
).await?;
let status = client.system_status().await?;
let first_page = client.projects_list(None, Some(25)).await?;
# Ok::<(), alcomd_client::ClientError>(())
```

For another repository, pin the ALCOMD Git revision or use a local checkout as a Cargo path dependency.
The standalone [example](examples/native-client) can be built from a separate directory with those
same dependencies. Use `probe` to connect to an installed Core and `probe-running` to connect without
starting it.

```toml
[dependencies]
alcomd-client = { git = "https://github.com/ALCOMD/ALCOMD.git", rev = "<tested-commit-sha>" }
alcomd-protocol = { git = "https://github.com/ALCOMD/ALCOMD.git", rev = "<tested-commit-sha>" }
```

Windows x86-64, Linux x86-64 and macOS arm64 use per-user local IPC. Official and external native
desktop apps run with that system user's Core authority. Each app must keep the connection out of
untrusted pages, WebViews and extensions. MCP authentication and the Extension API have their own
contracts.

Capabilities describe support on the negotiated RPC version. For live updates use
`EventSubscription::connect` and resume from the last cursor the app handled. Inspect
`ClientError::Remote` for a stable RPC code. An interrupted write may already have been accepted.
Recover by querying its operation or explicitly reusing the same idempotency key and request parameters.
