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

## 2026-08-26 Material Symbols icon foundation candidate

状态：`@material-symbols/svg-400` 与后续 `@material-symbols/svg-500` 仅保留为历史评估，均为
`rejected_as_final_icon_source`；两者的静态 SVG 固定为 `opsz=48`，不能提供当前 GUI 要求的真实 opsz 20/24。
当前生效合同见本节末尾的“Official vendored SVG replacement”。

推荐 exact dependency：

```json
"@material-symbols/svg-400": "0.47.0"
```

- placement：只作为 `packages/alcomd-ui` 的 direct production dependency；business page 不得直接 import
  `@material-symbols/svg-400`。
- registry live metadata（2026-08-26）：license `Apache-2.0`，零 dependency，零 install/build script，npm tarball
  1,915,158 bytes，unpacked 13,020,796 bytes，共 23,421 files；integrity
  `sha512-M3vW/MQCkr7NlN+1D9LDwRiKJeUjXbUB/O3gqPExNFAz1Vk/IFr0Ajlhr/lXVti46yhWknooVKAYMQj4o1VE3w==`。
- source/maintenance：`marella/material-symbols` 自动从 Google Material Symbols 更新并发布三种 style 的 weight-400
  SVG。它是社区维护的 npm packaging，不应描述为 Google 官方 npm publisher；自动更新频繁，因此必须 exact pin，升级重新审计。
- isolated lock probe：使用 `npm install --package-lock-only --ignore-scripts --no-audit --no-fund` 和 exact version
  得到唯一 package record；无 transitive closure、native addon 或 build script。若获批，预期 root `package-lock.json`
  只增加该 exact package record，三份 Cargo lock 不变；出现其他 package 必须停止。

### 建议冻结的 icon contract

- visual language：统一使用 **Material Symbols Rounded，weight 400**；普通 navigation/action 使用 fill 0，只有 selected
  navigation 或具有明确 on/off 状态的控件可使用对应 `-fill.svg`。不混用 Material Symbols Outlined、Lucide、Heroicons
  或第二套通用 icon language。
- ownership：`@alcomd/ui` 通过显式、静态、逐图标 import 提供 named icon exports/shared icon primitive；business
  page 只依赖 `@alcomd/ui`。不得建立 `string -> icon` 动态 registry、目录 glob 或 runtime path 拼接。
- rendering：优先以 `?url` 静态导入 SVG，并在 shared icon primitive 中使用 CSS mask + `currentColor`；不使用
  `dangerouslySetInnerHTML`，不引入 SVGR。装饰图标为 `aria-hidden`，icon-only control 的 accessible name 由 control
  提供。
- offline：所有 SVG 进入本地 npm/install/build graph；不得使用 Google Fonts、远程 stylesheet、远程 font 或运行时网络请求。
- size gate：只允许显式 import 实际使用图标；每次新增图标都应能从 `@alcomd/ui` 的静态 import 和 shipping bundle
  中审计。selected navigation 的 filled variant 是独立资产，也必须显式 import。
- sizing：navigation/dense control 默认 20 px，primary button/icon button 默认 24 px；本包 SVG 固定 optical-size 48，
  缩放后的笔画密度必须继续经过 real GUI visual gate，不把 package choice 当作视觉验收。

### Vite 与实际 bundle probe

当前 GUI 使用 Vite 7.3.6，现有配置无需 SVG loader。隔离 probe 以 `?url` 显式导入四个 Rounded SVG
（`folder`、`folder-fill`、`search`、`settings`）并用 CSS mask 渲染：

- `vite build` 成功，未新增 dependency 或 manifest 变更；
- 只处理四个 reachable assets，没有把 23,421-file catalog 打入产物；
- 四个 SVG 均小于 Vite 默认 4096-byte inline limit，因此成为四个 `data:image/svg+xml` URL；没有 emitted `.svg`；
- probe output 为 `index.html` 349 bytes 和 JavaScript 3,810 bytes（Vite 报告 gzip 1.73 kB）。

这里的保证来自 Vite static asset graph 与显式 import，不应称为 package entrypoint 的 JavaScript tree-shaking。
若改为 glob、dynamic registry 或 directory import，就失去“只打包实际使用图标”的可审计保证。

### Variable-font alternative

同版本的 font alternatives 均为 `Apache-2.0` 且零 dependency：

| candidate | Rounded shipping asset | unpacked package | conclusion |
|---|---:|---:|---|
| `@material-symbols/font-400@0.47.0` | `material-symbols-rounded.woff2` 567,372 B | 1,609,395 B | 即使只 import Rounded，也为固定约 554 KiB；仅 FILL 可变 |
| `material-symbols@0.47.0` | `material-symbols-rounded.woff2` 5,352,780 B | 12,918,328 B | 支持 FILL/weight/grade/optical-size，但当前 GUI 代价过高 |

当前 desktop GUI 的实际 icon set 远小于完整字体，因此 SVG candidate 在 offline、按需资产和 bundle 可审计性上更合适。
若未来图标数量增长到字体更有优势，必须以真实 shipping bundle 重新评估，不能现在预装完整字体。

### Rounded 与 v3 continuity

v3 readonly GUI 使用 Lucide line icons，视觉特征是轻量、圆润、未填充，并配合 rounded navigation container。
M7 不继续引入 Lucide；Material Symbols Rounded fill 0 比 Material Symbols Outlined 更接近 v3 的柔和轮廓，同时与当前
MD3 rounded surfaces 连续。selected navigation 可用同名 fill 1 增强状态，但普通 action 保持 fill 0，避免界面从 v3 的
轻量 icon density 突然转为大面积实心符号。

Rounded/Outlined 的判断是视觉连续性建议，不是像素复刻。最终 icon size、alignment、selected fill 与文本基线仍须在
Windows real GUI visual gate 中人工验收。

### 审批点

项目所有者已批准：

1. exact dependency `@material-symbols/svg-400 = 0.47.0`；
2. dependency placement 为 `packages/alcomd-ui`；
3. 上述 Rounded/weight 400、静态 named import、CSS mask、selected fill 和 offline contract。

安装前按 registry live metadata 重新核验 exact version、integrity、license、dependencies 与 scripts，结果与评估一致。
安装前根 `package-lock.json` SHA-256 为
`7e143c8ecd505befc9b42804f362489f2093e254c7b6bb221d9497ce043102c1`；安装后为
`48d32aae0ce5a290c0dc4ee2ff3c5baa3c3cc506a203ac5dd1a908c8f20f8aca`。新增 package record 只有
`node_modules/@material-symbols/svg-400` 0.47.0；`packages/alcomd-ui` workspace record 只增加同一 direct dependency。
没有删除 package record、版本变化、传递 dependency、native addon 或 install/build script。

供应链记录：source 为 npm registry package `@material-symbols/svg-400@0.47.0`，package repository 为
`marella/material-symbols`，icon origin 为 Google Material Symbols，license 为 `Apache-2.0`，lock integrity 为
`sha512-M3vW/MQCkr7NlN+1D9LDwRiKJeUjXbUB/O3gqPExNFAz1Vk/IFr0Ajlhr/lXVti46yhWknooVKAYMQj4o1VE3w==`。
未来任何 patch/minor upgrade 都重新执行 dependency audit。

### 2026-08-27 Official vendored SVG replacement

项目所有者纠正了 static npm SVG foundation：`@material-symbols/svg-400` 与
`@material-symbols/svg-500` 的静态 SVG 均固定为 `opsz=48`，不得通过 CSS 缩小、裁边或单图标补丁模拟
20/24 optical size。两项 npm package 均从 production dependency 与 lockfile 移除，不替换为固定 opsz=48 的
`@material-symbols/font-400`，也不建立 variable-font subset pipeline。

最终 source of truth 固定为 Google 官方 `google/material-design-icons` 提交
`e083cc60a0828fdd3b404cea0cb8a5b900e9c23e`。`packages/alcomd-ui/assets/material-symbols/` 只 vendoring
产品真实使用的 Rounded / weight 400 / grade 0 / fill 0 / opsz 20 或 24 SVG，来源路径和 SHA-256 记录在
`manifest.toml`。business page 继续只使用 `@alcomd/ui` named export；不接受 arbitrary icon string、asset URL 或 SVG
path，不使用 glob、dynamic registry、remote font/CDN、SVGR 或第二套 icon language。共享 mask primitive 保留官方
viewBox/path 与 `100%` geometry，不再裁切 optical keyline；默认 action/icon-button 与 primary navigation 为真实
24px/opsz24，table sort indicator 仅在显式传入 20 时使用真实 opsz20。primary navigation 曾短暂评估真实
20px/opsz20，但项目所有者在 release GUI 中观察其官方 keyline 后判定视觉过小，因此整体切换到 24px/opsz24，
并以 12px icon-label gap 保持原有标签起点；这不是 per-icon scale hack。
