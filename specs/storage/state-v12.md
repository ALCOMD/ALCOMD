# State Schema 12

状态：M7 P6 production migration 与 repository package metadata wiring 已实现；User Package 与
plan.v2 application wiring 在同一 P6 checkpoint 内继续完成。daemon 广告 `dataSchema: 12`。

权威 migration 是 `crates/alcomd-store/migrations/0012_package_functional_closure.sql`。State v12 只：

- 重建 `package_plans`，在既有 action 上增加 `reinstall` 与 `bulk`；
- 为 `repository_package_versions` 增加 nullable `documentation_url` / `changelog_url`；
- 新增专用 `user_package_sources` 表。

repository link 只在 Core 使用 `reqwest::Url` 完成 closed http/https、host、userinfo 与 2048-byte
检查后持久化；它不是 resolver authority，缺失或无效 link 不影响 resolver-ready package。

User Package 是 user-selected loose package root directory，不伪装 Repository，不持久化 fake URL。
其表只保存 owner-scoped identity、规范 manifest/content fingerprints、ALCOMD-owned deterministic archive
digest、revision 与 timestamps；remove 不删除用户目录或 cache object。

migration 使用单一 `BEGIN IMMEDIATE` transaction，完整保留 v11 authority、Event sequence、Operation/
journal/idempotency state 与 foreign keys，最后才设置 `user_version=12`。失败完整回滚到 v11；future schema
继续 fail closed。精确机器合同位于 `state-v12-migration.proposal.contract.json`。
