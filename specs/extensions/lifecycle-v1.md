# Extension lifecycle, Host topology and lease v1

状态：M6 contract-first Stop A candidate；production 尚未实现。

## Durable desired state 与 runtime state

durable `desired_state`：`installed_disabled | enabled | uninstalling | quarantined`。

runtime `instance_state`：`stopped | starting | running | stopping | crashed`。

`enabled + crashed` 合法。`enabled` 不证明 OS process 存在。runtime row 还绑定 daemon epoch；daemon restart 时
旧 epoch 的 `starting|running|stopping` 先变为 `crashed`，绝不能产生 phantom running。

## Host topology

- 一个 enabled ExtensionId 对应一个独立 `alcomd-extension-host` OS process。
- 一个 Host 只装载该 ExtensionId；first-party 不共享 privileged Host，不存在跨扩展 pool。
- 第一条 slice 每 Host 最多一个 active Component instance。未来同扩展多 instance 仍受 per-extension quota。
- Host crash/hang/OOM 只影响该 ExtensionId；daemon、其他 Host 与唯一写入者不得一起退出。

内部 channel 使用 daemon-spawned child 的 dedicated piped stdin/stdout，不创建 public/listening endpoint。framing 是
u32 little-endian length + UTF-8 JSON，单 frame 524,288 bytes。daemon 首帧发送一次性 bootstrap nonce 与 lease，
Host ready 必须 echo nonce；nonce 只存在于进程内存与 private pipe，不持久化。guest 不继承 Host stdio。

## ExtensionInstanceLease

每次 start 都由 daemon 创建 lease，绑定：ExtensionId、opaque InstanceId、PrincipalId、current grant revision、
lifecycle generation、daemon epoch、expiresAt/cancelled。guest/Host input 不能决定这些值。

每次 capability/data call 以 pipe session + lease ID 查 current row，并复核 desired/runtime state、generation、grant
revision、expiry、permission 和 scope。grant revision durable update 是 revoke linearization point：此后新 call、Host
queue 中未被 application 接受的旧 call、session/handle 全部失败或取消。

已由 application 正式接受并获得 OperationId 的高影响 Operation 按 M2-M5 authority/recovery 继续；revoke/Host
termination 不自动回滚它，也不授予任何新的后续 capability authority。

## Restart/quarantine

- crash 后 1,000 ms 第一次 restart；第二次 crash 后 5,000 ms restart。
- rolling 300,000 ms window 内第 3 次 crash 把 desired state durable 改为 `quarantined`，runtime 为 `crashed`。
- crash window 从每次 Host abnormal exit 的 durable timestamp 计算；daemon restart 不清零。
- quarantine 只能由显式 enable/recovery action 在 package/signature/grant 重验后解除；不无限 auto-restart。

## Plan/Apply 与 recovery

Install Plan immutable 固定 source identity、ExtensionId/version/API、package/Manifest/component digest、publisher key/
trust decision、permissions/interfaces、archive profile v1、expected absence/revision。Apply 不重新 Plan。

Install phases：`accepted -> source_verified -> archive_verified -> staging_complete -> publish_intent ->
package_published -> state_commit_intent -> state_committed -> cleanup_complete`。

Uninstall Plan immutable 固定 ExtensionId/revision/package digest、`retain_data|delete_data`。默认 retain；delete 是显式
high-impact choice。Uninstall phases：`accepted -> lease_revoked -> host_stopped -> package_backup_intent ->
package_moved_to_backup -> data_delete_intent? -> data_deleted? -> state_commit_intent -> state_committed -> cleanup_complete`。

publish/move 后只有 identity/digest/evidence 全匹配才可 forward recover；否则 `extension_recovery_required`。
state evidence 完成前不得删除 staging/backup。重试复用原 PlanId/OperationId/idempotency，不重新 Plan、不虚假 succeeded。
