# 软件包管理v3行为对照与本机更新

2026-11-08，基于`4206ad38063b080caad309b762d8d49b4817d7f0`后的未提交工作树。所有者要求整体核对软件包页面，并明确批准[对照计划](../exec-plans/M7-package-v3-parity-audit.md)中的手动Unity兼容性窄合同。本轮未提交、推送或改写历史，M7/H7人工状态保持。

## 实际改动

当前版本仅展示实际安装版本或未安装，撤回标记随当前版本显示。展开菜单使用纯版本号，每个版本一行，真实当前版本只出现一次，点击当前版本无操作。兼容版本直接列出；不兼容和Core未知证据使用非选项标题及分隔线。来源消歧仍由既有显式入口完成，GUI不替Core挑选仓库。

升级按钮及目标版本回到最新列；行尾为已安装包的直接移除或未安装包的直接安装。更多菜单依次为重装、更新日志、文档。单来源显示名称，多来源保留完整提示和已有来源选择。补齐Shift范围选择，保留搜索隐藏项、Core批量资格及单个原子Plan。版本/资格技术说明进入提示，不再混入选项文字和额外行。

`allowUnityIncompatible`默认false，仅三个既有单包Plan v2接口允许用户确认后指定精确根目标。Core提供`manualUnityOverrideAllowed`，GUI取消不创建计划；确认意图进入既有formatVersion 4的可选`unityCompatibilityOverrides`和计划摘要。依赖、撤回、范围、来源、权限、过期及冻结Apply继续校验。无新数据库结构、迁移、权限或生产依赖，State31保持；闭合DTO扩展需要Core/SDK/GUI配套更新。

异步版本菜单的焦点可能仍停在触发字段，已通过Material正常focusout路径处理该处Escape；选项内部键盘事件继续交给Material处理。没有访问其私有open状态或引入假当前版本占位。

## 验证结果

- 浏览器最终相关范围104/104通过，另排序/100ms导航15/15通过，共119项，零自动重试、零跳过。覆盖版本菜单、分组、撤回、来源、搜索/过滤、已安装样式、直接操作、批量、Shift选择、取消零计划、确认后精确请求、键盘/焦点、共享Material和设置交互。最终菜单及工作区截图已查看。
- 受影响Rust库：Application 76、Protocol 17、Store 88、VPM 169通过，VPM 3项既有显式忽略。Application/Protocol/Store复用首轮对应库完整通过结果；VPM初轮失败后已单独完整重跑通过。
- M4合同、P6闭包、候选合同、快照竞争、项目直接读取、legacy真实事务及手动兼容性集成32项通过。候选合同在最后增加负向形状向量后额外6项复验通过，非额外独立用例数。
- M4合同最终6项复验包含额外断言：公开Schema只能在批准的三个方法出现覆盖标记，Resolve/Remove/Apply不得接受。
- 真实RPC 7项通过，1项子进程入口按既有规则忽略并由父用例调用。新增用例经过默认拒绝、明确覆盖、公开计划证据、真实Apply安装；原事务中断/重启矩阵和候选RPC边界通过。RPC解析定向1项通过。
- 以上为390项不同Rust测试通过，4项既有显式忽略。严格Clippy覆盖受影响七个包及all-targets，fmt、xtask、元数据、前端全工作区检查/构建均通过。公开候选形状29项通过；39项语义向量引用清单仅检查引用，不计为执行通过。
- Core locked debug与Tauri debug no-bundle构建通过。未重跑完整workspace门禁、全部浏览器、三平台Hosted CI/CodeQL；本轮Windows定向结果不替代Unix原生或真实Unity Editor验收。原生窗口内部交互未自动操作，H7保持人工待定。

早期失败保留在`.local/package-v3-parity-20261108/`：新夹具缺少resolver-ready展示元数据、撤回版本已被筛掉与同来源歧义的测试预期、Material选项属性/空白和弹窗宿主定位、Schema同步范围、异步空菜单Escape，以及首次修正对原有选项Escape的影响。后续分别修正夹具、定位与产品焦点处理，保留资格/冻结/权限断言；最终结果以上述完整相关范围为准，没有删除失败用例或放宽安全条件。

## 本机更新与数据

更新前只读RPC确认无活动任务，核对原Core PID266476和GUI PID273676路径，备份程序和数据后关闭。新Core PID269324，SHA256 `8597568BC09E39D0EA0E54DA34605B866B82143D8E5414AAAE50E3C4C582DD80`；新GUI PID239804，SHA256 `BA748AC32DE2BA464D31AE6D56F64632D82FCEAF2D5F640E2C82E9E7A53281E0`，已打开且响应，沿用原WebView profile。Core实际RPC为ready、State31/Config12、2项目、2仓库、0活动任务及候选v2。

继续使用`target/m7-package-navigation-100ms-20261108/native-reopen/data`，未恢复旧数据库或修改真实项目。更新后34张表保持；项目、计划、操作、仓库软件包索引及配置/模板文件一致。自动仓库检查更新了仓库检查时间，新增对应幂等记录、事件和日志及序列号；这些属于已批准的后台刷新，不描述为所有数据库字节不变。

本地原始日志、前后只读RPC、表级比较、备份、输入与程序摘要见`.local/package-v3-parity-20261108/verification.json`及`native/`。浏览器截图/trace位于`apps/alcomd-gui/.local/package-v3-parity-20261108/`。上述本机窗口状态和浏览器夹具不代替真实原生页面内部的人工确认。
