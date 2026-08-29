# settings.toml Config Schema 2 proposal

状态：M7 P6 已获项目所有者批准，production wiring 已由 active `settings-v2.*` 合同实现。

Config Schema 2 保持 daemon 唯一 writer、strict UTF-8、16 KiB 上限、checked revision CAS 与既有
same-directory `.new`/`.bak` 原子替换和恢复纪律。它只在 Config Schema 1 上增加 package presentation设置：

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

`source_color` 仍仅在非空时出现。`hidden_repository_ids` 最多256个canonical lowercase UUID，必须unique并按
lexical byte order写出；重复输入是`invalid_request`，不是静默去重。最坏的256项canonical serialization与其他
最大合法v2字段仍小于现有16 KiB上限，因此不修改`MAX_SETTINGS_BYTES`。

v1 -> v2只在startup ready前执行，保留revision并补三个默认值。迁移失败恢复合法v1 authority且Config subsystem
fail closed；它不影响独立的State Schema version。只有成功的用户`settings.update`令revision加一。

`hidden_repository_ids`和`hide_local_user_packages`只影响official GUI presentation/source chooser，不修改resolver
catalog、refresh、dependency authority、Plan、pin、Apply、recovery、enrollment或cache。

精确机器合同见`settings-v2.proposal.schema.json`。
