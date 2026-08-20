# ADR 0018：M5 CLI 公共进程合同

状态：Accepted（2026-08-20，contract-first；生产实现尚未获批）

## 决策

M5 的 CLI 采用 `human` 默认输出与互斥的 `--json`/`--ndjson` machine mode。最终结果、日志、
进度、诊断与错误严格按 `specs/cli/alcomd-cli-v1.md` 路由；退出码只使用
`0/1/2/3/130`，机器错误分类来自稳定 `error.code`。

批准的通用控制项为 `--quiet`、`--yes`、`--dry-run` 与 `--no-wait`：

- quiet 只抑制非错误 human progress/diagnostic。
- yes 只代替确认，不能越过 permission/revision/writer/stale/security gate。
- dry-run 可以写 durable Plan，但绝不 Apply 或产生外部 filesystem/business mutation。
- no-wait 在 Operation 接受后返回 OperationId；默认 mutation follow 到终态。

非 TTY、关闭 stdin 和 machine mode 不进入 prompt。Ctrl-C 在 Operation 创建前不产生 mutation；
OperationId 已产生后只 detach，不取消 daemon Operation。completion 完全由静态 command tree 生成，
不连接 daemon 或业务存储。

## 边界

CLI 只依赖 `alcomd-client`/RPC，不直接访问 state、项目、repository、cache、template/backup 或 Unity
executable。命令只有在 backend capability 真实实现时才进入 help。M5 contract-first 先冻结行为和
名称，不把现有 CLI 生产实现描述为已经满足该合同。

`clap_complete` 尚未获批。若静态 completion 不能由现有 clap 能力直接完成，必须先提交精确版本、
features、许可证、MSRV、feature graph 与 Cargo.lock diff。

## 结果

这套边界使脚本可以稳定依赖 stdout/stderr、machine envelope 和退出码，也使客户端中断与持久
Operation 生命周期解耦。代价是命令实现必须通过 subprocess golden 覆盖，且没有真实 backend 的
产品清单入口必须保持隐藏或明确 capability unavailable，不能由 CLI 自己补业务逻辑。
