# M7 H2-A：官网主题逻辑对齐

所有者明确要求与 VRCLearn 官网整套主题逻辑对齐，并批准 `specs/config/theme-controls.proposal.contract.json` 的最小公开合同增量。本轮仅在标准 `main`、HEAD `7d938cf2845cd982c25b12f106a8dac33981c8e7` 上实施，保留原有翻译、字体、项目最后修改时间及观察字段移除 WIP，未提交／推送。Visual Gate 2 **PENDING**。

## 来源与实现

只读核对冻结 v3 `vrc-get-gui/lib/material-theme.ts` 与 `components/MaterialThemePanel.tsx`；联网核对 `https://www.vrclearn.com/` 当前主题模块。两者均以 `#6CB6FF` 为默认种子、Vibrant 为默认方案、跟随系统为默认模式，提供九方案与 HEX／拾色器／HCT 联动、即时应用、持久化和主题重置。网站 cookie 和 v3 配置实现不进入 v4；没有复制网站／v3 源码。

共享引擎采用官方 `@material/material-color-utilities 0.4.0`（Apache-2.0，Google LLC，纯 TypeScript，无 runtime dependencies/native addon），版本、完整性、用途／替代方案／风险及许可已记录于 lockfile、当前 ExecPlan 和 THIRD_PARTY_NOTICES。库生成完整主／次／第三色、surface、error、fixed/inverse、shadow/scrim/tint 等语义角色；共享 CSS 包含同算法静态默认，避免首次应用前回到旧紫色。删除三色名映射和页面里的系统暗色硬编码，合法任意用户 HEX 都按真实种子生成色板。系统媒体查询统一切换成对的明暗色板。

九种方案为 tonal-spot、fidelity、monochrome、neutral、vibrant、expressive、content、rainbow、fruit-salad。HEX 只接受完整六位，可 trim／规范化大小写；无效输入不产生写入，失焦恢复最后有效值。拾色器与 HEX 双向同步，HCT 的 hue 0–360、chroma 0–150、tone 0–100 交给官方 HCT 转换和可达 sRGB 色域；HCT 不另行持久化。Slider 使用真实 Material Web 控件，浏览器原生颜色选择对话框是 Material Web 没有对应组件的唯一例外，集中封装于共享适配器且检查规则只允许该精确构造。三语言术语沿用 v3，现有翻译总数 838。

## 持久化与并发

Config 2→3 增加 `appearance.scheme`，默认 vibrant；旧 v1 先按原规则补 package 默认，再规范化到 3。保留原 revision、用户颜色／模式／其他设置，不用迁移冒充用户修改。Config 1/2 Schema 作为历史输入保留，当前写入／公开返回为 3。现有 `settings.get/update`、权限、bounded UTF-8、16 KiB 上限、revision CAS 和原子替换／恢复规则保留，不添加 method、capability、permission 或 State 表。对于严格依赖 configSchema:2 的旧客户端，此为已批准的发布前破坏性配置／响应变更，不保留旧公开结果或别名。

保存交互按所有者后续指令改为外观中的“主题”按钮打开弹窗：编辑只实时预览，点击 Save 才提交 sourceColor/mode/scheme 三字段；Cancel、Escape、关闭或离开丢弃预览、恢复已接受主题。主题编辑不后台自动保存，确认提交与普通设置共用串行 CAS 协调。主题重置也只编辑草稿，确认后才写。保存期间禁止重复提交、编辑与关闭，成功后关闭弹窗并给出持续保留的保存结果；失败保留草稿和错误，可显式重试或取消。

非主题设置按所有者最新指令修改即保存，页面总 Save／Discard 栏已移除；主题写入不覆盖其他字段或丢弃普通待处理意图。失败／冲突后的显式重试先读取当前权威，处理未知提交结果，并在三主题字段上做三方合并，本地未改字段采用其他客户端的新值。App 的连接／revision 门控和预览基线分离继续保留。最新交互验证见 [主题弹窗](m7-theme-dialog-20261030.md) 与 [普通设置即时保存](m7-settings-autosave-20261030.md)。

## 验证

最终 `cargo test --workspace --no-fail-fast -- --test-threads=1` 退出码 **0**：**432 passed / 0 failed / 8 ignored**，76 组结果。8 项均为既有 kill/restart 父测试调用的 subprocess 入口，未增加跳过。真实 Config 1/2→3、九枚举 RPC 往返、restart／原子恢复／严格拒绝及完整历史业务回归通过。

原官网自动保存交互检查点的浏览器组合 **28/28**，退出码 **0**；该历史结果不替代后续 Save／Cancel 弹窗的独立验证。包括 18 项主题用例、现有设置无障碍名称与多语言回归。`npm run check/build`、最终全 Workspace Clippy、fmt、xtask、metadata、baseline freeze、Schema／合同 JSON 解析与 diff check 均通过。只新增已批准的一个前端调色库，Cargo.lock 无变动，保留现有大 chunk 警告。

保留中间失败证据：Clippy 首轮新 schema 匹配出现 manual_range_patterns，改为同义范围且仍按 -D warnings；fmt 修正新增长断言排版。完整 Workspace 初轮唯一失败为旧源码断言要求已删除的 sourceColorName，更新为共享 resolveThemeSourceColor 正向与旧函数不存在反向断言，再完整重跑通过，不恢复三色映射。初轮仅底层主题 9/9，之后完整交互专项 15/16（HEX Material 计数器改变辅助描述，删除不必要 maxlength，保留原断言）；并发专项 18/19（新增测试对 Shadow DOM slotted 弹窗按钮作用域定位错误，按既有 host 定位修正，保留取消队列业务断言）。原日志均保留，不拼接为同轮全通过。

新增浏览器检查不调用生产 generator 作为唯一 oracle：使用固定官方关键色、全语义角色存在／关系、静态与动态等值、普通文字对比度、真实事件和请求计数，覆盖 18 种 scheme/mode 组合、自定义／小写 HEX、HCT 边界、重置、自动保存与失败、冲突合并、晚响应、延迟 shell 读取、延迟导航期间队列取消、其他设置草稿和三语言标签。真实持久化／重启由 Config/RPC 测试证明，不用 fixture 的假 storage 冒充后端证据。

证据统一位于 `target/m7-theme-colors-20261030/`。本轮无新的真实桌面颜色选择对话框操作或屏幕阅读器证据；仅恢复隔离 dev 供所有者人工检查，不提升 Visual Gate 2。窗口默认已按所有者设置为 1400×800；窗口记忆开关及持久化尚未实施，不属于本次主题审批。


最终 Vite 产物相对主题变更前：JS minified 891.55→1011.33 kB（gzip 222.50→249.24 kB，增加 26.74 kB）；CSS 65.64→78.02 kB（gzip 11.62→12.69 kB，增加 1.07 kB）。这些是实际构建大小，包含完整九方案库／控件／保存与并发处理，不用 npm unpacked size 代替产物；既有大 chunk 告警保留。


## 主题检查点隔离 dev（历史构建）

当前 Hub 打开增量的匹配构建与隔离进程见 [Hub 验收记录](m7-unity-hub-location-20261031.md)。以下仅保留主题检查点的构建摘要，不作为当前进程清单。

完整测试结束后恢复原获准 `target/m7-icon-acceptance-20260906/data`；仅暂停与恢复已核实属于本轮的进程，未关闭 Unity／v3 或未知进程。daemon PID 48368，dev helper PID 2560；构建为 dev（unoptimized + debuginfo），不是 release。源 HEAD 仍为 `7d938cf` 加本地未提交 WIP；前端来自工作区 Vite 1420，窗口默认 1400×800。

真实 daemon 只读状态 ready；验证工具只读比较启动前后隔离 settings.toml，确认 schema 2→3、scheme=vibrant、revision 1→1、其余全部用户偏好逐项相等，source_color 仍省略，未写入用户颜色。文件升级由 daemon 完成，验证工具没有写产品配置；结果见 `native-config-before.json`／`native-config-check.json`。CLI 项目列表仍为原两个隔离样例，没有对真实项目 Apply／迁移／复制／删除。构建摘要和窗口就绪状态见 `native-session.json`，本轮没有 Hosted CI 或 push。


Tauri dev 最终 GUI PID **29388**，窗口响应正常；daemon PID **48368**。GUI SHA-256 `CB0824035076E90F8E47DD143CCA6F1088A7E21E93CA1CF8D9890D0ED25BFCFA`，daemon `F581871718A4D0094CD7A67838DD92F5E2FAE6B26E661385A4051D1D3D3A046C`，CLI `85038835AC972ADB0F8384123832A0259DB86AD86C646B8246E2A1255AE8BD3F`。窗口就绪仅为可人工检查状态，不替代真实控件／屏幕阅读器验收。
