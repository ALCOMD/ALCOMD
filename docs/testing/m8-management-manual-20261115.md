# M8 管理扩展人工查看准备

2026-11-15，当前候选`36a782c7b78584a46e242b91270fb464bfd9fa29`。匹配程序、样例及隔离Core启动已准备并验证；**GUI已实际启动并连接指定隔离Core，窗口保留等待人工查看；尚未签收**。不自动恢复桌面控制。

## 已核实的开发包

可复制路径：

```text
C:\Users\M2922\MyProjects\Rust\ALCOMD\target\m8-mcp-management\management-development-state35.alcomdextension
```

- 文件真实存在，125919字节，SHA256 `3f41189c80200eec2c0c8ef79676f51896b0518721180f10f55c04587f752ca7`。
- 测试身份`dev.alcomd.fixture`，名称`MCP management fixture`、1.0.0、AGPL-3.0-only；发布者`ALCOMD Test`，指纹`ed25519-sha256:10ba682c8ad13513971e8b56881aab8bd702bb807796eca81932c735a94d6e6d`。它不是正式产品扩展ID或正式发行签名。
- 必需权限`mcp.clients.read`，可选`mcp.clients.manage`及`projects.read`；必需Host接口`alcomd:mcp-management/clients@1.0.0`，Portable UI v1。
- 包内Component须等于当前冻结夹具`crates/alcomd-testing/fixtures/m8/mcp-management-v1.wasm`，SHA256 `ce56c941109c0a034ff0191baedd03c32b814aa1e40915222c6da00004350b7e`；provenance逐文件核对当前guest源码、WIT和构建锁。核对记录位于`target/m8-oauth-audit-20261115/manual-package-audit.json`。
- 正常验签、发布者确认、未授权启用拒绝及真实Core/Host/Component操作已有78920f3同SHA三平台证据。本轮只核对包字节和输入，**没有重新运行安装或声称重新做过密码学验签**。不得手工改包或绕过发布者/权限确认。

## 隔离查看入口

审计时只读Windows进程检查Core/GUI/Host/MCP均0，本轮没有暂停或恢复进程。审计当时`target/debug`、GUI的独立target/debug均没有当前程序，`target/m8-check/debug`只有先前MCP程序；找到的M7备份/旧构建不能作为State35/M8匹配Core/Host候选。以前真实管理用例会在结束时停用、卸载并清理可丢弃数据，不能据其成功声称仍有持久可查看环境。

随后6bf3493及0cc9b28已补齐Core/MCP/CLI及真实OAuth/游标验证，当前36a782c另补齐Host和嵌入前端的debug GUI。使用独立`target/m8-check`、单编译任务，Host构建成功，Tauri `build --debug --no-bundle`成功且前端check/build通过，没有运行完整Workspace/Chromium/release。444份输入摘要仍为`1b2a4e32d6545093cb1233c0b4547164f19fb641b5f75ca6b927b3799d635abf`；运行时代码自0cc9b28未变，后续只改测试/夹具/文档。五个实际程序的路径与SHA256绑定在`target/m8-review-36a782c/candidate.json`，不声称不同构建的文件逐字节相同。

程序组位于`target/m8-review-36a782c/`：Core/CLI/MCP/Host在`core/`，GUI单独在`gui/`且没有同目录alcomd.exe，避免隔离Core失联时GUI默认自启动访问原数据。`data/`、`webview-profile/`及两个`samples/`项目均为本轮可丢弃数据，不使用原M7启动入口。两个样例`M8-allowed-project`和`M8-other-project`已通过正式CLI登记，清单读取返回2项目、0活动任务、0客户端、0 OAuth登记。重开后五项公开结果逐项一致，两个ProjectId/revision保持；准备阶段随后再次停止自有Core，runtime残留0；本次查看已按下述入口重新启动并保持打开。原Core/GUI原本关闭并保持关闭，原36份data/346份profile摘要保持，未操作Unity（本次准备前后均0进程）。证据为`records/{preparation-result,reopen-result,reopen-preservation}.json`及两轮公开读取，不把隔离读取冒称原环境五组状态核对。

当前隔离窗口已打开，无需重复启动。以后关闭后，可在PowerShell执行以下命令打开本轮隔离环境：

```powershell
& 'C:\Users\M2922\MyProjects\Rust\ALCOMD\target\m8-oauth-audit-20261115\Open-Review.ps1'
```

该入口核对五程序摘要、当前运行进程及隔离路径，再启动指定数据目录的Core；随后GUI使用独立WebView2 profile。`-CoreOnly`分支已真实启动并通过公开查询，准备结束后核对0任务再停止自有Core；**GUI启动分支现已按所有者明确授权实际成功执行**。只读核对确认可见窗口ALCOMD3响应正常，GUI PID42264的5条管道连接均以隔离Core PID45200为服务端；Core命令行指定本轮data，WebView2使用本轮独立profile。证据为`records/gui-connection.json`，原36份data/346份profile摘要保持，见`records/gui-start-preservation.json`。没有恢复桌面控制、发送界面输入或拍摄截图，人工体验仍待用户查看。若检测到已有Core/GUI/Host/MCP则拒绝启动，不自动暂停、强杀或连接原环境。若需要处理新的端点冲突，应先核对进程归属及无活动任务，再沿原有获批流程处理。

## 手动打开后的最短操作

1. **扩展 → 安装扩展**：选择上面的完整包路径，核对测试发布者与指纹，明确勾选为此扩展批准发布者。创建安装计划后应看到上列申请权限，确认安装并等待真实任务成功。
2. **扩展 → MCP management fixture → 权限**：先允许必需的“MCP客户端读取”全局范围并启用；打开扩展页面应只读显示，不出现可写控件。再允许“MCP客户端管理”全局范围，以及`M8-allowed-project`的“项目读取”，刷新该页；暂不授权`M8-other-project`。
3. **同窗口扩展页**：新建一个测试客户端，初始零授权；为它增加该隔离项目的读取权限。可选项目应只在扩展当前获准范围内；修改后刷新仍为同一客户端和授权。撤销后应显示已撤销且不可继续修改。不要在这里查找或展示token。
4. **同窗口切换/关闭/重开**：切到普通页面后返回，再关闭和重新打开扩展页，核对当前状态。启用/停用/卸载仍在扩展上下文操作，失败保留可理解的错误；没有实际制造的断线/超时不计通过。

权限强制与双客户端隔离继续引用真实Core/Store/Host证据，不要求人工重复证明整个安全系统。没有实时连接/请求/订阅API，页面不宣称客户端“在线”；OAuth浏览器授权、正式默认分发、表单/会话超时/外部断线和MCP/Discord正式业务不因本清单通过而算完成。

普通浏览器OAuth人工接入另需用户提供受信任的localhost证书与PKCS8私钥，使用本组`core/alcomd-mcp.exe serve-oauth --no-start-daemon --port <已登记端口> --tls-cert <DER证书> --tls-key <PKCS8私钥>`。本轮没有启动监听、安装系统信任或把公开测试私钥用于日常服务；实际浏览器/目标MCP客户端接入仍未执行，不能从SDK自动化通过推定人工接入通过。

查看结束后确认隔离Operation全终态，再关闭本轮自己的GUI/Core/Host。若未暂停原环境则保持原状态；若实际暂停过，恢复原程序/数据并核对五组公开状态、设置及无活动任务，不能仅报告恢复进程守候。
