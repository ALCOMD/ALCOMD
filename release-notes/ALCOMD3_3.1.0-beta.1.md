# ALCOMD3 v3.1.0-beta.1

## English

This beta makes ALCOMD3 MCP easier to configure while moving client connections to a protected local Streamable HTTP endpoint.

### Application updates

- ALCOMD3 MCP now provides a Streamable HTTP endpoint with bearer-token authentication instead of requiring clients to launch the MCP process through stdio.
- Added optional one-click setup on Windows for Codex, Claude Code, and Cursor. Existing client settings are preserved, and ALCOMD3 asks before replacing a conflicting environment variable or MCP entry.
- Endpoint details and quick setup are now grouped in an MCP configuration dialog opened from the Configure button beside Enable or Disable.

### Installation and upgrade

- This release has no user-visible installation or upgrade changes.

### Compatibility and security

- MCP remains disabled by default, listens only on the local loopback interface, and validates bearer authentication, host, and origin information.
- MCP client configurations created for the previous stdio transport must be updated to use the endpoint and token shown by ALCOMD3, or replaced with the new quick setup.

## 日本語

このベータ版では、クライアント接続を保護されたローカルの Streamable HTTP エンドポイントへ移行し、ALCOMD3 MCP をより簡単に設定できるようにしました。

### アプリの更新

- ALCOMD3 MCP は、クライアントが stdio 経由で MCP プロセスを起動する方式に代わり、Bearer トークン認証付きの Streamable HTTP エンドポイントを提供するようになりました。
- Windows 版で Codex、Claude Code、Cursor の任意のワンクリック設定を追加しました。クライアントの既存設定は保持され、環境変数または MCP エントリと競合する場合は置き換える前に確認します。
- エンドポイントの詳細とクイック設定を、［有効化］または［無効化］の横にある［設定］ボタンから開く MCP 設定ダイアログにまとめました。

### インストールとアップグレード

- このリリースには、ユーザーに見えるインストールまたはアップグレードの変更はありません。

### 互換性とセキュリティ

- MCP は引き続き既定で無効であり、ローカルループバックインターフェイスのみで待ち受け、Bearer 認証、ホスト、オリジン情報を検証します。
- 以前の stdio transport 用に作成した MCP クライアント設定は、ALCOMD3 に表示されるエンドポイントとトークンを使用するよう更新するか、新しいクイック設定で置き換える必要があります。

## 中文

此测试版将客户端连接迁移到受保护的本机 Streamable HTTP 端点，并让 ALCOMD3 MCP 配置更加便捷。

### 应用更新

- ALCOMD3 MCP 现在提供带 bearer 令牌认证的 Streamable HTTP 端点，不再要求客户端通过 stdio 启动 MCP 进程。
- Windows 版新增 Codex、Claude Code 和 Cursor 的可选一键配置。客户端的其他已有设置会保留；如果环境变量或 MCP 配置项存在冲突，ALCOMD3 会在替换前请求确认。
- 端点详情与快速配置现在集中在 MCP 配置弹窗中，可通过“启用”或“停用”旁的“配置”按钮打开。

### 安装与升级

- 此版本没有面向用户的安装或升级变化。

### 兼容性与安全

- MCP 仍然默认停用，仅监听本机回环接口，并验证 bearer 认证、主机和来源信息。
- 为旧版 stdio 传输创建的 MCP 客户端配置需要改用 ALCOMD3 显示的端点和令牌，或通过新的快速配置进行替换。
