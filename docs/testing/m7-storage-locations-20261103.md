# M7 默认项目与备份保存位置

状态：本地功能候选，H7 最终人工签收仍待所有者。2026-11-03。

所有者明确确认这是严重影响使用的既有核心功能遗漏，批准在 M7 补齐 Config5、公开设置及必要持久定位。
H2–H6 签收与此前 `c273c76` 的技术证据保留；本记录不代表 M7 完成，也不进入 M8。

## 行为与数据边界

- 设置页增加默认项目和备份保存位置，复用目录选择器、串行 CAS、共享错误与重试。
- 新建、模板创建、复制及恢复表单使用当前有效项目目录，允许当次覆盖；迟到读取不替换用户输入。
- 新备份直接写入所选目录；接受任务时冻结目录及文件身份，设置变化不改动已接受任务。
- 历史备份继续使用原定位；新定位不替代 identity/hash 校验。恢复计划保持自身冻结来源，不搬迁旧文件。
- Config1–4 严格迁移为 Config5，保留 revision 和其他设置。State16、RPC v1、权限与生产依赖不变。
- 系统 Documents 仅为默认值来源；双显式目录不依赖它可达。配置读取不创建目录，不执行 shell。

活动合同为 `specs/config/settings-v5.*`、现有 RPC Schema 及 `specs/storage/state-v6.md`、`state-v7.md` 中的当前增量说明。
Windows 不覆盖发布为窄 `MoveFileExW(flags=0)`，Unix 复用 rustix `NOREPLACE`；没有扩大 crate 级 unsafe 许可。
Windows Documents 查询按 [Known Folder 选项](https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/ne-shlobj_core-known_folder_flag) 使用 `DONT_VERIFY` 只取得当前配置位置，存在性与写入条件由实际操作检查。

## 定向验证

| 范围 | 实际结果 |
| --- | --- |
| 协议与设置 Schema | unit 15/0、Schema 35/0；旧版本形状保留，新增字段与严格 PATCH 拒绝向量通过。 |
| application 设置 | 最终 21/0，包含双显式目录绕过不可用默认值、清除失败不提交、CAS/权限/取消与未知发布。 |
| 配置文件模块 | 21/0，目录保存/重开/清除、Config4 迁移及 16 KiB 总上限拒绝。 |
| 平台目录与发布 | 最终 3/0，目录规范化/准备、冲突不覆盖、Linux user-dirs 纯文本解析；后者在 Windows 宿主运行解析测试。 |
| 归档引擎 | 7/0，旧定位、两个新目录、身份替换/冲突/源项目内目的地拒绝及旧恢复计划字节保持。 |
| SQLite 目的地与重放 | 修正后 2/0，跨设置变化/重开冻结，排队及运行中取消均保持既有状态机。 |
| 配置到实际备份/恢复集成 | 1/0，无 IPC；真实 FileConfigStore/application/SQLite/BackupEngine，包含已接受后切换目录、提供者不可用时幂等重放及三份旧/新备份实际恢复内容。 |
| 相关旧合同及候选快照 | Backup Create 4/0、Restore 4/0、候选配置并发 3/0。 |
| GUI | check（1036 条三语言、Material 边界）、保存队列 20 场景；最终集中 Chromium 26/0，相关创建/恢复/复制/路径独立 12/0，320/640px 独立 2/0；均 1 worker、0 retries。 |
| 受影响静态门禁 | 八个受影响 crate 的 all-targets Clippy、fmt check、xtask check 通过。 |
| 真实隔离 RPC | 设置 1/0、备份创建/恢复/kill-restart 8/0，2 个既有 subprocess helper 顶层 ignored，由矩阵调用。 |
| 启动端点所有权 | 开启既有 test-kill-gates feature 后 4/0；包括占用时先拒绝再初始化，以及服务 task 结束后保留 handle 所有权。 |

各轮结果独立保留：首轮 Schema 33/1 的 hello 快照遗漏、浏览器 8/1 的错误 fixture 名称、
新存储回归 1/1 的 queued 取消状态误判、新集成 0/1 的 fixture 未创建 data 目录，以及 Clippy 的测试写法和遗漏字段；
修正后只补对应失败或继续未完成门禁，不回填原轮、不拼成全套通过。启动端点初命令未开 feature，实际 0 项，
不计覆盖，随后开启 feature 的独立 4/0 才作为证据。原 GUI/项目数据未用于测试写入。

本地日志在 `target/m7-storage-locations/`；浏览器最终 26 项在 `target/m7-directory-gui-final/`，
相关回归在 `target/m7-directory-gui-regression/`、`target/m7-directory-responsive/`。没有运行完整 Workspace、完整浏览器套件或新 CI。

## 构建、环境与人工查看

一次 Core/CLI release 构建通过（4m27s），GUI release no-bundle 构建通过（Rust release 4m11s，含前端 check/build）；
依赖和 lockfile 未变，大 chunk 提醒保留。实际 Core SHA-256 `13a92b96e58cf3c505359e09d947a55941edf23c4a06e2b5a6db80b72545aaca`，
GUI `eea46ddb02101874635579a0a6658bc8c76e411c1056557398a3acf46a0d17d5`。
原 GUI 在切换前已关闭；只对已核对身份的原 Core `131912` 执行临时停止。
五组公开快照、旧配置与旧二进制保存在本轮 target 目录，既有 screenshot/output/验收材料保留。
现已实际打开原数据目录、原 WebView profile 的 Core **130768**、GUI **132996**，窗口 **101191372**，可响应。
`environment-comparison.json` 的项目、Editor 登记、Operation、Status、已有 Settings 全部为 true；
`config-migration-comparison.json` 证明实际 TOML 仅 schema **4→5**，revision **166** 与其余字段完全一致。
默认目录为用户 `Documents/ALCOMD/Projects`、`Documents/ALCOMD/Backups`，两个显式覆盖均为 null。
本轮没有对用户 Unity 执行进程操作或真实项目写入；最后只读采样时原 Editor PID 已不在，不推断原因、不自动重启。
桌面控制继续停用；浏览器结果不代替原生目录选择、界面观感或所有者最终签收。
本轮仅 Windows 实跑，Unix 发布原语及系统目录解析的目标平台执行留待后续获批 CI，不把旧三平台通过归于当前代码。
