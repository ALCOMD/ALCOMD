# Extension-owned Data v1

状态：M6 Stop A review closure；production 尚未实现。

每个 `(ExtensionId, publisher cryptographic fingerprint)` 只有一个 daemon-owned bounded key/value namespace。value 是 opaque bytes；没有 blob store、
streaming object、list/prefix scan、shared namespace 或 filesystem-backed virtual disk。

lease 同时绑定 ExtensionId 与已验证 publisher fingerprint。重装只有相同 pair 才能 attach retained namespace；如果
ExtensionId 相同但 publisher fingerprint 不同，旧 namespace 不可见且 fail closed 为
`extension_data_owner_mismatch`。M6 不实现 publisher ownership transfer 或 key rotation。

## Exact API

- `get(key)`：存在时返回 bytes、key revision、namespace revision；缺失返回 `none`，不是跨 namespace probe。
- `set(key, value, expected-key-revision)`：`none` 只允许 create；`some(n)` 必须 exact match current key revision。
- `delete(key, expected-key-revision)`：必须 exact match；不存在返回 `data-not-found`。
- 每次成功 mutation 在单个 SQLite transaction 内原子校验 quota，key revision 与 namespace revision 均 +1。
- guest 不能选择 ExtensionId/publisher；namespace owner 来自 current `ExtensionInstanceLease` 和安装 registry。

## Exact quotas

| limit | value |
|---|---:|
| namespace total value bytes | 4,194,304 |
| keys per ExtensionId | 1,024 |
| key UTF-8 bytes | 1-128 |
| value bytes per key | 65,536 |
| mutations per transaction | 1 |
| concurrent data calls per Host | 1 |

key 必须是 lowercase ASCII，匹配 `^[a-z0-9][a-z0-9._/-]{0,127}$`；它不是 filesystem path，`//`、`/./`、
`/../` 和 trailing `/` 均拒绝以避免未来语义歧义。quota 计算只计 value bytes，key count 单独限制；SQLite overhead
不暴露为 guest 可操纵 quota。

disable 保留数据且 guest 无法访问。uninstall 默认保留；namespace/data 不得通过 registry FK cascade 被删除。
`delete_data=true` 只能出现在明确 high-impact immutable uninstall Plan，并在 package removal 后、state commit 前按
recovery journal 执行。任何 uninstall 都撤销全部 grant；未来 reinstall 不恢复旧 authority，始终 deny-by-default。
