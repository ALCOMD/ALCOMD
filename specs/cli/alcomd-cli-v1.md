# ALCOMD CLI contract v1

状态：M5 已发布命令面的生产实现与本地验收已完成（2026-08-24；等待最终 Hosted CI 与人工验收）

本合同只定义 `alcomd-cli` 的公共进程行为。CLI 永远是
`alcomd-cli -> alcomd-client -> RPC -> alcomd-application` 的适配器，不读取 `state.db`、项目、
repository、package cache 或备份文件，也不直接启动 Unity。命令只有在对应 daemon
capability 已真实实现时才可进入 help；合同中存在名称不等于后端已经可用。

`package install` 和 `package upgrade` 默认建议省略 `--version-range`，由 Core 推荐兼容版本。指定范围时，Core 在范围内选择。安装、升级和降级均可用 `--repository-id` 或 `--user-package-id` 指定一个来源。两个参数互斥。省略来源时由 Core 选择。

手动安装、升级或降级时，可用 `--allow-unity-incompatible` 为本次操作的目标包版本启用 Unity 兼容性例外。安装和升级须同时提供精确选择器，例如 `--version-range "=1.2.3"`。降级使用位置参数指定版本，例如 `1.2.3`。依赖包仍按项目的 Unity 版本检查。

`operation list`、`project list`、`repository list`、`repository packages` 可将上一页的 JSON `nextCursor` 原样传给 `--cursor`。任务列表的游标包含时间和任务 ID；`--active-only` 选择活动任务，续页时沿用相同筛选。

`settings get` 读取完整设置及修订。`settings update --expected-revision <revision> --update <JSON>` 使用同一份 Core 部分更新合同。JSON 只需包含要修改的字段，例如 `{"packages":{"showPrerelease":false}}`。修订冲突时重新读取设置，由调用者确认新的修改。回执不明时也可通过读取核实已保存的值。

权威 machine-readable 工件：

- `alcomd-cli-v1.schema.json`：machine output、stream record、退出码与全局选项。
- `command-catalog-v1.json`：已发布的命令和 alias。只列出已有真实 daemon capability 的入口。
- [ZIP 备份与恢复命令](backup-commands-v1.json)定义 `backup create` 和 `backup restore` 的参数、确认及任务回执。

## 输出模式

`client list [--cursor CURSOR] [--limit N]` 查询本机已安装或当前活动的原生程序；`client instances PROGRAM_KEY` 查询实例及连接，可接受相同分页参数。`client disconnect connection|instance ID --core-instance-id CORE_ID --idempotency-key KEY` 经确认后断开选定连接或实例。可重连，凭据仍有效。与 AI 身份登记相关的命令位于 `mcp client` 和 `mcp oauth`。

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

`project migrate-unity` 将受支持的 2022 Avatar 或 Worlds 迁移到 Unity `6000.0.67f1` 时，默认复制到同级目录 `<原工程名>-Unity6`。`--copy-name` 可指定副本目录名。`--in-place` 在原目录执行，确认前应先备份。`--dry-run` 在副本模式返回 `sourcePlan` 和 `copyPlan`。确认执行后，CLI 等待复制成功，读取副本项目 ID 并重新制定迁移计划。结果中的 `copy` 保存复制回执，`migration` 保存迁移结果或 `--no-wait` 请求的任务回执。跟踪已接受的复制或迁移任务时，CLI 遇到临时读取错误会重新连接，继续查询原任务 ID。若迁移失败，副本保留，可通过任务列表读取复制结果，在该副本上重新计划。按 ID 执行 `project apply-unity-migration` 时，Unity 6 路线需要 `--copy-operation-id` 对应已完成的副本，或用 `--in-place` 确认原地写入。内容经 Unity 6 上传后，再次更新同一内容需要 Unity 6。

## completion

`completion <shell>` 只根据静态 command tree 向 stdout 输出 script，不连接/启动 daemon，不访问
数据库、网络、repository 或项目。dynamic completion 后置。`clap_complete` 尚未获批，生产实现若
需要它必须先提交精确依赖审批。

## 首发合同

正式发布前 CLI 合同编号固定为 1。变更命令、参数、退出码或输出格式时，同步改写命令目录、调用方和测试。CLI 只接受当前命令树和参数定义。

`project apply-copy`、`project apply-delete-directory` 和 `project apply-unity-migration` 接受原预演返回的 planId 和 `--idempotency-key`。软件包、复制与删除缺省使用原计划的项目修订，显式传入的 `--expected-revision` 由 Core 复验。交互式按 ID Apply 使用 `plans.get` 读回当前主体的原始计划并展示；非交互调用使用 `--yes` 确认已审阅相应预演，Core 在执行时复验计划。

项目复制、目录删除和 Unity 迁移使用 `--plan-idempotency-key` 创建计划。直接确认执行时另传 `--idempotency-key`，预演可省略执行键。回执未知时携原参数与原键重试。
# Core desktop actions

`desktop focus-project-unity <PROJECT_ID> --expected-revision <REVISION> --idempotency-key <KEY>` focuses the project's running Unity editor on the Core machine. `desktop open-project-directory <PROJECT_ID> --idempotency-key <KEY>` opens the registered directory. `desktop open-unity-hub-download --idempotency-key <KEY>` opens the official Unity download page. Reuse the original key and arguments to recover a lost response. Each command reports the system outcome and rejects `--dry-run`.


## 扩展声明的 CLI 命令

`extension commands [--extension-id ID] [--cursor CURSOR] [--limit LIMIT]` 列出已启用扩展签名声明的命令。`extension run EXTENSION_ID COMMAND [--arguments JSON]` 在 Core 的扩展宿主内运行该扩展的 CLI 导出。未传参数时输入为空对象。Core 会再次检查实际扩展权限和调用者身份。

声明为 plan 的命令可用 `--dry-run` 获取真实计划。plan、apply、cancel 均需 `--idempotency-key`。apply 使用 `--plan-id` 及原计划修订，交互时展示实际计划，也可显式提交 `--expected-revision` 和 `--yes`。cancel 使用 `--operation-id`、`--expected-revision` 及确认。[声明 Schema](../extensions/cli-commands-v1.schema.json) · [原生 RPC](../rpc/extension-cli-commands-v1.schema.json)


`settings update`、`unity hub-open` 和 `repository export-list-file` 要求 `--idempotency-key`。同一次操作的回执丢失时，发送原参数和原键。Hub 使用 `--expected-revision` 及设置中观察到的 `--expected-target-token`，目标为 null 时省略 token。仓库导出传入目标路径、`--expected-fingerprint`，默认原生格式，显式 `--format vcc-txt` 使用 VCC 交换格式。
