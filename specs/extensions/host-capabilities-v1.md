# Extension Host Capabilities v1

状态：M6 contract-first Stop A candidate；仅 Project summary read 被选为第一条生产 slice。

## `host-projects.get-summary`

```text
guest Component
    -> alcomd:extension/host-projects@1.0.0
    -> extension Host channel bound to ExtensionInstanceLease
    -> alcomd-application existing project read use case
    -> extension-safe project-summary projection
```

Authority：

- permission 精确为 `projects.read`；scope 必须包含 request 中的 specific ProjectId。
- daemon/application 以 Host channel 绑定的 current lease 重新解析 ExtensionId、PrincipalId、grant revision、
  lifecycle generation 和 scope；忽略 guest 自报 authority metadata。
- grant revision 更新是 revoke linearization point。之后排队/未接受 call 返回 `lease-revoked` 或
  `permission-denied`；旧 session/handle 不可复用。
- Host 不返回项目绝对路径、filesystem identity、raw manifest、repository credential、process information 或日志。

Input/Output：

- input 只有 lowercase UUID ProjectId；canonical WIT message 不超过 256 KiB。
- output 是 `project-id`、bounded display name、`vpm|upm|unknown` kind、optional normalized Unity version 和 revision。
- project missing/inaccessible、scope denied、lease stale 和 internal error 保持 WIT stable code。
- call 使用 runtime wall timeout 2,000 ms、每 Host 单 guest call 与 runtime host-call rate limit。

第一方和第三方走完全相同的 WIT/interface/permission/scope/use case。扩展不得读取 `state.db` 或 Project filesystem。

## Planned、未实现能力

以下名称在 M6 Stop A 不进入 WIT world、不链接、不广告，未来每项需要独立 permission、Schema、scope、quota、
redaction、revocation 和人工审批：

```text
host.network.request
host.filesystem.*
host.clipboard.*
host.notification.send
host.external-config.*
host.discord-presence.*
```

raw socket、arbitrary filesystem、shell/process control 永久不由本合同暗示。
