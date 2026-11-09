# 软件包确认后下载验证（2026-11-10）

## 实际行为

普通安装、更新、降级、解析、重装和批量 Plan 不再获取仓库包 ZIP，也不为缺失发布方摘要执行首次下载。先固定来源元数据、求解、比较已安装声明及发现新增旧资源，再显示确认；Apply 接受后才下载、校验、暂存。普通更新继续跳过已有路径声明，重装完整检查所选声明。

经过来源/旧资源准备的新普通计划（含空清理范围）使用 formatVersion 4，原项目元数据指纹 v6 升为 v7。仓库 archiveSha256 可省略，用户包摘要仍必需。执行时实际摘要绑定既有来源记录，计划本身保持不变；同一源已固定的摘要可供原来没有摘要的计划复用。目录/清单写入前及清单提交前核对归档依赖、Unity 要求和有效清理清单，拒绝增加未确认的操作。索引声明仍为权威，归档省略已确认的声明不撤销清理。

GUI 移除普通未更新项及数量文案；Core 候选 API 的排除证据保留，确认窗口只展示变更及必要冲突。Schema、公开 DTO、Core 桥接和 GUI 类型同步；无 SQL 表列、State/Config 版本、权限或生产依赖变化。既有客户端若强制要求仓库摘要字段，需要同步支持省略；旧未执行普通计划须重新生成，已进入事务的恢复保留。

## 实际验证

证据文件均位于 target/，`m7-confirm-final-inputs.json` 绑定最终源码和合同摘要。

- `m7-confirm-modules-second.log`：Application/Protocol/Store/VPM 模块及其集成共 525 项通过，3 项既有忽略。之后新增协议缺省摘要用例由 `m7-confirm-protocol-final.log` 单独通过；最终 VPM 输入由 `m7-confirm-vpm-final.log` 的 182 项重验通过（不重复计数）。
- `m7-confirm-integrations-third.log`：52 项通过，包含旧资源 24、VPM 语义 19、包闭环 3、合同 6。新增本地 HTTP 用例逐项验证有哈希、无哈希、批量、已有持久摘要复用、依赖差异、Unity 差异及新增清理目标；Plan 的包请求数为零，Apply 才请求，复用摘要且已有缓存时保持零请求。摘要不符与未确认操作均在项目写入前失败；保存的 Plan 来源和摘要不变。
- `m7-confirm-rpc-second.log`：6 项 Windows RPC/事务回归通过，1 项子进程专用入口按原设计忽略。含真实隔离 IPC、手动 Unity 覆盖、用户包目录变更/登记删除后的拥有快照、缓存缺失拒绝，以及归档阶段和破坏阶段的 kill/restart 矩阵。无摘要计划的恢复复用由上述持久摘要夹具覆盖，未额外运行其专门 kill/restart 矩阵。
- 共 584 项不同 Rust 用例通过；重验和辅助子进程不重复计数。
- `m7-confirm-browser-final.log`：3 份包管理浏览器文件共 85 项通过。确认窗口不含普通排除项，操作目标、键盘/焦点返回、候选证据及来源关闭继续成立。
- `m7-confirm-frontend-first.log`、`m7-confirm-build-final.log`：全前端检查及生产构建通过。Vite 保留既有大 chunk 提示，不计为失败。
- `m7-confirm-clippy-final.log`：Application/Protocol/Store/VPM/daemon/testing 全目标严格 Clippy 通过。`m7-confirm-fmt-final.log`、`m7-confirm-xtask-final.log` 及最终 git diff --check 通过。
- 已核对上次模板/预取输入清单：除本次有意调整的 actions.ts、m7-package-candidates.spec.ts 和 schema_contract.rs 外，原输入摘要保持一致。

## 实际失败和修复

- 首次编译发现测试助手参数和忽略的真实 VRCFury 用例仍依赖 Plan 首次下载，已同步到确认后获取；后者保持缓存损坏/项目不变的断言，未运行外网忽略用例。初轮还缺少 M3RegistryStore 的测试导入，最终 Clippy 已覆盖编译。
- 初轮沙箱链接器无法在用户 Temp 创建 lnk 临时文件；将 TEMP/TMP 指向项目拥有的 target/m7-confirm-tmp 后通过。浏览器首轮 esbuild 被沙箱阻止读取上级目录，按审批以真实用户环境重试并通过，未改权限断言或源码解析规则。
- 新增归档旧资源用例首先返回旧格式的 package_legacy_cleanup_required，暴露空清理计划仍使用旧格式；新普通计划统一冻结 formatVersion 4 后，按完整确认范围返回 plan_stale，断言未放宽。
- 原来源关闭并发用例等待 Plan 下载，因新合同超时；已移到确认后的 Apply 下载期间，并增加 Plan 零包请求断言，继续证明关闭来源不写项目。
- 首轮 RPC 的两处 formatVersion 3 断言按新合同更新为 4；完整套件复验通过，其他断言保留。

## 范围

只操作隔离测试项目和本地测试 HTTP/IPC。未提交、推送、替换运行中的 Core/GUI，未对真实项目安装、更新或删除包。未运行全 workspace 测试、三平台 Hosted、原生 GUI 人工验收或真实多包延迟基准，不扩大 M7/H7 签收。

旧资源清单缓存缺失时，仍按原来源身份/清单指纹读取对应仓库清单；不会据此刷新已登记候选或下载包 ZIP。Plan 的依赖计算、新增声明的路径/GUID 查找仍可能耗时，不能宣称确认窗口必定立即出现。
