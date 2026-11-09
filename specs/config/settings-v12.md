# Config12 全局代理

状态：历史 Config12 迁移输入；当前设置合同见 [Config13](settings-v13.md)。

状态：2026-11-06 项目所有者批准。

`settings.network.proxy` 是 ALCOMD 的出站 HTTP(S) 代理偏好。「设置 → 常规 → 网络」可选择跟随系统（`system`，默认）、不使用代理（`direct`）和自定义代理（`custom`）。自定义模式需要完整 HTTP(S) 代理地址，可带端口，地址不含账号、密码、路径、查询参数或片段。Core 校验和规范化地址；其他模式拒绝代理地址。设置更新以完整的 `network.proxy` 对象替换该偏好，沿用 `settings.read`、`settings.manage` 和 revision CAS。

Core 保存成功后，新选取 HTTP 客户端的请求使用已接受的偏好。在途请求保留已选取的客户端。跟随系统复用平台代理检测，直连跳过自动代理，自定义代理取代自动代理。当前入口涵盖仓库索引、归档下载及其调用方。后续应用自有出站入口复用该合同。外部浏览器、Unity、系统代理和本地 IPC 各自管理。

TOML 使用 `[network]` 下的 `proxy_mode`，仅自定义模式另含 `proxy_url`。有效 Config11 升级只增加默认代理，保留原设置及 revision。旧版本夹带该组时拒绝。公共设置读写和协商报告 Config12，严格客户端须更新必填字段。RPC major 和 State26 不变。

[Schema](settings-v12.schema.json) · [契约向量](settings-v12.contract-vectors.json)
