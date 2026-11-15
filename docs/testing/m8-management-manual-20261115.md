# M8 管理扩展人工查看准备

2026-11-15，当前本地代码`5884d5e1dc5152995bec20206f1a642d21d7ae9f`。已按所有者反馈修正零授权启动与“管理扩展 → 管理权限”弹窗，**新版GUI已实际启动并连接指定隔离Core，窗口保留等待人工查看；尚未签收**。新代码未推送、没有对应远端验证，36a782c的三平台CI/CodeQL通过只属于此前基线。不恢复桌面控制。

## 已核实的开发包

可复制路径：

```text
C:\Users\M2922\MyProjects\Rust\ALCOMD\target\m8-zero-grant-20261115\management-zero-grant.alcomdextension
```

- 文件真实存在，126423字节，SHA256 `223b35f44ee3b35437da209b4b5c30968305897d487ffa3f4f35c06cfb6c47a6`。
- 测试身份`dev.alcomd.fixture`，名称`MCP management fixture`、1.0.0、AGPL-3.0-only；发布者`ALCOMD Test`，指纹`ed25519-sha256:10ba682c8ad13513971e8b56881aab8bd702bb807796eca81932c735a94d6e6d`。它不是正式产品扩展ID或正式发行签名。
- 没有必需业务权限；可选`mcp.clients.manage`、`mcp.clients.read`及`projects.read`。必需Host接口`alcomd:mcp-management/clients@1.0.0`，Portable UI v1。
- 包内Component等于当前夹具`crates/alcomd-testing/fixtures/m8/mcp-management-v1.wasm`，SHA256 `1ec0fb1ac969e6711db44103d95cba225b8c9b251c1c1a325be4af75c7156ba8`；provenance逐文件核对当前guest源码、WIT和构建锁，合同用例2/0。
- 本轮通过正式Core签名/发布者批准/安装Plan/Apply流程，替换了review中旧包（卸载retain_data），两个实际Operation均成功。当前已启用、实际grants为空，真实Host页面可打开为未授权状态；没有绕过签名或授予权限。记录为`target/m8-zero-grant-20261115/live-package-update.log`。原来125919字节、required-read的旧开发包仍保留作历史证据，**当前不要再用旧包安装**。

当前窗口已装好新包，无需再次安装。先查看零授权下能否打开页面，再进入“管理权限”：每项直接显示用途、已允许/未允许及操作；读取客户端与管理客户端分别允许，项目读取只对明确选择的项目生效。选择项目本身不授权，需要点击“允许所选项目”。

## 隔离查看入口

以下初始准备段落保留36a782c时的真实历史输入与执行记录，不作为新代码验证；当前程序及窗口见本节后半段。

审计时只读Windows进程检查Core/GUI/Host/MCP均0，本轮没有暂停或恢复进程。审计当时`target/debug`、GUI的独立target/debug均没有当前程序，`target/m8-check/debug`只有先前MCP程序；找到的M7备份/旧构建不能作为State35/M8匹配Core/Host候选。以前真实管理用例会在结束时停用、卸载并清理可丢弃数据，不能据其成功声称仍有持久可查看环境。

随后6bf3493及0cc9b28已补齐Core/MCP/CLI及真实OAuth/游标验证，当前36a782c另补齐Host和嵌入前端的debug GUI。使用独立`target/m8-check`、单编译任务，Host构建成功，Tauri `build --debug --no-bundle`成功且前端check/build通过，没有运行完整Workspace/Chromium/release。444份输入摘要仍为`1b2a4e32d6545093cb1233c0b4547164f19fb641b5f75ca6b927b3799d635abf`；运行时代码自0cc9b28未变，后续只改测试/夹具/文档。五个实际程序的路径与SHA256绑定在`target/m8-review-36a782c/candidate.json`，不声称不同构建的文件逐字节相同。

程序组位于`target/m8-review-36a782c/`：Core/CLI/MCP/Host在`core/`，GUI单独在`gui/`且没有同目录alcomd.exe，避免隔离Core失联时GUI默认自启动访问原数据。`data/`、`webview-profile/`及两个`samples/`项目均为本轮可丢弃数据，不使用原M7启动入口。两个样例`M8-allowed-project`和`M8-other-project`已通过正式CLI登记，清单读取返回2项目、0活动任务、0客户端、0 OAuth登记。重开后五项公开结果逐项一致，两个ProjectId/revision保持；准备阶段随后再次停止自有Core，runtime残留0；本次查看已按下述入口重新启动并保持打开。原Core/GUI原本关闭并保持关闭，原36份data/346份profile摘要保持，未操作Unity（本次准备前后均0进程）。证据为`records/{preparation-result,reopen-result,reopen-preservation}.json`及两轮公开读取，不把隔离读取冒称原环境五组状态核对。

当前隔离窗口已打开，无需重复启动。上述隔离准备与旧45200/42264为此前记录；**现在为Core47292/GUI39500**，新GUI摘要`9a2381b194948d6cd76067a22822169754378645f91956f7a7c70b4223efaa8a`，其余四程序字节保持。窗口可见且响应、5条管道均指向该Core，WebView仍使用同一隔离profile；证据`target/m8-zero-grant-20261115/gui-connection-zero-grant.json`。本次只暂停并恢复自有review实例进行两项定向Core回归，原环境本来关闭并保持关闭、原36份data/346份profile和Unity保持。

以后关闭后，可在PowerShell使用以下入口。更新后的候选/hash与有界只读启动连接已同步，**此版入口仅语法检查，实际程序启动和连接已分步验证，未重新端到端执行整份入口**：

```powershell
& 'C:\Users\M2922\MyProjects\Rust\ALCOMD\target\m8-oauth-audit-20261115\Open-Review.ps1'
```

该入口核对五程序与启动助手摘要、当前运行进程及隔离路径，再启动指定数据目录的Core，GUI使用独立WebView2 profile。有界只读连接仅覆盖WebView启动间隙，不启动其他Core或执行写入；不改变Core空闲退出合同。检测到已有Core/GUI/Host/MCP即拒绝，不自动暂停、强杀或连接原环境。若有新的端点冲突，应先核对归属与无活动任务，再沿既有授权处理。原始36a782c入口的CoreOnly/GUI分支曾实际执行成功，当前5884d5e程序已按相同明确参数分步启动，连接证据以上述新记录为准。没有恢复桌面控制、发送界面输入或拍摄原生截图，人工体验仍待用户查看。

## 手动打开后的最短操作

1. **扩展 → MCP management fixture → 打开扩展页面**：当前已启用但没有授权，应能打开同窗口页面，显示“尚未授权读取 MCP 客户端”，没有客户端数据或创建控件。启动不再要求预先授权。
2. **返回扩展详情 → 管理权限**：查看每项用途和未允许状态；允许“读取 MCP 客户端”后重新打开页面，应只读显示且没有创建控件。按需分别允许“管理 MCP 客户端”和`M8-allowed-project`的“读取项目”；暂不允许`M8-other-project`。撤销按钮位于对应权限/项目旁，不用再从通用权限下拉框拼装范围。
3. **同窗口扩展页**：授权管理后可新建测试客户端（客户端初始零授权），为它增加已允许隔离项目的读取权限；可选项目应只来自扩展当前获准范围。修改、刷新、撤销结果应准确；不要在这里查找或展示token。
4. **切换/关闭/重开**：在普通页面与扩展页之间切换，再关闭重开。撤销扩展读取权限后旧会话应失效，重新打开仍可显示未授权页面；启停/卸载保持原管理入口。未实际制造的断线/超时不计通过。

只有新建空隔离环境时才需要先安装：选择上列包路径，核对测试发布者与指纹并明确批准，创建安装计划、确认安装、等待真实任务成功，随后可不授权直接启用。其他真正声明必需权限的扩展继续按原合同处理。

本轮验证：guest3/0、doc-test1/0、Host ABI1/0、合同2/0、两项真实Core用例各1/0；权限相关浏览器最终单轮14/0，之后真实错误码与可读提示三例3/0（与14项重叠）。首轮浏览器6/8及新包清单排序拒绝0/1保留，未拼成完整704项通过。前端check/build、嵌入GUI debug构建、定向严格Clippy、fmt/metadata/xtask/diff通过；新代码远端未验证。浏览器前后截图为`target/m8-mcp-management/permissions-global-scope.png`及`apps/alcomd-gui/target/m8-zero-grant/permissions-{explicit-scopes,narrow-light,narrow-dark,error-light,error-dark}.png`，不是原生桌面截图。

权限强制与双客户端隔离继续引用真实Core/Store/Host证据，不要求人工重复证明整个安全系统。没有实时连接/请求/订阅API，页面不宣称客户端“在线”；OAuth浏览器授权、正式默认分发、表单/会话超时/外部断线和MCP/Discord正式业务不因本清单通过而算完成。

普通浏览器OAuth人工接入另需用户提供受信任的localhost证书与PKCS8私钥，使用本组`core/alcomd-mcp.exe serve-oauth --no-start-daemon --port <已登记端口> --tls-cert <DER证书> --tls-key <PKCS8私钥>`。本轮没有启动监听、安装系统信任或把公开测试私钥用于日常服务；实际浏览器/目标MCP客户端接入仍未执行，不能从SDK自动化通过推定人工接入通过。

查看结束后确认隔离Operation全终态，再关闭本轮自己的GUI/Core/Host。若未暂停原环境则保持原状态；若实际暂停过，恢复原程序/数据并核对五组公开状态、设置及无活动任务，不能仅报告恢复进程守候。
