# M0：仓库骨架、身份与 CI

状态：草案，M-1 后补全。

## 目标

让全新 workspace 在支持的平台上通过格式化、静态检查、单元测试、前端检查与身份扫描，同时不实现 M1 RPC。

## 非目标

- 不实现 IPC。
- 不创建 SQLite Schema。
- 不实现 VPM。
- 不实现真正的 Extension Host。
- 不实现 MCP 或 Discord。
- 不实现迁移。

## 允许修改

```text
Cargo.toml
apps/* 的骨架
crates/* 的骨架
packages/*
scripts/*
.github/*
xtask/*
alcomd.product.toml
```

## 验收命令

```powershell
.\scripts\setup.ps1
.\scripts\check.ps1 -SkipGuiRust
cargo check -p alcomd-gui
npm run gui:build
```

## 停止条件

- 所有命令通过。
- 二进制名称与 Bundle ID 正确。
- `cargo xtask check` 无旧内部标识。
- `Cargo.lock` 与 `package-lock.json` 已由固定工具链生成并提交。
- CI 的前端安装使用 `npm ci`。
- CI 通过。
- `docs/status.md` 更新。
- 不开始 M1。
