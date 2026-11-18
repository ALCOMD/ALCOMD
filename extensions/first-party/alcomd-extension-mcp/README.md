# MCP 管理第一方扩展

扩展 ID：`com.vrclearn.alcomd.extension.mcp`

职责：

- MCP 客户端配置向导。
- 已连接客户端与活跃请求可视化。
- 权限、审批、操作进度与诊断。
- `alcomd-mcp` 组件状态展示。

本扩展不实现 MCP 协议，不直接读 token，不直接修改外部配置，也不管理 stdio 子进程。

M8 当前实现位于 `backend/`，使用获批独立 Host/WIT 和原 Portable UI 组件承载客户端与授权管理。
当前技术验证及尚未覆盖内容以 `docs/exec-plans/M8-mcp-protocol-and-management.md` 为准；不能由代码存在宣称已验收。
“活动”分段提供 Core 确认的 IPC 连接和在途 Core RPC，以及 MCP 适配器上报的工具列表变更订阅流。三个列表各自需要读取权限。订阅流会区分报告不可用、过期、覆盖不全及确实没有已跟踪记录，客户端身份仅在 Core 已关联时显示。HTTP 空闲连接和全部 MCP 协议请求未在该页面展示。预登记客户端 HTTPS/OAuth 已有隔离 Core/SDK 证据，2026-11-17 实际 Codex CLI 0.153.4 在仅信任测试证书的隔离进程中完成 OAuth 登录和读取。普通浏览器人工 TLS 接入及扩展人工检查仍待验证，CIMD/DCR 尚未支持。

构建（标准仓库根目录，Rust 1.97.1）：

```powershell
rustup target add wasm32-unknown-unknown
cargo build --locked --manifest-path extensions/first-party/alcomd-extension-mcp/backend/Cargo.toml --target wasm32-unknown-unknown --release
cargo xtask component extensions/first-party/alcomd-extension-mcp/backend/target/wasm32-unknown-unknown/release/alcomd_extension_mcp_backend.wasm target/new-management-component.wasm
```

编码入口只创建新文件。包仍须真实签名和完整性校验，用户选择安装即可，无额外发布者确认；本目录的占位fingerprint不是签名，
不能直接压缩当作可安装正式包。开发验证使用独立测试身份和密钥、可丢弃数据，不更改正式签名材料。
默认随产品安装/启用的分发尚需后续证据，不由一次开发包测试代替。

本扩展申请五项可选权限，包括客户端读取、客户端管理、Core 连接读取、在途 Core 请求读取和 MCP 订阅流读取。零授权即可启用并打开页面，
此时不显示客户端数据或写控件；明确授予读取权限后才能查看，修改仍须管理权限。
启用本身不授予权限，撤销后原会话失效，新页面仍可打开为未授权状态。
这一声明不改变其他扩展必需权限未授予时拒绝启用的既有合同。

凭据仅由本机所有者通过正式CLI一次签发：`alcomd-cli mcp client issue-credential <id> --expected-revision <n> --idempotency-key <key>`。
每次签发立即作废旧凭据；管道或JSON/NDJSON输出须显式`--secret-stdout`，结果未知不自动重签。
扩展不会读取、存储或展示凭据，不修改外部配置，不启动MCP子进程。
已登记客户端的详情提供接入提示。Codex 回环 Bearer 的配置核对、凭据传入与清理步骤见[客户端接入速查](../../../specs/mcp/toolset-v1.md#客户端接入速查)。

客户端按能力开关授权，项目读取和包修改覆盖当前及以后登记项目。
切换只编辑草稿，审阅实际增减并确认后调用原change-grants；不重建协议或授权系统。
扩展声明只是申请，不产生授权；旧局部授权经State37移除，OAuth旧项目令牌失效，须重新明确授权。
