# M7 实现依据与跨模块复核

状态：修复前只读实现复核与隔离探针已完成；A01–A10已由所有者批准实施。以下保留修复前事实，当前实施结果见ExecPlan。当前阶段仍以 `docs/status.md` 为准，
统一整改方案位于 [M7 ExecPlan](../exec-plans/M7-official-gui-extension-ui.md#实现依据与跨模块整改计划2026-11-05已批准实施)。
不替代 H2–H6 签收或 H7 最终人工确认。

## 对象、方法与证据等级

- 检查对象：标准目录 `main`，起始 HEAD `cba0b64fddaee2696accb35e907ee02c8dfddc0f`，包含保留的未提交修改。
  该 SHA 单独不能表示本工作区候选；日志历史等并行增量也不能借本报告获得完整通过结论。
- 检查实现、合同和测试的对应关系，优先验证外部输入、跨模块结果和持久化边界；没有重新运行全量门禁。
- 探针仅使用 `target/m7-official-basis-audit-20261105/` 下的离线样本、内存数据与临时配置/项目。
  本轮没有修改产品源码、真实项目或用户设置，没有启动 Unity、重启 Core/GUI、恢复桌面控制、提交或推送。
- “已复现”表示调用现有实现得到记录中的结果；“静态确认”表示可沿当前调用链确认；
  “待平台实证”不等同于三平台已经失败；“策略/子集”不自动列为实现错误。
- 外部格式以对应版本官方规范为主；真实官方样本用于验证输入；vrc-get 作为辅助生态实现。
  项目自有合同依据已批准决策，不因为与别的工具不同就推翻。源码测试可以证明内部一致性，不能自行证明外部格式正确。

## 来源

| 来源 | 使用范围与固定信息 |
| --- | --- |
| [VRChat 包格式](https://vcc.docs.vrchat.com/vpm/packages/) | `vpmDependencies`、legacy 字段形状、包字段；不把可选 SHA-256 误称官方必需字段 |
| [VRChat 模板](https://vcc.docs.vrchat.com/vpm/templates/) / [官方模板仓库](https://github.com/vrchat-community/template-avatar/blob/main/Packages/vpm-manifest.json) | VPM 项目直接依赖的版本对象与合法范围示例 |
| [VRChat 仓库格式](https://vcc.docs.vrchat.com/vpm/repos/) / [官方索引](https://vrchat.github.io/packages/index.json) | 2026-11-05 样本 266002 bytes，SHA-256 `ab2b60cf8c290cc5ea71d45b0a687e159a0525d6d2ab6c860ab2ec7c7741e5a5`；4 包、303 版本，不代表以后在线索引固定不变 |
| VRChat 文档引用的 [SemanticVersioning.NET](https://github.com/adamreeve/semver.net/tree/cbd74616bb01348f596715d6d22479106d04dda7) | 固定提交 `cbd74616bb01348f596715d6d22479106d04dda7` 的独立普通范围对照；不等于已经运行 VCC 全部调用路径 |
| [vrc-get 冻结实现](https://github.com/vrc-get/vrc-get/tree/14d73d018bdc90c5b005064eb10f8e9714fa8409) | 本地 `../vrc-get-readonly` HEAD 与冻结 SHA 一致；只读核对范围上下文、锁信息、Editor 版本获取；未复制/移植/包装其生产代码 |
| Unity 2022.3 [项目清单](https://docs.unity3d.com/2022.3/Documentation/Manual/upm-manifestPrj.html)、[包清单](https://docs.unity3d.com/2022.3/Documentation/Manual/upm-manifestPkg.html)、[Editor 参数](https://docs.unity3d.com/2022.3/Documentation/Manual/EditorCommandLineArguments.html) | UPM 与 VPM 区分、`unity`/`unityRelease`、明确 argv；不以当前 Unity 大版本的文档替代 2022.3 合同 |
| [TOML 1.0](https://toml.io/en/v1.0.0) | 注释、basic/literal string、Unicode 转义；Python 标准库 `tomllib` 独立核对语义，不引入生产依赖 |
| [Linux fsync 手册](https://man7.org/linux/man-pages/man2/fsync.2.html) | 文件内容同步与目录项持久化的区别；不能用进程 kill/restart 结果证明掉电可靠性 |
| [Rust Command](https://doc.rust-lang.org/std/process/struct.Command.html#method.args) / [Tauri 2 Channel](https://v2.tauri.app/develop/calling-rust/#channels) | argv 与传输 API；ACK、游标及业务失效语义仍来自 ALCOMD 已批准合同 |
| 实际锁定的 Material Web 2.5.0 类型、Wasmtime 48.0.8、版本化 WIT | 核对真实依赖和生成接口；不把浮动最新版文档当成当前依赖 API |

## 问题清单

### A01 / P1：legacy 对象被当作数组（已复现）

`alcomd-vpm/src/package.rs::parse_legacy` 对 `legacyFolders`、`legacyFiles`、`legacyPackages` 共用数组读取。
官方前两项为路径到 GUID 的对象，只有第三项是数组。
`unity_migration.rs::inspect_legacy_manifest` 也用 `string_array` 读取前两项；测试夹具则用数组配合实现。

官方索引逐项读取只有116个版本通过，187个失败；诊断探针仅移除前两个字段后303个全部通过。
这不是应当删除真实字段的建议。完整只读入口实际返回303条展示记录、0条 resolver-ready 元数据、`issues=[]`，
原因是 `lib.rs::parse_repository` 的整仓 resolver 解析失败被 `if let Ok` 忽略。
需要识别正确格式、保留 GUID 语义并准确报告不支持的操作，不授权新增 legacy 删除。

### A02 / P1：官方合法范围不在当前解析器能力内（已复现）

`range.rs::caret/tilde/parse_version` 要求完整三段版本，拒绝官方示例 `^3.1.x`，以及引用库支持的 `~1.2`、`1.2`。
同一逻辑还用于包依赖、用户包和模板，不是单一登记入口问题。
能力不足不应被包装成“官方清单无效”；冻结少量自造 vectors 不能代替正确的外部语法覆盖。

### A03 / P1：普通范围与项目直接依赖混用，差分依据不足（复现及辅助实现确认）

当前 `VpmRange` 将裸 `1.2.3` 统一转成 `>=1.2.3`；普通引用库对该字符串是精确匹配。
16个现有向量及3个 loose 接受项与固定引用库对照，有两处冲突：bare-minimum、`1.2.3 -`。
原依赖评估甚至据这个 bare-minimum 预期淘汰 npm matcher，不能继续把此项当作已验证的普通 VPM 范围规则。

新增 vrc-get 辅助核查纠正了“所有裸版本都应改成 exact”的过度结论：

- `version/range.rs::DependencyRange` 对项目直接依赖的单版本执行 minimum 语义；
- 普通 `VersionRange` 的完整裸版本执行 exact 语义；
- `package_manifest/mod.rs` 的 `vpm_dependencies` 和 `unity_project/vpm_manifest.rs` 的锁定依赖范围使用普通 `VersionRange`；
  项目直接依赖版本使用 `DependencyRange`。

ALCOMD 当前将这些上下文交给同一个解析器，差异必须按入口归类，而不是统一改成 minimum 或 exact。
辅助实现的这项观察不是 VCC 全链路实机结果，也不授权移植其 parser。

### A04 / P1：写出重建丢失未修改包的锁定依赖（已复现）

`plan.rs::write_manifest_maps` 重建每个 locked 条目时只保留 version。合法 Remove ChangeSet 的纯内存探针证明：
删除B，会使未修改A的 `locked[A].dependencies` 从对象变成缺失。
冻结 vrc-get 锁模型明确保存这项信息；真实项目样本也存在该字段。
当前证据证明信息丢失，未执行其他工具对被改写项目的后续 resolve，不能声称该影响已完整实测。
修正应覆盖 Plan/hash、Apply、模板 staging 和迁移中相同的写出入口。

### A05 / P1：非 Windows Editor 版本来源仍缺真实依据（静态确认、待平台实证）

`apps/alcomd/src/m5_platform.rs::observation` 在非Windows上从
`Resources/PackageManager/Editor/manifest.json` 顶层读取 `version`；测试相应构造 `{"version":"2022.3.22f1"}`。
本机真实同类文件是 schemaVersion/packages 包管理器目录，不提供这个顶层 Editor 版本。
Windows 已改用 PE 元数据，但不能把 Windows 修复推广为 macOS/Linux 已正确。
vrc-get 的辅助实现存在隔离 argv 的 Editor `-version` 查询，说明其没有依赖上述自造字段；
本轮未执行该方法，也不把它直接指定为唯一正确实现。

### A06 / P1：内置模板发布结果不满足自身启动合同（完整隔离创建链已复现）

`specs/templates/builtin-scaffolds/{blank,avatars,worlds}-v1.json` 都写 `m_EditorVersion: 2022.3`。
`template_create.rs` 发布后保留它；`m5.rs::canonical_unity_version` 和 launch/options 要求完整版本。
隔离空白模板执行真实 adapter Plan→prepare→stage→publish，结果 `unity_version="2022.3"`，
再调用现有规范化校验返回 `VersionUnverified`。没有启动Unity，没有写真实项目。

模板兼容范围与项目实际 Editor 版本是两种信息。不能编造补丁号，也不能为此放宽所有启动校验。
应在创建计划中有可验证的完整版本来源，并验证“创建后可进入启动流程”，而不止验证目录存在。

### A07 / P2：settings.toml 实际只支持未明确标注的语法子集（已复现）

`alcomd-store/src/config.rs::parse_settings_with_schema/parse_string/parse_path_string` 手工按行解析。
对同一默认Config10，仅添加行尾注释、把locale改为单引号、或把system中的s写为Unicode转义，
`tomllib`均得到与原配置相同的值，ALCOMD却全部返回Corrupt。

该结果不是“任何手写TOML都必须接受”，未知业务字段、错误类型、版本和大小限制仍须拒绝。
问题是合法语法与非法业务内容混为一谈。优先评估复用workspace已用的TOML库，保留closed DTO校验；
若新增store的生产依赖边，需记录并批准。版本/revision数值边界也必须保持，不因换parser暗改合同。

### A08 / P1：配置替换缺少目录持久化步骤（静态确认，未实测掉电）

`config.rs::publish_settings` 同步临时文件后rename目标/备份并删除备份，没有目录同步；恢复分支也没有。
Linux手册明确文件fsync不保证所在目录项落盘。包、模板、备份路径已经存在目录同步步骤，配置路径没有对齐。
这证明当前实现不足以据此承诺掉电后可恢复，不证明本机已经丢数据。
应复用经平台验证的原语并分别验证rename/目录同步失败和恢复窗口，不将kill/restart冒充断电测试。

### A09 / P2：Unity 补丁级兼容性是已限定子集（能力限制）

package parser保留`unityRelease`，resolver/candidates只比较Unity major/minor。
Unity官方定义该字段可补充具体release；M7候选合同也明确没有新增其解释。
因此不能称为完整Unity兼容性，更不能把它列作本轮新回归。若补齐，先统一普通Plan和候选查询的判断及合同，
不能只修GUI显示，也不能以包`unity`是最低版本为由将项目启动的exact Editor选择一起改成minimum。

### A10 / P2：历史文档仍把旧 Config 标成“当前”（静态确认）

例如`settings-v1.md`、`settings-v6.md`头部仍指向Config7作为当前合同，当前实际为Config10。
状态入口是正确的，但这些交叉引用会给后续实现者提供错误入口。整改只更新现行引用与能力声明，保留真实历史证据。
同样，已有passing CI、synthetic fixture、平台编译与真实安装证据必须分开表述。

## 其他模块覆盖与保留结论

| 模块 | 本轮检查及结论 | 未覆盖 |
| --- | --- | --- |
| IPC / Rust客户端 / TS SDK | 自有RPC明确声明不是JSON-RPC2.0；请求响应ID实际校验；通知另有类型；不能因为“相似”强改成外部协议。读超时废弃连接与写命令不自动重发有具体实现 | 未重跑全SDK兼容/所有畸形帧向量 |
| Store事件 / 任务计数 | 登记watch后检查水位；任务list/count在唯一写线程同次执行；activeOnly与count分离，终态三态与domain/GUI一致 | 未重跑全并发/提交断开/断电矩阵 |
| Tauri通知 / GUI同步 | 独立命令/订阅入口、显式ACK、取消/代际检查、合并和blocked读取重试均有合同及代码；Channel发送成功未被当作消费成功。没有据此新增通用消息框架 | 未重跑慢消费者/窗口重载和全部浏览器用例 |
| GUI设置 / Material / 路径 | 保存通过RPC/CAS，失败/未知结果核对有实现；Material使用真实2.5.0控件与本地类型。布局偏好不是官方协议，不能因“看起来不像默认”判错 | 桌面控制保持停用，无新原生视觉/焦点签收 |
| 备份/恢复/复制/删除 | 原生Backup Archive v1与Template Bundle v1是批准的ALCOMD格式；配额/内容选择和不覆盖恢复不是Unity或vrc-get标准。抽查no-replace发布、摘要和持久化入口，未确认新的格式混淆；模板另有A06 | 未重跑全部profile、坏归档、恢复、源/目标竞争矩阵 |
| 扩展/Portable UI | exact自有WIT+Wasmtime生成绑定；ABI明确不链接ambient WASI，未将无WASI导入误判为漏实现；权限/宿主与自有格式受批准合同约束 | 未复测全语言组件、资源限额、重入与所有hostile组件 |
| 系统目录/平台 | Windows Documents经平台API，Linux读取XDG user-dirs；没有因路径外观而判其硬编码。Editor来源缺口见A05，目录持久化见A08 | 无本轮macOS/Linux真实安装验证 |
| 技术日志 | 只读检查了当前未提交的读取/桥接改动边界；它们仍属并行候选，未获得本轮功能通过结论 | 文件历史轮转/游标/正文及通知完整行为未在本轮重验 |
| MCP/后续组件 | 官方`2026-07-28`规范页面实际存在，当前latest亦指向它；因此没有把该日期误判为编造。当前MCP等scaffold不等于已实现 | 不进入M8–M12，不重建或逐条验收其未来合同 |

## 本轮验证与证据

全部本地原始证据在`target/m7-official-basis-audit-20261105/`：

- `scope.json`、`probe-inputs.json`：起始对象、官方样本摘要和parser文件摘要。
- `probe-results.txt`、`official-index-parser-results.txt`、`writer-reader-results.txt`：格式、范围、官方完整读取与写出丢字段。
- `official-reference-results.json`、`fixture-reference-comparison.json`：固定引用库与现有向量的对照。
- `other-modules-results.txt`、`toml-reference-results.json`：现有store与独立TOML读取的差异。
- `template-chain-results.txt`：隔离空白模板完整adapter创建链与版本校验。

临时探针首次链接曾因并行缓存中application/Tokio产物不匹配而失败；最终选择一致产物后完成，
没有改产品来迎合探针。模板副作用仅在target中的独立fixture，不涉及真实项目或Unity进程。
没有把此前125项VPM测试、8项IPC或历史CI拼接成本轮跨模块回归通过；本轮没有功能修复或应用更新。
