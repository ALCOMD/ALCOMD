# ALCOMD Extension ABI v1

状态：M7 active ABI v1；Portable UI 以 pre-release direct replacement 进入同一个 world。

## Runtime 与 world

- ABI v1 使用 WebAssembly Component Model 与 `alcomd:extension@1.0.0` versioned WIT package。
- exact world 是 `alcomd:extension/extension-v1@1.0.0`。
- imports：`host-projects@1.0.0`、`host-data@1.0.0`；exports：`guest-lifecycle@1.0.0`、
  `guest-ui@1.0.0`。
- WIT call shape 是同步 request/response。Host implementation 与 guest invocation 使用 Wasmtime async embedding，
  以便 host use case await、wall timeout 与 cancellation；ABI v1 不使用 Component Model future/stream。
- guest language 不受限制，只要产物是满足 exact world 的 Component；Rust、C/C++、Go 等没有 privileged SDK。

## No ambient WASI authority

Component Model 是 ABI/runtime 基础，不等于授予 `wasi:cli/imports`。ABI v1 第一条 slice 不链接任何 WASI 0.2
interface，包括 clocks、random、filesystem、sockets、DNS 和 terminal。Host 不使用 inherit convenience config。

默认 guest 不获得 preopened/arbitrary filesystem、environment、argv、stdin/stdout/stderr、socket/DNS、process
control、terminal、daemon RPC socket、Host private IPC 或 OS credential store。未来每个 import 必须是已审批的
ALCOMD WIT interface，或另行审批的 exact WASI 0.2 interface。

Host configuration 明确禁止 `inherit_env`、`inherit_args`、`inherit_stdio`、`inherit_network` 与 preopened current
directory 等 convenience configuration；未列入 exact linker allowlist 的 import 一律不能实例化。

## Negotiation

1. Manifest `api` 必须等于 1；另一 major 返回 `extension_api_unsupported`。
2. Host 解析 required/optional interface ID，required 缺失则拒绝 enable；unknown optional 可忽略。
3. Host 只链接 permission/grant/scope 当前允许的 interface。Manifest request 不是授权。
4. `guest-lifecycle` 与 `guest-ui` exports 的 exact type 必须匹配；缺失或不匹配是
   `extension_api_unsupported`，不是 trap。所有 ABI v1 Component 都必须 export `guest-ui`；没有 Manifest `[ui]`
   时 daemon 不调用它，reference guest使用empty/default stub。
5. activation context 是 Host 提供的诊断/代际信息，guest 不能用它声明 Principal、publisher、trust 或 scope。

## Shape compatibility

已发布 WIT function signature 或现有 type 的 field 增删改、variant/enum 语义改变、tuple/result/option 改变、
parameter/result 改变、existing required import 改变均 breaking。WIT record 不具有 JSON unknown-field semantics。

首发前直接修订版本 1 的 WIT，并同步宿主、guest、生成的绑定和组件夹具。宿主只按现行 world 校验组件。
当前接口与导出类型记录在 `abi-compatibility-v1.json`。

## Stable WIT error

M8 client management uses `alcomd:mcp-management/clients@1.0.0`, negotiated by Manifest.
Its `management-v1` world includes the base world and adds the client management import.
See `mcp-management-v1.md` for types and errors.

The additive `alcomd:extension/background-integration-v1@1.0.0` world includes the exact base world and adds `host-unity-sessions`, `host-local-ipc`, and `guest-background`, each at `1.0.0`. A signed manifest opts into the world by declaring all three interface identifiers as required. The base Host world cannot import these capabilities. Core schedules a single awaited tick per active background instance. Interactive UI and MCP tool invocations cannot use the new imports. Local IPC handles are opaque and do not persist between daemon runs. [WIT](wit/extension-v1/background-integration.wit)

`extension-error.code` 是 machine fact。`diagnostic-id` 只允许用于 `internal-error`，不得包含 path、trap backtrace、
environment、argv、token 或 extension-owned value。lease/grant/scope error 在 guest boundary 分别保持稳定。
