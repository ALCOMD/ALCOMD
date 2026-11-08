# 软件包表头提前显示与加载样式（2026-11-09）

按所有者要求，软件包数据尚未返回时，表头随项目标题和工具栏提前显示。只让表体等待；全选在没有可见包行时禁用。复用 DesktopList 的 emptyMessage 和现有 Material Progress，在表头下显示全宽 4px 线性进度条，去除重复的可见“加载中”文字，保留状态播报。

导航共用的100ms预算、局部指示的160ms延时、请求/分页/权限及后台完整结果一次替换逻辑保持。已有行的后台刷新不会呈现空表体或加载条，无新增依赖或公共合同。

验证输入为 main 的未提交工作树，仅本项四份GUI源/测试文件改变软件包加载呈现；此前Core、原生连接池及PageRead修改保留。

- `npm run build --workspace @alcomd/gui`通过，包含TypeScript、Material边界、本地化检查及生产构建；保留既有bundle大小提示。日志：`target/m7-package-loading-style-20261109/build.log`。
- 现有导航和软件包工作区浏览器回归 **39/39** 通过，零重试、零跳过。更新提前表头的预期，保留100ms边界、快速读取、迟到响应、后台刷新、筛选及计划/确认/失败断言。日志：同目录`browser.log`。
- 独立生产模式Edge夹具对照：旧版加载时无表头，新版6列表头先行、0包行、全选禁用。读取完成后仍为同一个thead，位置/尺寸均不变（1280窗口下x284/y172/960×48），2包行正常到达，无pageerror。表头下进度条960×4。证据：`output/playwright/package-loading-style-20261109/{before,after}.log`及PNG。
- 深色1280、浅色900窗口及减少动画设置核对通过；窄窗口保留既有880px表格最小宽度和横向滚动，未改变列宽策略。证据：同目录`themes.log`及`after-dark.png`、`after-narrow.png`。

未修改Rust，不重跑Rust/完整工作区或Hosted CI。本项未替换或重启运行中的原生GUI/Core，截图为生产浏览器夹具，不记作原生交互或H7人工签收。未提交、推送。

## 下圆角裁切纠正

所有者截图指出加载条只有下方圆角。上轮只核对进度条尺寸及表头位置，漏查了祖先单元格的裁切。实际是通用表格末行规则的特异性覆盖了加载行的`border-radius: 0`：顶部两角0px、底部两角16px，导致4px进度条被裁切。提高现有加载行选择器的特异性，使其四角均为0px；只作用于包含加载条的空单元格。

独立浏览器对旧生产夹具和当前开发源码作浅色/深色/900px窄窗口核对：修复后加载单元格四角均0px、4px高度和表头宽度保持，数据返回后正常末行下圆角仍为16px，无pageerror。局部截图与计算值见`output/playwright/package-loading-style-20261109/corners*.{log,png}`。这次仅修改一条CSS选择器及说明，未重跑前轮39项功能测试；前端检查/生产构建单独记录在`target/m7-package-loading-style-20261109/corners-build.log`。未更新运行中的原生GUI。
