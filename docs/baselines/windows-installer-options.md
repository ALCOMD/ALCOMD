# Windows 全产品安装器基线

状态：A-024 已接受；实现与永久 AppId 尚未开始

最后核验：2026-08-16

## 产品边界

ALCOMD 是多组件 Rust 本地应用平台，只有 `alcomd-gui` 是 Tauri 应用。Windows 安装器部署
完整产品，不是单独的 Tauri GUI bundle。以下旧候选已被全产品打包模型替代：

- 单个 Tauri NSIS `installMode = both`。
- 两个 Tauri NSIS 安装器。
- 当前用户 NSIS + 全局 WiX/MSI。
- Tauri Updater 作为整个产品的权威安装/更新系统。

Superseded by the full-product packaging model.

## 正式安装器

Windows x86_64 主安装器是单文件 Inno Setup EXE：

```text
ALCOMD_4.0.0_windows_x86_64_setup.exe
```

同一安装器支持两个互斥模式：

| 模式 | 默认 | 提权 | 安装路径 | PATH scope |
| --- | ---: | ---: | --- | --- |
| 当前用户 | 是 | 否 | `%LOCALAPPDATA%\Programs\ALCOMD\` | 用户 PATH |
| 所有用户 | 否 | 用户明确选择后请求 | `%ProgramFiles%\ALCOMD\` | 系统 PATH |

当前用户与所有用户安装不得同时存在。安装器必须发现另一 scope，生成可理解的转换计划，并在
成功安装、bootstrap 健康检查和旧 scope 清理之间保持可恢复性。

## 产品布局与 CLI

```text
ALCOMD/
├─ bin/
│  └─ alcomd-cli.exe
└─ runtime/
   ├─ alcomd.exe
   ├─ alcomd-gui.exe
   ├─ alcomd-mcp.exe
   ├─ alcomd-extension-host.exe
   ├─ alcomd-bootstrap.exe
   ├─ alcomd-updater.exe
   └─ 其他内部组件与第一方扩展
```

仅 `ALCOMD\bin` 加入 PATH。不得把完整安装目录加入 PATH，也不得把 `alcomd`、GUI、MCP、
Extension Host、bootstrap 或 updater 作为普通终端命令公开。

PATH 合同：

- 当前用户安装只写用户 PATH；所有用户安装只写系统 PATH。
- 更新幂等，不重复添加相同条目。
- scope 转换先建立新入口，Commit 后删除旧 scope 精确条目。
- 卸载只删除安装器创建且与当前安装目录精确匹配的条目。
- 安装结果提示已打开终端可能需要重新启动。

## Inno Setup 与 bootstrap 职责

Inno Setup 负责：

- 部署经过 staging 验证的完整产品文件。
- 注册卸载项、快捷方式、`alcomd://` 和文件关联。
- 建立公开 CLI PATH 入口。
- 调用 `alcomd-bootstrap`。

Inno Setup 脚本不得解析 v3 数据库、修改 Unity 项目、成为状态持有者或实现业务迁移。

`alcomd-bootstrap` 负责：

- 停止和恢复组件。
- v3→v4 数据迁移与临时 migrator 生命周期。
- Health Check、回滚、Commit 与 v3 残留清理。
- scope 转换和更新期间的组件协调。

正常 Windows 更新由 `alcomd-updater`、`alcomd-bootstrap` 和签名完整 Inno Setup 包共同完成。
Tauri Updater 不参与完整产品权威更新。

## v3.4.0 与 bridge installer

ALCOMD3 v3.4.0 是最后受支持的 v3 迁移入口，不执行完整 v4 替换迁移。它从冻结更新 API 发现、
验证并启动 ALCOMD v4 bridge installer。

bridge installer 是完整 v4 Inno Setup 包，包含或调用 `alcomd-bootstrap`。它接受 v3 updater
要求的 `/SP- /SILENT /NOICONS` 与 `/CURRENTUSER|/ALLUSERS`，但未知参数不得成为 shell 输入。
v3 成功启动 EXE 不等于迁移成功；恢复责任直到 v4 Health Check 与 Commit marker 完成才转移。

## 正式发行流水线边界

未来 `cargo xtask dist --target x86_64-pc-windows-msvc` 负责构建/收集全部组件、验证统一版本、
权限和许可证、形成 staging、调用 Inno Setup、签名最终 EXE 与生成 update manifest。Tauri 只
构建 `alcomd-gui.exe`，不能单独定义 Windows 产品包。

本基线不包含 Inno 脚本、真实 AppId、签名材料或发行实现；这些进入后续全产品发行 ExecPlan。

## 必测合同

- 当前用户安装在无管理员凭据账号中成功且不出现 UAC。
- 只有用户明确选择所有用户安装时请求 UAC；覆盖同意、取消、凭据错误和恢复。
- 两 scope 不可并存；双向转换、同 scope 升级、降级阻止和崩溃恢复。
- PATH 添加、去重、转换、卸载和同名第三方条目保留。
- 完整组件清单、版本一致性、签名、错误架构、损坏/缺少组件和许可证清单。
- Unicode、空格、长路径、文件占用、磁盘不足、重启和 WebView2 缺失/离线。
- 安装、更新、卸载、repair/reinstall 和未知文件保留。
- v3 bridge 在启动安装器、UAC、部署、bootstrap、Health Check 和旧版卸载每个故障点恢复
  v3.4.0 与用户数据。
- v3 `.alcomtemplate` ProgID 与 `vcc` handler 卸载后，v4 独立关联仍有效。

## 官方依据

- Inno scope override：`https://jrsoftware.org/ishelp/topic_setup_privilegesrequiredoverridesallowed.htm`
- Inno command line：`https://jrsoftware.org/ishelp/topic_setupcmdline.htm`
- Inno AppId：`https://jrsoftware.org/ishelp/topic_setup_appid.htm`
