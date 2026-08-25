# settings.toml Config Schema 1

状态：M7 active production contract。

`config/settings.toml` 是官方用户设置的可读持久化文件。daemon 是唯一 writer；GUI、CLI、扩展和
其他客户端只能通过 `settings.get` / `settings.update` 访问。该文件不属于 `state.db` State Schema，
不得保存项目、仓库、扩展授权、Operation、Activity、credential 或 extension-owned data。

## 规范化文件

```toml
schema = 1
revision = 1
locale = "system"

[appearance]
mode = "system"
density = "default"
motion = "system"
```

`appearance.source_color` 仅在非空时出现，并使用 canonical uppercase `#RRGGBB`。

## 读取和更新

- 文件是有界（最大 16 KiB）的 UTF-8；NUL、无效 UTF-8、重复键、重复 section、未知字段、未知
  section、错误类型或不规范枚举全部 fail closed。
- `schema` 固定为 `1`；`revision` 是 checked monotonic positive `u64`，更新时要求精确
  `expectedRevision`，溢出 fail closed。
- 缺少文件表示 revision 1 的规范默认值，而不是损坏；存在但无效的文件不得静默恢复默认值。
- `settings.update` 是 closed partial update。省略的字段保持当前值；显式 `sourceColor: null` 清除
  source color。
- 写入顺序固定为 `schema`、`revision`、`locale`、空行、`[appearance]`、`mode`、可选
  `source_color`、`density`、`motion`，末尾一个换行。
- 更新先在同目录创建 exclusive temporary file，完整写入并 `sync_all`，再使用既有的
  target/backup/rename/recovery 模式替换。启动读取会先完成该窄替换协议的恢复；不存在同时可信的
  target/backup 时 fail closed。
- 文件和目录不得包含 token、Authorization、credential、私钥、路径或任意 extension setting。

公开 DTO 与边界由 `settings-v1.schema.json` 和
`specs/rpc/m7-official-gui.schema.json` 冻结。
