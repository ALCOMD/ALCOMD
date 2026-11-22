# 客户端权限与扩展平台基线

状态：M-1 架构边界已审计，公共契约仍为 Draft

最后核验：2026-08-16

## 信任边界

ALCOMD 的入口共享同一个 `alcomd-application` 用例层，但不共享万能身份或隐式信任：

| 主体 | 身份与授权 | 禁止路径 |
|---|---|---|
| 官方与外部原生 GUI/CLI | 系统账号限定的本地 IPC，共享本机所有者权限 | 直写数据库或业务文件，绕过 Core 校验 |
| MCP 客户端 | 独立 Principal，协议请求映射到 ALCOMD 权限与审计 | 把 MCP 传输连接当作永久授权 session |
| 第一方/第三方扩展 | 每扩展 Principal、Manifest 权限、数据命名空间 | 第一方私有 IPC、Tauri command、直接项目写入 |

原生 RPC 由 OS 端点限定账号，Core 继续校验权限、revision、确认和业务目标。MCP 和扩展
按各自 Principal 和授权合同处理。

## 第一方与第三方一致性

第一方扩展与第三方扩展必须使用相同：

- `.alcomdext` 包格式与 `alcomd-extension.toml`。
- Extension API、权限名称、UI Bridge、Host Capability 和数据 API。
- UI 隔离、WASM/WASI 后台宿主、资源配额、崩溃隔离和撤销语义。
- 哈希、签名、发行者身份与安装/更新流程。

允许的差异仅为可信发行源、签名身份、默认安装策略和官方支持等级；这些差异不能产生额外业务能力。

## UI 与后台边界

- UI 运行在 sandboxed iframe 或隔离 WebView，不能访问主 DOM、Tauri IPC 或宿主凭据。
- UI Bridge 验证扩展实例、origin、协议版本、请求 ID、消息大小、并发与权限。
- Extension ABI v1 使用 WASI 0.2 Component Model 与版本化 WIT；后台运行在
  `alcomd-extension-host`，不加载 DLL、`.so` 或 `.dylib`。
- 运行时采用实现时合适的 Wasmtime LTS，固定主版本线；兼容的安全与关键正确性补丁必须
  升级，不能把“固定版本”解释成拒绝安全维护。
- 首发采用 WASI 0.2。采用其他 WASI 版本时单独评估宿主支持；首发前直接修订唯一的 Extension ABI v1。
- 宿主只暴露窄 Host Capability；扩展不能获得裸文件系统、命名管道、数据库或凭据句柄。
- 禁用扩展或撤销权限时必须关闭 UI 实例、后台租约和已授予句柄。

## 客户端管理与扩展验收

### 客户端管理

- GUI 设置页「客户端」通过 Core 管理连接。关闭 GUI 不影响 `alcomd-mcp` 的 STDIO/HTTP 协议服务。
- 请求、连接、订阅流、客户端、权限、审批与配置管理全部通过公开 API。
- 不读取 MCP 适配器私有内存或创建 GUI 私有控制通道。
- 使用 `mcp.requests.read`、`mcp.connections.read`、`mcp.subscription-streams.read`，不使用
  已批准移除的 `mcp.sessions.read`。

### Discord 扩展

- 扩展独立实现 Discord 协议与连接，仅通过应用无关的公开接口调用宿主。任何新增通用能力按独立合同审批。[M9 ExecPlan](../exec-plans/M9-discord-first-party-extension.md)
- 不读取 Discord token、用户数据库，不模拟用户操作，不控制其他应用 Presence。
- 禁用、卸载、权限撤销或后台租约结束时立即清除 Presence 并停止通信。

Discord 扩展默认安装，新用户默认禁用。v3 升级
用户迁移原 Discord 启用状态。

## 必须进入后续契约测试的行为

- AI 客户端连接可独立登记与撤销。扩展的权限可逐项授予、拒绝、查看和撤销。
- required 权限被拒绝时扩展不能启用；optional 权限被拒绝时只能降级，不得扩大权限。
- 权限不隐式包含子权限；高风险调用同时检查 Principal、资源范围与当前授权。
- 撤销后旧连接、请求、订阅流、句柄和并发请求不能继续获得新能力。
- 写调用使用核心 Plan/Apply、revision、幂等与资源锁，并进入活动审计。
- 恶意 UI、WASM 崩溃、CPU/内存/网络滥用和供应链替换不能影响核心完整性。
- 第一方扩展在测试中必须能被替换为具有相同公开权限的测试扩展，以证明不存在隐藏捷径。

## 尚未关闭

- Manifest v1、其余权限名称、Host Capability 与 UI Bridge 仍为 Draft，冻结前需要人工审批。
- Wasmtime 的具体 LTS 主版本在 Extension Host 实施里程碑按当时维护状态选择，必须符合 A-022。

## 依据

- `docs/adr/0002-single-writer-daemon.md`
- `docs/adr/0003-application-boundaries.md`
- `docs/adr/0006-client-permissions.md`
- `docs/adr/0007-extension-sandbox.md`
- `docs/adr/0011-first-party-public-api.md`
- `docs/adr/0012-mcp-protocol-ui-separation.md`
- `docs/adr/0013-discord-first-party-extension.md`
- `specs/extensions/*.md`
- `specs/security/extension-threat-model.md`
- `specs/security/local-client-threat-model.md`
