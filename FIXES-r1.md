# 初始化仓库 r1 修正

本修订解决初版初始化包中的两个启动问题：

1. 补齐 Tauri 桌面图标资源，包括 Windows `icon.ico`、macOS `icon.icns` 和标准 PNG 尺寸。
2. 在 `tauri.conf.json` 中显式声明图标列表。
3. 在 README 中补充 Windows `RemoteSigned` 环境下解除 ZIP / 解压文件 Internet 区域标记的步骤。

`@tauri-apps/cli` 原本已经声明为 GUI workspace 的开发依赖。若出现 `tauri is not recognized`，说明尚未成功执行 `npm install`，通常是因为 `setup.ps1` 先前被 PowerShell 执行策略阻止。
