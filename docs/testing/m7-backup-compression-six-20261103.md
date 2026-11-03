# M7 备份压缩六档

状态：实现、集中本地回归及配套桌面更新完成；M7/H7人工状态保持。

## 合同

所有者在已说明Config9/State18最小合同后，以NanaZip截图明确选择0/1/3/5/7/9六档（A-038）。
设置默认值、单次备份及备份详情共用六档名称：仅存储、极速压缩、快速压缩、标准压缩、最大压缩、极限压缩。
store/fast/maximum仍表示0/1/9，新增level3/level5/level7；默认仍fast。CLI/RPC和实际ZIP编码同步。
Config9只扩展枚举，旧配置的已有值及revision保留。State18只扩展backups该CHECK，保留原rowid、所有行/列/约束、显式索引/触发器及恢复计划外键；提交前实际检查外键，失败回滚并恢复FK开关。
归档v1同步扩展压缩枚举，旧归档/已接受任务/指纹保留；旧程序不承诺读取新增等级归档。

## 已执行

- GUI check、根前端build、metadata及Rust fmt检查通过。
- 浏览器首轮6/3：新三语用例错误读取只含revision的settingsWrites作为设置正文，选择器本身、真实选中及其余6项流程通过。修正测试读取settingsRequests.update，并同时核对提交计数，不改产品逻辑/超时/断言要求。
- 同组独立重跑9/0，覆盖三语六档真实选择及保存、默认继承、单次level3覆盖不回写、设置level5/level7、读取失败和项目列表回归。日志位于target/compression-six-browser与-r2，分别保留。
- State18纯SQL的内存SQLite成功、两种rollback烟测已执行。Rust真实恢复计划外键/原rowid/全部表行/索引/触发器保留、故障回滚和损坏外键拒绝测试通过；六档×包/Library共24组合均实际归档、发布、提取恢复并核对正文。
- 首轮完整Rust604/1、8既有helper ignored；唯一失败为旧store_handle测试仍将18当作未来版本，已改为当前版本+1，保留未来版本拒绝及原状态不变的全部断言。修正后独立完整Workspace **605/0、8既有helper ignored**，不把窄补验与首轮拼成完整通过。
- 严格Workspace Clippy、Rust fmt、xtask和metadata通过。工作区另有提醒界面改动，保留后针对六档整合复验9/0（target/compression-six-browser-r3），不冒充完整浏览器套件通过。

最终467项代码/契约/构建输入记录为target/compression-six-inputs-final.json；此前GUI整合快照和版本拒绝测试修正分别保留。完整回归使用临时合成夹具，原数据未用于测试。首轮失败后原Core/GUI已实际恢复；最终回归通过后按本轮授权配套构建更新，升级前保留原程序、配置及关闭Core后的数据库/WAL/SHM文件。日志分别位于target/compression-six-desktop及-r2，不运行真实工程备份，不提交或推送。

## 桌面产物与真实迁移核对

Core/CLI release构建通过（4分04秒），GUI release no-bundle构建通过（3分40秒，含GUI check/build）。构建前后Rust和压缩相关输入一致；期间工作区有独立的表格圆角样式调整，保留并由最终前端构建纳入，结束时输入差异仅styles.css。该样式及此前提醒改动的原生观感不作为本六档浏览器9/0的验收范围；不冒称全仓库相同SHA或完整浏览器门禁。

已沿用原数据目录及WebView profile打开Core **151912** / GUI **160368**，GUI有窗口且可响应。公开读取核对全部通过：State18和Config9就绪，3个Project、2个Backup、全部Operation、Editor登记及Status与升级前完全一致；设置只变化configSchema8→9，原全部值及revision168保持，compressionMode仍fast（1级）。没有运行真实备份/恢复或启动Unity。

当前Core SHA-256为`0796F00768027C8927EC3C4F2316B4674F0D7F49492A85FCAD0D56ABC44857E9`，GUI为`6AFD5F104B090B20294D0202220B71C050A5786FA51FFA8431D1278B1CDF93D2`。`target/compression-six-desktop-r2/verification.json`保存各组核对结果，candidate-runtime.json保存程序身份；最新运行身份同步至target/m7-storage-locations/candidate-runtime.json。原程序及迁移前数据库文件保留在该目录的original/中。

没有提交、推送、Hosted CI或新的原生人工验收；M7/H7和跨平台剩余边界继续以docs/status.md为准。
