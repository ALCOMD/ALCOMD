# State29 · 仓库显示名

在 State28 的 `repositories` 表增加可空的 `display_name`。现有记录初始使用来源名称或内置名称。新增列只保存用户覆盖值，原索引、包、凭据和仓库来源保留。

设置和清除显示名沿用仓库修订与幂等记录。变更事务同时写入 `repository.display-name-changed` 事件。刷新仓库索引时保留显示名。
