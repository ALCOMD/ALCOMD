# 项目软件包正常读取与副本撤除（2026-11-08）

按所有者明确要求撤除新加的项目展示数据库副本及补采流程。此前的展示持久化方案和 GUI 自动 projects.refresh 方案均已被本轮取代。v3 仅用于只读行为审计，本轮实现独立编写，没有复制参考源码。

## 实际行为

- 普通候选请求先校验权限、项目 scope、请求及登记 revision，再由 Core 读取项目清单和本地 package.json，返回当前安装版本、依赖范围及展示元数据。GUI 仅展示 RPC 结果，不直接读取项目文件。
- 新普通请求重新读文件；分页、搜索和版本选择可复用单份有界内存结果，不写数据库。文件指纹进入候选快照，新普通读取发现变化后旧游标被拒绝。
- 与 Core 项目写入共用项目资源锁，避免读取正在 Apply 的半成品。查询不发布项目登记快照、revision、事件、幂等记录或操作日志。Plan/Apply/recovery 冻结授权、项目文件过期检测保持。
- 移除项目发布路径中的展示副本写入钩子、Store 读写模块和两张表。内部格式31的唯一删除清单为 project_package_presentations、project_presentation_observations。仓库 presentation_json 仍是仓库规范化索引，既有登记/事务/历史保持。
- GUI 移除额外提示区域、独立补采按钮和自动填库调用。原刷新入口更新仓库后正常重查候选，安装样式、来源过滤、排序及100ms导航预算保持。summary 的可选 requestedRange 与 installed 同源，避免拼接旧登记版本。

## 验证

| 范围 | 实际结果 |
| --- | --- |
| application、store（含历史迁移与冻结恢复）、vpm 模块和集成 | 434 通过，3 项既有显式忽略 |
| 候选公开合同、项目动作合同、真实文件读取 | 16 通过 |
| M4 合同、候选取消/分页/设置竞争 | 9 通过 |
| 真实私有 RPC / Plan 权威回归 | 1 通过 |
| Chromium 软件包展示、工作区、候选和导航加载 | 82/82，零重试、零跳过 |
| 受影响 crate all-targets check、严格 Clippy | 通过 |
| fmt、xtask、元数据、公开 DTO/Schema 形状 | 通过；形状26例、语义引用39向量/81引用（形状脚本不代替执行测试） |
| 前端全工作区检查/构建、locked Core、Tauri debug no-bundle | 通过 |

真实文件用例在登记后修改 package.json 名称、锁定版本和直接依赖范围，普通请求读到新值，无 projects.refresh 前置步骤；数据库项目记录、revision、事件及日志不变。该用例还证明分页不重复扫描、项目写锁期间不会扫描文件。删除表用例核对既有项目权威与历史不变。共享项目锁接入后的相关模块、文件读取及竞争用例已复验。

早期测试夹具的依赖格式和 Rust future 生命周期问题已修正并复验。首次私有 RPC 回归被本机运行中 Core 占用的 Windows 用户管道阻断（AlreadyRunning），未改生产端点；确认无活跃任务、备份并停止原 Core 后，同一回归通过。早期日志保留，不计作通过。

证据位于 `.local/package-direct-read-20261108/`：affected-tests、lib-final-tests、read-contract-final、direct-final-tests、rpc-owned-final、clippy-verified、browser-results、前端/原生构建等日志。跨平台 Hosted CI/CodeQL、完整 workspace 测试未在本轮重跑；三项既有 VPM 忽略测试不计为执行通过。未执行原生页面内部自动化，窗口状态核对不代替 H7 人工签收。

## 本机数据与程序更新

更新前确认 Core ready、活跃任务0，备份数据和旧 Core/GUI 二进制；继续使用原数据路径和 WebView profile，没有恢复旧数据库覆盖用户数据。正式 Core 初始化自动清理上述两张表，从内部格式30进入31；不是产品发布升级。

清理前后只读 quick_check 均通过。对所有剩余34张表（包含 SQLite 内部表）逐表比较列、行数及完整记录摘要，全部一致；设置与四个模板文件逐字节一致。两张退役表均不存在。证据为 database-comparison.json、data-before 与 backup-manifest.json。

更新后实际 RPC 为 ready / State31 / Config12 / packages.candidates.v2，1项目、2仓库、0活跃任务，正常候选查询成功。Core PID271004，GUI PID233792；窗口句柄非零且响应，6个 WebView 进程匹配原 profile，Core/GUI stderr 均为空。

| 程序 | SHA256 |
| --- | --- |
| Core | F846F6EFDD687EF25F932E44AC9697CAAF1B165A235D195378B978114CBCD345 |
| GUI | 6DF21CD4A29D6DCCB813FC3942DAEC7AB323551A279EEB2AFE30DF2C3DBE1B1D |

构建输入为 `832899acf86e537f25300a272efd1ee38f86b8e1` 后的本轮未提交工作树；文件摘要和运行核对保存在本地验证清单，不冒充绑定已提交候选的最终验收。本记录对应的构建与更新当时尚未提交，随后所有者已要求并授权本地提交本轮读取修正。未推送、改写历史或发布；M7/H7 总体阶段保持。
