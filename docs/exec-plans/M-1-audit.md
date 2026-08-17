# M-1：只读审计、基线冻结与开放决策

## 目标

在不写生产实现的前提下，建立可验证的 v3、vrc-get 功能与行为、VPM 生态兼容、MCP、安装器与迁移基线，并把 v4.0.0 功能范围变为机器可读发布合同。

## 非目标

- 不实现 daemon、RPC、数据库或 GUI 功能。
- 不导入 v3 源码。
- 不新增生产依赖。
- 不修改旧只读仓库。
- 不配置远端、发布、签名或 Secrets。

## 输入

- `AGENTS.md`
- `docs/architecture/ALCOMD-V4.md`
- `docs/decisions/open.md`
- `../ALCOMD3-v3-readonly`
- `../vrc-get-readonly`（仅用于可复现的行为基线，不是代码上游）
- 真实 v3 安装快照与脱敏 Fixture（缺失时保持 artifact `confirmed = false` 并后移到 M11，
  不得为完成 M-1 伪造或放宽删除授权）
- 公开 VPM 格式、生态兼容要求与脱敏项目 Fixture

## 允许修改

```text
docs/baselines/
docs/decisions/
docs/testing/
docs/status.md
feature-parity.toml
migrations/v3/artifacts.toml
migrations/v3/fixtures/
docs/exec-plans/M0-bootstrap.md
scripts/freeze-baselines.ps1
scripts/validate-metadata.py
Cargo.toml（仅修正规范仓库 URL）
extensions/first-party/alcomd-extension-discord/backend/Cargo.toml（仅修正规范仓库 URL）
```

## 禁止修改

```text
apps/
crates/
extensions/
packages/
sdk/
alcomd.product.toml
```

除非修正文档中已经确认的明显拼写错误。

## 工作流

1. 使用 `scripts/freeze-baselines.ps1` 确定性生成 `source-lock.toml`，锁定 v3 审计源、v3.4.0 迁移入口版本与 GitHub Release 资产、vrc-get 功能行为源和 MCP 规范版本。
2. 以冻结的 vrc-get commit 审计功能、安全行为、CLI 和错误处理，并冻结公开 VPM 格式、生态兼容需求和独立实现边界；该 commit 不授权源码复用。
3. 用只读 Subagent 分别审计：
    - v3 用户功能与隐性行为。
    - 固定版本 vrc-get 的全部用户功能、安全行为、CLI、错误处理，以及 VPM 公开格式和生态兼容行为；只读审计不得演变为实现源码复用。
    - 安装器、更新器、路径和残留。
    - MCP、权限与扩展边界。
    - 跨平台构建和测试。
4. 将每个功能写入 `feature-parity.toml`。
5. 为每个结论记录源码路径、符号、截图或安装快照证据。
6. 完善 `migrations/v3/artifacts.toml`，未知条目不得标记 confirmed。
7. 将必须由所有者决定的问题写入 `docs/decisions/open.md`。
8. 完成 M0 ExecPlan，但不执行 M0。
9. 更新 `docs/status.md`。
10. 停止。

## 验证

```powershell
.\scripts\freeze-baselines.ps1 -Check
cargo xtask check
python .\scripts\validate-metadata.py
```

## 验收标准

- `source-lock.toml` 中 v3 与 vrc-get 审计用 commit 均不为空。
- v3.4.0 迁移测试资产具有 release ID、asset ID、名称、大小、SHA-256、对应签名资产和 updater 公钥指纹。
- `scripts/freeze-baselines.ps1 -Check` 通过；本地仓库不是 shallow clone，标签集合完整，commit/tag 已由远端证明，所有锁定输入均为 `frozen`。
- 所有已知第三方图标均在 `docs/baselines/asset-provenance.toml` 中具有来源、许可证和摘要。
- `feature-parity.toml` 不再只有示例项，所有 v3 与冻结版本 vrc-get 用户入口均有记录。
- 所有 release blocker 有来源与验收测试计划。
- 所有迁移删除对象有所有权证据或保持 `confirmed = false`。
- 开放决策与技术 TODO 分离。
- M0 有明确允许修改范围、命令和停止条件。
- 未修改生产代码。
- `docs/status.md` 指向人工审核，而不是自动进入 M0。
- `feature-parity.toml` 仅在上述细粒度审计与证据要求全部满足后才能设置 `m1_complete = true`。

## 人工审批点

M-1 完成后必须等待项目所有者批准：

- 发布平台最低系统与运行库基线已由 A-025 批准，不再是开放审批点；后续只需提交真实机器
  与发行产物证据证明实现符合该基线。
- 永久 Windows AppId/GUID；安装器技术已由 A-024/ADR 0015 决定为独立全产品流水线和
  单一 Inno Setup EXE，具体实现留在 M12 ExecPlan。A-025 已决定当前平台代码签名身份不是
  4.0.0 前置审批；应用层更新签名合同仍按 ADR 0010 执行。
- v3 删除清单。

Local API/SDK 范围、第一方扩展默认状态、GUI 等价边界、MCP Tasks 映射、Extension ABI v1、
MCP 管理权限、全产品打包模型和平台技术基线已由 A-018 至 A-025 批准。
