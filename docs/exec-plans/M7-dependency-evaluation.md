# M7 Stop A dependency candidate evaluation

状态：原 Stop A 候选审计；本报告摘要中的 production/component 候选均未批准或安装。唯一后续例外是文末已获批并落盘的
`@playwright/test = 1.62.1` GUI test-only devDependency 及其正常 lock closure。

查询日期：2026-08-24。版本、metadata 与维护时间来自 npm registry / crates.io exact package metadata；生产采用前
仍须以独立 lockfile diff 和现有三平台 build 验证。Workspace 继续固定 Node 24 与 Rust 1.97.1。

## 结论摘要

| need | candidate | Stop A recommendation |
|---|---|---|
| native file/directory picker | `@tauri-apps/plugin-dialog 2.7.2` + `tauri-plugin-dialog 2.7.2` | 候选，等待 capability/lock diff 审批 |
| MD3 source color/HCT | `@material/material-color-utilities 0.4.0` | 候选；仅在 Material Web 2.5.0 确认不提供所需 HCT 时采用 |
| component runner | `vitest 4.1.11` | dev-only 候选 |
| React DOM queries | `@testing-library/react 16.3.2` + `@testing-library/dom 10.4.1` | dev-only 候选 |
| keyboard/pointer interaction | `@testing-library/user-event 14.6.6` | dev-only 候选 |
| automated a11y rules | `axe-core 4.13.0` | dev-only 候选；不冒充人工 screen-reader smoke |
| DOM environment | `jsdom 30.0.1` | 暂不采用：要求 Node `^24.15.0`，当前合同只保证 Node 24 而非该 patch floor |
| actual WebView isolation | no new dependency; existing Tauri 2.11.5 test-only in-app harness | 推荐；Playwright/WebDriver 不能替代 WKWebView/WebKitGTK/WebView2 内部隔离证据 |
| router/query/global state | none | 不引入；18 个稳定 route 与现有 read model 未证明需要通用框架 |

## Tauri dialog plugin

Exact proposal：

```text
npm dev/runtime package: @tauri-apps/plugin-dialog = 2.7.2
Rust production package: tauri-plugin-dialog = 2.7.2,
    default-features = false,
    features = ["gtk3"]
```

- license：两端均 `MIT OR Apache-2.0`；Rust MSRV 1.77.2。
- maintenance：Tauri 官方 plugins-workspace；npm exact metadata 依赖 `@tauri-apps/api ^2.11.0`，与已锁 2.11.1
  同 major/minor compatibility line；2026-08 registry current exact version 2.7.2。
- Node 24：包未声明更窄 engine；纯 ESM/JS adapter，当前 npm metadata 无 native install script。
- Rust features：关闭 default 后显式保留 Linux `gtk3`，不得启用 `xdg-portal`；现有 Ubuntu build 已安装 GTK3。
- direct Rust dependency/build impact：`log 0.4.21`、`raw-window-handle 0.6`、`rfd 0.16`、serde、serde_json、
  Tauri 2.10-compatible、`tauri-plugin-fs 2.5.1`、thiserror 2、url 2；build dependency `tauri-plugin 2.5`。
  其中大部分可能已锁定，但 `rfd`/dialog/fs plugin 的完整 target closure 必须由批准前 isolation resolve 报告确认。
- bundle：npm unpacked 33,316 bytes（不是最终 gzip/bundle 值）；Rust 会增加 native dialog/rfd/plugin glue，实际三平台
  release delta 必须在实现审批前测量。extension frame 不获得 dialog capability。
- expected lock diff：root `package-lock.json` 增加 exact JS package并复用现有 `@tauri-apps/api`；root `Cargo.lock`
  增加 `tauri-plugin-dialog 2.7.2`、`rfd 0.16`、`tauri-plugin-fs 2.5.1` 与其未锁 target closure；Discord lock 不变。
  该清单是预期类别，不是 blanket approval，任何实际未解释 package 必须停下。
- no-dependency alternative：暂不提供 picker，让用户通过 CLI 输入路径；浏览器 `<input type=file>` 不能可靠返回 daemon
  所需 native absolute path，不能作为正式替代。自行写三平台 picker 会扩大 platform API，劣于官方 plugin。

## MD3 source color/HCT

Exact proposal：

```text
@material/material-color-utilities = 0.4.0
```

- license Apache-2.0；Google Material Components 仓库维护，registry 2026-01-21 更新。
- 没有 runtime dependency、native addon 或 install/build script；不要求 Node API，Node 24 build 可用。
- npm unpacked 1,064,332 bytes；生产应只 import HCT/theme generation symbols并以 Vite tree-shaking后的实际 delta 验收，
  不把整包大小冒充最终 bundle。
- expected lock diff：只增加该 exact package record，不应增加 transitive package；Cargo locks 不变。
- no-dependency alternative：固定默认 source color + CSS design tokens。若 M7 不允许用户 source color，这一替代足够；
  不自行实现 HCT/color science。只有 settings v1 的 source color 获批且 Material Web 不提供生成能力时才采用候选。

## Component 与 accessibility dev stack

候选均只进入 root `devDependencies`，不得进入生产 runtime/bundle：

| package | exact | license | Node 24 / maintenance | direct transitive impact | unpacked |
|---|---:|---|---|---|---:|
| `vitest` | 4.1.11 | MIT | engines `^20 || ^22 || >=24`；2026-08-18 更新 | Vite 6/7/8、Vitest runner/expect/snapshot、tinyexec/glob 等 | 1,909,160 B |
| `@testing-library/react` | 16.3.2 | MIT | Node >=18；React 18/19 peers；2026-08-07 更新 | `@babel/runtime`; peers DOM/types/react | 336,758 B |
| `@testing-library/dom` | 10.4.1 | MIT | Node >=18；2026-08-07 更新 | aria-query、pretty-format、dom-accessibility-api 等 | 2,426,344 B |
| `@testing-library/user-event` | 14.6.6 | MIT | Node >=12；2026-08-22 更新 | DOM peer，无 direct dependency | 435,026 B |
| `axe-core` | 4.13.0 | MPL-2.0 | Node >=4；2026-08-21 更新 | 无 direct dependency | 3,113,323 B |

- expected `package-lock.json` diff：上述 exact dev roots及 npm metadata列出的 closure；Cargo locks 不变；生产 Vite output
  必须证明没有这些 package。
- native/build scripts：registry direct metadata未显示 native addon/install script；完整 lock resolve后仍须永久 gate 拒绝
  unexpected native/build dependency。
- DOM environment 尚未选择。`jsdom 30.0.1`（MIT，unpacked 7,086,515 B）要求 Node
  `^22.22.2 || ^24.15.0 || >=26`，不能在仅 `>=24 <25` 的现有 Workspace 合同下安全批准。可选 no-dependency alternative
  是 Vitest browser mode或小型 pure-function/component tests，但 browser provider会带来另一项依赖审批；因此 Stop A
  只记录测试 stack，不安装。
- no-dependency alternative：Node built-in test + React server rendering可覆盖纯 reducer/schema，但不能可靠覆盖 focus、
  pointer、custom elements 与 axe。若采用候选，人工 Narrator/VoiceOver/Linux reader 仍保留。

## Actual WebView / e2e

推荐 no-new-dependency 方案：使用现有 Rust `tauri 2.11.5` 创建 test-only in-app probe，由实际 WebView执行 bounded JS，
host 写出/断言 JSON。Linux 在已有 GTK/WebKitGTK前提下使用 CI display harness；Windows 使用 WebView2；macOS使用
WKWebView。probe 不进入 production binary或 capability。

Tauri WebDriver/Playwright/Selenium只在能证明它们实际驱动上述 app WebView且三平台等价时再评估。普通 Chromium
Playwright只能提供 DOM/browser evidence，不能证明 WKWebView/WebKitGTK/Tauri private IPC isolation，因此 Stop A
不提出它作为依赖。manual screenshot与screen-reader smoke保持独立证据类别。

## Lockfile 与审批门禁

本报告未执行 `npm install`、`cargo add` 或 manifest变更，三份 lockfile应保持 byte-for-byte不变。任何候选获批后先
在隔离变更中生成锁文件并报告 exact diff、active feature graph、native/build script与三平台 bundle delta；出现本报告
无法解释的 production package时停止。候选版本不是 ranges，升级需要重新审计。

## 2026-08-26 Official GUI Completion test dependency approval

项目所有者批准唯一新增 GUI test-only devDependency：

```json
"@playwright/test": "1.62.1"
```

- placement：仅 `apps/alcomd-gui` devDependencies，exact version；
- license：Apache-2.0；Node engine `>=20`，兼容固定 Node 24；
- direct lock closure：`@playwright/test 1.62.1 -> playwright 1.62.1 -> playwright-core 1.62.1`；
- optional `fsevents 2.3.2` 已存在于锁图，只是 Playwright 下的 optional dev record；
- browser：只使用 package-matched Chromium revision，不使用 system Chrome、Firefox 或 WebKit；
- authority：真实 browser DOM/keyboard/focus/layout/ARIA/contrast automation，不是 WebView2/WebKitGTK/WKWebView 或平台
  screen-reader certification；
- production：不得进入 Vite shipping bundle、Tauri runtime、Rust graph、Extension Runtime、Portable UI contract 或 SDK；
- excluded：axe、Vitest/jsdom/happy-dom/testing-library、Puppeteer/Selenium/WebDriver 与 screenshot-diff framework。

安装前根 `package-lock.json` SHA-256 为
`61117ba5e1fa9d3804912aa1ab43b0946a020abd8bf372d09d27b14dfe6e46d1`。安装后审计只新增上述三项与 optional
`playwright/node_modules/fsevents` record，没有删除或改变既有 package version。安装后 SHA-256 为
`ead52597d90d5dc02d780e20edcf737a5c673ef46be7f967ed2c3fd4f5984639`。Windows 本地使用
`npx playwright install chromium` 安装 package-matched Chromium 与 headless shell revision `1234`；同时取得 Playwright
测试工具所需的 `ffmpeg-1011` 与 `winldd-1007`，这些均位于用户测试缓存，不进入 repository、production bundle 或发行资产。
