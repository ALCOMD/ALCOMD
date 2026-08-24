# M6：统一扩展运行时与公开 Extension API

状态：统一 Stop A 与 review closure 已完成并通过合同门禁；获准进入 M6 production，尚未进入 M7

## 目标

在 M1-M5 已完成的 RPC、Principal、Operation、Event、Revision、资源锁和本地工作流基础上，交付
第一方与第三方完全共用的最小 Extension Runtime 垂直切片：

```text
.alcomdext
    -> alcomd manifest/package validation
    -> immutable install plan / lifecycle state / scoped Principal
    -> one alcomd-extension-host OS process per enabled ExtensionId
        -> WASI 0.2 Component Model + Extension ABI v1
        -> versioned WIT imports/exports
        -> deny-by-default Host Capabilities
    -> alcomd-application use cases
```

M6 优先证明一个无隐藏权限的测试扩展可以安装、启用、通过 `projects.read` 读取一个指定 ProjectId
的安全摘要、被撤销、崩溃隔离、
禁用并卸载。第一方扩展必须走完全相同的包、Manifest、ABI、权限、Principal、宿主和数据 API。
不在本里程碑预建通用插件框架、service locator、任意依赖注入或 workflow engine。

## 前置条件

- M5 最终技术候选 `48b188e3bf3f90c10b5cb3257365fa2ff8259faf` 与 GitHub Actions run
  `32668802957` 已通过三平台验收并由项目所有者最终人工验收；M5 正式完成。
- M5 acceptance closure `c39fa0fc4107bc6fcb2f3873135ede04618346b7` 对应 run `32671272354`
  的 Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 均成功。
- A-022 已冻结 ABI 方向：WASI 0.2 Component Model、版本化 WIT、固定合适的 Wasmtime LTS 主版本线，
  并持续接收兼容的安全与关键正确性补丁；WASI 0.3 不阻塞 M6，也不得破坏 ABI v1。
- `specs/extensions/*.md`、`specs/security/extension-threat-model.md` 仍是 Draft；本计划已获准直接完成
  contract-first 工件，但所有工件必须在统一 Stop A 再次取得人工审批后才允许生产实现。

## 最小交付物与完成定义

1. Extension Manifest/package v1：稳定 identity/version/API range、entrypoint、贡献项、权限、资源限制、
   publisher/signature metadata 与无歧义的 canonical package identity。
2. Extension API/ABI v1：WASI 0.2 Component Model、版本化 WIT world、兼容协商和稳定 machine error。
3. daemon-owned 生命周期：Plan install、Apply install、enable、disable、uninstall、crash/quarantine/restart；
   状态、revision、Event、幂等与恢复均由 `alcomd` 权威持有。
4. 每扩展独立 Principal、deny-by-default permission grant、resource scope、即时 revocation、
   `ExtensionInstanceLease` 和独立数据命名空间。
5. 每个 enabled ExtensionId 独占一个 `alcomd-extension-host` OS process；第一条垂直切片每 Host 只有一个
   active Component instance，并冻结 CPU/instruction、wall timeout、内存、table、并发、消息和后台租约硬限制。
6. 最小 Project-summary read Host Capability 与 UI Bridge 安全基础；扩展不能获得数据库、RPC socket、裸文件系统、
   任意进程、环境变量、凭据或 Tauri private command。
7. 第一方/第三方 parity 与 sandbox-abuse 合同测试冻结；未来生产实现必须在三平台通过，真实第一方 Manifest
   可被同权限测试扩展替换，
   不存在隐藏 privileged API。
8. Stop A 的 Schema/WIT/spec/fixtures/contract tests 与依赖评估通过完整本地门禁、锁文件无变化并取得人工审批；
   后续生产完成仍需三个 hosted job、工作树干净，并停止在 M7 前等待人工验收。

## Contract-first 顺序

生产代码前依次冻结并测试：

1. 更新 ADR 0007，并新增窄 Extension ABI/lifecycle ADR。
2. 冻结 `manifest-v1` 文档、JSON Schema、canonicalization、包目录和 hostile package vectors。
3. 冻结 Extension API v1/WIT world、ABI negotiation 与 compatibility matrix。
4. 冻结 permission/resource-scope/revocation、Host Capability、UI Bridge 和 extension-owned data contract。
5. 冻结 lifecycle/state migration、RPC v1 兼容新增、错误、Operation/Plan/Apply 和 crash recovery。
6. 冻结 exact runtime limits、Host process topology、package/signature、State migration draft、RPC/Plan/Apply/recovery
   与 Wasmtime/WASI 依赖候选评估。
7. 运行 Schema/WIT/Manifest/permission/migration snapshot tests；全部工件在统一 Stop A 报告并取得人工审批后
   才开始生产实现。

## Manifest、身份与版本

- Extension ID 使用稳定 reverse-DNS；品牌代际、显示名称和安装路径不参与 identity。
- `version` 使用标准 SemVer；`api` 只表达 Extension API/ABI compatibility，不等于产品版本或 RPC major。
- package content identity、publisher cryptographic identity、publisher trust policy 与 first-party policy 是四个
  独立层次。package identity 使用 canonical manifest/package SHA-256；publisher identity 使用稳定 public-key
  fingerprint；trust 是 daemon/local policy 的判断；first-party 只来自固定官方 publisher trust 和已批准发行来源。
  Manifest 的 `first_party = true` 一类字段永远不产生 first-party 身份或权限。
- entrypoint 只能是包内规范化相对路径；拒绝绝对路径、`..`、symlink/reparse、Unicode/case collision、
  duplicate entry、unsupported file type 和超额 archive。
- 安装计划固定 package digest、publisher identity、Manifest digest、entrypoint digest、requested permissions
  和目标版本；Apply 必须重新验证且不得静默改成另一版本。
- `.alcomdext` v1 若使用 ZIP，必须复用现有 `zip 8.6.0`、hostile path validation、`sha2` 与 Unicode
  normalization/collision helpers，并冻结 Extension 专用 quota；不引入第二套 archive stack，也不复用未说明
  理由的 Template/Backup quota。拒绝 traversal、absolute/device/UNC、symlink/reparse、duplicate、case/Unicode/
  file-directory collision、special entry 与 unsupported codec。
- M6 只规划一个原生 `.alcomdext` v1，不导入 v3 扩展格式，也不支持原生 DLL/`.so`/`.dylib`、tar/gzip/zstd。

## Host、core/application 与依赖方向

```text
alcomd RPC adapter
    -> alcomd-application extension use cases
        -> alcomd-domain lifecycle/permission values
        -> extension registry/data/runtime ports

alcomd-extensions
    -> manifest/package/WIT/runtime adapter
alcomd-extension-host
    -> exactly one ExtensionId per OS process
        -> one active Component instance in the first vertical slice
```

- `alcomd` 仍是唯一状态、权限和 extension-owned durable data 写入者。
- Host 不直接打开 `state.db`、项目、package cache 或 daemon 私有数据；只经认证的窄 host protocol 调用
  application 用例。该 host protocol 是内部进程合同，不作为绕过 RPC/权限的第二套业务 API。
- 每个 enabled ExtensionId 对应一个独立 Host process；Host 不得装载另一个 ExtensionId，不建立跨扩展 pool，
  第一方也不共享 privileged Host。Host crash/hang/OOM 只影响该 ExtensionId。同一扩展未来需要多个 Component
  instance 时仍受该扩展 instance quota；Host pooling 属于未来独立评估，不是 ABI v1 能力。
- `alcomd-domain` 不依赖 Wasmtime、WASI、WIT、archive、SQLite、Tauri 或 OS API。
- `alcomd-extensions` 不演变成通用 utilities；只负责 M6 Manifest、ABI、runtime、capability 和 package adapter。
- UI Bridge 只冻结消息/会话/撤销合同和 headless harness；完整 GUI 容器与产品页面属于 M7。

## 权限、资源范围与撤销

- 新扩展 Principal 默认没有任何权限。required grant 未满足则不能 enable；optional grant 未满足只能降级。
- 权限名不形成隐式层级；每次 Host Capability/application 调用都重新检查 extension identity、当前 grant、
  resource scope、instance lease 和必要 revision。
- scope 使用稳定资源 ID/opaque selector，不把路径、URL、Manifest、capability 或自报 metadata 当作授权。
- revoke/disable/uninstall 必须使新调用立即失败，并取消或关闭旧 session、lease、subscription、capability
  handle 与待处理请求；不能只更新数据库后等待进程自然退出。
- durable grant revision 更新是 revoke 的 authority linearization point。此后尚未被 application 接受的调用失败、
  Host 队列中的旧调用取消、旧 handle/session 失效，且每个新 call 必须重新检查 grant revision；guest 不能缓存旧授权。
- 已进入核心 Operation 的高影响写入遵循既有 Operation/Plan/Apply 语义；扩展进程终止不自动回滚已由
  `alcomd-application` 正式接受并获得 OperationId 的 Operation，也不能继续扩大任何后续 capability authority。
- daemon 每次启动 instance 都创建 guest 无法自报的 `ExtensionInstanceLease`，至少绑定 ExtensionId、InstanceId、
  PrincipalId、current grant revision、lifecycle generation 与 expiry/cancellation。guest/Host 参数不得指定或决定
  PrincipalId、ExtensionId、publisher、first-party status 或任意 permission scope；daemon/application 每次调用均
  以 lease 重新解析真实 Principal、grant 与 scope。长期 bearer credential 不得写入包、data、log 或 Event。
- 第一方只允许预设默认安装/启用策略和官方 publisher trust，不允许额外权限名、private endpoint 或
  跳过审批/revocation。

## 生命周期与持久状态

durable desired state、quarantine enforcement 与瞬时 runtime instance state 分开建模：

- durable lifecycle/desired state：`installed_disabled`、`enabled`、`uninstalling`；
- quarantine state：`clear`、`quarantined`；
- runtime instance state：`stopped`、`starting`、`running`、`stopping`、`crashed`。

`desired_state = enabled + quarantine_state = quarantined + instance_state = stopped` 是合法组合。quarantine 不覆盖或
丢失用户 enabled intent。数据库中的 `enabled` 不代表进程仍存在，daemon/Host crash 不得产生 phantom running
instance；解除 quarantine 与重启后的自动启动只由 bounded restart policy 决定。不存在或未复验的包不能仅凭
数据库行启动。每 ExtensionId crash evidence 最多保留 16 条。

- install/uninstall 是高影响 Operation，使用 immutable Plan、digest revalidation、Extension(id) 资源锁、
  staging、atomic publish、journal 与恢复；不能直接解压到 live directory。
- enable 在启动 Host 前验证 package、ABI、grant 和 data namespace；disable 先撤销 lease，再有界停止 Host。
- crash 产生脱敏 Event/error，并按冻结的有界策略重启或 quarantine；不得无限 crash loop。
- uninstall 默认先 disable/revoke，再移除 live package。extension-owned data 的保留或删除必须在 Plan 中
  明示；M6 不静默删除用户数据，也不实现未来迁移清理。retained namespace 绑定 ExtensionId + publisher
  fingerprint，uninstall 总是撤销所有 grants，reinstall 从 deny-by-default 开始。
- 最小 State Schema 只容纳真实需要的 extension registry、grant/scope、instance/crash 状态、install plan
  和 journal；具体 Schema 版本及 migration 必须另行人工审批。

## Extension-owned data isolation

- M6 v1 只冻结 extension-owned bounded key/value store；value 是 opaque bytes，最小 API 为 `get`、`set`、`delete`。
  `list`/prefix scan 仅在 contract-first 证明真实需求时加入；不创建 blob object store、streaming blob、共享 store
  或 filesystem-backed virtual disk。
- 每个 ExtensionId 获得独立逻辑 namespace、byte quota、key count quota、key/value size limit 与 transaction/revision；
  第一方无共享 namespace，写入经 daemon transaction、revision 和 quota 原子校验。
- 扩展只能读取自己的 namespace；跨扩展共享必须未来通过显式公共 capability 设计，不在 M6 暗中开放。
- disable 保留数据但阻止后台访问；uninstall 默认保留数据。删除 extension-owned data 必须是显式 high-impact
  immutable uninstall Plan 参数，不得静默删除。日志、Event 和错误不返回
  原始私密路径、argv、token、Authorization 或其他扩展数据。

## UI contribution 与后台能力

- M6 只冻结 packaged static UI asset identity、sandbox/origin、versioned Bridge session、request/response/event
  envelope、request ID、replay protection、size/rate/concurrency limit、permission/revocation 与 headless malicious-UI
  harness；不冻结 sidebar、settings page、toolbar、context menu 或 navigation placement，这些属于 M7 产品合同。
- 合同测试若需要 UI contribution，只使用明确标记为 `headless/test contribution` 的 synthetic fixture，不形成
  M7 public placement API，也不实现完整页面、导航或 Material Design 3 产品体验。
- UI 不能访问主 DOM、Tauri IPC、Node API、Host filesystem 或其他扩展 frame；所有消息有版本、request ID、
  大小/频率/并发上限和当前 permission check。
- UI-only 扩展不因此获得 `background.run`；后台扩展也不自动获得 `ui.contribute`。
- background lease 有明确激活原因、空闲/总时限和取消；GUI 关闭不等于授予永久后台执行权。

## 第一条 Host Capability 与其余敏感能力

- 第一条 vertical slice 唯一业务 capability 是窄 Project summary/read：Extension WIT -> Host Capability ->
  既有 `alcomd-application` project read use case；权限为 `projects.read`，scope 为 specific ProjectId。
  DTO 只含 extension-safe projection。扩展不得直接读取 Project filesystem 或 `state.db`，也不得复制 Project backend。
- 该切片真实验证 deny by default、grant、ProjectId scope、Host -> application routing、revoke、stale lease 与
  first/third-party parity。
- `host.network.request`、filesystem、clipboard、notification、Discord/external-config 在 M6 Stop A 仅保持
  planned/not implemented，不进入第一条切片。不得提供 raw socket、任意 filesystem 或隐藏第一方能力。

## ABI、兼容性与稳定错误

- ABI v1 使用 WASI 0.2 Component Model 与版本化 WIT world。Host 返回实际支持的 ABI/API capability；
  Manifest 自报版本不是安全身份。
- Component Model 是 ABI/runtime 基础，不是 ambient authority。guest 默认没有 preopened/arbitrary filesystem、
  inherited env/argv/stdin/stdout/stderr、socket/DNS、process/terminal、daemon RPC socket、Host private IPC 或 OS
  credential store；不得使用 `inherit_env`、`inherit_args`、`inherit_stdio`、`inherit_network`、preopened current
  directory 等 convenience configuration。每个 import 必须是已冻结 ALCOMD WIT interface 或单独批准的 WASI 0.2
  interface；第一条切片不需要的 clocks/random 等接口不自动开放。
- 已发布 WIT function signature 或现有 type 的 record field 增删改、variant/enum 语义变化、tuple/result/option
  变化、parameter/result 类型变化、existing required import 变化均默认是 breaking ABI change。WIT record 不使用
  JSON unknown-field 兼容规则。
- 兼容扩展通过新的独立 optional interface、negotiated capability、optional function group 或新 world/version；
  Host/Manifest negotiation 决定 guest 是否要求新增接口。ABI v1 发布后现有 type/function shape frozen，并以
  compatibility matrix 和 old-guest/new-host golden fixture 固定。
- 未知 optional capability 可忽略；不支持 required import 返回稳定 `extension_api_unsupported`，不得 trap
  后伪装成内部错误。
- 至少冻结：`extension_manifest_invalid`、`extension_package_untrusted`、`extension_already_installed`、
  `extension_not_installed`、`extension_permission_denied`、`extension_scope_denied`、
  `extension_api_unsupported`、`extension_resource_limit`、`extension_crashed`、`extension_quarantined`、
  `extension_plan_stale`、`extension_data_quota_exceeded`、`internal_error + diagnostic_id`。
- 普通错误不携带 trap backtrace、宿主路径、环境变量或其他扩展数据；诊断访问仍受既有诊断边界控制。

## 资源限制、取消与崩溃隔离

- 生产前必须冻结可机器测试的 exact max linear memory、table elements、Component instances、concurrent guest calls、
  WIT input/output/message bytes、host-call rate、fuel 或等价 instruction budget、epoch/wall timeout、background lease
  duration 与 crash-loop threshold/window；不得使用 `reasonable`、`bounded` 或 `implementation-defined` 代替数值。
- 若 fuel + epoch 组合有明显性能或实现成本，Stop A 依赖评估比较后只选实际需要的组合。
- timeout/cancel 会中断 guest 执行并撤销临时 handle；Host hang/crash 不阻塞 daemon 单写者和其他扩展。
- Host 进程退出、guest trap、OOM、fuel exhaustion、host-call timeout 和 malformed component 映射为不同稳定
  internal states，但公开错误保持脱敏。
- 扩展不能控制 kill/signal/priority，也不能要求 daemon 加载 native code。

## 测试与三平台验收

### Contract/unit

- Manifest/Schema/WIT golden、API negotiation、SemVer compatibility、canonical digest、hostile package vectors
  和 old-guest/new-host compatibility fixture。
- permission deny/default、required/optional、scope intersection、revocation race、namespace isolation 和 quota。
- lifecycle desired/runtime transition、lease generation/grant revision、revision/idempotency、crash-loop quarantine、
  install/uninstall journal recovery。

### Integration/fault/security

- 生产阶段以真实 daemon + 每 ExtensionId 独立 Host + 最小 Component 执行 install -> enable -> scoped Project
  summary read -> revoke -> disable -> uninstall；Stop A 只冻结并验证 synthetic contract fixtures，不广告 capability。
- 在 package publish、registry commit、Host start、grant revoke、data commit 和 uninstall 各 durable boundary
  强制终止/重启；不得产生 phantom enabled、混合包或虚假 succeeded。
- malicious components 覆盖 trap、无限循环/instruction budget、memory growth、oversized message、host-call flood、invalid WIT、
  data cross-read、scope escape、revocation in-flight 和 crash loop。
- malicious UI harness 覆盖 origin spoof、replay/request-ID collision、oversized/flood、DOM/Tauri/private channel 尝试。
- 同一个测试扩展分别以“第一方发行策略”和普通第三方安装，断言其公开能力集合完全相同；测试扫描禁止
  first-party-only method、permission bypass、private IPC 和 hidden Tauri command。
- 网络测试只用本地 mock，不执行攻击性公网、真实 credential 或凭据传播测试。

### Hosted

- Windows Server 2025、Ubuntu 22.04、macOS 15 arm64 运行 setup/check/test、M0-M6 contract/fault/security、
  release、Tauri no-bundle、锁文件、unsafe 和 final diff。
- Ubuntu 继续验证最高 `GLIBC_2.35` 上限；macOS 验证全部预期产物 arm64/minos 11.0。
- hosted 测试不替代 M12 Windows 10/11 完整客户端安装、WebView2、更新和卸载验证。

## 允许修改范围（获批生产实施后）

```text
apps/alcomd/                         # lifecycle/RPC/recovery adapter
apps/alcomd-extension-host/          # sandboxed background host
crates/alcomd-application/           # extension use cases/ports
crates/alcomd-domain/                # pure lifecycle/permission values
crates/alcomd-extensions/            # manifest/package/ABI/runtime adapter
crates/alcomd-protocol/              # compatible RPC DTO only
crates/alcomd-store/                 # approved minimal Schema/migration only
crates/alcomd-client/                # official typed client
extensions/                          # same-contract examples/first-party manifests
specs/extensions/
specs/security/extension-threat-model.md
specs/rpc/                           # approved compatible additions only
specs/storage/                       # approved M6 Schema only
docs/adr/
docs/testing/
docs/exec-plans/M6-extension-runtime.md
docs/status.md
feature-parity.toml
xtask/src/main.rs
scripts/
.github/workflows/ci.yml             # only M6 acceptance commands
```

任何公共 RPC、State Schema、permission name、WIT/Extension API、host capability 或数据删除语义在修改前
仍需人工审批。本次审阅只授权 Stop A 的 ADR/spec/Schema/WIT/metadata/synthetic fixtures/contract tests/
migration contract draft；不得修改 production Rust/TS implementation、advertise capability 或发布 CLI/GUI backend。

## 明确排除

- M7 完整 GUI 产品页面、扩展商店、Material Design 3 管理体验和截图 parity。
- M8 MCP 协议/管理产品；M9 Discord Presence 产品逻辑；Local API 与新 SDK。
- v3 migration、bootstrap/updater、installer/signing/dist 和 M12 客户端发行验证。
- 原生动态库、任意 shell/process control、raw socket、任意 filesystem、通用 browser/desktop automation。
- WASI 0.3、ABI v2、热重载、跨扩展 service discovery、通用 dependency injection/workflow engine。
- M6 第一条切片不实现 network、filesystem、clipboard、notification、Discord 或 M7 产品 UI placement。

## 生产依赖审批点

Stop A review 已批准以下精确 production dependency，除此之外均需重新人工审批：

- `wasmtime = 48.0.0`，defaults off，仅 `async/component-model/cranelift/runtime/std`，只进入 Extension Host graph；
- `ed25519-dalek = 3.0.0`，defaults off，只用于 daemon package/signature verification。

明确不批准 `wasmtime-wasi`、`component-model-async`、direct `wit-component`、新 platform sandbox crate 或新
`windows-sys`/`rustix` feature。首次写入 manifest 后立即检查 active graph 与 lock diff。任何其他 Component
tooling、archive/signature dependency 或平台 primitive 都必须重新提交 exact version/features/license/MSRV/
unsafe/native/build-script/lock diff 评估；不得用通用 RPC/HTTP/framework crate 替代窄 Host protocol。

## Release blockers 与风险

- 第一方隐藏能力、权限撤销后仍可调用、跨扩展数据读取或直接项目/数据库访问是 blocker。
- daemon 内加载不受信任 native code、无限 guest 执行、无硬内存/并发限制或 crash loop 是 blocker。
- install/uninstall 半提交、digest/signature 复验缺失、symlink/path collision 或 stale Plan 静默重规划是 blocker。
- UI origin/session 验证不足或 Bridge 暴露 Tauri/private RPC 是 blocker。
- Wasmtime 主版本/LTS 选择不明确、无法及时接收兼容安全修复或三平台行为未经实测是 blocker。
- 将 synthetic parity 描述为真实 v3/Discord/MCP 产品验证是 blocker。

## 统一 Stop A 人工审批结果

统一 Stop A 已于 2026-08-24 通过项目所有者人工审批。review closure 作为独立提交完成并通过 contract gates 后，
可按批准的 Slice A-F 开始 production：

1. `.alcomdext` exact layout/quota、canonical package/publisher/signature、ExtensionId/version/API negotiation。
2. exact WIT world/interfaces、compatibility matrix、old guest/new host fixture 与 ambient WASI deny list。
3. exact permission names、specific ProjectId read scope、ExtensionInstanceLease 与 revoke-in-flight semantics。
4. durable desired/runtime state、install/uninstall Plan/Apply/recovery、Schema version/tables 与 RPC/errors/capability。
5. extension-owned key/value API/quota、UI Bridge security envelope/headless harness、exact runtime limits 与 one-host-per-ID topology。
6. Wasmtime/WASI/WIT dependency assessment 及任何生产 dependency/unsafe/platform API 需求。

closure commit 前不得修改 production Rust/TS implementation 或 Cargo manifest/lock。closure 后 production 必须保持
Wasmtime 只进入 Host graph、daemon 不实例化 guest、无 ambient WASI、无 M7 placement，并遵守新的停止条件。

## 验证命令

ExecPlan 修正至少运行：

```text
cargo xtask check
<fixed-python> scripts/validate-metadata.py
pwsh -NoProfile -File scripts/freeze-baselines.ps1 -Check
git diff --check
```

Stop A 完成后至少运行完整 `fmt`、`clippy --locked`、Workspace tests、xtask、metadata、baseline freeze 与 diff gate，
并确认三份锁文件不变。生产实现完成后还必须运行 setup/check/test、WIT/Schema/Manifest compatibility、sandbox
fault matrix、Tauri no-bundle、dependency/unsafe/lockfile gates 和三平台 CI。

## M7 前停止条件

1. contract-first 工件与所有公共合同在统一 Stop A 取得人工审批后才允许 M6 生产实现。
2. 最小 extension lifecycle/ABI/capability/data 垂直切片和真实 hostile/fault tests 全部通过。
3. 第一方/第三方 parity 证明无隐藏 privileged API；`extensions.runtime`/`extensions.abi-v1` 只按真实覆盖更新。
4. 最终提交三平台 hosted CI 成功、SHA 对齐、工作树干净。
5. 项目所有者完成人工验收后 M6 才可正式完成；随后仍停止在 M7 前，不自动开始 GUI 实现。

## 进度日志

- 2026-08-24：M5 已通过最终人工验收并正式完成；acceptance closure
  `c39fa0fc4107bc6fcb2f3873135ede04618346b7` 对应 CI run `32671272354` 三平台全部成功。
- 2026-08-24：创建本 M6 ExecPlan 草案。只规划统一 Extension Runtime、公开 ABI/API、权限、生命周期、
  数据隔离、Host/UI 边界和验收；未修改生产代码、RPC、State Schema、permissions、依赖或 M6 feature/test 状态。
- 2026-08-24：项目所有者完成人工审阅并附最小修正批准 M6 contract-first。冻结 one enabled ExtensionId per Host
  process、无 ambient WASI authority、WIT shape breaking 规则、lease/revocation linearization、desired/runtime
  分离、bounded key/value、Project summary read 第一能力、publisher 信任分层、exact runtime limit 与统一 Stop A；
  production implementation 和 Wasmtime/WASI/WIT 依赖仍未批准。
- 2026-08-24：统一 Stop A 已形成 Manifest/package/signature/archive、WIT ABI v1、permission/lease/revocation、
  lifecycle/State v8 migration contract、RPC/error/hello optional contract、bounded key/value、UI Bridge headless
  envelope、threat model、synthetic fixtures 与 contract tests。隔离依赖评估推荐 `wasmtime 48.0.0` 和
  `ed25519-dalek 3.0.0`，但未修改 Cargo manifest 或 lock file，也未接入生产代码。`fmt`、Clippy、Workspace
  tests、xtask、metadata、baseline freeze、diff/lock gates 全部通过；停止等待项目所有者审批。
- 2026-08-24：项目所有者批准 Stop A 与精确 `wasmtime 48.0.0`、`ed25519-dalek 3.0.0` production dependency，要求先
  以独立 closure commit 修正 desired/quarantine/runtime 三分模型、publisher-bound retained data、uninstall grant
  revocation、logical UI origin、两个 exact Operation kind、16 条 crash evidence 上限与本地/first-party source kind；
  closure contract gates 通过后可直接按 Slice A-F 实施，不得进入 M7。
- 2026-08-24：review closure 已完成：desired/quarantine/runtime 三分、publisher-bound retained namespace、uninstall
  grant revocation、logical UI origin、仅 `extensions.install/uninstall` 两个 Operation kind、每 ExtensionId 16 条
  crash evidence、受控本地 source kind 与 Host child/protocol hardening 均已冻结。M6 contract 6/6、RPC Schema
  22/22、fmt、xtask、metadata、baseline freeze、diff/三锁文件门禁通过；production source 与 Cargo manifest 未修改。
