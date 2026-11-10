# 仓库清单可见性修复验证（2026-11-10）

## 结果与授权

本轮修复 Core 对可选展示字段的过度限制，见[计划](../exec-plans/M7-repository-manifest-visibility.md)及获所有者明确批准的 [State32](../../specs/storage/state-v32.md)。无公开 RPC 形状、Config13、权限或扩展 API 变更，无新增依赖。主分支继续开发，无引用实现源码移植。

Nadena 的 4 包/397 版本中，Modular Avatar - Resonite support 的 42 版本没有 author；lilxyzw 的 11 包/139 版本全部没有 author。旧普通读取保留展示行，但 resolver 解析要求作者非空，候选 Store 最终排除这些行。作者/显示名/描述不再决定安装资格；缺省作者保存 NULL，对象与字符串作者均受原长度边界归一化。姓名、描述等无效展示值按缺省处理，数据文档的总体大小/深度/字符串边界保持。

扩查全部 30 个当前匿名公开仓库，共 4,630 条版本记录；其中 892 条涉及 28 个不同软件包缺省 author。另发现 UniGLTF 六版本合法的 path-only legacyFiles 使用 null GUID，统一转换为既有空 GUID 表示。实际删除路径、范围、链接、内容证据、项目/来源冻结及事务检查保持，不能据此授权任意旧资源删除。

最终 4,630 条全部通过有界读取、Store 持久化、求解目录构造、软件包候选与摘要链路。按原合同，已撤回版本仍可入库但不成为新安装目标；预发布及禁用来源/包仍由真实设置筛选。本审计显式包含预发布、未禁用来源，用来检查解析完整性，不改用户开关。此结果不是对每个远程归档、Unity 或真实依赖组合的安装证明。

## 依据与回归

- Unity 2022.3 [Package manifest](https://docs.unity3d.com/2022.3/Documentation/Manual/upm-manifestPkg.html) 将 author 设为可选 object/string，displayName/description 为推荐展示字段。[VPM Packages](https://vcc.docs.vrchat.com/vpm/packages/) 的发布说明列出更严格的字段清单，同时支持无 GUID 的路径声明。本修复采用现有发布物的消费语义，不把发布建议变成用户安装门槛。
- 原始两仓库获取摘要、30 仓库收据、实际原始 JSON、全量审计结果位于 `target/m7-repository-visibility-20261110/{raw,corpus,corpus-receipts.json,audit-result.json}`。全部只读获取，不下载包归档、不使用认证、不写外部服务。
- 版本化冻结三份逐包代表声明共 16 包，包含真实 Resonite、全部 lilxyzw 包及 null GUID UniGLTF；附来源与 SHA-256。新回归覆盖完整读取→Store→resolver/candidate、无显示名最小包、字符串作者/异常展示字段、解析失败旧索引重新读取及健康 HTTP 条件请求。
- author/displayName 误必填与 null GUID 在修复前均被独立回归复现失败，保留 red 日志。早期过长测试输入触发全局 JSON 边界，随后使用字段边界内用例区分展示与文档有效性；全局边界没有放宽。既有健康行/坏行测试增加真实非法依赖结构，保留其所有原断言，展示异常另行验证可安装资格。
- State31→32 隔离迁移保留行、已固定归档摘要、版本清单指纹、修订/顺序/自定义名；只让错误索引待刷新并去除其 validators，健康来源保持。INSERT/UPDATE 接受 NULL 作者/显示名，空字符串/超长作者、缺少真正业务元数据仍拒绝。原 State27 的 HTTP(S) 归档规则继续成立。
- 沿用真实 RPC 安装/卸载全流程的全部断言，将该用例的仓库展示字段移除后再次成功，其他有作者的手动兼容性、幂等和恢复用例继续保持。

## 实际检查

| 检查 | 结果 |
| --- | --- |
| application / Store / VPM 库 | 80 / 92 / 187 通过；VPM 的 3 项原有外部 fixture 用例仍忽略 |
| Store m3_registry / schema_contract / store_handle | 16 / 42 / 12 通过 |
| m3_read_rpc / m4_contract / m4_package_rpc / m7_candidates_rpc | 4 / 6 / 6 / 1 通过；子进程入口 1 项按原约定忽略 |
| repository_auto_refresh / auto_refresh_rpc / refresh_all | 2 / 1 / 1 通过 |
| 新清单可见性套件（包含显式全量审计） | 4 通过，4,630 条版本进入完整链路 |
| 缺省展示字段的真实 RPC 安装/卸载复验 | 1 通过，与上述 m4 用例重叠不累计 |
| State32 最终迁移保全复验 | 1 通过，与上述 Store 用例重叠不累计 |
| 受影响三个库 all-targets 严格 Clippy、相关 integration Clippy | 通过 |
| fmt / xtask check / diff check | 通过 |
| 独立 Core 构建 | 通过；显式输出路径产生两项既有 Cargo 输出命名提示，无源代码 Clippy 警告 |

额外 `m7_project_actions_contract` 为 8 通过、1 既有失败：`official_gui_local_project_affordances_remain_closed` 仍要求旧 `gui_select_directory(app: tauri::AppHandle)` 完整签名，而此前选择器实现已有路径参数。本轮 GUI 字节与 HEAD 完全相同，HEAD 测试也含同一断言，证明既有不一致；证据 `baseline-static-guard.json`。保留该断言，不借本轮降低或改写无关验收条件，也不把该套件记为全通过。

一次 RPC 刷新测试与另一独立 RPC 套件同时运行发生 Windows 命名管道 AlreadyRunning；两套件分别完成后顺序复跑通过，不修改测试或占用真实运行实例。旧元数据检查脚本 Config12/13 的已知不一致不在本项修改，未将其计为通过。没有运行全工作区、跨平台 Hosted CI/CodeQL、真实 Unity 或逐一远程包 Apply。GUI 未修改，未重复前端/浏览器门禁；本项不扩大 M7/H7 签收。

## 构建与运行边界

独立候选：`target/m7-repository-visibility-20261110/alcomd-new.exe`，SHA-256 `dec4c7b5e34f5a9fec120536cf62f82cf5abdb33fad073e6325d5d0463f8706f`。当前运行 Core PID54660 / GUI PID46468 及安装目录二进制保持；Core 旧 SHA-256 `B003BAFE778869CA00FA66CFCF35FE15EFD35736DCDDE6354E8A6768A2FAAC8F`。

State32 方案已由所有者明确批准；本轮未替换运行程序、未迁移真实数据库、未操作真实项目软件包、未提交或推送。将来更新后由既有后台维护重建原错误索引，健康缓存继续复用；旧 Core 不接受 State32 数据，回退时应恢复升级前离线数据备份。

## 分次提交与推送授权

所有者随后明确要求将仓库可见性、删除任务未移动失败这两项修复分别提交并推送。本项作为独立提交保存；上文未提交/推送为实施验证当时的状态。推送前单独核对目标 main 的既有本地历史，实际远程结果以 refs 核实。本次提交/推送不包含运行程序更新、真实数据迁移或真实项目删除。
