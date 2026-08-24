# ALCOMD Extension Manifest v1

状态：M6 contract-first Stop A candidate；production 尚未实现或广告。

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
background_component = "component/extension.wasm"

[interfaces]
required = [
    "alcomd:extension/host-projects@1.0.0",
    "alcomd:extension/host-data@1.0.0",
]
optional = []

[permissions]
required = ["background.run", "projects.read"]
optional = []
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

- `background_component` 如存在必须精确为 `component/extension.wasm`；第一条 M6 slice 要求存在。
- `ui_entry` 如存在必须位于 `ui/`，只声明 packaged static asset identity，不声明 sidebar/settings/toolbar/
  context-menu/navigation placement。
- 至少一个 entrypoint；native DLL、`.so`、`.dylib`、script/shell entrypoint 均禁止。
- entrypoint 经过 `package-profile-v1.json` 的 normalized path、type、digest 和 quota 检查。

## 权限

- Manifest 只声明 permission name，不包含 scope 或 grant。scope 由 daemon grant authority 保存。
- required permission 未授予时不能 enable；optional 未授予时 Host 不链接对应 optional interface。
- 第一条生产 slice 的 required permission 仅为 `background.run` 与 `projects.read`。Project read grant 必须
  绑定一个或多个 specific ProjectId；它不允许 guest 读取项目路径、项目文件或 `state.db`。
- `network.request`、filesystem、clipboard、notification、Discord 和 M7 UI placement 不属于第一条 slice。

## 长度与确定性

- Manifest raw bytes 最大 65,536；string 禁止 NUL/control character，数组顺序不产生授权优先级。
- required/optional permission 各最多 64 项、去重且 lexicographic canonical order。
- required/optional interface 各最多 32 项、去重且 lexicographic canonical order。
- 同一 ExtensionId + version + publisher fingerprint 但 package digest 不同是供应链冲突，不得静默覆盖。
