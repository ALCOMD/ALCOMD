# M7 最终候选技术收尾

2026-11-12 所有者认可目前使用体验并提出结束M7，随后明确授权“允许临时关闭并恢复，执行本地收尾”。本项收束当前M7，不增加产品功能；M11真实v3差分与M12安装/辅助技术等继续保留各自范围。H2–H6已有人工签收不重做，也不把历史未执行项目改成通过。

## 授权与完成边界

- 仅在标准main检查、修正实际功能/门禁缺口，完整保留现有业务与安全断言；已批准合同引起的旧测试/元数据失配按当前合同修正，不放宽要求。
- 按同一最终候选记录完整本地fmt/workspace Clippy/workspace tests/xtask/前端检查构建/metadata、基线冻结校验、CLI自动启动、完整浏览器、release bins及Tauri no-bundle。优先使用scripts/check.ps1，完成项按输入摘要复用，不叠加完整聚合脚本重复执行。
- 临时关闭前核对权威活动任务为0，正常关闭准确GUI并等待Core退出，离线备份旧程序、原data/profile并逐文件核对；测试仅使用隔离夹具，结束后恢复原数据和窗口，不关闭Unity/Hub。
- 当前源基线645e0141；远端main仍是978b1de7。最新该远端SHA三平台CI的Check均失败，CodeQL成功；完整日志读取返回HTTP403，不能以本地复现替代远端根因证据。
- 首个确定的本地缺口是metadata仍冻结Config12；当前已批准Config13。同步显式版本、协议常量、完整历史Config11/12检查及Config13的封闭字段/容量/默认值，不改生产DTO或Schema。
- 所有者随后明确授权本轮分次提交、正常推送和更新本机程序，包含现有9个前置提交及收藏/通知增量。正常推送后取得同SHA三平台CI/CodeQL；这些门禁未通过前不宣布M7正式完成，不强推、不发布，不自动推进M8。

## 人工认可

用户本次的整体使用判断作为当前功能体验认可记录；技术完成与最终阶段状态依照本项结果更新。原生表单/超时/外部断线、读屏和真实v3等未覆盖范围如实保留，不以脚本或浏览器伪造原生人工证据。

## 收尾发现与候选修正

完整工作区检查发现目录选择器合同测试仍匹配加入currentPath前的签名，现按既有批准行为检查路径转发，同时保留项目打开与权限限制。完整浏览器初轮e5d090eb为665通过/8失败：扩展进度/权限刷新定位、中文和日文软件包友好名称、多来源版本可选、两处创建完成窗口及设置页Unity控件均存在过期断言。首次44项定向复验为39通过/5失败，补齐Material Shadow DOM内容定位与第二次刷新后，原8个失败场景全部通过。

上述收尾修改仅涉及元数据校验、测试夹具保护和现行行为断言；没有生产功能、公共RPC、数据库Schema、权限或依赖变更。最终候选仍须完整复验，不能将中间轮次拼作完整通过；失败日志、截图与trace保留在target/m7-final-closure-20261112和target/m7-final-candidate-20261112。

## 收藏按钮选中态补充

所有者在推送确认答复中提出收藏应显示实心；该答复不构成推送批准。共享项目收藏按钮现按Core返回的favorite选择实心/空心星形，列表和卡片一致，沿用原有aria-pressed、文字、颜色及失败行为。新增的24px fill-1 SVG来自既有固定Material Symbols提交，保留原几何及SHA-256；资产门禁逐项核对fill轴、路径、来源及摘要，没有新依赖或业务合同变化。

前端检查/生产构建、36份图标资产校验及项目偏好/Material基础的36项浏览器回归通过。覆盖已有收藏首次显示、收藏后实际SVG变化、跨视图保持、取消恢复空心及写入失败不误显示选中。日志见target/m7-favorite-icon-20261112/browser.log及target/m7-favorite-build-20261112.log。所有者随后授权本增量分次提交、正常推送和更新本机程序，实际交付另记；1d1e5db的完整本地门禁仅作为修正前基线，不算本增量已完成最终候选验收。

## 应用内通知补充

所有者随后明确要求恢复应用内通知，并批准[实施方案](M7-in-app-notifications.md)。该授权取代此前视觉通知的删除约束；旧活动系统、业务日志补记及系统通知不恢复。本增量完成GUI会话内的结果提示、后台任务观察及模态/关闭动画的呈现去重，保留收藏实心修正。分轮192项不同浏览器回归及前端检查/构建通过，实际范围和中间失败纠正见[记录](../testing/in-app-notifications-20261112.md)。实现验证时未提交/推送/更新运行实例，所有者随后授权分次提交、正常推送和本机更新；1d1e5db仅为这些增量前的完整本地门禁基线，正式收尾仍需新的统一候选及适用远端门禁。

## 本轮推送的Linux检查修正

按所有者授权完成收藏/通知分次提交和正常推送后，a1f88a9e的Ubuntu任务131059573696在Check阶段明确报gui_select_unity_hub第291行unused_mut。直接读取已结束job日志取得证据；此前旧远端日志403不能作为本次根因。该变量仅Windows/macOS过滤器分支会赋值，Linux不赋值；现改为不可变builder和各平台条件重绑定，保留当前路径解析、可执行文件/app过滤和blocking_pick_file行为，不放宽-D warnings。此修正属于当前M7交付必需的门禁修复，单独提交/推送后重新构建GUI并沿原数据入口更新。无RPC、Schema、权限、依赖或迁移变化；验证和交付结果另记。

本机cargo fmt检查、alcomd-gui all-targets严格Clippy及5项picker_location单元回归通过。Linux实际编译验证待修正提交的Hosted CI，未以Windows通过代替Linux结果。日志见target/m7-notifications-delivery-20261112/picker-{fmt,clippy,tests}.log。

收藏/通知及Linux检查修正已按授权分次提交和正常推送，本机运行1f48e668候选已备份、更新及保全验证，见[交付记录](../testing/notifications-runtime-update-20261112.md)。当前新候选三平台CI/CodeQL仍运行；未重新执行新统一候选完整本地门禁，不将此前1d1e5db或本轮分项结果拼接为M7最终签收。

## 继续跟进CI与最终签收

所有者明确要求持续跟进CI，全部通过后完成M7收尾；该授权包含当前M7必要修正的正常提交/推送以及最终完成记录。当前候选d5eab483的CodeQL已成功，但macOS job 131066931193在repository auto-refresh真实IPC回归初始化处失败：系统临时目录前缀加长夹具名导致Unix socket超出既有103字节可移植上限。修正仅缩短该测试的目录组成（保留完整UUID、系统temp_dir及隔离IPC；复用夹具根作为runtime，避免macOS系统路径别名规范化后再次超长），不硬编码平台路径、不改变生产传输限制、隔离或断言。随后以新的统一候选执行完整本地门禁并等待同SHA三平台Hosted CI/CodeQL；继续沿既有授权临时正常关闭/备份/恢复原Core/GUI，不关闭Unity/Hub。候选、失败与最终结果另记，不以先前候选的分轮结果代替最终签收。


统一候选074fadeb的scripts/check.ps1已完整通过，但浏览器回归在设置页重连处发现真实竞态：能力查询走仍活动的旧CoreEvents读取队列，随后旧会话停止取消该查询，使设置已读而能力保持不可用。原用例隔离复现3/3失败，改为通过原始typed client执行连接引导的systemStatus查询后3/3通过；普通页面/业务读取仍使用既有协调器，旧响应active/generation隔离保持。不放宽能力资格或测试断言。完整浏览器旧轮次因已确证失败而显式停止，失败附件/中断位置保留；后续新候选须完整重跑。

旧本地流程在aggregate通过后因元数据探针import产生未跟踪__pycache__被干净工作区保护停止，finally已按hash恢复原程序/数据/profile及窗口。该缓存已移至target证据目录，探针禁用字节码输出。后续通过逐文件输入摘要核对复用未变的Rust门禁，重新执行受影响前端全部门禁及未执行的CLI/发布构建；不把旧候选分项冒充最终完整通过。

## macOS 完整作业时间预算修正

0270967b 的 Ubuntu Hosted CI 和四语言 CodeQL 已通过；macOS job 131088329792 在完整浏览器阶段被平台取消。GitHub annotation 明确为「The job has exceeded the maximum execution time of 1h0m0s」：09:25:01 UTC 开始，前序检查、契约、发布构建、Tauri 和目标平台检查通过，10:12:21 才开始浏览器步骤，10:25:13 达到总时限。已取得的浏览器日志未报告测试失败，但完整套件未完成，因此不能算通过。原始日志保留于 target/m7-signoff-final-20261112/macos-027.log。

仅将 macOS job 总时限从60分钟调为90分钟，与既有 Windows 预算一致；所有步骤、浏览器单项超时、worker、重试数、断言和安全检查保持。不取消当前 Windows 作业，不以重试旧60分钟配置替代修正。修正后的同SHA三平台 Hosted CI/CodeQL 仍须全部通过，才满足远端门禁。

所有者同时提出默认备份体积和文件名问题，仍处于只读核对与讨论。最新两份真实归档的11,940个项目文件路径、未压缩大小和CRC一致；v3约935 MB、v4约1,180 MB。锁定依赖确认 v3 Fast 实际走默认等级6，v4 fast 为真实等级1；当前 vrc-get/ALCOM 上游也有相同选项映射。v4 UUID 文件名来自现行归档命名合同，并非中文编码损坏。证据见 target/m7-backup-audit-20261112/archive-comparison.json；尚未修改备份实现、默认值、命名合同或用户归档，不能在这些反馈处理完毕前宣布M7完成。

随后所有者接受“项目名＋时间”并选择默认等级5，按[备份修正计划](M7-backup-names-default-compression.md)实施。通过既有settings.update将当前实例压缩默认值由fast保存为level5，revision7→8，其他设置完整保持；真实旧备份不改名或重写。此后最终候选须包含该增量，前述只读状态为讨论当时记录。

f2689f42 的四语言CodeQL成功，macOS在第二次运行workspace tests（Test repository contracts）时发现另一个独立失败：storage_directories::tests::publishing_a_directory_never_replaces_an_empty_destination 创建目标时报NotFound；同SHA前一轮Check中的该测试已通过。夹具根使用PID+SystemTime纳秒显示值，不能保证并行调用不同，其他测试可能清理同一根。仅将测试夹具后缀改为进程内AtomicU64计数，与既有平台测试模式一致，不依赖系统时钟精度；生产发布函数及全部原断言保持。日志见 target/m7-signoff-budget-20261112/macos.log。该测试修正独立提交，最终Hosted macOS仍须验证，不以本机通过替代。

## 目录缺失增量与本轮候选门禁

所有者批准[目录状态与明确复制错误](M7-missing-project-directory.md)后，9f47336b 完成本机全部原生门禁：workspace严格Clippy、901项Rust通过/14项原有忽略、GUI检查、Discord检查/测试、前端check/build、元数据及12项反例探针、基线冻结、CLI构建/自动启动、release bins和Tauri no-bundle均通过。992份输入最终逐项一致。证据位于target/m7-signoff-missing-directory-20261112；该轮未运行完整本地浏览器，不能计为完整最终签收。

更新恢复时，已完成的离线原data/profile哈希核对通过，但生成目录target/debug已不存在，首次Copy-Item失败；未归因该目录消失的来源。仅重建原生成目录，沿原data/profile恢复已验证发布程序，并按产品清单补回CLI、扩展宿主等辅助程序，各项hash一致。Core79012/GUI134976连接、响应、ready，原4项目/30仓库/5模板/1Unity登记、完整设置revision8、备份记录、12个项目标志检查位及当时3个Unity/Hub进程保持；原删除仍failed/source_missing/revision9。Core实际报告2个available、2个missing，未恢复或回写用户已缺失的项目内容。此为程序/公开状态核对，不替代新增原生交互截图。

9f47336b四语言CodeQL成功，三平台Check及Ubuntu/macOS构建、平台目标检查通过；Ubuntu完整浏览器699/1在来源选项点击被下方表格拦截处失败，macOS699/1在菜单Escape后仍展开、焦点未返回处失败。原始日志为ubuntu-browser-failed.log与macos-browser-failed.log。两个旧用例在展开动画结束前即可继续点击或发送Escape，现复用Material公开opened/closed事件同步原有流程；所有真实点击、Escape、焦点、样式、选项资格、Core自动/显式来源及无业务派发断言保留，无force、数值等待、增加重试/超时或新专项用例。产品代码与所有者已接受的Material快速重开行为保持。

本机原来源用例6/6通过，未复现跨平台失败，不能写为修复前本机失败；调整后47项相关回归及两项原失败场景各3轮共6项通过，前端check通过。初次隔离重现因标题锚定筛选未匹配测试而未执行，已记录；有效重复轮次另存。证据位于target/m7-source-picker-repro-20261112。后续仅测试/文档变更，逐文件核对未变的生产、Rust测试、锁文件与构建输入后复用9f47336b原生结果；重新执行受影响前端检查、完整700项浏览器和新SHA三平台CI/CodeQL。最终全部通过前，M7仍在收尾。
