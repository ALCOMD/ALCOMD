# Discord 状态第一方扩展

扩展 ID：`com.cqmhv.alcomd.extension.discord`

职责：

- Discord Rich Presence 生成。
- 项目名称、Unity 版本和会话时间隐私过滤。
- 设置、预览、连接状态与诊断。
- GUI 关闭后的受控后台运行。

扩展后台最终编译为 WASM/WASI，并通过 Extension Host 暴露的窄 Discord Presence 能力通信。
