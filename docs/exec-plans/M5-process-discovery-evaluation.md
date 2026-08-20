# M5 process discovery 生产依赖评估

状态：候选已完成隔离解析；尚未批准、尚未加入 Workspace

## 需求边界

M5 Unity writer gate 需要在 Windows、Linux 与 macOS 上枚举当前进程，并取得足以建立保守证据的：

- PID；
- executable path（若操作系统允许读取）；
- argv（若操作系统允许读取）；
- process start time。

这些字段只用于判断 `running_confirmed`、`running_suspected`、`not_observed` 或 `unknown`。任何字段
缺失、权限拒绝、进程在枚举期间退出、PID reuse 无法排除或平台不支持时，都必须降为
`running_suspected`/`unknown`，不得猜测为未运行。该依赖不得用于系统遥测、硬件信息、网络、用户
列表或通用进程管理。

## 推荐候选

```toml
sysinfo = {
    version = "=0.39.6",
    default-features = false,
    features = ["system"],
}
```

- 放置：仅 `crates/alcomd-platform` 的直接 production dependency；类型不越过 platform adapter。
- 版本/维护：0.39.6 发布于 2026-07-09；上游持续维护并提供三平台 process API。
- 许可证：MIT。
- MSRV：1.88，低于本仓库 Rust 1.97.1。
- 官方证据：
  [0.39.6 release](https://github.com/GuillaumeGomez/sysinfo/releases/tag/v0.39.6)、
  [Cargo.toml.orig](https://docs.rs/crate/sysinfo/0.39.6/source/Cargo.toml.orig)、
  [Process API](https://docs.rs/sysinfo/0.39.6/sysinfo/struct.Process.html)。

上游文档明确提示部分 process 字段可能因平台、权限或进程竞态不可得；Windows 命令行读取也可能
需要更高权限。ALCOMD 不把这些字段的存在视为保证，并通过上述 `unknown` 语义封闭失败。

## 隔离 feature graph

使用独立临时 manifest、Rust 1.97 工具链和精确配置运行：

```text
cargo generate-lockfile
cargo tree -e features -i sysinfo
cargo tree -i quinn
cargo tree -e build
```

结果：

- `sysinfo` 只激活 `system`（以及该 feature 在 Windows 上内部选择的 `windows`）；未激活默认 feature。
- 未激活 `multithread`，解析图中没有 `rayon`。
- 没有 `quinn`，也没有 HTTP/HTTP3 依赖。
- 没有 build dependency 或本地 C/C++ build script。
- Linux active target closure 为 `sysinfo`、`libc`、`memchr`。
- macOS 另使用 `objc2-core-foundation` 与 `objc2-io-kit`。
- Windows 另使用 `ntapi`、`winapi` 与 `windows` 0.62 平台绑定族。

`sysinfo` 及平台绑定内部包含第三方维护的 unsafe/FFI；本候选不会扩大 ALCOMD 自有 unsafe 文件白
名单。Windows 与 Apple target 会链接系统 API/framework，但不编译或捆绑第三方 native library。

## 相对当前 Cargo.lock 的预计新增 package

隔离锁文件与当前 Workspace `Cargo.lock` 按 `name + version` 比较，预计新增且仅新增：

```text
sysinfo 0.39.6
ntapi 0.4.3
objc2-io-kit 0.3.2
windows 0.62.2
windows-collections 0.3.2
windows-future 0.3.2
windows-numerics 0.3.1
windows-threading 0.2.1
```

预计复用当前已锁定的 `bitflags`、`libc`、`memchr`、`objc2-core-foundation`、`winapi`、
`windows-core` 及其宏/字符串/result 支持包。审批后实际 Cargo.lock 若出现以上列表之外、不能由该
精确配置解释的 production package，必须停止并报告。

## 替代方案

- Linux：直接解析 `/proc` 可以避免通用 crate，但需要自行处理 PID 竞态、权限、启动时间单位、
  namespace 与 argv/exe 不可读语义。
- Windows：需要新增 Tool Help/process query/command-line 等 Win32 API、扩大 `windows-sys` feature，
  并新增第四个或扩展既有 unsafe 平台边界。
- macOS：需要新增 `libproc`/`sysctl` FFI、平台 API 和 unsafe 边界。

三个自有 adapter 会复制竞态与权限处理，且带来新的 Windows/macOS unsafe 审批面。当前需求只是
保守识别 Unity writer，不需要自建通用 process framework，因此单一成熟跨平台 crate 更直接。

## 建议决策

建议批准上面的 `sysinfo = 0.39.6` 精确配置，并附带以下门禁：

1. 只进入 `alcomd-platform`，只暴露 ALCOMD 自有的 bounded process evidence DTO。
2. 不增加其他 sysinfo feature，不用于 CPU、内存、磁盘、网络、用户或系统遥测。
3. 每个平台的权限拒绝、字段缺失和枚举竞态都返回 suspected/unknown，永不推断 not-observed。
4. Windows/Linux/macOS 均以 fake provider 单测和真实短生命周期子进程集成测试覆盖 PID、exe、argv、
   start time、退出竞态和并行枚举。
5. 实际 Cargo.lock 与 feature graph 必须复核本文件记录；偏离时重新审批。

在项目所有者批准前，不修改 Workspace manifest/Cargo.lock，也不开始 Unity writer gate 生产实现。
