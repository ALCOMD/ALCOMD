# VRCLearn 组织身份统一

日期：2026-11-05（Asia/Shanghai）。合同见 A-040、ADR0001 和 M7 ExecPlan 的组织身份切片。

## 最终范围

- `alcomd.product.toml` 声明发布者 `VRCLearn`、Bundle/Linux desktop ID `com.vrclearn.alcomd`、
  Windows AUMID `vrclearn.ALCOMD` 和站点 `https://alcomd.vrclearn.com/`。
- 同步 Tauri、Cargo 作者及官网、自有版权声明、第一方扩展派生身份、Discord backend、
  Windows pipe/mutex、GUI 夹具/路由、有效规范及旧身份回退门禁。
- v4 自有 Schema 的 `$id`/`$ref` 统一新域名；57 份 Schema、6 个跨文件引用闭合校验通过。
  v3 source-lock、迁移 artifacts 和 Windows collection fixture 与 HEAD 内容一致。
- Windows IPC/第一方扩展 ID 是未发布身份的破坏性替换，不增加兼容别名。数据目录、
  RPC 形状、State18、Config10、权限和依赖不变。更新时须成套停旧换新，禁止同一数据目录双 Core。
- Windows AUMID 本轮仅修改声明，GUI 显式应用仍待接入；Inno AppId 仍待 M12。
  新站首页只读访问返回 ALCOMD3 页面，不据此宣称 Schema 托管或旧更新地址衔接已验证。

## 实际验证

- Python metadata、`cargo xtask check` 通过；受影响 xtask/protocol/platform 的 fmt 通过。
- protocol `schema_contract`：42 passed / 0 failed；platform lib：50 passed / 0 failed。
- xtask/platform/gui 的 all-targets 严格 Clippy 通过，涵盖 Tauri 新配置的编译检查。
- 根 `npm run check` / `npm run build` 通过，三语 1067 条词条；生产构建保留既有 chunk size 提示。
- 首次 `cargo fmt --all -- --check` 失败，包含本轮两处格式和另一聊天正在编辑的
  `crates/alcomd-application/src/m5.rs`。本轮两处已修正，未修改并行文件或将全仓格式记为通过。
- 浏览器独立定向回归：15 passed / 3 failed，零重试。扩展卡片/安装/打开/权限/卸载 4 项通过；
  Portable UI 10 项通过、1 项旧语言会话身份读取失败；共享 Select 三语切换通过。
  两条路由本地化用例已走过新扩展 ID 的断言，后在任务页失败：夹具返回空列表、页面显示
  “暂无任务”，旧用例仍寻找状态列头。未降低断言、修改业务实现或把此轮称为完整通过。
  证据位于 `target/organization-identity-browser-results/`（各失败的 screenshot/trace/error-context 保留）。
- H4 失败的读取顺序已只读定位：App 打开会话后的身份请求被旧夹具挂起；切换语言后，
  CoreEvents 复用同连接/方法/参数的在途读，新会话前置身份读取因此仍在等待。
  旧用例先等待第二会话显示、再释放旧请求，形成等待循环。`hold(false)` 只解除未来请求的挂起。
  本轮仅换身份字符串，没有修改这条共享读取链路或扩大范围调整其测试。

- Windows IPC 所有权与真实握手回归：启用既有 `test-kill-gates` 后，`test_support::tests`
  4 passed / 0 failed。覆盖重复 Core 在打开数据前拒绝、初始化失败不发布端点、实际 hello/status，
  以及退出/失败后由测试 handle 保持端点所有权；仅使用隔离 store。初次未启用该 feature 时
  运行 0 项并过滤 28 项，未将其计为回归通过。
- 独立 Discord backend 编译及 doc-test 入口成功；该占位 crate 无测试，实际 0 项，未称作功能测试。
- 最终差异检查通过，所有 Schema 文件相对 HEAD 仅有已批准域名替换；三份依赖锁文件无变更。

## 交付边界

本轮没有推送、修改远端站点、创建安装器或部署 release 程序，没有操作真实项目/Unity/用户数据。
验证期间存在另一聊天的 Unity writer 判断改动，未纳入本轮实施或验收。
GUI 的现有开发监视进程可因源码变化自行重编；本轮没有主动停止、启动或替换用户桌面进程。
