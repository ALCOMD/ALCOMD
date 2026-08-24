# M7 official GUI contract-first Stop A proposal

状态：**proposal，等待项目所有者人工审批；不是 production contract**

本文件只冻结供下一审批轮评审的候选。它不发布 RPC、Permission、State Schema、Tauri capability
或 Extension UI physical ABI，也不授权 production implementation。

## 1. Route identity 与 flow matrix

route identity 是下表的 ASCII 常量；参数只使用 opaque ID。翻译文本、显示名和完整路径不能参与 route identity。

通用状态代码：`L` initial-loading、`R` refreshing（保留 last-known-good）、`E` empty、`X` structured error、
`D` disconnected、`O` operation-running、`C` confirmation-required、`T` terminal success/failed/cancelled、
`S` stale/revision conflict。每个已启用业务 route 至少实现 `L/R/E/X/D`；表中的额外代码不得由前端推断。

| route identity | path pattern | exact authoritative RPC | permission | states | retry / stale rule |
|---|---|---|---|---|---|
| `home` | `/` | `system.status`; `operations.list`; `events.list` | status 无业务权限；`operations.read`; `events.read` | L/R/E/X/D/O/T | 只重试 read；Operation 终态从 daemon 恢复 |
| `projects.list` | `/projects` | `projects.list`; `projects.inspect`; `projects.register`; `projects.refresh`; `projects.unregister` | read: `projects.read`; mutation: `projects.manage` | L/R/E/X/D/S | mutation 复用原 idempotency key；revision conflict 回到显式 review |
| `projects.overview` | `/projects/:projectId/overview` | `projects.get`; `projects.refresh`; `projects.unregister` | `projects.read`; mutation: `projects.manage` | L/R/X/D/S | `:projectId` 为 lowercase UUID；不按路径重新发现 |
| `projects.packages` | `/projects/:projectId/packages` | `projects.get`; `repositories.packages`; `packages.planInstall`; `packages.planRemove`; `packages.planUpgrade`; `packages.planDowngrade`; `packages.planResolve`; `packages.applyPlan`; `operations.get` | Plan: `projects.read` + `repositories.read` + `packages.read`; Apply 另需 `packages.manage` | L/R/E/X/D/C/O/T/S | Apply 只提交原 PlanId/expectedRevision；stale 不静默 re-plan |
| `projects.unity` | `/projects/:projectId/unity` | `unity.projectEditor.get`; `unity.projectEditor.set`; `unity.writerState`; `unity.launch`; `unity.launchStatus` | read: `unity.read`; set: `unity.manage`; launch: `unity.launch` | L/R/E/X/D/O/T/S | writer state unknown/suspected 不由 GUI降级；launch 重试复用 idempotency key |
| `projects.backups` | `/projects/:projectId/backups` | `backups.list`; `backups.get`; `backups.create`; `backups.planRestore`; `backups.applyRestore`; `operations.get` | list/get: `backups.read`; create: `backups.manage` + scoped `projects.read`; restore Plan: `backups.read` + `projects.create`; Apply 另需 `backups.manage` | L/R/E/X/D/C/O/T/S | restore 只使用原 immutable Plan；不覆盖/合并 target |
| `repositories.list` | `/repositories` | `repositories.list`; `repositories.inspect`; `repositories.register`; `repositories.refresh`; `repositories.unregister` | read: `repositories.read`; mutation: `repositories.manage` | L/R/E/X/D/S | read 可重试；mutation 复用 idempotency key/expectedRevision |
| `repositories.detail` | `/repositories/:repositoryId` | `repositories.get`; `repositories.packages`; `repositories.refresh`; `repositories.unregister` | read: `repositories.read`; mutation: `repositories.manage` | L/R/E/X/D/S | `:repositoryId` 为 lowercase UUID；失败保留 last-known-good |
| `templates.list` | `/templates` | `templates.list`; `templates.inspectBundle`; `templates.planImport`; `templates.applyImport` | read: `templates.read`; import: `templates.manage` | L/R/E/X/D/C/O/T/S | import Apply 只提交原 Plan；target/source 变化视为 stale |
| `templates.detail` | `/templates/:templateId` | `templates.get`; `templates.planDerive`; `templates.applyDerive`; `templates.export`; `templates.setFavorite`; `templates.remove`; `templates.planCreateProject`; `templates.applyCreateProject` | 依 method 复用 M5 精确权限；create-project Apply 另需 `packages.manage` | L/R/X/D/C/O/T/S | `:templateId` 为 opaque template ID；不得从标题重建 ID |
| `operations.list` | `/operations` | `operations.list`; `operations.cancel` | read: `operations.read`; cancel: `operations.cancel` | L/R/E/X/D/O/T/S | cancel 复用 expectedRevision/idempotency；接受 cancel 不等于 cancelled |
| `operations.detail` | `/operations/:operationId` | `operations.get`; `operations.cancel`; `events.list` | `operations.read`; cancel: `operations.cancel`; events: `events.read` | L/R/X/D/O/T/S | `:operationId` 为 lowercase UUID；以 daemon 终态为准 |
| `extensions.list` | `/extensions` | `extensions.list`; `extensions.planInstall`; `extensions.applyInstall` | read: `extensions.read`; install: `extensions.manage` | L/R/E/X/D/C/O/T/S | install Apply 不重新 Plan；不因 first-party 绕过确认 |
| `extensions.detail` | `/extensions/:extensionId` | `extensions.get`; `extensions.enable`; `extensions.disable`; `extensions.planUninstall`; `extensions.applyUninstall`; `extensions.setGrant`; `extensions.revokeGrant` | read: `extensions.read`; lifecycle: `extensions.manage`; grant: `extensions.permissions.manage` | L/R/X/D/C/O/T/S | `:extensionId` 为 canonical reverse-DNS；grant revision conflict 显式 review |
| `extensions.ui` | `/extensions/:extensionId/ui` | host 先 `extensions.get`；extension 页面只经获批 UI Bridge allowlist | host: `extensions.read`; extension: current grant/scope/lease | L/X/D | 无 `ui_entry`、disabled、revoked、quarantined 或 digest 变化即关闭 session |
| `settings.appearance` | `/settings/appearance` | proposal: `settings.appearance.get`; `settings.appearance.update` | proposal: `settings.read`; update: `settings.manage` | L/R/X/D/S | 尚未获批；update 使用 expectedRevision + 永久 idempotency |
| `activity` | `/activity` | `events.list`; `operations.list`; `operations.get` | `events.read`; `operations.read` | L/R/E/X/D/O/T | 两条有界 cursor 独立推进；不建立 GUI activity store |
| `about` | `/about` | `system.status` 与 build-time license inventory | 无额外业务权限 | L/X/D | 不显示 PID、完整路径或未实现子系统版本 |

以下 surface 在 M7 Stop A 明确 deferred，production 不注册可导航 route：external access、technical diagnostics/logs、
MCP、Discord、migration/import、updater/channel、deep link、installer/distribution。`gui.v3-entry-parity` 因 M11
真实 Fixture 缺失继续 blocked。

## 2. Appearance/settings v1 proposal

exact wire shape 由 `m7-settings-appearance-v1.proposal.schema.json` 描述：

```json
{
    "appearanceMode": "system",
    "themeSourceColor": "#6cb6ff",
    "density": "comfortable",
    "motion": "system",
    "locale": "system",
    "revision": 1,
    "updatedAtMs": 0
}
```

- `themeSourceColor` 是 lowercase ASCII `#rrggbb`，不接受 alpha、缩写、命名色、CSS function 或宽松修正。
- locale 只允许 `system`, `de`, `en`, `fr`, `ja`, `ko`, `zh-Hans`, `zh-Hant`；未知 locale fail closed。
- `motion=reduced` 强制 reduced motion；`system` 服从 OS preference。最终行为取用户设置与 OS 要求中更严格者。
- defaults 如上；没有行时读取等价于 revision 1 defaults，但 v8→v9 migration proposal 会在同一事务插入 singleton row。
- persistence proposal 选择现有 SQLite/state authority，而不是 Draft `settings.toml` Config subsystem。原因是现有 daemon/
  state store 已提供单 writer、transaction、revision、永久幂等和 migration；为五个 GUI preference 建立第二套 writer
  没有真实需求。
- proposed State Schema v9 只增加一行 `gui_appearance_settings(id=1, ..., revision, updated_at_ms)`；未知 future columns
  由旧 client 忽略。Stop A 不创建 migration 或修改 `dataSchema`。
- proposed compatible RPC capability：`settings.appearance.v1`。proposed permissions：read `settings.read`，update
  `settings.manage`。这两项目前不在生产 Permission enum，必须经下一次人工审批后才可加入。
- `settings.appearance.get` 无 params；`settings.appearance.update` 是完整替换，params 固定为五个设置字段、
  `expectedRevision`（正 u64）与 1-128 printable ASCII `idempotencyKey`。返回新 record + `replayed`；不创建 Operation。
- stable errors 只复用 `invalid_request`、`permission_denied`、`revision_conflict`、`idempotency_conflict`、
  `component_upgrade_required` 与 `internal_error + diagnosticId`；普通错误不含原始 OS/DB/string detail。

`localStorage` 唯一 allowlist key 是 `alcomd.gui.view-state.v1`，总 UTF-8 JSON 最大 4,096 bytes：

```json
{
    "schema": 1,
    "drawerCollapsed": false,
    "projectView": "list",
    "lastTabs": {
        "project": "overview",
        "extension": "overview"
    }
}
```

只允许上列字段与 enum；unknown/corrupt/oversize 直接清除。不得保存 ID、路径、URL、form draft、PlanId、OperationId、
cursor、error、diagnosticId、permission、grant、token、credential 或业务结果。清空 localStorage 不丢任何权威状态。

## 3. Activity 与 diagnostics decision

M7 Activity 是两个现有 read model 的纯客户端组合：

- `events.list(afterSequence, limit<=1000)`，permission `events.read`；
- `operations.list(cursor, limit<=1000)` / `operations.get(operationId)`，permission `operations.read`；
- cursor 分开保存于当前 view memory；断线后从 daemon durable state 恢复，不持久化第三份 Event/Operation 副本；
- 排序只用于展示，不能合并或改写 daemon sequence/revision；Event 与 Operation 的关联只使用公开 opaque ID。

Stop A 未发现 M7 必需、且不能由上述 read model、`system.status`、稳定 `error.code` 与 `diagnosticId` 表达的独立
technical diagnostics use case。因此本提案：

- 不新增 `activity.read`；
- 不提出或预批准 `diagnostics.read`；
- 不创建 diagnostics RPC/DTO/capability/page；
- 原始日志、stack、argv、完整私密路径、token、Authorization、credential 与任意内部错误字符串绝不发送到 WebView。

若未来出现真实缺口，必须从 daemon/application 边界先生成 exact safe DTO 并独立审批；前端隐藏/截断不构成脱敏。

## 4. Main WebView typed Tauri adapter proposal

主窗口只允许三个 application command 名称：

| command | closed request | response | authority |
|---|---|---|---|
| `gui_query` | `GuiQueryRequest` tagged enum | matching `GuiQueryResponse` tagged enum | 只调用对应 typed `alcomd-client` read method |
| `gui_command` | `GuiCommandRequest` tagged enum | matching `GuiCommandResponse` tagged enum | 只调用对应 typed `alcomd-client` command method |
| `gui_select_path` | `{kind: projectDirectory|repositoryFile|repositoryDirectory|templateBundle|templateExportTarget|backupArchive|backupTargetParent|extensionPackage}` | `{selection: cancelled|selected, path?: string}` | 仅 OS picker；返回值仍须交给 daemon 校验 |

`GuiQueryRequest.kind` exact allowlist：

```text
systemStatus, projectsInspect, projectsList, projectsGet,
repositoriesInspect, repositoriesList, repositoriesGet, repositoriesPackages,
operationsGet, operationsList, eventsList,
unityInstallationsList, unityInstallationsGet, unityProjectEditorGet, unityWriterState, unityLaunchStatus,
templatesList, templatesGet, templatesInspectBundle,
backupsList, backupsGet,
extensionsList, extensionsGet,
settingsAppearanceGet (proposal; unavailable until approved)
```

`GuiCommandRequest.kind` exact allowlist：

```text
stateCheck, operationsCancel,
projectsRegister, projectsRefresh, projectsUnregister,
repositoriesRegister, repositoriesRefresh, repositoriesUnregister,
packagesPlanInstall, packagesPlanRemove, packagesPlanUpgrade, packagesPlanDowngrade, packagesPlanResolve,
packagesApplyPlan,
unityInstallationsRegister, unityInstallationsRemove, unityInstallationsRefresh,
unityProjectEditorSet, unityLaunch,
templatesPlanImport, templatesApplyImport, templatesPlanDerive, templatesApplyDerive,
templatesExport, templatesSetFavorite, templatesRemove,
templatesPlanCreateProject, templatesApplyCreateProject,
backupsCreate, backupsPlanRestore, backupsApplyRestore,
extensionsPlanInstall, extensionsApplyInstall, extensionsEnable, extensionsDisable,
extensionsPlanUninstall, extensionsApplyUninstall, extensionsSetGrant, extensionsRevokeGrant,
settingsAppearanceUpdate (proposal; unavailable until approved)
```

每个 enum variant 的 payload/response 必须直接引用相应 `alcomd-protocol` typed DTO；不存在 `method: string`、
`serde_json::Value` params/result、raw frame、raw socket、generic invoke 或 unknown variant fallback。encoded request/response
仍受 4 MiB RPC frame 上限；unknown/oversize/malformed fail closed。Tauri 层不实现业务 permission、resolver、revision、
Plan、Operation、project writer 或 extension lifecycle authority。

TypeScript 选择方案 2：M7 使用 app-private、从同一 JSON Schema authority 生成并 snapshot-verified 的 types；
`@alcomd/sdk` 保持不动，公共 SDK 硬化延迟到 M10。不得再维护一套手写 DTO。

main-window capability proposal（本轮不修改实际 capability）：

```json
{
    "identifier": "main",
    "windows": ["main"],
    "permissions": [
        "core:app:default",
        "allow-gui-query",
        "allow-gui-command",
        "allow-gui-select-path"
    ]
}
```

生产实现前必须由 `tauri-build` 生成三个 app-command permission。现有 `core:default` 会连带 event/window/webview/path/
menu/tray 等默认权限，不能作为 M7 最终 capability。extension WebView/frame 不匹配任何 Tauri capability。

## 5. Extension UI placement 与 physical mapping proposal

唯一 placement 是 host-owned `/extensions/:extensionId/ui`。Manifest v1 仍只声明 `ui_entry`；不增加 sidebar、toolbar、
context-menu、top-level contribution 或 Manifest v2。

候选 asset URL 是宿主私有实现，不是 Extension ABI：

```text
Windows/WebView2: https://alcomd-extension-ui.localhost/v1/s/<session-token>/<ExtensionId>/<packageDigest>/<asset>
Linux/WebKitGTK:  alcomd-extension-ui://localhost/v1/s/<session-token>/<ExtensionId>/<packageDigest>/<asset>
macOS/WKWebView:  alcomd-extension-ui://localhost/v1/s/<session-token>/<ExtensionId>/<packageDigest>/<asset>
```

- `<session-token>` 是每次打开随机 128-bit lowercase hex，不持久化、不进入 Bridge envelope，不从页面输入取得。
- ExtensionId 必须等于 installed record canonical ID；digest 是 64 lowercase hex；root entry 精确映射 verified
  `ui_entry`，其他 asset 只能在 normalized `ui/` root 下。
- path 使用 package profile 的 forward-slash + NFC + full Unicode casefold collision rules；拒绝 empty/dot/dotdot、
  percent-encoded separator、absolute/device/UNC、backslash、NUL/control、duplicate/collision、symlink/reparse/special file。
- 只从 immutable verified package object 读取 regular file；每响应再次绑定 session + current lifecycle generation +
  exact package digest。目录、listing、fallback-to-index、OS path 和 arbitrary file 均拒绝。
- MIME allowlist：`.html text/html; charset=utf-8`、`.js/.mjs application/javascript; charset=utf-8`、
  `.css text/css; charset=utf-8`、`.json application/json; charset=utf-8`、`.svg image/svg+xml`、`.png image/png`、
  `.jpg/.jpeg image/jpeg`、`.gif image/gif`、`.webp image/webp`、`.avif image/avif`、`.ico image/x-icon`、
  `.woff font/woff`、`.woff2 font/woff2`。unknown extension/MIME、WASM、HTML sniffing 全部拒绝；发送 `nosniff`。
- 每文件/总量沿用已验证 package profile（single regular 32 MiB、UI total 64 MiB）；不解压临时副本。
- cache：HTML/JSON `no-store`；其他 immutable asset `private, max-age=31536000, immutable`，且 cache key 包含
  session-token + ExtensionId + digest + normalized path。不得使用 service worker；`worker-src 'none'`。
- navigation：root 只允许同一 session/ExtensionId/digest 下的 asset；fragment-only 允许；其他 scheme/origin、
  download、popup、new window、form、top navigation 拒绝并生成安全分类，不记录原始 URL。
- disable/revoke/uninstall/package replacement/navigation away/crash 立即撤销 token、Bridge session、pending request、
  queue、cached authority 与 handler binding。旧 digest 即使文件仍在 object store 也不能重新打开。

响应 CSP proposal：

```text
default-src 'none'; script-src 'self'; style-src 'self'; img-src 'self' data:;
font-src 'self'; connect-src 'none'; media-src 'none'; object-src 'none';
child-src 'none'; frame-src 'none'; worker-src 'none'; manifest-src 'none';
form-action 'none'; base-uri 'none'; frame-ancestors https://tauri.localhost tauri://localhost
```

不允许 inline script/style、eval、WebSocket、fetch/XHR、service worker 或 remote font。最小 iframe sandbox 候选只有
`allow-scripts`；没有 `allow-same-origin/forms/popups/modals/downloads/top-navigation`。Bridge transport 只接受 host
创建 frame 的 exact `contentWindow`、current session/generation、strict sequence/requestId 和 schema；`event.origin=null`
不能单独证明 authority，任意 sibling/parent message、forged envelope 和 confused-deputy request 均拒绝。

## 6. Physical isolation comparison 与当前停止点

| candidate | 已确认的静态事实 | 必须取得的 actual WebView evidence | 当前结论 |
|---|---|---|---|
| Tauri-managed isolated child WebView | Tauri 2.11.5 为每个 managed WebView main frame注入 invoke bootstrap；即使没有 matching capability，`window.__TAURI_INTERNALS__` 的存在不能由静态 ACL 否定 | WebView2/WebKitGTK/WKWebView 中 `__TAURI__`/`__TAURI_INTERNALS__`、raw invoke/event/channel 的实际 presence/denial，以及 storage/origin/navigation isolation | **not yet frozen**；当前证据不能满足“无 private IPC surface” |
| sandboxed cross-origin iframe | Tauri initialization script 是 main-frame-only；`sandbox=allow-scripts` 不授予 same-origin/top-navigation，Bridge 可绑定 exact contentWindow + session | 三引擎实际验证 Tauri globals/transport、parent/opener/DOM、confused deputy、daemon socket、FS/clipboard/notification/network/top navigation 全部 fail closed | **preferred probe candidate，not yet frozen** |

不能以源码检查、DOM automation 或“extension 不调用”替代真实 WebView evidence。test-only example
`apps/alcomd-gui/src-tauri/examples/m7_isolation_probe.rs` 已通过 compile check；本机 Windows 运行在进入 `main` 前因
loader `STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139)` 终止，因此没有产生 WebView2 结果，不能记为 security evidence。
同时尚未取得 Ubuntu 22.04 WebKitGTK 与 macOS 15 WKWebView 的 actual smoke，因此本 Stop A **不选择最终容器**，
也不把上面的 physical URL/CSP/sandbox proposal 升级为生产合同。下一审批轮需要先批准 test-only in-app harness
进入三个 hosted job；任一平台 iframe 失败则该平台必须改用能满足相同 negative contract 的隔离 WebView，或重新停下。

## 7. Three-platform WebView evidence contract

每个平台必须由真实 WebView 中的脚本生成 bounded JSON result，host 逐字段断言，缺项/timeout/crash 失败：

```text
window.__TAURI__ absent
window.__TAURI_INTERNALS__ absent
raw invoke/event/channel unavailable or rejected before app command
parent/opener/main DOM unreadable
forged sibling/parent postMessage not admitted
daemon socket unavailable
filesystem unavailable
clipboard unavailable
notification unavailable
fetch/WebSocket/image beacon to local deterministic deny endpoint blocked
top-level navigation/popup/download/form blocked
allowed Bridge ping admitted only from exact frame/session
wrong extension/digest/session/replayed sequence rejected
package replacement invalidates old token/session/pending/queue/cache authority
```

Evidence classes不得互相冒充：

- actual WebView security：上述 in-app harness，WebView2 / WebKitGTK / WKWebView 各自产生 JSON artifact；
- DOM/component automation：只证明 React state、keyboard/focus、ARIA 和 rendering；
- manual screenshot：只证明可见 flow/appearance，不证明 sandbox；
- manual accessibility smoke：Narrator/VoiceOver/Linux reader 人工记录，不等于自动 axe 结果。

Windows Server 2025 只证明 hosted WebView2 smoke，不是 Win10/11 M12 客户端发行验证。Ubuntu 仍执行 GLIBC_2.35
ceiling；macOS 仍检查 arm64/minos 11.0。hosted runner 无交互桌面时允许 test-only hidden/in-app harness；不得修改
production path 或静默跳过。

## 8. Stop A approval boundary

下一轮需要人工批准：State v9/settings RPC 与两项候选 permission、三个 typed Tauri command/main capability、
app-private TS contract、最终 UI container/physical mapping/CSP/sandbox、test-only three-platform harness 与依赖候选。
在批准前不修改 production Rust/TS、RPC/permission/State/Config Schema、Manifest/WIT/UI Bridge public contract、
Tauri capability、dependency/lockfile、unsafe 或 platform API。
