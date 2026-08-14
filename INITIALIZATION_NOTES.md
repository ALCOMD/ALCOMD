# 初始化说明

本包用于启动 ALCOMD v4 的 M-1 审计阶段。

已包含：

- 全新 Rust workspace 与稳定命名。
- Tauri 2 + React + Vite GUI 骨架。
- `alcomd`、CLI、MCP、API、Extension Host、Bootstrap 的占位二进制。
- 第一方 MCP 管理扩展与 Discord 扩展的清单和占位 UI。
- 架构、ADR、RPC、Extension API、MCP、迁移和安全文档骨架。
- Codex `AGENTS.md`、项目 `.codex/config.toml`、ExecPlan 和首个任务提示词。
- CI、Dependabot、PowerShell / Bash 初始化与检查脚本。
- `xtask` 旧内部标识扫描。

刻意未包含：

- v3 源码或 Git 历史。
- 真实 VPM 实现。
- 真正的 IPC 与 daemon 生命周期。
- SQLite Schema。
- Discord IPC。
- MCP 工具实现。
- v3 数据解析器。
- 安装器 GUID、签名密钥和发布凭据。
- 最终许可证。

这些内容必须在审计、冻结基线和人工决策后逐里程碑实现。

## 首次本地验证后必须生成

本压缩包未携带在当前离线构建环境中无法可靠解析的依赖锁文件。首次在 Node.js 24 与 Rust 1.97.1 环境中执行 `scripts/setup.ps1` 和 M0 验证后，应生成并提交：

```text
Cargo.lock
package-lock.json
```

之后 CI 应由 `npm install` 切换为 `npm ci`，并禁止无意更新锁文件。
