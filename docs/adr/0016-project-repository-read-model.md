# ADR 0016：M3 项目与 Repository 只读模型

状态：Accepted（2026-08-18）

## 决策

M3 提供项目与 VPM Repository 的只读垂直切片。这里的“只读”约束外部 Unity 项目与
repository source；`register`、显式 `refresh` 与 `unregister` 可以原子修改 ALCOMD 自有
`state.db` registry 和 last-known-good normalized cache。它们是低影响同步命令，不创建
Operation，也不修改外部源。

M3 仅支持本地项目、本地 repository 文件和匿名 HTTP(S) repository。远程 refresh 是 M3
blocker；不支持 credential、自定义 header、cookie、proxy、后台 refresh 或 package payload。

## 项目根与路径身份

- RPC 项目路径必须是绝对路径；CLI 可以先将相对路径解析为绝对路径再调用 RPC。
- `exact-root` 只检查指定目录；只有显式 `search-parents` 才向父级查找。
- symlink 指向的根先解析为真实文件系统对象；同一对象不能经 symlink、路径拼写或 Windows
  大小写差异重复注册。
- `path_identity_key` 是平台生成的 opaque binary identity，与 Unicode-lossless display/root path
  分开保存。Unix 非 UTF-8 路径返回 `path_encoding_unsupported`。
- 读取内部固定组件时必须阻止 symlink/reparse point 逃逸项目根或 local repository 根。
- 不用 lowercase 路径、用户名、环境变量或调用方 metadata 近似文件身份。

## 项目类型基线

项目类型按以下固定优先级识别；每项都来自冻结的 vrc-get 项目类型公开行为基线，不解释版本：

1. `avatars`：VPM locked 包含 `com.vrchat.avatars`。
2. `worlds`：VPM locked 包含 `com.vrchat.worlds`。
3. `vpm-starter`：VPM manifest 含任意 locked package。
4. `upm-avatars`：UPM dependencies 包含 `com.vrchat.avatars`。
5. `upm-worlds`：UPM dependencies 包含 `com.vrchat.worlds`。
6. `upm-starter`：UPM dependencies 包含 `com.vrchat.base`。
7. `legacy-sdk2`：存在 `Assets/VRCSDK/Plugins/VRCSDK2.dll`。
8. `legacy-worlds`：存在 `Assets/VRCSDK/Plugins/VRCSDK3.dll`。
9. `legacy-avatars`：存在 `Assets/VRCSDK/Plugins/VRCSDK3A.dll`。
10. `unknown`：以上 marker 均不存在。

这些 marker 只作只读分类。M3 不做 SemVer、依赖求解、Unity compatibility、latest 或远程查询。

## 有界输入

| 输入 | byte 上限 |
|---|---:|
| `ProjectVersion.txt` | 65,536 |
| `vpm-manifest.json` | 4,194,304 |
| `manifest.json` | 4,194,304 |
| repository JSON | 16,777,216 |

JSON 最大递归深度为 64；单 object/array 最大 16,384 项；单字符串最大 65,536 UTF-8 bytes；
每次 parse 最多产生 1,024 个 issue。超过 issue 上限使整个 parse 失败，不能截断后提交部分结果。
Project snapshot 最多 4,096 个 dependency identity。

## Repository 模型与 HTTP

local 与 remote source 共用同一个 bounded parser，输出按 package ID、原始 version string 和 issue
key 的 UTF-8 byte order 稳定排序。顶层结构无效使整个 parse 失败；顶层合法时，单个非法
package/version 只生成 bounded issue。M3 不判断 SemVer，不选择 latest，也不读取 package URL、
hash、dependency、header 或来源优先级语义。

Remote identity 使用 normalized registration URL：仅 `http`/`https`，保留 query、忽略 fragment、
拒绝 userinfo。redirect 的最终 URL 不改变注册 identity。HTTP client 固定：

- `reqwest 0.13.4`，`default-features = false`，只启用 `rustls`；
- 显式 `no_proxy()`、无 cookie、`referer(false)`、总 timeout 15 秒；
- 最多 5 次 redirect，每一跳重新校验 scheme/userinfo；拒绝 HTTPS 降级到 HTTP；
- 可跨 origin redirect，因为从不发送 credential；允许 HTTP 升级到 HTTPS；
- `Content-Length` 超限可提前拒绝，且仍用 `Response::chunk` 累计实际 body，严格限制 16 MiB；
- 200 完整 parse 成功后才提交；304 只有存在旧成功 snapshot 时有效；失败保留 last-known-good；
- 使用 ETag/Last-Modified 条件请求。validator 的变化本身不是 aggregate semantic change。

M3 不实现 proxy、SSRF gateway、认证 repository、credential store、package download 或通用 HTTP
framework。

## 状态、revision、Event 与幂等

Schema v2 只增加 `projects`、`repositories`、`repository_package_versions` 三张业务表；不修改
`0001_state.sql`。migration 原子重建 `events` 与 `idempotency_records`：

- Event aggregate kind 兼容增加 `project` 和 `repository`，保留 sequence、索引、排序和原
  `sqlite_sequence`。
- completed 同步 command 的 `operation_id` 可以为 null；pending 记录仍必须绑定 Operation。
- aggregate semantic state 真正改变时 revision 才递增并写 Event；no-op、304、validator-only 更新
  和失败不递增、不发 Event。
- unregister 只删 registry/cache row，以 expected revision 校验，并在同一 transaction 写 revision
  `+1` 的 unregistered Event 与 durable idempotency response。Event/idempotency history保留；之后
  re-register 产生新 ID。

外部文件/HTTP fetch 与 parse 必须在 SQLite transaction 外完成。最终短 transaction 顺序固定为：
幂等查验/保留、expected revision 校验、状态更新、Event、durable response、commit。同一 Principal、
method、key、fingerprint 重放原结果；同 key 不同 fingerprint 返回 `idempotency_conflict`。提交前
外部失败不消耗 key。并发 fetch 可以重复，但只有一次提交获胜。

## RPC 与权限

RPC v1 兼容增加 capability：`projects.read.v1`、`projects.registry.v1`、
`repositories.read.v1`、`repositories.registry.v1`，以及经批准的 `projects.*`、
`repositories.*` 方法。新增字段和 capability 是兼容增加；既有 M1/M2 方法不变。

权限固定为 `projects.read/manage` 与 `repositories.read/manage`。`manage` 只允许 ALCOMD registry
与 normalized cache，不允许写项目、下载包或使用 credential。M3 的 `builtin:local-owner` 获得
四项权限；完整外部 Principal resource scope 与 credential enrollment/revocation 留给后续里程碑。

## 后果与排除

M3 能提供真实的项目/repository inspect、registry 和 last-known-good 查询，但不会形成完整 VPM
引擎。包解析/求解/下载、Plan/Apply、Unity 写入、迁移、GUI、MCP、扩展与发行均不属于 M3。
`projects.v3-parity` 继续 blocked 到 M11 的真实脱敏 Fixture，不以 synthetic fixture 冒充通过。
