# 刷新按钮圆圈反馈验证

2026-11-10，依据[实施计划](../exec-plans/M7-refresh-button-feedback.md)。基线 `978b1de7f7d8ea5bf80885f89a08afd8c059cc6f`，本项为其上的未提交 GUI 修改。

## 行为与范围

共用刷新按钮点击后立即替换为 Material circular progress，文字按钮在原有宽度内显示圆圈并保留可访问名称。请求期间持续显示，快速完成也至少保留300ms；不延迟请求、结果或错误。忙碌按钮保留原生禁用及重复触发保护，使用 Material 主题令牌让圆圈不受禁用图标/文字的低透明度影响，普通禁用状态保持。

覆盖软件包管理、仓库详情、用户包、两处 Unity 查找刷新及扩展权限。权限错误重试可能替换错误区，因此父级反馈同步到底部刷新按钮。Portable UI 普通 action 的按钮共用异步反馈，不按名称或 opaque actionId 判断刷新；只显示被点击按钮的本地反馈，保留既有全局忙碌限制。未改 Core、RPC、数据库、权限、依赖或真实项目。

## 验证与失败修正

证据保存在 `target/m7-refresh-feedback-20261110/`。

- 新增 `m7-refresh-feedback.spec.ts`：12项通过（`browser-final2.log`）。覆盖同步结果已呈现时首帧圆圈、固定宽度/名称、慢请求、重复点击、受控状态、即时错误、禁用、卸载迟到、键盘/减少动态效果、真实 Unity/Portable UI/权限入口、浅深色圆圈透明度及截图。
- 第一批与 Unity discovery、仓库列表、扩展工作区、H4 Portable UI 的回归中，57项既有用例及9项新增用例通过，1项新增权限错误重试失败（`browser.log`）。后续只调整忙碌状态的显示令牌、夹具和新测试，57项既有结果按未改变的行为复用。
- 最初的选择器错误分别来自 Material 将 ARIA 代理为 `data-aria-*`、影子 DOM 原生按钮与自定义 host 的差异、带空白的文字匹配；显式传递字符串 busy 值修正实际可访问状态，测试通过 host 检查圆圈、原生按钮检查禁用。未放宽业务断言。
- 权限错误夹具初用运行中替换 RPC 方法，无法影响已缓存的绑定方法；改在既有确定性客户端中用查询参数启用错误、显式事件恢复读取。最终保留“错误区消失后底部仍有圆圈”的原验收。对应失败保留于 `browser-final.log`，修正后12项全部通过。
- 全工作区 `npm run check`、`npm run build` 通过：`check-closure.log`、`build-final.log`。既有大于500kB产物提示保持。默认沙箱中的 esbuild 无法读取祖先目录，浏览器及生产构建改在获准真实 Windows 用户环境完成。
- `cargo fmt --all -- --check`、`cargo xtask check` 和 `git diff --check` 通过；无 Rust 修改，未重复 Rust 测试/Clippy、三平台 CI 或 CodeQL。

最终 `m7-refresh-feedback.spec.ts`、`m7-material-foundation.spec.ts`、`m7-package-workspace.spec.ts` 共59项通过（`browser-closure.log`），其中47项既有用例和12项新增用例；结合上轮未改变行为的57项既有用例，共116项不同浏览器用例通过。浅深色图标/文字按钮截图已查看，圆圈在300ms后仍清晰显示，完成后按钮恢复。最终截图复制到同一证据目录；`inputs.json` 记录收尾源码/测试/文档摘要与基线。

## 限制与交付

浏览器使用项目既有 React/Material harness 与确定性客户端；未更新或重启运行中的 Core/GUI，未进行原生 WebView 人工验收，也未操作真实数据。2026-11-12所有者随后授权本项本地提交，未推送。本项完成不代表 M7/H7 整体签收。
