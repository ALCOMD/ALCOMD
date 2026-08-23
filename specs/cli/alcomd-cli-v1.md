# ALCOMD CLI contract v1

状态：M5 已发布命令面的生产实现与本地验收已完成（2026-08-24；等待最终 Hosted CI 与人工验收）

本合同只定义 `alcomd-cli` 的公共进程行为。CLI 永远是
`alcomd-cli -> alcomd-client -> RPC -> alcomd-application` 的适配器，不读取 `state.db`、项目、
repository、package cache、template/backup 文件，也不直接启动 Unity。命令只有在对应 daemon
capability 已真实实现时才可进入 help；合同中存在名称不等于后端已经可用。

权威 machine-readable 工件：

- `alcomd-cli-v1.schema.json`：machine output、stream record、退出码与全局选项。
- `command-catalog-v1.json`：M5 已发布命令和 alias；只列出已有真实 daemon capability 的入口。
- `m5-template-commands-v1.json`：已发布的 Template 命令合同；对应 daemon capability、RPC 与生产
  use case 已真实存在，并以兼容增加方式进入 help/catalog。
- `m5-backup-commands-v1.json`：已发布的 Backup list/get/create/restore 命令合同。

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

## completion

`completion <shell>` 只根据静态 command tree 向 stdout 输出 script，不连接/启动 daemon，不访问
数据库、网络、repository 或项目。dynamic completion 后置。`clap_complete` 尚未获批，生产实现若
需要它必须先提交精确依赖审批。

## 兼容规则

新增命令、alias、可选 machine 字段或 NDJSON record type 是版本内兼容增加，但只有真实 backend
存在后才能发布。删除/重命名命令或 alias、改变退出码、stdout/stderr 路由、既有字段类型或语义是
破坏性变化，必须提升 CLI contract version 或提供经过批准的兼容 alias。
