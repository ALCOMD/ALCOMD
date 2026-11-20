# Unity 应用发现

所有者授权在独立工作树实施三平台发现。Hub 使用系统登记，Editor 先查询 Unity 官方安装清单，再用同一套系统登记补齐。未登记的安装通过手动添加。三平台不扫描固定安装目录。

Hub 启动协商 `unity.hub.open.v1`。设置位置证据包含可空 `targetToken`，打开请求以 `expectedTargetToken` 替换 `expectedPath`，成功结果为 `launch_requested`。严格使用旧版 Hub 合同的测试期客户端需更新。配置、数据库及权限保持现有结构。

## 系统查询与验证

- Windows 按当前用户 MSIX 包或 Win32 安装登记发现。macOS 使用 Launch Services 的 Bundle ID 查询。Linux 读取 XDG 应用登记及已安装包的元数据。
- Editor 官方清单是首选候选来源，系统查询补齐。Core 校验目标并按原文件身份去重。配置保存、刷新、启动通过既有应用用例分派。
- 平台沿用锁定的 `windows-sys`、`objc2-foundation`、`serde_json` 与 Tokio。应用标识、限时子进程和结构化清单分别需要这些绑定。固定路径和未版本化 Hub 数据库在自定义部署或升级后可能漏检，因此查询目标由系统登记和官方列表提供。Hub 的 CLI 安装登记仅用于寻找其 CLI，可由已验证的 PATH 候选补齐。

## 2026-11-21 验收记录

- 真实 Windows 会话检测到 MSIX Hub，系统接受原生启动请求。隔离 Core 的真实 IPC 分别使用官方清单和注册表发现同两项 Editor 安装。两次刷新均保留安装身份。
- Windows 平台单元串行运行 69 项通过、2 项原有忽略。Hub Core 单元 14 项、官方设置 IPC 2 项、RPC Schema 54 项、Chromium 48 项通过。定向严格 Clippy、仓库检查和前端检查及生产构建通过。
- macOS arm64 和 Linux x64 的平台模块及测试源码通过交叉编译。两平台的真实桌面登记、Hub 启动与 Editor Locate 尚未实机验证。
- 新的直接绑定使用已有锁定包。Foundation 为 MIT，JSON 解析库为 MIT 或 Apache-2.0。平台分别承担系统绑定和清单解码，依赖锁只增列直接引用。

## 并入 M11

审查修正了 Linux 的包查询回退。桌面登记指向不可执行文件时，继续检查包管理器登记。新增回归用例已通过 Linux x64 交叉 Clippy，尚未在 Linux 桌面执行。

本地整合包含 Unity Hub 应用定位、打开前复验，以及 ZIP 备份和恢复。整合树通过格式、定向严格 Clippy、仓库检查、52 项 RPC Schema、14 项 Hub Core 单元、69 项 Windows 平台单元、2 项设置 IPC、6 项 ZIP 回归、48 项 Chromium 和前端生产构建。Windows 平台和 ZIP 套件各保留原有忽略用例。macOS 平台和测试源码通过交叉编译。
