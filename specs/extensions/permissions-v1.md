# Extension Permissions v1

状态：M7 active permission contract；M6 runtime 权限与 M7 Portable UI client permission 已接入 production。

## 通用数据与业务权限

```text
projects.read
projects.manage
projects.create
projects.delete
projects.unity-migrate
packages.read
packages.manage
repositories.read
repositories.manage
unity.read
unity.manage
unity.launch
backups.read
backups.manage
operations.read
operations.cancel
events.subscribe
settings.read
settings.manage
unity.hub.open
access.read
access.manage
extensions.read
extensions.manage
extensions.permissions.manage
extensions.ui.use
```

`unity.hub.open` 与 `settings.read` 共同授权 Core 打开已解析的 Hub。Core 复验设置 revision 和 expectedTargetToken，找不到 Hub 时可返回 download_required。调用者不能提供任意可执行文件、参数或环境变量。扩展仍受宿主权限和沙箱约束。

## 扩展专用权限

```text
background.run
network.request
notifications.send
clipboard.read
clipboard.write
external-config.read
external-config.manage
mcp.clients.read
mcp.clients.manage
mcp.connections.read
mcp.requests.read
mcp.subscription-streams.read
mcp.configuration.read
mcp.configuration.manage
```

## 规则

M8管理Host桥使用已存在的`mcp.clients.read/manage`，范围为`Global/*`；后者是全部MCP客户端的高风险管理权。
不向扩展开放签发凭据；扩展`projects.read`使用`Global/*`。每次Host调用同时复查交互Client、
Extension及当前lease/UI状态，声明不代表已授予。细节见`mcp-management-v1.md`。

- 第一方和第三方扩展使用同一权限名称。
- 权限按 Principal 授予并可随时撤销。
- Host Capability 每次调用仍需检查权限。
- 权限不能隐式扩大到子权限。
- 高风险权限必须显示具体资源范围。

## M9 后台观察与本机通信

`unity.sessions.read` 使用 `Global/*`。`local-ipc.connect` 使用 `LocalIpcTarget/<signed-target-sha256>`。两项还要求当前实例的 `background.run` 和有效租约，只服务于已协商的后台集成调用。`local-ipc.connect` 作为可选声明，安装同意不会替目标授予权限。Core 在授权时验证已安装签名包确实包含该目标，在连接、读写和关闭时重新验证 exact scope。旧实例的 scope 不会转移到更新后的不同目标声明。见 [State1](../storage/state-v1.md)。

## M3 项目与 Repository 权限

- `projects.read`：允许 `projects.inspect/list/get` 以及读取被授权项目的 normalized snapshot；不允许
  修改项目文件。
- `projects.manage`：允许在 ALCOMD registry 中 register/refresh/unregister 项目；它不隐含
  `packages.manage`，也不允许创建、删除或修改项目目录。
- `repositories.read`：允许 `repositories.inspect/list/get/packages` 的 normalized metadata 查询及
  `repositories.previewListExport` 的有界全量导出范围预览。`repositories.inspectListFile/exportListFile`
  仅接受已认证的交互客户端。扩展没有直接读写本机列表文件的宿主接口。列表登记仍需
  既有 `repositories.manage` 权限并经过相同的 inspect/register 用例。
- `repositories.manage`：允许 register/refresh/unregister local 或匿名 HTTP(S) source 及
  last-known-good cache；不允许 credential、自定义 header、package payload 下载或项目写入。
- GUI、CLI 和已认证 MCP 客户端拥有相同的上述权限。capability、client metadata、路径、URL 与资源 ID
  均不是身份或授权凭据。
- 项目访问只检查对应能力；内部AccessContext也不保存项目ID授权集合或提供按项目缩小授权的入口。
  MCP 每次请求校验实际客户端身份，项目新建或删除不会改变其业务权限。
  资源owner及实际调用Principal、任务和幂等隔离保持；其他尚未实现的外部权限不由名称存在而开放。

## M7 Project Directory Delete 权限

- `projects.delete` 只授权 `projects.planDeleteDirectory` / `projects.applyDeleteDirectory` 对一个已注册
  `ProjectId` 执行已冻结的 sibling-quarantine permanent delete；它不接受 caller path，也不隐含
  `projects.manage`、`projects.create`、package mutation 或任意目录删除能力。
- Plan 同时要求 `projects.read`；Apply 再次要求 `projects.delete`，并复验 Plan owner、Project revision、
  root/parent filesystem identity、ProjectVersion marker、writer evidence 与 protected-root policy。
- Core 以实际调用主体执行受权的文件操作。`projects.delete` 在 4.0.0 不授予 extension；
  first-party 身份、ExtensionId、ProjectId、PlanId、OperationId 与 client metadata 都不能替代授权。
- `quarantine_intent` 之后只能 forward recovery。权限不允许跳过 sibling quarantine、跨 mount、跟随 link、
  触碰 quarantine 后重建的原路径，或把 `not_observed` 表述为 Unity 确定未运行。

## M7 Project Unity Migration 权限

- `projects.unity-migrate` 只授权 `projects.planUnityMigration` / `projects.applyUnityMigration` 的 sealed
  Project Unity Version workflow；不接受 caller path、target version 双 authority、任意 package ChangeSet 或 public child Operation。
- Plan 同时要求 `projects.read + unity.read`；Apply 再次要求 `projects.unity-migrate`，并 fresh revalidate immutable
  Plan、Project revision/root/marker、target installation identity/version 与 writer state。
- 受权文件变更由 Core 执行。`projects.unity-migrate` 在 4.0.0 不授予 Extension，第一方
  Extension 也没有隐藏 privileged API。
- `preparation_intent` / `launch_intent` 之后只能按同一 Operation forward recovery，不允许普通 cancel、自动 respawn、
  kill Unity、跳过 exact Project reobservation 或产生中间 Project/package public revision/Event。

## M4 Package Plan/Apply 权限

- `packages.planInstall/planRemove/planUpgrade/planDowngrade/planResolve` 必须同时具备
  `projects.read`、`repositories.read` 与 `packages.read`，并复核目标项目及所引用仓库的可见性、归属和来源。
  不存在逐项目授权集合。Plan 的内部 durable persistence 不要求 `packages.manage`，也不授权项目写入。
- `packages.applyPlan` 必须再次具备上述 read 权限，并额外具备 `packages.manage` 能力；
  每次 Apply 都复验 Plan owner、当前能力、目标项目revision与pinned source。
- `packages.manage` 不隐含 repository credential、legacy cleanup、任意项目路径写入或 Unity 启动。
  它只授权执行已经批准且绑定目标 ProjectId 的 package ChangeSet。
- capability、PlanId、OperationId、package/repository ID、SID 和 client metadata 都不是身份凭据。
- GUI、CLI 和经认证的 MCP 通过相同 Core 用例 Apply。Core 复验计划所属主体、项目修订、候选可见性和文件系统证据，按原幂等键记录回执。

## CLI、Unity、项目创建与备份权限

- `unity.read`：查询 installation registry、project launch config、exact launch options 与 writer state；
  不允许修改 registry、启动 Editor 或读取任意进程技术信息。
- `unity.manage`：manual installation add/remove、受限 discovery refresh 与 project launch arguments
  修改；全局 `settings.unity.defaultLaunchArguments` 更新另需同时具备 `settings.manage`，包括 null/空数组操作。
  不包含 `unity.launch`、Installation preference、package mutation 或任意 settings 写入。
- `unity.launch`：只允许通过 application 启动/观察与 Project canonical exact version 匹配的已验证 Editor，
  且 InstallationId 只作为 one-shot request authority；不允许 registry
  修改、shell command、任意 executable 或绕过 writer gate。
- `projects.create`：只允许在显式、不存在的 destination 创建一个全新 Project；不允许删除、覆盖、
  merge 或修改既有 Project，也不扩大 `projects.manage` 的 M3 registry 语义。
- `projects.create` 入口与 `projectTemplates.*` 目录操作按已认证客户端的业务权限校验。模板目录不新增权限。
  使用模板依赖创建项目时另检查 `packages.read`、`repositories.read` 和 `packages.manage`。
- `backups.create` 根据项目身份和修订创建普通 ZIP。Core 校验备份及项目读取权限，复验项目归属、写入状态和目的文件。缺省压缩与内容选项从当前设置读取。
- `backups.restore` 以源 ZIP、目标目录和幂等键创建恢复任务。Core 复验归属、目标目录身份、归档内容和路径范围。GUI、CLI 和 MCP 共享同一用例。

## M6 Extension Runtime 权限与 scope

扩展生命周期与项目摘要使用以下权限。

| permission | caller | exact authority |
|---|---|---|
| `extensions.read` | authenticated management Principal | list/get installed extension、desired/runtime safe summary；不返回 Host PID、路径、lease secret 或 data value |
| `extensions.manage` | authenticated management Principal | plan/apply install、enable、disable、plan/apply uninstall；不隐含 permission grant |
| `extensions.permissions.manage` | authenticated management Principal | set/revoke extension grant 与 specific resource scope；grant revision durable update 是 revoke linearization point |
| `background.run` | extension Principal | 允许已验证 background Component 被 Host 启动；不隐含任何业务 capability |
| `projects.read` | extension Principal | 通过 `host-projects.get-summary` 读取当前及未来登记项目的安全摘要，须有 `Global/*` 授权 |

规则：

- extension Principal 默认零权限。Manifest request、ExtensionId、publisher、first-party status、package digest、
  WIT capability 或 `ExtensionInstanceLease` ID 都不是 grant。
- 扩展的`projects.read`只接受`Global/*`，覆盖当前及未来登记项目。
  Host 只返回安全摘要，文件路径和写入能力不在此权限范围。
- `ExtensionInstanceLease` 绑定 current grant revision；每次 Host/data call 重新解析真实 Principal/grant/scope。
- revoke transaction 提交后，尚未被 application 接受的 call、Host queue、session/handle 均失败或取消。
- uninstall 无论 `retain_data|delete_data` 都在 package authority removal 前 revoke 全部 grant/lease/session/handle；
  retained namespace 不保留 active grant，未来 reinstall 重新从 deny-by-default 开始。
- 第一方只可获得同名 permission 与同种 scope；official trust 不绕过 grant/revoke/quota。
- `network.request`、notifications、clipboard 与 external-config 尚无可用宿主接口。
- MCP 状态观察使用 `mcp.requests.read`、`mcp.connections.read` 与 `mcp.subscription-streams.read`，权限逐组检查。

## M7 Portable UI 权限与双重授权

- `extensions.ui.use` 是 client permission，resource kind 固定为 `Extension`，resource ID 必须是 exact ExtensionId。
  它不允许 list/install/enable/grant、读取 extension-owned data 或调用其他 extension scope。
- `extensions.ui.open/refresh/dispatch` 每次都要求 client 同时拥有 `extensions.read` 与 scoped
  `extensions.ui.use`；close 只允许当前 connection/session owner best-effort 执行。
- interactive UI 中的 `host-projects.get-summary(ProjectId)` 要求 Client Principal 与 Extension Principal 对同一
  ProjectId 均有 `projects.read`。任一侧不能扩大另一侧 authority。
- interactive UI 中的 host-data 要求 client scoped `extensions.ui.use`，同时 Extension Principal 只能访问自身
  ExtensionId namespace；client 不因 UI session 获得 raw extension data authority。
- 不存在 `ui.contribute`。Manifest `[ui]` 只是 package declaration，不是 permission 或 OS/Core capability。
- `background.run` 只允许没有 active client UI session 时持有 background lifecycle lease，不由 `[ui]` 隐式加入。
  UI-only extension 可不请求它；Portable UI session 也不产生隐式 background lease。
- client capability/metadata、GUI identity、session ID、guest-session-id 与 InvocationContextId 都不是授权凭据。

## M7 Official GUI settings

- `settings.read` 只允许读取 Config1 中的规范化公开设置；不允许直接读取文件、extension-owned
  data 或未来 updater/MCP/Discord/migration setting。
- `settings.manage` 只允许以 `expectedRevision` 更新同一 closed partial settings DTO，包括 Core
  校验后的 `unity.hubPath` 程序位置偏好；它不授权通用 key/value config、向所选路径写文件、执行
  该程序或 State DB 修改。`settings.read` 的公开配置与派生 `unityHub` 结果可以返回该授权路径，
  但不授权读取该程序内容或任意文件。Editor 发现仍单独要求 `unity.manage`。全局默认启动参数更新（含清除和空数组）同时要求 `unity.manage`；
  Hub-only 更新不受该额外要求影响。窗口 `rememberState` 开关仅要求既有 settings 权限，不需 unity.manage；
  它不授权任意窗口几何、文件或数据库写入。
- 日志与活动权限已随2026-11-05所有者批准的整套系统删除撤销；不再提供 `activity.read`、`diagnostics.read` 或对应读取接口。普通任务和Event读取保留自身权限。
- `settings.get`、`settings.update` 保持base RPC v1，不把client metadata当作permission。


### MCP tool providers

Optional `mcp.tools.provide` publishes signed tool declarations. Additional extension grants `repositories.read`, `packages.read`, `packages.manage`, `operations.read`, `operations.cancel` all use Global/*. Tool execution checks its declared operations, current extension grants and lease, and the actual MCP client identity. `projects.read` retains safe project projections. See `../mcp/extension-tools-v1.md`, [State1](../storage/state-v1.md) and the [MCP client contract](../rpc/m8-mcp-clients-v1.md).


## M8 Core observation reads

`mcp.connections.read` and `mcp.requests.read` independently allow the bounded
Core IPC / in-flight Core RPC projection via mcp.runtime.snapshot and the readonly
observations@1.0.0 Host interface. Extension scopes are exactly Global/* ,
optional and off by default; both interactive Client and Extension must permit each
group. These are administrative observation permissions, not external MCP business
grants or OAuth scopes. They grant no connection termination or Operation control.

## 完整扩展授权目录与安装同意

当前可授予扩展的完整目录为background.run、projects.read、mcp.clients.read、mcp.clients.manage、mcp.connections.read、mcp.requests.read、mcp.subscription-streams.read、mcp.tools.provide、repositories.read、packages.read、packages.manage、operations.read、operations.cancel。所有项目均可声明required或optional；required仅决定安装集中确认与后续运行前置，不自动扩大交互Client权限。

permissions.list返回Core的availablePermissions及当前包声明/实际grants。管理页按必需权限、可选权限、其他权限三类显示；其他权限指当前可授权但未声明的项，三类均允许显式授权；仅限上述目录，扩展只可使用已实现且属于其权限目录的宿主能力。background.run保持Extension/<ExtensionId>，其余保持Global/*，无逐项目授权。安装需extensions.manage；涉及初始required授权另需extensions.permissions.manage。撤销仍使旧lease/UI失效，不因幂等重放或恢复补回授权。

### Adapter-reported subscription streams

`mcp.subscription-streams.read` allows the subscription-stream group of
`mcp.transport.snapshot` and `alcomd:mcp-transport/observations@1.0.0`, exactly
`Global/*`. It can be declared required or optional and is never granted implicitly.
Connection and request groups keep their independent existing read permissions.
All transport rows are `adapterReported`; only an actual Core-authenticated
request can bind a client identity. Authenticated GUI, CLI and MCP callers use the
same Core observation permissions. Extension calls also require the corresponding
extension grant, lease, lifecycle and UI checks. The projection excludes parameters,
credentials and tool results.
