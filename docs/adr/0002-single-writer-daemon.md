# ADR: 每用户唯一核心与唯一写入者

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

`alcomd` 是每用户唯一核心进程，也是数据库和项目的唯一写入者。

M1 使用与进程生命周期绑定的 OS advisory lock/所有权机制取得每用户 daemon 唯一实例；某个
lock 文件存在本身不构成进程存活证据。该锁只负责 daemon 生命周期所有权，不是 M2 的业务
Resource Lock。

本地端点固定为：

```text
Windows: \\.\pipe\CQMHV.ALCOMD.<current-user-SID>.rpc-v1
Linux:  $XDG_RUNTIME_DIR/alcomd/rpc-v1.sock
macOS:  经过所有权检查且满足 sockaddr_un 长度限制的短 per-user runtime/temp 路径
```

Windows pipe ACL 明确限制当前用户，不授予 Everyone/Anonymous。Unix runtime directory 为
`0700`、socket 为 `0600`，不跟随不可信 symlink，并验证所有权。只有取得唯一实例锁后，才能
删除位于已验证 runtime directory、由当前用户拥有且经 `lstat` 确认为 socket 的 stale socket。

M1 的 `alcomd-cli system status` 在 endpoint not found/connection refused 时默认按需启动
sibling `alcomd`，使用总计不超过 5 秒的有界等待；其他错误不得被重启掩盖。
`--no-start-daemon` 可明确禁止启动。并发客户端可以竞争启动，但最终只有一个 daemon 获得
唯一实例，daemon 生命周期不依赖启动它的 CLI。

## 结果

GUI、CLI、MCP、API 和扩展只能通过 RPC 提交请求。M1 只实现 daemon 生命周期单实例与只读
状态切片；业务资源锁、事务与恢复仍属于 M2 或后续里程碑。
