# M8 管理扩展人工查看准备

## 2026-11-17 权限目录与安装同意候选

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
