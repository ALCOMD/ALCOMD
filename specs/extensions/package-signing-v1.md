# `.alcomdextension` v1 content identity, publisher and signature

状态：当前有效合同；严格签名验证已实现。

## 内容、来源身份与安装选择

1. package content identity：`alcomd-extension-content-sha256-v1` digest；
2. publisher cryptographic identity：Ed25519 public key 的稳定 SHA-256 fingerprint；
3. installation choice：用户选择有效的本地包或单扩展源版本并请求安装，无独立发布者批准或信任设置；
4. first-party source policy：仅由受控发行来源决定，不接受调用者自报的官方身份。

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

## 用户选择安装与签名的边界

- 未知或自行签名的发布者不需要额外确认；签名、摘要、指纹与包格式有效即可进入原安装流程。不存在全局或逐扩展的“可信发布者”设置，也不自动填写已批准字段。
- 签名只证明内容完整性和密钥身份，不证明扩展安全或发布者经过认证。Manifest display name 不能冒充受控来源。
- Plan/Apply 仍冻结和复查 ExtensionId、签名/摘要、实际来源身份、版本与权限声明；包被替换时拒绝，不静默重新规划。
- Manifest 申请权限不等于实际授权。安装仅在用户明确同意冻结 Plan 的必需权限后，于同一状态提交事务授予这些权限；可选及未声明能力默认不授予。原必需权限门禁和沙箱保持。
- 留存数据仍绑定 ExtensionId 和发布者指纹，另一发布者不能继承该数据。删除发布者确认不等于删除密码学身份隔离。

- 长期 private/signing key 不进入 ALCOMD state；package、data、log、Event 不保存 bearer credential。

## M6 install source

当前接受 owner-selected local package（`local_owner_selected`）及已批准的单扩展源（`remote_extension_source`），受控 first-party 来源不接受调用者自报。在线清单只提供一个扩展的多个版本与既有 `.alcomdextension` 下载地址，不提供市场、仓库管理或自动更新。匿名 HTTPS、快照绑定、摘要/身份验证及限额见 [source-v1](source-v1.md)。下载完成后仍走本合同的签名、内容树和包校验，清单不代替包内权限或作者条款。
