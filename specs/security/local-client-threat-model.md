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
- MCP 每客户端身份和权限，扩展每实例实际授权。
- Loopback API 默认关闭。
- Host / Origin 校验。
- MCP 客户端的独立可撤销凭据。
- 幂等键和 revision。
- 核心重新验证所有路径。
- 敏感日志脱敏。
- 审计写操作与授权变化。

## M10 原生桌面客户端

| 边界 | 受保护能力 | 控制 |
| --- | --- | --- |
| Windows 系统账号 | 监听与连接 Core 的本机命名管道 | 当前用户 SID 限定的 DACL，拒绝远程连接。客户端以识别级别打开，核验已连接的服务端 SID 与 DACL |
| Linux 与 macOS 系统账号 | 连接当前账号的 Core socket | 核验私有运行目录、无链接的 socket、类型、所有权、权限及已连接对端 UID |
| 安装登记 | 定位用户的已初始化 Core | 从安全的本机安装登记按产品身份解析可执行文件。端点安全检查失败、登记无效或存在链接时拒绝启动 |
| SDK helper | 把 Node 请求送入同一原生 RPC | 私有 stdio 限长分帧、方法清单、响应关联及结构化错误。安装包提供对应平台的 helper |
| Core | 项目、仓库、状态及安装配置的业务写入 | 同一应用用例、显式确认、Plan/Apply、revision、幂等与目标校验 |

`builtin:local-owner` 是能访问该账号原生 IPC 的进程的共同权限。官方和第三方 GUI
均可使用，客户端名称和实例 ID 只用于诊断。任意代码若已在同一系统账号中运行，
可访问该账号的原生业务权限。单应用撤销、同账号恶意代码隔离和通过自报应用名阻挡
本地请求均超出该入口的保护范围。

MCP 连接仍需按其合同认证并复核当前凭据，失败后终止，不回落到本机所有者。
扩展仍须通过沙箱和权限检查。Node SDK 的本机连接不能暴露给不受信任的 WebView、
插件、页面脚本或网络调用者。跨账号和伪造端点属于拒绝场景，SDK 传输失败不会把
已发送写请求自动重放。公开错误仅暴露批准的结构化字段。

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

## M11 文件操作与数据基线

备份、恢复以及 MCP 项目工作流的绝对路径均在 Core 主机上解析。请求权限来自连接身份与现行业务权限，路径文本不赋予权限。Core 冻结源和目标父目录的文件身份，执行配额、取消、独占同卷发布和持久恢复。解压仅接受根部构成 Unity 工程的 ZIP。恢复目标在任务提交前保留给该任务，后台项目发现遵守同一保留。

新运行时在打开 WAL 和恢复持久任务前预检现行 State46 和 Config13。未知文件、旧版本和不受信任的目录在写入前拒绝。已验证的安装登记可支撑原生 SDK 的首次连接。
