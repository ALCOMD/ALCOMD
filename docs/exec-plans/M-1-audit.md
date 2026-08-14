# M-1：只读审计、基线冻结与开放决策

## 目标

在不写生产实现的前提下，建立可验证的 v3、vrc-get、MCP、安装器与迁移基线，并把 v4.0.0 功能范围变为机器可读发布合同。

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
- `../vrc-get-readonly`
- 真实 v3 安装快照与脱敏 Fixture

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
```

## 禁止修改

```text
apps/
crates/
extensions/
packages/
sdk/
Cargo.toml
alcomd.product.toml
```

除非修正文档中已经确认的明显拼写错误。

## 工作流

1. 冻结 v3 commit、tag、版本与数据 Schema。
2. 冻结 vrc-get commit、tag 与版本。
3. 用只读 Subagent 分别审计：
    - v3 用户功能与隐性行为。
    - vrc-get 功能与安全边界。
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
cargo xtask check
python .\scripts\validate-metadata.py
```

## 验收标准

- `source-lock.toml` 中 v3 与 vrc-get commit 不为空。
- `feature-parity.toml` 不再只有示例项，所有 v3 用户入口均有记录。
- 所有 release blocker 有来源与验收测试计划。
- 所有迁移删除对象有所有权证据或保持 `confirmed = false`。
- 开放决策与技术 TODO 分离。
- M0 有明确允许修改范围、命令和停止条件。
- 未修改生产代码。
- `docs/status.md` 指向人工审核，而不是自动进入 M0。

## 人工审批点

M-1 完成后必须等待项目所有者批准：

- 最终许可证。
- v3 迁移入口范围。
- 发布平台矩阵。
- Local API / SDK 的 v4 范围。
- MCP Tasks 映射。
- 安装器技术和永久 GUID。
- v3 删除清单。
