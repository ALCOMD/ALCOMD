# Portable UI v1 exact limits

状态：M7 Stop A contract candidate；production 尚未实现。机器可读权威值在 `portable-ui-limits-v1.json`。

v1 只采用一个隐式 `main` surface。MCP management 与 Discord Presence synthetic fixture 均能在一个 page 中通过
section/list/form 表达；增加 surface identity、安装 Plan 集合和选择 API 没有当前收益。

Snapshot 上限 256 KiB、256 nodes、depth 8、总显示文本 64 KiB，足以覆盖两个 fixture，同时把 renderer 与 Host
验证成本固定在小范围。单项文本 4 KiB，select 64 options，form 64 fields。所有尺寸按 canonical compact UTF-8
JSON bytes 计算；Snapshot 上限覆盖 daemon wrapper，dispatch 上限覆盖完整 RPC params。

会话按 extension 8、connection 16、daemon 128；每 session 与每 Host 同时只执行一个 UI action/call。rate limiter
是容量 10、每秒补充 1 token 的 token bucket，即 60/minute、burst 10。成功 open/refresh/dispatch 重置 300,000 ms
idle deadline；3,600,000 ms absolute deadline 不重置。最后一个 UI session 关闭且没有 background lease 后，Host 在
5,000 ms 内停止。guest UI call 继续受 2,000 ms wall timeout 约束。

标识符按 UTF-8/ASCII 同值计数，必须匹配 `^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$`。signed integer 限制在
`[-9007199254740991, 9007199254740991]`，确保 JSON/JavaScript consumer 精确表示。progress 只能为
`indeterminate` 或 0-10000 basis points。

locale 是 2-15 ASCII bytes 的 canonical core BCP 47 子集：language，可选 Script，可选 Region；不接受 extension、
private-use 或 grandfathered tag。客户端必须先解析系统 locale 再传入，daemon 对不匹配值返回 `invalid_request`。

所有普通文本拒绝 NUL、C0/C1、DEL、U+061C、U+200E/U+200F、U+202A-U+202E 与 U+2066-U+2069。`text` node
允许 LF；`text-field` 只有 `multiline=true` 时允许 LF；其他字段全部单行。CR 与 TAB 始终拒绝。

