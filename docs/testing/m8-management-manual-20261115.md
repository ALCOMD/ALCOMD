# M8 管理扩展人工查看准备

## 2026-11-18 Store安装同意断言续修

远端223e8ce4的CodeQL44156608097四语言success；CI44156608608三平台均在Store旧12项断言失败，各15/1，前面的Schema模块各52/0，后续阶段未执行。新本地80ebb264仅同步一行至13，原权限及无副作用断言不变；本机原单项0/1、修复后模块16/0和窄严格检查分别保留，不拼作完整套件通过。按所有者本轮持续授权继续普通快进和自动检查，无需逐笔重审。

这次不占Core端点、不操作桌面或原数据，窗口和保全证据沿用已有恢复记录；扩展人工继续延期，M8不据此完成。以下为历史检查点。

## 2026-11-18 权限目录通过后的CI续修

远端337f4972的CodeQL44151487149四语言通过；CI44151487184三平台均为旧OAuth Schema直接引用断言失败，各51/1。权限目录修复后的生命周期各11/0/1原有ignored、实际MCP业务各12/0、STDIO各12/0，均按原始模块保留，后续阶段未执行，不记完整套件通过。新本地修复7b1a4f0e仅同步已批准的hello认证组合，先复现失败、修复后Schema模块52/0及严格检查通过；新修复的远端结果待取得。

本轮不占Core端点、没有操作窗口或原数据，原GUI84872/Core98828继续保留，程序与保全证据沿用前次恢复记录；未恢复桌面控制。所有者已授权本轮必要CI修复与普通快进推送直接推进，不再逐笔询问。扩展人工仍延期，不因技术结果记为通过；以下均为历史检查点。

## 2026-11-18 六笔推送后的当前状态

远端候选a94428b3的自动CodeQL44146171781四语言全部通过；CI44146171369已终态failure，三平台同一旧权限数量断言失败，各10通过/1失败/1原有ignored，后续阶段未执行。真实安装用例仍期待12项权限，已批准目录实际13项；本地b9948171仅改成完整13项精确断言，真实Core单项1/0及严格检查通过，未推送。该单项不代替完整CI或人工签收，历史失败保留，完整终态及三平台原始日志已归档。

定向验证后已恢复同一旧GUI84872/Core98828（fb72f0aa/3d6c2a1b、State42），被动确认窗口可见/响应，5条GUI自身管道连接正确Core。五组公开状态、原36/346/10/303份文件集合与摘要及五程序摘要保持。入口仍为下方Open-Review.ps1，当前窗口已打开，不重复启动；它不是本轮新A/B桌面构建。原开发环境保持原先关闭状态，没有恢复桌面控制或操作Unity。扩展人工继续延期，不能由本轮技术结果替代；以下PID和远端状态为历史检查点。

## 2026-11-18 OAuth订阅续验后的当前窗口

测试04a7fbec仅新增一条真实HTTPS/SDK/Core订阅用例，首轮1/0，覆盖权限上限、refresh重放、凭据与注册轮换/撤销；不是扩展人工签收或整个M8验收。自然到期/长时测试仍未执行。

当前旧窗口已恢复为GUI105140/Core105036（fb72f0aa/3d6c2a1b、State42），五组公开状态与测试前一致，原数据/profile集合及五程序摘要保持。启动入口仍复用下方Open-Review.ps1，窗口已打开；没有替换为新构建或恢复Computer Use。扩展人工继续延期。以下PID为历史检查点，不能据它们重复启动或关闭其他进程。

## 2026-11-18 A/B定向验证与窗口保留

本轮代码为71a1ffe9a77d4fd109fe844c41b55461bcbd1ba6，仅本地；新SDK/Core订阅、真实适配器关联及票据回包丢失最终单轮3/0，新只读Wasm/Host的授权/组省略/撤销单项1/0。它们是技术用例，不代替扩展人工签收。分轮失败与有效结果见现有M8 ExecPlan“2026-11-18 A/B 本地候选与定向结果”。

当前保留窗口是旧GUI fb72f0aa + 匹配旧Core3d6c2a1b/State42：GUI97464、Core97292，已被动确认可见/响应和5条自身管道，五组公开状态与暂停前一致。未将新State43程序指向旧data，也没有重新构建/替换这套桌面程序。准确启动入口仍是target/m8-remote-source-20261117/review/Open-Review.ps1；只在无运行实例且明确需要时使用，当前窗口已打开，不重复启动。

原开发环境此前关闭并保持其原状，四组36/346/10/303份文件摘要和五份旧程序摘要保持；没有恢复Computer Use或操作Unity。扩展人工继续按所有者要求延后，不要求重验已签收页面；普通可信HTTPS源、未覆盖表单/会话等既有事项保留原归属，未完成正式分发。新代码远端待验证，不能用e6d0b44b的CodeQL成功代替本轮结果。

## 2026-11-17 当前顺序：扩展人工延后，先测MCP

所有者要求扩展相关人工测试留到后续实际开发扩展时，当前先测MCP。普通可信HTTPS扩展源安装及其他明确未覆盖的扩展状态保留未验证，不要求现在提供源或重复扩展页面；具体后续里程碑未指定，不擅自写为M9/M12完成或豁免。

已使用本机Codex CLI0.153.4完成真实MCP接入，非GUI夹具/假客户端。一次性CODEX_HOME、独立MCP身份、仅可丢弃项目，未使用模型推理或操作真实项目：

- 回环HTTP Bearer场景：零授权连接但工具目录为空；明确授权后在新线程重新发现六工具，真实读取项目/包、创建卸载Plan（文件不变）、显式确认应用原Plan、查询Operation成功并核对实际包移除、同键返回同一Operation、撤权后旧线程调用拒绝。13项检查通过；此场景没有调用cancel_operation，不把已有SDK取消证据改记为Codex本轮覆盖。
- 预登记HTTPS/OAuth场景：公开测试证书只通过CODEX_CA_CERTIFICATE供本轮Codex子进程信任，校验证书及localhost，不关闭TLS校验。实际Codex生成S256请求、原Core/typed client明确审批、回调及授权码交换、仅获两项读取工具、真实Core读取，9项检查通过。HTTP协议驱动完成授权网页流程，没有桌面输入；普通浏览器人工体验、系统普通信任、实际Codex刷新/到期仍未验证。

实际Codex需在隔离配置启用`[features] mcp_2026_07_28 = true`才能使用这里的现代HTTP入口。其STDIO仍发initialize并被拒绝，不能因exe包含2026-07-28字符串就宣称默认兼容；没有为此给Core增加旧协议回退。两类失败及最初私有驱动的Tokio产物不匹配、错误的零授权六工具预期分别保留，详见ExecPlan。

证据在`target/m8-codex-client-20261117/`：HTTP最终`run-2fb96d14bc0e4ee2b68edae4a3617ff3/result.json`和独立`cleanup-follow-up.json`，OAuth独立`run-065067eea8ef43b7b972ec22266e6062/result.json`。本轮临时客户端及OAuth注册已撤销、凭据文件已清理、监听已关闭。样例登记清理曾误取外层revision，后经精确ID/目录身份核对取消登记，文件保留，最终0项目/0活动任务，原记录不覆盖。当前GUI85304/Core47824保持，原环境摘要不变，不恢复桌面控制。没有生产代码变更、全量或远端运行，M8未据此整体签收。下方“仍缺客户端”的段落是此前检查点，以上述实际结果更新。

## 2026-11-17 续查：窗口保持与真实接入缺口

当前GUI85304仍可见/响应，以5条自身管道连接Core47824；Core数据和WebView profile仍为`target/m8-remote-source-20261117/review`下的隔离目录。五程序摘要再次核对相符，原环境四组36/346/10/303份文件摘要保持。本轮只同步过时浏览器用例（7aed2b36，两相关模块单轮12/0），没有替换窗口程序或恢复桌面输入，也不要求重验已确认文案/页面。原3/1失败及其他轮次分别保留。

尚不能宣布下面两项原生接入通过：

- 在线安装仍缺实际普通可信HTTPS源网址。取得后从“扩展”主按钮输入网址→“继续”→选版本→安装，沿原签名条款/必需权限流程；右侧箭头才是本地文件。不使用已关闭的私有TLS测试服务或example.invalid。
- OAuth仍缺目标客户端及用户提供的受信任localhost证书/PKCS8私钥路径。当前候选实际`serve-oauth --help`和`mcp oauth registration set --help`已核对，记录在`target/m8-candidate-audit-20261117/`。须先在管理扩展创建隔离客户端、配置明确能力并取得ID，再用原CLI预登记客户端的实际回调URI；没有CLI create/grants子命令，不提供虚构步骤。启动参数为`serve-oauth --port <固定非零端口> --tls-cert <证书路径> --tls-key <PKCS8私钥路径> --no-start-daemon`，再按原request show/approve显式审批。此处只是已核对的入口，不代表已启动监听或取得人工结果。

真实Host旧Component单项1/0并未复现历史resource_limit；原失败仍未定位。完整传输/订阅观测、正式默认分发及其余M8缺口见现有ExecPlan，不用这份准备记录宣布完成。未推送，未重复全量/发布构建/三平台。

## 2026-11-17 安装操作文案更正

当前网址输入后的主操作为“继续”，用户流程为“输入扩展地址→继续→选择版本→安装扩展”；获取清单是Core内部工作，不再作为用户动作名称。主按钮直接网址安装、右侧菜单本地安装、原条款/必需权限/取消规则保持，没有增加步骤。

窗口已更新：GUI85304通过5条实际管道连接原Core47824，原data/profile沿用，旧GUI文件不覆盖。关闭自有启动保活后仍可见/响应；五组公开状态及安装/授权完整行摘要保持。只关闭归属明确且0活动任务的自有GUI，没有重启Core、访问真实项目、操作Unity或恢复桌面控制。

相关来源模块单轮8/0，三语言/Material/TypeScript检查、前端build及仅GUI debug构建通过；纯状态检查只用metadata/diff。原36/0及另1/0继续按原输入保留，不合并轮次，不重跑Rust/完整Chromium/release/三平台。记录在target/m8-source-continue-20261117和对应三个build/browser日志；真实程序来源见原review/candidate.json。未推送/无本批远端验证，普通受信任HTTPS源原生人工安装仍待查看。下方“读取清单”等按钮名均是历史候选措辞。

## 2026-11-17 安装入口与弹窗窄修

当前GUI `953ce174614b382762d0c0c0551d911efbcdb5fe`，Core仍用 `3d6c2a1b80a124e1c16794b4a303ac0d534791c7`，不改RPC/State42/权限或安装校验。

实际窗口已更新：GUI91416可见/响应，5条管道连接原Core47824；沿用本页下方State42的data/profile。GUI位于 `C:/Users/M2922/MyProjects/Rust/ALCOMD/target/m8-remote-source-20261117/review/gui-953ce174/alcomd-gui.exe`。原入口Open-Review.ps1复用更新后的candidate.json，各程序分别记录真实来源与摘要，已有实例时拒绝，不自动连接其他Core。

查看两项即可：

1. “扩展”页直接点“安装扩展”，应直接出现网址框，没有来源分段。底部只有“取消／读取清单”；读取后才出现版本与“安装扩展”。不会再有整行读取按钮和一个尚不能用的安装按钮。
2. 点主按钮右侧下箭头→“本地文件”，应进入本地包选择。取消后焦点回到下箭头，再点主按钮仍是空白的网址输入。原签名样例包路径沿用下方，不要求重新验收其他页面。

相关四模块最终独立单轮36/0，含新入口、键盘菜单与焦点、Enter读取、读前不安装、取消迟到结果、三语言错误、原本地路径/安装同意与权限。另通用弹窗具名1/0；首次行号315没有匹配该用例，后用316实际执行。修复前布局回归0/1，改入口后首轮30/1的Enter缺口均保留；不得拼接成完整浏览器套件通过。前端check/build、仅GUI debug构建、locked metadata/diff通过，Core等复用原准确证据，没有全量Rust/Chromium/release或CI重跑。

截图 `target/m8-source-dialog-20261117/before.png`、`after.png`、`preview.png` 为完整窗口的浏览器夹具对比，非原生截图；未恢复桌面控制。938份非docs输入摘要 `bf130488448aa500db6af3339c90862a110d387710affbb1f11feec3bc151feb` 和本轮原始日志在同目录及target/m8-source-dialog-*.log。

更新只关闭本轮自己的旧GUI，在确认0活动任务、精确Core/GUI归属后用有界只读连接维持原Core，未重启Core。数据/profile保持，前后五组公开状态及安装/授权完整行摘要一致（本轮均0/0），旧GUI文件未覆盖，独立候选目录避开Windows文件锁。原环境/Unity未操作。普通受信任HTTPS源的原生成功在线安装仍待人工查看，原证书/代理/远端及resource_limit未覆盖/风险继续保留；未推送，不宣布M8完成。下方为历史候选与完整样例说明。

## 2026-11-17 单扩展源在线安装候选

代码 `3d6c2a1b80a124e1c16794b4a303ac0d534791c7`，只在本地。一个HTTPS地址对应一个扩展及多个版本，从直链下载现有签名 `.alcomdextension`，没有升级、自动更新或扩展市场。GUI保留本地入口，在线准备后继续原条款、必需权限和安装任务流程。

**当前实际窗口：GUI95592可见/响应，5条管道指向候选Core47824，启动助手退出后已再次核对。** 两者及CLI/MCP/Host均为匹配debug程序，独立State42 data/profile位于 `C:/Users/M2922/MyProjects/Rust/ALCOMD/target/m8-remote-source-20261117/review`；自动发现关闭，项目和备份路径只在该目录。开始查看时0安装/0授权/0活动任务；这只是窗口初始状态，不冒称已完成原生在线安装。

真实存在且经HTTPS/Core安装验证的签名测试包，可直接复制完整路径：

```text
C:/Users/M2922/MyProjects/Rust/ALCOMD/target/m8-remote-source-20261117/review/source-demo.alcomdextension
```

SHA256 `7efc24ce73236421f413fbb7067a67e4a0d473fd3ea02fa1f9b5ce6f0b6731af`。包为dev.alcomd.tools-fixture / 1.0.0，公开测试签名，含“仅使用可丢弃测试数据”条款及必需projects.read；仅用于本隔离环境，不要求安装到原环境。

最短查看步骤：

1. 侧栏“扩展”→“安装扩展”：本地文件可直接粘贴上方路径；应先显示作者条款，再显示必需权限，同意后才提交安装。无需发布者信任确认。
2. 同一弹窗切“在线来源”：输入你要安装的普通受信任HTTPS清单地址→“读取清单”→选版本→“安装扩展”。应显示名称、版本/API，默认最高受支持正式版；条款/权限仍取自下载后的签名包。清单变化要求重读，不自动换版本。
3. 准备时取消或Esc、重新打开，旧结果不得带回或自动安装；失败保留可重试反馈，长文本不能挤坏操作区。不要求重验既有M7页面。

**原生在线成功安装仍待人工查看，当前没有可公开分发的在线测试源。** 真正HTTPS传输/下载/安装已由隔离Core用例验证，浏览器夹具覆盖交互；一次性HTTPS测试服务已关闭。测试证书只注入私有测试进程，未写系统信任，普通候选不会信任该证书。不要把已停止的localhost测试地址当可用下载地址，也不把本地文件安装当在线桌面成功。源文件合同及示例见 `specs/extensions/source-v1.md`，不能将示例example.invalid地址当可用服务。

如窗口已关闭，可手动运行下方入口；已实际执行GUI分支并核对管道，已有实例时它拒绝，不自动停止或连接其他Core：

```powershell
& 'C:/Users/M2922/MyProjects/Rust/ALCOMD/target/m8-remote-source-20261117/review/Open-Review.ps1'
```

本地证据均绑定上述代码及937份输入摘要 `221deb5697ab1c2b8bf3d9ab4a9a700cc8310b1cdbf2b32c78a54017b3794eef`：新HTTPS/Core最终1/0、原本地生命周期1/0、解析5/0、Schema1/0、M6合同5/0、State42最终3/0、重开6/0、包/签名9/0、缓存10/0与3 ignored、URL1/0；两浏览器模块独立单轮26/0，初轮新来源6/0单独保留。受影响严格Clippy、前端check/build、五程序debug及fmt/metadata/xtask/diff通过，无完整Workspace/Chromium/release或三平台重跑。State42首轮1/2及具体修复、开发检查失败保留于ExecPlan，不改写失败。既有外部代理/认证夹具三项未执行，未宣称本轮真实代理场景全部覆盖。

记录位于 `target/m8-remote-source-20261117` 及 `target/m8-source-*.log`；程序摘要在review/candidate.json，最终真实测试在source-real-core-final.log/local-lifecycle-final.log，窗口证据在review/records/window-connection.json。每轮占端点前确认旧自有窗口0任务和精确归属，结束后先恢复旧程序/data/profile、五组公开状态一致。最后才切至新的可丢弃环境，旧State41数据/授权/程序保留，没有迁移原运行数据。原四组data/profile集合及摘要36/346/10/303份保持，原开发环境原本关闭并保持，Unity未干扰，不恢复桌面控制，未拍摄或伪造原生截图。

未推送、无这批代码的Hosted CI/CodeQL。717d3e0的CI失败和CodeQL成功仍为旧轮次；旧resource_limit根因未定风险、普通浏览器/实际客户端OAuth、正式默认分发及其他M8未覆盖项继续保留，不由本片宣称通过。M8仍进行中；下方e58窗口和更早PID均为历史检查点。

## 2026-11-17 权限三分类更新

当前GUI代码`e58c0d58bdc259a70482c7f8bc22fc1a0c7b5a38`，Core程序继续使用`1c47a1793e92394bcc35fcfb01754fe6167447d6`；本轮只改呈现，没有RPC/State41/授权语义变化。管理权限按“必需权限”“可选权限”“其他权限”显示；其他指扩展未声明的可授权项。三组均保留，空组“无”，每项仍一个开关，不再逐行重复类型标签。

权限相关浏览器独立单轮19/0、零重试，覆盖类别归属与空必需组、12开关完整性、未声明项授权、窄窗/主题/键盘、错误重试与读回不位移；前端check/build及嵌入资源debug GUI构建通过。Core/RPC/迁移测试复用1c47a17的准确证据，不为此重复完整Rust、Chromium、release或CI。

**窗口已更新并保留：GUI84624可见/响应，5条管道连接隔离Core83680。** GUI位于`C:\Users\M2922\MyProjects\Rust\ALCOMD\target\m8-install-consent-20261117\review\gui-e58c0d5\alcomd-gui.exe`，data/profile及包沿用本页既有路径；candidate.json分别记录GUI与Core来源。打开侧栏“扩展”→现有样例→“管理权限”即可查看三类，无需重新安装或授权。

更新前后均为1扩展、1授权记录，原版本、包身份、启用意图、grant revision及完整授权行摘要均为`dc8c96819b791324b9a4b7159c60bef03f7fd16ef961e60b10bd959b2d63236f`，没有重置。准备时先核对0活动任务；旧GUI停止后文件仍被Windows占用，覆盖失败且旧文件摘要未变。60秒只读保活结束后原隔离Core87756正常空闲退出，随后改用独立候选程序目录，以同一原隔离Core程序、data和profile恢复。不得将此写成Core进程始终未重启。原开发环境与Unity未操作，未恢复桌面输入控制。

最新构建/浏览器及前后授权证据在`target/m8-permission-groups-20261117/`。未推送、尚无新代码远端验证；下列零安装/尚未Apply等措辞是最初交付检查点，之后所有者已实际操作安装，当前不再适用。M8仍进行中，旧资源限额风险继续保留。

## 2026-11-17 首次权限目录与安装同意候选（历史检查点）

代码`1c47a1793e92394bcc35fcfb01754fe6167447d6`，未推送，尚无本轮Hosted/CodeQL结果。窗口已实际打开并保留：GUI90176、Core87756，主窗口可见/响应，5条GUI管道均连接此Core；60秒只读保活已退出后再次核对仍正常。全程未恢复桌面输入控制或伪造原生截图。旧窗口不复用：原本轮GUI88248/Core83476已经在保存五组公开状态、确认0活动任务及精确程序/数据归属后关闭，旧State40数据保留。

程序、data和独立WebView profile均在`C:\Users\M2922\MyProjects\Rust\ALCOMD\target\m8-install-consent-20261117\review\`。五个匹配debug程序构建通过；GUI使用tauri/custom-protocol嵌入已验证前端，不依赖开发服务器。本轮实际执行了该目录的Open-Review.ps1完整启动分支并通过Check-Gui-Connection.ps1被动核对，不沿用旧封装“仅语法检查”的结论。入口会拒绝已有Core/GUI等运行进程，窗口仍开着时不要重复运行。

可复制样例完整路径：

```text
C:\Users\M2922\MyProjects\Rust\ALCOMD\target\m8-install-consent-20261117\review\management-consent.alcomdextension
```

样例`dev.alcomd.fixture`版本1.0.1，复用已批准参考WASM，组件SHA256仍为`e6d80999cb6da6984953317d292c29b1c115e46127a4e1a9ba7b56acc1dad46c`。使用仓库公开测试种子签名，真实包校验和当前Core安装预览均通过；不使用生产签名材料。ZIP文件SHA256 `034d82adcba3f6597a75ee5e28a69dfcecc5c7cc4dd74c1b1ded104080ac643e`；content-tree摘要`b5db0df7c207e269f4933b1268b74b13a58d5f70dffdf4bcca169af71b66f743`，二者含义不同。包含纯文本使用条款，必需mcp.clients.read，可选mcp.clients.manage/mcp.connections.read/mcp.requests.read。

当前是全新隔离State41，0扩展/0授权/0Operation、0项目/0MCP客户端/0OAuth注册；仅生成了1份未Apply的冻结安装Plan，用于证明包与当前合同一致。没有提前替用户同意、安装或开启权限。

本次只看这三项：

1. **侧栏下方“扩展”→“安装扩展”**，选择上面的包并开始安装。先见作者条款，点“同意并继续”；再见必需“读取MCP客户端”的作用，点“同意并安装”。拒绝任一步应返回输入并明确未完成，列表不增加扩展；重试需重新确认。
2. 安装任务成功后进入该扩展的**管理权限**：12项分“扩展已声明”（1必需＋3可选）和“扩展未声明”（8项）。初始只开启必需读取项；两组均能开关，没有逐项目选择。撤销必需项后保留安装但不能使用，恢复需明确再开启。
3. 按现有流程启用并**打开Portable UI**，确认在同一窗口使用；可按需开管理/活动读取项。未声明的权限开关不代表包凭空实现对应功能，不要求验收MCP/Discord正式业务。

验证记录：真实Core新闭环1/0及原生命周期1/0（records以外两份real-core日志位于该review上级）；Core权限/声明读回1/0、对应RPC合同各1/0；扩展Store最终16/0（首轮6/7，旧缺权前置和声明上限断言已按获批语义明确调整，并补独立初始授权/撤销重放证据）；State41 3/0、相关State39 4/0、State40 3/0、事件4/0；包/签名18/0。浏览器首轮26/4，四项均停在旧2/3开关数；精确更新完整目录数量后权限模块7/0，最终稳定三模块独立单轮30/0、零重试；不拼接轮次。早期编译检查修正了新增测试的刷新参数/错误类型，Clippy首检发现的unwrap已改为显式错误上下文；最终受影响全部targets严格检查通过；新增Store测试严格检查也通过。前端check/build、匹配debug构建、fmt/metadata/xtask/diff通过，845份输入与程序摘要绑定candidate-inputs.json。

验收整理器还曾把无关GUI链接目录纳入辅助工具搜索路径，以及混用host/runtime serde构建产物；已在忽略目录内去除无关依赖，签名包与Core预览实际成功，生产代码/依赖未因其改变。不将这些准备失败伪装为产品功能通过。

原开发data36/profile346、旧review data10/profile303份集合和摘要全部保持，原环境此前关闭并保持原状，未升级原数据或触碰Unity。证据为上级preservation-and-fresh-state.json及review/records/，只读检查数据库没有替代GUI的Core写入权威。M8人工待验；先前96c5322旧guest资源限额失败根因仍未定位，本轮不是该风险的修复，历史证据继续有效。未新增远端结果、不进入M9。

---

以下是旧检查点，当时“当前窗口”等措辞不代表现在仍运行；本轮状态以上方新候选为准。

## 2026-11-17 旧Core只读观测窗口（已关闭）

加载位置修正：所有者指出扩展页首开仍误用正文内的局部加载条。现已在源码中改为既有PageLoadingState：主内容画布上沿、全宽4px、不占正文、不显示加载标题；保留可访问名称。前端check通过，真实Windows用户Chromium定向用例1/0，覆盖位置、尺寸、加载标题隐藏、完成后移除及标题不位移。首次沙箱执行因浏览器安装路径不存在而未进入用例，随后真实用户执行又被该次残留测试服务占端口阻止；结束本轮测试会话后单项通过，未改测试配置或下载浏览器。当前原生窗口仍为下述2c4f2a1程序，尚未重新构建/部署此视觉修正，不宣称当前窗口已显示新版，也不以此证明扩展运行时加载风险已解决。

代码候选2c4f2a1a9c124296caa5d3e1e6a29f598c3bbc51（观测功能96c5322＋有界失败诊断），未推送、无新增代码远端验证。**窗口已打开并保留，GUI88248可见且响应，5条管道实际连接隔离Core83476；人工尚未签收M8。** 两个有界启动保活助手已自然退出，最终被动核对仍为5条GUI管道；不把保活存在当成GUI连接证据。 全程只有进程/管道被动核对，没有恢复桌面控制、输入自动化或原生截图。

- 程序、全新data和WebView profile均在`C:\Users\M2922\MyProjects\Rust\ALCOMD\target\m8-runtime-observations-20261117\`；实际State40，两个既有可丢弃样例经正式Core登记，包经正式Plan/Apply安装、启用并打开真实Host页面。包位置位于本次data内，未复制旧数据库或改写路径。
- 可复制签名参考包：`C:\Users\M2922\MyProjects\Rust\ALCOMD\target\m8-runtime-observations-20261117\management-runtime-verified.alcomdextension`，129500字节，SHA256 `b921e7847ae4adbe5df6de855e714e68588f53ba332ac91a83a2bb132a154419`。这是已批准参考代码的隔离样例，不是正式默认分发包。
- 准备时扩展`Core activity fixture`已启用，四项可选权限全部未授予，required为空。公开五组状态为ready、2项目、1次成功安装/0活动任务、0 MCP客户端、0 OAuth注册。Operations的nextCursor是续读位置，不代表有活动任务；初次脚本误判已保留，未重复安装。
- 原data36/profile346、旧review data10/profile303份集合/摘要均保持。原环境此前关闭并保持原状，无需替换或启动原程序；Unity未操作。

只查看本轮内容，不重验M7：

1. **扩展 → Core activity fixture → 管理权限**：四项各一个可选开关，默认关闭；先按需开启“读取MCP客户端”，打开同窗口页面应显示空客户端列表。需要体验写入时再开“管理MCP客户端”，可在此隔离数据里创建测试客户端，默认八项业务能力仍全关。
2. 按需开启“读取MCP的Core连接”或“读取MCP的Core调用”后，**打开扩展页面 → 活动 → 刷新活动**：只显示获准组，名称准确为Core连接/调用。当前没有MCP身份连接，0是正常真实结果；两个身份及在途行已在真实Core测试验证，无需人工证明整个权限系统。
3. 撤销读取权限后旧页面应失效；重开按新权限呈现。检查错误/受限反馈和返回客户端、刷新即可，不要求重新验证全部历史流程。

窗口当前已打开，无需再次启动。`Open-Review.ps1`的新封装GUI分支仍只做语法检查；本轮实际成功路径是已验证Core-only启动＋`Complete-Prepared-Review.ps1`只读续接后的GUI分段启动，不把两者混写成完整入口已验证。全部程序/包摘要见该目录candidate.json，公开状态与真实连接见records/。查看结束后先核对0活动任务，再关闭本轮自有窗口；不关闭或恢复归属不明的程序。

保留未覆盖与风险：96c5322一轮真实组合2/2，两个旧guest首开resource_limit的具体原因仍未知；后续分类诊断单项1/0、组合4/0，不能记为已修复/豁免。浏览器原模块6/1的空白页Target crashed保留，精确单项复验1/0，不拼为完整通过。当前迁移3/0、来源合同3/0、其他定向和严格检查见ExecPlan。所有完整MCP协议请求/HTTP空闲连接/队列/订阅仍未覆盖；本片只观测Core。真实外部OAuth客户端人工接入、正式默认分发及M9业务继续留既有阶段，M8仍进行中。

## 历史准备记录（下列State37与717窗口均已关闭）


> 2026-11-17更新：下文485eddb/State37窗口是历史记录，现已关闭，勿使用旧入口作为当前验收。已准备独立717d3e0程序组和数据副本；五组公开读取、2样例、0活动任务及State39已核对，Core-only启动后正常空闲退出。GUI程序已debug构建但此新窗口分支尚未执行，不宣称已打开或体验通过。新入口：`C:\Users\M2922\MyProjects\Rust\ALCOMD\target\m8-push-717d3e0-20261117\Open-Review.ps1`。后续只读核对发现复制登记仍包含旧review的绝对包位置，故717新入口已禁止GUI启动，仅保留Core-only只读证据；将用全新隔离数据和正式Core登记/安装重建可查看候选，不直接改数据库路径。新版Core不要求发布者额外确认；原签名校验仍有效。获批State40观测正在接线，以上717d3e0程序不含新观测功能。

更新于2026-11-16。本页验收程序对应代码`485eddb2548f122037585667b19c383b729d48e6`，**新版真实GUI已连接新版隔离Core并保留窗口，等待人工查看，未签收M8**。未推送，无此代码远端验证；36a782c的CI/CodeQL只是此前基线。此前5884d5e的按钮/逐项目选择及三项声明已被本轮整改替代，历史证据保留在[M8 ExecPlan](../exec-plans/M8-mcp-protocol-and-management.md)。不恢复桌面控制。本地源码已推进至`465f82386ba530b35cf883c7fe1370cea19cb6c6`，包含严格读取和内部逐项目模型删除；本轮未重建部署下列验收程序，不能把源码定向通过算作该窗口或远端通过。

## 当前可安装样例

```text
C:\Users\M2922\MyProjects\Rust\ALCOMD\target\m8-zero-grant-20261115\management-capabilities.alcomdextension
```

文件真实存在，119860字节；SHA256 `8980614f33694a4fadd11faf05d2a106d848b48ee8eed8cc651988aaaa10b9da`。测试身份`dev.alcomd.fixture`、MCP management fixture、1.0.0、AGPL-3.0-only；测试发布者ALCOMD Test，指纹`ed25519-sha256:10ba682c8ad13513971e8b56881aab8bd702bb807796eca81932c735a94d6e6d`。不是正式产品ID或发行签名。

- required=[]，optional=[mcp.clients.manage,mcp.clients.read]，不再申请自身用不到的项目读取。
- 复用原Host接口及Portable UI v1，Component SHA256 `d43bd812027db11da1670755874c3a87c523dfd41acea759ff88cf44ff41a496`，12项provenance与当前guest源码逐项一致。
- review中旧样例已通过正常retain_data卸载、发布者明确批准、签名校验和安装Plan/Apply更换，两个Operation均成功；当前已启用、grants为空、0活动任务。未绕过信任、签名或权限检查。
- **当前窗口已安装，无需再装**。新建空隔离环境时才选择此路径，核对测试身份/发布者，明确批准发布者，创建安装计划、确认并等待任务成功，再直接启用。旧management-zero-grant及更早包只留历史，不再用于此次查看。

## 只需查看这些操作

1. **扩展 → MCP management fixture → 管理权限**：应只有“读取MCP客户端”和“管理MCP客户端”两项开关。没有项目下拉列表或逐项目授权。当前均关闭；这是本扩展只申请两项权限，不是整个系统只有两种权限。
2. **打开扩展页面**：零授权也能启用、打开同窗口受限页面；没有客户端数据或创建控件。返回管理权限，开启读取后重开，应显示只读客户端列表；再按需开启管理，才出现创建/变更控件。
3. **同窗口扩展页 → 创建一个测试客户端 → 编辑权限**：客户端默认零授权，八项能力各一个开关；切换编辑草稿，审阅实际增减并确认后保存。项目读取覆盖当前及以后登记项目；包管理也覆盖全部项目，真实写入仍需原Plan、明确确认和Apply。这里不选择项目，也不展示或签发秘密。
4. **撤销与重开**：关闭扩展读取开关后旧页面应失效，重开进入受限页；不能仍显示为已授权或误报写成功。窄窗/键盘操作只需查看本次开关，不重复验收M7各页面。

新建/授权仅用于上述可丢弃数据，不操作用户实际工程。系统仍只允许Core支持且扩展清单申请的权限；required/optional是扩展的申请，不等于已授予。其他扩展真正声明必需权限但未获授权时，原启用拒绝合同保持。

## 历史State37环境及启动入口（已关闭）

- 程序组：`target/m8-review-36a782c/capabilities-37/core/`与`capabilities-37/gui/`，GUI无同目录Core。
- 明确数据：`target/m8-review-36a782c/data/`，实际State37；此前State36备份`backup-before-state37/`保留。原开发数据未升级。
- WebView profile：同review的`webview-profile/`；两个既有可丢弃项目保持。M8-allowed-project/M8-other-project是历史样例名称，当前不再代表两种授权范围。
- 当前Core49940、GUI48912，窗口可见且响应，5条真实GUI管道指向该Core。有界启动助手退出后仍有实际GUI连接；无截图或输入自动化。
- 五程序与包摘要在review/candidate.json；连接在records/gui-connection.json，保全在`target/m8-zero-grant-20261115/capability-final-preservation.json`。

窗口已打开，请勿重复启动。以后完全关闭本轮Core/GUI/Host/MCP后可用：

```powershell
& 'C:\Users\M2922\MyProjects\Rust\ALCOMD\target\m8-oauth-audit-20261115\Open-Review.ps1'
```

更新后的完整入口本轮已实际成功执行，核对当前候选/五程序/启动助手摘要，明确指定隔离data/profile。发现已有运行实例就拒绝，不自动暂停、强杀、连接原环境或启动默认Core。只读有界连接只覆盖WebView启动，不改变Core空闲合同。若出现端点冲突，先核对归属与任务状态。

原环境本来关闭并保持关闭，原36份data/346份profile摘要一致、Unity未干扰。没有替换原程序或迁移原运行数据。隔离包升级第一次误传不存在的包路径，Core正确拒绝；核对已接受卸载终态后继续真实包安装，未重复删除。启动期Core退出、脚本检查失败及修正细节均保留在ExecPlan，不把失败记录抹掉。

## 历史证据与当时未覆盖项

浏览器相关最终单轮15/0，guest5/0、doc-test1/0、Host ABI1/0、两项真实Core/Host各1/0、真实双客户端身份复验1/0、Store关联13/0与14/0、State37 4/0、CLI OAuth2/0、合同5/0＋9/0＋2/0。真实六工具首轮4/2，两个旧格式断言修正后各定向1/0；没有重跑整模块，不拼成6/0。早先浏览器12/2（焦点环溢出）及5884d5e的6/8等原始轮次均保留。协议范围样例另以仅测试提交768022a补齐，具名1/0，运行程序输入未变。前端/匹配五程序debug构建、受影响严格Clippy、fmt/metadata/xtask/diff通过，未重跑完整Workspace/Chromium/release/三平台。

截图`apps/alcomd-gui/target/m8-zero-grant/permission-switches-{light,dark}.png`及error两图是浏览器夹具，**不是原生桌面截图**。真实窗口只读核对可见/响应/连接，体验留给用户确认。

普通浏览器和目标MCP客户端OAuth接入仍未人工验证，需要用户受信任localhost证书/PKCS8私钥；本轮没有安装系统信任、开新监听或把公开测试私钥用于日常服务。实时连接/请求/订阅、正式默认分发、会话超时/外部断线及正式MCP/Discord业务等原有未覆盖项继续保留，不能由这份开关查看宣称通过或整个M8完成。

查看结束后先确认隔离Operation全终态，再关闭本轮自有GUI/Core/Host。原环境未暂停，不需要启动原程序；如后续实际暂停过，须按既有授权恢复并核对公开状态，不能只报告守候进程。
