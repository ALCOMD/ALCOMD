# settings.toml Config Schema 2

状态：M7 active production contract。

Config Schema 2 保持 daemon 唯一 writer、strict UTF-8、16 KiB 上限、checked revision CAS 与既有
same-directory `.new`/`.bak` 原子替换和恢复纪律，并增加 package presentation 设置：

```toml
schema = 2
revision = 1
locale = "system"

[appearance]
mode = "system"
density = "default"
motion = "system"

[packages]
show_prerelease = false
hidden_repository_ids = []
hide_local_user_packages = false
```

`source_color` 仍仅在非空时出现。`hidden_repository_ids` 最多 256 个 canonical lowercase UUID，必须
unique 并按 lexical byte order 写出；重复输入是 `invalid_request`，不是静默去重。最大合法 v2
序列化仍低于既有 16 KiB 上限。

v1 -> v2 在 daemon ready 前执行，保留 revision 并补三个默认值。迁移失败恢复合法 v1 authority，
Config subsystem fail closed；只有成功的用户 `settings.update` 令 revision 加一。

`hidden_repository_ids` 与 `hide_local_user_packages` 只影响 official GUI presentation/source chooser，
不修改 resolver catalog、refresh、dependency authority、Plan、pin、Apply、recovery、enrollment 或 cache。

公开 DTO 与边界由 `settings-v2.schema.json` 和
`specs/rpc/m7-official-gui.schema.json` 冻结。Schema 1 仅作为受支持的启动迁移输入保留。
