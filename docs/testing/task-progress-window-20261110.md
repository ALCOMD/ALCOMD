# 任务执行窗口验证（2026-11-10）

范围见[实施合同](../exec-plans/M7-task-progress-window.md)。本次只修改 GUI 交互与测试；未新增 Core RPC、Schema、权限或依赖。旧有未提交修改保留，未写真实项目、替换运行程序、提交或推送。

## 行为

- 包操作、项目创建/复制/还原/删除、Unity 迁移、备份、扩展安装/卸载接受任务后交接至共享窗口。模板在原对话框连续显示进度，保留失败后的编辑入口。
- 显示 Core 阶段和不确定进度；结束显示结果，终态隐藏进行中的阶段。项目/备份/列表通过显式按钮打开。
- 最小化、Esc、查看任务后不自动弹回；任务中心仍能看到运行和完成。只对本 GUI 接受的任务显示窗口，其他客户端与历史任务不主动弹出。
- 进度读取失败重试原 ID，取消沿用原确认与修订号，未知写入结果仍不能重复提交。扩展详情在卸载后保留完成说明，避免自动导航或缺失详情错误。

## 验证与失败轮次

15 个受影响套件共 195 项不同用例分轮最终通过：任务进度、OperationFollow、项目创建/还原、模板操作/生命周期/对话框、备份默认值、扩展工作区、包候选、H3、H4、项目偏好、Material 基础、上下文错误及无障碍。

- 首轮 83 项：68 通过、15 失败。原用例仍要求自动关闭/跳转，且最初的统一模板交接丢失失败后返回编辑；恢复模板原连续窗口，保留错误内容，并将成功导航改为用户主动打开。日志 `target/m7-task-window-browser-first.log`。
- 第二轮 81 项中途停止，未计为全套通过。发现 Material 对话框操作区应从宿主定位、交接阶段重复旧进度和完成结果按钮选择器问题，修正后 9 项新增任务回归通过。日志 `target/m7-task-window-browser-second.log`、`target/m7-task-window-new-tests.log`。
- 195 项广泛回归：190 通过、5 失败。两项沿用旧包标识展示断言，改为显示名与标识 tooltip 同时验证；一个共享窗口定位和两处外层容器无尺寸的布局测量断言修正。日志 `target/m7-task-window-browser-closure.log`。
- 定向 83 项：81 通过、2 失败。卸载结果页返回按钮和完成窗口按钮同名，限定窗口范围；终态用例直接读取未创建的夹具任务，改为创建后显式推进终态。日志 `target/m7-task-window-browser-final.log`。
- 截图发现完成状态仍显示最后阶段，修正共享进度后复验 59 项：58 通过，上述夹具用例修正后单项通过（同一用例含失败与取消两种状态）。日志 `target/m7-task-window-terminal-final.log`、`target/m7-task-window-terminal-case.log`。
- 最终中文浅色 1280px、深色 640px 两项截图回归通过，覆盖执行与完成、操作区边界及终态不出现“正在解压”。截图人工查看，保存在 `target/m7-task-window-evidence/`，日志 `target/m7-task-window-visual-final.log`。

全工作区 `npm run check`、`npm run build`、`cargo xtask check`、`git diff --check` 通过。最终检查/构建日志为 `target/m7-task-window-frontend-closure.log`、`target/m7-task-window-build-closure.log`，仓库检查日志为 `target/m7-task-window-xtask.log`。生产构建仍提示既有大块体积提示，不影响构建。

最终源文件、测试及记录摘要见 `target/m7-task-window-final-inputs.json`。仅用浏览器夹具验证界面和真实共享事件入口；未实际更新用户包、创建/删除用户项目或对运行中的 Core/GUI 做原生操作。本轮无 Rust 变更，未重复此前的 Rust 套件。v3 只读核对最小化/保留后台任务行为，未复制源码。
