# ALCOMD Extension Manifest v1

状态：Draft

扩展包后缀：`.alcomdext`
清单文件：`alcomd-extension.toml`

最低字段：

```toml
schema = 1
id = "dev.example.extension"
name = "Example"
version = "1.0.0"
api = "^1"
publisher = "Example"

[entrypoints]
ui = "ui/index.html"
background_wasm = "backend/extension.wasm"

[activation]
background = false
events = []

[contributions]
sidebar = true
settings_page = false
command_palette = false

[permissions]
required = ["ui.contribute"]
optional = []
```

规则：

- ID 使用 reverse-DNS。
- 第一方身份由签名和官方发行源确定，不信任 Manifest 中的布尔字段。
- 路径必须相对扩展包根目录，不允许绝对路径和 `..`。
- 包安装前验证哈希、签名和路径安全。
- `api` 表示 Extension API 兼容范围。
- required 权限拒绝时扩展不能启用。
- optional 权限拒绝时扩展必须优雅降级。
