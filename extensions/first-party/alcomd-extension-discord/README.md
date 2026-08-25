# Discord 状态第一方扩展

扩展 ID：`com.cqmhv.alcomd.extension.discord`

职责：

- Discord Rich Presence 生成。
- 项目名称、Unity 版本和会话时间隐私过滤。
- 设置、预览、连接状态与诊断。
- GUI 关闭后的受控后台运行。

扩展后台最终编译为 WASM/WASI，并通过 Extension Host 暴露的窄 Discord Presence 能力通信。

当前目录只保留 M9 之前的非安装规划 scaffold：没有已签名 Component package 或 Discord Presence 产品接线。
Manifest 使用 active Portable UI shape 仅用于保持统一公开合同，不代表该扩展已可安装或已实现。
