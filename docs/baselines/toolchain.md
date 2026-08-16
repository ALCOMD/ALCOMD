# Toolchain Baseline

状态：M-1 已冻结

最后核验：2026-08-16

| Component | Baseline |
|---|---|
| Rust 构建工具链 | 1.97.1（精确固定） |
| Rust edition | 2024 |
| Node.js | 24 LTS（固定主版本，使用最新受支持的 24.x） |
| npm 依赖图 | 根 `package-lock.json`，CI 使用 `npm ci` |
| Tauri | 2.x，由 `Cargo.lock` 与 `package-lock.json` 固定解析结果 |
| Frontend | React + Vite + TypeScript + Material Web |

MCP 是独立协议基线，不属于编译工具链；冻结版本见 `docs/baselines/mcp.md` 和
`docs/baselines/source-lock.toml`。

## 固定策略

- `rust-toolchain.toml`、Cargo Workspace 与 CI 必须使用 Rust `1.97.1`。该值是 v4
  仓库构建工具链，不代表尚未发布 SDK 的公共 MSRV 承诺。
- Node.js 使用仍处于 LTS 的 `24.x`。根 `package.json` 约束主版本，锁文件固定依赖图；
  CI 不得使用会重新求解依赖的 `npm install`。
- 依赖版本以已提交锁文件为准。升级工具链或重新生成锁文件必须单独审查并运行完整
  Rust、前端、Tauri 与元数据检查。
- 平台原生依赖与 CI 覆盖见 `docs/baselines/platforms.md`。

Official references:

- Rust releases: `https://blog.rust-lang.org/releases/latest/`
- Node releases: `https://nodejs.org/en/about/previous-releases`
- Tauri GUI setup（仅 `alcomd-gui`）: `https://v2.tauri.app/start/create-project/`
- Tauri GUI structure（仅 `alcomd-gui`）: `https://v2.tauri.app/start/project-structure/`
- Node.js 24 LTS: `https://nodejs.org/en/blog/migrations/v22-to-v24`
