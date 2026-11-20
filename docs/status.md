# 项目状态

最后更新：2026-11-21

## 当前阶段

M11 仓库调序已在独立 `repository-order` 工作树实现。Windows 真实 IPC 用例受单实例锁阻塞。本地证据见[仓库顺序与来源优先级](exec-plans/M11-repository-order.md)。

**M10 已完成。** 签名候选 `dab76fcd` 的三平台 Hosted CI 和四项 CodeQL 均通过，Chromium 每平台 738 项通过。Rust 与 TypeScript SDK 共覆盖 94 个公开方法，含官方 GUI 的全部 93 个 Core 方法。三个平台的 npm 开发包已完成外部安装和真实 IPC 验收。[M10 验收记录](exec-plans/M10-native-sdk-and-rpc.md#2026-11-20-验收结果)

**M9 已完成。** 第一方 Discord 扩展已通过公开 Extension API 接通 Core 和 Host，并沿用 v3 的 Discord 应用身份。主仓候选 `feca8df3` 的三平台 Hosted CI 和四语言 CodeQL 全部通过，Chromium 各 738 项通过。独立扩展候选 `0ff04bc` 已通过三平台构建及 Actions、Rust 和 Python CodeQL。Windows 已完成真实 Unity 与 Discord 的 IPC 验收，Ubuntu 和 macOS 的真实桌面验收按计划延期。[M9 收尾记录](exec-plans/M9-discord-first-party-extension.md#2026-11-20-m9-收尾)

M8 冻结候选为 `622cb62a`，Windows、Ubuntu 和 CodeQL 已通过。macOS 未获运行器，按所有者决定延期补验。其他遗留验收项见 [M8 收尾记录](exec-plans/M8-mcp-protocol-and-management.md#2026-11-20-m8-收尾)。

2026-11-20 [三传输隔离夹具远端复验](exec-plans/M8-mcp-protocol-and-management.md#2026-11-20-三传输隔离夹具远端复验)。`622cb62a` 已按授权推送。自动 CI `44566846494` 首轮 `failure`，Windows 与 Ubuntu 全部门禁通过，各自 Chromium 734/0，Ubuntu 的 OAuth 综合和 Provider 均通过。macOS 因 GitHub arm64 运行器容量不足被取消，未执行步骤。同 SHA CodeQL `44566846484` 四项分析通过。macOS 同 SHA 验收延期。

2026-11-20 [Unix OAuth 夹具远端复验](exec-plans/M8-mcp-protocol-and-management.md#2026-11-19-unix-oauth-夹具远端复验)。`36137b6c` 的自动 CI `44551756595` 首轮总体失败，Windows 全部门禁通过（Chromium 734/0），macOS 和 Ubuntu 通过原编译与仓库复验，随后均在综合用例的 STDIO 探测失败。Ubuntu 另有六项 `AlreadyRunning` 和一项独立 Provider 计划失败。同 SHA CodeQL `44551755201` 四项分析通过。`78b90927` 修复三传输隔离端点，`622cb62a` 补充 Provider 错误码诊断；Windows 合计四个定向用例分别通过，Unix 复验依赖下一次获准推送。

2026-11-19 [M8 候选远端验收](exec-plans/M8-mcp-protocol-and-management.md#2026-11-19-m8-候选远端验收)。`83c02356` 已按授权推送。自动 CI `44531024198` 首轮总体失败，Windows 全部门禁通过（Chromium 734/0），macOS 与 Ubuntu 均停在 Unix OAuth 仓库测试夹具的编译错误 `E0509`。同 SHA 自动 CodeQL `44531022627` 四项分析通过。签名修复 `36137b6c` 只有一处夹具改动，双 Unix 目标最小编译验证和 Windows 真实隔离定向通过，真实 Unix 结果待新候选远端验证。

2026-11-19 [M8 本地候选验收](exec-plans/M8-mcp-protocol-and-management.md#2026-11-19-m8-本地候选验收)。代码 `522dabc5` 的 Windows Rust 整仓单轮 1096 通过、0 失败、18 忽略。严格 Clippy、原生 GUI 检查、MCP 管理扩展后端测试及前端检查和构建通过。前端完整 Chromium 730 通过、4 超时，四项原断言独立复验 4/0。待按新候选收齐同 SHA 远端技术结果，延期与未验证项见验收记录。

2026-11-19 [项目包观察和模板生命周期](exec-plans/M8-mcp-protocol-and-management.md#2026-11-19-项目包观察和模板生命周期)。`e7c2c526` 接通项目包实际状态、模板详情与编辑，`539d8185` 接通模板创建、导出、移除和同键恢复。最终真实 Windows 隔离 Core 定向 2/0，验证移除提交后回包截断、同键找回原回执、同名重建后保持新记录。共享 RPC Schema 53/0。完整本地验证和远端状态在同一节按候选记录。

2026-11-19 [Host 就绪修复](exec-plans/M8-mcp-protocol-and-management.md#2026-11-19-项目包观察和模板生命周期)。整仓首轮真实 Host 生命周期模块 8/4、1 项忽略，四项均超限。`7548e77c` 在组件准备完成后发送 Ready，定向模块 12/0、1 项原有忽略，独立 Component 模块 6/0。原执行预算保持。

2026-11-19 [MCP 连接级授权](exec-plans/M8-mcp-protocol-and-management.md#2026-11-19-mcp-启用并连接后使用工具)。`9235349e` 移除客户端逐项权限，`ce1e93de` 修复 HTTP 凭据在正文传输中被撤销时的响应。真实隔离 Core 已验证 STDIO、HTTP Bearer、HTTPS OAuth 的完整工具目录一致，以及身份隔离、撤销、原键恢复。各轮结果见 ExecPlan。新增候选没有远端结果。

2026-11-19 [当前能力与证据清单](exec-plans/M8-mcp-protocol-and-management.md#2026-11-19-当前能力与证据清单)。Windows Cursor CLI `2026.11.17-f532c45` 在隔离配置下实测发出旧握手，本版本与冻结 MCP 协议不兼容。`06dd8b03` 修复独立工作树导致的仓库门禁误报，原主仓检查与负面探针已通过。新候选尚无远端结果。

2026-11-19 [设置、Unity 和项目路径对齐](exec-plans/M8-mcp-protocol-and-management.md#环境读取按三端权限对齐)。`3ec540fe` 接通既有读取 RPC 和权限，授权后返回与本机入口一致的环境设置、安装路径与完整启动参数。`24c4f36d` 将 Core 的同一项目登记路径返回给有 `projects.read` 的客户端。真实 Windows 环境读用例 1/0，项目读取与卸载综合首轮 0/1、最后复验 1/0，原隔离窗口和五组公开状态已恢复。当前提交尚无远端结果。

2026-11-19 [项目与备份风险复核](exec-plans/M8-mcp-protocol-and-management.md#2026-11-19-项目与备份风险复核)与 [Claude Code 接入](exec-plans/M8-mcp-protocol-and-management.md#2026-11-19-claude-code-回环接入)。`ec99aa05` 修正项目错误映射和 Schema，真实 HTTP 回包截断、OAuth 授权上限与实时撤权分别 1/0。`78ef488` 的在途请求取消保全备份和原 STDIO Apply 风险复验各 1/0。`17dc4a65` 的 Claude Code 2.1.327 隔离配置、真实回环连接和移除用例 1/0。同名拒绝覆盖，无关配置保留，原隔离窗口和五组状态已恢复。早期夹具失败保留。整仓 xtask 因本机嵌套 worktree 路径产生 18 项失败，对应新提交尚无远端结果。

2026-11-19 [项目与备份本地实现和验证](exec-plans/M8-mcp-protocol-and-management.md#项目与备份本地实现和验证)。已批准合同由 `aa3a8c5f` 接通。真实 Windows typed RPC 和冻结 SDK HTTP 各 1/0，MCP 单元 16/0，定向 Store、Core、管理扩展及严格检查通过。恢复了原隔离窗口，五组公开状态一致。新流程的 OAuth、在途取消和真实写响应丢失未再执行。本地候选尚无对应远端结果。后续公共差异见[合并合同草案](exec-plans/M8-mcp-protocol-and-management.md#下一组合并公共合同草案)。

2026-11-19 [项目详情、模板与环境审计和 Codex 接入](exec-plans/M8-mcp-protocol-and-management.md#2026-11-19-项目详情模板和环境接口审计)。`84a2e53f` 增加管理扩展只读接入提示。隔离 Codex CLI 配置实测添加、同名替换和移除。扩展单元 9/0、严格检查和 Wasm 构建通过。Claude Code、Cursor 和自动配置仍待验证。

2026-11-19 [包计划、任务分页与仓库管理本地候选](exec-plans/M8-mcp-protocol-and-management.md#2026-11-19-包计划任务分页与仓库管理本地候选)。`c3e7912a` 接通通用包计划、任务列表和共享仓库管理。最终真实 Windows Core 的包/任务、仓库 OAuth 定向各 1/0，首轮夹具失败分别保留。原生仓库和授权分页回归、MCP 单元、管理扩展单元及 Wasm 构建和受影响严格检查通过。原隔离窗口及五组公开状态已恢复。后续项目、备份、模板和环境仍在 M8，未推送，无对应远端结果。

2026-11-19 [包 Apply 与任务权限整改](exec-plans/M8-mcp-protocol-and-management.md#2026-11-19-包-apply-与任务权限整改)。`dbda0679` 统一三端的包 Apply 和任务取消权限，查询保持独立授权，任务状态可跨业务读取。真实 Windows Core 首轮四项各 1/0，OAuth 综合首轮 0/1，更新过时目录断言后单项 1/0。MCP 单元 15/0，受影响严格检查通过，原隔离窗口及五组公开状态已恢复。包计划、任务列表和仓库的新公共差异集中待审。未推送，无对应远端结果。

2026-11-19 [MCP 与 GUI、CLI 一致性审计](exec-plans/M8-mcp-protocol-and-management.md#2026-11-19-mcp-与-guicli-一致性审计)。`7e3efd24` 统一 Core 包来源规则，撤回独立认证来源授权草案。Windows 真实 Core 三项独立用例各 1/0，SQLite 定向 1/0，受影响严格检查通过。原隔离窗口已恢复，五组公开状态一致。[实际验证与限制](exec-plans/M8-mcp-protocol-and-management.md#2026-11-19-共享包来源验证)。包操作和任务权限差异、其余业务接线继续按现有合同边界收束。未推送，无对应远端结果。

2026-11-19 [包目录 OAuth 与 HTTPS 补验](exec-plans/M8-mcp-protocol-and-management.md#2026-11-19-包目录-oauth-与-https-补验)。`33aca62a` 修复新工具缺少 scope 被 HTTP 误判为 404，`d2cf20c2` 含测试预检诊断。最终真实 OAuth/匿名 HTTPS 下载和签名 Provider 合并目录各 1/0，MCP 单元 15/0、受影响严格检查通过。旧 Provider 预检失败原因仍未确认。原隔离环境已恢复，五组公开状态一致。无新远端验证。

2026-11-19 [首批包目录本地候选](exec-plans/M8-mcp-protocol-and-management.md#2026-11-19-首批包目录本地候选) `495cd4dd` 已接通查询、安装和重装。真实 Windows SDK/Core 包闭环 1/0，SQLite 来源定向 1/0。旧卸载及 HTTP 取消与写响应丢失分别通过。签名 Provider 安装预检两轮失败，独立 Host 预检可通过，Core 路径待定位。原隔离环境已恢复，五组公开状态一致。本地候选未推送，后续公共合同集中待审。

2026-11-18 [v3 MCP 能力对照与首批包目录合同](exec-plans/M8-mcp-protocol-and-management.md#v3-mcp-能力对照与首批包目录合同首批已批准)。以冻结 v3.4.0 的实际用户能力为下限，按可完成的用户流程验收。33 个旧工具用作查漏索引。所有者批准实施首批查包、包详情、安装、重装及原 Plan/Apply；认证来源、项目/备份、模板、环境与快速配置的公共差异集中记录。A-026 已撤销的七项日志能力单列。

2026-11-18 [订阅与管理扩展候选远端结果](exec-plans/M8-mcp-protocol-and-management.md#2026-11-18-订阅与管理扩展候选远端结果)。获批的 10 笔提交已普通快进至 `2735119f6245d7c97552b2c19c5f467c5c0ad4ae`。[CI 44300586408](https://github.com/ALCOMD/ALCOMD/actions/runs/44300586408) 首次自动运行整体 failure，Windows 和 Ubuntu 作业全部通过，Chromium 各 731/0。macOS 因没有 hosted 运行器接手而在执行前取消，仍缺该平台结果。同 SHA 的 [CodeQL 44300585894](https://github.com/ALCOMD/ALCOMD/actions/runs/44300585894) 四语言全部通过。仅收集自动运行，M8 仍在进行中。

2026-11-18 [订阅容量与独立寿命续验](exec-plans/M8-mcp-protocol-and-management.md#2026-11-18-订阅容量与独立寿命续验)。首次实际十分钟用例触发异常断开，修复 `1883dac8` 后在 599.043 秒取得正常协议终态，原 Bearer 仍可调用及重新订阅。真实容量、冻结 SDK HTTP/STDIO 和 OAuth 授权变动回归同二进制均通过，HTTP 单元 10/0。原隔离窗口恢复，五组状态一致。持续满载、TCP 发送缓冲耗尽及真实 Codex 推送消费未覆盖。

2026-11-18 [Core 随包初始化](exec-plans/M8-mcp-protocol-and-management.md#2026-11-18-core-随包初始化接线)代码 `6f9da743`。已接通一次性安装和零授权启用，重启尊重停用和卸载。真实 Windows Core/Host 签名测试包定向 1/0，最终 Store 4/0、受影响严格检查通过。原隔离窗口恢复，五组公开状态一致。M12 仍负责正式签名包、新配置资格和发行装配。扩展人工继续延期。

2026-11-18 [默认安装证据和订阅流接线](exec-plans/M8-mcp-protocol-and-management.md#2026-11-18-默认安装的持久证据核对)。State43 Store 已验证安装记录在卸载并删除数据后仍可用于判断是否已初始化。代码 `dccf953f` 在管理扩展活动页接通独立可选订阅流读取，Guest、合同、Wasm 和独立 Host 检查通过。所有者关闭旧隔离窗口后，真实 Windows Core/Host 定向用例 1/0，验证独立授权、订阅流展示及撤销后旧会话失效。原隔离窗口已恢复，五组公开状态一致。扩展人工继续延期。

2026-11-18 [订阅续验与默认分发审计](exec-plans/M8-mcp-protocol-and-management.md#2026-11-18-oauth-订阅自然到期)。本地测试 `af4ff5f6` 的真实 HTTPS/OAuth 自然到期用例 1/0，约 600 秒到期关闭订阅，旧 token 返回 401，新 token 可读取工具。实际 Codex 0.153.4 回环 Bearer 接入未创建订阅，撤销后主动查询同一线程会刷新目录。隔离窗口已恢复，五组公开状态一致。这一轮只新增测试和记录，未推送。

2026-11-18 本批 CI 阻塞已收束。已普通快进推送测试修复 48c6e534 和记录 2d6e4bb2，远端确认为 `2d6e4bb235bc40e85b93b4159e7a6df729d8602f`。同 SHA 的 [CI 44254367013](https://github.com/ALCOMD/ALCOMD/actions/runs/44254367013) 首次自动运行三平台全部通过，Chromium 每平台 731/0。[CodeQL 44254366154](https://github.com/ALCOMD/ALCOMD/actions/runs/44254366154) 首次自动运行四语言全部通过。扩展人工继续延期，历史 Host resource_limit 仍待定位，M8 保持进行中。以下保留此前各候选的结果。

2026-11-18 已按所有者明确授权将2818f978和91065849普通快进推至 `origin/main`。候选 `91065849a71e5b6d2c280dd809a2f250c97db03e` 的自动 CodeQL 44245716183 四语言通过。自动 CI 44245716844 的三平台仓库检查、CLI 集成及发布版和 Tauri 构建通过，Chromium 均为 729 通过、2 失败。两条旧用例仍从在线主入口寻找本地文件控件，已由 `48c6e5341bfde68e1a5d2e60f731f298244327b1` 同步为现行本地入口和确认流程。本机复现两项失败后，四个相关 Chromium 模块同轮 38/0，GUI 检查和 xtask 通过。所有者同时明确授权本轮同批 CI 阻塞的必要修复普通快进，待新候选自动验收。以下保留此前检查点。

2026-11-18 当前远端候选`9461c76f199b1261835ad51dae51d02e059614d2`的自动CodeQL44167329975四语言success；CI44167330779三平台均在扩展工具合同旧10项权限数量断言失败，各1/1，之前的原日志安全合同各3/0，后续阶段未执行。本地`2818f97845f70eb4a4fc654cbb3b03fccb9606d2`仅将旧数量改为完整13项精确目录，保留原ABI/权限/来源断言；本机先复现0/1，同一次关联静态合同四模块分别5/0、7/0、2/0、4/0及窄严格Clippy/fmt/xtask/diff通过，不拼作完整套件。前次检索只覆盖旧12项，遗漏更早10项，本轮已集中核对并运行相关静态合同。按所有者持续授权直接修复/提交/快进推送与收齐自动结果，不dispatch/rerun、不进M9；原环境未动，新修复远端结果待取得。以下为此前检查点，历史失败和resource_limit未定位风险保持。

2026-11-18 持续CI收束：已按本轮授权快进推送修复/记录至`223e8ce48cd7ebda747b3d01121166a07465c739`。同SHA自动CodeQL44156608097四语言success；CI44156608608三平台均在Store安装同意测试的旧12项断言失败，各15/1；此前Schema合同三平台各52/0，后续阶段未执行。新本地`80ebb2643543b4d801330d37e670727e536c025d`只同步这一行至13，原拒绝无副作用、逐项必需授权、幂等断言保持；先复现0/1，受影响模块16/0及窄Clippy/fmt/xtask/diff通过。已集中核对相关目录/数量引用，无其他旧12项断言，不改生产规则或运行数据；继续按授权快进推送并收齐自动检查，不dispatch/rerun、不进M9。以下旧失败与检查点保留。

2026-11-18 已按准确两笔授权普通快进推送至`337f49724a61c0c5af22a9eac294543bfc808318`并确认远端。同SHA自动CodeQL44151487149四语言success；CI44151487184三平台均因OAuth Schema旧直接$ref断言失败，Schema模块各51/1，后续阶段未执行；此前权限目录修复在三平台已通过。各平台生命周期11/0/1原有ignored、实际MCP业务12/0、STDIO12/0分别保留，不拼作全套通过。本地`7b1a4f0efaf8ee0d8db0a376656f57af45e36432`仅精确同步已批准的hello认证oneOf，先复现原失败，修复后整个Schema模块52/0及窄Clippy/fmt/xtask/diff通过，生产代码和合同未改。所有者已明确授权本轮必要CI窄修、验证、提交及普通快进推送不再逐笔询问；继续至CI无问题，不dispatch/rerun、不放宽检查、不进M9。原验收窗口及数据未动，新修复的远端结果待取得；详见[M8持续收束记录](exec-plans/M8-mcp-protocol-and-management.md#2026-11-18-本轮持续ci收束授权与oauth合同测试同步)。以下为此前检查点，旧失败保留。

2026-11-18 已按所有者对准确6笔的明确授权，将origin/main从e6d0b44b普通快进推至`a94428b3bfd86e7c8911b15c48168234ac69a6a6`并ls-remote确认，推送前工作树干净、969份验证输入一致。同SHA自动CodeQL44146171781四语言全部success；CI44146171369已终态failure：三平台同一旧权限数量断言失败，m6_lifecycle各10通过/1失败/1原有ignored，后续阶段未执行；不记通过、不dispatch/rerun。失败为真实安装用例仍按12项断言权限目录，本轮已批准目录实际13项；本地修复`b9948171f236c15b465c2e71329a2c7d54e583fe`改为完整13项精确断言，原安装同意/撤权/未声明授权断言保持。真实Core单项1/0、窄严格Clippy、fmt/xtask/diff通过，不重复全量。旧隔离GUI84872/Core98828已恢复，五组公开状态和原36/346/10/303份文件及五程序摘要保持；不是新桌面构建。修复未推送、没有新远端验证，本次6笔授权已执行完；以下“未推送/旧远端”为此前检查点，M8仍进行中。详见[M8本轮远端与窄修记录](exec-plans/M8-mcp-protocol-and-management.md#2026-11-18-六笔推送与权限目录断言窄修)。

2026-11-18 OAuth订阅续验：仅测试提交`04a7fbec8bb6eff92217e53f4f25e00ff1b6adca`，生产代码与71a1ffe9相同。真实HTTPS/冻结SDK/Core具名用例首轮1/0，补齐实时授权收窄、token上限、refresh轮换及重放、Core凭据轮换、OAuth注册轮换/撤销的既有订阅证据；未用正常refresh错误推定旧access失效。没有新增合同/依赖/生产实现，不重跑完整套件。旧隔离GUI105140/Core105036已恢复、五组公开状态一致，原36/346/10/303份文件和五程序摘要保持，未恢复桌面控制。自然到期、长时压力、实际Codex通知消费、扩展人工、正式分发及历史resource_limit保留；新增提交未推送，远端仍e6d0b44b的CI失败/CodeQL通过，M8仍进行中。详见[M8续验](exec-plans/M8-mcp-protocol-and-management.md)。下方为前一检查点。

2026-11-18 A/B本地候选 `71a1ffe9a77d4fd109fe844c41b55461bcbd1ba6` 已接通目录指纹/冻结订阅、受限传输上报/真实身份关联/只读投影及独立Host、State43/第13项权限；未推送。末轮SDK真实STDIO/HTTP及票据回包丢失单轮3/0，新Core/Wasm/Host单项1/0、迁移3/0、MCP单元15/0、协议12/0、合同最终4/0；CI前置修复f8ad8f60的真实Core单项1/0。前端两模块10/0与另单项1/0分别保留，原失败轮次不拼为全套通过。远端e6d0b44b的CI仍failure、CodeQL四语言success，不代表新增代码通过。原隔离窗口/五组公开状态及原36/346/10/303份文件与五程序摘要保持，未恢复桌面输入或替换/升级原环境。扩展人工继续延期；长时/压力、OAuth订阅专项和历史resource_limit等缺口保留，M8不宣布完成、不进M9。详见[M8本轮记录](exec-plans/M8-mcp-protocol-and-management.md)。

2026-11-17 本轮明确批准后，真实Windows用户核对干净main、准确20笔链和实际远端，已将717d3e0普通快进推送至e6d0b44b953bbd0d3e404555d3e6875e94e2e9a3并ls-remote确认；独立草案bba8b905没有推送。同SHA自动CI44117137295已failure：三平台同一旧用例各9通过/1失败/1原有ignored，后续检查未执行；CodeQL44117136645四语言全部success。未dispatch/rerun。失败定位为安装同意已授予必需权限，但旧负向用例未先撤销，正在保留原拒绝断言并修正前置。所有者同时明确批准A/B，现开始目录指纹/订阅、受限传输观测与State43/只读Host的接线和定向测试，无需重审身份或这批合同；新代码仍仅本地，不在e6d0b44b远端验证范围。扩展人工继续延期，不操作原环境/数据或恢复桌面控制，不进入M9。下方“待审批/未推送”是此前检查点。

2026-11-17 待审差异已整理：现有推送前缀冻结为`e6d0b44b953bbd0d3e404555d3e6875e94e2e9a3`（20笔），下一批草案另作本地文档提交，不包含在此前缀内。A为工具目录指纹与冻结toolsListChanged订阅；B为受限本机上报、Core认证关联及独立只读传输投影/Host，新增订阅读取权限及仅扩CHECK的State43。只读审计已核对精确冻结Schema/SDK与现有目录指纹，未改公共接口、Schema、迁移、权限或生产代码。既有其余业务用例的外部Principal/来源委托缺口另记，不夹入A/B；扩展人工继续延期、不进入M9。具体字段、可信度、限额和测试见[集中审批差异](exec-plans/M8-mcp-protocol-and-management.md#下一批最小合同目录变化订阅与传输观测2026-11-17已批准实施中)。本轮“批准继续”按本地审计/准备执行，推送和新增公共合同仍待具体批准。

2026-11-17 推送准备：现有代码/证据候选`edd2927213c7153dd7cbed4de45c81ff10131b4c`相对实际远端717d3e0为19笔线性提交、124文件，均属已批准M8。工作树原本干净；fmt/xtask/locked metadata/diff再次通过，无workflow变更、包版本升级或无关内容。828份已跟踪非Markdown/非docs输入摘要`78865250244ccfe44b3ecd715bcb79848368266222514749520e05f8ff486a1f`及完整链在target/m8-prepush-contract-audit-20261117/prepush-inputs.json。对照已有937/938份证据输入，Core相关运行代码与3d6c2a1b相同，GUI与fb72f0aa相比仅后续测试及README变化，按精确差异复用有效证据。仍保留Host resource_limit未定位风险、新增代码尚无远端验证及本候选未跑完整本地套件的事实，不能称全门禁通过。已完成现有候选的本地推送准备，**尚无本轮推送授权，未推送**；下一批传输观测/订阅合同只审计、不实施。详见[推送材料](exec-plans/M8-mcp-protocol-and-management.md#2026-11-17-现有候选的推送准备)。

2026-11-17 MCP真实客户端验证：所有者要求扩展相关人工测试延后到后续实际扩展阶段，当前先测MCP；未验证项保留，不算通过或豁免。本机Codex CLI0.153.4在一次性CODEX_HOME启用mcp_2026_07_28后，回环HTTP Bearer真实发现六工具并完成隔离项目读取→原卸载Plan→显式Apply→Operation成功→同键恢复→撤权拒绝（单场景13项检查）；预登记HTTPS/OAuth另一个独立场景9项检查通过，含真实Codex S256/本机审批/授权码交换及两项只读工具。OAuth仅使用已有公开测试证书及子进程信任束，无系统信任变更，不等同普通浏览器人工TLS接入；STDIO即使开该flag仍发送旧initialize并被拒绝，保留不兼容记录。Core/MCP复用3d6c2a1b及匹配程序，相关运行代码至611436c4无变化，无生产修复/新增合同/依赖。临时客户端、OAuth注册已撤销，测试监听退出；样例登记清理脚本误取revision后已按精确ID/目录身份补清理，原失败记录保留，最终0项目/0活动任务、样例文件保留。GUI85304仍连接Core47824，原36/346/10/303份文件摘要保持，未恢复桌面控制。详见[本轮证据](exec-plans/M8-mcp-protocol-and-management.md#2026-11-17-实际codex客户端接入与扩展测试顺序调整)。仅本地记录、未推送/无新增远端验证，不重跑全量，不进入M9。

2026-11-17 本地候选续查：`7aed2b368ad8f5e470afe46dfbb03b50020452d8`只同步一项仍使用旧入口/旧安装审阅的浏览器用例，保留并加强冻结身份、原始路径及拒绝后不Apply断言。修正前原模块3/1；修正后安装与在线来源两模块独立单轮12/0，前端检查通过，不拼为完整浏览器通过。真实Host旧管理Component单项1/0；历史resource_limit的2/2失败仍未复现/未定位，不记修复。fmt/xtask/diff通过；没有全量、release或新增远端验证。已只读确认远端仍717d3e0，至本代码候选17笔线性本地提交均属于已批准M8；完整链及日志/输入摘要见target/m8-candidate-audit-20261117/candidate-audit.json。GUI85304可见/响应、5条管道仍连接Core47824，五程序摘要及原36/346/10/303份文件摘要保持，未操作桌面输入或原环境。普通可信HTTPS原生安装和实际OAuth客户端接入仍缺外部源/客户端及受信任证书输入；完整传输/订阅观测和正式默认分发仍有合同/实现缺口，详见[本轮审计](exec-plans/M8-mcp-protocol-and-management.md#2026-11-17-候选续查与剩余接线边界)。未推送，不进入M9。

2026-11-17 当前GUI文案更正：按所有者反馈，将网址输入后的“读取清单”改为“继续”（en-US Continue / ja-JP 続行），移除原用户文案，保持“输入扩展地址→继续→选择版本→安装扩展”。只是呈现更正，没有新步骤、Core/RPC/State42/权限或校验变化。原相关来源模块单轮8/0、前端check/build及仅GUI debug构建通过；没有重复其他全量。当前GUI85304可见/响应并以5条自身管道连接原Core47824，保活助手已关闭，原data/profile及五组公开状态、安装/授权摘要保持。程序来源与摘要见现有review/candidate.json，代码仅本地提交，未推送；普通可信HTTPS源原生人工安装及原未覆盖项仍保留。以下953ce174和更早窗口为历史检查点。

2026-11-17 当前GUI窄修候选：`953ce174614b382762d0c0c0551d911efbcdb5fe`按所有者补充，工具栏“安装扩展”直接打开网址输入，右侧下箭头菜单仅提供“本地文件”；弹窗取消本地/在线分段。读取清单改到底部唯一主操作，读完才变安装；网址Enter正确触发当前步骤，保留IME。仅GUI/相关夹具和呈现说明变更，Core/RPC/State42/权限及签名同意边界复用3d6c2a1b。修复前具名0/1；首轮相关30/1定位真实Enter提交缺口，修正后四模块独立单轮36/0，另弹窗取消/焦点1/0，不拼接历史轮次；前端check/build、仅GUI debug构建、metadata/diff通过，无全量/Rust/远端重跑。当前GUI91416可见/响应并连接原隔离Core47824（3d6c2a1b，5条管道），data/profile沿用，前后五组公开状态和0安装/0授权完整行摘要相同；没有重启Core、修改原环境或恢复桌面输入。938份非docs输入摘要bf130488448aa500db6af3339c90862a110d387710affbb1f11feec3bc151feb及浏览器对比图见[当前入口](testing/m8-management-manual-20261115.md#2026-11-17-安装入口与弹窗窄修)。只本地提交，未推送/无新增代码远端结果，M8仍进行中。以下3d6c2a1b窗口为上一检查点。

2026-11-17 当前本地代码候选：`3d6c2a1b80a124e1c16794b4a303ac0d534791c7`完成已批准的单扩展源在线安装。匿名HTTPS单ID/多版本索引→签名.alcomdextension→原条款/必需权限确认→原Plan/Apply；State42冻结来源，变化先拒绝、取消准备忽略迟到结果，不提供升级/自动更新。最终真实HTTPS/Core用例1/0（含十次重定向边界、外部Principal拒绝、匿名头、摘要/身份/大小拒绝、同意/幂等/重启），原本地生命周期1/0；解析5/0、Schema1/0、M6合同5/0、State42最终3/0、相关重开6/0、包/签名9/0、缓存10/0且3项原外部夹具ignored、两浏览器模块单轮26/0。受影响全部targets严格Clippy、前端check/build、匹配五程序debug构建及fmt/metadata/xtask/diff通过；937份非docs输入摘要`221deb5697ab1c2b8bf3d9ab4a9a700cc8310b1cdbf2b32c78a54017b3794eef`及程序摘要已绑定，没有生产依赖/版本升级。未重复完整Workspace/Chromium/release/三平台。

新隔离窗口GUI95592已可见/响应，5条真实管道连接正常候选Core47824，启动助手已退出；独立State42 data/profile、自动发现关闭、项目/备份路径均限可丢弃目录，0安装/0授权/0活动任务。真实签名样例路径与启动入口见[当前人工记录](testing/m8-management-manual-20261115.md#2026-11-17-单扩展源在线安装候选)。没有把打开窗口当成原生在线安装通过：真实HTTPS链已自动验证，普通受信任HTTPS源的桌面人工安装仍待查看，没有安装系统测试CA或公开托管源。此前State41自有窗口只在0任务且精确归属后暂停，两轮测试后均先恢复原程序/数据并核对五组公开状态一致；最后切至独立新候选，旧data/profile/程序保留。原四组36/346/10/303份摘要保持，原开发程序原本关闭并保持、Unity未操作，未恢复桌面输入控制。未推送、无本批Hosted CI/CodeQL，旧717d3e0的失败CI及成功CodeQL不作新增代码通过；原资源限额未定位风险继续保留，M8仍IN_PROGRESS、不进入M9。下方e58及更早PID为历史检查点。

2026-11-17 当前GUI候选：`e58c0d58bdc259a70482c7f8bc22fc1a0c7b5a38`按所有者纠正，将权限管理改为“必需权限／可选权限／其他权限”三类，去除逐行重复类型标记，空组显示“无”。授权、安装同意、Core/RPC/State41均未改，沿用1c47a17的Core及技术证据。权限相关浏览器单轮19/0、前端check/build及仅GUI的debug构建通过，没有完整Rust/Chromium/release或远端重跑。现窗口GUI84624可见/响应、5条管道连接隔离Core83680；GUI在独立候选程序目录，数据/profile沿用，更新前后1扩展/1授权记录摘要完全相同。更新曾因旧GUI文件锁未能覆盖（旧文件摘要保持），60秒保活结束后Core87756空闲退出；已用原隔离程序与数据正常恢复，不宣称Core进程未重启。原开发环境和Unity未操作，不恢复桌面控制；未推送、无新远端验证，M8待人工查看。[当前入口](testing/m8-management-manual-20261115.md#2026-11-17-权限三分类更新)。

2026-11-17 上轮完整本地候选：`1c47a1793e92394bcc35fcfb01754fe6167447d6`已实现获批的完整12项扩展权限目录、已声明/未声明分组及显式授权、全部权限必需/可选声明、签名作者条款与两步安装同意、State41和事务初始授权。原声明上限已移除，原签名/来源/双Principal/撤销/Plan与Operation边界保持；不恢复发布者信任或逐项目授权。真实Core两项各1/0、扩展Store最终16/0、State41 3/0、相关旧迁移4/0及3/0、扩展事件4/0、包/签名18/0、Core授权与RPC合同定向通过；三浏览器模块最终单轮30/0，首轮26/4及中间7/0分别保留。受影响严格Clippy、前端check/build、fmt/metadata/xtask/diff通过，无完整Workspace/Chromium/release/三平台重跑。匹配五个debug程序及未改变WASM的签名样例已准备；**新GUI90176可见且响应，5条管道连接新隔离Core87756；保活已退出，State41、0安装/0授权/0任务与公开五组状态均已核对。** 当前Core已实际生成样例预览，尚未Apply，窗口留给所有者检查两步确认及12项开关；未恢复桌面控制。原data36/profile346及旧review10/303份集合/摘要保持。新增代码未推送、无对应Hosted CI/CodeQL；旧717d3e0的CI失败及CodeQL成功不作本轮通过。M8仍IN_PROGRESS，旧资源限额风险保留，不进入M9。[真实入口与精简清单](testing/m8-management-manual-20261115.md#2026-11-17-权限目录与安装同意候选)。

2026-11-17 上轮本地候选：Core只读观测代码`96c5322697ef8426fd96828c4c2d7af29c71b7bf`，有界失败诊断`2c4f2a1a9c124296caa5d3e1e6a29f598c3bbc51`，均未推送。已接通mcp.runtime.snapshot/capability、两项可选Global/*读权限、State40及独立Host/WIT，扩展页复用既有Portable UI“活动”。真实双客户端/授权/撤销/断线/重启及旧guest兼容原组合4/0；96同提交复验2/2，两个旧guest首开返回extension_resource_limit，新两项通过；诊断单项1/0、诊断组合4/0，**原资源失败根因仍未定位，不记已修复或豁免**。模型/DTO/Store/Schema/合同定向检查与受影响严格Clippy通过；当前LF迁移3/0、来源合同3/0。浏览器模块6/1的空白页Target crashed保留，精确行号单项复验1/0，不拼为7/0；一次正则未命中记未执行。未重跑完整Workspace/Chromium/release或三平台。匹配debug程序和129500字节签名参考包已准备；全新隔离State40数据经正式Core登记/安装，零授权可启用并实际打开Host页面。GUI88248可见/响应，5条管道指向隔离Core83476，窗口保留、人工待查看，未恢复桌面控制。原data36/profile346、旧review10/303份集合及摘要保持，原环境原本关闭并保持原状，不升级原数据、不操作Unity。详见[当前人工入口与保留风险](testing/m8-management-manual-20261115.md#2026-11-17-当前隔离窗口)。M8仍IN_PROGRESS，不进入M9。

2026-11-17 当前已推送：所有者明确批准225726b、717d3e0，真实Windows用户核对干净main、精确两笔链及最新远端后，已普通快进至`717d3e085b6ce29649633e24c08e732c2e172855`并ls-remote确认。[CI44019052791](https://github.com/ALCOMD/ALCOMD/actions/runs/44019052791)同SHA/attempt1终态failure：Ubuntu、Windows、macOS完整Chromium分别均714通过/1失败，唯一失败均为新增权限读回布局测试的开关初始位置基准；三平台Check、CLI自动启动、release/Tauri和锁文件/diff通过。新增用例在打开点击前测量，而真实点击会等待弹窗动画稳定，导致12.7–17.8px差异；本地cd9d009将原1px基准移至打开点击完成且写入已挂起后，保留原断言/超时，受控旧逻辑24px失败、修复1/0、无注入模块6/0。此测试修复未推送，无对应远端结果；[CodeQL44019052564](https://github.com/ALCOMD/ALCOMD/actions/runs/44019052564)同SHA/attempt1四语言全部success。原29/0、原例8/0及前端/build/metadata/xtask/diff证据复用，未重跑本地全量或手动workflow。所有者要求CI并行运行，继续已批准M8的独立工作：更新过时隔离人工入口，并核对[下一批Core实时观测最小差异](exec-plans/M8-mcp-protocol-and-management.md#下一批实时-core-观测最小合同2026-11-17已批准实施中)。本节Core只读观测合同其后已获所有者明确批准，已完成本地接线，当前结果见上方候选；新增capability须真实验证后开放，原环境/数据/Unity与桌面控制未操作，不进入M9。

2026-11-17 上轮远端候选：所有者批准f1f1027、0887106、6bbf876三笔后，核对真实Windows用户、干净main、完整链、远端及祖先关系，已普通快进e10bce7→`6bbf876fb5b2c413eaa50fba0f7fde1dcc3258e2`并ls-remote确认。[CI43915767271](https://github.com/ALCOMD/ALCOMD/actions/runs/43915767271)同SHA/attempt1终态failure：Windows完整Chromium714/0、18.0分钟，整个job成功；Ubuntu713/1、15.6分钟，唯一失败为连续切换项目读取权限时没有发出第二次撤销请求。两平台Check、CLI自动启动、release/Tauri及锁文件/diff通过。macOS job取消且零步骤，官方注解确认arm64托管runner容量不足、多次未获取runner，记未执行。[CodeQL43915766843](https://github.com/ALCOMD/ALCOMD/actions/runs/43915766843)四语言全部success；未dispatch/rerun，不用历史macOS通过替代。

上轮推送前本地记录：本地窄修`225726bc4ebb13156688d6b58588f19cadc29d1a`：原速复现6通过/2失败，失败只有第二次pointerdown、无pointerup/change及撤销RPC；权限读回移除开关使弹窗布局变化。相同Core/扩展的读取期间保留禁用行，避免重新定位；仍只允许当前归属读回后写入，更换Core/扩展不沿用旧快照，并将读取能力变化纳入归属。旧代码新布局单例1失败；修复后相关三模块单轮29/0（45.5秒），原失败例原速8/0（21.8秒），前端check/build与xtask/diff通过，locked metadata沿用相同依赖输入的有效结果，不拼成完整套件通过。Windows动画原例本轮通过，但上轮根因仍未知。新修复未推送、无对应远端/原生桌面复验；原Core/GUI、数据、Unity及桌面控制未操作。详情见[本轮记录](testing/m8-publisher-confirmation-20261116.md#6bbf876推送与权限读回布局2026-11-17)。M8仍IN_PROGRESS，不进入M9。

2026-11-17 上轮远端记录：所有者批准d667a01、e10bce7后，已由真实Windows用户核对干净main、最新origin/main、精确两笔链与祖先关系，普通快进dd5aa9b→`e10bce7b67e77ef76e2f7db9a7dce2b41f22c4fe`并ls-remote确认。自动[CI43885504578](https://github.com/ALCOMD/ALCOMD/actions/runs/43885504578)、[CodeQL43885504395](https://github.com/ALCOMD/ALCOMD/actions/runs/43885504395)均同SHA/attempt1、终态已收齐：Ubuntu完整Chromium713/0并整个job成功；macOS712/1，Windows710/3，CI整体failure；CodeQL四语言全部success。三平台Check、CLI自动启动、release/Tauri及锁文件/diff均通过，仓库焦点原失败例与两新增回归本轮三平台通过。历史原结果独立保留，未dispatch/rerun；只读状态端点EOF后用官方直接API继续收集同一run。

上轮推送前记录：本地权限修复`f1f1027819b78558d9ddc8e82863a1ccf99a8473`绑定授权快照到当前读归属，防止换代读取前暴露旧授权可编辑窗口；测试等待实际写入与Core读回，不把原生开关的乐观选中当成提交完成，原错误/权限/revision断言保持。动画诊断`088710612a7bb37fdaac7a759c3b626a87bdc586`仅核对原生媒体输入/事件并记录有界失败状态，**原Windows动画失败仍未复现、原因未定位，不记已修复**，生产动画逻辑未改。稳定本地输入的四相关模块单轮37/0、前端check/build及locked metadata/xtask/diff通过；旧失败、诊断与辅助测试设置失败分别保留，详见[当前记录](testing/m8-publisher-confirmation-20261116.md#e10bce7远端收集与权限读回2026-11-17)。新代码未推送，无对应远端/原生桌面复验，不用旧结果替代；未操作原Core/GUI、数据、Unity或桌面控制。M8仍IN_PROGRESS，不进入M9。

2026-11-16 上轮远端候选：所有者批准6ffde60、dd5aa9b后，核对真实Windows用户、干净main、最新origin/main及精确两笔链，普通快进e371d27→`dd5aa9bae0a4b04ff4f3dd51b1433a240f0be7cf`并ls-remote确认。自动[CI43865931992](https://github.com/ALCOMD/ALCOMD/actions/runs/43865931992)、[CodeQL43865930193](https://github.com/ALCOMD/ALCOMD/actions/runs/43865930193)均同SHA/attempt1，终态已收齐：Ubuntu、Windows完整Chromium各711/0并整个job成功；macOS710/1，CI整体failure；CodeQL四语言全部success。三平台Check、CLI自动启动、release/Tauri及锁文件/diff均通过，事件初始化回归及前两轮选择用例本轮三平台通过，历史失败保留；未dispatch/rerun。

上轮推送前定向记录：macOS唯一失败为仓库包浏览刷新后返回列表时的焦点丢失，已受控复现实际产品竞态：分页会话换代重建列表，旧组件局部焦点记录随之丢失。本地窄修`d667a012e601b729d220b4527cdc6948f7a0e1a0`仅在RepositoryPackageCatalog保留恢复意图，旧列表移除前捕获其实际焦点，不在用户已转向其他控件时抢焦点；不改通用分页/提醒框、RPC、State/Config或权限。受控旧行为1失败/1通过，修复后仓库模块单轮35/0（含两新增回归及原失败例）、前端check/build及locked metadata/xtask/diff通过。输入/构建摘要和本轮终态见[现有记录](testing/m8-publisher-confirmation-20261116.md#dd5aa9b两笔推送与仓库刷新焦点)。新修复仅本地，尚无远端/原生桌面复验；未恢复桌面控制，未操作原Core/GUI、数据或Unity。M8仍IN_PROGRESS，不进入M9。

2026-11-16 上轮远端候选：所有者批准56eed3f、e371d27两笔后，核对干净main、真实Windows用户、最新origin/main及精确两笔链，普通快进3752c08→`e371d27f961404a6606700ed87e4c01a8cfb6c6c`并ls-remote确认。自动[CI43851129715](https://github.com/ALCOMD/ALCOMD/actions/runs/43851129715)、[CodeQL43851129372](https://github.com/ALCOMD/ALCOMD/actions/runs/43851129372)均同SHA/attempt1、终态已收齐：Ubuntu710/0、macOS710/0并整个job成功；Windows709/1，CI整体failure；CodeQL四语言全部success。三平台Check、CLI自动启动、release/Tauri及锁文件/diff均通过，前两轮的压缩/来源选择用例本轮三平台均通过。Windows唯一失败是事件证据模块异步加载尚未完成时，初始化轮询直接读取window.eventLimits.coordinator而抛TypeError；不拼接旧平台结果或掩盖本轮失败，未dispatch/rerun。

上轮推送前定向记录：本地窄修`6ffde60599087e895cdaec86f5c2bfac9957e416`仅让原精确ready轮询等待异步证据模块定义，并补受控延迟回归：旧初始化逻辑下1失败，复现同一coordinator未定义异常；修复后事件模块单轮10/0（原9例+新1例），前端check、locked metadata/xtask/diff通过。缓存、限流、任务/断线断言及超时均保持，生产代码未改，未重复完整测试或构建。新修复与本轮记录仅本地、无对应远端验证；最新证据见[现有验收记录](testing/m8-publisher-confirmation-20261116.md#e371d27两笔推送与事件证据初始化)。下文为历史检查点；M8仍IN_PROGRESS，不进入M9，不操作原环境或恢复桌面控制。

2026-11-16 上轮两笔推送：所有者批准882eb3d、3752c08后，已核对真实Windows用户、干净main、最新远端及精确两笔链，普通快进e99cd2c→`3752c0870574536524d966c975e5c831a656f9ed`，ls-remote确认，无额外提交/强推/手动workflow。自动[CI43837500362](https://github.com/ALCOMD/ALCOMD/actions/runs/43837500362)与[CodeQL43837499791](https://github.com/ALCOMD/ALCOMD/actions/runs/43837499791)均同SHA/attempt1、终态已收齐：CI failure，Ubuntu和Windows各710/0、macOS709/1；CodeQL四语言全部success。前轮备份压缩选择用例本轮三平台均通过；macOS唯一失败是用户包来源选择后displayText仍为Multiple sources，未将不同轮次拼为完整通过。

上轮推送前定向记录：该来源用例同样缺少Material opened/closed同步，按同模块现有模式窄修`56eed3fd9d4cdbd0077d86b3c8bde6cc170277b4`，只改一项测试的等待，不改生产代码、真实交互、原断言或超时。相关模块单轮29/0，修复后6倍CPU/单worker四次4/0，前端check、locked metadata/xtask/diff通过。原例本机6倍CPU四次4/0，未复现远端失败，不声称捕获远端具体时序；修复仅本地、远端未验证。本轮结果与下一候选见[现有验收记录](testing/m8-publisher-confirmation-20261116.md#3752c08两笔推送与远端终态)。下文为各轮历史检查点，该轮远端为3752c08。M8保持IN_PROGRESS，不进入M9、不操作原运行环境或恢复桌面控制。

2026-11-16 上轮四笔推送：所有者明确批准b9f19d5、9c6f329、de8ead3、e99cd2c四笔提交，已核对干净main、最新origin/main、完整准确提交链和祖先关系，由真实Windows用户普通快进2a9d1233→`e99cd2c9d9254025eb8926c23ea2976f599e99fd`。远端SHA已实际确认，推送前xtask/diff通过，未重复本地完整验证。自动[CI43816630421](https://github.com/ALCOMD/ALCOMD/actions/runs/43816630421)与[CodeQL43816630482](https://github.com/ALCOMD/ALCOMD/actions/runs/43816630482)均为同SHA/attempt1；CodeQL四语言全部success；CI已收齐为failure：Windows成功（Chromium710/0）、macOS成功（710/0）、Ubuntu失败（709/1，唯一为备份压缩下拉框英文用例），不记整体通过；未dispatch/rerun。下文“尚未推送/无对应远端”为当时本地检查点，该轮结果以上述e99cd2c终态为准，不用2a9d123旧结果替代。

2026-11-16 上轮CI定向收尾（推送前记录）：本地测试修复`882eb3d99474158a7440dc8050ff3a5c8f64e276`仅给既有Material压缩选择用例等待opened/closed，保留全部实际交互、断言与超时；相关备份+motion单轮19/0、英文6倍CPU四次4/0、前端check通过。原例本机未复现，远端具体事件时序未捕获，不把同步候选写成已远端修复。三平台Check、release及Tauri构建已通过，前轮Host读取冲突不再出现；本轮Ubuntu709/1原结果保留。锁定metadata、xtask、diff通过，修复及本轮记录仅本地，超出已批准四笔推送范围，等待新的推送授权；无重复workflow或完整本地重跑。

2026-11-16 当前[发布者确认彻底删除](testing/m8-publisher-confirmation-20261116.md)：所有者已批准完整公共删除和State39，代码`de8ead33f789c1264df574c69ecae0fb5bf155bd`已本地提交。有效扩展包无需额外信任/发布者确认即可按原流程安装；publisherApproval、trustDecision、对应运行时类型/错误/GUI展示及两个数据库列已删除，不自动补批准、不保留旧输入兼容。严格签名/摘要/来源身份、原Plan/Apply、权限和留存数据的发布者身份隔离保持。旧迁移、Config13、依赖和身份模型不改，不升级原运行数据。

Store单轮141/0、扩展Store13/0、协议24/0；真实Core/Host整轮7/0及1原有ignored，包含坏签名拒绝和无确认正常安装；Provider具名1/0（70.69秒）。相关Chromium单轮26/0、零重试；随后只清理弃用译文/映射，前端最终check/build、受影响严格Clippy/门禁和三程序debug构建通过。834份输入及程序摘要已绑定，失败/未执行分别保留，无全量或三平台重跑。原data36份、review data10份/profile303份集合及摘要保持，无Core/GUI/Host遗留；原Unity进程最终已不再观测到，原因未定，本轮未执行Unity启停或自动恢复。未部署新版原生GUI、未恢复桌面控制。此处为推送前本地检查点；本次与b9f19d5现已推送，当前Hosted/CodeQL结果见首段，不进入M9。下段“Core门禁尚未移除/State39待确认”是审批前检查点，当前已完成删除。

2026-11-16 补记：按所有者“可以先推送”授权，完整12笔链已普通快进推送至`2a9d12331d01dfde1ca82958e71bb3fc9f7edb10`，未dispatch/rerun。[CI43803776166](https://github.com/ALCOMD/ALCOMD/actions/runs/43803776166)三平台均失败，[CodeQL43803775667](https://github.com/ALCOMD/ALCOMD/actions/runs/43803775667)四项全部成功。共同失败为真实Host崩溃隔离用例读取extensions.get时返回revision_conflict：声明预览误把runtime/grant状态变化当成包替换。窄修`b9f19d5`改为保留包摘要/发布者绑定并返回最新运行状态，原具名用例1/0、Core所有targets严格Clippy及diff通过；未重跑整模块，未推送该修复。

审批前检查点：[移除发布者确认](exec-plans/M8-mcp-protocol-and-management.md#移除发布者确认2026-11-16state39与公共删除清单已批准)的GUI确认框及信任状态展示已从源码移除、前端check通过，当时未提交或部署。Core门禁和公共/存储模型完整删除当时尚未完成，不计为无确认安装已通过；State39删除清单其后获所有者明确批准，当前实现见前述删除记录。签名、摘要、权限与沙箱保留，未修改原运行数据或恢复桌面控制。下文“未推送/无本轮远端验证”保留为当时本地候选的历史状态，该检查点的实际远端结果为上述2a9d123；后续e99cd2c终态见首段。

2026-11-16 最新[扩展 MCP 工具本地候选](testing/m8-extension-tools-20261116.md)：代码 `54ba42fd47b7228d1a7be08dff9d468a6d31821e` 已本地提交。已批准的签名工具声明、独立 provider/core WIT、Core 合并目录/调用、双方当前能力与 OAuth 上限、原 Plan/确认/Apply、State38 原子接受记录及同键恢复已接通；可选 `mcp.tools.provide` 默认关闭，GUI 复用原开关/声明预览。无逐项目授权、owner 代理或任意 RPC 透传，不隐含 UI/后台启动，不改旧 WIT/Config13/身份模型。

真实冻结 SDK→STDIO/HTTP/HTTPS OAuth→Core/Host 业务模块单轮7/0；最终生产 capability 开放后的 Provider 同例1/0、77.01秒，实际八次卸载、取消/真实写响应丢失恢复、跨客户端隔离、OAuth上限/撤销及故障隔离均有证据。Store整轮136/0后新增重启用例1/0，旧State16/Schema分别3/0和42/0；应用94/0、协议23/0、扩展15/0、Core46/0及1原有ignored、MCP11/0、Host组件5/0、Schema策略3/0。GUI相关Chromium单轮21/0、零重试及前端check/build通过，匹配三程序debug构建、受影响严格Clippy与fmt/metadata/xtask/diff通过。各轮输入/失败分别保留，不拼成全量结果。831份输入及程序摘要已绑定；新依赖闭包和SIMD/unsafe影响见记录。

本轮原环境/review测试前均已关闭，当前仍关闭，无自有Core/GUI/Host遗留；原data36份、review data10份/profile303份集合及摘要不变，原Unity PID40632保持。未升级原库、替换或打开原GUI，未恢复桌面控制；下文旧PID和“窗口保持”属于各轮历史。本批新增代码未推送、尚无对应Hosted CI/CodeQL、跨平台或原生GUI人工验收；运行中任务经新Provider成功取消的独立原生场景仍未制造。M8继续IN_PROGRESS，不进入M9，不将本批局部完成写成M8整体完成。

2026-11-16 此前[内部逐项目授权模型清理](exec-plans/M8-mcp-protocol-and-management.md#内部逐项目授权模型清理2026-11-16)：代码`465f82386ba530b35cf883c7fe1370cea19cb6c6`已在标准目录/main本地提交。AccessContext已删除项目读取/包写入范围字段、逐项目构造器和检查器；项目调用只检查对应能力，MCP身份不再补造项目All范围。通用范围模型改为仅用于ExtensionId的ExtensionUiScopes，保留扩展页面隔离；管理授权指纹只绑定当前能力、ExtensionId及Principal/连接/会话，不再编码项目范围。目标项目ID、owner、原Plan/revision/身份、当前授权与OAuth上限、任务/幂等归属继续复查；不新增RPC、State/Config、依赖或兼容层，历史迁移未改。

436份相关源码的旧模型符号扫描零匹配。应用MCP5/0、Portable UI14/0、候选查询5/0、Store/OAuth16/0、安全合同3/0、备份合同4/0、真实项目包读取1/0、扩展项目能力2/0，受影响application/store/Core全targets严格Clippy及fmt/metadata/xtask/diff通过，各模块分别记录，不称完整Workspace或原生闭环通过。新增授权指纹回归保留能力撤销、ExtensionId隔离、Principal/连接/实例/会话变化拒绝混读。关联架构/权限/RPC/威胁模型规范及测试已同步。**源码清理完成，未推送、无本轮远端验证；本轮没有重建部署验收程序，review仍对应485eddb。** 原窗口和原数据未操作，M8保持IN_PROGRESS，不进入M9。

以下1b676761及485eddb记录保留各自真实输入与结果，不作为本次内部清理的运行或远端证据。

2026-11-16 追加[未发布授权格式严格读取](exec-plans/M8-mcp-protocol-and-management.md#未发布授权格式严格读取2026-11-16)：所有者明确逐项目授权不作为兼容格式。发现Store快照读取/历史幂等重放尚可返回原格式，已统一复用现行能力校验；不支持的请求拒绝，存储格式不支持则整个读取/认证/重放失败，不静默过滤、补全、转换为全局或回退授权。扩展权限读取和OAuth token既有校验已严格拒绝，无需改动。State37和旧迁移不改，原始历史记录保留仅供证据，不作为支持格式。新增具名用例修复前0/1、修复后受影响M8模块最终16/0，State37模块4/0，定向严格Clippy及fmt/metadata/xtask/diff通过；未重跑全量或原生闭环，未推送。当前真实窗口仍运行下段485eddb，不把本次后端补充写成已部署/新增桌面或远端通过。

2026-11-16 此前[能力开关与State37本地收束](exec-plans/M8-mcp-protocol-and-management.md#统一能力开关与state37本地收束2026-11-16)：所有者已批准扩展、MCP客户端及OAuth统一按能力授权，不再逐项目授予。代码候选`485eddb2548f122037585667b19c383b729d48e6`：扩展四种现有权限统一Material开关，项目读取覆盖当前及以后登记项目的安全摘要；MCP八项能力全部global/*，包修改仍须原Plan/确认/Apply，身份/任务隔离保留。管理扩展删除不用的自身项目权限，仅申请read/manage两项可选权限，零授权仍可启用。State37删除旧扩展项目授权、为受影响客户端建立去除旧局部授权的新当前快照，并撤销相关OAuth family；不自动扩大旧授权、历史快照和幂等证据保留，旧迁移与依赖未改。

本轮定向浏览器最终单轮15/0，guest5/0和doc-test1/0，Store关联13/0与14/0、State37四项4/0、合同5/0＋9/0＋2/0、CLI OAuth2/0、真实Host ABI及两个Core/Host用例各1/0，双客户端身份复验1/0。真实六工具模块首轮4/2，定位两处旧格式/单项目假设后各定向复验1/0，**未重跑整个模块，不拼成6/0**；其余初期失败按轮次保留。前端check/build、匹配五程序debug构建、受影响严格Clippy及fmt/metadata/xtask/diff通过，无完整Workspace/Chromium/release或三平台重跑。最后残留核对另以仅测试提交`768022aa361779e2710e4fd2972f7df08b73a209`同步一处旧Project协议样例并补范围断言，具名1/0，未改变运行输入。806份代码候选输入与五程序/包摘要已绑定；新代码未推送、无对应远端验证，36a782c的CI/CodeQL仅为此前基线。

隔离review已备份并实际升级36→37，正常签名/发布者批准及Plan/Apply更换真实样例包（119860字节，SHA256 `8980614f33694a4fadd11faf05d2a106d848b48ee8eed8cc651988aaaa10b9da`），当前已启用、grants为空、0活动任务，两个项目/客户端/OAuth登记保持。新版GUI48912可见且响应，5条真实管道连接隔离Core49940；有界启动连接已退出，复核仍有5条GUI自身连接。原开发环境本来关闭并保持关闭，原36份data/346份profile摘要保持；不升级原数据、不操作Unity、不恢复桌面控制。窗口保留供[查看两个扩展开关及客户端八项能力](testing/m8-management-manual-20261115.md)，尚未人工签收，M8保持IN_PROGRESS，不进入M9。

以下5884d5e的按钮、逐项目选择、三项可选声明及旧PID属于整改前历史；当前以485eddb和上述窗口为准。

2026-11-15 历史[管理扩展零授权启动与权限弹窗整改](exec-plans/M8-mcp-protocol-and-management.md#管理扩展零授权启动与权限弹窗整改2026-11-15)：按所有者反馈，本地代码`5884d5e1dc5152995bec20206f1a642d21d7ae9f`将本扩展三项业务权限改为可选，零授权可启用并打开受限页面；不自动授权，不改变其他扩展的必需权限门禁。管理权限弹窗按权限展示用途、实际状态、允许/撤销及明确项目范围，完整读取授权分页，保留错误、结果未知和迟到读取保护，路径继续复用displayPath。真实Core/Host新增零授权用例及原授权/撤销闭环各1/0；guest3/0、doc-test1/0、Host ABI1/0、合同2/0、浏览器相关子集最终单轮14/0，错误合同补充后三例3/0（覆盖前述子集，不拼成完整套件）。首轮浏览器6/8及新包清单排序拒绝0/1保留，修正后通过；前端check/build、嵌入GUI debug构建、定向严格Clippy、fmt/metadata/xtask/diff通过，未重跑完整测试。隔离样例已通过正常签名、发布者批准及Plan/Apply保留数据地更换，新包真实存在，当前已启用、grants为空、0活动任务。新版GUI39500可见且响应，5条真实管道均连接隔离Core47292，原36份data/346份profile保持，未操作Unity或恢复桌面控制。窗口保留供[人工查看](testing/m8-management-manual-20261115.md)，尚未签收；新增代码未推送、没有对应远端验证，36a782c远端通过仅作为此前基线。M8仍IN_PROGRESS，不进入M9。

2026-11-15 此前[OAuth候选36a782c与隔离查看准备](exec-plans/M8-mcp-protocol-and-management.md#oauth候选36a782c与隔离查看准备2026-11-15)：所有者确认继续后，已核对干净main、完整两笔链及远端祖先关系，将State16修复174aa35及记录36a782c普通快进推送至`36a782c7b78584a46e242b91270fb464bfd9fa29`。同SHA自动CI `43708317935`三平台整个job全部成功，含完整Check、CLI、release/Tauri及适用平台/锁/diff门禁，Chromium各单轮704/0、零重试；CodeQL `43708317682`四项全部通过，均attempt1，没有dispatch/rerun。三平台MCP业务模块各6/0、Store各124/0、State16模块各3/0，两项真实OAuth及旧State19/16修复均有本轮证据，原失败仍按轮次保留。本地只补齐独立Host和嵌入前端debug GUI构建、隔离启动/重开及两个正式CLI登记样例，未重复全量。五程序摘要及444份输入已绑定，[手动查看入口](testing/m8-management-manual-20261115.md)已准备，GUI已按所有者授权实际启动；窗口可见且响应，5条真实管道连接均指向指定隔离Core。当时Core45200/GUI42264已在本轮整改中关闭，当前窗口见上段；当时启动时0活动任务，尚未人工签收，原环境原本关闭并保持关闭，原data/profile保持，未操作Unity；不恢复桌面控制。普通浏览器TLS接入、实时观测、正式分发等后续保持，M8仍IN_PROGRESS，不进入M9，不额外推送文档。

历史候选4005583的[远端结果与State16补充](exec-plans/M8-mcp-protocol-and-management.md#oauth候选4005583远端收齐与state16补充2026-11-15)：同SHA CI `43703345842`三平台失败、CodeQL `43703345588`四项通过。三个MCP业务模块各6/0、Store各124/0，State19及macOS证书/IPC修复已取得真实证据；随后旧State16模块各2/1，后续CLI/挂载/release/Tauri/Chromium等skipped，不能记完整门禁通过。本机原样复现2/1，仅补两张OAuth表必须为空，原保全/序列/外键/重开/回滚断言保持，修复174aa35同模块3/0及定向严格检查通过。该修复现已随上述36a782c推送，不再记为待推送；历史失败保留，不拼接轮次。

前轮33056db9的CI43699307220三平台失败（State19/macOS证书与IPC）及CodeQL43699306717四项通过继续保留，详见原始记录，不改写历史运行。

以下0cc9b28/6bf3493的“本地验证、未推送”属于当时历史状态；当前推送及远端结果以上述36a782c为准。

2026-11-15 当前[OAuth游标补充本地结果](exec-plans/M8-mcp-protocol-and-management.md#oauth游标补充的本地收束2026-11-15)代码为 `0cc9b28fef07661d3e4d99f47a1d11cdcea865f3`。所有者批准的`authorizationBinding`/`authorization_binding`已接通DTO、Schema、typed client、Core与六工具中的项目列表；以独立Core私钥绑定token、当前授权修订、上限及位置，同一SQL读取快照校验，缺失/篡改/跨token/快照变化均拒绝。重启后原token仍按原合同有效，旧游标需从第一页重新读取；仓库及非OAuth游标保持原形状。新增真实游标用例和原HTTPS卸载闭环分别1/0，MAC2/0、Schema51/0、MCP单元11/0、OAuth/迁移11/0及两项旧分页各1/0，受影响六包严格Clippy、fmt/metadata/xtask/diff及Core/MCP/CLI debug构建通过。444份输入已绑定，此前缺口现已收束；无新依赖/State/Config/权限/session或缓存。**仅本地验证，未推送、没有新增代码远端通过结果**。原环境本来关闭并保持关闭，原数据/profile和Unity前后保持；未恢复桌面控制。管理扩展匹配Host/GUI人工窗口、普通浏览器TLS接入及其余M8后续仍未完成，M8保持IN_PROGRESS，不进入M9。

以下6bf3493及其“游标来源绑定限制”是补充前候选的历史结果，该限制已由上段0cc9b28收束；旧轮次通过不改写为新增代码的验证。

2026-11-15 已批准的[HTTPS/预登记OAuth本地候选](exec-plans/M8-mcp-protocol-and-management.md#已批准oauth本地候选与定向结果2026-11-15)代码为 `6bf3493cc69d28e7364f27192635e1f3f7aecade`：9个RPC、两个受限hello分支、State36两表、正式CLI审批及真实HTTPS/SDK/Core六工具接线完成，本地定向通过，现发布协商能力`mcp.oauth.v1`；**尚无该代码远端验证，不宣称整个M8完成**。OAuth/迁移11/0、Schema最终单轮50/0；真实HTTPS闭环、OAuth CLI、旧MCP CLI各1/0，包含实际写响应丢失、取消、撤销及原数据/Unity保全。初期Store失败、迟到refresh重放0/1、Schema编译失败及49/1分别保留，不拼接为完整套件通过。444份输入摘要和原生Core/MCP/CLI产物已绑定记录；TLS仅新增两条锁定直接依赖边，无锁包/版本升级，但MCP生产图纳入既有TLS闭包及第三方unsafe影响。CIMD/DCR、普通浏览器信任与原生人工查看未通过，既有tuple项目游标的跨token来源绑定限制明确保留。原生管理扩展[人工准备](testing/m8-management-manual-20261115.md)仍缺匹配Host/GUI，不能称窗口就绪。测试前原Core/GUI/Host/MCP均未运行，结束后原环境仍关闭、原数据/profile摘要和Unity进程保持；未替换或迁移原数据，未恢复桌面控制。未推送、未进入M9、未重复Workspace/Chromium/release或三平台验证。

当前[管理扩展候选同SHA远端技术收束](exec-plans/M8-mcp-protocol-and-management.md#管理扩展候选同sha远端技术收束2026-11-15)：所有者批准4eb4131＋78920f3后已普通快进推送至`78920f382d1a0c58dece4a51fb2f54d84f48fcf2`。同SHA CI `43616497449`三平台整个job全部成功，含完整Check/Workspace/独立guest、真实CLI/Core/Host管理闭环、CLI自启动、release/Tauri、平台/锁/diff门禁；Chromium各单轮704/0、零重试（Windows18.4m、Ubuntu15.0m、macOS22.1m）。两个Unix真实挂载具名各1/0、零忽略。CodeQL `43616496767`四语言成功，两run_attempt均为1。CLI输出误报成功及guest原生ELF/Mach-O链接两个历史阻塞均在本轮真实验证通过；dcc76ffe和5207287失败、各轮本地定向与完整远端分别保留，不拼接轮次。完整JSON、三job/四语言日志及摘要已归档；Wasm产物仍为原逐字节相同的ce56c941…，不为状态文档重复本机全量。此管理切片技术门禁通过，M8保持IN_PROGRESS；完整OAuth、实时观测、正式默认分发、来源委托及原生人工查看仍待完成。原环境/数据未替换，桌面控制未恢复，纯状态收尾只本地提交、不追加推送。

M8 当前[执行与复用记录](exec-plans/M8-mcp-protocol-and-management.md)：所有者再次确认后，静态合同修复`5bfbc90`与记录已普通快进推送至`3ccc3f428ddadd6b113777b77cce7fe6fa78506d`。同SHA CI `43383251231`三平台完整成功，分别完成严格检查、Rust/合同验证、release/Tauri构建及Chromium单轮703/0（零重试）；CodeQL `43383250817`四语言成功。本批冻结STDIO、客户端身份及六工具卸载切片已取得本地与同SHA远端技术通过证据；真实身份、实际包卸载、在途取消和截断写响应风险用例在三平台均通过。历史失败与本地复现结果继续保留，未拼接为完整通过；本次不重复本地全量验证或推送纯状态记录。该远端基线不含HTTP；HTTP首片的最新结果见下一段，安装/搜索/重装的来源委托仍未实施，当时未实施的管理扩展/Host桥现已有首段所述本地候选，M8保持IN_PROGRESS，不重开M7，不宣布整个M8完成；原Core/GUI及原数据未替换或迁移。

获批[HTTP回环预配Bearer首片](exec-plans/M8-mcp-protocol-and-management.md#9fea7b6最终技术结果与后续边界)的5行浏览器同步修正dbe82de及记录已按所有者确认普通快进推送至`9fea7b6e5afca24674230408041ae5e0789baa9f`。同SHA CI `43571868211`三平台整个job全部成功：完整Check/CLI/release/Tauri及平台/锁/diff检查，Chromium各703/0、零重试，Windows/Ubuntu/macOS浏览器分别15.3/16.9/22.9分钟；两Unix真实挂载各具名1/0、正常清理。CodeQL `43571868047`四项成功，最终JSON、逐job完整日志及准确测试SHA已归档。本轮5行同步已获得同SHA完整远端验证，不再记为仅本地；生产源文件、Rust测试、manifest/锁文件和构建配置相对7eac047未变，复用既有本地输入证据，不冒称不同CI环境或产物字节相同。7eac047通过与f452ac5的Windows夹具失败/macOS702/1、更早失败/取消均保留，不拼接轮次。该远端轮次不含后续管理扩展，M8仍IN_PROGRESS，完整OAuth未实施；该轮只读管理审计及接口/大小/凭据入口缺口属于当时记录，随后获批与本地实现见首段。仅收齐push自动运行，没有dispatch/rerun或操作原环境，收尾只本地提交及metadata/xtask/diff，不额外推送纯文档。

历史候选`fa1e7fc`的CI `43377645951`三平台旧静态合同模块6/3失败、CodeQL `43377644992`四语言成功均保留。失败为三处当前State32预期未随获批State34更新；本机原样复现6/3，仅同步严格相等预期后同模块9/0，修复现已随3ccc3f4推送，未删除其他合同断言，不将历史失败改写成通过。

历史远端`49d0687f8af00aee0672d43c6457fcdca22fe561`：CI `43365902105`三平台**失败**，均在旧State16保全模块2/1，旧State19用例已通过；CodeQL `43365902090`四语言**通过**。该失败为保全夹具未列入获批三张空MCP表；本机复现0/1，保留旧行/序列/外键断言、明确断言三表为空，本地修复`d98038f`同模块3/0。随后Store单轮210/0包含两项旧迁移及委托写回归，现已随fa1e7fc推送，不把历史失败改写成通过；当时该候选没有执行到release/Tauri/Chromium。历史`db37593`的CI `43359695787`三平台Store105/1、CodeQL `43359695853`四语言通过，以及`ca196c47`的CI `43328525847`依赖名单失败、CodeQL `43328524955`通过均保留，不用旧M7结果替代。

身份合同首批历史证据：State33初始化/迁移5项、DTO2项、当时Schema42项/StoreHandle12项分别通过，当时认证尚未接通、不发布capability。获批State34保全旧授权并支持永久幂等快照，0033原迁移未改；本次六工具不新增Core RPC、State或Config。身份收尾的Store10项、Core2项、hello1项、STDIO12项及真实IPC双身份1项等原结果和462份输入继续保留，不拼成完整Workspace通过。六工具实际证据、定向结果、465份候选输入与本轮原环境恢复见ExecPlan末节；只暂停已确认无活动任务的原Core/GUI，未操作真实项目或恢复桌面控制。

M0–M7阶段正式验收完成。M7最终功能候选`c46f7544`本地完整700项、三平台Hosted CI各700项及四语言CodeQL全部通过；未变Rust输入对应901项通过/14项原有忽略。独立验收在`e9879837`补跑最新增量28项及GUI/metadata/diff检查通过，994份输入未变，未把旧结果冒充完整703项。两处过时文档已同步到现行Config13和`logs.read`。详见[M7完成记录](testing/m7-completion-20261113.md)。

H2-A/Visual Gate 2及H2-B、H3–H6已有签收和保留边界沿用，H7按所有者整体认可与正式收尾授权收束，不新增原生人工操作声明。下列M7实施记录保留当时的候选和状态，不作为M8重新开发既有能力的任务。

- 2026-11-13 M7收尾后按所有者反馈补充[删除缺失目录的明确提示](exec-plans/M7-missing-project-directory.md#m7收尾后的删除缺失提示补充2026-11-13)：实际日志确认Core已报SourceMissing，GUI现单独显示“项目文件夹不存在，请检查项目路径后再删除。”。源码044d0559；28项局部回归、前端check/build、metadata/xtask及原生GUI构建通过。36份data/343份profile备份核对后更新恢复Core137312/GUI142392，ready/State32/Config13、0活动任务，原资源/设置revision8、12个项目标志检查位及3个Unity/Hub进程保持。Core字节未变，无公共合同变化；本补充未声称完整703项或新三平台门禁通过，M7验收仍绑定c46f7544，未开始M8。

- 2026-11-13 af528c7a完整本地浏览器700/0、零重试和四语言CodeQL通过，macOS/Ubuntu整个CI成功，Windows浏览器699/1仍在同一资源分段读取次数断言失败；起始页等待不足以解决问题，保留此轮失败。进一步临时事件轨迹及12倍CPU双worker隔离重现6次中1次真实二次读取：页面接管后旧按钮事件重新建立预取。现修正同一交互的交接所有权，结束交互/新访问仍重新读取，禁用控件只释放自身所有权；原缓存断言修复前失败、修复后通过，同条件慢渲染6次及导航35项/前端check通过。没有改次数断言、重试或超时，临时诊断已移除；最终新候选门禁及GUI构建更新另记。见[交接修正](exec-plans/M7-final-closure-20261112.md#预取接管后的旧交互重复读取)。

- 2026-11-13 5bd7fd0b完整本地浏览器700/0、零重试、全部适用本地门禁和四语言CodeQL通过；Windows/Ubuntu整个CI成功，macOS浏览器699/1在资源分段预取的严格单次读取断言收到2次读取处失败。未提前签收。原用例只等待启动检查态也会展示的导航，现补齐起始页真实空状态就绪条件，保留按压前预取、导航后共享请求及精确次数断言；生产代码不变。本机原用例正常和6倍CPU节流各6次通过，未冒称已本机复现。后续候选验证见[就绪条件记录](exec-plans/M7-final-closure-20261112.md#资源分段预取的起始页就绪条件)。

- 2026-11-12 e7f8cb96完整本地浏览器700/0、零重试和四语言CodeQL通过，Ubuntu/macOS整个CI成功，Windows浏览器699/1在另一旧还原慢加载用例未出现弹窗处失败。保留三平台原始结果，没有提前签收。已将相同普通菜单选择模式的49处调用统一到公开opened/closed助手，全部定位器、点击、断言和700项总数保持，生产/Rust/锁文件不变；15项重复快慢加载及14项菜单焦点/还原/缺失目录回归通过，最终新候选门禁另记。当前Core/GUI仍为已保全更新的9f47336b源程序，正常运行组件hash已核对；独立迁移工具保持隔离。见[菜单同步记录](exec-plans/M7-final-closure-20261112.md#普通菜单选择统一同步)。

- 2026-11-12 9f47336b已通过全部本地原生门禁：workspace901项通过/14项原有忽略、严格Clippy、前端检查/构建、CLI自动启动、release bins及Tauri no-bundle等；原数据/profile备份核对后更新Core79012/GUI134976，ready/0活动任务，设置revision8、原4项目/30仓库/5模板/1Unity登记、备份记录及12个项目标志检查位保持，Core报告2个目录存在、2个缺失。生成目录消失的恢复异常及辅助程序补全已记录，未回写缺失项目。该SHA四语言CodeQL通过，Ubuntu/macOS浏览器各699/1暴露旧用例未等待Material生命周期；已仅修正测试同步，47项相关回归及两例各3轮、前端check通过。生产输入不变，按逐文件摘要复用原生结果，完整700项浏览器及新SHA三平台结果仍待执行；不提前宣布M7完成。详见[当前候选记录](exec-plans/M7-final-closure-20261112.md#目录缺失增量与本轮候选门禁)。

- 2026-11-12 所有者明确批准[缺失项目目录合同增量](exec-plans/M7-missing-project-directory.md)：ProjectSnapshot.directoryState为available/missing/unavailable，Core仅观测根目录；缺失复制源明确返回project_copy_source_missing。列表/卡片灰显、说明及按钮前错误已实现，不写登记或改变revision，无数据库Schema/权限变更。13项Core/协议及5项新增浏览器回归和前端check通过，实际IPC及最终统一门禁待执行。6fef3613三平台CI均因遗漏的旧Fast默认断言失败，已独立同步为所有者选择的Level5；该SHA四语言CodeQL通过。前轮完整本地检查受资源不足中断，记录为未完成，不能用于最终签收；原程序/窗口已恢复，未恢复或回写用户已不存在的项目内容。新候选继续M7，尚不宣布完成。

- 2026-11-12 备份反馈核对后，所有者选择默认等级5并接受“项目名＋时间”命名，按[修正计划](exec-plans/M7-backup-names-default-compression.md)继续本阶段。新默认、设置页恢复默认及友好文件名已实现；显式旧偏好和旧UUID归档保持，名称随内部归档证据冻结、碰撞不覆盖。当前实例经settings.update仅将压缩默认保存为level5，revision7→8。29项定向浏览器、10项归档/恢复、9项目录/合同、应用/配置库、fmt/xtask及配置合同检查通过；真实RPC仅已编译，严格Clippy与最终统一门禁仍待执行，不宣布M7完成，运行中的程序尚不含新命名。前一候选0270967b的Ubuntu/Windows及CodeQL通过，macOS总时限不足已单独修正；后续macOS并行夹具撞名风险亦已独立修正，新的同SHA远端结果另记。

- 2026-11-12 按所有者“分次提交推送并更新”完成实际交付：收藏`7d353b34`、通知`a1f88a9e`分别提交并正常推送（含9个前置提交）；远端Ubuntu检查发现的选择器unused_mut独立修复并推送为`1f48e668`。发布构建及本机更新完成，Core PID101844 / GUI PID131692，ready、GUI连接响应、State32/Config13、0活动任务；原数据/完整设置revision7、12个项目标志检查位、原profile和10个Unity/Hub进程保持。旧程序及34数据/336profile文件已备份核对；实际证据及CI仍运行的限制见[交付记录](testing/notifications-runtime-update-20261112.md)。无新Schema/RPC/权限/依赖变更，不发布/强推，M7/H7仍为技术收尾。

- 2026-11-12 按所有者批准恢复[应用内结果通知](exec-plans/M7-in-app-notifications.md)：右下角单条共享出口，手动操作反馈、后台任务终态及任务入口；前台/局部结果去重，模态延后，最小化关闭动画内完成不丢结果。收藏/自动保存成功、普通读取、预加载、自动同步及历史/重连补读静默，最终保存失败可见。新增21项通知回归，分轮192项不同浏览器用例通过，最终41项及前端check/build、图标/语言检查通过，见[验证记录](testing/in-app-notifications-20261112.md)。无Core/公共合同/Schema/权限/依赖变化；实现验证时未提交、推送或替换运行窗口，所有者随后授权本轮分次提交、正常推送和更新程序；原1d1e5db全门禁为本增量前基线，M7/H7保持。

- 2026-11-12 收藏按钮选中态补充：已收藏使用实心Material星形，未收藏/取消后使用空心；列表与卡片共享Core权威状态。前端check/build、36份固定图标资产校验及36项定向浏览器回归通过，见[M7收尾计划](exec-plans/M7-final-closure-20261112.md#收藏按钮选中态补充)。所有者随后授权本项分次提交、正常推送和更新本机程序；实际交付另记，1d1e5db仍仅为增量前全门禁基线。

- 2026-11-12 原删除卡住的纠正交付完成：87edbfbe修正已构建并仅更新Core，原真实任务由正常恢复明确failed/source_missing，revision9/completedAt存在、活动数0，不再自动重试。实测正常关闭GUI后Core在1314ms内自动结束，重开后终态不变；Core PID126408 / GUI PID114740，ready/State32/Config13。原4项目/30仓库/5模板/1Unity登记/0扩展、完整设置/revision7、12个项目标志检查位、原profile及Unity/Hub进程保持；GUI文件未变。备份、22项回归、构建和运行限制见[记录](testing/project-delete-missing-target-20261112.md)。未手工改库、推送或发布，M7/H7保持；此前原任务仍恢复中为旧版观察。

- 2026-11-12 所有者反馈原删除仍卡住，完成[源目录/载荷均缺失的终态修正](exec-plans/M7-project-delete-missing-target.md)：owned wrapper验证、尚无移动身份且两处明确NotFound时，Core以既有source_missing失败结束、保留登记，不永久恢复或重删后来重建的目录；已有移动/尚存payload/不明错误仍走原恢复边界。22项Rust回归（含4项新增）、受影响严格Clippy及fmt/xtask/diff通过；已备份原运行实例，接续本次更新作纠正交付，见[记录](testing/project-delete-missing-target-20261112.md)。无RPC数据/Schema/权限/依赖变化，不推送，M7/H7保持。

- 2026-11-12 按所有者“提交并更新”要求，将Core自动退出提交为98e6b95并构建/更新Core及嵌入式GUI（含已提交刷新反馈）。离线备份旧程序、34份数据和329份profile后，打开Core PID126816 / GUI PID113392，ready/State32/Config13；原4项目/30仓库/5模板/1Unity登记/0扩展、完整设置/revision7及12个项目标志检查位保持，原Unity/Hub进程保持。State32为此前已批准的迁移。原删除任务因源目录/载荷缺失仍在恢复中，活动数1，本轮未伪造完成；因此当前Core继续运行。交付、重开入口、摘要及限制见[记录](testing/core-idle-runtime-update-20261112.md)。未推送或发布，M7/H7保持；下文未更新为各轮历史状态。

- 2026-11-12 按所有者要求完成[Core 空闲自动退出](exec-plans/M7-core-idle-shutdown.md)：所有普通 RPC/事件连接关闭且全局无未完成任务时，经1秒交接缓冲正常退出；首次连接留5秒，新连接取消旧判断。所有 Principal 的非终态任务及查询失败均保持运行，在途同步/发现/刷新先收尾，后台扩展随 Core 正常停止。107项不同 Rust 回归通过，含8项新增真实 IPC/任务查询回归；受影响严格 Clippy、fmt/xtask/diff通过，见[记录](testing/core-idle-shutdown-20261112.md)。无RPC数据形状/Schema/权限/依赖变化，未实现MCP/API；所有者随后授权本项提交并更新程序，实际交付另记，未推送，M7/H7保持。

- 2026-11-10 按所有者要求统一[刷新按钮圆圈反馈](exec-plans/M7-refresh-button-feedback.md)：点击立即显示 Material 圆圈，快请求至少300ms，慢请求持续到完成；结果/错误即时呈现，保留禁用与重复触发保护。覆盖包管理、仓库、用户包、两处 Unity 查找、扩展权限及 Portable UI 普通 action。分轮116项不同浏览器用例通过，最终59项回归、前端检查/构建及fmt/xtask/diff通过，浅深色截图已核对；失败和复用范围见[记录](testing/refresh-button-feedback-20261110.md)。无Core/公共合同/Schema/权限/依赖变化；2026-11-12所有者随后授权本项本地提交，未推送或更新运行实例，M7/H7保持。

- 2026-11-10 修复删除任务因目录占用长期挂在恢复中的问题：rename 明确失败且证明源目录未移动时，Core 记录 failed/completedAt，保留项目和登记；释放占用或重启后不自动再删，空准备目录清理失败仅补充诊断。已移动/不明状态的恢复、取消边界及重建原路径保护保持，恢复停止日志保留 Waiting 和真实原因。10 项删除回归（含新/旧任务真实 Windows 占用）、2 项 P7 契约、受影响库、严格 Clippy、fmt/xtask/diff 和独立 Core 构建通过，无新增 RPC 形状/错误码、Schema、权限或依赖。所有者选择先修代码，未关闭 Hub/Editor、更新运行程序或执行真实删除；最后一次实机只读复核因自动审批服务额度错误未执行，详见[计划](exec-plans/M7-project-delete-unmoved-failure.md)和[验证记录](testing/project-delete-unmoved-failure-20261110.md)。前一项 State32 修复保持；所有者随后授权两项修复分别提交并推送，实际远程范围与 refs 另行核对；M7/H7 保持。

- 2026-11-10 修复公开仓库软件包缺失：Core 将作者/显示名/描述归为有界展示信息，支持无作者及字符串作者；null GUID 旧文件声明按 VPM 的路径语义读取。所有者已批准 State32，移除作者/显示名必填触发器并让原错误索引完整重读，保留真实资格/来源/依赖/清理及冻结事务校验和健康缓存。30 个当前公开仓库的 4,630 条版本全部通过读取→Store→resolver/candidate 审计，覆盖 28 个缺作者软件包；定向回归、最小包真实 RPC 安装/卸载、迁移保全、严格 Clippy/fmt/xtask 及独立 Core 构建通过。额外界面契约有 1 项已证实的既有静态签名断言失败，详见[计划](exec-plans/M7-repository-manifest-visibility.md)及[验证记录](testing/repository-manifest-visibility-20261110.md)。当前运行 Core/GUI 与真实数据保持，未更新程序或操作真实包；所有者随后授权本项与删除失败修复分别提交并推送，实际远程范围与 refs 另行核对；M7/H7 保持。

- 2026-11-10 按本次“分次提交更改并更新”要求，将后续项目圆角/首帧、当前包输入、MA 降级、多来源版本、默认目录、完整仓库配置、扩展包完整后缀及 Unity 安装入口分别完成 9 项本地功能提交。分组后累计源码与原验证摘要对应，原 83 个工作文件字节保持；最终 fmt/xtask、前端检查/生产构建及独立 Core/GUI 构建通过。离线逐文件备份旧程序、29 份数据和 327 份界面配置后，更新打开 Core PID54660 / GUI PID46468，ready/State31/Config13、0 活动任务。完整设置/revision7、3 项目/30 仓库/5 模板/1 Unity 登记/0 扩展及 9 份项目清单保持，原 profile 与 v3/Unity 保持。详见[交付记录](testing/pending-delivery-20261110.md)。未推送或操作真实项目包，不扩大 M7/H7 签收；下方未提交/未更新均为历史轮次状态。

- 2026-11-10 按所有者要求调整设置页已安装 Unity 区域：直接提供“查找并刷新”和“手动选择”，版本列表逐行警告色移除，确认仅取消登记并保留 Editor；沿用 Core 身份/修订/引用保护及分页、重连、路径规则。126 项界面回归、前端检查/生产构建及嵌入式 GUI 独立构建通过，宽窄截图已核对，见[计划](exec-plans/M7-unity-installation-controls.md)和[记录](testing/unity-installation-controls-20261110.md)。本轮 Core 未改动，配对复用上一轮已验证产物；未操作真实安装、替换运行实例、提交或推送，M7/H7 保持。

- 2026-11-10 按所有者要求将扩展包后缀统一为 `.alcomdextension`：同步产品元数据、系统选择器、当前规范/架构、Git 二进制规则和测试夹具，沿用原包内容、验签及安装运行合同。6 项包验证、1 项隔离真实 Core/host 生命周期、32 项界面回归、受影响严格 Clippy、前端检查/构建、fmt/xtask 及 Core/GUI 独立构建通过，见[计划](exec-plans/M7-extension-package-full-name.md)及[记录](testing/extension-package-full-name-20261110.md)。历史证据保留当时名称；未重命名用户文件、操作真实扩展、替换运行实例、提交或推送，M7/H7 保持。

- 2026-11-10 按所有者要求完成[完整存储库配置文件](exec-plans/M7-repository-configuration-files.md)：默认 `.alcomdrepositories` 包含全部内置/本地/远程来源、自定义显示名、顺序、仓库/包开关及已有认证，保留旧 TXT 兼容导出/导入。导入可选覆盖已有配置且默认开启，取消后仅新增并保留原配置，文件外来源保持；无关索引刷新不阻止配置恢复，同配置重复导入不产生多余修订。公开配套同步，无 SQL 迁移、新权限名或依赖。188 项不同 Rust、53 项不同浏览器、受影响严格 Clippy、前端检查/构建及门禁通过，见[验证记录](testing/repository-configuration-files-20261110.md)。独立构建产物已准备，未替换运行实例、操作真实仓库/项目、提交或推送；M7/H7 保持。

- 2026-11-10 按所有者要求将默认项目及备份位置改为用户主目录/ALCOMD/Projects、Backups：Windows 查询 Profile，Unix 使用 HOME，移除 Documents/XDG 文档目录解析；显式路径、旧资源与已接受任务目的地保持。40 项目录/设置/存储回归、平台严格 Clippy、fmt/xtask 和 Core/GUI 构建通过；额外旧元数据脚本有既有 Config12/13 断言不一致，详见[验证记录](testing/home-storage-defaults-20261110.md)。备份后更新打开 Core PID43844/GUI PID2416，实际默认值已核对，原设置/revision5、3 项目/30 仓库/5 模板、profile、v3/Unity 及真实项目清单保持，0 活动任务。未迁移资源、提交或推送；Unix 未实跑，M7/H7 保持。

- 2026-11-10 修复 NDMF 等多来源同版本全部置灰：GUI 去掉“条数必须为 1”的额外限制，默认多来源提交版本意图，由已有 Core Plan 按优先级确定并冻结来源；显式选源、Core 已知阻止条件、快照和真实依赖冲突校验保持。84 项浏览器、23 项核心回归、严格 Clippy、前端/原生构建及门禁通过；隔离验证重复仓库自动选源成功及 MA 约束拒绝过低 NDMF。备份后更新打开 GUI PID23972，Core PID31256 和原 3 项目/30 仓库/5 模板、设置/revision5、profile、v3/Unity 及真实项目清单保持，0 活动任务。未操作真实包、提交或推送，见[计划补充](exec-plans/M7-package-v3-parity-audit.md)及[验证记录](testing/version-source-availability-20261110.md)，M7/H7 保持。

- 2026-11-10 修复 MA 1.18.7→1.18.6 降级的清单误判：归档依赖与仓库记录统一复用现有 VPM 解析器规范化后比较，避免复合条件顺序差异被误报 manifest_fingerprint_changed；真实依赖/Unity/旧资源差异、无效条件和结构仍在写项目前拒绝。修复前隔离复现、修复后完整降级成功且 NDMF 保持；236 项不同 Rust、严格 Clippy、fmt/xtask 和独立 Core 构建通过。备份后更新打开 Core PID31256/GUI PID27580，GUI 二进制与此前 81 项浏览器验证输入保持；3 项目/30 仓库/5 模板、设置/revision5、profile、v3/Unity 和 9 份真实项目清单保持，12 仓库正常刷新时间推进，MA 仍为 1.18.7。未代替用户真实降级、提交或推送，见[验证记录](testing/ma-downgrade-range-validation-20261110.md)及[计划补充](exec-plans/M7-package-current-project-inputs.md)，M7/H7 保持。

- 2026-11-10 修复软件包版本切换/卸载点击无反馈：Core 的普通 Plan/Apply 复验改为直接读取当前项目文件，保留注册身份/修订但不与过期登记依赖比较，不回填登记观察；确认后变化仍拒绝，元数据指纹 v8 补入 Unity 版本/修订，旧未执行计划需重建。修正归档检查错误访问未替换包暂存清单，准备失败在确认窗口显示原因并可关闭重试。241 项不同 Rust、81 项浏览器回归、严格 Clippy、前端检查/构建及门禁通过。备份后更新打开 Core PID34112/GUI PID36256，原 3 项目/30 仓库/5 模板、完整设置/revision5、profile、v3/Unity 及 9 份真实项目清单保持，0 活动任务；未执行真实包 Apply、提交或推送。见[计划](exec-plans/M7-package-current-project-inputs.md)和[验证记录](testing/package-current-project-inputs-20261110.md)，M7/H7 保持。

- 2026-11-10 项目页接回其他工作区已有的已就绪首读路径：预加载完成时直接初始化真实列表/网格或空状态，未完成才显示加载态，消除 effect 回填导致的额外空表结构；共享请求、慢读工具栏、筛选、重试和迟到隔离保持。四项十秒悬停首次 DOM 提交回归及既有相关套件共 57 项通过，前端检查/构建通过。接续本项目页修复的更新要求，备份后更新打开 GUI PID29816，Core PID33968、3 项目/30 仓库/5 模板、设置/revision5、原 profile 与 v3/Unity 保持，0 活动任务。见[计划](exec-plans/M7-progressive-loading-preload.md)及[验证记录](testing/project-prefetch-first-render-20261110.md)。未提交/推送，不扩大 H7 签收。

- 2026-11-10 修正项目列表第一条记录上圆角切换：项目专有的 64px 行高仅作用于真实数据行，加载占位透明并保持 4px，消除方角整行色块切换成圆角记录的观感。仅两处项目列表样式变化，其他加载区域保持；12 项定向浏览器回归、前端检查/构建通过，浅深色宽窄截图已核对，见[GUI 计划](exec-plans/M7-official-gui-extension-ui.md)。随后按所有者要求备份并更新打开 GUI PID32464，Core PID33968 保持；3 项目、30 仓库、5 模板、完整设置/revision5、原 profile 及 v3/Unity 保持，0 活动任务。未操作真实项目、提交或推送。

- 2026-11-10 所有者要求“都分次提交并更新”：预取、基础模板、确认后下载、移除配色、选择器路径、任务窗口分别完成 6 次本地功能提交，并保存独立交付记录。共享文件按功能拆分后与各阶段验证摘要匹配，原工作文件字节保持。最终前端/独立 Core/嵌入式 GUI 构建、fmt 与仓库门禁通过；离线备份后更新打开为 Core PID33968、GUI PID39112，ready/State31/Config13、0 活动任务。切换前后及 GUI 打开后的 3 项目、30 存储库、5 模板、完整设置与 revision5 保持，沿用原 profile，v3/Unity 保持。详见[交付记录](testing/batch-delivery-20261110.md)。未推送或操作真实项目，不扩大 H7/M11 签收；下方未提交/未更新均为各轮历史状态。

- 2026-11-10 按所有者要求完成[任务执行窗口](exec-plans/M7-task-progress-window.md)：确认任务后连续显示进度和结果，可最小化并在任务中心跟踪；后台完成不弹回、不自动导航。覆盖包操作、项目创建/复制/还原/删除、迁移、备份、模板和扩展；模板失败编辑、读取重试、取消及未知写入保护保留。补齐阶段翻译，终态不再显示进行中的阶段。195 项不同浏览器用例分轮最终通过，全工作区前端检查/构建、仓库及差异检查通过；轮次、截图与限制见[记录](testing/task-progress-window-20261110.md)。无 Core 合同/权限/依赖变更，未更新运行实例、操作真实项目、提交或推送。

- 2026-11-10 按所有者批准完成[系统选择器起始位置](exec-plans/M7-native-picker-locations.md)：各入口传递当前路径/默认目录，原生适配器统一定位已有目录、文件父目录、最近已有上级及主目录；取消保留原值，配置提示读取失败仍可手选。覆盖项目添加/创建/复制/还原、设置、Hub/Editor、仓库列表/清单、模板、扩展和用户包；导出改名/重试保留位置提示，Core 独占创建保持。23 项 GUI Rust、114 项浏览器回归、严格 Clippy、前端检查/构建及门禁通过；范围和未执行的系统弹窗实测见[记录](testing/native-picker-locations-20261110.md)。无 Core RPC/Schema/权限/依赖变化，保留此前全部修改，未更新运行实例、提交或推送。

- 2026-11-10 按所有者要求将软件包行移除、批量移除、纯移除确认，以及存储库行移除和移除确认接入 Material error/on-error 配色。补齐既有危险操作样式的图标/实心按钮令牌，禁用按钮保持默认灰色；安装/更新确认保持原色。前端检查/构建和 Playwright 合成页面浅/深色、禁用状态核对通过，实际范围见[GUI 计划](exec-plans/M7-official-gui-extension-ui.md)。仅 GUI 局部颜色修改，未更新运行实例、提交或推送。

- 2026-11-10 按所有者明确批准完成[软件包确认后下载](exec-plans/M7-package-confirm-before-download.md)：普通包 Plan 仅计算固定来源的版本/依赖/兼容性及清理范围，不下载 ZIP 或首次固定未知摘要；Apply 确认后下载校验，复用同源持久摘要，依赖、Unity 要求或清理目标不符时在项目写入前拒绝。更新跳过已有声明、重装完整检查保持；空清理计划也冻结 formatVersion 4，项目元数据指纹 v7 拒绝旧未执行计划。仓库 archiveSha256 可省略，公开类型和 Schema 同步，无 SQL/权限/生产依赖变更。确认窗口移除普通未更新项。584 项不同 Rust、85 项浏览器用例、严格 Clippy、前端检查/构建及最终门禁通过；失败修复和实际限制见[验证记录](testing/package-confirm-before-download-20261110.md)。保留此前模板/预取修改，未提交、推送或更新运行实例，不扩大 M7/H7 签收。

- 2026-11-10 所有者明确要求“更新”，已构建并打开包含软件包开关/Config13、50ms及按下预取、空白与基础模板恢复的新版Core/GUI。离线备份原程序、数据19份文件和WebView配置319份文件并逐字节校验；新Core PID38052、GUI PID15580，ready/State31/Config13、0活动任务。2项目、30仓库、4个原模板逐项保持并新增空白；设置revision5及原值保持，沿用原profile，v3/Unity进程保持。首次最早快照因旧Core启动后的后台刷新过时，已自动恢复后改以切换前最终快照严格比较并通过；完整经过及原生UI读取未获Computer Use授权的限制见[运行更新记录](testing/runtime-update-20261110.md)。未提交/推送，不扩大H7签收；下文“未更新”保留其历史含义。

- 2026-11-09 按所有者要求及提前批准完成[空白与基础模板恢复](exec-plans/M7-template-bases.md)：内置Blank生成最小Unity工程并允许显式版本选择，新建项目默认空白；手动创建模板默认基于空白，可选已有模板继承工程/资产/meta/UPM/VPM完整锁图并追加依赖，固定版本随基础继承。接受时冻结基础修订与归档，恢复和删除后执行有引用保护，保存结果独立。153项Rust测试、32项浏览器回归及受影响严格Clippy、前端检查/构建通过，中文宽浅/窄深截图已核对；实际失败与复验见[记录](testing/template-bases-20261109.md)。仅增加可选baseTemplate请求和内部journal字段，无State/Config版本、SQL表列、权限或依赖变化；保留原预取修改，未提交/推送或替换运行GUI/Core，M7/H7保持。

- 2026-11-09 按所有者追加授权调整[通用意图预取](exec-plans/M7-progressive-loading-preload.md)：悬停由80ms缩短为50ms，主指针左键/触摸按下立即准备并提升排队请求，点击复用；取消按下释放指针保活，禁用控件及右键/中键/非主触摸不因按下触发。全工作区前端检查、生产构建及6份文件130项浏览器回归通过，含8项新增用例；[验证记录](testing/preload-press-20261109.md)保留命令及覆盖范围。仅GUI交互与内存读取调整，未提交/推送或替换运行GUI/Core，M7/H7保持。

- 2026-11-09 按所有者批准完成[存储库内软件包开关](exec-plans/M7-repository-package-switches.md)及[Config13](../specs/config/settings-v13.md)：仓库列表/详情以仓库+包ID关闭全部版本，Store尽早过滤，候选/求解/Plan/Apply一致；其他来源独立，已安装事实与管理重开入口保留。原设置16KiB基线、旧格式16KiB完整文件限制保持，新增偏好扩展至1MiB；无State/RPC方法/权限/依赖变更。547项不同Rust测试、92项不同浏览器用例最终分批通过，3项既有Rust忽略；严格Clippy、前端检查/构建及浅色外观已核对。最后fmt/xtask/差异检查及失败轮次见[记录](testing/repository-package-switches-20261109.md)。所有者随后授权本项本地提交；未推送/替换运行实例或操作真实项目，M7/H7保持。

- 2026-11-09 所有者明确授权“提交，推送，打开新的”，本批次Core/GUI按用途分组交付至既有`origin/main`，不强推。7项Rust和77项GUI最终输入摘要匹配既有验证，fmt/xtask及独立Core/嵌入式GUI构建通过。已备份旧程序/数据并更新打开：Core PID294584、GUI PID282808，ready/State31/Config12、0活动任务；原2项目、30存储库、设置、WebView profile及v3进程保持。原生UIA只读确认官方仓库分组浏览及标题行刷新，未代替用户操作。提交/推送及运行证据见[交付记录](testing/delivery-20261109.md)，下文“未提交/未更新”等保留其历史含义，M7/H7不扩大。

- 2026-11-09 按所有者要求完成[存储库列表与包浏览](exec-plans/M7-repository-browser.md)：行内勾选统一到40px控件令牌，仓库行60px；内置编辑/移除禁用。刷新移出编辑，按后续纠正并入浏览标题右侧，与标题同排；独立详情页同理。目录按包名称去重，单独查看版本/ID/说明/Unity条件/文档和日志链接，跨页完整后显示版本选择，刷新保留选择并原子替换。全工作区前端检查、生产构建、分批115项不同用例最终通过覆盖、失败与复用边界见[记录](testing/repository-browser-20261109.md)；中文浅/深色宽窄窗口已核对。无Core/公开合同/依赖变更，未更新运行GUI/Core或提交/推送，M7/H7保持。

- 2026-11-09 按所有者授权完成[通用意图预取与分区域加载](exec-plans/M7-progressive-loading-preload.md)：悬停80ms、焦点即时，悬停/焦点/打开交互共同保活，全部离开3秒清理，导航点击接管；有界队列/容量与迟到隔离，版本菜单复用。标题/已知表头先行，主体条与表格条按区域互斥，资源表头固定，设置Unity列表独立加载；100ms上限及已有内容后台刷新保持。全工作区前端检查/构建和分批236项不同浏览器用例最终通过范围、早期失败与旧断言修正详见[记录](testing/progressive-loading-preload-20261109.md)。浅/深色中文窄窗口和实际溢出截图已核对。无Core/公开合同/依赖变更，未更新运行GUI/Core或提交/推送，M7/H7保持。

- 2026-11-09 按所有者要求修正共享固定表头列表的纵向轨道：观察实际布局高度，轨道从表头底边开始，缩放及尺寸改变同步，保留列对齐和滚轮/横向/拖拽；包安装换为固定上游的官方add_circle，与cancel同样式。全工作区前端检查/构建、35份图标边界通过；45项不同浏览器用例最终通过范围、首轮1项异步断言失败及3轮复验详见[记录](testing/package-scrollbar-install-icon-20261109.md)。真实溢出、72px表头、125%缩放及浅/深色窄窗口截图已核对。无新依赖、Core/合同变更，未更新运行GUI/Core、提交或推送，M7/H7保持。

- 2026-11-09 所有者知悉v3重装跳过旧声明后，明确批准将重装改为完整重新检查：单包、全部重装及批量显式重装目标重新发现旧资源，确认后在同一事务替换，保留原版本、直接/传递身份和失败回滚；混合批量其他更新仍跳过已有声明。内部重装目标进入既有ChangeSet摘要/持久JSON，不新增公开DTO、SQL表/列、权限或迁移；普通计划指纹v6使旧未执行计划需重建。此前GUID递归、8KiB流式读取、大meta支持及总体安全校验保持。391项不同Rust测试通过、3项既有忽略，受影响严格Clippy、fmt/xtask及差异检查通过；含四种重装组合、更新范围隔离、冻结意图/持久化及发布冲突恢复。未更新运行Core/GUI、操作真实项目或提交/推送，M7/H7保持；见[范围与验证](exec-plans/M7-legacy-discovery-scopes.md)。

- 2026-11-09 确认弹窗按v3的内容组织独立调整：整行分组变更、按包汇总冲突、名称强调、主视图无重复范围/圆点、Material固定操作区；前端检查/构建及42项既有浏览器回归通过，中文常规/窄窗口截图已查看。更新无弹窗日志定位到旧资源meta大小校验错误。所有者随后澄清AI可读取获准测试项目，质疑的是Core普通包操作的读取范围；修正进展见上条，失败呈现保持。未更新运行实例或提交/推送，M7/H7保持；见[记录](testing/package-review-layout-20261109.md)。

- 2026-11-09 按所有者要求更新名称展示GUI：7项前端/测试/产物摘要复核一致，独立嵌入式GUI构建通过，旧程序备份后更新并打开为PID294004；原Core PID285544、2项目/2仓库、设置与原数据/profile保持，ready/0活动任务。原生UIA确认项目列表；自动置前重试遇用户输入，后续确认窗口最小化，停止输入并保留窗口状态，未执行原生包操作。证据及限制见[名称展示记录](testing/package-display-names-20261109.md)。未提交/推送，M7/H7保持。

- 2026-11-09 按所有者要求补齐软件包名称优先展示：变更审阅、依赖冲突、Unity不兼容确认/警告及旧资产所属包复用Core已加载的未筛选名称映射；缺失名称回退标识，技术详情及RPC身份保持，无新增读取/持久化/公共合同。全部前端检查、生产构建、42项既有浏览器回归及仓库/元数据检查通过，SDK审阅截图已实际核对。未更新运行Core/GUI或提交/推送，M7/H7保持；见[验证记录](testing/package-display-names-20261109.md)。

- 2026-11-09 所有者再次复查两项修正后明确授权“都提交并推送吧”：本项修正、回归与文档提交后，连同此前15个本地提交正常推送至`origin/main`，不强推。远端读取确认无新增提交；源码摘要与329项定向测试的输入一致，复用既有验证，不更新运行Core/GUI，不扩大H7/M11签收。下方未提交/推送为各项完成当时的历史状态。

- 2026-11-09 按所有者要求修复`41de9ab8`复查两项问题：普通变更及卸载清理保护所有仍保留锁定包的共享依赖，本地用户源候选独立读取撤回资格，不受可选展示字段解析失败影响。新增两项正式回归含四种真实Plan/Apply组合及Summary/Versions/安装一致性；相关套件329项不同Rust测试通过、3项既有忽略，严格Clippy、fmt/xtask/元数据及差异检查通过。无公共合同、Schema、权限或GUI变更；未提交/推送或更新运行Core/GUI，M7/H7边界保持。输入、失败轮次与限制见[验证记录](testing/vpm-review-fixes-20261109.md)。

- 2026-11-09 按所有者授权完成[七项VPM修正](exec-plans/M7-vpm-seven-gaps.md)：来源关闭贯穿Core目录/候选/Plan/Apply及发布资格，撤回状态统一；批量增量保留、旧依赖清理、缺失目录重装、本地提供方及固定版本恢复补齐。13项新增正式回归及相关套件共471项不同Rust测试通过，含更新时的7项原生RPC和既有kill/restart矩阵；严格Clippy、fmt/xtask/元数据、前端检查/构建和独立Core/GUI构建通过。已备份并更新为Core PID285544、GUI PID291160，ready/State31/Config12，原2项目/2仓库、数据、设置/profile保留，GUI响应。100ms与失败呈现保持，无新DTO/Schema/权限或迁移，旧未执行包计划需重建；SDK专用Unity规则保持原选择。证据、失败轮次与未覆盖项见[记录](testing/vpm-seven-gaps-20261109.md)。未实际卸载用户SDK；所有者随后授权本项本地提交，未推送，不代表全部VPM/v3对齐或H7/M11签收。

- 2026-11-09 所有者确认修正后授权本地提交：本轮将此前已验证的软件包Core/合同修正、GUI请求隔离、软件包确认框及加载展示分组提交。实现及运行产物不再改变，按输入摘要复用已完成验证；不推送，不扩大H7/M11范围。下方“未提交”等文字为各项完成当时的历史状态，当前交付以本条及对应提交为准。

- 2026-11-09 按所有者授权修复前轮四处VPM/v3行为缺口：未锁定本地包的清单依赖纳入保护/冲突/指纹及Resolve、批量入口；卸载只清理所选包旧依赖闭包；单包重装不再要求无关来源；计划前缺失目录可按冻结证据卸载，计划后变化仍拒绝。普通操作不扫描payload，内部缺失证据不进入公开RPC，无Schema/权限变更；旧未执行包计划需重建。337项不同Rust测试、受影响严格Clippy、fmt/xtask/元数据、前端检查/构建通过；原生RPC在更新暂停旧Core后通过，含既有kill/restart矩阵。已备份并更新为Core PID260200、GUI PID272600，ready/State31/Config12，原2项目/2仓库、数据、设置/profile保留，GUI响应正常。证据及失败轮次见[四处缺口修正记录](testing/vpm-four-gaps-20261109.md)。未实际卸载用户SDK、未提交/推送，不代表全部VPM/v3对齐或H7/M11签收。

- 2026-11-09 按所有者要求更新Core/GUI并再次审计VPM管理行为：已验证产物部署为Core PID279216、GUI PID271640，ready/State31/Config12，原数据/profile、2项目/2仓库及设置保留，完整备份位于`target/m7-package-removal-update-20261109/`。含SDK卸载确认、表头先行及加载条直角修正。在线官方资料、只读v3/当前源码及独立夹具核对仍确认四处缺口：未托管项目包依赖保护、无关孤立包清理范围、单包重装的无关来源依赖、计划前目录缺失的卸载Apply。具体证据、正向Resolve对照、保留设计差异与限制见[复查记录](testing/vpm-management-audit-20261109.md)。未修改这些新发现的生产行为，未操作用户项目包；未提交/推送，不能宣称全面对齐，M7/H7保持。

- 2026-11-09 SDK卸载异常已按所有者批准的[窄合同](exec-plans/M7-package-removal-confirmation.md)修正：Core按锁定记录接受直接/传递目标并实际移除，冻结保留依赖方的冲突，由原确认框审阅后Apply；纯批量资格及GUI预检同步，无来源不阻挡卸载，混合意图仍拒绝破坏依赖。公开DTO/Schema/SDK配套，无迁移/新权限，失败呈现与100ms保持。380项不同Rust测试及48项不同浏览器用例的最终通过范围、严格检查、生产中文截图、独立Core/GUI构建和失败轮次见[记录](testing/package-removal-confirmation-20261109.md)。实际项目仅只读计算，Avatars/Base均为1项移除及正确冲突，未实际卸载SDK或替换当前Core/GUI；未提交/推送，M7/H7保持。

- 2026-11-09 软件包表头先行：按所有者要求，项目标题/工具栏及表头提前显示，仅表体等待；无包行时禁用全选，加载改为表头下全宽4px Material线性进度条，无重复可见文字。100ms导航、160ms局部指示延时及后台已加载行保持不变。前端检查/生产构建和39项现有浏览器回归通过，生产夹具浅/深色、窄窗口及表头DOM/位置稳定核对通过。未更新本项原生GUI、未提交/推送，M7/H7保持；见[记录](testing/package-loading-style-20261109.md)。

  所有者随后指出加载条只有下方圆角；定位为通用末行圆角规则覆盖加载单元格直角设置，已仅提高该加载态规则的特异性。浏览器对照确认四角0px，正常包行下圆角16px及表头/条尺寸保持。浅/深色与窄窗口核对及前端检查/生产构建结果另见同一记录；前轮尺寸检查未覆盖裁切，未将其计为本问题通过。运行GUI尚未更新。

- 2026-11-09 按所有者后续要求，前述包计划性能及原生RPC隔离修正已重建并更新Core/GUI。Core PID288700、GUI PID283656，ready/State31/Config12；原数据/profile及2项目、2仓库保持，设置摘要一致，更新前0活动任务，完整数据和旧程序已备份。运行摘要和备份在`target/m7-core-gui-runtime-update-20261109/`。原生UIA确认项目列表，之后所有者Esc中断桌面操作，未继续执行原生管理页/包操作复验；不视作H7签收。此更新早于本项软件包加载样式，不含本项表头先行修改。未提交/推送。

- 2026-11-09 修正耗时包操作阻塞所有页面的问题：GUI 原生桥接不再持唯一连接锁等待全部 RPC，按页面读、耗时操作和状态/取消隔离有界连接池，保留仓库预览和扩展会话的连接绑定；中断流不复用，Core 重连淘汰各池旧连接，不重放写请求。生产切页验证另定位并修正初始读挂起重试绕过同一 thenable 导致 React 整根恢复，100ms与版本首帧保持。原生/客户端29项通过，正常及4倍CPU各3轮跨页验证零React错误/迟到弹窗/Apply，首帧及慢读取边界通过；严格Clippy、fmt、xtask、前端检查/生产与独立GUI构建通过。失败处理保持现状，未提交、推送或替换实际Core/GUI，M7/H7保持。证据与限制见[记录](testing/gui-rpc-concurrency-20261109.md)。

- 2026-11-09 按所有者纠正包操作关键路径：普通包计划/复验改为仅读取清单、已锁定包目录身份和包元数据，不遍历包内容；单包及纯批量卸载依本地锁定依赖图计算，不重求解仓库版本。失败处理与 GUI 不改；下载摘要、旧资产证据、事务恢复及 Unity 迁移原有内容校验保留。最终282项不同测试通过、3项既有忽略，受影响严格Clippy、fmt、xtask和独立Core构建通过。实际项目两轮只读计算约28–120ms，不代表完整GUI/RPC耗时。普通包旧未执行计划需重新生成；未提交、推送或替换运行Core/GUI，M7/H7保持。范围、失败轮次及未覆盖项见[记录](testing/package-plan-inputs-20261109.md)。

- 2026-11-09 再按实际渲染位置检查页面提示：清除代理保存错误、VCC注册结果、仓库筛选错误文字及独立刷新失败播报；保留原有重试控件。项目列表、Unity安装列表和日志页区分首次读取失败与已加载内容，成功加载的空列表后续读取失败也不追加或替换为错误提示。确认框内错误、首读失败重试及业务完成回调保留。最终GUI检查和33项定向浏览器回归通过，另复用本轮此前通过且末次改动未影响的相关回归；修正夹具选择器及旧读取流程断言的失败轮次保留在`target/m7-page-feedback-audit-20261109/`。代码已分批本地提交`b3e8cbd`、`c1d9c26`；GUI生产及原生构建通过，源码摘要与已验证候选一致。新版GUI PID276768已使用原profile打开并响应，产物摘要核对一致；原Core PID269324、启动时间、可执行文件及数据目录保持，Unity未变。构建与运行摘要在`target/m7-page-feedback-update-20261109/`；未执行原生画面复验，未恢复桌面控制。并行出现的其他Core修改保留且未纳入本轮提交，未推送，M7/H7保持。

- 2026-11-09 补清软件包操作的页面提示遗漏：移除确认框外的失败文字与任务进度；计划读取失败保持页面内容，Apply失败留在原确认框并阻止重复提交，关闭后清除错误。任务后台跟踪和任务中心结果保留。GUI检查及5项定向浏览器回归通过，包含截图中的package_not_found、原操作重试、Apply失败关闭和任务终态；证据在`target/m7-package-page-feedback-20261109/`。修正提交`b3e8cbd`并已包含在上述更新的GUI中，M7/H7保持。

- 2026-11-09 版本菜单完整展开：原实现点击后才请求且提示延后160ms，造成空菜单。按所有者补充的v3体验，改为悬停/聚焦时按包准备，点击最多等待100ms；快速读取完整展开，超时显示非选项加载状态，分页完整前不显示部分列表，保留取消、固定快照及两请求并发。生产模式正常/4倍CPU快速和慢速对照无空菜单；最终86项不同浏览器回归及检查/生产/原生构建、元数据、xtask通过。GUI PID237824已更新打开，原Core与数据/profile保持；所有者已确认保持设计并授权本条所属的本地提交，提交前源码摘要与验证一致。验证范围和限制见[记录](testing/package-version-menu-loading-20261109.md)。未推送，M7/H7保持。

- 2026-11-09 所有者确认首帧显示正常并决定保持当前设计，授权本地提交。本轮分为Core/协议的`44654ca`和本条所属的GUI配套提交：保留100ms导航、按需后台刷新、完整结果一次更新和已安装版本首帧显示；数据不变时允许React内部比对，不额外引入跳过更新的优化。已按源码摘要复用既有390项Rust及相关浏览器/生产模式实证，提交前工作区前端检查、Rust fmt、元数据、xtask和diff检查通过。原运行窗口和Core保持，未推送；本次页面效果确认不扩大为H7全部人工签收。

- 2026-11-09 首帧版本文字后续：已安装版本随首屏读取提供；普通effect仍被Material空选项初始化清空，前轮只验证稳定后的文字，未证明首帧。现使用公开Lit controller保持显式displayText，生产模式正常/4倍CPU各3次访问，旧版6次首采样空白，新版6次首帧及后续72次采样无空白；81项相关浏览器回归、前端检查/生产构建通过。100ms及后台完整准备后一次更新保持；原生构建/更新及限制见[记录](testing/package-first-frame-version-20261109.md)。未提交/推送，M7/H7保持。

- 2026-11-09 软件包管理页闪动纠正：实际是后台刷新清除显示证据，导致升级按钮反复撤除、Select禁用底线改变外观。按所有者最新要求，后台准备完整列表/筛选/批量信息后一次更新，前台保持显示及交互，实际变更仍由Core固定快照校验；来源关闭/修改继续隔离旧证据，100ms导航保持。另撤回前次Select文字同步前移，生产模式验证其会清空版本文字。前端/原生构建、元数据及xtask通过，最终范围覆盖121个不同浏览器用例（119通过、2时序断言修正后整个文件23/23通过）；生产模式正常与4倍CPU对照通过。最终GUI PID272240已打开，原Core PID269324和数据/profile保持；实际版本文字恢复，首次原生逐帧因用户正在操作未完整记录。合成帧、失败轮次和限制见[记录](testing/package-background-refresh-flicker-20261109.md)。前次仅导航空档修正不能视为解决本问题。未提交/推送，M7/H7保持。

- 2026-11-08 首次进入软件包管理页闪动：原生连续截图与浏览器合成帧定位到延后挂载导致只读请求启动过晚、越过100ms预算后显示整页fallback。导航时启动同一访问请求、同步消费已完成结果，并前移Select首帧布局，100ms截止规则保持。最终慢速29包对照中读取启动约236ms降至约5ms，主体空白采样28帧降至0；152项前端相关回归、检查/构建、元数据及xtask通过。独立修复程序构建后曾因所有者Esc中断桌面操作而保留旧窗口；随后按所有者新要求已替换并打开修复版，GUI PID277736，原Core、数据/profile保持，ready/State31/Config12。新版项目列表画面正常，管理页首次进入的原生效果仍待人工确认。证据、限制及程序摘要见[记录](testing/package-first-open-flicker-20261108.md)。本轮未提交/推送，M7/H7保持。

- 2026-11-08 按所有者要求完成本轮[软件包页面整体对照清单](exec-plans/M7-package-v3-parity-audit.md)：纯版本菜单、去重与非选项分组、最新列升级、直接安装/移除、菜单顺序、来源显示及Shift选择。手动Unity不兼容版本的窄合同已明确获批，Core资格、确认、精确根目标及固定计划证据配套实现，无新增数据库迁移。119项浏览器及390项不同Rust测试通过，4项既有显式忽略；定向严格Clippy、fmt、xtask、形状/元数据、前端与原生Core/GUI构建通过。Core/GUI已配套更新并打开，保留原数据/profile，ready/State31/Config12，2项目/2仓库/0活动任务；详情、失败轮次及保留的v4差异见[验证记录](testing/package-v3-parity-20261108.md)。当前为`4206ad38`后未提交工作树，未推送，H7人工状态保持。

- 2026-11-08 按所有者要求修正当前版本列与v3的展示差异：移除额外“请求版本”行，撤回标记移至当前版本并使用错误色，调整列宽确保常用状态完整可见。Core依赖事实与逐仓库刷新设计保持。前端检查/构建、最终65项浏览器回归和原生GUI构建通过；中间裁剪失败、最终截图与更新证据见[记录](testing/package-version-v3-alignment-20261108.md)。GUI已使用原profile重开，原Core进程保持；当前为 `4206ad38` 后未提交工作树，未推送，H7保持。

- 2026-11-08 按所有者要求补齐仓库按需后台刷新：成功的本机仓库/候选读取先返回已有索引，独立 Core 任务按5分钟时效刷新，合并通知、跳过关闭仓库并对失败限频；手动刷新保持立即执行。沿用现有用例、条件请求、锁和事件，无新增 RPC/权限/Schema。受影响 Rust 209通过、1既有忽略，严格 Clippy、all-targets check、fmt、xtask、元数据、公开形状及 Core locked 构建通过。Core已更新并 ready，GUI沿用原构建/profile重开；两份内置仓库已自动检查且语义 revision 不变，项目/计划/操作和设置保持。本轮已按所有者要求本地提交，未推送，H7保持，见[记录](testing/repository-auto-refresh-20261108.md)。

- 2026-11-08 按所有者最新要求完成撤除额外项目展示数据库副本与补采流程，落实[正常读取合同](exec-plans/M7-package-presentation-completion.md)：普通候选请求由 Core 直接读取项目文件，分页仅暂存有界内存结果；GUI 不再先调用 projects.refresh 填库。受影响 Rust 460 项通过、3 项既有忽略，82 项浏览器回归、严格 Clippy、fmt、公开形状、元数据、xtask、前端检查/构建及原生 Core/GUI 构建通过。现有数据仅删除两张展示表，其余 34 张表记录、设置与模板文件一致。Core/GUI 已配套更新并打开，ready / State31 / Config12，原 profile 保留。本轮已按所有者要求本地提交，未推送，H7 保持；详见[验证与更新记录](testing/package-direct-read-20261108.md)。

- 2026-11-08 中间页面读取修正（已由上项取代）：移除新增的信息不完整区域及独立按钮后，曾通过 projects.refresh 自动核对并补充 State30 展示副本。该流程已按所有者最新要求撤除，仅保留[当时验证记录](testing/package-presentation-refresh-flow-20261108.md)作为历史证据。

- 2026-11-08 软件包展示补齐完成本地实施：[候选 v2 与 State30](exec-plans/M7-package-presentation-completion.md)已获所有者批准并配套更新 Core、协议、SDK 和 GUI。补齐本地安装元数据、说明/搜索词、撤回、SDK/legacy 显示规则；已安装/未安装名称层级不同，可见 provider 直接提供来源标签/顺序，版本按 Core Unity 分类分组。旧观测有明确状态与手动补采入口。受影响 Rust 461 通过、3 项既有忽略；严格 Clippy、GUI Rust all-targets、fmt、xtask、公开形状与元数据检查、全工作区前端检查/构建通过；浏览器 86 个独立用例覆盖与验证复用范围见[记录](testing/package-presentation-20261108.md)。源码已提交为 `e0073759`，并按所有者后续要求更新本机 Core/GUI：State30 / Config12、候选 v2、ready，GUI 已打开且响应；原数据 33 张表的旧列记录与设置完整保留，沿用原 WebView profile。备份和核对见[更新记录](testing/package-presentation-update-20261108.md)。未推送，H7 人工状态保持。

- 2026-11-08 按所有者澄清修正项目候选可见性：Core 在形成软件包/provider/版本候选前排除关闭来源和关闭的预发布版本，不再下发禁用的预发布条目。安装/声明事实保留；无可见 provider 时显示“未知来源”，与本地缺少存储库的来源证据一致。另隔离设置 revision 改变前的旧候选。前端检查/构建、62 项定向浏览器、Core VPM 完整回归 163 通过/3 既有忽略、应用层 4 项、RPC 合同 5 项、fmt 与受影响严格 Clippy 通过；早期失败与运行环境边界见[验证记录](testing/package-candidate-visibility-20261108.md)。未提交、推送或部署运行中的 Core/GUI，M7/H7 状态保持。

- 2026-11-08 文件选择器重置基线：按所有者授权移除旧选择器专用提交，混合提交只保留共享弹窗动画；旧 `FilePicker` / `FilePickerControl` 及 CSS 全部不存在。相关入口直接拼接路径输入框和原生选择按钮，GUI 类型、Material 与本地化检查通过。下一步仅重建并接入明确指定的四处。历史重写后的 SHA 与既有 CI 证据 SHA 不同，历史结果不作为新候选通过；当前 Core/GUI 均未重启。
- 随后按已确认外观重建唯一 `FilePicker`，仅接入仓库列表导入、模板导入、添加已有项目和添加用户软件包；其余入口使用普通路径输入框和选择按钮。全工作区前端检查/构建及定向浏览器回归通过；早期旧焦点断言与截图宿主错误、复验范围及原生限制见[重置与重建记录](testing/file-picker-reset-rebuild-20261108.md)。软件包排序、100ms 等待与顶部加载条保留；未部署或主动重启原生 GUI/Core，未操作真实项目。

- `main` 当前工作树使用 [State31](../specs/storage/state-v31.md) 与 [Config13](../specs/config/settings-v13.md)。State31 清理撤除的项目展示副本，保留仓库规范化索引与 State29 仓库显示名；内部数据格式编号不代表产品发布版本。仓库操作资格、项目列表、包批量操作、Unity 版本选项和操作日志筛选由 Core 提供。GUI 使用同一套用例结果。
- 此前 Core 部署基于 `0a73acca`，报告 State29 / Config12 和 `ready`，已核对九组业务读取。历史证据位于 `target/m7-core-boundary-update-20261107/`；当前新路径运行环境见下方记录。
- 随后的 GUI 单独更新基于 `0a73acca` 加文件选择器工作树改动，记录了窗口响应、GUI profile 和同一 Core 进程。证据位于 `target/m7-shared-file-picker-update-20261107/`。CLI 仍匹配 `7fe5f157` 部署记录中的产物摘要，见 `target/m7-repository-core-update-20261107/`。各自的证据只覆盖对应构建。
- 此前一次 GUI 部署使用 `34cac0a2` 加当时文件选择器工作树修改，证据位于 `target/m7-file-picker-integration-update-20261107/`。当轮验证后恢复原 Core/GUI 二进制、数据目录和 profile；九组业务读取一致，Unity 进程未变，没有另行部署。
- 2026-11-08 当前工作树将所有页面的切页首读等待统一限制为 **100ms**，超时显示目标页加载状态，不再无限停留于旧页。项目页先显示标题/工具栏，再独立加载列表或卡片；项目管理页的元数据与软件包首屏共用剩余预算，超时只让包列表继续读取同一个请求。旧响应隔离、分页/修订/权限及操作资格保持。前端全工作区检查与构建通过；浏览器扩展轮覆盖 150 项，其中 149 通过、1 项旧等待时序断言失败，修正测试后导航/管理页 **25/25** 通过；项目初始读隔离收束后导航/项目修改时间 **24/24** 通过，未变范围复用前轮结果，均无自动重试或跳过。证据位于 `target/m7-package-navigation-100ms-20261108/`。本轮未替换或重启原生 GUI/Core，未操作 Unity；Rust/远端 CI 未重跑，M7/H7 状态保持。
- 上一次 GUI 部署使用 `aced843c` 加软件包排序及“立即显示管理页基础内容、仅软件包列表延后加载”工作树修改，原 profile 保留，前端检查/构建、相关测试 **92/92** 及 Tauri debug no-bundle 构建通过。当时记录 GUI PID260640、原 Core PID218092；证据位于 `target/m7-package-list-loading-20261108/`，更早整页加载方案位于 `target/m7-package-navigation-20261108/`。该原生构建不包含当前全局 100ms 修改，浏览器结果不代替本机窗口内部人工验收。
- 所有者随后要求重建并打开当前 GUI：Tauri debug no-bundle 构建通过，复用前端的五份源码及 JS/CSS 摘要与已验证输入一致。新窗口 PID255916 已打开且响应，产物 SHA256 `62DAF418ED7506339CF757AC09CA0BD33B62F93D6BC5B7FE8D1D7C27B97BAD55`。重开前未发现 GUI/Core 进程；此前数据目录 `target/m7-icon-acceptance-20260906/data` 和 WebView profile 均不存在。窗口使用本轮独立临时 profile，VCC 注册关闭；尚未启动 Core、恢复备份或替换数据，数据来源等待所有者指定，不能计作实际项目页面交互通过。启动证据位于 `target/m7-package-navigation-100ms-20261108/native-reopen/`。
- 所有者已允许新路径，当前 Core locked debug 构建通过，PID263632 使用 `target/m7-package-navigation-100ms-20261108/native-reopen/data` 初始化新数据并报告 `ready`，自动发现 1 个项目，未恢复旧备份。GUI PID255916 按要求保持运行，可用现有“重新连接并重试”入口连接；桌面控制未获准操作该 GUI，未验证原生按钮后的结果。Core 及 GUI 身份、只读 RPC 证据仍在上述 `native-reopen/`。
- 当前源码新增整页加载条：4px Material 不定进度条位于主内容画布顶部，沿用主题和减少动画设置；局部列表/弹窗加载边界保持。前端检查/构建通过；67 项范围首轮 64 通过、3 项新增测试的宿主定位错误，修正测试后导航/管理页 **28/28** 通过，未变产品源码和其余 39 项复用首轮结果，零自动重试/跳过。证据位于 `target/m7-page-loading-bar-20261108/`。现有原生 GUI 尚未重建进该加载条，M7/H7 人工签收保持待定。
- 本轮源码整合候选为 `213d0d2`：独立文件/文件夹选择内容接入仓库列表导入、模板导入、添加已有项目和用户软件包登记；调用方负责确认、预览及弹窗生命周期。另同步旧浏览器预期、State29 精确断言，以及 Config12 和已有平台功能的元数据校验。只有标准 `main` 工作树，本地归档分支均为其祖先，无 stash；截图及生成材料保留在本地。
- 该候选的完整 Rust fmt、严格 workspace Clippy、workspace 测试及仓库检查通过，测试汇总 772 通过、13 项既有显式忽略。前端全工作区检查/构建和 Chromium **514/514** 通过，零重试、零跳过；元数据及 7 组内存反例通过。指纹已核对各验证范围的输入未变；早期失败轮继续保留在 `target/m7-main-integration-20261107T040906Z/`。原生系统选择窗口仍待人工核对。
- `e5ede89f` 的 [CodeQL 42340358565](https://github.com/ALCOMD/ALCOMD/actions/runs/42340358565) 四语言通过，[CI 42340358982](https://github.com/ALCOMD/ALCOMD/actions/runs/42340358982) 三平台未通过：Unix 测试模块位置和 Windows 专用 HTTP 测试钩子的编译范围触发严格 Clippy；Windows 两项设置测试争用用户 IPC 端点。`de3c06a`、`41a1653` 修正这三处测试边界，保留原断言及生产端点合同。默认并行 workspace 回归 772 通过、13 项既有忽略，两线程设置 IPC 额外三轮共 6 项通过，fmt 和受影响严格 Clippy 通过。原 Core、数据和 profile 保留，GUI 维持原先关闭；Unity 原有编辑器新增导入子进程，清单不完全相同，未操作这些进程。证据位于 `target/m7-ci-e5ede89f-20261107/`；后续 Hosted 结果按实际推送 SHA 从 [Actions](https://github.com/ALCOMD/ALCOMD/actions?query=branch%3Amain) 核对。
- `b1ad980c` 的 [CodeQL 42372423392](https://github.com/ALCOMD/ALCOMD/actions/runs/42372423392) 四语言通过，[CI 42372423496](https://github.com/ALCOMD/ALCOMD/actions/runs/42372423496) 继续发现测试生命周期问题：Unix 四项版本探测夹具并行争用两个生产探测名额；Windows 私有管道创建测试在终态后的归档清理释放前删除目录。整改仅为共享探测夹具互斥，以及私有测试运行时释放后再删除目录；生产并发上限、超时和业务断言保持。定向 fmt、Windows Core/平台严格 Clippy、Linux/macOS 平台跨目标严格 Clippy 通过；私有管道创建测试连续 8 轮通过。跨目标检查不算 Unix 原生执行，后续 Hosted 结果以实际修复候选为准；证据仍存上述目录。
- `af1bf478` 的 [CodeQL 42374353138](https://github.com/ALCOMD/ALCOMD/actions/runs/42374353138) 四语言通过，[CI 42374353476](https://github.com/ALCOMD/ALCOMD/actions/runs/42374353476) 三平台 Check 和仓库契约测试通过；Unix 发布与 GUI 构建通过，浏览器为 Ubuntu 512/514、macOS 510/514。`d295d1a`、`2400ce0` 保留原断言，修正正常菜单关闭后的继续操作、弹窗缩放未稳定时采样，以及异步保存尚未接受就断言原生 checkbox 立即变更的测试时序；设置夹具可暂停/释放下一次提交。两项菜单各重复 8 轮通过；设置两项修复前 20 次中 9 次失败、修复后 20 次全通过；四个受影响文件共 44 项通过。证据位于 `target/m7-ci-af1bf478-20261107/`。
- 同轮 Windows 发布与 GUI 构建也通过，浏览器 339 项通过后触及 60 分钟 job 上限，被 GitHub 中断（2 项 interrupted、173 项未运行）；不是完整浏览器通过。仅将 Windows 作业上限改为 90 分钟，保留全部步骤、单用例超时、断言和其余平台上限。
- `7720f77a` 的 [CI 42379056402](https://github.com/ALCOMD/ALCOMD/actions/runs/42379056402) 三平台全部通过，各平台 Chromium 均为 514/514；[CodeQL 42379055861](https://github.com/ALCOMD/ALCOMD/actions/runs/42379055861) 四语言全部通过。结果绑定该候选，证据位于 `target/m7-ci-7720f77a-20261107/`，此前失败与中断记录保留。
- Material Web 菜单快速重开行为已由项目所有者评估：当前遵循[官方公开 API 与用法](https://material-web.dev/components/menu/)，保持现状，不增加规避逻辑，不列为待修、M7 整改或验收阻塞项。正常菜单功能、焦点和滚动条的正式测试保留，不另增快速重开专项 CI。

## 按任务查合同

| 任务 | 实施合同 |
| --- | --- |
| GUI、仓库、模板、操作日志、共享组件、可访问性、Portable UI | [M7 ExecPlan 任务索引](exec-plans/M7-official-gui-extension-ui.md) |
| 项目动作、包管理 | [项目与包功能收口](exec-plans/M7-project-package-functional-closure.md)、[包候选合同](exec-plans/M7-package-candidate-evidence-stop-a.md)、[VPM 事务](exec-plans/M4-package-transactions.md) |
| Unity 项目版本迁移 | [Unity 版本迁移合同](exec-plans/M7-unity-project-version-stop-a.md) |
| GUI 与公开 RPC 用例对照 | [M7 GUI RPC 覆盖](exec-plans/M7-gui-rpc-coverage.md) |
| Discord 第一方扩展（M9 已完成） | [M9 实施顺序、公共合同与验收记录](exec-plans/M9-discord-first-party-extension.md) |

## 剩余边界与下一停止点

- H7沿用所有者整体认可及条件收尾授权；新增修正按实际门禁和运行交付验收，不以浏览器fixture冒充原生人工操作。Portable UI原生表单/超时/外部断线等旧未覆盖项保持，见[桌面验收清单](testing/m7-official-gui-manual-checklist.md)。
- 最终功能证据绑定`c46f7544`；此前候选的失败、分轮结果和历史三平台基线分别保留，不替代本轮完整通过。收尾文档为后续独立提交，不声称其新CI已完成。
- 14项原有`#[ignore]`未全部直接单跑；子进程入口部分由父回归调用，联网VRCFury、Windows单实例/注册表、显式公开仓库快照及专用HTTP/代理隔离项不计作本轮直接通过。
- v3 真实工程差分夹具仍属 M11。原生安装、更新、卸载和完整发行验收仍属 M12。后续里程碑按各自授权推进。
- M7 已正式完成，M8 已按所有者要求收尾，M9 和 M10 已验收完成。Discord 启用状态迁移和随包装配分别归入 M11 和 M12。原生 Core 安装登记由 M12 安装器维护。[M10 验收记录](exec-plans/M10-native-sdk-and-rpc.md#2026-11-20-验收结果)

[M7 状态与验收历史](testing/m7-status-history-through-20261107.md) 保存原状态页的候选、签收、遗留事项、失败轮次及证据链接。
