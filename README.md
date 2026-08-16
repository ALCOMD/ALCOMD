# ALCOMD3 v4

ALCOMD3 v4 是 ALCOMD 产品家族中具有独立 Git 历史与代码库的全新项目。它在
品牌和功能定位上继承 ALCOMD3 v3，但不是对 v3 的增量补丁，也不复用 v3 或
vrc-get 的源码。

当前状态：**M-1 审计与规划阶段**。仓库中只有架构、契约骨架、最小可编译程序和 Codex 工作约束。不得把这些占位实现误认为已完成功能。

## 永久身份

| 用途 | 值 |
|---|---|
| 当前用户品牌 | `ALCOMD3` |
| 产品家族 | `ALCOMD` |
| 技术名称根 | `alcomd` |
| Bundle / Tauri identifier | `com.cqmhv.alcomd` |
| Windows AUMID | `CQMHV.ALCOMD` |
| URI Scheme | `alcomd://` |
| 数据目录 | `ALCOMD` |

用户品牌、产品家族与技术身份彼此独立；品牌未来变化时，技术身份不得随之变化。

## 架构一句话

`alcomd` 是唯一状态持有者和项目写入者。GUI、CLI、MCP、Local API、第三方应用和扩展都通过同一套 ALCOMD RPC 与应用用例访问核心。

MCP 协议由独立的 `alcomd-mcp` 提供。MCP 管理 GUI 与 Discord Rich Presence 分别作为第一方扩展实现，并且只能使用与第三方扩展相同的公开 Extension API、权限和沙箱。

## 开始前

1. 阅读 `AGENTS.md`。
2. 阅读 `docs/architecture/ALCOMD-V4.md`。
3. 阅读 `docs/decisions/open.md`。
4. 完成 `docs/exec-plans/M-1-audit.md`，不要直接开始生产实现。
5. 将旧仓库放在并列的只读目录，例如 `../ALCOMD3-v3-readonly`。
6. 运行 `scripts/freeze-baselines.ps1` 生成 v3 审计源、v3.4.0 迁移入口版本与发行资产、vrc-get 功能行为及 MCP 规范锁，并用 `-Check` 对远端引用和 GitHub Release 摘要执行校验。
7. 遵守洁净实现边界：不得复制、移植或改写 v3、vrc-get 或 vrc-get-vpm 源码。

## 本地初始化

Windows PowerShell：

从浏览器下载 ZIP 时，建议在解压前先解除 Internet 区域标记：

```powershell
Unblock-File .\ALCOMD-v4-initial-r1.zip
Expand-Archive .\ALCOMD-v4-initial-r1.zip -DestinationPath .\ALCOMD
```

如果已经解压，则在仓库根目录执行：

```powershell
Get-ChildItem -Recurse -File | Unblock-File
.\scripts\init-repo.ps1
.\scripts\setup.ps1
.\scripts\check.ps1 -SkipGuiRust
```

Linux / WSL：

```bash
./scripts/init-repo.sh
./scripts/setup.sh
./scripts/check.sh --skip-gui-rust
```

Tauri GUI：

```powershell
npm install
npm run gui:dev
```

## Codex 第一项任务

将 `docs/prompts/M-1-audit.md` 的内容交给 Codex，并使用 Plan mode。M-1 完成并经人工审核前，不应进入 M0 实现。

## 目录概要

```text
apps/                    可执行程序
crates/                  核心 Rust crate
extensions/first-party/  第一方扩展参考实现
packages/                TypeScript SDK 与 UI 包
sdk/                     其他语言 SDK 占位
docs/                    架构、ADR、ExecPlan 与审计材料
specs/                   RPC、扩展、MCP、迁移和安全契约
migrations/v3/           与正常运行时隔离的 v3 迁移代码与样本
xtask/                   仓库一致性检查
```

## 验证状态

打包时已完成的静态校验和仍需在目标开发机执行的构建命令见 `VALIDATION_REPORT.md`。

## 许可证

ALCOMD v4 自有代码、SDK、规范、文档、脚本与第一方扩展统一采用
[`AGPL-3.0-only`](LICENSE)。v4 是独立新项目，v3 只用于迁移与功能行为审计，
vrc-get 不是代码上游。详细边界见 `LICENSE-DECISION.md`；第三方依赖与图标仍受
各自许可证约束，见 `THIRD_PARTY_NOTICES.md`。

## 重要限制

- 不包含真实签名私钥、GitHub token 或发布凭据。
- `alcomd-migrate-v3` 仅为占位程序，不包含旧格式解析器。
- 所有二进制当前只证明命名与 workspace 边界，尚未实现 RPC、VPM 或迁移。
- 首次完成 M0 本地验证时应生成并提交 `Cargo.lock` 与 `package-lock.json`，随后将 CI 的 npm 安装切换为 `npm ci`。
