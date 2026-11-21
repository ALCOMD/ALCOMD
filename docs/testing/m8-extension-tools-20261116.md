# 扩展 MCP 工具本地候选（2026-11-16）

代码候选：`54ba42fd47b7228d1a7be08dff9d468a6d31821e`。所有者已批准设计并要求实施；标准目录/main、真实 Windows 用户本地提交。M8 仍为 IN_PROGRESS，未推送，没有本批 Hosted CI / CodeQL；不使用此前 36a782c 的远端结果替代。

## 实际完成

- 正常签名包通过固定的 `component/mcp-tools.json` 声明工具；安装时校验声明、Schema 和真实组件 ABI。扩展自身申请权限不等于实际授权，发布工具是独立的可选开关，默认关闭，零授权启用不变。
- Core 合并内置六工具与扩展目录，按客户端能力、OAuth 上限和扩展当前授权过滤。名称含 ExtensionId 摘要，来源绑定发布者/包摘要；分页游标绑定凭据、权限和来源快照。无逐项目/逐工具白名单。
- 新的独立 provider/core WIT 复用原 Host。自定义计算及既有项目读取、卸载 Plan/Apply、任务读取/取消的类型化桥接，不提供任意 RPC、owner 代理、数据库或文件访问。工具调用不隐含 UI 或后台启动。
- 持久工具每次至多一个 Core 动作。确认、原 Plan/Operation、revision 和幂等键由 Core 捕获；Apply/cancel 的 guest 导入没有可替换的目标参数。实际 MCP Principal 持有计划/任务/幂等，资源 owner 不改。
- Core 记录已接受调用并验证扩展授权。接受结果与既有业务事务原子提交；不保存参数、guest 正文或凭据。同键只恢复原结果，取消、断线、陷阱、无效输出不撤销已接受任务，也不自动重新执行。
- GUI 复用现有 Material 开关、错误组件与声明预览，保留身份和派生能力说明；不增加导航或第二套通知体系。扩展工具使用已认证的 MCP 调用者身份。

## 实际验证与输入

日志位于 `target/m8-extension-tools-20261116/`。以下是各自真实运行，不相加为 Workspace 或完整浏览器通过。

| 范围 | 结果与证据 |
| --- | --- |
| Application / Protocol / Extensions | 分别 94/0、23/0、15/0，`affected-libs-final.log`；包含声明、原确认参数、权限分离、参数摘要与禁止伪造身份 |
| Store | `store-lib-second.log` 整轮 136/0；其后仅新增重启恢复用例 1/0（`receipt-restart-final.log`），不称整轮 137/0。持久化保全、回滚、重开、双方授权/撤销、lease 续期、原子结果与重复动作拒绝包含在该模块 |
| Core / MCP 单元 | Core 46/0、1 项原有 ignored；MCP 11/0，`core-lib-final.log`、`adapter-lib-final.log`。开放生产 capability 后 hello 具名 1/0（`production-hello-final.log`） |
| Host | Schema 策略 3/0、真实组件模块 5/0，`host-policy-second.log`、`host-abi-final.log`。外部/动态/循环引用拒绝，普通 JSON 数据不误判；真实 Provider 不经过 lifecycle activate |
| RPC / ABI / 产物合同 | M6 5/0、M7 当前版本引用 9/0、管理扩展 2/0、新 Provider 2/0，`contracts-final.log`；M7 原签收不重开。LF 规范化后再核对 Provider 2/0（`provider-provenance-final.log`） |
| 真实 Core + 冻结 SDK | `business-module-final.log` 单轮 7/0，183.08 秒；包含原 STDIO、HTTP、OAuth、游标、两种传输风险与新增签名 Provider 全闭环。不是将之前局部结果拼成整轮通过 |
| 最终代码候选 Provider | 生产 capability 开放、DTO 可选字段省略及最终程序构建后，具名闭环 1/0、77.01 秒（`provider-candidate-final.log`），精确对应上述提交；其余六例不重复执行 |
| GUI | `frontend-check-4.log`、`frontend-build-final.log` 通过；1294 条三语言消息及 36 份图标校验通过。相关 Chromium 单轮 21/0、零重试（`provider-browser-second.log`），含新发布开关、失败还原、安装审阅、明暗主题和窄窗长文本。此后 GUI 输入未变 |
| 构建及门禁 | Core/MCP/Host 正常 debug 程序构建通过（`production-build-final.log`），受影响十包所有 targets 严格 Clippy、fmt、locked metadata、xtask、diff 通过；无完整 Workspace、完整 Chromium、Tauri/release 或远端 workflow 重跑 |

真实 Provider 整轮数据在 `target/m8-extension-provider-tests/7a8d47f2-d61b-4a27-9945-27e526508945/`，最终代码复验数据在 `target/m8-extension-provider-tests/87ff3ea4-0151-4dc2-b640-ffbafddb841f/`。各自 `summary.json` 记录两个独立客户端和八个成功的真实卸载 Operation。直接只读核对各自恰好八次 Apply、项目 owner 未改。测试复用正常签名/发布者确认、Core 登记/授权和同一 Host，不存在测试专用生产旁路。

候选证据 `final-evidence.json` 绑定 831 份输入，汇总 SHA256 `afc5f95449f63f8b3175d2b1007751dfbd513bb6e1df4598129700e49a4f55d1`。三程序摘要见 `production-artifacts.json`。此前整轮业务运行后只开放已验证 capability、调整 DTO 可选字段省略并补测试/规范；最终新 Provider 具名重验覆盖这两项运行增量。GUI、Store 等未变输入复用各自结果，不声称所有检查在提交后重跑过。

风险验证包括：真实响应接受后发送协议取消；截断真实 Core 响应并返回 `core_write_outcome_unknown`；显式同键恢复同一 Operation；第二次持久动作拒绝；guest 输出错误、陷阱和配额耗尽后仍保留已接受结果；三次真实故障进入既有隔离，新调用拒绝，停用后仍能只恢复原结果。OAuth 通过真实 HTTPS/SDK 证明 token 绑定分页、能力上限、真实 Provider Plan/Apply 和撤销拒绝。重启证据是额外的真实 Store 持久库测试，不能描述为整个 Provider 原生流程重启复验。

## 失败轮次保留

- 首次真实 Provider 0/1：ABI 预检临时文件名不符合现有 `extension.wasm` 合同；改用独占 staging 子目录，不放松路径检查。随后基础闭环 1/0。
- 扩展 OAuth 风险首轮 0/1：尝试空 consent 被现有合同正确拒绝；沿用非空能力上限，不新增 OAuth 语义。次轮 0/1：八次真实卸载已完成，但最后断言仍期待隔离后的扩展可调用；改为明确检查 quarantine 与新调用拒绝。最终整轮 7/0 如上。
- Store 首轮 132/4，修正与数据库结构不符的测试快照后复验 136/0。
- 最终具名命令曾因缺少 Rust 模块前缀筛选到 0 项（`provider-real-core-risk-final.log`），不算通过；随后整轮实际执行 7 项。Schema 单元命令首试误用无库包的 `--lib`，未执行；修正为实际 bin 测试 3/0。
- 初期编译/Clippy/前端检查失败和修复日志仍在原目录；没有禁用 lint、放宽断言或修改失败轮结果。

## 依赖与产物

Host 直接依赖 `jsonschema =0.61.2`（MIT、声明 MSRV 1.85），`default-features=false`；`jsonschema` feature 集为空，禁止 HTTP/file/async/TLS/idna resolver，另有拒绝外部资源的 retriever。现有 Rust 1.97.1 已成功构建。五个传递包未声明 MSRV，不将该信息补写为已知。

锁新增 28 包，无既有包移除/升级：ahash 0.8.12、borrow-or-share 0.2.4、bytecount 0.6.9、data-encoding 2.11.1、email_address 0.2.9、fancy-regex 0.20.2、fluent-uri 0.4.1、fraction 0.17.0、jsonschema/jsonschema-regex/jsonschema-value/referencing 0.61.2、micromap 0.3.0、num 0.4.3、num-bigint 0.4.8、num-cmp 0.1.0、num-complex 0.4.6、num-integer 0.1.47、num-iter 0.1.46、num-rational 0.4.2、outref 0.5.2、strum/strum_macros 0.28.0、unicode-general-category 1.1.0、uuid-simd/vsimd 0.8.0、zerocopy/zerocopy-derive 0.8.63。许可为 MIT、Apache-2.0、MIT-0 或其列明的双重/三重许可（zerocopy 含 BSD-2-Clause）；逐包许可/MSRV/features 见 `dependency-audit.json`，实际树见 `host-feature-tree.txt`。

明确存在第三方 SIMD/unsafe 影响：bytecount 的 runtime-dispatch-simd、uuid-simd/vsimd 的 detect/alloc/std、zerocopy 的 simd。serde_json 新增 feature 统一为 float_roundtrip，已有 raw_value/unbounded_depth 等保留；不能描述为零 unsafe 或所有 feature 未变。MCP 只加三个仓库已有的测试依赖边，未新增直接生产依赖。

参考 guest 及 provenance 已提交到 `extensions/examples/mcp-tools-provider` 与 `crates/alcomd-testing/fixtures/m8`。构建输入统一仓库 LF，再生成组件逐字节仍相同：SHA256 `d9ac5e894881ed90228f32cef88eceb240337dfb3df050555a769282d8ee8721`，108974 字节。旧 WIT 六份文件逐字节保持；不复制第三方产品源码。

## 保留边界与未验证

- 本批尚无 Hosted CI/CodeQL、macOS/Linux 实际执行或新原生 GUI 人工签收；浏览器截图是夹具，不能冒充真实桌面。GUI 未重新部署或打开。
- 新 Provider 的真实接受测试重点是卸载 Plan/Apply；取消桥接已接通并复用原 Core，但本参考 Provider 对运行中任务成功取消的独立原生场景尚未制造，不冒称覆盖。原六工具取消/断线回归及 Store 的既有权威规则保持。
- OAuth 仍要求既有非空 consent；零业务权限纯计算由现有凭据 STDIO/Bearer 验证，不新增空 scope OAuth 模型。
- 本批不提供 Unity、备份、外部网络的 Host 用例；正式 MCP/Discord 业务、发布分发和既有人工待办不由测试包替代。M8 整体未完成，不进入 M9。
- 原环境与 review 在测试前实际已关闭；只运行本轮可丢弃数据，没有暂停待恢复进程或升级原库。`final-preservation.log` 实际核对原 data 36 份、review data 10 份及 review WebView profile 303 份，集合和 SHA256 全部不变。唯一原进程 Unity PID40632、程序路径保持；无本轮 Core/GUI/Host 遗留，原先关闭状态保持，不存在守候恢复。原 profile 未打开或升级，不恢复桌面控制。

后续人工只需看新增声明预览与权限开关：扩展安装审阅的身份/工具/申请能力；详情中的工具说明；管理权限中的“发布 MCP 工具”和对应业务能力。现有管理扩展未声明工具时不应出现发布工具开关。不要求重验 M7 或证明整个权限系统。
