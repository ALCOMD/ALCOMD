# 客户端程序发现与连接管理

本分域实现核定的 A1–A4、B1–B3。安装程序用 `alcomd_platform::client_installation::publish/unpublish` 维护独立记录；Core 在握手和查询时得出当前程序、实例与连接。正式安装器按 M12 使用相同格式。已连接的 AI 登记与授权由独立的 `mcp.clients.*` 和 OAuth 用例处理。

## 安装与连接

- 每份 [安装记录](../../specs/formats/client-installation-v1.schema.json) 位于用户或系统产品目录的 `clients.d/<programId>.json`。安装器在最终可执行文件发布后原子替换记录，卸载时核对同一程序、安装位置与版本。Core 的预检允许安装器在数据库首次创建前登记程序。逐条有界读取，单份损坏通过 `coverageIncomplete` 披露。
- Windows 读取真实管道客户端 PID、内核进程创建 FILETIME 和文件标识；Linux 读取 socket peer credential、procfs 进程起始 tick 及已加载映像设备/inode；macOS 读取 socket peer credential、Darwin 进程创建时间和可执行文件标识。无法核实的连接仍然可用，以临时行表示。产品清单派生的官方程序名还须匹配受保护布局；身份标签不影响 RPC 权限。
- Rust 与 Node 的诊断握手值不会覆盖 Core 从系统获得的归属。Node SDK 子进程仅在实际父进程与已登记程序匹配时归并到该 GUI；设置页明确显示每条连接的自报资料。Core 以进程和创建标记聚合同一程序的连接，实时 key 在实例结束后失效。程序列表的已安装项由文件保留。
- 三个[原生方法](../../specs/rpc/client-registry-v1.schema.json) 共用 `clients.registry.v1`。分页结果保存有界且短期的快照，可在独立 CLI 进程续页。调用人、查询种类和目标程序绑定到游标。断开指定连接/实例按权威 ID、Core 实例 ID 与幂等键执行。已接受的写操作继续完成。关闭终止订阅并按原连接清理预览、扩展 UI 和各传输观察。后续重新连接生成新连接 ID。

## 依赖与维护

平台层直接使用 workspace 已锁定的 `serde` 解析严格的 v1 安装记录。保留 `serde_json::Value` 并手工解析需要再实现字段完整性与类型检查。项目已统一使用 Serde 派生，维护风险随既有 workspace 版本。

macOS 使用已锁定的 `libc 0.2.189` 调用 `proc_pidinfo(PROC_PIDTBSDINFO)`，取得微秒级创建时间并校验内核返回的 PID 与完整结构长度。此依赖已由现有 macOS 运行时引入，许可证为 MIT 或 Apache-2.0。直接使用绑定免去复制 Darwin 的 C ABI 结构，也避免 `sysinfo.start_time()` 秒级时间戳造成 PID 重用歧义。ABI 验证依赖 macOS 构建与真实 socket 用例。Windows 复用已有 `windows-sys` 与文件 ACL 检查；各平台均复用 `sysinfo` 的定向进程观察，不为客户端发现枚举全部进程。

## 验收入口

桌面动作在 Core 所在机器执行。平台层使用工作区已锁定的 `open 5.4.2` 调用默认文件管理器和浏览器，继续采用 GUI 原有的系统默认处理程序。逐系统手工拼装命令会重复该依赖现有的转义与分派实现。路径来自已登记项目，下载页是固定的 Unity 官方地址，调用者无法指定命令或 URL。维护时须复验无桌面会话、缺少处理程序、系统拒绝以及平台升级后的启动结果。

独立 Rust 客户端通过 Named Pipe 或 Unix socket 启动隔离 Core，覆盖从未启动的 GUI、同名自报、单条连接断开、重放和重新连接。[真实 Core 测试](../../crates/alcomd-testing/tests/m7_official_rpc.rs)

设置 → 客户端提供程序列表与实例/连接详情；CLI 使用 `client list`、`client instances`、`client disconnect`，MCP 对应 `alcomd_list_clients`、`alcomd_list_client_instances`、`alcomd_disconnect_client`。客户端只传显式目标，Core 负责权威列表、身份和断开结果。[GUI 浏览器回归](../../apps/alcomd-gui/tests/browser/native-clients.spec.ts) · [MCP 工具定义](../../specs/mcp/client-registry-v1.schema.json)
