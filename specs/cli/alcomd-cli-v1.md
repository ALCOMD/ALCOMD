# ALCOMD CLI contract v1

状态：M5 已发布命令面的生产实现与本地验收已完成（2026-08-24；等待最终 Hosted CI 与人工验收）

本合同只定义 `alcomd-cli` 的公共进程行为。CLI 永远是
`alcomd-cli -> alcomd-client -> RPC -> alcomd-application` 的适配器，不读取 `state.db`、项目、
repository、package cache 或备份文件，也不直接启动 Unity。命令只有在对应 daemon
capability 已真实实现时才可进入 help；合同中存在名称不等于后端已经可用。

`package install` 和 `package upgrade` 默认建议省略 `--version-range`，由 Core 推荐兼容版本。指定范围时，Core 在范围内选择。安装、升级和降级均可用 `--repository-id` 或 `--user-package-id` 指定一个来源。两个参数互斥。省略来源时由 Core 选择。

手动安装、升级或降级时，可用 `--allow-unity-incompatible` 为本次操作的目标包版本启用 Unity 兼容性例外。安装和升级须同时提供精确选择器，例如 `--version-range "=1.2.3"`。降级使用位置参数指定版本，例如 `1.2.3`。依赖包仍按项目的 Unity 版本检查。

`project list`、`repository list`、`repository packages` 可将上一页的 JSON `nextCursor` 原样传给 `--cursor`。任务列表的两个游标字段成对传递；`--active-only` 选择活动任务。

权威 machine-readable 工件：

- `alcomd-cli-v1.schema.json`：machine output、stream record、退出码与全局选项。
- `command-catalog-v1.json`：已发布的命令和 alias。只列出已有真实 daemon capability 的入口。
- [ZIP 备份与恢复命令](backup-commands-v1.json)定义 `backup create` 和 `backup restore` 的参数、确认及任务回执。

## 输出模式

`human` 是默认模式；`--json` 与 `--ndjson` 互斥。

| 模式 | stdout | stderr |
|---|---|---|
| human | 最终结果 | progress、warning、diagnostic、确认提示与 fatal error |
| JSON 成功 | 恰好一个 `result` document | 空；不得混入普通日志 |
| JSON 失败 | 空 | 恰好一个稳定 `error` document |
| NDJSON | 每行一个带 `type` 的完整 JSON record | 非结果 diagnostic/log；不得混入 NDJSON |

NDJSON Operation follow 可依次输出 `operation`、`progress`、`event`，最后输出 `result` 或 `error`。
stream 尚未开始时发生的 CLI/transport error 写 stderr；stream 已开始后的终态错误是 stdout 最后一条
`error` record，随后非零退出。所有模式遇到 broken pipe 必须有界退出且不得 panic。

`--quiet` 只抑制非错误 human progress/diagnostic；不抑制最终结果或 fatal error，也不改变 JSON/
NDJSON。普通日志不得污染任何 machine stdout。

## 退出码

| code | 语义 |
|---:|---|
| 0 | success |
| 1 | command、domain 或 Operation failure |
| 2 | CLI usage / argument error |
| 3 | 显式 partial-success batch result |
| 130 | 本地 CLI interrupted/detached |

稳定 RPC/CLI `error.code` 是机器分类来源；不得为每个 error.code 分配 shell code。

## 交互、Plan 与 Operation

- 非 TTY 永不读取确认；stdin EOF 永不重试或等待。`--json`/`--ndjson` 总是 non-interactive。
- 需要确认的 mutation 未带 `--yes` 时立即返回 `confirmation_required`（exit 1）。
- `--yes` 只跳过已经冻结的用户确认，不绕过 permission、revision、Unity writer gate、Plan stale、
  source revalidation 或安全检查。
- `--dry-run` 对有 Plan 的 mutation 只创建并返回 durable immutable Plan，不 Apply；允许 Plan row，
  但不得产生外部 filesystem/business mutation。
- mutation 返回 Operation 时默认 follow 到终态；`--no-wait` 在接受后立即返回 OperationId。
- Ctrl-C/SIGINT 发生在确认或 Operation 创建前时以 130 退出且不产生 mutation。OperationId 已产生后
  只 detach，输出该 ID，以 130 退出；daemon Operation 继续运行。业务取消必须显式调用
  `operation cancel`。

`project create <target-parent> <name> --unity-version <version> --idempotency-key <key>` 默认直接创建项目。
选用模板时同时提供 `--template-id <id>` 和 `--template-revision <revision>`。
命令直接接受任务，支持 `--no-wait`，不接受 `--dry-run`。

## completion

`completion <shell>` 只根据静态 command tree 向 stdout 输出 script，不连接/启动 daemon，不访问
数据库、网络、repository 或项目。dynamic completion 后置。`clap_complete` 尚未获批，生产实现若
需要它必须先提交精确依赖审批。

## 首发合同

正式发布前 CLI 合同编号固定为 1。变更命令、参数、退出码或输出格式时，同步改写命令目录、调用方和测试。CLI 只接受当前命令树和参数定义。

`project apply-copy`、`project apply-delete-directory` 和 `project apply-unity-migration` 接受原预演返回的 planId 和 `--idempotency-key`。软件包、复制与删除缺省使用原计划的项目修订，显式传入的 `--expected-revision` 由 Core 复验。交互式按 ID Apply 使用 `plans.get` 读回当前主体的原始计划并展示；非交互调用使用 `--yes` 确认已审阅相应预演，Core 在执行时复验计划。

项目复制、目录删除和 Unity 迁移使用 `--plan-idempotency-key` 创建计划。直接确认执行时另传 `--idempotency-key`，预演可省略执行键。回执未知时携原参数与原键重试。
