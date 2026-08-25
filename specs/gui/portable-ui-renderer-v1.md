# Portable UI v1 consumer contracts

状态：M7 Stop A proposal-only；没有 production React renderer 或 headless executable。

## Official React / Material Design 3 renderer

- 只消费 public RPC session/Snapshot/action DTO，exhaustive match全部 v1 nodes/actions；unknown protocol/node fail closed。
- 使用 `@alcomd/ui`/MD3决定 component、theme、density、type、tone、spacing 和 reduced motion；extension没有 CSS/DOM。
- Snapshot 外显示 daemon record中的 host-owned extension name/ID/publisher-trust/version/desired/runtime/quarantine 与
  extension-provided 标记。
- draft 严格绑定 session/snapshot revision/form nodeId，只存在 GUI process memory；不写 localStorage、state、data、
  Event或log。revision改变、disconnect、stale或close时失效，禁止按 field name/type保留或合并。
- Portable UI无server push。只允许用户主动 refresh，或页面可见且没有dirty form时有界轮询；dirty draft的刷新/导航必须
  先经过host-owned discard confirmation。
- keyboard order遵循 tree/order；form labels、status live region、progress semantics由 renderer生成，不接受 custom ARIA。
  invalid field设置原生invalid state，并让 `aria-describedby` 指向host-generated validation message ID；最多512 UTF-8
  bytes的extension validation text不是security confirmation。
- component tests覆盖 keyboard/focus/ARIA、200% zoom、320 CSS px、reduced motion、loading/empty/error/disconnected。
- 不使用 extension Tauri command/capability、iframe/WebView、private first-party node或 GUI-to-Host direct channel。

## Non-Tauri headless reference consumer

- 位于 Core contract test/fixture 层，只读取 public JSON DTO；不依赖 `apps/alcomd-gui`、Tauri、React 或 Material。
- 将每个 Snapshot确定性归约为 ordered semantic records：node kind/id/parent/order、plain text/tone、fields/options、
  disabled/read-only/validation 与 actions。
- 验证全部 17 node kinds、两个 action kinds、form editable typed values、exact parent/child matrix、unknown protocol/node
  fail closed，以及同一 fixture与official renderer contract得到相同 semantic order/authority。
- 不渲染像素，不执行 guest，不直接访问 Host/state/project/data；它是 portability/conformance evidence，不是产品 GUI。

`crates/alcomd-testing/fixtures/m7/headless-renderer-conformance.json` 冻结两个 synthetic document的期望 semantic summary。
Stop A 只提供 contract test；production renderer、真实 third-party consumer、a11y automation与三平台 GUI验收仍 planned。
