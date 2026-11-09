# 软件包名称优先展示

日期：2026-11-09。所有者要求大部分面向用户的软件包引用使用名称而非包标识。范围见[M7展示合同](../exec-plans/M7-package-v3-parity-audit.md)。输入基于`2f838da2`叠加本轮差异。

## 修改

- 工作区已有Core Summary生成未经过搜索筛选的名称映射，交给软件包操作审阅复用；无额外名称查询、全版本预读、持久化或公共合同修改。
- 变更清单、依赖冲突两端、Unity不兼容确认与审阅警告、旧资产所属包优先显示展示名称。无名称回退原标识，变更与旧资产条目的标识保留在tooltip，列表副信息及明确的技术详情保持。
- 标识仍用于选择、RPC、Plan/Apply及React key；显示名称不参与业务校验或来源资格。100ms、按包准备版本、后台刷新与失败呈现保持。

## 验证

证据：`target/m7-package-display-names-20261109/`。

- 全部前端工作区检查通过（`check.log`），GUI生产构建通过（`build.log`），保留既有bundle大小提示。
- 42项现有浏览器回归通过（`browser.log`）：软件包workspace、v3 parity和presentation三个文件。同步原断言为名称优先，保留原请求标识、Cancel/Apply、能力、来源、搜索及刷新断言；涵盖未知依赖方标识回退、搜索隐藏后批量审阅及不兼容版本两次名称提示。本轮没有新增测试文件。
- Playwright CLI独立会话在合成SDK场景打开Base卸载审阅：变更显示“VRChat SDK - Base”，冲突显示“VRChat SDK - Avatars requires VRChat SDK - Base”，没有名称的依赖方继续显示其标识。截图`output/playwright/sdk-base-review.png`（相对证据目录）已实际查看，随后Cancel并关闭会话，未Apply。开发夹具仍有原Material/React ARIA与Lit提示、favicon 404，未改动这些无关问题。
- 仓库、元数据及差异检查通过，见`xtask.log`、`metadata.log`、`diff.log`。两个实现文件、两个回归文件及前端产物的摘要见`inputs.json`。

初轮只验证隔离浏览器夹具，未执行用户项目包操作、完整workspace/三平台Hosted或原生GUI端到端；Core、SDK及公共合同未变，未提交/推送，M7/H7边界保持。

## 所有者要求更新GUI

所有者随后要求“更新GUI，让我看看效果”。核对全部7项已验证前端/测试/产物摘要一致，独立构建嵌入生产前端的GUI通过，保留显式rustc输出路径的两条提示。证据与旧程序备份位于`target/m7-package-display-names-update-20261109/`。

- 已备份旧GUI并更新、重新打开：PID294004，SHA256 `A563B2D4B45D0C4E473282BB3A2DCE7CB35E65DB0E9880FABBEDA28A9F4C36FF`，窗口响应，原WebView profile已核对。
- Core PID285544及程序/启动参数保持，ready/State31，0活动任务；原2项目、2仓库、数据路径及设置摘要保持。没有更新Core或操作用户项目软件包。
- Computer Use的原生UIA读取确认项目列表及两项目正常。画面捕获与目标不符，不作为截图验收；置前失败后按规则刷新选择并重试，返回`user input was detected in this window`，后续只读状态确认`window is minimized`，停止自动输入并保留用户窗口状态。未自动进入管理页或执行包操作，不声称实际项目确认弹窗已经端到端验证。
