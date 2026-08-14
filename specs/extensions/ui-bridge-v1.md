# Extension UI Bridge v1

状态：Draft

扩展 UI 不能直接访问 Tauri IPC 或主页面 DOM。

```text
Extension iframe / WebView
    -> postMessage / isolated channel
    -> alcomd-gui bridge
    -> scoped extension session
    -> ALCOMD RPC
```

Bridge 必须：

- 验证扩展实例、origin、消息版本与请求 ID。
- 只暴露已授予权限的方法。
- 限制消息大小、频率和并发。
- 统一审计写操作。
- 在扩展禁用或权限撤销时立即关闭 session。
