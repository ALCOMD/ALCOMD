# `.alcomdext` v1 content identity, publisher and signature

状态：M6 contract-first Stop A candidate；生产签名验证尚未实现。

## 四层模型

1. package content identity：`alcomd-extension-content-sha256-v1` digest；
2. publisher cryptographic identity：Ed25519 public key 的稳定 SHA-256 fingerprint；
3. publisher trust policy：daemon/local policy 的 `official` 或 `user_approved_for_extension` 判断；
4. first-party policy：固定 official publisher fingerprint 与已批准发行来源同时成立。

Manifest 的 publisher display name、`first_party` 或任意自报 metadata 都不能改变 2-4。第一方和第三方使用
同一签名格式、WIT、Host、permission、scope、revocation 和 runtime quota。

## Canonical content digest

签名 envelope `META-INF/alcomd-signature-v1.json` 是唯一从 digest 输入排除的 entry。其余 regular file 先通过
package hostile-path profile，再按 normalized UTF-8 path bytewise ascending 排序。digest 输入精确为：

```text
ASCII "ALCOMD-EXT-CONTENT-SHA256-V1\0"
for each entry:
    u32-le(path UTF-8 byte length)
    path UTF-8 bytes
    u64-le(uncompressed content byte length)
    exact uncompressed content bytes
```

目录不进入 digest；空文件进入。ZIP entry order、timestamp、permission bits、compression level 和 container bytes
不影响 identity。Manifest digest 是 `alcomd-extension.toml` exact raw bytes 的普通 SHA-256，另行固定在 Plan 中。

## Publisher fingerprint 与签名

- public key 是 32-byte Ed25519 verifying key，wire encoding 为 64 lowercase hex characters。
- fingerprint 是 `ed25519-sha256:` + `SHA-256(raw public key)` 的 64 lowercase hex。
- signed message 是 ASCII `ALCOMD-EXT-SIGNATURE-V1\0` 后接 package digest 的 32 raw bytes。
- signature 是 RFC 8032 Ed25519 detached signature，64 raw bytes，编码为 128 lowercase hex。
- verifier 必须执行 strict verification；key、fingerprint、Manifest fingerprint 和 envelope fingerprint 必须一致。

## Trust 与 unknown/self-signed publisher

- official：fingerprint 命中 ALCOMD 固定官方 trust 且 package 来源通过已批准官方来源策略；仅命中其一不够。
- unknown/self-signed：允许 Plan，但默认返回 `extension_publisher_confirmation_required`。只有显式 high-impact
  confirmation 产生的 immutable install Plan 才可记录 `user_approved_for_extension`；approval 只绑定
  ExtensionId + fingerprint + package digest，不建立全局 CA 或跨扩展信任。
- Apply 重验签名、digest、source identity 与 trust decision。任一变化返回 `extension_plan_stale`。
- uninstall 删除该 installed-extension 绑定的 user approval；重装必须重新确认。official trust 由产品 policy 持有。
- 长期 private/signing key 不进入 ALCOMD state；package、data、log、Event 不保存 bearer credential。
