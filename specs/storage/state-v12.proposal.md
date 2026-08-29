# State Schema 12 proposal

状态：M7 P6 已获项目所有者批准，等待 production migration与完整wiring。

State v12只允许：重建`package_plans`以增加`reinstall|bulk` action；为
`repository_package_versions`增加nullable sanitized docs/changelog URL；新增单一专用
`user_package_sources`表。Operation kind保持`packages.apply`，不增加ResourceKey或generic source/workflow state。

User Package是一个user-selected loose package root directory。State只持久化其canonical UUID、owner、root path、opaque
filesystem identity、package identity/version、normalized manifest、manifest/content fingerprints、ALCOMD-owned deterministic
archive digest、revision和timestamps。它不伪装repository，也不持久化fake URL。

Migration必须`BEGIN IMMEDIATE`，完整保留State v11 authority与event sequence，`foreign_key_check`为空，最后才设置
`user_version=12`；失败精确回滚到v11。精确机器合同见`state-v12-migration.proposal.contract.json`。
