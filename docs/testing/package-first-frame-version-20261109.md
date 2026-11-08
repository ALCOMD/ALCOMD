# 软件包首帧版本文字修正

2026-11-09，当前M7软件包展示任务后续。所有者指出包已出现但首帧版本为空，随后才恢复。仅修改GUI Select的受控文字同步及相关回归，没有Core/RPC/Schema或权限变化，没有增加依赖、提交或推送。

## 原因

项目lockedDependencies与候选installed字段已经包含安装版本，packageWorkspaceRows同步生成installedVersion，PackageVersionPicker首个render已经传入displayText。可选版本目录仍按原规则在展开菜单时才读取，这不应影响已安装版本。

Material Select在没有选项时会在初始化及后续更新中将displayText置空。此前改回普通effect，只保证某些时序下最终恢复文字，不能保证首帧；上一报告中等待控件稳定后检查displayText和原生可访问值，不足以证明首次绘制正确。

现在显式displayText通过Select已有Lit宿主的公开controller生命周期保持受控：挂载时注册，在控件更新后恢复调用方提供的文字，值相同时不重复赋值，卸载或参数变化时移除controller。全部更新在绘制前的微任务中收束，不等待另一次React effect、Core读取或刷新。未传displayText的普通Select保持其选项选择行为；菜单值、用户确认、版本资格及固定计划校验不变。

## 验证

保留修改前生产构建作为独立夹具，在正常速度和4倍CPU限速下各进行3次新访问，每次采样列表可绘制后的12个requestAnimationFrame。旧版6次均在首个采样中出现两个空白文字；新版本6次的首个采样均为`1.2.3`及`Not installed`，全部72次后续采样无空白。这里记录的是绘制前DOM状态，不声称它是原生WebView的逐合成帧录像。

新增永久回归检查首次可绘制后的8个采样，并确认没有versions请求或projects.refresh。相关展示、导航100ms、来源隔离、版本菜单、操作确认和共享Material回归81/81通过，零重试、零跳过；GUI类型/控件/本地化检查及生产构建通过。最初把controller方法加入通用事件元素类型导致TypeScript断言不兼容，改为Select专用类型后通过；生产代码运行行为相同。没有Rust业务修改，未重跑Rust workspace或Hosted CI/CodeQL。

本地证据：`.local/package-first-frame-version-20261109/`；浏览器附件在`apps/alcomd-gui/.local/package-first-frame-version-20261109/`。原生构建和最终进程/摘要按本地verification.json记录。保持100ms导航上限、后台完整准备后一次更新、原Core和数据/profile。

最终原生构建、元数据、xtask和diff检查通过。GUI已更新打开，PID266532，SHA256 `72D2D4F9E81AF46A2CAB455173663B8CFA6D88963A2F6D60E9BA25AA3EF0EF57`，窗口存在且响应；Core PID269324及启动时间不变。未进行本次原生首帧录像，首帧结论来自上述生产模式逐绘制采样，不用窗口启动成功替代首帧实证。

所有者随后确认“现在看起来正常了”，并确认保持现有设计、授权本地提交。当前文件、控件和页面读取源码摘要与上述验证一致；提交前工作区前端检查、Rust fmt、元数据、xtask及diff检查通过，未重复执行摘要未变的测试。保留React内部比对和当前后台刷新流程，不追加数据相同时跳过状态提交的优化。Core/协议与GUI分开本地提交，未推送或重启程序。
