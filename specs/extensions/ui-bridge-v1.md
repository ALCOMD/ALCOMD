# Extension UI Bridge v1 security envelope

状态：M6 contract-first Stop A candidate；只冻结安全基础与 headless harness，不冻结 M7 placement。

M6 不定义 sidebar、settings page、toolbar、context menu 或 navigation placement。package 可携带 static `ui/`
assets；测试使用 `headless/test contribution` synthetic fixture，不形成 public product slot。

## Isolation 与 origin

- UI 位于 sandboxed iframe/isolated WebView，origin 精确为
  `alcomd-extension://<ExtensionId>/<packageDigest>/`，只加载已验证 package 的 static assets。
- 无主 DOM、Tauri IPC、Node、Host filesystem、daemon RPC socket、private channel 或 cross-extension frame access。
- Bridge session 由 core 创建，绑定 ExtensionId、InstanceId、package digest、PrincipalId、grant revision、origin 和
  lifecycle generation；页面 input 不能声明这些 authority values。

## Envelope

request：`bridgeVersion=1, sessionId, sequence, requestId, method, params`。

response：`bridgeVersion=1, sessionId, sequence, requestId, result|error`。

event：`bridgeVersion=1, sessionId, sequence, event, data`。

- requestId 是 1-64 printable ASCII，在 session 内不可复用；sequence 是每方向从 1 开始严格递增的 u64。
- out-of-order/replayed sequence、requestId collision、wrong origin/session/generation/grant revision 均 fail closed。
- disable/revoke/uninstall 在 durable grant revision linearization 后关闭 session、取消 pending request、清空 event queue。

## Exact limits

| limit | value |
|---|---:|
| encoded message bytes | 262,144 |
| request rate | 30 / 60,000 ms |
| burst | 10 |
| concurrent requests | 8 |
| pending requests | 64 |
| queued events | 128 |
| idle session | 300,000 ms |
| absolute session lifetime | 3,600,000 ms |

M6 headless harness 只使用 `headless.test.ping` synthetic method；它不进入 production Bridge catalog。origin spoof、
replay、collision、oversize、flood、DOM/Tauri/private-channel attempt 都必须有 negative vector。
