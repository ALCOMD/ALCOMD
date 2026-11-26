# 项目状态

最后更新：2026-11-26

## 当前阶段

**M12 阶段 0 已批准，正在实施产品安装和更新。** 永久 Windows AppId、安装和登记布局、v4 更新声明及卸载归属已冻结。`cargo xtask dist` 已接通运行组件构建、清单验签与 Windows、Linux 产品打包输入。首装、整产品更新和客户端生命周期分项验收。macOS 原生验收按所有者决定延期，公开发版仍按 A-017 的三平台四格式条件审批。[M12 实施计划与审批矩阵](exec-plans/M12-full-product-distribution.md) · [阶段 0 冻结合同](exec-plans/M12-stage-0-release-contract.md)

Windows 11 Sandbox 的当前用户和全用户夹具已验证 Core 安装登记、外部 CLI 冷启动、账号隔离和卸载保留状态。Ubuntu 22.04 x86_64 隔离环境已原生编译七个发行组件，产物最高要求 `GLIBC_2.34`。[Windows 登记及 Ubuntu 构建记录](exec-plans/M12-full-product-distribution.md#2026-11-25-windows-core-安装登记)

Windows 已在两套隔离环境用 `25afa078` 的同一个全 Release Inno 夹具通过当前用户和全用户安装。验收覆盖 CLI 和 GUI 首启、签名篡改拒绝、同账号四个 CLI 接入同一 Core、注销登记以及卸载保留资料。[Windows 双范围验收](exec-plans/M12-full-product-distribution.md#2026-11-25-windows-全用户安装首次启动)

Windows 11 的产品 Inno 已使用同一个 `e79db74f` 离线签名载荷验收永久 AppId 和两种安装范围。当前用户卸载会拒绝正在使用的 Core 和篡改清单，正常卸载注销该账号并保留状态库。全用户安装按账号初始化，卸载后保留各账号资料。[Windows 产品 Inno 双范围](exec-plans/M12-full-product-distribution.md#2026-11-25-windows-产品-inno-双范围)

Windows 双范围 PATH 与开始菜单快捷方式已用主仓候选 `62b6f562` 的完整 Inno 实包通过 11 项原生验收。覆盖 AUMID、第三方入口保全、精确卸载、`/NOICONS`、稳定 CLI 冷启动、Core 登记和 Updater 入口。跨范围转换、同范围更新和其他系统入口继续按实施计划验收。[Windows 产品 Inno 双范围](exec-plans/M12-full-product-distribution.md#2026-11-25-windows-产品-inno-双范围)

Windows 签名候选 `72a85893` 的同一个 Inno 实包通过两种安装范围的 12 项原生检查。全用户卸载会拒绝任一仍使用 Core 的账号，已经持有维护锁时也拒绝旧 Core 首启。正常卸载保留两个普通账号的资料与登记。当前用户卸载只注销该账号登记，保留资料和未知文件。[Windows 跨账号卸载](exec-plans/M12-full-product-distribution.md#2026-11-25-windows-跨账号卸载)

Ubuntu 22.04 用签名候选 `25afa078` 复验了原生 AppImage 挂载、DEB 安装和卸载、Tauri 窗口及 Core IPC。[Ubuntu 同候选原生夹具](exec-plans/M12-full-product-distribution.md#2026-11-25-ubuntu-2204-同候选原生夹具)

Ubuntu 22.04 的产品 DEB 已通过原生首装、解包中断恢复、逐账号升级和同数据版本下的 APT 回退。两套签名测试包验证了 CLI、真实 GUI、外部 SDK、坏签名与不安全登记拒绝、双账号设置保全及 purge 后数据库摘要一致。[Ubuntu 产品 DEB 升级与回退](exec-plans/M12-full-product-distribution.md#ubuntu-2204-deb-跨版本登记与回退)

Ubuntu 22.04 的产品 AppImage 使用签名源码 `b324627e` 与隔离测试密钥，验证原生 FUSE、真实桌面首启、独立 SDK 冷启动、移位拒绝与主动重新绑定。可选 CLI 集成继续按发行矩阵验收。[Ubuntu 产品 AppImage 首启](exec-plans/M12-full-product-distribution.md#2026-11-25-ubuntu-产品-appimage-首启)

Ubuntu 22.04 的 AppImage 整包更新与中断恢复已通过 10 项原生工程检查。验收覆盖同路径原子替换、实际进程终止、篡改拒绝、删除原下载后的恢复，以及新版 AppImage 经原生 FUSE 打开并读回原工程。后续干净 Release 的外部 Rust SDK 冷启动单次通过，用时 817 毫秒。[AppImage 整包更新](exec-plans/M12-full-product-distribution.md#2026-11-26-ubuntu-产品-appimage-整包更新) · [完整 Linux 发行候选](exec-plans/M12-full-product-distribution.md#2026-11-26-ubuntu-完整-appimage-与-deb-原生验收)

AppImage 可选 CLI 集成已用完整 Release 候选在真实 WebKit「关于」页验收启用、修复、取消更新、确认替换重启及移除。DEB 同源候选通过离线 APT 首装、双账号升级、dpkg 中断恢复、同数据版本回退、移除和 purge，两账号资料保全。外部 Rust SDK 的两次首次冷启动为 870 和 1076 毫秒。[完整 Linux 发行候选](exec-plans/M12-full-product-distribution.md#2026-11-26-ubuntu-完整-appimage-与-deb-原生验收)

新建的 Ubuntu 22.04 x86_64 WSL 2 实例已复验 AppImage 首启。用此前的签名 DEB 另通过三项双格式归属检查，覆盖双向接管拒绝与 `dpkg purge` 后资料保全。[Ubuntu 双格式归属](exec-plans/M12-full-product-distribution.md#2026-11-25-ubuntu-产品-appimage-首启)

v4 更新声明、HTTPS 下载、有界暂存和持久防重放已接入。Windows 11 Sandbox 通过 9 项 HTTPS 验收，覆盖下载中断、四进程竞争与回执保全。另通过 6 项签名响应接力检查，覆盖原始声明与资产的离线复验。平台安装提交和回滚继续按阶段 3 实施。[更新声明、下载与暂存核验](exec-plans/M12-full-product-distribution.md#v4-更新声明离线核验)

Windows 当前用户的更新暂存与只读数据预检通过 9 项原生检查。完整签名源码 `f75808a9` 的 Inno 源版本包验证了安装维护锁、内外摘要、数据预检、中断后重试以及原程序、登记和数据库保全。全用户范围还复验了同一实包的六项卸载互斥。[Windows 当前用户更新预检](exec-plans/M12-full-product-distribution.md#2026-11-25-windows-当前用户更新预检)

Windows 当前用户的版本切换和提交恢复已通过原生验收。完整 Release 源版 `b95e132f` 和版本派生 `6a79c90e` 通过 8 项正常升级和 11 项中断恢复检查，覆盖实际终止、签名篡改、登记占用、新版 WebView2 窗口、原工程重开和卸载资料保全。[Windows 当前用户版本切换与恢复](exec-plans/M12-full-product-distribution.md#2026-11-25-windows-当前用户版本切换与恢复)

Windows 全用户切换使用签名源码 `d1555aa3` 的断网夹具通过 6 项正常升级和 8 项中断恢复检查。修复源码 `20a9d8f7` 另通过 6 项不同 SID 的传输及升级检查和 8 项实际终止与恢复复验。两个账号在一次连接内分别接入新版 Core 并保留资料。干净 Release 的首次冷启动和交互凭据提升另验。[Windows 全用户版本切换与中断恢复](exec-plans/M12-full-product-distribution.md#2026-11-26-windows-全用户版本切换与中断恢复)

Windows 同范围缺失文件修复已通过两种 Win11 安装范围验收。同一 `310065bf` 完整 Inno 实包在两种范围各通过 8 项自定义目录检查，覆盖 Unicode 与空格目录、缺失卸载登记后的修复、Core 冷启动和卸载归属。Win10、双向范围转换和自定义目录整包更新继续验收。[Windows 修复与转换维护锁](exec-plans/M12-full-product-distribution.md#2026-11-26-windows-同范围修复与转换维护锁)

Windows Inno 已接入 WebView2 作用域检测、在线 Bootstrapper 和可选离线 Standalone。隔离 Win11 验证了已有运行时、坏摘要和提升范围保护。两种工程包已编译，离线包超过冻结 v3 更新器的 256 MiB 上限。干净 Win10 的实际部署另验。[WebView2 前置部署](exec-plans/M12-full-product-distribution.md#2026-11-26-windows-webview2-前置部署)

Windows 和 AppImage 的「关于」页已接入产品更新。三项 Chromium 用例覆盖用户确认前保留当前进程、取消下载和恢复确认。Windows 签名夹具另通过 8 项真实 WebView2 检查，覆盖仅检查声明、下载保留原版、取消确认、自动安装重启及新版 Core 读回原工程。Windows 正式包签名前须核对两只 Microsoft x64 CRT 的固定摘要。[应用内产品更新](exec-plans/M12-full-product-distribution.md#2026-11-26-应用内产品更新)

Windows 签名源码 `12057897` 的完整 Inno 与临时迁移载荷通过合成签名和摘要配对。Win11 Sandbox 的两种安装范围共通过 8 项原生检查，覆盖惰性暂存、重试、未知目录保全及缺少有效迁移日志时拒绝启用。完整冻结版更新交接继续验收。[Windows 桥接程序双范围暂存](exec-plans/M12-full-product-distribution.md#2026-11-26-windows-桥接程序双范围暂存)

Windows 桥接启用现在可以复用原迁移日志恢复。Win11 隔离 Inno 夹具在两种范围各实际终止两次进程，复验原生卸载登记、活动版本、Shell 入口和无关文件保全，共 8 项通过。同源完整实包再通过双范围暂存的 8 项检查。原账号的数据提交和启用仍待冻结版交接验收。[安装程序中断恢复](exec-plans/M12-full-product-distribution.md#安装程序中断恢复)

冻结 Windows 更新器的外层 EXE 和出包入口已实现。全用户桥接由每个账号迁移自己的资料，确认无旧资料的账号也可独立初始化。Win11 安装脚本覆盖机器首启门禁的发布时序及卸载，真实双账号交互和冻结更新器外层验签仍待验收。[冻结 Windows 更新器入口](exec-plans/M12-full-product-distribution.md#冻结-windows-更新器入口与逐账号首启)

Windows 签名源码 `b605e19b` 的完整 Inno 与配套迁移载荷已在断网 Win11 标准账号通过冻结 v3.4.0 实际首启、惰性暂存、迁移副本上的项目读写、提交前回滚及 v3 桌面重开。全用户实包验证尚未迁移的标准账号无法登记 Core 或创建数据。同包两种安装范围均通过 Core RPC、CLI、WebView2、MCP 和第一方扩展的原生健康检查。[冻结 Windows 更新器入口](exec-plans/M12-full-product-distribution.md#冻结-windows-更新器入口与逐账号首启)

同一 Windows 外层桥接包在 Win11 标准账号通过取消保全和从预检到 v4 激活的提交，并独立通过原 v3 公钥的离线分离签名验证。冻结 v3.4.0 更新器已通过当前用户及全用户 beta 的实际下载、启动与取消，并拒绝同名旧资产的签名。全用户原账号的交互 UAC 和迁移提交继续验收。[外层桥接验收](exec-plans/M12-full-product-distribution.md#冻结-windows-更新器入口与逐账号首启)

Linux 同源候选 `310065bf` 的 AppImage 和 DEB 在 Ubuntu 22.04 均通过原生桌面、Core、CLI、MCP 和第一方扩展健康检查。Ubuntu 24.04 已走真实 APT 交接，从冻结 v3.4.0 DEB 完成逐账号回滚和提交，验证旧资料保全与新版读回。AppImage 桥接已通过原生混合夹具，完全同源的发布归档和冻结桌面交接继续验收。[Linux 冻结版桥接](exec-plans/M12-full-product-distribution.md#2026-11-26-linux-冻结版桥接入口)

M12 首批工程候选 `5d58b43b` 已快进到远端 `main`。本机、Windows Hosted 和 Ubuntu Hosted 的整仓检查均通过，三处 Chromium 各通过 762 项，四项 CodeQL 也通过。macOS Hosted 重跑取得运行器，整仓检查中 5 个打包夹具失败，后续 Chromium 与构建步骤未执行。macOS 原生验收继续延期，正式发行继续按 A-017 放行。[首批工程候选门禁](exec-plans/M12-full-product-distribution.md#2026-11-25-首批工程候选门禁)

主仓工程候选 `595fc2ab` 已通过本机整仓、762 项 Chromium、三平台 Hosted CI 和 CodeQL。Windows 原生 Inno 入口的 11 项验收对应上文单独记录的 `62b6f562` 实包。[Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/45552186695) · [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/45552185931)

跨账号卸载和离线更新声明候选 `81e7a498` 已通过本机整仓、762 项 Chromium、Windows 与 Ubuntu Hosted 以及 CodeQL。五笔提交逐笔验签后快进至主仓 `main`。该次 macOS Hosted 的 AI 客户端撤销用例在重开确认窗口时超时，等待关闭完成的修正已通过 Windows 定向复验。[Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/45601387153) · [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/45601387132)

更新预检候选 `f75808a9` 已通过本机整仓、762 项 Chromium、三平台 Hosted CI 和四项 CodeQL。三笔提交逐笔验签后快进至主仓 `main`。Windows、Ubuntu 和 macOS Hosted 作业分别核对了 SDK 消费者、构建与 Chromium。[Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/45606585593) · [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/45606585513)

当前用户版本切换候选 `b95e132f` 已通过本机整仓、762 项 Chromium、Windows／Ubuntu／macOS Hosted CI 和四项 CodeQL。五笔提交逐笔验签后普通快进至主仓 `main`。三平台 Hosted 均通过外部 SDK 消费者、Release 构建和 Chromium。[Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/45641567398) · [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/45641566264)

全用户更新、应用内更新和 AppImage 整包更新的签名候选 `73b1a108` 已快进至主仓 `main`。本机整仓、三平台 Hosted 和四项 CodeQL 均通过。本机及每个平台的 Chromium 各通过 765 项。Ubuntu Hosted 首轮发现的迁移日志锁问题已修复，并在隔离 Jammy 连续通过 20 轮并行复验。[实施计划](exec-plans/M12-full-product-distribution.md#2026-11-26-桌面更新候选门禁) · [Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/45696134018) · [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/45696132490)

隔离发行演练使用生产 Updater 和既有完整签名 Inno 包，在断网 Win11 通过九项回环 HTTPS 检查，覆盖下载、频道隔离、停推、恢复和篡改拒绝。最终发行资产按同一脚本复验。[隔离发行演练](exec-plans/M12-full-product-distribution.md#2026-11-26-隔离发行停推与恢复演练)

M11 已实施[三端调用、客户端管理与扩展入口](exec-plans/M11-client-parity.md)，从当前生产注册和[三端操作矩阵](exec-plans/M11-client-operation-matrix.md)核对用户入口。Core 已提供原生客户端发现和连接登记，GUI 从设置页「客户端」管理连接。扩展可分别声明 GUI、CLI 和 MCP 入口，启用后由对应客户端发现，执行时由 Core 核对业务权限。

Unity `6000.0.67f1` 的两个内置模板和八组迁移已完成 Windows 开发验收。三平台门禁见 [PR #13](https://github.com/ALCOMD/ALCOMD/pull/13)。[Unity 6 ExecPlan](exec-plans/unity-6-first-release.md)

项目登记、创建和复制预演以及仓库检查、登记和移除已共用原生方法。仓库、设置、事件和 Unity 分域候选的[首发清单](../specs/first-release-contracts.json)涵盖 127 个 RPC、44 项能力和 103 个内建 MCP 工具，原生 SDK 的 116 个方法见[方法目录](../specs/rpc/native-sdk-v1.methods.json)。手动创建的模板从选定的内置或用户模板继承内容，成品独立使用。v3 派生模板迁移只生成 `2022.3.22f1` 的版本。[首发合同及验收范围](exec-plans/M11-first-release-contracts.md)

**M11 的 Windows 和 Linux 已验收。** 普通 ZIP、Core 主机绝对路径、State1 和 Config1 已通过 Windows 隔离验证。冻结 v3.4.0 的当前用户安装和全用户安装均已实测，全用户安装下两个账号分别导入并重开对账。Windows 当前用户的冻结更新器已在隔离环境验证 stable、beta 同频道跳转、旧钥签名 beta 下载与交接、篡改拒绝和回滚后旧版重启。独立 NTFS 卷验证了原生导入时实际空间耗尽后的回滚和重试。Ubuntu 24.04 隔离环境中的冻结 DEB 和 AppImage 均已通过默认资料、项目、仓库、模板及合成 Unity 登记与默认参数的 Linux 原生导入和重开。Windows 和 Linux 均已验证 v3 生成的 ZIP 恢复与同键重放，普通测试用户也分别通过了真实系统凭据库的合成凭据往返。[M11 实施计划](exec-plans/M11-v3-migration-and-zip.md)

Windows 当前用户和全用户安装均已通过冻结更新器的 Stable 同频道下载、原 v3 密钥验签、范围传递、Bootstrap 暂存回滚和旧版重启。Linux 冻结 DEB 已由 dpkg 实际配置，冻结 AppImage 已从 FUSE 挂载运行。两种 Linux 范围均通过运行中拒绝迁移、临时签名载荷的原生暂存对账、回滚和旧版重启。[Windows 和 Linux 补充验收](exec-plans/M11-v3-migration-and-zip.md#2026-11-24-windows-stable-双范围签名交接)

Windows v3.3.0 已实测升级到冻结 v3.4.0，再以同一隔离账号完成签名 Stable 桥接的暂存与回滚。选择性模板导入、外部 MCP 配置补丁及回滚通过本地夹具验收。[补充证据](exec-plans/M11-v3-migration-and-zip.md#2026-11-24-windows-v330-经冻结-v340-进入桥接)

Windows 当前用户和全用户安装、Linux AppImage 和 DEB 均已通过隔离原生数据提交、篡改证明拒绝和同事务恢复。Windows 两种安装范围也通过配对旧版实例清理、第三方关联及未知文件保全、提交后 Core 重开。真实 v3 ZIP 已在 Windows 通过实际 GUI、CLI 和认证 MCP 从配置目录外恢复。官方 VCC 2.4.5 与旧 ALCOM 1.1.8 生成的默认资料均已通过选择性导入。[隔离提交记录](exec-plans/M11-v3-migration-and-zip.md#2026-11-24-windows-双范围原生数据提交)

专用 `ALCOM` 目录和 ALCOMD3 2.1.0 Beta 已按冻结源码构造自制样本，经真实 Core 补验选择性导入。Windows 双范围与 Linux AppImage、DEB 均补验了三处提交相关检查点的系统权限拒绝和签名失败，并在健康检查点复现实际磁盘满后用原事务重试。Windows 和 Linux 的日志自动化覆盖全部七个正向检查点。[样本来源和故障回滚记录](exec-plans/M11-v3-migration-and-zip.md#2026-11-24-专用-alcom-与-alcomd3-210-beta-自制样本)

Windows 当前用户、全用户与 Linux DEB、AppImage 已用真实 v3 工程的只读副本，核对两代桌面的工程行、迁移状态及冻结版 ZIP 恢复。各范围核对 6,232 个恢复文件，原工程通过前后摘要复核。Windows 全用户和 Linux 两种范围还通过两代桌面核对了包管理读回和编辑器启动参数。Linux DEB 另外通过真实桌面的合成包卸载预览、确认与写入。[真实工程对照](exec-plans/M11-v3-migration-and-zip.md#2026-11-24-windows-真实-v3-工程对照)

Windows 双范围和 Linux 两种范围已实测前四个迁移检查点的系统权限拒绝和前三个检查点的进程终止，验证回滚、原事务恢复、原件保全和新事务成功提交。[早期故障验收](exec-plans/M11-v3-migration-and-zip.md#2026-11-24-早期迁移检查点系统故障)

四种安装范围也已通过后四个检查点的进程终止恢复。Windows 从独立 Core 启动真实 Unity `2022.3.22f1` 打开工程副本并正常退出。Linux AppImage 和 DEB 的冻结 v3 和 v4 桌面均已核对包安装和升级。Windows 在同一隔离工程用冻结 v3 的认证 MCP 和 v4 原生 CLI 验证安装、升级和移除。[后四个检查点和真实工程操作](exec-plans/M11-v3-migration-and-zip.md#2026-11-24-后四个检查点和真实工程操作)

Windows 双范围及 Linux AppImage 和 DEB 的七个检查点共通过 28 项实际磁盘耗尽测试，验证提交前回滚和提交意图后的同事务恢复。[实际磁盘耗尽验收](exec-plans/M11-v3-migration-and-zip.md#2026-11-24-windows-和-linux-七个检查点实际磁盘耗尽)

签名候选 `fb6b0455` 已通过本机整仓、762 项 Chromium、Windows 与 Ubuntu Hosted CI 及四项 CodeQL，两个 Hosted 平台的 Chromium 各通过 762 项。该 SHA 的 macOS Hosted 仍待验收，两轮均因运行器容量不足在开跑前取消。所有者因当前没有 Apple Silicon 环境延期 macOS 原生验收。正式部署归 M12。[候选门禁与原生验收缺口](exec-plans/M11-v3-migration-and-zip.md#2026-11-24-签名候选门禁)

软件包默认显示预发布版，安装和升级优先兼容稳定版。CLI 和 MCP 安装、升级均可省略版本。Windows 已通过真实 Core 的候选与设置重启测试、MCP 安装升级、27 项 VPM 回归和 74 项软件包浏览器测试。[配置规范](../specs/config/settings-v1.md)

M11 仓库调序让列表顺序决定同版本包的仓库来源优先级。仓库移动、专用格式 v1 与验证记录见[仓库顺序与来源优先级](exec-plans/M11-repository-order.md)。

Unity Hub 使用系统登记发现和打开，Editor 优先读取官方安装清单，再通过系统登记补齐。Windows 已通过真实 MSIX 启动请求和隔离 Core 的 Editor 发现验证。macOS 和 Linux 已通过交叉编译，真实桌面流程待验收。[实施与验收记录](exec-plans/unity-application-discovery.md)

**M10 已完成。** 签名候选 `dab76fcd` 的三平台 Hosted CI 和四项 CodeQL 均通过，Chromium 每平台 738 项通过。Rust 与 TypeScript SDK 共覆盖 94 个公开方法，含官方 GUI 的全部 93 个 Core 方法。三个平台的 npm 开发包已完成外部安装和真实 IPC 验收。[M10 验收记录](exec-plans/M10-native-sdk-and-rpc.md#2026-11-20-验收结果)

**M9 已完成。** 第一方 Discord 扩展已通过公开 Extension API 接通 Core 和 Host，并沿用 v3 的 Discord 应用身份。主仓候选 `feca8df3` 的三平台 Hosted CI 和四语言 CodeQL 全部通过，Chromium 各 738 项通过。独立扩展候选 `0ff04bc` 已通过三平台构建及 Actions、Rust 和 Python CodeQL。Windows 已完成真实 Unity 与 Discord 的 IPC 验收，Ubuntu 和 macOS 的真实桌面验收按计划延期。[M9 收尾记录](exec-plans/M9-discord-first-party-extension.md#2026-11-20-m9-收尾)

M8 冻结候选为 `622cb62a`，Windows、Ubuntu 和 CodeQL 已通过。macOS 未获运行器，按所有者决定延期补验。其他遗留验收项见 [M8 收尾记录](exec-plans/M8-mcp-protocol-and-management.md#2026-11-20-m8-收尾)。

## 按任务查合同

| 任务 | 实施合同 |
| --- | --- |
| GUI、仓库、模板、操作日志、共享组件、可访问性、Portable UI | [M7 ExecPlan 任务索引](exec-plans/M7-official-gui-extension-ui.md) |
| 项目动作、包管理 | [项目与包功能收口](exec-plans/M7-project-package-functional-closure.md)、[包候选合同](exec-plans/M7-package-candidate-evidence-stop-a.md)、[VPM 事务](exec-plans/M4-package-transactions.md) |
| Unity 项目版本迁移 | [Unity 版本迁移合同](exec-plans/M7-unity-project-version-stop-a.md) |
| GUI 与公开 RPC 用例对照 | [M7 GUI RPC 覆盖](exec-plans/M7-gui-rpc-coverage.md) |
| Discord 第一方扩展（M9 已完成） | [M9 实施顺序、公共合同与验收记录](exec-plans/M9-discord-first-party-extension.md) |
| 正式安装、完整产品更新、卸载与发行 | [M12 实施计划和审批边界](exec-plans/M12-full-product-distribution.md)、[发行测试矩阵](testing/release-matrix.md) |

## 剩余边界与下一停止点

- H7沿用所有者整体认可及条件收尾授权；新增修正按实际门禁和运行交付验收，不以浏览器fixture冒充原生人工操作。Portable UI原生表单/超时/外部断线等旧未覆盖项保持，见[桌面验收清单](testing/m7-official-gui-manual-checklist.md)。
- 最终功能证据绑定`c46f7544`；此前候选的失败、分轮结果和历史三平台基线分别保留，不替代本轮完整通过。收尾文档为后续独立提交，不声称其新CI已完成。
- 14项原有`#[ignore]`未全部直接单跑；子进程入口部分由父回归调用，联网VRCFury、Windows单实例/注册表、显式公开仓库快照及专用HTTP/代理隔离项不计作本轮直接通过。
- v3 真实工程差分余项仍属 M11。原生安装、更新、卸载和完整发行验收仍属 M12。后续里程碑按各自授权推进。
- M7 已正式完成，M8 已按所有者要求收尾，M9 和 M10 已验收完成。Discord 启用状态迁移和随包装配分别归入 M11 和 M12。原生 Core 安装登记由 M12 安装器维护。[M10 验收记录](exec-plans/M10-native-sdk-and-rpc.md#2026-11-20-验收结果)

[M7 状态与验收历史](testing/m7-status-history-through-20261107.md) 保存原状态页的候选、签收、遗留事项、失败轮次及证据链接。
