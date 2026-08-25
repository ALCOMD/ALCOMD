# Extension Manifest v1 Portable UI direct-rewrite proposal

状态：M7 Portable UI Stop A proposal；不是 active Manifest，也未被 production parser 使用。

Stop A 获批进入实现后，本 proposal 将原子替换 `specs/extensions/manifest-v1.*`，proposal 文件随即删除。它不是
Manifest v2、兼容层或第二个 parser。

## Exact shape

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

`background_component` 直接改名为 `component`，不保留 alias。`ui_entry` 被删除且不得忽略；遇到旧字段或任何未知
字段都 fail closed。一个 Component 同时承载 lifecycle 与可选 `guest-ui` export。

`[ui]` 可选。存在时只允许 `protocol = "portable-v1"`，并隐式声明唯一稳定 surface ID `main`；Manifest 不声明
surface title，title 由 Snapshot 返回。两个 M8/M9 synthetic fixture 已证明单一页面能表达本轮真实用例，因此 v1
不声明 surface 数组、动态 surface、GUI identity、URL、asset 或 renderer hint。

没有 `[ui]` 的扩展不需要导出 `guest-ui`。具有 `[ui]` 但不请求 `background.run` 的 UI-only 扩展仍可安装、启用并
由 `extensions.ui.open` 按需启动；`background.run` 不是 Portable UI 权限。

