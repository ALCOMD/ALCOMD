# Portable UI v1 permission proposal

状态：M7 Stop A proposal-only；不修改 active Permission enum 或 production grant wiring。

## Client permission

新增候选 client permission `extensions.ui.use`，resource kind 固定 `Extension`，resource ID 固定 exact ExtensionId。
它不允许 list/install/enable/grant、读取 extension-owned data，或调用 extension 的其他 scope。

| RPC | Client Principal | Extension Principal |
| --- | --- | --- |
| extensions.list/get 中读取 optional UI declaration | `extensions.read` | 不启动 guest |
| extensions.ui.open/refresh/dispatch | `extensions.read` + `extensions.ui.use` scoped exact ExtensionId | current instance/lease/grant/lifecycle valid |
| extensions.ui.close | caller connection 对 session 的 best-effort ownership；stale/absent 也可安全 close | 不要求 live lease |
| UI invocation 内 `host-projects.get-summary(ProjectId)` | `projects.read` scoped same ProjectId | `projects.read` scoped same ProjectId |
| UI invocation 内 `host-data.get/set/delete` | `extensions.ui.use` scoped ExtensionId；不获得 data direct read | extension self namespace 与 current lease/grant |

缺任一 required authority 时 fail closed。client capability、metadata、GUI identity、session ID、guest-session-id 与
InvocationContextId 都不是授权凭据。

## Extension permissions

不新增 `ui.contribute`。最终 direct rewrite 从 active proposal 删除该名字；声明 `[ui]` 只是 package contract，不是 OS/
Core capability。

`background.run` 只授权 extension 在没有 active client UI session 时持有受控 background lifecycle lease。Manifest
required permissions不得自动加入它。它不授权 Portable UI、项目/数据/network/filesystem、任意长调用或 process control。
UI-only extension不请求该权限也能 install/enable/open；扩展自行将其列为required且未授权时enable失败，列为optional且
未授权时UI仍可使用。open创建 `interactive-ui` lease，最后 session关闭且无background lease时按exact idle deadline停Host；
Portable UI不产生隐式background lease。

Host capability 在 interactive UI invocation 中取 Extension Principal grant/scope/lease 与 Client Principal 等价业务
permission/scope 的交集。`host-data` 是唯一例外：数据属于 extension self namespace，客户端不取得直接 data permission。
未来 Plan/Apply/Operation 和 credential 等高影响 authority 不因 UI session 放宽。
