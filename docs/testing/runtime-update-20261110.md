# 2026-11-10 Core与GUI运行更新

所有者在本对话明确要求“更新”。范围为把已验证的工作区及此前提交的软件包开关构建并运行，保持原数据、设置和WebView配置；不包含提交、推送、发布/tag或真实项目包操作。仓库仍在main，HEAD为b55f9118加未提交工作区。

## 构建与候选

`target/m7-template-bases-20261109/final-inputs.json`的27份源码/测试/合同摘要及预取记录的6份摘要均与当前输入一致，复用此前153项Rust/32项模板浏览器回归、130项预取回归和软件包开关的已记录验证，不重复累加相同用例。本轮重新执行全工作区`npm run build`，随后独立输出Core及启用`tauri/custom-protocol`的嵌入式GUI。命令、日志与产物在`target/m7-runtime-update-20261110/`。构建通过，保留既有bundle大小提示和显式rustc输出路径的两条提示。本轮未重跑所有测试或Hosted CI。

仅替换既有`target/debug/alcomd.exe`与`target/debug/alcomd-gui.exe`。本轮使用旧exe路径，未创建/切换worktree或修改功能源码。

## 备份与状态保持

开始时v4 Core/GUI均已关闭，只有原v3在运行，原WebView profile无占用。离线备份旧程序、数据目录19份文件（含数据库及WAL）、profile319份文件；复制后逐文件SHA-256及原树再次校验一致。备份在`target/m7-runtime-update-20261110/backup/`，摘要为`backup-manifest.json`。随后用旧Core读取升级前权威状态，无活动任务。

沿用的数据路径为`target/m7-package-navigation-100ms-20261108/native-reopen/data`，profile为同级`webview-profile`。切换前再次确认无活动任务，按精确进程路径/PID/启动时间停止本次启动的旧Core；新Core、GUI与旧二进制均按摘要核对。GUI继续使用此前的环境设置，禁用本轮VCC协议注册，未更改v3安装或系统关联。

- Core PID **38052**，GUI PID **15580**；Core ready、State31、Config13、0活动任务，GUI有可响应的原生窗口。
- Core SHA-256：`DAA184995A4CDE48952B762B24FD17E2E27FD321CAB8F0D66C8FC88FBD5F4C01`。
- GUI SHA-256：`389849156E12646823AC2DEC579BF331FD7635DEAE4477EDC49164BB33EACFFB`。
- 切换前最后快照与新版逐项比较：2个项目记录、30个仓库记录、原4个模板记录完全相同。新增锁定Blank模板后共5个，Blank无固定Editor版本。
- Config12→13仅升级schema并增加空`hidden_repository_packages`。持久TOML移除这两项差异后与原配置相同；只读settings DTO原值与revision **5**保持，新增偏好为空。GUI打开后配置文件摘要保持。旧Core不支持Config13，回退必须带升级前数据备份。
- GUI打开前profile319份文件仍与备份相同，实际WebView子进程命令行指向原profile。GUI正常启动后的缓存变化不描述为逐字节不变。v3及Unity进程身份/启动时间保持。

完整最终结果见`runtime-result.json`、`state-after-gui.json`和`attempt-2/state-verification.json`。

## 首次校验拒绝与恢复

首次升级已进入ready/Config13并保留2项目/30仓库/设置revision5，但严格比较使用的是旧Core刚启动时的最早快照。旧Core在之后正常后台刷新了30个仓库的refreshedAtMs，其中1个catalog revision也更新，故最早快照与新版不同。校验拒绝后，在GUI尚未打开且0活动任务时按预案保留失败数据到`failed-data/`，完整恢复备份程序和数据并启动旧Core。

只读核对确认：第一次的**切换前最终快照**与第一次新版的全部项目、仓库对象逐项完全相等，设置也未变。第二次不放宽对象比较，改以停止旧Core前的最终快照为基线，保留第一次全部日志/快照及失败数据，迁移、完整对象、设置与新增模板验证全部通过。第二次证据单独位于`attempt-2/`。没有反复执行包操作、删除用户项目或修改功能实现来绕过检查。

## 原生窗口验证限制

Computer Use已通过返回的精确exe路径列出新版GUI和单一窗口，随后读取窗口内容时工具拒绝：`Computer Use was not approved to use alcomd-gui`。因此没有原生页面内容、截图或功能点击的验收，也未改用其他自动化绕过授权；仅完成进程/可响应窗口/WebView profile与Core只读RPC验证。GUI日志未输出启动错误，未据此宣称零控制台问题。

本次没有提交/推送，没有改变真实项目包，M7/H7/M11人工签收范围保持。各项旧“未更新”记录只描述其原验证时点。
