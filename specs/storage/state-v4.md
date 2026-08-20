# ALCOMD State Schema v4

状态：Implemented（M5 Unity 最小生产切片已接入；Template/Backup 仍仅冻结表结构）

权威 migration 是 `crates/alcomd-store/migrations/0004_local_workflows.sql`。daemon 已在启动事务中
从 v3 自动迁移到 v4，并在 store ready 后广告 `dataSchema: 4`；本阶段只接入 Unity registry、project
Editor preference 与 launch 的既有持久幂等记录。Template/Backup 表仍不得被描述为生产功能。

## 范围

v4 只增加 M5 已有持久需求：

- `unity_installations`
- `project_editor_preferences`
- `templates`
- `backups`

不增加通用 settings/property/value、Hub mirror、process history、CLI view state、workflow 或未来 GUI
表。0001/0002/0003 不修改。

## Unity

`unity_installations` 保存 owner、validated executable path、opaque filesystem identity、version、
architecture、source kind、revision 与 observed/updated timestamp。filesystem identity 唯一；无法取得
identity 时 fail closed，不回退到 path string。

`project_editor_preferences` 以 ProjectId 为主键，引用一个 installation，并保存 JSON argv array。
`expectedRevision=0` 表示调用方要求 row 尚不存在；正整数必须精确匹配现有 revision。应用层还必须
验证最多 64 项、单项最多 4,096 UTF-8 bytes、总 JSON 最多 65,536 bytes，并拒绝 `-projectPath` 及
等价 selector。删除仍被 preference 引用的 installation 必须明确失败或先显式清除 preference。

Unity installation 与 project preference 是有 revision/Event 语义的 aggregate；v4 只为它们及
template 增加 `events.aggregate_kind`。migration 必须 byte/row-equivalent 保留既有 Event sequence、
index 与 AUTOINCREMENT 状态。

## Template 与 Backup 结构边界

v4 预留且只预留已批准的最小 registry 字段。Template 的 archive/quota/import/create-project RPC
合同仍须在 M5 Template slice 冻结；Backup exclusion/quota/create/restore RPC 合同仍须在对应 slice
冻结。表存在不等于这些业务已实现。

- `templates`：stable TemplateId、owner、`builtin | user` source、versioned bounded manifest、payload
  locator/SHA-256、favorite、revision 与 timestamp。`imported | derived | authored` 只属于 manifest
  provenance，不扩展 source_kind。M5 derived template 是 self-contained，因此没有 base cycle/depth 或
  inheritance graph。
- `backups`：BackupId、允许成为历史引用的 optional source ProjectId、archive locator、file identity、
  SHA-256、size、format version、createdAt、compression mode 与 exclude-VPM flag。它是 immutable
  artifact metadata，不强制建立 aggregate/Event state machine。

## Migration 不变量

- `BEGIN IMMEDIATE` 内原子执行，最后设置 `user_version=4`；失败完整回滚到 v3。
- 完整保留 M2-M4 Operation、Plan、package filesystem journal、Project/Repository revision、Event
  sequence、永久 idempotency 与全部 foreign key。
- v4 不改变既有 table/column/trigger 语义，也不改变 M1-M4 method。
- future schema 继续由 daemon fail closed；`CURRENT_DATA_SCHEMA` 固定为 4。

## Template Bundle v1 兼容断言

Template contract 适配既有表，不修改 migration：

- `manifest_json` 保存 `template-bundle-v1.schema.json` 的 bounded normalized object，仍受 1 MiB DB 上限；
- user/imported/derived object locator 固定 `sha256:<64-lower-hex>`；builtin locator 固定
  `builtin:<lowercase-uuid>@<template-version>`；两者均为内部 opaque locator；
- `payload_sha256` 是完整 `.alcomdtemplate` object bytes 的 32-byte SHA-256；RPC 不返回 locator/path；
- TemplateId 是 identity，displayName/file name 不参与唯一约束；同名不同 ID 可以共存；
- builtin immutable、user conflict/no-op/override、locator grammar、manifest bounds 和 bundle digest
  revalidation 是 application 不变量，不能通过增加关系表或修改 0004 规避。

现有字段足以表达 M5 Template v1 必需不变量，因此本 contract-first slice 不增加 Schema v5、不改
`0004_local_workflows.sql`，也不宣称 Template registry 已有生产 use case。
