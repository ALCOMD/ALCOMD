# Extension Host protocol InvocationContextId proposal

状态：M7 Stop A proposal-only；不修改 active Host protocol 或 production Host。

现有 requestId/callId 继续只做 correlation。M7 direct rewrite 在每个 `invoke-export` 增加 daemon-issued
`invocationContextId`，格式为 `ictx_` 加 43 个 unpadded base64url 字符（256 random bits），总长 48 ASCII bytes。
它只存在 daemon/Host memory，不写 state/Event/log/diagnostic，也不返回 public RPC。

daemon context record 绑定：

- kind：`background`、`lifecycle` 或 `interactive-ui`；
- ExtensionId、Extension PrincipalId、InstanceId、leaseId、grant revision、lifecycle generation；
- deadline/cancellation；
- 仅 interactive-ui 存在的 Client PrincipalId、client connection/instance、UiSessionId、surfaceId、SnapshotRevision。

`invoke-export` 携带 context ID；该 export 内的每个 `capability-call` 必须原样回显。Host/guest 不能创建、选择、修改、
替换或跨 session 重用 context。Host 不把 Principal/scope/grant metadata 放进 capability call；daemon 从 context 与当前
application authority重新解析。

well-formed、由当前 daemon 签发、但已因 export complete/cancel/revoke/deadline 失效的 ID得到内部稳定
`invocation_context_stale` capability rejection，guest call fail closed；不重新启用 authority。unknown ID、wrong
InstanceId/generation/session、在另一个 in-flight export 中重用、malformed ID 或 Host 声称的 kind/session 与 context
不一致是 Host protocol violation：立即终止 Host 并进入现有 crash/quarantine路径。

每个 context 最多绑定一个 in-flight export；Host 每次最多一个 guest UI call。export 完成后 context 立即 stale，直到
current Host generation结束只保留 bounded 64-entry stale fingerprint set；更旧 ID当 unknown protocol violation。

UI open 不传 hostId/framework/renderer/Tauri/platform。已有 background-running Host 不重复调用 lifecycle activate；UI-only
Host 以 activation kind `interactive-ui` 启动。最后 session 关闭且无 background lease，5,000 ms idle 后以
`interactive-ui-idle` deactivate/stop。

