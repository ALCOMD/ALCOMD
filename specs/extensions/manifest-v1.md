# ALCOMD Extension Manifest v1

状态：M7 active Manifest v1；由 pre-release direct replacement 实现，不保留旧 Web UI shape。

扩展包后缀是 `.alcomdextension`，根 Manifest 固定为 UTF-8、无 BOM 的 `alcomd-extension.toml`。解析后的
对象必须满足 `manifest-v1.schema.json`；未知字段 fail closed。

## 最小示例

```toml
schema = 1
id = "dev.example.project-summary"
name = "Project Summary"
version = "1.0.0"
api = 1
publisher_name = "Example Publisher"
publisher_key_fingerprint = "ed25519-sha256:0000000000000000000000000000000000000000000000000000000000000000"
license = "MIT"

[entrypoints]
component = "component/extension.wasm"

[ui]
protocol = "portable-v1"

[interfaces]
required = [
    "alcomd:extension/host-projects@1.0.0",
    "alcomd:extension/host-data@1.0.0",
]
optional = []

[permissions]
required = ["projects.read"]
optional = ["background.run"]
```

`publisher_name` 仅用于显示，不是身份。`publisher_key_fingerprint` 必须与签名 envelope 中的公钥相符；
Manifest 不允许 `first_party`、trust 或 grant 字段；不要求额外发布者信任确认，签名与身份校验保持。

## Identity 与版本

- `id` 是 3-255 ASCII bytes 的 lowercase reverse-DNS，segment 以字母或数字开始和结束，可含内部 `-`。
- `version` 是不含前导 `v` 的完整 SemVer，最多 128 bytes；build metadata 是 package identity 的一部分。
- `api = 1` 表示 Extension ABI major 1。Manifest 不能用范围隐式接受另一 major。
- required/optional interface 使用完整 versioned WIT interface ID，最多各 32 项。required 不可满足时不能 enable；
  unknown optional interface 可忽略。
- package identity 是 `package-signing-v1.md` 定义的 content-tree SHA-256，不由文件名、安装路径或 ZIP bytes 决定。

## Background integration targets

The background integration world is requested by declaring `alcomd:extension/guest-background@1.0.0`, `alcomd:extension/host-local-ipc@1.0.0`, and `alcomd:extension/host-unity-sessions@1.0.0` in `[interfaces].required`. Partial declarations and optional declarations of these three interfaces are rejected.

`local_ipc_targets` is a signed table of up to eight named targets. A manifest with targets declares `local-ipc.connect` as optional and lists `background.run`, `projects.read`, and `unity.sessions.read` as required or optional. Every target supplies a `label` and at least one platform section. Names are sorted, unique literal basenames.

| Platform section | Fields |
| --- | --- |
| `windows` | `pipe_names`, `executable_name`, and the exact Authenticode signer organization in `publisher` |
| `macos` | `socket_names`, `bundle_identifier`, and `team_identifier` from the Apple signing identity |
| `linux` | `socket_names` and the canonical absolute `system_executable` installed in a root-owned non-user-writable directory tree |

The scope key is lowercase SHA-256 over `alcomd-local-ipc-target-v1`, target ID and label, followed by the present platform sections in Windows, macOS, Linux order. Each section contributes its lowercase name, the identity values in table order, the endpoint names in sorted order, and one empty terminator. Every field is encoded as UTF-8 with a trailing NUL. Signed values contain no NUL. Target declarations are informational until an explicit grant is committed.

## Entrypoint 与 UI

- `component` 必须精确为 `component/extension.wasm`。`background_component` 与 `ui_entry` 是未知字段并 fail closed；
  不存在 alias、deprecated parser 或 compatibility warning。
- `[ui]` 可选；存在时只允许 `protocol = "portable-v1"`，表示唯一隐式 main Portable UI。它不包含页面 identity、
  URL、asset、framework、renderer 或 GUI placement。没有 `[ui]` 的 backend extension 仍完整有效。
- 所有 ABI v1 Component 都必须实现 `guest-ui`；没有 `[ui]` 时 daemon 永不调用，官方 SDK/reference guest提供空桩。
- native DLL、`.so`、`.dylib`、script/shell entrypoint 和 static Web UI asset 均禁止。
- entrypoint 经过 `package-profile-v1.json` 的 normalized path、type、digest 和 quota 检查。

## 权限

- Manifest 只声明 permission name，不包含 scope 或 grant。scope 由 daemon grant authority 保存。
- 全部当前可授权扩展权限均可放入required或optional，二者互斥；包括mcp.tools.provide。required未授予时不能enable/打开UI；optional未授予时对应Host调用拒绝，不自动授权。
- 声明用于说明安装需求，不再是用户显式授权的上限；未声明但可授权的权限也可在管理页开启。接口、工具声明及调用者自身权限仍独立验证。
- `background.run` 不由 `[ui]` 隐式添加。UI-only extension 可不请求该权限，并由 interactive UI session按需启动。
  Project read grant 使用 `Global/*`，覆盖当前及以后登记项目的安全摘要，不逐ProjectId授权；
  它不允许 guest 读取项目路径、项目文件或 `state.db`。
- `network.request`、filesystem、clipboard、notification、Discord 与 private GUI authority 不属于本合同。

## 长度与确定性

- Manifest raw bytes 最大 65,536；string 禁止 NUL/control character，数组顺序不产生授权优先级。
- required/optional permission 各最多 64 项、去重且 lexicographic canonical order。
- required/optional interface 各最多 32 项、去重且 lexicographic canonical order。
- 同一 ExtensionId + version + publisher fingerprint 但 package digest 不同是供应链冲突，不得静默覆盖。

## 安装条款与明确同意

可选`[installation_terms]`只有`title`与`text`：title非空、最多128 Unicode字符、禁止控制字符；text非空、UTF-8最多16384字节，仅允许换行/回车/制表控制字符。它属于原签名Manifest，不读取远端文本，不执行HTML/Markdown/脚本。不存在时跳过条款步骤。

Plan冻结条款与权限声明。GUI先确认作者条款，再集中确认required权限；最后一次确认才调用带consent的applyInstall。拒绝不提交，不产生安装/授权Operation。Core以相同Plan fingerprint验证同意；安装登记与初始required grants同事务提交；optional/未声明默认关闭。条款同意不构成包安全认证，不替代签名、沙箱及双Principal检查。用户发起的安装完成后仍为 `installed_disabled`。随包入口仅接受没有安装条款和必需权限的已验证包。
