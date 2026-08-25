# ALCOMD State Schema v9 Portable UI

状态：M7 production migration 与 Portable UI B1 wiring 已完成；daemon 广告 `dataSchema: 9`。

M6 已真实实现、广告并验收 Schema v8，因此同一个 v8 identity 不得代表两套结构。v9 migration 位于
`crates/alcomd-store/migrations/0009_portable_extension_ui.sql`。完整 production wiring 完成前 `system.hello` 继续广告 v8。
v9 只保存已验证 package declaration；UI session、Snapshot、action、replay evidence、
draft、renderer 与 browser state全部只在内存。

## Exact additions

`extensions` 只增加：

- `ui_protocol TEXT NULL`：合法值严格为 NULL 或 `portable-v1`。

`extension_plans` 只增加 immutable `ui_protocol TEXT NULL`，合法值相同。install Plan 在 archive/Manifest验证后固定该列，
Apply/recovery逐次复核；uninstall Plan从 current extension row固定它。单一隐式页面不产生页面 identity、JSON常量或
dynamic guest query。

不增加 UI session/Snapshot/replay/action history/draft/renderer registry/GUI host/browser/cache/workflow table。v8 -> v9
migration必须在一个 `BEGIN IMMEDIATE` transaction中完成，保留全部 v8 rows、foreign keys、revision与Event sequence；
existing extension默认 `ui_protocol=NULL`。migration或foreign-key check失败必须回滚为原 v8 authority，future schema
必须 fail closed。这不是旧 Web UI compatibility migration，只是已广告 Data Schema 的正常版本推进。
