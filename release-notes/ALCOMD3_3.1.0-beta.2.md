# ALCOMD3 v3.1.0-beta.2

## English

This beta adds complete MCP extension shutdown, keeps ALCOMD3 responsive while
the local MCP endpoint starts, and makes project type changes appear
immediately.

### Application updates

- The main window now appears before the MCP endpoint starts in the background, reducing delays during application startup.
- The MCP page now displays its tools immediately instead of waiting for the endpoint to start, and updates the endpoint status when it becomes available.
- The built-in MCP extension can now be disabled like the Theme and Logs
  extensions. Disabling it revokes MCP access, stops the local endpoints,
  removes MCP from the sidebar, and cancels MCP project tasks still owned by
  the application. The extension switch returns immediately while endpoint
  lifecycle work continues in the background.
- Project cards now refresh their avatar, world, or unknown type immediately after project creation and after either VRChat SDK package is installed or removed.

### Installation and upgrade

- This release has no user-visible installation or upgrade changes.

### Compatibility and security

- This release has no user-visible compatibility, security, or known-issue changes.

## 日本語

このベータ版では、MCP 拡張機能の完全な停止に対応し、ローカル MCP エンドポイントの起動中も ALCOMD3 の応答性を保ち、プロジェクト種別の変更をすぐに反映するようにしました。

### アプリの更新

- メインウィンドウの表示後に MCP エンドポイントをバックグラウンドで起動するようになり、アプリの起動時に生じる遅延を軽減しました。
- MCP ページはエンドポイントの起動を待たずにツールをすぐ表示し、エンドポイントが利用可能になると状態を更新するようになりました。
- 組み込み MCP 拡張機能をテーマとログの拡張機能と同様に無効化できるようになりました。無効化すると MCP アクセスを取り消し、ローカルエンドポイントを停止し、サイドバーから MCP を削除して、アプリが管理中の MCP プロジェクトタスクをキャンセルします。拡張機能の切り替えはすぐに完了し、endpoint lifecycle 処理は background で続行します。
- プロジェクトカードは、プロジェクト作成後、およびいずれかの VRChat SDK パッケージをインストールまたは削除した後に、アバター、ワールド、不明の種別をすぐ更新するようになりました。

### インストールとアップグレード

- このリリースには、ユーザーに見えるインストールまたはアップグレードの変更はありません。

### 互換性とセキュリティ

- このリリースには、ユーザーに見える互換性、セキュリティ、既知の問題の変更はありません。

## 中文

此测试版支持完整关闭 MCP 扩展，让 ALCOMD3 在本机 MCP 端点启动期间保持响应，并让
项目类型变化立即显示。

### 应用更新

- 主窗口现在会先显示，再在后台启动 MCP 端点，减少应用启动时的等待。
- MCP 页面现在无需等待端点启动即可立即显示工具，并会在端点可用后更新状态。
- 现在可以像主题和日志扩展一样关闭内置 MCP 扩展。关闭后会撤销 MCP 访问许可、停止
  本机端点、从侧边栏移除 MCP，并取消仍由应用管理的 MCP 项目任务。扩展开关会立即完成，
  端点生命周期操作继续在后台执行。
- 项目卡片现在会在项目创建完成，以及任一 VRChat SDK 软件包安装或卸载后立即刷新为虚拟形象、世界或未知类型。

### 安装与升级

- 此版本没有面向用户的安装或升级变化。

### 兼容性与安全

- 此版本没有面向用户的兼容性、安全或已知问题变化。
