# ADR: 扩展沙箱

- 状态：Accepted
- 日期：2026-08-14

## 背景

参见 `docs/architecture/ALCOMD-V4.md`。


## 决策

UI 扩展运行在 sandboxed iframe / 隔离 WebView，后台扩展运行在 WASM/WASI Extension Host。

## 结果

禁止原生动态库扩展、直接 SQLite、直接项目写入和任意 OS 访问。
