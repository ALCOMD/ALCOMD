# Windows v3.4.0 Fixture 分阶段采集手册

状态：M-1 尝试已停止，完整采集后移到 M11；本手册只采集证据，不执行 v4 迁移或删除

## M-1 操作记录

2026-08-17 在 Windows 11 25H2 VM 中完成的操作报告：

- 仓库 commit `d60a35b`，工作树干净。
- 冻结安装器大小 `14514948`、SHA-256
  `c35346b32152dfea62b4936db737a16452fcfe64bddc02bd6d1e4b97c7ea5eb2` 匹配。
- 安装前 19 个精确目标均不存在；规范化 inventory 报告摘要为
  `b1221f9c86e877319060266bc26aca86c8635e158ee6dfe3273035c72e609597`。
- 当前用户默认安装无 SmartScreen/UAC，卸载登记、可执行文件产品版本与 GUI 均为 3.4.0；
  安装器退出码未捕获，不得推断为 0。
- 关闭窗口后进程仍存在；应用内显式退出后主进程和子进程正常结束。
- 未创建合成状态。安装后采集器在写出结果前以脱敏通用错误终止，没有产生可审查输出。

以上内容是项目所有者转述的操作报告，不是已提交的原始/脱敏 Fixture。项目所有者决定停止
M-1 采集并后移到 M11；所有实例 `confirmed` 必须继续为 `false`。

## 首个目标

先完成 `win-per-user-default`：Windows x86_64、v3.4.0 当前用户默认安装。不要在同一 VM 状态
继续做全局安装；`win-per-machine-custom` 必须从干净快照另起。

## 绝对边界

- VM 只能使用合成项目、设置、路径、token 和仓库地址，不登录 Discord、VRChat/VCC、Unity
  Hub、真实 MCP 客户端或其他个人服务。
- VM 内 Codex 自己的凭据、配置、日志和环境变量不是迁移 Fixture。不得导出整个用户配置、
  整棵注册表、完整环境变量、浏览器数据或 `%USERPROFILE%`。
- 原始证据只放在 VM 的仓库外目录，例如 `C:\ALCOMD-Fixture-Work\raw`，不得直接 `git add`。
- 采集阶段不得修改 `migrations/v3/artifacts.toml` 的 `confirmed`，不得运行清理、卸载、bridge
  或任何 v4 migrator。
- 每个停止点都先人工检查输出路径和敏感信息，再继续下一阶段。

## 冻结输入

从 `docs/baselines/source-lock.toml` 使用：

```text
文件：ALCOMD3_3.4.0_windows_x86_64_setup.exe
大小：14514948 bytes
SHA-256：c35346b32152dfea62b4936db737a16452fcfe64bddc02bd6d1e4b97c7ea5eb2
Release ID：370982635
Asset ID：515430064
URL：https://github.com/ALCOMD/ALCOMD3/releases/download/v3.4.0/ALCOMD3_3.4.0_windows_x86_64_setup.exe
```

哈希或大小不一致时立即停止。Release 当前不可视为 immutable，不能只相信 tag、文件名或 URL。

## 阶段 0：干净检查点

1. 关闭 VM 中除 Codex 外的个人应用，确认 VM 没有真实项目和服务账号。
2. 创建虚拟机快照 `m1-win-clean-codex-before-v3`。
3. 将包含本手册的 ALCOMD 仓库副本放入 VM；记录 `git rev-parse HEAD`，保持工作区干净。
4. 下载冻结安装器到 `C:\ALCOMD-Fixture-Work\input`，运行：

   ```powershell
   Get-Item -LiteralPath 'C:\ALCOMD-Fixture-Work\input\ALCOMD3_3.4.0_windows_x86_64_setup.exe' |
       Select-Object Name, Length
   Get-FileHash -Algorithm SHA256 -LiteralPath 'C:\ALCOMD-Fixture-Work\input\ALCOMD3_3.4.0_windows_x86_64_setup.exe'
   ```

5. 记录 Windows edition/version/build、架构、PowerShell 与采集仓库 commit。不得记录设备序列号、
   MachineGuid、用户 SID、网络地址、Codex 账号或 token。

### 停止点 A

向项目所有者只报告：VM 平台、仓库 commit、安装器大小/哈希是否精确匹配、原始输出目录。
此时不要运行安装器。

## 阶段 1：安装前最小快照

让 VM 内 Codex 在仓库外生成一次性只读采集脚本。脚本只允许读取并规范化：

- 三个卸载注册表视图中两个精确 AppId：
  `{3CA473A7-9CEA-4EB6-9949-803C3D0B8057}_is1` 与
  `{4C3D0631-AE29-4D20-A231-678D9CF8D6DB}_is1`。
- HKCU/HKLM 对 `.alcomtemplate`、`ALCOMD3 Project Template` 和 `vcc` 的精确 Classes 键。
- `%LOCALAPPDATA%\ALCOMD3`、`%LOCALAPPDATA%\com.cqmhv.alcomd3`、
  `<DOCUMENTS>\ALCOMD3` 及 Start Menu/Desktop 中精确 ALCOMD3 路径是否存在。
- 只针对这些路径的类型、相对名称、大小、SHA-256、ACL 摘要和 symlink/reparse 信息。

不存在也是有效证据。禁止递归导出注册表根、用户主目录或 `%TEMP%`；临时 updater 只在已知
进程产生并记录确切路径时采集。

将规范化结果写到 `raw\before`，生成确定性清单和 SHA-256。截图只留 raw，且必须裁剪/遮盖
用户名、任务栏账号、网络和通知。

## 阶段 2：当前用户默认安装

1. 再创建快照 `m1-win-before-v3-install`。
2. 交互运行已验证的安装器，选择默认当前用户范围和默认路径；不要选择所有用户，不要手工
   改注册表或 PATH。
3. 记录安装器页面选择、是否出现 UAC、最终路径和错误；当前用户默认安装若请求管理员权限，
   保留证据并停止。
4. 首次启动 v3.4.0，确认显示版本。不要启用自动清理或执行任何 v4 更新/bridge。

## 阶段 3：只建立合成状态

允许创建：

- `C:\ALCOMD-Fixture-Data\Projects\SyntheticProject`，只含最小公开 Unity/VPM 合成文件。
- `C:\ALCOMD-Fixture-Data\Backups` 与合成模板/设置。
- 固定测试值，例如 `fixture-token-not-secret`、`https://example.invalid/vpm.json`。

不得让 v3 修改 VM 中 Codex 的真实配置。MCP 外部配置测试使用仓库外的合成副本，包含无关
条目和注释，以便以后证明精确 patch；不要把 `%USERPROFILE%\.codex`、Claude 或 Cursor 的
真实配置交给 v3。

逐项记录 GUI 中实际可达的入口、默认值、保存后行为和错误；GUI 截图仍只进入 raw。

## 阶段 4：安装后快照与脱敏

1. 正常退出 v3，确认无相关进程后复用阶段 1 的同一脚本采集 `raw\after`。
2. 额外记录安装目录相对树、主程序/卸载器版本与哈希、快捷方式目标、卸载登记、协议与文件
   关联、数据根、WebView 数据根、默认项目/备份目录和确切 ACL 摘要。
3. 将路径替换成 `<USER_HOME>`、`<LOCAL_APP_DATA>`、`<DOCUMENTS>`、`<V3_INSTALL>`；合成值
   保持固定。任何疑似 bearer/token/cookie/Authorization、用户名、SID、MachineGuid、私有 URL
   或绝对主目录都使脱敏失败。
4. 从 `manifest.example.toml` 生成 `manifest.toml`，但保持：

   ```toml
   status = "sanitized-draft"
   human_reviewed = false
   artifact_instance_confirmation_allowed = false
   ```

5. 只把最小脱敏结果复制到待审目录，不复制 raw 截图、完整日志、数据库原件或 Codex 配置。

### 停止点 B

将脱敏树的文件清单、总大小、总摘要、敏感扫描结果和 `manifest.toml` 发给项目所有者复核。
在人工批准前：不提交 Fixture、不把任何 artifact 设为 `confirmed = true`、不卸载 v3、不执行
迁移或清理。

## 后续场景

首个 Fixture 获批后，恢复 `m1-win-clean-codex-before-v3`，再以独立快照采集
`win-per-machine-custom`。它需要显式 UAC、Unicode/空格自定义安装路径与第二个合成 Windows
用户；不得在当前用户 Fixture 上原地转换后冒充干净全局安装证据。

## 交给 VM 内 Codex 的首轮任务

在 VM 的 ALCOMD 仓库中发送：

```text
阅读 AGENTS.md、docs/architecture/ALCOMD-V4.md、docs/decisions/accepted.md、
migrations/v3/AGENTS.md、migrations/v3/fixtures/README.md 和
migrations/v3/fixtures/windows-collection.md。只执行“阶段 0”，验证冻结安装器大小和 SHA-256，
记录不含设备/账号/凭据的最小平台信息，然后在停止点 A 停下汇报。不得运行安装器，不得读取
或导出 Codex 凭据/配置，不得修改仓库，不得开始阶段 1 以后工作。
```
