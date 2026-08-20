# ADR 0020：ALCOMD Template Bundle v1 与全新项目创建事务

状态：Accepted（2026-08-21，contract-first；Template 生产实现尚未获批）

## 决策

M5 定义 ALCOMD v4 自有的 `ALCOMD Template Bundle v1`，文件扩展名为
`.alcomdtemplate`。它不是 v3 `.alcomtemplate` v1/v2，也不提供兼容导入。真实 v3 模板
差异测试和 tar/gzip 输入继续 blocked 到 M11。

bundle 使用 M4 已锁定的 ZIP stack，只允许 Stored 与 Deflate，并复用同一 UTF-8、Unicode NFC、
路径、碰撞、链接/special-file 和 bounded streaming 安全 engine。根布局只能是：

```text
template.json
payload/
resources/
```

`template.json` 是唯一根 manifest；`payload/` 是项目模板内容；`resources/` 中每个文件都必须被
manifest 显式声明。其他根 entry、未声明 resource、link/reparse/hardlink、special file、路径逃逸、
大小写或 Unicode collision 均使整个 bundle 失败。格式和精确 quota 由
`specs/templates/template-bundle-v1.md` 与 JSON Schema 冻结。

## Registry、身份与来源

TemplateId 是唯一业务身份；displayName、文件名和来源路径都不是 identity。State Schema v4 的
`templates.source_kind` 继续只有 `builtin | user`：imported、derived、authored 是 manifest provenance，
不是新的 registry ownership 类别。不得修改 `0004_local_workflows.sql` 或增加依赖/resource/history 表。

用户 bundle 使用 `sha256:<64-lower-hex>` opaque locator；builtin 使用
`builtin:<stable-id>@<version>`。RPC 不暴露 object-store 路径。`payload_sha256` 是完整 bundle/object
bytes 的权威 SHA-256，每次 import/create/export 前重新验证。manifest 中的 payload tree digest 和
resource digest 是额外的内容校验，不替代 object digest。

builtin TemplateId 不可变，只允许读取、favorite 和 create project。升级可以在同一 TemplateId 下发布
新 templateVersion，但不得 overwrite/remove/import replacement。user import 以 TemplateId 和完整
bundle digest 决定：新 ID 可 Plan；同 ID 同 digest 为 no-op；同 ID 不同 digest 为 conflict；builtin ID
永远 immutable。显式 override 必须固定 old/new digest、revision、Plan 与 idempotency，Apply 不得改选
另一 bundle。

derived template 是 self-contained 新 bundle，具有新 TemplateId 和完整 payload；provenance 可记录来源
TemplateId/ProjectId，但 create project 不依赖 base 仍存在，也不建立继承 DAG。

## Derive、Import 与 Export

derive from Project 是 Operation：writer gate -> Project lock -> initial fingerprint -> bounded traversal ->
archive/object staging -> final fingerprint -> publish -> registry commit。`running_confirmed` hard reject；
suspected/unknown 只 advisory，但前后 fingerprint 不一致必须返回
`project_changed_during_template_create`，不得发布混合 snapshot。遍历不 follow symlink/reparse/special file，
include/exclude policy 由 bundle 规范逐项冻结。

`templates.inspectBundle` 纯只读；`planImport` 完成 ZIP preflight、manifest/digest/conflict/dependency/resource
验证并返回 immutable Plan；`applyImport` 复验相同 source identity/digest 后才 publish object 和短事务写
registry。import 不写 Unity project，不使用 Project writer gate。

export 只从已验证 object 读取，target 必须不存在且 create-new，禁止 overwrite/merge。对同一已存 bundle
可直接复验 digest 后 byte-for-byte copy；格式层只承诺 semantic determinism，不承诺不同 writer 重新
生成 ZIP 时 byte-stable。输出不得含 locator、owner、state.db metadata、credential 或原始绝对路径。

## Create Project Plan/Apply

`templates.planCreateProject` 固定 TemplateId/revision、bundle/tree/manifest digest、validated parent
filesystem identity、normalized target leaf、target 不存在、Unity compatibility、M4 exact resolved package
graph/source revision/digest、resource digest 与最终项目摘要。Plan 不写目录、不刷新 repository；catalog
不是 resolver-ready 时返回 `repository_refresh_required`，M4 SHA-256-required 子集无法满足时 fail closed。

目标尚不存在，因此 Resource Key 是 `ProjectCreate(parent_identity, target_leaf)`，不得伪造目标 identity。
Apply 只允许在新目录创建，禁止 overwrite、merge、delete-then-create、source 内嵌 target 与 path escape。
它返回 OperationId，按以下固定顺序执行：

1. 复验 permission、Plan、parent identity、target absent、template revision/digest 和 package pins；
2. 取得 ProjectCreate lock，并在同 volume sibling staging materialize payload/resources/package ChangeSet；
3. 验证完整 Unity project，写 durable intent/evidence，再 atomic publish target；
4. 注册唯一 ProjectId，在短 SQLite transaction 提交 Event/revision/idempotency/Operation；
5. state commit 后才报告 succeeded。

package dependency 只能复用 M4 resolver、cache、archive 和 project mutation primitive，不创建 nested child
Operation 或第二 package engine。若现有 M4 primitive 无法安全作用于 validated staging project，生产实现
前必须另提一个窄 internal adapter 审批。

create-project recovery 复用原 OperationId、Plan 与 idempotency，至少覆盖 prepared、staging complete、
publish 前/后、Project registry commit 前、state committed 后 response 前的真实 kill/restart。最终只能是
目录不存在，或完整且已注册的新 Project；不得重新 resolve、切 source、创建第二 ProjectId 或留下永久
filesystem/DB 分裂。

## RPC、权限与实施边界

RPC v1 兼容冻结 `templates.read.v1`、`templates.manage.v1`、`templates.create-project.v1` 及
`specs/rpc/m5-template.schema.json` 的方法/DTO。生产 adapter 不存在前，daemon 不广告 capability，CLI
planned commands 也不进入实际 catalog/help。

query/inspect/export 至少需要 `templates.read`；import/derive/favorite/remove 需要 `templates.manage`；derive
另需目标 Project 的 `projects.read` scope；create Plan 需要 `templates.read + projects.create`，含依赖时还
需 `packages.read + repositories.read`；Apply materialize package 还需既有 `packages.manage` authority，
不能由 `templates.manage` 隐式获得。所有外部 filesystem write 仍只允许 `builtin:local-owner`。

本 ADR 只批准合同、Schema、inventory、synthetic Fixture 与测试快照。它不批准 registry/application
use case、import/export adapter、derive、create-project transaction、新 production dependency、unsafe 或
平台 API。
