# M7 Project Actions dependency evaluation

状态：2026-08-28 隔离 probe 完成；两个候选均未写入 production manifest/Cargo.lock，均等待人工审批。

## 方法与不变量

- host：Windows x86_64，Rust 1.97.1，Cargo `--offline --locked`，Tauri `=2.11.5`；
- probe 位于 ignored `target/m7-project-actions-probe/`，各自独立 workspace/target directory；
- 主 `Cargo.lock` SHA-256：`1657d6349da5c71d4b5b53828f98f659dea4acb2cc484e09fcc186fada2b0e6b`；
- 主 `package-lock.json` SHA-256：`7e143c8ecd505befc9b42804f362489f2093e254c7b6bb221d9497ce043102c1`；
- probe 前后主 manifest/lock 未变化。

lock closure 的“新增”以当前主 `Cargo.lock` package identity 集合为基线。为了避免 scratch workspace pruning 把主 workspace
未激活 package 误报为 removed，评估只报告候选解析中当前主锁文件缺少的 package；production 落盘时必须再次以真实 workspace
resolution 核对，任何额外 package 都重新停止。

## `tauri-plugin-opener = "=2.5.4"`

| 项目 | 结果 |
|---|---|
| crates.io checksum | `17e1bea14edce6b793a04e2417e3fd924b9bc4faae83cdee7d714156cceeed29` |
| `.crate` size | 58,041 bytes |
| license | `Apache-2.0 OR MIT` |
| MSRV | upstream README 声明 1.77.2；normalized manifest 未声明 `rust-version`；Workspace 1.97.1 满足 |
| direct feature | crate 没有 feature table；Cargo 显示 `default` marker |
| build script | crate `build.rs`，经 `tauri-plugin 2.6.3` build support 生成 plugin metadata |
| Tauri compatibility | dependency 要求 `tauri ^2.10`；隔离 probe 与 `tauri =2.11.5` 成功 release build |
| Rust free function | `tauri_plugin_opener::open_path(path, None::<&str>)` 是 public free function |
| plugin registration | free function path 不需要 `tauri_plugin_opener::init()` |
| npm binding | 不需要 |
| Tauri capability | 不需要；不注册 plugin，不向 WebView 暴露 opener command |
| own ALCOMD unsafe/API | 无新增 ALCOMD unsafe 或 direct platform API；crate/transitive `open` 在 Windows 有第三方 unsafe/FFI |

相对当前主锁文件预计精确新增 36 个 registry package：

```text
async-broadcast 0.7.2
async-channel 2.5.0
async-executor 1.14.0
async-io 2.6.0
async-lock 3.4.2
async-process 2.5.0
async-recursion 1.1.1
async-signal 0.2.14
async-task 4.7.1
blocking 1.6.2
concurrent-queue 2.5.0
endi 1.1.1
enumflags2 0.7.12
enumflags2_derive 0.7.12
event-listener 5.4.2
event-listener-strategy 0.5.4
futures-lite 2.6.1
hermit-abi 0.5.2
is-docker 0.2.0
is-wsl 0.4.0
open 5.4.1
ordered-stream 0.2.0
parking 2.2.1
piper 0.2.5
polling 3.11.0
tauri-plugin 2.6.3
tauri-plugin-opener 2.5.4
tempfile 3.27.0
uds_windows 1.2.1
zbus 5.19.0
zbus_macros 5.19.0
zbus_names 4.3.4
zcheapstr 1.1.0
zvariant 5.15.0
zvariant_derive 5.15.0
zvariant_utils 4.2.0
```

主要图：`tauri-plugin-opener -> open 5.4.1`; Linux/BSD target 另有 `url 2 + zbus 5.19` 及上述 async/zvariant
闭包；macOS target 有 `objc2-app-kit/foundation 0.3`（主锁已存在）；Windows target 有 `windows 0.61`（主锁已存在）。
plugin 自身 build dependency 是 `tauri-plugin 2.6.3`。没有新的 native build dependency；Linux runtime 通过
`xdg-open`，失败后尝试 `gio open`、`gnome-open`、`kde-open`，均用独立 argv，不使用 shell string；macOS 使用
`/usr/bin/open` 独立 argv；Windows 因 `open/shellexecute-on-windows` 使用第三方 `ShellExecuteExW`，不调用
`cmd /c` 或 PowerShell command string。

源码中即使本 slice 不调用 reveal API，crate 仍包含 Windows reveal/COM 与 `open` 的第三方 unsafe；这不扩大 ALCOMD unsafe
whitelist，但必须作为供应链事实保留。Linux 的 zbus 闭包是 target-specific locked/active dependency，不应描述为 Windows/macOS
runtime component。

同机、独立 clean target、release probe：baseline 224.46 s / 123,392 bytes，opener 186.12 s / 135,680 bytes，产物
`+12,288` bytes。时间受顺序和系统缓存影响，只是一次可复现方法下的 observation，不能解释为负 compile cost；lock/feature/
binary byte delta 才是审批基线。

app-private error mapping：metadata/registered-directory validation 失败先由 adapter 返回明确 private code；opener 的 IO/launch
错误统一映射为 `project_directory_open_failed`，不返回 underlying path、command、HRESULT/errno 或 Debug。

## `tauri-plugin-dialog = { version = "=2.7.2", default-features = false, features = ["gtk3"] }`

| 项目 | 结果 |
|---|---|
| crates.io checksum | `b2d3c1dbe38037e7f590cdf2492594d5ceebe031e7bc7e827509b22a999d2940` |
| `.crate` size | 129,548 bytes |
| license | `Apache-2.0 OR MIT`; direct backend `rfd 0.16.0` 是 `MIT` |
| MSRV | manifest 明确 1.77.2；Workspace 1.97.1 满足 |
| direct features | 只启用 `gtk3`；default false，`xdg-portal` 未启用 |
| build scripts | dialog 与 `tauri-plugin-fs` 使用 `tauri-plugin` build support；`rfd` 有 `build.rs` |
| Tauri compatibility | dependency 要求 `tauri ^2.10`；与 `tauri =2.11.5` release probe 成功 |
| Rust API | `DialogExt::dialog().file().pick_folder` / `blocking_pick_folder` |
| plugin registration | **需要** `tauri_plugin_dialog::init()`；它安装 managed state、JS init 与 plugin invoke handler |
| npm binding | closed Rust adapter 不需要，也不批准 |
| frontend capability | closed adapter 不需要；production capability 继续不包含 `dialog:*` |
| own ALCOMD unsafe/API | 无新增 ALCOMD unsafe/direct platform API；dialog/rfd 有第三方 unsafe/FFI |

相对当前主锁文件预计精确新增 14 个 registry package：

```text
rfd 0.16.0
tauri-plugin 2.6.3
tauri-plugin-dialog 2.7.2
tauri-plugin-fs 2.5.1
windows-sys 0.60.2
windows-targets 0.53.5
windows_aarch64_gnullvm 0.53.1
windows_aarch64_msvc 0.53.1
windows_i686_gnu 0.53.1
windows_i686_gnullvm 0.53.1
windows_i686_msvc 0.53.1
windows_x86_64_gnu 0.53.1
windows_x86_64_gnullvm 0.53.1
windows_x86_64_msvc 0.53.1
```

production resolution 必须保留主锁中已有 `windows-sys 0.52.0`；scratch metadata 的“update 0.52 -> 0.60”只是因为它裁剪了
主 workspace package，不能作为 production lock diff。真实落盘若移除/升级现有锁定项或增加上述列表之外 package，必须停止。

图为 `tauri-plugin-dialog -> rfd 0.16.0 + tauri-plugin-fs 2.5.1`。Windows 使用 COM Common Item Dialog；macOS
使用 `NSOpenPanel`；Linux/Ubuntu 22.04 使用既有 `libgtk-3-dev` 对应 GTK3 backend。`xdg-portal`、ashpd 与其 zbus graph
未激活。Windows `rfd` 带 `windows-sys 0.60.2` target 闭包；macOS objc2 target packages已在主锁；Linux GTK sys
packages已在主锁。

同机、独立 clean target、release probe：dialog 207.77 s / 2,415,616 bytes，相对 baseline `+2,292,224` bytes。
同样不把顺序相关时间当稳定 benchmark。较大的 Windows产物增量来自 native dialog/plugin path；最终 official GUI release
产物必须在三平台重新测量。

closed command 只返回 user-selected directory path 或 cancelled；前端不能指定起始任意路径，也没有直接 dialog capability。
plugin registration 带来的 guest command surface 必须由 capability absence 永久隔离并加入 source/config gate。错误映射只暴露
`directory_selection_failed`/`internal_error`，不暴露 native error、window handle 或完整诊断。

## 结论与审批点

- opener 的 free-function integration 满足 closed ProjectId-only architecture，但 36-package cross-platform lock closure 与第三方
  unsafe 仍需明确批准。
- dialog 确实需要 plugin registration；不能声称当前 GUI 已有 picker，也不能把 Rust-side use 说成零 plugin runtime surface。
- 两者均不需要 npm binding、frontend capability、ALCOMD unsafe 或 direct platform API；均未落盘。
- 生产批准应分别给出，不能把 directory chooser 需求自动扩张为 opener 或反向绑定。
