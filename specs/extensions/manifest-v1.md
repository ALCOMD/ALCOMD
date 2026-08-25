# ALCOMD Extension Manifest v1

状态：M7 active Manifest v1；由 pre-release direct replacement 实现，不保留旧 Web UI shape。

扩展包后缀是 `.alcomdext`，根 Manifest 固定为 UTF-8、无 BOM 的 `alcomd-extension.toml`。解析后的
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
可信与 first-party 状态由 daemon policy 决定，Manifest 不允许 `first_party`、trust 或 grant 字段。

## Identity 与版本

- `id` 是 3-255 ASCII bytes 的 lowercase reverse-DNS，segment 以字母或数字开始和结束，可含内部 `-`。
- `version` 是不含前导 `v` 的完整 SemVer，最多 128 bytes；build metadata 是 package identity 的一部分。
- `api = 1` 表示 Extension ABI major 1。Manifest 不能用范围隐式接受另一 major。
- required/optional interface 使用完整 versioned WIT interface ID，最多各 32 项。required 不可满足时不能 enable；
  unknown optional interface 可忽略。
- package identity 是 `package-signing-v1.md` 定义的 content-tree SHA-256，不由文件名、安装路径或 ZIP bytes 决定。

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
- required permission 未授予时不能 enable；optional 未授予时 Host 不链接对应 optional interface。
- `background.run` 不由 `[ui]` 隐式添加。UI-only extension 可不请求该权限，并由 interactive UI session按需启动。
  Project read grant 必须
  绑定一个或多个 specific ProjectId；它不允许 guest 读取项目路径、项目文件或 `state.db`。
- `network.request`、filesystem、clipboard、notification、Discord 与 private GUI authority 不属于本合同。

## 长度与确定性

- Manifest raw bytes 最大 65,536；string 禁止 NUL/control character，数组顺序不产生授权优先级。
- required/optional permission 各最多 64 项、去重且 lexicographic canonical order。
- required/optional interface 各最多 32 项、去重且 lexicographic canonical order。
- 同一 ExtensionId + version + publisher fingerprint 但 package digest 不同是供应链冲突，不得静默覆盖。
