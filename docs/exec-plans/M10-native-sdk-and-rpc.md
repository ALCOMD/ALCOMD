# M10 原生 GUI 接入与 SDK

状态为已完成。所有者选择同一系统用户直接使用，并要求官方与第三方桌面 GUI 的完整 Core 业务等价。当前阶段见[项目状态](../status.md)。

## 合同

- 原生桌面客户端通过系统账号限定的 RPC v1 端点使用该账号的本机所有者权限。Core 以同一套用例处理官方和第三方请求。商品名、路径、声明的实例 ID 或 capability 不提供额外授权。同账号的应用之间没有独立撤销或隔离。MCP 凭据与扩展权限仍按各自合同执行。[信任模型](../../specs/security/local-client-threat-model.md)
- SDK 以当前官方 GUI 使用的 Core 方法为最低覆盖范围。目录记录 93 个官方 GUI 方法及事件回放的读、写或流、能力、Rust 函数和 Schema。窗口、选取文件及拖放由各应用处理。SDK helper 的 stdio 通道仅用于父进程及其子进程。[方法目录](../../specs/rpc/native-sdk-v1.methods.json)
- RPC v1 的帧、方法、业务校验、revision、Plan/Apply 和幂等语义继续有效。握手可选的存储与配置格式版本接受正 u32。保持旧响应兼容并拒绝不完整或含歧义的响应。该调整兼容有效的既有 RPC v1 消费者。
- 官方和独立 SDK 共享平台端点验证与按安装登记冷启动的实现。安装登记为 M10/M12 的本机启动接口。登记由完成初始化的安装器写入，M10 通过隔离安装暂存验收。

### 安装登记 v1

- 原位存于当前账号平台数据目录的 `core-installation-v1`，与 `state.db` 同目录。内容恰为一行 UTF-8 绝对规范化运行目录，路径最多 4096 字节，以 LF 结束。路径只描述目录，可执行文件基名从产品元数据派生。
- v1 读端不创建父目录。已运行的端点通过检查时直接连接。安全检查失败会终止连接，登记非法或指向缺失文件也终止启动。独立 SDK 未找到登记时返回可处理错误，开发态可沿用 sibling 启动。正式安装器只能在初始化成功后原子提交登记。

## 实施顺序

1. 同步架构、权限决策和威胁模型，核对当前官方 GUI 调用与公开 Schema，固定安装登记和方法目录。
2. 实现共享平台连接与启动策略，并加固 Rust RPC 对握手、响应关联、帧和错误的校验。
3. 建立本地打包的 Node.js 24 ESM 接口与 Rust 源码消费入口。GUI 继续经 Rust SDK 调用，复用 SDK DTO。helper 包装沿用现有 Rust 依赖，正式发布和产品安装接线分别走原审批。
4. 使用隔离产品验证两个仓库外消费者、官方同业务入口、三平台冷启动、错误和恢复，再汇总整仓及远端门禁。

三平台 Hosted CI 保留实际外部消费测试所用的 npm 包，便于从同一候选下载。工作流沿用 GitHub Actions 的产物存储，固定官方 `actions/upload-artifact` 的精确提交。可手动从同一 SHA 构建归档，代价是每个平台各需一套工具链。Action 升级随现有固定版本流程审查，SDK 运行时不引入该依赖。[产物 Action 版本](https://github.com/actions/upload-artifact/releases/tag/v7.0.2)

## 验收

- Windows x86_64、Ubuntu 22.04 x86_64 和 macOS arm64 上，用独立 Node 与 Rust 消费者完成真实 IPC 握手、冷启动、已启动再连接、并发连接和空闲退出后重启。验证安装登记失效拒绝启动，拒绝不符合平台 IPC 安全边界的端点。
- 目录覆盖所有官方 GUI Core 调用。至少复验项目分页、包 Plan/Apply 与显式同键恢复、任务与事件、备份、设置和扩展管理。异常响应不得污染另一条请求或伪造成功。
- 通过整仓 Rust、仓库检查、前端检查和构建、Chromium 回归，以及同一候选 SHA 的 SDK 消费者、三平台 Hosted CI 和 CodeQL。收尾时记录实际执行结果，更新项目状态与结构化覆盖。

## 2026-11-20 验收结果

签名候选 `dab76fcd539bd72c0e32f9ef8829db1ffd14d1e5` 已普通快进推送至 `main`。同一提交的三平台 Hosted CI 和 Actions、JavaScript/TypeScript、Python、Rust 四项 CodeQL 全部通过，两个工作流均为首轮自动运行。[Hosted CI](https://github.com/ALCOMD/ALCOMD/actions/runs/44740160653) [CodeQL](https://github.com/ALCOMD/ALCOMD/actions/runs/44740160572)

| 平台 | SDK 外部消费 | 完整 Chromium | 开发包 |
| --- | --- | --- | --- |
| Windows Server 2025 x86_64 | 通过 | 738 通过，13.3 分钟 | [Windows x64](https://github.com/ALCOMD/ALCOMD/actions/runs/44740160653/artifacts/14635145220) |
| Ubuntu 22.04 x86_64 | 通过 | 738 通过，12.7 分钟 | [Linux x64](https://github.com/ALCOMD/ALCOMD/actions/runs/44740160653/artifacts/14633446370) |
| macOS 15 arm64 | 通过 | 738 通过，25.5 分钟 | [macOS arm64](https://github.com/ALCOMD/ALCOMD/actions/runs/44740160653/artifacts/14634987755) |

从干净 checkout 完成整仓 Rust、前端检查与构建、发布二进制、Tauri 无包构建及适用的系统目标检查。三平台在真实 IPC 上完成仓库外 Rust 源码消费者与离线 npm 消费者的并发冷启动、运行中直连、空闲退出与重启、缺失及失效登记拒绝、项目分页、包 Plan/Apply 和丢回包后同键恢复、任务与事件、备份、设置和扩展管理。Windows 实机另复验了同账号隔离 IPC、旧 CLI 并发自启动，以及新启动进程不会继承调用方的管道写端。

方法目录与两个 SDK 对齐了 94 个公开方法，其中 93 个对应官方 GUI，另一个用于事件回放。目录记录 21 组稳定错误码。部分既有 RPC 在内部失败未记录诊断时可能省略 `diagnosticId`，SDK 可处理该响应。登记前的安装验收使用独立的有界初始化等待，登记后的 SDK 冷启动仍使用五秒就绪预算。

三个 npm 包均从本次 CI 下载，核验了原始 SHA-256，并检查 ESM、类型声明和对应平台 helper。实际运行索引、三份作业日志、包摘要与机器可读验收结果保存在本地 `target/m10-ci-dab76fcd/`，覆盖状态同步到项目的 feature parity 和测试计划。正式安装器对登记的维护归 M12。

此前各轮结果独立保留。早期 macOS 和 Windows 遇到预先创建的数据目录，Windows 随后暴露并发探测的同步等待和进程间句柄继承，Unix 也发现一个测试写死了 Windows 的错误类型。Ubuntu 的两条浏览器失败分别发生在 Unity 对话框重开和软件包版本选择，已按实际组件生命周期同步测试。最新候选在三平台跑完原完整门禁。
