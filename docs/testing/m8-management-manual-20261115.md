# M8 管理扩展人工查看准备

2026-11-15，源码远端基线`78920f382d1a0c58dece4a51fb2f54d84f48fcf2`；本地93967f5仅状态记录。以下是准备核对与短清单，**尚未形成可操作的原生窗口，不是人工验收通过**。不自动恢复桌面控制。

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

## 当前入口缺口

只读Windows进程检查Core/GUI/Host/MCP均0，本轮没有暂停或恢复进程。`target/debug`、GUI的独立target/debug均没有当前程序，`target/m8-check/debug`只有先前MCP程序；找到的M7备份/旧构建不能作为State35/M8匹配Core/Host候选。以前真实管理用例会在结束时停用、卸载并清理可丢弃数据，不能据其成功声称仍有持久可查看环境。

下次准备窗口仅需补齐**匹配当前源输入的一组独立原生构建**，不需重跑Workspace/704项或三平台。使用同一隔离输出目录放置Core、CLI、Host和嵌入前端的GUI，绑定源码SHA及文件摘要；不能将普通debug GUI的devUrl误当已嵌入产物。复用既有`--data-dir`和`WEBVIEW2_USER_DATA_FOLDER`隔离方式，使用新建workspace/target下的数据、profile及两个可丢弃项目。不要执行指向原M7数据的旧`open-runtime.ps1`来代替隔离入口。

此时不提供会把旧Core接到新GUI、或未完成构建仍继续启动的“一键启动”封装。匹配程序组和实例身份核对完成后，再给出准确启动命令与实际打开结果；新封装如只做语法检查，应继续标为未验证启动。Windows受保护管道每用户唯一：若届时发现原Core/GUI运行，先按已有授权核对无活动任务、保存五组公开状态，再沿既有暂停/恢复流程处理；归属不明或有活动任务即停在该步骤，不强杀、不操作Unity。

## 窗口具备条件后的最短操作

1. **扩展 → 安装扩展**：选择上面的完整包路径，核对测试发布者与指纹，明确勾选为此扩展批准发布者。创建安装计划后应看到上列申请权限，确认安装并等待真实任务成功。
2. **扩展 → MCP management fixture → 权限**：先允许必需的“MCP客户端读取”全局范围并启用；打开扩展页面应只读显示，不出现可写控件。再允许“MCP客户端管理”全局范围，以及一个隔离项目的“项目读取”，刷新该页。
3. **同窗口扩展页**：新建一个测试客户端，初始零授权；为它增加该隔离项目的读取权限。可选项目应只在扩展当前获准范围内；修改后刷新仍为同一客户端和授权。撤销后应显示已撤销且不可继续修改。不要在这里查找或展示token。
4. **同窗口切换/关闭/重开**：切到普通页面后返回，再关闭和重新打开扩展页，核对当前状态。启用/停用/卸载仍在扩展上下文操作，失败保留可理解的错误；没有实际制造的断线/超时不计通过。

权限强制与双客户端隔离继续引用真实Core/Store/Host证据，不要求人工重复证明整个安全系统。没有实时连接/请求/订阅API，页面不宣称客户端“在线”；OAuth浏览器授权、正式默认分发、表单/会话超时/外部断线和MCP/Discord正式业务不因本清单通过而算完成。

查看结束后确认隔离Operation全终态，再关闭本轮自己的GUI/Core/Host。若未暂停原环境则保持原状态；若实际暂停过，恢复原程序/数据并核对五组公开状态、设置及无活动任务，不能仅报告恢复进程守候。
