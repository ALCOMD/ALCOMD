# 本地客户端威胁模型

状态：Draft

## 资产

- Unity 项目和备份。
- 包、仓库与模板状态。
- 用户隐私设置。
- 外部客户端凭据。
- 更新信任链。
- 扩展权限。

## 威胁

- 同机恶意进程伪装官方客户端。
- 浏览器页面攻击 localhost API。
- DNS Rebinding。
- token 泄露或重放。
- 路径注入。
- 客户端篡改 revision。
- 日志泄露敏感信息。
- 未授权取消或审批任务。

## 最低缓解

- 用户限定 IPC ACL。
- 每客户端身份和权限。
- Loopback API 默认关闭。
- Host / Origin 校验。
- 独立可撤销凭据。
- 幂等键和 revision。
- 核心重新验证所有路径。
- 敏感日志脱敏。
- 审计写操作与授权变化。

## M8 transport observer and tools-list subscriptions (approved A/B)

The protected local reporting role is untrusted telemetry, not an authentication
shortcut. Core restricts it to report only, derives all public row IDs/times and
rejects supplied identity/body fields. Core authenticates every correlation through
a separate actual MCP credential/OAuth connection, consumes a random short-lived
256-bit ticket once, and stores only the ticket digest in memory. Tickets never
enter logs, event payloads, persistent idempotency or read projections. An HTTP
socket has no Principal owner. Report loss/failure does not retry or cancel business.

64 reporters / 4096 rows, per-channel cardinality/64-KiB/rate limits and bounded
adapter binding work bound memory and overhead. Expired/disconnected reports are
unavailable/incomplete, not an assertion of zero activity. A local user able to
report can fabricate activity, so all rows retain adapterReported source. Reports
never grant any permission or establish Plan/Operation ownership.

Tool subscriptions require the current authenticated Core catalogue, including
OAuth binding, grant revision and provider lifecycle. Pre/post-ack comparison closes
establishment races; failures terminate rather than publish an empty catalogue.
Four streams/auth context, 600-second lifetime, one pending invalidation and bounded
blocked writes prevent notification backlog. Normal HTTP limits and no-session
authentication remain. Cancelling a protocol stream does not cancel accepted Core
Operations. No session recovery, automatic business retry or credential logging.
