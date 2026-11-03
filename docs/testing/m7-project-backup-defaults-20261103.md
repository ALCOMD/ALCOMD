# M7 项目与备份默认设置（2026-11-03）

状态：功能及本地验证完成；M7/H7 人工验收状态保持。

## 范围与权威

项目所有者明确批准 A-037 / M7 ExecPlan 最小合同。常规设置分为“项目”和“备份”：

| 设置 | 默认值 | 生效位置 |
| --- | --- | --- |
| 项目默认路径 | 原系统默认或已存覆盖 | 创建表单、Core 自动发现 |
| 自动添加默认目录内项目 | true | Core 启动、相关配置通知及30秒周期 |
| 备份默认位置 | 原系统默认或已存覆盖 | 接受新备份时固定目的地 |
| 默认压缩等级 | fast | 不压缩/快速/最大三档，允许单次覆盖 |
| 备份保留软件包 | true | 映射原 excludeVpmPackages 的反值 |
| 备份保留 Library 缓存 | false | 标准项目根 Library 的内容选择 |

Config8 沿用唯一 Core/application 设置权威、串行 CAS 和16 KiB严格文件边界。
Config1–7有效迁移保留 revision/全部既有值，只补新默认；项目发现字段和启用时的项目路径更新附加现有 projects.manage。
备份创建的省略项由同一配置快照解析，已接受操作和幂等重放不随后续设置改变。
Library=false保留原指纹与历史请求语义，true进入指纹；State17、BackupRecord及Archive v1 manifest形状不变。

## 并发与边界

- 扫描只读枚举直属目录，每轮最多4096项，单任务在途，不递归/跟随链接；登记前重验身份及配置revision，失败不删除已有项目。
- 正常发布复用ProjectCreate锁；启动恢复在调度前同步预留RAII屏障，防止Template/Copy/Restore恢复准备期间被扫描抢先登记。屏障不阻塞ready、列表或设置查询。
- GUI不扫描磁盘或登记项目；自动发现开启时只对可见项目页做3秒单在途只读跟进。共享读取代际使过期结果无法覆盖显式刷新或项目更新。
- 默认目录内仅从列表移除而文件仍在的项目会再次发现；不增加持久排除清单。
- Library=true仍保留所有归档路径、链接、配额、取消、哈希和一致性验证；其他Library前缀目录继续原排除规则。

## 已执行的前端验证

- GUI check/build及SDK/UI check通过；三语言词条和Material控件/图标边界通过。
- 设置保存队列经现有esbuild打包执行，29个场景通过，包含并发备份字段合并、false意图及未知已提交核对。
- 首轮浏览器夹具构建未在原30秒门槛内就绪，未执行用例；当时完整Rust并行编译正在运行。未修改门槛、重试或断言。
- 独立Chromium定向41项为40/1。唯一失败为新读取失败夹具只注入一次错误，StrictMode后续真实读取已成功，未持续保持所需失败条件。
- 夹具改为明确保持错误至测试解除，产品代码未为该断言放宽。新增功能组与备份创建/未知写边界独立补验8/0，包括实际设置继承、单次覆盖不回写、读取失败不猜默认、自动列表跟进及过期读取保护。
- 三语分组和正常桌面尺寸的检查保留；已查看中文截图，确认“项目 / 备份”板块及默认true/true/false勾选状态。

最终前端增量后独立补验同一组8项仍为8/0（23.4秒）；清理已删除软件包区块的两处残留CSS选择器后，GUI check/build再次通过（1064条三语词条），同组最终补验8/0（21.7秒）。浏览器输出分别位于target/config8-project-backup-browser-20261103、-r2、-r3、-r4、-r5；不把各轮结果拼成一次完整浏览器通过。

## Rust与整合验证

application设置模块早期定向27/0通过。完整Workspace编译曾因本机资源争用调整为2个编译任务，之后因启动恢复并发整改中断；均不记完整通过。
后续完整编译遇到并行日志增量的Tauri异步命令返回类型错误，另一次不含GUI的尝试未完成。
日志增量自行修正签名后，沙箱完整Workspace运行实际结束：554 passed / 42 failed / 8既有helper ignored，日志为target/config8-workspace-tests-final.log。失败来自真实IPC端点访问拒绝及两项Windows junction夹具权限，未放宽任何断言。受影响的自动发现/启动恢复屏障、配置迁移/权限、实际ZIP内容矩阵及Store/协议用例已执行通过。

两项junction夹具在真实Windows用户环境独立重验2/0。随后只读启动所有权探针报告AlreadyRunning，确认当前原Core占用唯一端点；探针没有关闭进程或修改真实数据。所有者另行明确批准临时关闭并恢复GUI/Core后，保存原进程身份、二进制hash、Config7/revision168配置hash和五组完整公开读取。

真实用户首轮Workspace运行的正常汇总为593 passed / 0 failed / 7 ignored，但m4_package_rpc在报告组结果前发生STATUS_STACK_OVERFLOW，整个命令exit101，不能记完整通过。其余项目/备份/恢复RPC及kill-restart回归通过。日志为target/config8-runtime-validation/workspace-tests.log。

RPC派发新包裹的大型future在普通请求/可取消日志读取之间移动时增加栈占用；将该future放到堆上，保持两条路径原有语义。修正后独立m4_package_rpc实际流程5/0、1既有helper ignored通过，包含软件包安装/卸载及kill-restart，不增加线程栈、不改断言。日志为target/config8-runtime-validation-r2/package-rpc-tests.log，不把这次补验与上一轮拼成完整通过。

修正后的最终完整Workspace独立执行为**598 passed / 0 failed / 8既有helper ignored**，命令exit0，运行时段10:50:30–10:52:18 UTC。日志及汇总为target/config8-runtime-validation-r3/workspace-tests.log、test-summary.json；预编译先在原GUI/Core保持运行时完成，再短时释放端点执行测试。项目/备份默认值、实际归档及恢复、接受后修改设置、幂等重放、启动恢复并发和原软件包RPC失败组均在该完整运行中通过。

Workspace Clippy（-D warnings）、fmt --check、cargo xtask check、metadata及diff空白检查均通过。

最终463项代码/契约/构建输入记录在target/config8-validation-inputs-final-candidate.json，末轮完整测试结束后逐项hash一致（target/config8-runtime-validation-r3/inputs-after.json）。较早快照期间有并行日志修改，最终快照前另有上述CSS清理，均不借用早期快照的结论。这是未提交工作区的本地结果，不作为HEAD或旧远端CI的通过证据。

## 未覆盖与运行环境

不操作真实工程，不运行真实数据目录的自动发现或备份，不替换桌面产物。
所有者已单独批准为IPC验证暂时关闭并恢复原GUI/Core；测试服务只使用临时合成fixture。各轮finally均已恢复原二进制/数据/profile并核对五组状态及配置hash一致。最终保持Core138008 / GUI169992，GUI有窗口且可响应；项目、Editor登记、Operation、Settings及Status与测试前完全一致。原配置保持Config7/revision168，不迁移到Config8。

原Core SHA-256为163458B5137C0BAF22F8B8EBD5BE65E810B0BAA757B667D2C3B51225E6AC1F46，GUI为8CEDF3E66FDA554DDECCDBB714368007FC58D7E88CC6ED31058F7C1B80105039，TOML hash为7A58EF3E64C9579470204A2BC918D81159815C0204321EF0F1B359FD58147E82；恢复证据为target/config8-runtime-validation-r3/restored-verification.json及restored-runtime.json。当前桌面仍是原版本，尚未包含本次Config8增量。没有提交、推送、远端CI或新的原生人工验收。
完整M7/H7和三平台客户端验收仍按docs/status.md保留，不因这项功能自动完成。

## 桌面程序更新

所有者随后明确要求“重新构建并更新桌面程序，我看看效果”。原二进制和配置已保存至target/config8-desktop-update/original，核对进程身份和全部任务终态后，完成Core/CLI release构建（3分13秒）及GUI release no-bundle构建（2分22秒，含前端check/build）。构建期间463项代码/契约/构建输入与最终完整回归快照一致。

新版沿用原数据目录和WebView profile，当前Core165260 / GUI163356，窗口可响应。公开读取核对全部通过：Config8新增默认autoDiscover=true、compressionMode=fast、includeVpmPackages=true、includeLibrary=false；revision168和所有既有设置保留；原3个Project、Editor登记及Operation一致。未启动Unity或执行备份/恢复操作，不将窗口响应核对当作原生人工交互验收。

当前SHA-256：Core `26FC0D4B0AD097FC2861B6F572A8D4D3A5A7F0D4CCBE15788D7C1C343F76395F`，CLI `29DDE4B8436D5BE4BDBBD471840711B2522973E3F4FBC6EF079C9D3166685FA6`，GUI `EB7A876E0980B72A095F00957069B848298DD80A2FF54E36C2BF2B26C29CE291`。本次替换了桌面产物，覆盖前文测试结束时仍使用原程序的历史状态；构建及比较证据保存在target/config8-desktop-update，最新进程身份同步至target/m7-storage-locations/candidate-runtime.json。没有提交、推送或远端操作。

### 默认值文案补正

所有者指出保留选项应明确为默认值后，设置页改为“默认保留软件包 / 默认保留 Library 缓存”，英语和日语同步；备份弹窗的当次选项继续原文案及可编辑语义。只改显示文案及测试定位，不改后端或合同。默认值保存和单次覆盖不回写的现有浏览器用例2/0，GUI check/build及release no-bundle通过，未重复执行完整Rust门禁。

已更新GUI至162244，SHA-256为`D49854CC12A4BC9BBBF73F1EB39F58F9A53274BC506BDAA485AACDD4091EB16A`，Core165260保持运行，沿用原profile。五组公开状态与更新前完全一致，记录位于target/config8-default-labels-update；最新运行身份仍由target/m7-storage-locations/candidate-runtime.json提供。没有执行真实备份、提交或推送。
