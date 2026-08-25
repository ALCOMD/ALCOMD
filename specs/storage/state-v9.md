# ALCOMD State Schema v9 Portable UI proposal

状态：M7 Stop A proposal-only；不创建 production `0009` SQL，不改变当前 daemon 的 `dataSchema: 8`。

M6 已真实实现、广告并验收 Schema v8，因此同一个 v8 identity 不得代表两套结构。v9 只保存已验证 package declaration；
UI session、Snapshot、action、renderer 与 browser state全部只在内存。

## Exact additions

`extensions` 增加：

- `ui_protocol TEXT NULL`：NULL 或 exact `portable-v1`；
- `ui_surfaces_json TEXT NOT NULL DEFAULT '[]'`：canonical compact JSON，只允许 `[]` 或 `["main"]`；与 protocol
  联合约束为 `(NULL,[])` 或 `(portable-v1,[main])`。

`extension_plans` 增加相同的 immutable `ui_protocol` 与 `ui_surfaces_json` authority。install Plan 在 archive/Manifest
验证后固定两列，Apply/recovery 逐次复核；uninstall Plan 从 current extension row 固定它们。唯一 surface 不产生额外
table、identity 或 dynamic guest query。

不增加 UI session/Snapshot/action history/renderer registry/GUI host/browser/cache/workflow table。v9 migration proposal
可以用一个 `BEGIN IMMEDIATE` additive transaction；这只是 implementation convenience，不是公开数据库兼容承诺。
开发期旧数据库允许 reset。失败必须保持 v8 byte-equivalent authority，并在 migration 与 foreign key check完成前不广告
v9。

