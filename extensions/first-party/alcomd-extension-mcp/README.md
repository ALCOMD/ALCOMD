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
尚无实时连接/活跃请求/订阅查询，界面不把未撤销记录显示为在线。完整 OAuth 继续另行实施。

构建（标准仓库根目录，Rust 1.97.1）：

```powershell
rustup target add wasm32-unknown-unknown
cargo build --locked --manifest-path extensions/first-party/alcomd-extension-mcp/backend/Cargo.toml --target wasm32-unknown-unknown --release
cargo xtask component extensions/first-party/alcomd-extension-mcp/backend/target/wasm32-unknown-unknown/release/alcomd_extension_mcp_backend.wasm target/new-management-component.wasm
```

编码入口只创建新文件。包仍须真实签名、发布者确认和显式权限授权；本目录的占位fingerprint不是签名，
不能直接压缩当作可安装正式包。开发验证使用独立测试身份和密钥、可丢弃数据，不更改正式签名材料。
默认随产品安装/启用的分发尚需后续证据，不由一次开发包测试代替。

凭据仅由本机所有者通过正式CLI一次签发：`alcomd-cli mcp client issue-credential <id> --expected-revision <n> --idempotency-key <key>`。
每次签发立即作废旧凭据；管道或JSON/NDJSON输出须显式`--secret-stdout`，结果未知不自动重签。
扩展不会读取、存储或展示凭据，不修改外部配置，不启动MCP子进程。
