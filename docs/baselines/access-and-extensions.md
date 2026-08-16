# 客户端权限与扩展平台基线

状态：M-1 架构边界已审计，公共契约仍为 Draft

最后核验：2026-08-16

## 信任边界

ALCOMD 的入口共享同一个 `alcomd-application` 用例层，但不共享万能身份或隐式信任：

| 主体 | 身份与授权 | 禁止路径 |
|---|---|---|
| 官方 GUI/CLI | 明确客户端身份、用户限定本地 IPC | 直接数据库、项目文件或 VPM 实现 |
| 外部原生应用 | 独立 Principal、配对、最小权限、可撤销凭据 | 共享 token、绕过审批 |
| MCP 客户端 | 独立 Principal，协议请求映射到 ALCOMD 权限与审计 | 把 MCP 传输连接当作永久授权 session |
| 第一方/第三方扩展 | 每扩展 Principal、Manifest 权限、数据命名空间 | 第一方私有 IPC、Tauri command、直接项目写入 |

本地 IPC ACL 只限制操作系统用户，不替代应用级 Principal、权限、revision、审批和审计。

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
- WASI 0.3 不阻塞 4.0.0；后续只能经 ABI v1 兼容层或 Extension ABI v2 引入。
- 宿主只暴露窄 Host Capability；扩展不能获得裸文件系统、命名管道、数据库或凭据句柄。
- 禁用扩展或撤销权限时必须关闭 UI 实例、后台租约和已授予句柄。

## 第一方扩展验收不变量

### MCP 管理扩展

- 禁用或卸载管理 UI 不影响 `alcomd-mcp` 的 STDIO/HTTP 协议服务。
- 请求、连接、订阅流、客户端、权限、审批与配置管理全部通过公开 API。
- 不读取 MCP 适配器私有内存或创建 GUI 私有控制通道。
- 使用 `mcp.requests.read`、`mcp.connections.read`、`mcp.subscription-streams.read`，不使用
  已批准移除的 `mcp.sessions.read`。

### Discord 扩展

- 只使用公开权限 `integrations.discord.presence` 与窄 Discord Presence Host Capability。
- 不读取 Discord token、用户数据库，不模拟用户操作，不控制其他应用 Presence。
- 禁用、卸载、权限撤销或后台租约结束时立即清除 Presence 并停止通信。

默认状态：MCP 管理扩展默认安装并启用；Discord 扩展默认安装但新用户默认禁用；v3 升级
用户迁移原 Discord 启用状态。

## 必须进入后续契约测试的行为

- 每个外部客户端和扩展的授权可单独授予、拒绝、查看和撤销。
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
