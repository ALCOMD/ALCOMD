# M7 完成记录（2026-11-13）

M7官方GUI与Portable Extension UI已完成收尾。所有者于2026-11-12认可整体使用体验，随后明确授权全部门禁通过后完成M7收尾、提交及正常推送完成记录；本记录落实该条件。H2–H6已有签收和保留边界沿用，不重做逐页签收，也不把自动化结果写成新增原生人工操作证据。

最终功能候选：`c46f7544a5400fc136f3df6eb507e5a2e8626036`。下列门禁绑定该候选，完成记录是后续独立文档提交，不声称文档提交的新CI已完成。完成该候选时M8尚未开始；后续正式交接见下，没有发布、打标签、强推或修改GitHub设置。

## 正式验收收束

2026-11-13，所有者要求独立验收后，明确授权修正文档残留、本地提交并正式收束 M7，随后进入既有 M8 范围。
独立核对确认 `c46f7544` 的三平台 CI/四语言 CodeQL 全部通过，四份完整浏览器日志均为700项通过；249份未变 Rust/构建输入支持复用901项通过、14项既有忽略。
在 `e9879837c100fb2d79f46038e1d1820bd2c6b1c3` 上重新执行最新缺失目录/局部反馈28项、GUI check、metadata与diff检查，全部通过，994份跟踪文件在验证前后不变；八份部署组件与已验证产物摘要一致。
此次不把当前增量写成已跑完整703项或新三平台门禁，不增加原生人工操作声明。将完成定义中的旧Config3/Activity/Diagnostics与覆盖规则中的“全部日志撤销”同步到现行Config13、`logs.read`；保留以下原始失败、忽略和未覆盖边界。
M7正式验收完成，M8已获所有者授权进入能力核对与缺失实现；本次仅本地提交，不自动推送。

## 最后交付的修正

- 获批的可省略directoryState由Core仅检查项目根目录，区分available/missing/unavailable，不遍历内容或增加登记revision。缺失项目列表/卡片灰显；复制缺失源明确返回project_copy_source_missing，错误位于操作按钮前。无数据库Schema或权限变化。
- 备份新默认为真实等级5，新归档使用项目目录名和本地创建时间，保留中文；文件名随归档证据冻结，碰撞不覆盖。显式旧设置、旧UUID归档及恢复定位保持，未重命名或重写真实旧ZIP。
- 修正页面接管后旧按钮的焦点/指针事件再次创建预取：同一交互共享已交给页面的请求，结束交互或新访问重新读取。50ms悬停、主按压立即预取、100ms切页及原失效/并发边界保持。菜单测试仅同步Material公开生命周期，产品快速重开行为保持。

其他M7合同与交付见[实施索引](../exec-plans/M7-official-gui-extension-ui.md)和[收尾计划](../exec-plans/M7-final-closure-20261112.md)。

## 最终门禁

| 范围 | 实际结果 |
| --- | --- |
| Rust fmt、严格workspace Clippy、workspace tests | 未变Rust源码/测试/锁文件/构建配置按逐文件摘要复用9f47336b通过结果；901项通过、14项原有忽略 |
| GUI Rust检查、Discord检查、CLI构建/自动启动 | 对应未变输入和程序摘要已核对；Discord测试本身0项，不代表其业务完成 |
| 元数据、12项反例、基线冻结 | 当前metadata/xtask新跑通过；未变合同的反例与冻结检查复用原生基线 |
| 前端check/build、release bins、Tauri no-bundle | c46f7544重新执行，全部通过，未复用旧GUI产物 |
| 本地完整Chromium | 700项通过，0失败、0重试、0跳过 |
| 三平台Hosted CI | Ubuntu 22.04、macOS 15 arm64、Windows Server 2025全部成功，各完整Chromium700项通过 |
| CodeQL | 同SHA的Rust、JavaScript/TypeScript、Python、Actions四语言全部成功 |

官方结果：[CI 43281957555](https://github.com/ALCOMD/ALCOMD/actions/runs/43281957555)、[CodeQL 43281951187](https://github.com/ALCOMD/ALCOMD/actions/runs/43281951187)。已下载核对三平台完整日志及全部必需步骤。

993份跟踪输入最终逐文件一致。相对原生基线9f47336b，28份允许差异仅为2份GUI预取实现、22份浏览器测试/助手和4份文档，965份输入不变；复用不包含已变化的前端与GUI发布产物。14项原有`#[ignore]`未全部直接单跑，部分子进程入口由父回归调用；联网VRCFury、Windows单实例/注册表、显式公开仓库快照和专用HTTP/代理隔离项保留各自执行条件，不计本轮直接通过。

## 原实例更新与保全

门禁通过后公开RPC确认0活动任务，正常关闭准确原GUI134976；原Core79012在1025ms内自动退出。离线备份旧程序、36份data和342份profile文件，逐文件核对原树与副本后才替换程序。未恢复用户已缺失的项目内容。

沿原data/profile与既有启动入口恢复Core127300/GUI131944，GUI连接并响应，ready/State32/Config13、0活动任务。完整设置revision8保持；原4项目、30仓库、5模板、1项Unity登记、0扩展、旧备份记录/locator和12个项目标志检查位保持，目录状态为2个available、2个missing。原删除仍为failed/project_delete_source_missing，revision与完成时间保持；原3个Unity/Hub进程保持。

八个正常运行组件均与已验证发布产物摘要一致，独立旧迁移工具继续隔离。Core及六个辅助程序字节未变，本次实际改变GUI。

- Core SHA-256：`D06CC2959B181F22F364854188B86B15A15BCC902EA1316DEBBDC5B1353E9069`
- GUI SHA-256：`A552BD55B628CBE09F48640AA4A79E1F0410281970D9FB90994D0CF9076D795B`

恢复子进程已退出并写完结果，但外层日志观察器因长运行程序继承输出句柄继续等待；仅停止准确识别的观察进程，没有停止恢复后的应用，该观察器不计成功退出。另以独立公开RPC读取和只读保全复验取得实际0退出码，再核对八个程序及应用响应，作为实际交付依据。

## 失败记录与未覆盖边界

此前失败、中断、资源不足和生成目录恢复异常均保留。5bd7fd0b、af528c7a不能计为三平台通过；后者证明仅等起始页就绪未解决竞态。实际预取故障在12倍CPU/双worker下6次中1次重现，交接断言修复前失败、修复后通过，同条件修正后6次全部通过。诊断日志/节流已移除，原读取次数、业务与安全断言保持。详见[交接修正](../exec-plans/M7-final-closure-20261112.md#预取接管后的旧交互重复读取)。

本轮未恢复已停止的桌面自动控制，未新增原生交互截图；程序响应、IPC和浏览器fixture不替代原生人工交互。Portable UI原生表单、超时及外部断线等旧未覆盖范围保留。MCP、Local API及Discord正式业务不因占位程序构建成功而完成。v3真实工程差分仍属M11，原生安装/更新/卸载、Windows 10/11客户端发行及真实跨平台读屏等仍属M12；本机开发实例替换和Hosted服务器门禁不替代这些验收。

本地完整证据位于`target/m7-signoff-preload-handoff-20261113/`，含逐门禁退出码、输入/复用证书、三平台完整日志、CodeQL及`final-acceptance-certificate.json`。原生基线位于`target/m7-signoff-missing-directory-20261112/`；故障前后对照和失败trace/截图位于`target/m7-signoff-resource-ready-20261113/`；本次离线备份、前后状态、独立保全复验与观察器限制位于`target/m7-handoff-runtime-update-20261113/`。
