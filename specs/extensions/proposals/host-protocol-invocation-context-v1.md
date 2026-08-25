# Extension Host protocol InvocationContextId proposal

状态：M7 Stop A review closure proposal-only；不修改 active Host protocol 或 production Host。

现有 requestId/callId 继续只做 correlation。M7 direct rewrite 在每个 `invoke-export` 增加 daemon-issued
`invocationContextId`，格式为 `ictx_` 加 43 个 unpadded base64url 字符（256 random bits），总长 48 ASCII bytes。
它只存在 daemon/Host memory，不写 state/Event/log/diagnostic，也不返回 public RPC或暴露给 guest WIT。

daemon context record绑定 invocation kind、ExtensionId、Extension PrincipalId、InstanceId、leaseId、grant revision、
lifecycle generation、deadline/cancellation；interactive-ui context另外绑定 Client PrincipalId、client connection/instance、
UiSessionId和 SnapshotRevision。`guest-session-id` 只是 guest correlation token，不是 InvocationContextId或 authority。

`invoke-export` 携带 context ID；该 export 内的每个 `capability-call` 必须原样回显。Host/guest不能创建、选择、修改、
替换或跨 invocation/session重用 context。Host不把 Principal/scope/grant metadata放进 capability call；daemon从 context
与当前 application authority重新解析。invoke-export返回后 context立即完成并失效。

## Normal authority races

已知且正确绑定的 in-flight context若因 grant revision变化、lease撤销、UI Session关闭、deadline/cancellation触发或
lifecycle generation合法推进而失去 authority，固定映射为：grant revision变化返回 `permission-denied`；lease撤销返回
`lease-revoked`；UI Session关闭或generation推进返回private protocol `invocation_context_stale`；deadline/cancellation
返回 `cancelled`。这些stable rejection都取消当前invocation，是正常竞态，不把Host计为恶意crash，也不重新启用authority。

## Host protocol violations

unknown context、跨 ExtensionId、跨 UI Session、跨 invocation、completed context reuse、Host尝试修改 Client/Extension
Principal、scope/grant/generation，或 forged context，均为 Host protocol violation：关闭 Host channel、终止 Host并进入
既有 crash/quarantine evidence。completed reuse不能降级为 `invocation_context_stale`。每个 context最多绑定一个
in-flight export；Host每次最多一个 guest UI call。

UI open不传 hostId/framework/renderer/Tauri/platform。已有 background-running Host不重复调用 lifecycle activate；UI-only
Host以 activation kind `interactive-ui`启动。最后一个 session关闭且无 background lease时调用
`deactivate(interactive-ui-idle)`，并在5,000 ms deadline内停止 Host。
