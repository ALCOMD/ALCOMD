# Portable UI v1 consumer contracts

状态：M7 Stop A proposal-only；没有 production React renderer 或 headless executable。

## Official React / Material Design 3 renderer

- 只消费 public RPC session/Snapshot/action DTO，exhaustive match全部 v1 nodes/actions；unknown protocol/node fail closed。
- 使用 `@alcomd/ui`/MD3决定 component、theme、density、type、tone、spacing 和 reduced motion；extension没有 CSS/DOM。
- Snapshot 外显示 host-owned extension name/ID/publisher-trust/runtime/quarantine 与 extension-provided 标记。
- draft 只在当前 session/snapshot/form 本地保存；revision改变时按 matching field/type保留合法 draft，否则丢弃；从不
  把 local draft当 daemon authority。
- keyboard order遵循 tree/order；form labels、status live region、progress semantics由 renderer生成，不接受 custom ARIA。
- component tests覆盖 keyboard/focus/ARIA、200% zoom、320 CSS px、reduced motion、loading/empty/error/disconnected。
- 不使用 extension Tauri command/capability、iframe/WebView、private first-party node或 GUI-to-Host direct channel。

## Non-Tauri headless reference consumer

- 位于 Core contract test/fixture 层，只读取 public JSON DTO；不依赖 `apps/alcomd-gui`、Tauri、React 或 Material。
- 将每个 Snapshot确定性归约为 ordered semantic records：node kind/id/parent/order、plain text/tone、fields/options、actions。
- 验证全部 17 node kinds、两个 action kinds、form typed values、unknown protocol/node fail closed，以及同一 fixture与
  official renderer contract得到相同 semantic order/authority。
- 不渲染像素，不执行 guest，不直接访问 Host/state/project/data；它是 portability/conformance evidence，不是产品 GUI。

`crates/alcomd-testing/fixtures/m7/headless-renderer-conformance.json` 冻结两个 synthetic document的期望 semantic summary。
Stop A 只提供 contract test；production renderer、真实 third-party consumer、a11y automation与三平台 GUI验收仍 planned。

