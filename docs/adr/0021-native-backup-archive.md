# ADR 0021：原生备份归档与一致性边界

状态：Accepted（Backup Create contract-first；生产实现尚未批准）

## 决定

M5 Backup Create 使用 `ALCOMD Backup Archive v1`。每个 `BackupId` 对应 ALCOMD 托管备份根中的
一个不可覆盖 ZIP 对象；ZIP 根只允许 `backup.json` 与 `project/`。Create 不是高影响 Plan/Apply：
`backups.create` 在接受请求时持久分配 `OperationId` 与 `BackupId`，并以幂等键复用两者。

Create 在完整读取窗口持有 `ResourceKey::Project(ProjectId)`，通过初始 revision/fingerprint、确定性
inventory、逐文件身份与流式证据、最终 tree/fingerprint 复验检测可观察变化。它不宣称提供文件系统
快照，也不能阻止任意外部 writer。Unity `running_confirmed` 时拒绝；`running_suspected` 与 `unknown`
只形成 advisory 并继续一致性检查。

归档只接受普通目录和单硬链接普通文件。symlink、junction/reparse point、多硬链接、socket、FIFO、
device、special/unknown type 一律 fail closed 为 `backup_source_unsafe`。ZIP 配额、精确排除表、VPM
package 排除语义与 manifest Schema 由 `specs/backups/` 的 v1 工件冻结。

## 持久化与恢复

Schema v6 只向 `operations.kind` 增加 `backups.create`，继续使用 Schema v4 已有 `backups` 表，不增加
backup plan 或通用 workflow 表。内部阶段固定为 `accepted`、`inventory_ready`、`archiving`、
`archive_ready`、`publish_intent`、`archive_published`、`state_committed`。

进入 `publish_intent` 前可以取消并清理或由 recovery 接管 partial；之后必须完成 publish/finalize 或
进入 `recovery_required`。publish 后恢复必须验证原 `BackupId` 对应 final 对象的文件身份、大小和
SHA-256，复用原 Operation/request/idempotency；证据不可信时不得产生虚假 succeeded。

## 后果与边界

- Create 只写托管备份根，不接受任意输出路径；内部 locator/path 不进入公开合同。
- `excludeVpmPackages=true` 只排除 normalized locked set 中经验证的 `Packages/<package-id>/`；不发网络
  请求，也不保证未来可下载。归档声明 `packagesRequireResolve=true`，未来恢复后由显式 resolve 处理。
- 本 ADR 不批准 Backup Restore、生产 Backup worker、第二套 archive/transaction framework、新依赖、
  新 unsafe 或平台 API。
