# M3：项目与 VPM Repository 只读垂直切片

状态：合同、生产实现、本地验收、三平台 hosted CI 与项目所有者人工验收全部完成；尚未进入 M4

## 目标

在已经完成并通过人工验收的 M2 `state.db`、Revision、Event、Principal、资源锁和本地 RPC
基础上，完成第一个真实的项目与 VPM Repository 读取切片：

```text
alcomd-cli project/repository read commands
        │
        ▼
ALCOMD RPC v1 compatible additions
        │
        ▼
alcomd-application queries / low-impact registry commands
        │
        ├─ projects.read / projects.manage
        ├─ repositories.read / repositories.manage
        ├─ revision / idempotency / Event
        └─ Project / Repository resource locks
                │
                ├─ alcomd-store -> state.db read models
                └─ alcomd-vpm -> bounded read-only parsers/loaders
                                      │
                                      ├─ Unity project files (read only)
                                      ├─ local repository JSON (read only)
                                      └─ anonymous HTTP(S) repository JSON
```

M3 中“只读”的精确定义是：不得修改 Unity 项目、`Packages/vpm-manifest.json`、
`Packages/manifest.json`、`ProjectSettings/ProjectVersion.txt`、repository JSON 或其他外部源文件。
项目/仓库的注册、取消注册和显式刷新若按本草案获批，只能修改 ALCOMD 自有 `state.db` 注册表与
读取缓存；它们是低影响同步命令，不是项目写入、包事务或高影响写 Operation。

## 最小交付物

1. 项目根发现、显式根检查、Unity 版本和项目类型的 bounded、read-only parser。
2. VPM/UPM/ProjectVersion 的最小读取模型，不做 dependency resolve、版本选择或修复写回。
3. 本地文件与匿名远程 HTTP(S) repository 共用的文档加载/解析边界。
4. repository 顶层元数据、包 ID/版本身份和解析 issue 的确定性读取模型；不实现包安装元数据语义。
5. `state.db` Schema v2 的最小项目/仓库注册状态、成功快照、revision 与 Event。
6. RPC v1 的兼容新增方法、capability、DTO、稳定错误与 JSON Schema 草案。
7. CLI 的最小 project/repository human/JSON 入口；不宣称完整 CLI 已实现。
8. 合成公开 Fixture、SQLite migration、只读无写入、本地 mock HTTP 和三平台集成测试。

## 完成定义

- 显式项目路径只检查该目录；只有明确请求 parent discovery 时才向父级查找，不能隐式漂移到
  其他项目。
- 项目读取能区分合法、缺失、malformed 和 inaccessible 的 ProjectVersion/VPM/UPM 输入，
  返回稳定结构化结果或错误，不 panic、不泄露无权限的完整路径。
- 项目类型识别只依据本阶段冻结的可观察文件和 manifest 信息，不能调用 Unity、修改项目或
  进行包解析。
- 所有只读查询前后，受观察项目文件的内容摘要、mtime 与目录结构保持不变；旧 manifest 修复
  只能留给后续显式 migration/Plan/Apply。
- local/remote repository 使用同一 parser 和 normalized catalog；远程失败保留最后一次成功
  快照，不能把失败或部分文档提交为新成功状态。
- repository refresh/cache 只缓存 repository 元数据与包身份读取模型；不下载 package ZIP，
  不建立 M4 package cache。
- Project/Repository revision 只在成功提交的注册状态或 normalized snapshot 实际变化时递增；
  no-op、HTTP 304 和失败不递增也不发 Event。
- M2 Operation/Event 查询仍兼容；Schema v1 -> v2 migration 原子、可重复启动、失败完整回滚。
- M3 所需 RPC、权限、Schema、依赖和网络边界先经项目所有者批准，再开始生产实现。
- 本地完整门禁和 Windows Server 2025、Ubuntu 22.04、macOS 15 arm64 hosted CI 全部通过，
  工作树干净，并停止在 M4 之前等待人工验收。

## 前置条件

- M2 生产最终提交为 `9076574ef0f4d3de8690865dfb18aa5856d7ad64`；GitHub Actions run
  `32144082427` 的三个 hosted job 全部成功，项目所有者已确认人工验收通过。
- M2 状态文档收尾提交为 `54e13fc2b228ad26919b1f8a7efe2893cd60c965`；它只记录验收事实，
  不改变 M2 生产合同。
- A-004/A-005 唯一写入者和 application 边界、A-015/A-016 独立实现与只读参考边界保持有效。
- M1 RPC v1 与 M2 data Schema/Revision/Event/Principal 合同只能兼容增加，不能被 M3 静默改写。
- `projects.v3-parity` 所需 M11 脱敏真实 Fixture 仍不存在；该测试保持 `blocked`。

## 允许修改范围

M3 生产实现只有在本计划所列合同测试通过后，才允许最小修改：

```text
apps/alcomd/                         # 仅 M3 RPC adapter 与启动接线
apps/alcomd-cli/                     # 仅最小 project/repository 读取入口
crates/alcomd-domain/                # Project/Repository ID、revision/resource value
crates/alcomd-application/           # M3 ports、queries、低影响 registry use cases
crates/alcomd-protocol/              # 已批准的 M3 RPC DTO
crates/alcomd-client/                # 已批准的类型化 M3 调用
crates/alcomd-store/                 # Schema v2、migration 与 M3 repository implementation
crates/alcomd-vpm/                   # 只读 project/repository parser 与 document loader
crates/alcomd-platform/              # 仅确有需要的跨平台路径身份检查；不扩大 unsafe
crates/alcomd-testing/               # 合成 Fixture、mock HTTP、真实 IPC 测试支持
specs/rpc/                           # M3 方法、DTO、错误 Schema
specs/storage/                       # state Schema v2 与 migration 合同
docs/adr/                            # 新 M3 read-model ADR 或既有 ADR 的最小兼容补充
docs/exec-plans/M3-project-repository-readonly.md
docs/testing/test-plan.toml
docs/status.md
feature-parity.toml
Cargo.toml
Cargo.lock
scripts/                             # 仅接入 M3 验收
.github/workflows/ci.yml             # 仅接入 M3 测试，不改变平台范围
xtask/src/main.rs                    # 仅 M3 依赖方向/合同门禁
scripts/validate-metadata.py         # 仅 M3 元数据门禁
```

`package.json`、`package-lock.json`、GUI、MCP、扩展、迁移和发行文件预期不需要变化。任何扩大
范围、unsafe 边界或生产依赖都必须重新审批。

## 明确不属于 M3

- 包安装、移除、重装、resolve、升级、降级、版本范围求解或 Unity/package compatibility。
- package ZIP 下载、哈希 sidecar、解压、归档安全、package cache 或 local user package copy。
- Plan/Apply、项目文件事务、双 manifest 提交、legacy 删除或任何 Unity 项目写入。
- 创建/复制/删除项目目录、启动 Unity、Unity Hub、项目迁移、备份、模板与恢复。
- 自动后台扫描、文件 watcher、定时 repository refresh 或通用任务调度器。
- 高影响写 Operation、审批/输入、项目写 recovery journal 或 M4 package transaction recovery。
- repository credential/custom header、OS credential store、认证导入/导出和 `vcc://` deep link。
- GUI、MCP、Local API、Extension API/runtime、Discord 或完整 CLI 命令面。
- v3 数据读取、迁移 Fixture、删除清单、bootstrap、updater、安装器、签名和发行。
- Windows 10/11 完整客户端发行验证；继续 deferred 到 M12。

## 项目发现、注册状态与读取模型

### 发现模式

M3 建议冻结两个显式模式，禁止依赖 daemon 当前工作目录：

1. `exact-root`：调用方传入目录就是候选根，只检查该目录，不向父级搜索。
2. `search-parents`：调用方显式传入起始路径并请求父级发现；逐级向上到文件系统根，返回第一
   个符合条件的项目根。

Unity 项目根的最小必要证据是 `ProjectSettings/ProjectVersion.txt`。VPM manifest 优先于 UPM
manifest 仅用于可观察类型/读取模型；两者都缺失时仍可识别 legacy/unknown Unity 项目，而不是
把查询变成创建或修复 manifest。路径必须转为绝对、稳定的 per-platform identity key；
symlink/root identity、Windows 大小写与 Unix 非 Unicode 路径的精确政策必须在 contract-first
阶段审批并做三平台测试，不能用显示字符串充当唯一身份。

### 项目读取 DTO

建议 normalized `ProjectSnapshot` 只包含：

- `projectId`（已注册项目才有）、root path、display name；
- project type、Unity editor version 与可选 revision；
- VPM/UPM manifest 的 present/valid/missing 状态；
- direct/locked dependency 的有界字符串摘要，不做 SemVer 或 resolver 判断；
- observed timestamp、project revision 和 bounded structured issues。

不保存或返回原始未知 JSON、完整技术错误、目录枚举、环境变量或任意文件内容。完整路径只对
具有目标资源 scope 的 `projects.read` Principal 返回；普通错误和日志使用 project ID、相对组件
名与 `diagnostic_id`。

### 项目类型

项目所有者已批准按冻结行为基线逐项绑定 marker，枚举与优先级固定为：

```text
avatars
worlds
vpm-starter
upm-avatars
upm-worlds
upm-starter
legacy-sdk2
legacy-avatars
legacy-worlds
unknown
```

marker 依次为 VPM locked `com.vrchat.avatars`、VPM locked `com.vrchat.worlds`、任意 VPM locked、
UPM `com.vrchat.avatars`、UPM `com.vrchat.worlds`、UPM `com.vrchat.base`、
`Assets/VRCSDK/Plugins/VRCSDK2.dll`、`VRCSDK3.dll`、`VRCSDK3A.dll`，最后 `unknown`。M3 不因为
包版本范围、Unity compatibility 或网络 catalog 改变项目类型。

### 注册与刷新

为形成真实持久读取切片，本草案建议 M3 包含只修改 `state.db` 的低影响命令：

- register exact project root；
- refresh one registered project snapshot；
- unregister one project record，但永不删除或修改项目目录。

register/refresh 必须先在 transaction 外读取并验证所有项目文件，再在短 transaction 内比较
normalized snapshot、提交 revision/Event。失败保留旧快照。unregister 的幂等、Event 和历史
可见性需随 Schema 一并冻结。若项目所有者决定 M3 必须绝对不包含任何注册表写命令，则这些
命令和项目表应同时移出 M3，M3 只能提供无持久化的 `projects.inspect`；不能保留无生产入口的
空 Schema 或用查询隐式 upsert。

## VPM / UPM / ProjectVersion 只读解析边界

- `ProjectVersion.txt`：只读取 `m_EditorVersion` 与可选 revision；缺失、重复、畸形和超限均为
  稳定错误或 issue，不猜测版本。
- `Packages/vpm-manifest.json`：只解析 M3 项目类型与 direct/locked 摘要所需字段；不修复旧值，
  不解析 dependency range 语义。
- `Packages/manifest.json`：只解析顶层 dependencies 的字符串身份；未知字段忽略但不写回。
- M3 不枚举/解析 installed package 的 `package.json`，不区分完整 locked/unlocked 安装状态；
  这属于 M4 package model。
- 每个输入设置独立 byte limit，使用只读打开和 bounded read；JSON recursion/collection/字符串
  数量限制在合同测试中冻结。建议起点为 ProjectVersion 64 KiB、每份 project manifest 4 MiB，
  最终数值须经人工审批。
- malformed JSON 的公开错误只提供 component kind、稳定 code 和安全的 line/column；原始内容、
  绝对路径和 parser debug text 不进入普通响应或日志。

## Repository 来源、获取、解析与统一边界

### 统一来源模型

application 只看 transport-neutral source descriptor：

```text
RepositorySource
├─ LocalFile { absolutePath, identityKey }
└─ RemoteHttp { url }
```

`alcomd-application` 定义 document-source port；`alcomd-vpm` 组合 local/remote loader 与同一个纯
parser。parser 只接受 bounded bytes 和非敏感 source context，不知道 SQLite、RPC、CLI 或
credential。local/remote 不得各自维护不同 repository 语义。

### Repository JSON 的 M3 解析范围

M3 解析并规范化：repository declared id/name/url、package map key、version map key、display name、
description、yanked 与 Unity version 字符串。它不解释 SemVer、dependency range、package URL、
hash、headers、legacy replacement 或 Unity compatibility。map key 与 manifest identity 不一致、
单 package/version malformed 等以 bounded issue 记录；顶层结构无效则整个 refresh 失败。

包身份输出按 package ID 与原始 version 字符串稳定排序；M3 不选择 latest、不合并跨来源同版本、
不冻结 M4 的 repository precedence。完整 package manifest 语义和确定性版本选择留给 M4。

### Refresh 与 cache

远程 repository refresh 和 last-known-good metadata cache 属于 M3，因为没有成功快照就无法提供
稳定的远程只读查询。建议边界：

- 只允许用户显式注册并刷新 `http`/`https`；不自动联网、不在 daemon 启动时后台刷新；
- 支持 ETag/Last-Modified 条件请求与 200/304；304、内容相同和失败均不增加 revision；
- response body 在读取过程中执行 byte limit 和总超时；建议上限 16 MiB、总超时 15 秒、有限
  redirect，最终数值和 redirect 规则需审批；
- parse 完整成功后，才在一个短 SQLite transaction 中替换 normalized catalog；
- HTTP、解析或提交失败保持上次成功快照，不保存部分 catalog；
- local source 走相同 parse/commit 路径，但没有 HTTP validator；
- M3 cache 只包含 normalized repository/package identity read model，不包含 ZIP、sidecar、解压
  staging 或 package payload。

## 网络与 credential 边界

- M3 remote loader 不接受 Authorization、cookie、自定义 header、userinfo URL 或 credential ID。
- repository/package manifest 中出现 header/credential 字段时不得写入日志、错误、导出或普通
  DTO；M3 可返回 `repository_credentials_unsupported` 或记录安全 issue，但不能静默启用。
- M3 不实现 credential store、认证 repository、跨 origin credential redirect、proxy credential
  或 `vcc://` 导入。
- 测试使用进程内/本机 deterministic mock HTTP，不依赖公共互联网，不对第三方服务执行安全或
  攻击性请求。
- timeout、body limit、scheme allowlist 和 redirect limit 是可靠性边界；不得为 M3 建立通用
  HTTP gateway、认证框架或完整 SSRF policy。外部 Principal 真正可添加 URL 前，仍需后续权限/
  配对威胁模型复核。

## SQLite / state.db 最小 Schema v2

contract-first 阶段建议在 `0001_state.sql` 之后新增单独的 `0002` migration，不修改已发布的 v1
快照。最小逻辑表：

```text
projects
├─ project_id / root_path / path_identity_key (unique)
├─ normalized snapshot fields or bounded snapshot_json
├─ revision
└─ registered_at_ms / observed_at_ms / updated_at_ms

repositories
├─ repository_id / source_kind / source_locator / source_identity_key (unique)
├─ declared_id / name / etag / last_modified
├─ revision
└─ registered_at_ms / refreshed_at_ms / updated_at_ms

repository_package_versions
├─ repository_id (FK)
├─ package_id / version_text
├─ bounded display fields / yanked / unity_text
└─ PRIMARY KEY(repository_id, package_id, version_text)
```

不增加 package ZIP/cache、dependency、Plan、Unity install、template、credential、activity 或 v3
migration 表。项目 snapshot 只存 ALCOMD normalized DTO；repository 不保存原始不可信 JSON 或
header。所有路径/URL/JSON/字符串/行数有 CHECK limit。

M2 `events.aggregate_kind` 当前只允许 `operation`。Schema v2 如需要 project/repository Event，
必须在同一原子 migration 中安全重建该表，将允许集合兼容扩展为 `operation/project/repository`，
完整保留既有 sequence、索引和 `sqlite_sequence`；迁移失败回滚全部变化。

低影响同步 registry command 的幂等不能伪装成 Operation。M2 `idempotency_records.operation_id`
当前必需，因此合同阶段必须人工选择并冻结以下之一：

1. 推荐：兼容扩展记录，使 completed synchronous command 可没有 OperationId，同时保留 M2
   pending/Operation 不变量；或
2. M3 不提供 registry command，只提供无持久状态的 inspect query。

不得为绕过此决策创建重复的临时幂等系统，也不得把普通 register/refresh 虚构为高影响工作流。

## Revision、Event、资源锁与事务

- 新 Project/Repository revision 从 1 开始；normalized snapshot 或注册元数据实际变化才递增。
- update/unregister 携带 `expectedRevision`；create/register 携带 idempotency key。
- Event 与聚合变更同 transaction 提交，aggregateRevision 等于提交后 revision。
- 建议 Event kind：`project.registered/refreshed/unregistered` 与
  `repository.registered/refreshed/unregistered`；精确名称需人工批准。
- 扩展 `ResourceKey::Project(ProjectId)` 与 `ResourceKey::Repository(RepositoryId)`，复用 M2
  canonical ordering；同一资源 refresh 串行，不同资源可并行。
- 文件/网络 I/O、JSON parse 与 lock wait 不得发生在 SQLite transaction 内。资源锁可覆盖一次
  refresh 的读取/比较/提交，防止同一资源结果乱序。
- M3 仅支持用户显式、低频 refresh，继续保留 M2 Event 不清理策略。引入 background watcher、
  自动 refresh 或高频事件前，必须另行冻结 retention/compaction 与 `event_cursor_expired`。

## RPC v1 兼容新增草案

建议 capability 分为：

```text
projects.read.v1
projects.registry.v1
repositories.read.v1
repositories.registry.v1
```

建议最小方法：

```text
projects.inspect
projects.list
projects.get
projects.register        # 仅 state.db；待同步幂等决策批准
projects.refresh         # 仅 state.db snapshot；不写项目
projects.unregister      # 仅 state.db；不删目录

repositories.inspect
repositories.list
repositories.get
repositories.packages   # raw identity/display page，不选择 latest
repositories.register   # source descriptor + initial successful snapshot
repositories.refresh
repositories.unregister # 不删 local source 文件
```

如果审批决定移除 registry command，则同时移除对应 capability/method/Schema。`inspect` 是无状态
query；`list/get/packages` 只读成功快照。所有列表使用稳定排序、opaque/exclusive cursor、默认
100/最大 1000，并遵守 4 MiB RPC frame 上限。

建议冻结的 M3 稳定错误至少包括：

```text
project_not_found
project_not_registered
project_already_registered
project_inaccessible
project_version_missing
project_version_invalid
project_manifest_invalid
repository_not_found
repository_already_registered
repository_source_invalid
repository_inaccessible
repository_unavailable
repository_document_invalid
repository_document_too_large
repository_credentials_unsupported
revision_conflict
idempotency_conflict
permission_denied
internal_error + diagnostic_id
```

错误 data 只能包含对应 Schema 明确允许的非敏感字段。新增 method、capability 和可选 DTO 字段是
RPC v1 兼容增加；改变既有 M1/M2 字段或语义仍属于破坏性变化。

## Principal 与权限

M3 建议使用既有权限草案中的：

- `projects.read`：inspect/list/get 和读取被授权项目路径/快照；
- `projects.manage`：register/refresh/unregister ALCOMD registry，不授权写项目；
- `repositories.read`：inspect/list/get/packages；
- `repositories.manage`：register/refresh/unregister source，不授权 credential 或 package 下载。

这些公共权限名称和 resource scope 必须在生产实现前人工批准并同步规范。M2
`builtin:local-owner` 可按批准结果获得 M3 最小权限，但 capability、client metadata、路径、URL、
ProjectId 或 RepositoryId 都不能证明身份。真实 credential enrollment/revocation 仍未实现，
`access.principal-revocation` 保持 `planned`。

## 项目与 repository 错误模型

- missing：目标不存在或必要组件缺失，返回对应稳定 code；可选 manifest 缺失可成为 snapshot
  issue，不得与 ProjectVersion 缺失混淆。
- malformed：完整 bounded bytes 已取得但格式无效，返回 component kind、line/column 或 bounded
  issue；不返回原文。
- inaccessible：权限、sharing violation、所有权/路径检查失败；不得通过 parent fallback 或环境
  变量猜测掩盖。
- changed-during-read：读取前后 metadata 不一致时本次 snapshot 失败或有限重试一次，不能组合
  来自不同版本的文件后提交。
- repository partial entry：顶层有效时，坏 package/version 可按获批策略隔离为 issue；所有 issue
  有数量上限和 deterministic ordering。
- internal：未知错误只返回 `internal_error + diagnostic_id`，日志遵守路径与 credential 脱敏。

## v3/vrc-get 参考与独立实现

- v3/vrc-get 只提供功能行为、公开格式、风险输入和 Fixture 设计参考；不得复制、移植、包装、
  Fork 或改写其源码。
- 可以使用许可证兼容、维护良好的通用 Rust 库承担 HTTP、URL、JSON 等通用能力，避免重复造轮子；
  依赖本身不把 v3/vrc-get 变成代码上游。
- 不继承 vrc-get 的 VCC 数据路径、cache 布局、HashMap 来源顺序、读取时修复 manifest、明文 header
  或查询副作用。
- M3 不对冻结参考实现执行新的攻击性黑盒、网络安全、credential 传播或故障注入；测试对象只
  是 ALCOMD 自身实现，网络使用本地 mock。

## `projects.v3-parity` 的 blocked 处理

M11 前没有经项目所有者确认的真实 v3.4.0 脱敏项目 Fixture，因此：

- `projects.v3-parity` 保持 `blocked`，不得改为 implemented/verified；
- M3 使用仓库自有的合成公开 Unity/VPM/UPM Fixture 验证 discovery、parser、type、readonly 和
  registry，不把它们描述为 v3 差异证据；
- M3 可以完成“实现切片”的工程验收，但 `projects.management` 与完整 v3 parity 仍保持
  `in_progress`/blocked，直到 M11 Fixture 建立并执行差异测试；
- `repositories.vpm-parity` 与 `repositories.fixture-matrix` 可使用公开合成 repository Fixture，
  但只标记 M3 实际覆盖的 read/refresh/cache 子集；add/import/header/deep-link/credential 等完整
  管理能力不得提前标为 implemented。

## 依赖方向与内部接口

固定方向：

```text
alcomd-cli -> alcomd-client -> alcomd-protocol
alcomd -> alcomd-application -> alcomd-domain
alcomd -> alcomd-store
alcomd -> alcomd-vpm
alcomd-store -> alcomd-application / alcomd-domain
alcomd-vpm -> alcomd-application / alcomd-platform # 仅实现读取 ports 与平台对象身份
```

禁止：

```text
alcomd-protocol -> domain/application/SQLite/filesystem/HTTP
alcomd-application -> RPC/SQLite/reqwest/OS APIs
alcomd-domain -> SQLite/HTTP/Tauri/OS APIs
alcomd-vpm -> alcomd-store/RPC/CLI/GUI
CLI -> project files/repository files/HTTP/SQLite
```

application 定义 `ProjectInspector`、`RepositoryDocumentSource`/catalog port 与 use cases；
`alcomd-vpm` 实现 bounded loader/parser；`alcomd-store` 只持久化 normalized DTO。不得为 M3 建立
通用 repository framework、service locator、ORM、插件式 parser registry 或完整 VPM engine。

## 已批准的生产依赖

项目所有者批准 `reqwest = { version = "=0.13.4", default-features = false, features = ["rustls"] }`
作为 M3 唯一新增生产 crate，并批准 Tokio 增加实际使用的 `fs` feature；不得启用 `full`。URL 使用
`reqwest::Url`，不增加 `url` 直接依赖。HTTP client 固定 no-proxy、no-cookie、no-referer、15 秒总
timeout、5 次 redirect、逐跳 scheme/userinfo 校验与 16 MiB 实际 body 累计限制。

生成 Cargo.lock 后必须检查精确新增 package；出现批准依赖解析之外的意外 production package、
另一 HTTP/TLS stack 或新增直接依赖时停止审批。不得引入通用 RPC/HTTP server、ORM、SemVer/
resolver、watcher、credential、cache framework 或 async SQL crate。

## 测试与验收

### 单元与合同测试

- ProjectVersion、VPM、UPM 与 repository JSON 的正常、missing、malformed、BOM/null、duplicate、
  超限、深度和 issue ordering Fixture。
- exact-root/parent search、type precedence、路径 identity 与 snapshot deterministic serialization。
- repository 顶层失败与单 entry 隔离、stable package/version ordering、无 SemVer/latest 选择。
- RPC Schema/golden、capability、permission、pagination/cursor、错误 data 与未知可选字段兼容。
- state Schema v1 -> v2、events table 保序重建、较新版本拒绝、重复启动与失败回滚。

### 集成测试

- register/refresh/unregister 的 revision、expectedRevision、幂等重放、Event 原子性和 no-op。
- 同项目/仓库 refresh 串行、不同资源并行，I/O 不在 SQLite transaction 内。
- project query 前后对 ProjectVersion/VPM/UPM 做内容、mtime 和 tree snapshot，证明零写入。
- local repository 与 remote mock 走同一 parser；200/304、ETag、timeout、body limit、redirect limit、
  malformed 与 last-known-good rollback。
- daemon/CLI/client 真实 IPC：重启后 list/get/cache 可读，旧 M1/M2 client 继续工作。
- 日志/错误/DTO 不含 header、userinfo、原始文档、无权限完整路径或 parser debug 文本。

### 三平台 hosted 验收

- Windows Server 2025：NTFS 路径大小写/绝对身份、共享冲突、ProjectVersion/VPM/UPM、local mock
  HTTP、SQLite migration/revision/Event 与 Tauri no-bundle。它不替代 Win10/11 客户端发行验证。
- Ubuntu 22.04：Unix permission/symlink/非 Unicode 政策、local/remote repository、SQLite 与真实
  IPC；release ELF 继续要求最高 `GLIBC <= 2.35`。
- macOS 15 arm64：路径/symlink、local/remote repository、SQLite 与真实 IPC；预期产物继续验证
  arm64 / minos 11.0。
- 三平台继续运行 setup/check/test、release executables、Tauri no-bundle、三份锁文件与 final diff。

M3 实施时应在 `test-plan.toml` 增加或细化 synthetic readonly/registry/cache 测试，但不得把
`projects.v3-parity`、`access.principal-revocation` 或完整 `cli.complete` 错标为 implemented。

## Release blocker 与风险

- **读取即写入回归**：任何 parser/load query 写 manifest、创建目录或隐式 upsert 都阻塞 M3。
- **路径身份重复**：Windows case/symlink 与 Unix byte path 未冻结会导致同项目重复注册。
- **大 repository**：必须在读取与 parse 前后都有界，RPC 使用分页，不能整库塞入单帧。
- **旧 cache 污染**：失败/partial refresh 不能覆盖最后成功快照或错误提升 revision。
- **秘密持久化**：raw JSON/header/userinfo 不进入 state.db、日志、Event 或普通 DTO。
- **Schema v2 迁移**：重建 Event table 必须保持 M2 sequence/Operation 可恢复性。
- **幂等模型偏离**：同步 registry command 不能绕过 M2 command retry 契约或伪造 Operation。
- **parity 证据缺口**：synthetic Fixture 不能冒充 M11 的真实 v3 差异证据。
- **过度实现 VPM**：SemVer、resolver、package URL/hash、download/ZIP 与 Plan/Apply 必须留在 M4。
- **平台证据误用**：Windows hosted 仍不是 Win10/11 完整客户端发行验证。

## 已批准合同与下一审批点

项目所有者已于 2026-08-18 批准：

1. “外部源只读、state.db registry 可写”的 M3 定义，以及 register/refresh/unregister 是否纳入。
2. ProjectSnapshot、project type 枚举、发现模式、路径 identity 与 non-Unicode/symlink 政策。
3. RepositorySource、normalized catalog、单 entry error isolation 与 refresh/cache 边界。
4. Schema v2 的精确 SQL、Event table 兼容重建和同步 command 幂等方案。
5. RPC method/capability/DTO/error、pagination 与兼容快照。
6. `projects.read/manage`、`repositories.read/manage` 的名称、scope 和 builtin local-owner 授权。
7. HTTP(S) scheme、size/timeout/redirect/validator 行为，以及 remote refresh 是否是 M3 blocker。
8. 新生产依赖的精确版本/features、许可证、Cargo.lock diff 与 Tokio feature 变化。
9. M3 synthetic Fixture 可完成工程里程碑、但 `projects.v3-parity` 保持 blocked 到 M11 的验收解释。

上述决定采用 contract-first：先更新 ADR、RPC/storage Schema、migration snapshot 和合同测试；
合同通过后才可写生产实现。仅在新增生产 crate、Windows 路径身份需要扩大 windows-sys/unsafe、
Cargo.lock 出现非预期依赖、偏离冻结合同或进入 M4 时再次停止审批。

## 最小实施顺序

1. 冻结 M3 ADR、RPC/storage Schema、错误、权限、路径与网络/依赖合同。
2. 先写 synthetic project/repository parser、readonly 与 migration failure 合同测试。
3. 实现纯 parser 与 application ports，不接 daemon、不写 DB。
4. 实现 Schema v2/store registry/revision/Event，并验证 migration 与 transaction。
5. 接入 local project/local repository 的 RPC/client/CLI 端到端切片。
6. 获批 HTTP 依赖后接入 remote refresh、validator 和 last-known-good cache。
7. 运行完整本地与三平台验收，更新真实状态，提交推送最终候选。

不得用先搭建完整 VPM engine、通用 repository framework 或后台 refresh 服务替代上述顺序。

## 与 M2、M4、M11、M12 的关系

- M3 复用 M2 的唯一 SQLite owner、short transaction、revision、Event、Principal 与 Resource Lock；
  不另建状态库、事件总线或授权系统。
- M4 在 M3 normalized project/source identity 上增加 package manifest、SemVer/range、catalog
  precedence、Plan/Apply、download/cache/ZIP 与项目写事务。
- M11 建立真实 v3.4.0 脱敏 Fixture 后，才解除 `projects.v3-parity` 及相关迁移差异测试 blocker。
- M12 负责完整产品数据目录、安装/升级/卸载、Win10/Win11 客户端和发行验证；M3 hosted 结果
  不替代这些证据。

## M3 完成后的停止条件

1. 所有获批合同、Schema v2、migration、readonly、registry/cache 和三平台测试通过。
2. `feature-parity.toml` 与 `test-plan.toml` 只标记 M3 实际实现子集；上述四个完整功能面和
   `projects.v3-parity`/credential/revocation 等保持真实未完成状态。
3. 提交并推送最终候选，确认 HEAD、`origin/main` 与 CI head SHA 一致且工作树干净。
4. 状态设为“M3 等待人工验收”，不得创建或执行 M4 生产实现。
5. 项目所有者明确验收 M3 后，才能进入 M4 规划或实现。

## 进度日志

- 2026-08-18：M2 最终提交 `9076574ef0f4d3de8690865dfb18aa5856d7ad64` 与 GitHub Actions
  run `32144082427` 已由项目所有者人工验收；M2 正式完成。
- 2026-08-18：创建本 M3 ExecPlan 草案，只规划项目与 VPM Repository 只读垂直切片；未修改
  RPC/storage Schema、permission、Cargo 依赖、migration 或生产代码，等待项目所有者审批。
- 2026-08-18：项目所有者批准 M3 总体方向、Schema v2、同步幂等、RPC/权限、路径身份、bounded
  parser、anonymous HTTP(S) refresh 与精确 `reqwest 0.13.4` 配置；进入 contract-first 阶段。
- 2026-08-19：ADR 0016、RPC/storage Schema、权限、错误、Schema v2 migration 与合同测试完成；
  随后实现 bounded project/repository reader、平台文件身份、SQLite registry、RPC/client/CLI 和
  synthetic 真实 IPC 测试。外部项目与 repository 文件保持只读，M3 未实现任何 M4 package 操作。
- 2026-08-19：项目所有者批准 `reqwest 0.13.4 + rustls` 的 38 个传递锁定项。feature graph 实测
  `reqwest` 仅由 `alcomd-vpm` 激活 `rustls` 路径，`aws-lc-rs` 是该 TLS provider 的 active build
  dependency；`quinn` 与 `ring` 无反向依赖输出，HTTP/3、system-proxy、cookie 与压缩 feature
  均未激活。Cargo.lock 中未激活的 optional/target package 不描述为运行时组件。
- 2026-08-19：Windows 本机最终正式 `check.ps1`、`test.ps1`、Tauri `build --no-bundle`、冻结基线、
  metadata、format/clippy、全 Workspace/Discord/frontend 测试与三份锁文件摘要门禁通过。当前仍
  等待最终提交对应的三个 hosted job，不能将本地结果描述为三平台验收完成。
- 2026-08-19：首个 M3 候选 `5a2722bd51ecb47a73708217726e2f161d6e9ae6` 的 CI run
  `32170852025` 在 macOS `Check` 暴露测试夹具使用长系统 `TMPDIR` 导致 Unix socket 超过 portable
  路径上限；生产端正确拒绝该路径。夹具改用短的 `/private/tmp` 隔离根，本地完整门禁重新通过，
  等待修复提交的三平台结果。
- 2026-08-19：修复后生产候选 `ea22e3bf8d90b7924712141a66abe6c701fe3474` 对应 CI run
  `32171689291` 的 Windows Server 2025、Ubuntu 22.04 与 macOS 15 arm64 job 全部成功；Ubuntu
  实测最高 `GLIBC_2.34`，macOS 九个预期 Mach-O 产物全部为 `arm64 / minos 11.0`，三平台
  Tauri no-bundle、三份锁文件和最终 diff 门禁均通过。M3 现在停止并等待项目所有者人工验收。
- 2026-08-19：项目所有者确认最终提交 `2082b5596d246975ca7a48dab20826899103e03d` 与
  GitHub Actions run `32174028968` 人工验收通过；Windows Server 2025、Ubuntu 22.04 与
  macOS 15 arm64 全部成功，CI head SHA、`HEAD` 与 `origin/main` 一致。M3 正式完成，尚未
  开始 M4 生产实现。
