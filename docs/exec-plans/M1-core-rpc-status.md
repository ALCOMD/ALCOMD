# M1：核心进程、本地 RPC 握手与 CLI system status

状态：M1 已通过人工验收并正式完成；尚未进入 M2

## 目标

在 M0 可复现骨架上完成第一个可端到端运行的只读垂直切片：

```text
alcomd-cli system status
        │
        ▼
ALCOMD RPC v1（Named Pipe / Unix Domain Socket）
        │
        ▼
alcomd
        │
        ▼
alcomd-application 的 system status 查询
```

M1 只证明以下基础能力：

- 每个操作系统用户只有一个权威 `alcomd` 实例。
- 官方客户端能发现本用户 IPC 端点、建立连接并首先完成 `system.hello`。
- 握手成功后，`alcomd-cli system status` 通过真实 RPC 返回只读核心状态。
- RPC 帧、请求/响应、稳定错误、版本协商和断线行为具有冻结合同与跨平台测试。
- CLI 或客户端断开不影响核心进程；核心能优雅停止，并能在重启后重新接受连接。
- 本阶段不创建业务权威状态，不写数据库或 Unity 项目。

## 交付物

M1 实施获批后应交付：

1. 可运行的每用户单实例 `alcomd` 本地 IPC 服务。
2. `alcomd-protocol` 中不依赖内部领域对象的 RPC v1 帧、信封、握手、状态和错误 DTO。
3. `alcomd-client` 中供官方 Rust 客户端复用的端点发现、连接、握手和 `system.status` 调用。
4. 仅调用 `alcomd-application` 查询用例的 daemon RPC 适配器。
5. 使用真实 daemon 的 `alcomd-cli system status`，保留人类可读输出和 `--json` 输出。
6. `system.hello`、`system.status`、错误信封和帧格式的 JSON Schema、合同快照与兼容性测试。
7. Windows、Ubuntu 和 macOS hosted CI 上的 IPC、单实例和 CLI 端到端证据。
8. 更新后的 `feature-parity.toml`、测试元数据、ExecPlan 进度和 `docs/status.md`；这些更新只反映
   M1 实际完成的切片，不得把完整 RPC、CLI 或 daemon 业务能力标为已实现。

## 完成定义

以下条件全部满足后，M1 才可提交人工验收：

- 本计划中的线协议、端点、启动策略和依赖已经项目所有者批准并落入相应 ADR/Schema。
- `alcomd` 在同一用户范围内拒绝第二个权威实例；多个只读客户端可同时完成握手和状态查询。
- Windows 使用 Named Pipe，Linux/macOS 使用 Unix Domain Socket；端点只允许当前用户访问。
- 每条连接的第一条有效请求必须是 `system.hello`；未握手、版本不支持、重复握手、畸形或超限
  帧均返回冻结的结构化行为，不触发 panic、无限等待或未界定内存分配。
- `alcomd-cli system status` 不再返回 scaffold 数据，也不绕过 RPC 或直接读取核心内部状态。
- 公共 DTO 不引用 `alcomd-domain`；`alcomd-domain` 不新增 Tauri、SQLite、HTTP、MCP 或 OS API
  依赖；传输适配器不承载业务规则。
- 测试证明 M1 正常路径不创建 SQLite、恢复 journal、项目文件或其他业务持久状态。
- M1 的全部本地门禁与三个 hosted 平台 CI 通过，三份锁文件无意外变化，工作树干净。
- `docs/status.md` 明确写为“M1 等待人工验收”，且没有开始 M2。

## 前置条件

- M0 最终提交为 `8112415f1dae0dc6f521d5cc3a2c980baac3b408`，GitHub Actions run
  `32066209115` 的 Windows Server 2025、Ubuntu 22.04、macOS 15 arm64 三个 hosted job
  全部成功。
- M0 已由项目所有者人工验收；`origin/main`、本地 HEAD 和该 CI head SHA 一致。
- M-1 基线保持 `audit_status = "complete"` 与 `m1_complete = true`。这里的 `m1_complete`
  是历史上对 M-1 审计完成状态的字段，不得误解为本 ExecPlan 的 M1 已完成。
- A-004、A-005 和 A-026 的唯一写入者、应用层边界、稳定结构化错误方向保持有效。

## 允许修改范围

M1 实施获批后，只允许修改以下范围：

```text
apps/alcomd/
apps/alcomd-cli/
crates/alcomd-application/
crates/alcomd-protocol/
crates/alcomd-client/
crates/alcomd-platform/                # 仅 M1 端点、IPC、访问控制与实例锁
crates/alcomd-testing/                 # 仅 M1 测试支持
specs/rpc/
docs/adr/0002-single-writer-daemon.md   # 仅补充已批准的 M1 实现细节
docs/adr/0004-alcomd-rpc.md             # 冻结 M1 线协议
docs/exec-plans/M1-core-rpc-status.md
docs/testing/test-plan.toml
docs/status.md
feature-parity.toml
Cargo.toml
Cargo.lock
xtask/src/main.rs                      # 仅批准的 unsafe 边界硬门禁
scripts/validate-metadata.py           # 仅 M1 依赖与 lint 元数据门禁
scripts/                               # 仅增加或接入 M1 验收命令
.github/workflows/ci.yml               # 仅接入 M1 三平台测试，不改变 M0 平台基线
```

以下文件只有在批准的 M1 依赖或验证确实需要时才可最小修改，不能借机整理 Workspace 或升级
无关依赖。`package.json`、`package-lock.json` 和 GUI 文件预期不需要变化。

## 明确不属于 M1

- SQLite、`state.db`、配置持久化、数据 Schema 迁移与 credential store。
- Operation、Event、订阅、sequence、审批、输入、幂等结果持久化、revision、资源锁与恢复
  journal；这些属于 M2。
- 项目、仓库、VPM、模板、备份、Unity、包缓存或任何 Unity 项目读写；这些属于 M3 至 M5。
- 完整 CLI 命令面、NDJSON、确认、dry-run、progress、completion 等 M5 合同；M1 只实现
  `system status` 所需的最小输出和错误面。
- Extension API、WASI/Wasmtime、扩展权限和第一方扩展运行时；这些属于 M6。
- GUI RPC 接入、Tauri command 或界面；这些属于 M7。
- MCP 协议、MCP 工具、HTTP/STDIO、MCP Principal 或管理扩展；这些属于 M8。
- Discord、TypeScript/Rust 公共 SDK 硬化、Loopback API、Python/.NET SDK。
- v3 迁移、bootstrap、updater、安装器、全产品 staging、签名、发行与零残留；这些属于
  M11/M12。
- Windows 10/11 真实客户端安装、WebView2、托盘、注册表、用户数据路径、更新和卸载验证；
  这些继续 deferred 到 M12，不以 M1 的 Named Pipe 测试冒充通过。
- 面向同用户恶意进程的完整第三方配对、授权、撤销和细粒度权限。M1 只有无副作用的系统查询；
  引入任何写方法前必须先完成后续 Principal/权限合同。

## 内部接口与公共合同边界

### 内部接口

- `alcomd-application` 暴露最小的 `SystemStatus` 查询用例；它不接受 socket、JSON 或 CLI 类型。
- `apps/alcomd` 将 RPC DTO 转换为 application 请求并将结果转换回 DTO；所有方法分派集中在
  daemon 适配器，不在 codec 中实现业务判断。
- `alcomd-client` 负责端点发现、连接生命周期、帧收发、握手状态机和类型化调用；CLI 不自行
  拼接 JSON 或平台路径。
- `alcomd-protocol` 只拥有公开 DTO、帧限制和序列化合同，不依赖 application/domain/platform。
- M1 不为只有一个查询的场景建立通用 service locator、插件式 transport registry、宏生成 RPC
  框架或多协议抽象；以后出现第二个真实需求时再提炼。

依赖方向固定为：

```text
alcomd-cli -> alcomd-client -> alcomd-protocol
alcomd -> alcomd-application -> alcomd-domain
alcomd -> alcomd-protocol
```

不得出现：

```text
alcomd-cli -> alcomd-application / alcomd-domain / project files
alcomd-protocol -> alcomd-application / alcomd-domain / OS APIs
alcomd-application -> alcomd-protocol / IPC / Tokio transport
```

### M1 要冻结的公共合同

实施前必须一次性冻结并通过 Schema/快照验证：

1. `u32` 小端长度前缀、UTF-8 JSON payload、最大帧大小、零长度/截断/超限处理。
2. JSON-RPC 风格 request/response/error 信封的版本标记、request ID 类型、method/params 和
   error data 形状，以及与 JSON-RPC 2.0 的明确偏差。
3. 第一条请求为 `system.hello` 的状态机、RPC major 协商、客户端与服务端 capability 语义。
4. `system.hello` request/response Schema；`client.name/version/instanceId` 只用于诊断和能力
   协商，绝不作为安全身份。
5. `system.status` request/response Schema。结果仅暴露产品/daemon 版本、RPC 版本、就绪状态和
   经批准的非敏感 capability；不虚构 data/config/extension Schema，不暴露 PID、完整路径、
   环境变量或凭据。
6. M1 可观测错误的稳定 code：至少覆盖 `invalid_request`、`rpc_version_unsupported`、
   `component_upgrade_required` 和 `internal_error + diagnostic_id`；技术细节不得进入普通响应。
7. `alcomd-cli system status` 的成功 stdout、人类/JSON 输出、错误 stderr 和退出码；这不是完整
   M5 CLI 合同的提前冻结。

破坏上述合同必须提升 RPC major，或在项目所有者批准后提供有时限的兼容路径；不能只提升
应用版本后静默改变。

## 最小实施路径

1. 先批准“人工审批点”中的 M1 合同，不改生产代码。
2. 用 Schema 与 Rust 测试先固定 envelope、frame、hello、status 和 error DTO。
3. 在 `alcomd-application` 将现有 scaffold health 收敛为无 OS 依赖的真实只读 system status
   用例；不引入 repository/store 抽象。
4. 使用 Rust 标准库/Tokio 的本地 IPC 能力实现平台模块；只在标准能力无法满足当前用户访问
   限制或跨平台文件锁时，引入一个用途单一、维护良好的 crate。
5. 实现 daemon 的单实例所有权、连接握手状态机和两个 RPC 方法，不提前实现通用 Command、
   Event 或 Operation dispatcher。
6. 实现 `alcomd-client` 的同一用户端点发现、握手和 status 调用，再让 CLI 只调用该客户端。
7. 添加隔离运行目录的单元/集成测试；测试不得使用或污染真实用户 ALCOMD 目录。
8. 在现有三个 hosted job 接入 M1 测试，完成验收、状态更新并停止。

## 人工审批点

项目所有者已于 2026-08-18 批准以下 M1 技术合同；严格符合这些决定的实现无需再次审批：

1. **RPC v1 线合同**：4 MiB 上限、JSON-RPC-inspired 偏差、字符串 request ID、稳定错误、
   capability 协商及最小 `system.hello`/`system.status` Schema；framing error 关闭连接，完整
   payload 的 RPC error 返回结构化响应。
2. **IPC 端点与访问控制**：Windows 使用
   `\\.\pipe\CQMHV.ALCOMD.<current-user-SID>.rpc-v1` 和当前用户 ACL；Linux 使用
   `$XDG_RUNTIME_DIR/alcomd/rpc-v1.sock`；macOS/安全 fallback 使用经过所有权检查的短 per-user
   路径；Unix 父目录 `0700`、socket `0600` 且不跟随 symlink。
3. **单实例与按需启动策略**：使用生命周期绑定的 OS 锁；Unix stale socket 只有在取得锁并
   验证类型、所有者和父目录后删除；CLI 仅在 endpoint not found/connection refused 时默认
   启动 sibling `alcomd`，总等待不超过 5 秒，并提供 `--no-start-daemon`。
4. **M1 平台依赖已逐项批准**：Windows-only `windows-sys = 0.61.2` 仅用于 SID、当前用户 DACL、
   Named Pipe 与实例 mutex；Unix-only `rustix = 1.1.4` 仅启用 `std/fs/process`，用于有效 UID、
   no-follow/fd-based 校验、权限和 `flock`。两者只存在于 `alcomd-platform` 的目标条件依赖中。
   其他新增生产依赖仍未 blanket 批准。
5. **局部 unsafe 例外已批准**：仅私有 `windows_security.rs` 可使用带 SAFETY 说明的 Windows
   FFI；`alcomd-platform` 其余文件保持 safe Rust，xtask 对位置和 allowance 进行硬门禁。

以下不属于新的产品级开放决策：A-004 的唯一核心、A-005 的 application 边界和 A-026 的
结构化错误方向已经接受；M1 只审批它们的技术合同细节。

## 测试与验收

### 单元测试

- 帧编码/解码：最小/最大、零长度、超限、截断、非 UTF-8、畸形 JSON 和连续多帧。
- DTO 与 Schema：camelCase、未知字段策略、必填字段、稳定错误 code、`diagnostic_id` 和合同
  快照。
- 握手状态机：hello 必须首发、成功、重复、版本不支持、能力去重/未知能力策略。
- 端点计算与权限：环境缺失、路径长度、非法覆盖值、Unix mode 和 Windows pipe 配置。
- application system status：确定性字段、无 Operation/Store 依赖、无敏感技术信息。
- CLI 输出格式化：human/JSON、stdout/stderr 分离和稳定退出码。

### 集成测试

- 在独立临时 runtime 根启动真实 daemon，连接真实 client，执行 hello -> status -> disconnect。
- 同时启动两个 daemon，只有一个取得权威实例；失败者返回可诊断结果且不破坏存活实例。
- 多个客户端并发读取；慢速、提前断开和畸形客户端不阻塞后续合法客户端。
- daemon 未运行、启动中、已退出和重启后的客户端行为；若按需启动获批，验证并发客户端只
  产生一个 daemon。
- 错误版本、未握手调用、超限/截断帧和未知 method 的线级 golden tests。
- 执行前后比较隔离目录，证明没有 SQLite、journal、项目或业务配置写入。
- CLI 子进程端到端验证人类输出、`--json`、退出码以及 daemon 不可用错误。

### 跨平台验收

- `windows-2025`：真实 Named Pipe、单实例、并发 client 和 CLI 端到端；它仍只代表 Windows
  Server hosted 环境，不代表 Win10/Win11 客户端发行验收。
- `ubuntu-22.04`：真实 Unix Domain Socket、权限、过期 socket 恢复与 CLI 端到端；继续执行
  `GLIBC_2.35` 上限门禁。
- `macos-15` arm64：真实 Unix Domain Socket、短路径/权限/清理与 CLI 端到端；继续验证 arm64
  和 deployment target 11.0。
- 三个平台继续运行 M0 setup/check/test、release binaries、Tauri `--no-bundle` 与三锁文件
  无变化门禁。M1 不降低或替换 M0 验收。

### 验收命令

Windows PowerShell 7：

```powershell
.\scripts\setup.ps1
.\scripts\check.ps1
.\scripts\test.ps1
cargo test --locked -p alcomd-protocol -p alcomd-client -p alcomd-application
cargo test --locked -p alcomd-testing --test m1_rpc
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo xtask check
python scripts/validate-metadata.py
.\scripts\freeze-baselines.ps1 -Check
git diff --check
```

Linux/macOS 使用等价 `./scripts/*.sh` 与 `python3` 命令。具体集成测试 target 名可以在实现时
按 Rust 测试布局最小调整，但不得静默跳过任何目标平台或失败路径。

## Release blocker

- 三个平台本地 IPC 都能建立、隔离当前用户并通过真实端到端测试。
- 单实例竞态、过期端点与并发按需启动（若获批）不会产生两个权威核心。
- 帧上限和畸形输入在分配大内存前拒绝；连接失败不会拖垮 daemon。
- hello/status Schema、错误 code、CLI JSON 和线级 golden snapshot 已冻结且通过。
- `clientInfo` 不被当作 Principal；M1 没有未授权写方法或隐藏的业务状态入口。
- 依赖方向门禁和三平台 M0 CI 继续通过；不得用平台条件跳过实现来换取绿色 CI。
- 没有数据库、Operation、Event、Lock、VPM、GUI/MCP 或迁移实现混入 M1。

## 风险与缓解

- **Unix socket 路径与残留**：macOS 临时目录和 Unix socket 长度限制不同。冻结短且用户隔离的
  端点规则，以所有权检查和进程锁保护过期清理，测试并发恢复。
- **Windows pipe ACL**：默认安全描述符不能作为充分证据。明确只允许当前用户的配置并测试，
  失败时拒绝启动而不是退化为宽权限 pipe。
- **按需启动竞态**：多个客户端可能同时 spawn。让单实例机制决定唯一胜者，客户端只做有界
  重试，不以 sleep 或 PID 文件猜测成功。
- **协议过早泛化**：M1 只有 hello/status 两个真实方法。保持显式 DTO 和小型 dispatcher；不为
  M2/M5/M8 的未知需求预建宏框架。
- **未来能力兼容**：严格 `additionalProperties` 与 capability 扩展可能冲突。M1 审批时明确
  envelope/DTO 的向前兼容规则，并用未知字段/能力测试固定。
- **同用户恶意进程**：用户限定 IPC 不能区分同一用户下的恶意程序。M1 不提供写方法；外部
  Principal、配对、撤销与最小权限在后续合同完成前仍是 blocker。
- **平台证据误用**：hosted Windows 只证明 Windows Server 上的 pipe 和构建。Win10/Win11 的
  安装、WebView2、数据目录、更新和卸载继续由 M12 真实客户端测试关闭。
- **依赖漂移**：新增 crate 可能扩大供应链与 MSRV 风险。优先固定工具链现有能力，任何新增
  生产依赖先审查再更新 `Cargo.lock`。

## 与 M0、M12 和后续里程碑的关系

- M1 继承 M0 的身份、工具链、三锁文件、三个 hosted runner 和 GUI no-bundle 门禁；不得修改
  A-025 平台范围或把 M0 的构建证据重新解释为客户端发行证据。
- M1 产出的 transport/client/DTO 是 M2 Operations/Events/Locks 的承载基础，但 M1 不实现
  M2 状态，也不承诺未经审批的 M2 方法。
- M3 至 M10 的入口必须复用 M1 的 client/protocol/application 边界，不能旁路 daemon。
- M12 负责完整产品布局、daemon 启动集成、安装/更新/卸载和 Windows 真实客户端验证。M1 的
  开发树 sibling 启动策略（若批准）不能自行成为安装布局合同；M12 必须用正式 staging 再验证。
- `platform.windows-client-runtime` 保持 deferred；只有 M12 在 Win10 22H2/Win11 上验证完整
  安装、启动、WebView2、托盘、注册表、用户数据路径、更新、升级和卸载后才可标为通过。

## M1 完成后的停止条件

1. 完成全部验收命令和三个 hosted CI，记录精确提交、run/job、平台检测与未覆盖风险。
2. 更新本计划、`docs/status.md`、测试元数据和 feature parity 的真实实现状态。
3. 提交 M1 修改并确认 HEAD、`origin/main`（如另获推送授权）与 CI head 一致、工作树干净。
4. 将状态设为“M1 实现完成，等待人工验收”，不得自动开始 M2。
5. 项目所有者必须审查 RPC v1 合同、端点安全、依赖图、跨平台证据和范围 diff，明确批准后
   才能创建/执行 M2 ExecPlan。

## 进度日志

- 2026-08-18：M0 最终验收通过；创建本 M1 ExecPlan 草案。当前仅规划，未修改 M1 生产代码、
  公共 Schema、依赖或测试实现。
- 2026-08-18：项目所有者批准 M1 总体范围与上述 RPC/IPC/单实例/CLI 合同，授权按
  contract-first 顺序实施；任何偏离或新增生产依赖仍须停止审批。
- 2026-08-18：完成合同 Schema/DTO、application status、Windows Named Pipe + 当前用户 DACL、
  Unix socket + 0700/0600/no-follow、生命周期实例锁、daemon 握手 dispatcher、类型化 client、
  CLI status 与五秒有界并发按需启动；Windows 本地端到端、Linux/macOS target 编译检查通过。
- 2026-08-18：Windows-only `windows-sys 0.61.2`、Unix-only `rustix 1.1.4` 与唯一私有 Windows
  FFI unsafe 边界均已获项目所有者逐项批准；Cargo.lock 只新增预期的 rustix/linux-raw-sys。
- 2026-08-18：完整本地 `setup/check/test`、格式、Clippy、Workspace/合同/集成测试、并发按需启动、
  metadata、xtask、冻结基线与 diff 门禁全部通过；Linux x86_64 与 macOS arm64 目标交叉编译通过。
  提交 `e509554af6cb1029f4a023e26013b495c0a56ffe` 对应的 GitHub Actions run `32124358425`
  已通过 Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 三个 hosted job；Linux 实测最高
  `GLIBC_2.34`，九个 macOS Mach-O 均为 arm64 / minos 11.0。M1 已具备提交人工验收的证据，
  但未经项目所有者验收不得进入 M2。
- 2026-08-18：项目所有者确认最终提交
  `7ed70626a0176f855a8e9efdc6d35d317f51ca78` 与 GitHub Actions run `32126344788` 验收通过；
  HEAD、`origin/main` 与 CI head SHA 一致。M1 正式完成，后续仅可先规划 M2，不得据此开始
  M2 生产实现。
