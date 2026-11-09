# M7 50ms悬停与按下预取验证

2026-11-09；main基线 `b55f91187d9f94f511bc0b8f5f021fa235b176e2` 加当前工作区。所有者明确批准“50ms悬停＋鼠标按下立即启动或接管预取”。范围为既有[ExecPlan](../exec-plans/M7-progressive-loading-preload.md)内GUI交互，不改Core、RPC、数据库、权限或依赖。

## 行为

- 共享悬停阈值从80ms改为50ms；键盘聚焦仍立即准备。
- 主指针的左键/触摸按下立即准备；复用已有请求，将尚未启动的准备提升为前台，不受两个推测性任务名额阻塞。前台准备保留资源有效性检查。
- 主导航、只读导航按钮和分类切换自动复用共享hook，版本选择器显式接入按下与取消事件。按下本身不导航、不打开菜单、不发起Plan/Apply。
- 右键、中键、非主指针与禁用控件不因按下触发准备；取消按下释放指针保活。悬停、焦点和已打开交互共同保活，全部离开3秒清理，点击复用、迟到隔离和100ms导航/展开上限保持。

## 实际验证

证据目录：`target/m7-preload-press-20261109/`。

- `npm run check`：全工作区通过（`check.log`）；补充队列/禁用控件断言后再运行通过（`check-final.log`）。包含TypeScript、Material控件边界、文案和35份图标边界。
- `npm run build`：全工作区通过（`build.log`），对应最终生产源码；保留既有大于500kB的bundle提示。
- 新增用例首轮定向运行8项全部通过（`browser-new.log`）。使用暂停的浏览器时钟验证悬停49ms不读、50ms开始，20ms路过离开不读，10ms时按下即开始；按下前后没有焦点/导航变化，点击复用同一挂起请求。
- 完整回归命令：`npm run test:browser --workspace @alcomd/gui -- m7-navigation-loading.spec.ts m7-package-navigation.spec.ts m7-package-candidates.spec.ts m7-material-foundation.spec.ts m7-settings-sections.spec.ts m7-resources-lifecycle.spec.ts --workers=1 --reporter=line`。**130项全部通过**（`browser-regression.log`），包含8项新增用例；重复运行不另计用例数。
- 新增覆盖导航鼠标/触摸按下、版本选择器鼠标/触摸按下、Manage按钮与资源分类按下、非主键/指针忽略和取消后过期；现有队列回归新增前台准备与点击接管同一请求断言，现有证据过期回归新增禁用选择器按下不得产生请求断言。既有回归覆盖键盘、菜单分页、来源变化、过期/迟到、3秒保活、100ms边界及资源生命周期。
- 最终生产源码与测试摘要保存于`final-inputs.json`，供后续核对。`git diff --check`与`cargo xtask check`通过，见`final-gates.log`。

## 限制

本轮使用现有Chromium合成客户端；触摸路径通过PointerEvent模拟，未用触屏硬件验证。没有视觉/布局改动，未新增人工外观签收。未运行Rust测试、Clippy或三平台Hosted CI；本次不含Rust实现。未操作真实项目、提交/推送或替换运行GUI/Core；运行中的程序尚未包含本次修改。M7/H7保持。
