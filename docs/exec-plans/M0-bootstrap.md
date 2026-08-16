# M0：仓库骨架、身份与 CI

状态：计划已细化，必须在 M-1 人工批准后才能执行

## 目标

把当前初始化骨架收敛为可复现、跨平台验证的空 Workspace：固定产品身份、工具链、锁文件、
crate/app 边界和 CI，所有占位程序能够构建，但不实现 M1 RPC 或任何业务功能。

## 非目标

- 不实现 IPC、daemon 单实例或 `system.hello`。
- 不创建 SQLite Schema、Operation、Event、Lock 或 Recovery。
- 不实现 VPM、项目、仓库、模板、备份或 Unity 用例。
- 不实现真正的 Extension Host、MCP、Discord、Local API 或迁移。
- 不选择或实现正式安装器、签名、发布、更新端点和远端写操作。
- 不把能编译的占位程序描述为已完成功能。

## 背景与约束

- ALCOMD 是一个基于 Rust 的本地应用平台，其官方 GUI 使用 Tauri 构建；只有
  `alcomd-gui` 是 Tauri 子应用，Workspace 其他组件共同构成完整产品。
- 架构边界以 `docs/architecture/ALCOMD-V4.md` 和 Accepted ADR 为准。
- 工具链以 `docs/baselines/toolchain.md` 为准。
- 平台构建前提和当前 CI 缺口以 `docs/baselines/platforms.md` 为准。
- M-1 必须已经完成并获得项目所有者人工批准；不得从 M-1 自动进入本计划。
- A-017/A-024/A-025 已批准三个发布平台、四种主要格式、独立全产品发行模型与平台技术
  基线；M0 必须按 Windows 10 22H2/Windows 11、Ubuntu 22.04 + `GLIBC_2.35` 符号上限、
  macOS arm64 deployment target 11.0 配置验证。

## 影响范围

允许修改：

```text
Cargo.toml
Cargo.lock
rust-toolchain.toml
alcomd.product.toml
apps/* 的空骨架
crates/* 的空骨架
packages/*
package.json
package-lock.json
scripts/*
.cargo/*
.github/*
xtask/*
docs/status.md
docs/exec-plans/M0-bootstrap.md
```

禁止修改或实现：

```text
migrations/v3/ 中的迁移解析器
真实 RPC 方法与传输
数据库 Schema
Extension API/权限正式版本
VPM 解析与项目写入
正式签名、Secrets、更新端点和发行资产
```

## 接口决策

M0 不新增公共 RPC、数据库 Schema、Extension API 或权限名称。唯一允许冻结的是仓库与
发行身份元数据：二进制名、Bundle ID、AUMID、URI Scheme、数据目录名和 Workspace 包名；
它们必须从 `alcomd.product.toml` 派生并由 `cargo xtask check` 校验。

## 实施步骤

1. 验证当前 Workspace 成员、包名、crate 依赖方向和所有占位入口；删除重复或与永久身份冲突
   的骨架，不引入兼容别名。
2. 统一 Rust `1.97.1`、edition 2024、rustfmt、Clippy 和禁止未说明 `unsafe` 的仓库规则。
3. 让 `alcomd.product.toml` 成为身份唯一来源，并让 xtask/元数据校验覆盖 Cargo、Tauri、npm、
   扩展 Manifest 和禁止旧内部标识扫描。
4. 使用 Node.js 24 LTS 与根 `package-lock.json` 固定前端依赖；本地安装说明与 CI 全部切换为
   `npm ci`，证明空 GUI 和共享包可复现构建。
5. 使 PowerShell 与 Bash 的 setup/check/test 脚本具有相同检查面，并显式报告缺少的平台原生
   依赖；脚本不得静默跳过失败。
6. 所有 Cargo 检查使用 `--locked` 或等价只读锁定方式，并在 CI 末尾验证 Cargo/npm 锁文件
   没有变化；独立 Discord backend 也执行 test gate。
7. 根据 A-017/A-024/A-025 配置 Windows、Ubuntu 与 macOS CI 骨架；
   安装 `alcomd-gui` 所需 Tauri 前提，三平台都执行显式 `tauri build --no-bundle`。该命令只
   验证 GUI 子应用，不收集其他组件，不创建完整产品、安装器、签名或 Release。
8. 运行全部验收命令，检查 diff 不包含 M1 或后续功能，更新本计划进度日志和 `docs/status.md`，
   然后停止。

## 验证命令

Windows PowerShell 7：

```powershell
.\scripts\setup.ps1
.\scripts\check.ps1
cargo check --locked -p alcomd-gui
npm run gui:build -- --no-bundle
git diff --check
```

Linux/macOS：

```bash
./scripts/setup.sh
./scripts/check.sh
cargo check --locked -p alcomd-gui
npm run gui:build -- --no-bundle
git diff --check
```

各平台脚本最终必须覆盖：

```text
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo run --locked -p xtask -- check
npm run check
npm run build
python/python3 scripts/validate-metadata.py
```

## 验收标准

- 所有已批准平台的 CI 必须通过，且任务使用固定 action commit 和最小权限。
- Windows 正式验证覆盖 Windows 10 22H2 与 Windows 11；Linux 构建使用 Ubuntu 22.04 并检查
  产物最高所需 glibc 符号不超过 `GLIBC_2.35`；macOS arm64 固定 deployment target 11.0。
- `Cargo.lock` 与 `package-lock.json` 由固定工具链生成并提交；CI 前端安装仅使用 `npm ci`。
- 所有 Cargo 任务锁定依赖，构建后 `Cargo.lock` 与 `package-lock.json` 均无 diff。
- 所有 Workspace 成员、独立第一方扩展后台、Tauri Rust 壳和前端均通过适用检查。
- `npm run gui:build` 与 Tauri `--no-bundle` 只被记录为 GUI 验证，不被描述为完整产品发行命令；
  `cargo xtask dist` 与四种正式格式留在独立 M12 ExecPlan。
- 二进制名、Bundle ID、AUMID、URI Scheme 与数据目录符合 `alcomd.product.toml`。
- `cargo xtask check` 和元数据校验拒绝生产范围中的旧内部标识。
- Git 状态只包含 M0 允许范围；没有 IPC、数据库、VPM、扩展运行时、MCP 或迁移实现。
- `docs/status.md` 更新为 M0 已完成并指向 M1 人工规划/审批；不自动开始 M1。

## 风险与回滚

- 平台 CI 可能因 Tauri 系统包或 runner 镜像变化失败；固定 runner/依赖并记录官方来源，不通过
  删除平台任务来规避。
- 锁文件重生成可能引入大范围依赖变化；单独审查锁文件，必要时恢复到 M0 开始前提交，而不是
  手工拼接锁文件。
- 身份扫描可能误报迁移目录中的合法 v3 证据；扫描规则必须区分生产范围与
  `migrations/v3/`，不得放宽生产范围限制。
- M0 尚无用户数据或公共协议迁移，回滚应只恢复本里程碑文件，不修改 Git 历史或远端。

## 进度日志

- 2026-08-16：M-1 期间完成计划细化；当前仓库已有初始化骨架，但尚未按本计划执行或验收。
- 待执行：O-003 已由 A-025 关闭；在 M-1 其余审计条件完成并获人工批准后开始步骤 1。

## 人工审批点

- 开始 M0 前：项目所有者批准 M-1 功能基线、证据和迁移清单。
- 调整跨平台 CI 前：复核 A-025 的 runner/镜像实现不会把测试范围误写成主动 OS 拒绝逻辑。
- M0 完成后：项目所有者审查身份、依赖图、CI 与变更范围；批准前不得进入 M1。
