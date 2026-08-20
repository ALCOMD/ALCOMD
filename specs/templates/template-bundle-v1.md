# ALCOMD Template Bundle v1

状态：Accepted contract；生产 reader/writer 尚未实现

文件扩展名固定为 `.alcomdtemplate`。v3 `.alcomtemplate` 不属于本格式。

## ZIP profile 与 quota

所有数值都是 hard limit，等于上限时允许，超过即以 `template_bundle_invalid` 或更具体的稳定错误拒绝，
不得截断：

| 项目 | v1 上限 |
|---|---:|
| 完整 ZIP compressed bytes | 2,147,483,648 |
| entry count（含目录） | 100,000 |
| 单 entry uncompressed bytes | 2,147,483,648 |
| total uncompressed bytes | 8,589,934,592 |
| normalized path depth | 64 segments |
| normalized UTF-8 path length | 1,024 bytes |
| 每个非空 entry expansion ratio | 1,000:1 |

ratio 以 entry uncompressed bytes / compressed bytes 计算；非空 entry 的 compressed size 为零即拒绝。
完整文件大小在打开 ZIP 前检查；central-directory preflight 与 streaming extraction 都重新计数。仅 Stored、
Deflate；encrypted、其他 codec 和 data-descriptor 声明不可信，实际读取仍受相同 byte quota。

路径必须是 NFC-normalized UTF-8 root-relative `/` separated path。M4 的 absolute/device/UNC/drive/ADS、
`.`/`..`、control/NUL、Windows reserved/trailing-dot-space、case/Unicode/file-directory collision、link、
reparse、hardlink 和 special-file 拒绝规则全部适用。目标必须是 operation-owned empty staging。

## 根布局

- 根必须恰有一个 regular file `template.json`，最大 1,048,576 UTF-8 bytes，无 BOM。
- 其余 entry 只能位于 `payload/` 或 `resources/`；两个目录都不得以文件占位。
- `resources/` 下每个 regular file必须在 `additionalResources` 中恰好声明一次，反之亦然。
- `payload.root` 固定为 `payload/`。payload 不得为空，且必须包含
  `ProjectSettings/ProjectVersion.txt` 与 `Packages/manifest.json`。
- 根目录任意其他文件/目录、未声明 resource 或 manifest 声明但不存在的 entry 均拒绝。

完整 object digest 是原始 ZIP bytes 的 SHA-256。payload tree digest 使用按 NFC UTF-8 path 升序排列的
regular files，对每项串联 `u32-le path-byte-length || path-bytes || u64-le content-length || content-bytes`
后取 SHA-256；目录不进入 digest。resource digest 是对应解压后 file bytes SHA-256。

## Derive include/exclude v1

遍历只接受 root 内 regular file/directory，任何 symlink、junction/reparse、hardlink 或 special file
fail closed。规则按 root-relative path 应用，大小写/Unicode collision 在读取时即拒绝。

始终包含：

- `Assets/**`
- `ProjectSettings/**`
- `Packages/manifest.json`
- `Packages/vpm-manifest.json`（存在时）
- `Packages/packages-lock.json`（存在时）
- `Packages/<embedded-package>/**`，仅限未被 normalized VPM locked set 标识为已安装 VPM package 的目录

条件排除：normalized VPM locked set 中、且可由冻结 dependency requirement 与 resolver-ready SHA-256
source 表达的 `Packages/<package-id>/**` 不进入 payload，改写入 `dependencies`。无法安全表达的 installed
package 使 derive 返回 `template_dependency_unsatisfied`，不得偷偷 embed 或丢弃。

始终排除：

- `.git/**`、`.hg/**`、`.svn/**`
- `.vs/**`、`.idea/**`、`.vscode/**`
- `Library/**`、`Temp/**`、`Obj/**`、`Logs/**`、`UserSettings/**`
- `Build/**`、`Builds/**`、`MemoryCaptures/**`、`Recordings/**`
- `Library/ALCOMD/**`（已被 Library 总规则覆盖，列出用于防止未来例外误纳入）
- 未列入始终包含集合的其他 root entry

v1 不保留任何 Library 子项。ProjectVersion、UPM/VPM manifest 和 ProjectSettings 不得因缓存排除规则丢失。

## Determinism 与隐私

manifest canonical form 使用 UTF-8、无 BOM、对象 key Unicode codepoint 升序、无 insignificant whitespace、
LF。Plan 和 fingerprint 使用 canonical form。ZIP writer 的 timestamp/permissions/compression 参数尚未作为
跨实现 ABI 冻结，因此 v1 只承诺 semantic deterministic；同一已验证 object 的直接 export 必须保持原始
bytes/digest。

manifest 和 export 禁止 credential、header/token、绝对原项目路径、object-store path、Principal、
state.db metadata、cache path 与 shell command。
