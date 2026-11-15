# M8 管理扩展人工查看准备

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

## 当前环境及启动入口

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

## 实际证据和未覆盖项

浏览器相关最终单轮15/0，guest5/0、doc-test1/0、Host ABI1/0、两项真实Core/Host各1/0、真实双客户端身份复验1/0、Store关联13/0与14/0、State37 4/0、CLI OAuth2/0、合同5/0＋9/0＋2/0。真实六工具首轮4/2，两个旧格式断言修正后各定向1/0；没有重跑整模块，不拼成6/0。早先浏览器12/2（焦点环溢出）及5884d5e的6/8等原始轮次均保留。协议范围样例另以仅测试提交768022a补齐，具名1/0，运行程序输入未变。前端/匹配五程序debug构建、受影响严格Clippy、fmt/metadata/xtask/diff通过，未重跑完整Workspace/Chromium/release/三平台。

截图`apps/alcomd-gui/target/m8-zero-grant/permission-switches-{light,dark}.png`及error两图是浏览器夹具，**不是原生桌面截图**。真实窗口只读核对可见/响应/连接，体验留给用户确认。

普通浏览器和目标MCP客户端OAuth接入仍未人工验证，需要用户受信任localhost证书/PKCS8私钥；本轮没有安装系统信任、开新监听或把公开测试私钥用于日常服务。实时连接/请求/订阅、正式默认分发、会话超时/外部断线及正式MCP/Discord业务等原有未覆盖项继续保留，不能由这份开关查看宣称通过或整个M8完成。

查看结束后先确认隔离Operation全终态，再关闭本轮自有GUI/Core/Host。原环境未暂停，不需要启动原程序；如后续实际暂停过，须按既有授权恢复并核对公开状态，不能只报告守候进程。
