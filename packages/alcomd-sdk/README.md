# ALCOMD TypeScript SDK

`@alcomd/sdk` exposes browser-safe RPC types and the method catalogue. Desktop Node.js 24 applications connect through `@alcomd/sdk/node` and the native bridge packaged for their operating system. The bridge uses the current operating-system user's authority. Run it only in applications that should operate ALCOMD as that user.

```ts
import { AlcomdSdkError, connect, type AlcomdClient } from "@alcomd/sdk/node";

let client: AlcomdClient | undefined;
try {
    client = await connect();
    const status = await client.request("system.status", {});
    console.log(status.state);
} catch (error) {
    if (error instanceof AlcomdSdkError) {
        console.error(error.code ?? error.kind);
    } else {
        throw error;
    }
} finally {
    client?.close();
}
```

Each command connection runs one request at a time. Use separate connections for concurrent requests. A bridge error after dispatch can report `outcomeUnknown` for a mutation. Reconnect and use the original idempotency key and input when the RPC method supports recovery. The SDK never issues a replacement write or changes its key.

`subscribeEvents({ cursor })` opens a separate connection and returns the Core receipt and an async iterable for its notifications. When the receipt requires a resync, load fresh authoritative state before applying notifications. Retain the cursor only after consuming its batch. To recover after a disconnect, open another subscription with that cursor. Close the subscription when the consumer no longer needs it.

Build and install the local archive on Windows x64, Linux x64 or macOS arm64. Run the following commands from the workspace root. Use `alcomd-sdk-bridge.exe` as the bridge executable on Windows.

```text
cargo build --locked --release --bin alcomd-sdk-bridge
npm run pack:local --workspace @alcomd/sdk -- --bridge ../../target/release/alcomd-sdk-bridge
npm install /absolute/path/to/alcomd-sdk-0.1.0-<platform>-<arch>.tgz
```

Run the install command in your application. The first two commands create a local development archive under the workspace's `target/m10-sdk` directory. The bridge discovers or starts Core through the native installation registration. Trusted development setups can supply an absolute `helperPath` to `connect`.
