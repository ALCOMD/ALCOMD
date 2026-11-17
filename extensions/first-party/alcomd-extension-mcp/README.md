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
“活动”分段已通过独立只读Host/WIT读取Core确认的IPC连接和在途Core RPC；这不等于HTTP空闲连接、全部MCP协议请求或订阅，后面三项尚未接通。界面不把未撤销客户端记录显示为在线。预登记客户端HTTPS/OAuth已有隔离Core/SDK证据，并于2026-11-17通过实际Codex CLI0.153.4的隔离OAuth登录/读取；仅本轮子进程信任公开测试证书，不等同普通浏览器人工TLS接入。扩展相关后续人工测试按所有者要求延后，未覆盖项仍保留；CIMD/DCR尚不支持，不据此宣称任意客户端均兼容或M8完成。

构建（标准仓库根目录，Rust 1.97.1）：

```powershell
rustup target add wasm32-unknown-unknown
cargo build --locked --manifest-path extensions/first-party/alcomd-extension-mcp/backend/Cargo.toml --target wasm32-unknown-unknown --release
cargo xtask component extensions/first-party/alcomd-extension-mcp/backend/target/wasm32-unknown-unknown/release/alcomd_extension_mcp_backend.wasm target/new-management-component.wasm
```

编码入口只创建新文件。包仍须真实签名和完整性校验，用户选择安装即可，无额外发布者确认；本目录的占位fingerprint不是签名，
不能直接压缩当作可安装正式包。开发验证使用独立测试身份和密钥、可丢弃数据，不更改正式签名材料。
默认随产品安装/启用的分发尚需后续证据，不由一次开发包测试代替。

本扩展申请四项可选权限：客户端读取、客户端管理、Core连接读取和在途Core请求读取，不申请自身用不到的项目读取权限。零授权即可启用并打开页面，
此时不显示客户端数据或写控件；明确授予读取权限后才能查看，修改仍须管理权限。
启用本身不授予权限，撤销后原会话失效，新页面仍可打开为未授权状态。
这一声明不改变其他扩展必需权限未授予时拒绝启用的既有合同。

凭据仅由本机所有者通过正式CLI一次签发：`alcomd-cli mcp client issue-credential <id> --expected-revision <n> --idempotency-key <key>`。
每次签发立即作废旧凭据；管道或JSON/NDJSON输出须显式`--secret-stdout`，结果未知不自动重签。
扩展不会读取、存储或展示凭据，不修改外部配置，不启动MCP子进程。

客户端的八项能力用开关编辑，不逐项目授权；项目读取和包修改覆盖当前及以后登记项目。
切换只编辑草稿，审阅实际增减并确认后调用原change-grants；不重建协议或授权系统。
扩展声明只是申请，不产生授权；旧局部授权经State37移除，OAuth旧项目令牌失效，须重新明确授权。
