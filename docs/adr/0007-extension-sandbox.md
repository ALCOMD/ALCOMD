# ADR: 扩展沙箱

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

UI 扩展运行在 sandboxed iframe / 隔离 WebView，后台扩展运行在 WASI 0.2 Component Model 基础上的
WASM Extension Host。Component Model 不是 ambient WASI authority：guest 默认没有 filesystem、environment、
argv、stdio、socket/DNS、process、terminal、daemon RPC、Host private IPC 或 OS credential store；只链接经审批
的 versioned ALCOMD WIT interface 或单独批准的 exact WASI 0.2 interface。

M6 v1 每个 enabled ExtensionId 使用一个独立 `alcomd-extension-host` OS process。Host 只装载一个 ExtensionId，
第一条 slice 只运行一个 active Component instance；不建立跨扩展 Host pool，第一方不共享 privileged Host。

Host Capability authority 来自 daemon-created `ExtensionInstanceLease`，不是 guest/Host 自报 metadata。durable grant
revision 更新是 revoke linearization point；之后的新 call、排队旧 call 和旧 handle/session 必须失败或取消。

第一条业务 capability 只有 scoped `projects.read` Project summary；network/filesystem/clipboard/notification/
Discord 与 M7 UI placement 不属于该 slice。

## 结果

禁止原生动态库扩展、直接 SQLite、直接项目写入和任意 OS 访问。Host crash/hang/OOM 以 ExtensionId 为隔离
边界；first-party 与 third-party 使用同一 package、signature、WIT、permission、scope、quota 和 revocation。
