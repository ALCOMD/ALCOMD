# M8 MCP 协议与管理扩展

M8 已于 2026-11-20 收尾。当前首发合同与验收由[首发合同归一计划](M11-first-release-contracts.md)维护。

## 连接与业务权限

STDIO、回环 HTTP Bearer、HTTPS OAuth 连接均由 Core 认证。连接属于登记的 MCP 客户端。每次请求重验凭据或令牌、资源和撤销状态；闭环重放仍检查 Plan 与 Operation 的归属。GUI、CLI 和已认证 MCP 客户端使用相同的业务权限。预登记 HTTPS 使用单一连接 scope `mcp`，管理操作和授权端点按握手角色分派。凭据仅在签发时返回一次。

- [MCP 客户端 RPC](../../specs/rpc/m8-mcp-clients-v1.md)
- [OAuth RPC](../../specs/rpc/m8-mcp-oauth-v1.md)
- [回环 HTTP](../../specs/mcp/http-loopback-v1.md)与[HTTPS OAuth](../../specs/mcp/http-oauth-v1.md)

## 工具与管理界面

工具目录提供状态、项目、包、仓库、备份、操作与事件等已接入入口。调用端保留 Core 的默认参数，接受确认后应用同一计划，重试复用调用者、参数和幂等键。项目复制 Apply、备份恢复和仓库刷新使用公共原生 RPC；绝对或默认目录选择、摘要展示及避免把来源 URL 或敏感值暴露给模型由工具处理。工具目录订阅绑定目录快照，提供方启用、撤销或改动时由 Core 报告变化。

第一方管理扩展通过 Portable UI 和公开的管理、运行时及传输观察 WIT 工作。扩展宿主检查接口声明、实际调用者、扩展 grant、调用种类、租约与 UI 会话。观察报告限制大小，连接与请求提供可复核的身份绑定。签名的第三方工具提供者遵守相同沙箱、授权与取消规则。

- [工具目录](../../specs/mcp/toolset-v1.md)及[扩展提供方](../../specs/mcp/extension-tools-v1.md)
- [管理扩展](../../specs/extensions/mcp-management-v1.md)及[权限目录](../../specs/extensions/permissions-v1.md)
- [扩展签名来源](../../specs/extensions/source-v1.md)

### 桌面添加深链（2026-11-19，所有者批准独立工作树实施）

- `alcomd://repositories/add?url=…` 预填 VPM 仓库索引地址，用户发起预览后再确认登记。
- `alcomd://extensions/add?url=…` 预填 [单扩展源](../../specs/extensions/source-v1.md)索引地址，用户按「继续」后读取索引，沿用版本选择、包签名校验、条款与必需权限确认。
- 两条路由均接受单个百分号编码的匿名 HTTPS 索引地址，保留源地址的查询参数及顺序。打开链接仅预填表单。运行时接收复用冷启动参数、单实例转发和 macOS Open URL 事件。链接在内存排队，等已有弹窗或未保存编辑结束再打开。
- `vcc://vpm/addRepo` 沿用现有注册偏好及 `headers[]` 兼容行为，与新仓库深链共用添加面板。原始链接不进入日志或页面路由。Core RPC、Schema 和权限合同沿用现状。正式系统协议注册与安装生命周期验收归 [M12](M12-full-product-distribution.md)。
## 2026-11-20 M8 收尾

候选 `622cb62a` 的 Windows 和 Ubuntu 作业通过，两平台各运行 734 项 Chromium 测试。CodeQL 的四项分析通过。macOS arm64 因运行器容量不足在执行前取消，整体 CI 记录为失败。所有者将 macOS 补验延期。[CI 44566846494](https://github.com/ALCOMD/ALCOMD/actions/runs/44566846494) · [CodeQL 44566846484](https://github.com/ALCOMD/ALCOMD/actions/runs/44566846484)

管理扩展人工验收、普通在线扩展源、实际客户端目录变化通知、浏览器受信任 TLS 和持续传输满载仍待验证。Cursor 具名构建的握手、Host 激活预算和 Ubuntu Provider 的间歇性故障仍需复验。这些结果只适用于记录的候选。
