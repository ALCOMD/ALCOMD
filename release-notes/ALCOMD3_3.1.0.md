# ALCOMD3 v3.1.0

## English

ALCOMD3 3.1.0 adds protected local MCP integration, improves access to extensions and Unity projects, and keeps project and repository information current.

### Application updates

- Extension Management now provides an Open button for installed extensions whose sidebar entries are hidden. Projects, Resources, and Settings remain available in the sidebar, and existing configurations that hid them are repaired automatically.
- ALCOMD3 MCP now provides a bearer-token-protected local Streamable HTTP endpoint. Its configuration dialog includes optional quick setup for Codex, Claude Code, and Cursor while preserving unrelated client settings.
- The MCP page displays its tools immediately, starts its endpoint in the background after the main window appears, and updates its endpoint status when ready. The built-in MCP extension can also be disabled, which revokes access, stops local endpoints, removes its sidebar entry, and cancels application-owned MCP project tasks.
- The main window now appears without waiting for material theme extension state synchronization, which continues in the background.
- Project cards now update their avatar, world, or unknown type immediately after project creation and after VRChat SDK packages are installed or removed.
- ALCOMD3 now tracks Unity project opening and ready states. The Open Unity button shows progress, prevents duplicate launches, and can bring a ready Unity project to the foreground.
- Project lists remain visible during refreshes, and saved repository display names now refresh with package sources.

### Installation and upgrade

- This release has no user-visible installation or upgrade changes.

### Compatibility and security

- MCP remains disabled by default, listens only on the local loopback interface, and validates bearer authentication, host, and origin information.
- MCP client configurations created for the previous stdio transport must be updated to use the endpoint and token shown by ALCOMD3, or replaced with the new quick setup.

## 日本語

ALCOMD3 3.1.0 では、保護されたローカル MCP 連携を追加し、拡張機能と Unity プロジェクトへのアクセスを改善するとともに、プロジェクトとリポジトリの情報を最新に保ちます。

### アプリの更新

- 拡張機能管理で、サイドバーの項目を非表示にしたインストール済み拡張機能に［開く］ボタンを追加しました。プロジェクト、リソース、設定はサイドバーに常に表示され、これらを非表示にしていた既存の設定は自動的に修復されます。
- ALCOMD3 MCP は、Bearer トークンで保護されたローカルの Streamable HTTP エンドポイントを提供するようになりました。設定ダイアログには Codex、Claude Code、Cursor 向けの任意のクイック設定が含まれ、関連しないクライアント設定は保持されます。
- MCP ページはツールをすぐ表示し、メインウィンドウの表示後にエンドポイントをバックグラウンドで起動して、準備ができると状態を更新します。組み込み MCP 拡張機能は無効化することもでき、無効化するとアクセスを取り消し、ローカルエンドポイントを停止し、サイドバー項目を削除して、アプリが管理中の MCP プロジェクトタスクをキャンセルします。
- マテリアルテーマ拡張機能の状態同期を待たずにメインウィンドウを表示し、同期はバックグラウンドで続行するようになりました。
- プロジェクトカードは、プロジェクト作成後、および VRChat SDK パッケージのインストールまたは削除後に、アバター、ワールド、不明の種別をすぐ更新するようになりました。
- ALCOMD3 は Unity プロジェクトの起動中および準備完了の状態を追跡するようになりました。Unity を開くボタンに進行状況を表示し、重複起動を防ぎ、準備完了した Unity プロジェクトを前面に表示できます。
- プロジェクト一覧は更新中も表示されたままとなり、保存済みリポジトリの表示名はパッケージソースとともに更新されるようになりました。

### インストールとアップグレード

- このリリースには、ユーザーに見えるインストールまたはアップグレードの変更はありません。

### 互換性とセキュリティ

- MCP は引き続き既定で無効であり、ローカルループバックインターフェイスのみで待ち受け、Bearer 認証、ホスト、オリジン情報を検証します。
- 以前の stdio transport 用に作成した MCP クライアント設定は、ALCOMD3 に表示されるエンドポイントとトークンを使用するよう更新するか、新しいクイック設定で置き換える必要があります。

## 中文

ALCOMD3 3.1.0 新增受保护的本机 MCP 集成，改进扩展和 Unity 项目的访问体验，并让项目和软件源信息保持最新。

### 应用更新

- 扩展管理现在会为侧边栏入口已隐藏的已安装扩展提供“打开”按钮。项目、资源和设置会始终显示在侧边栏中，曾隐藏这些入口的已有配置会自动修复。
- ALCOMD3 MCP 现在提供由 bearer 令牌保护的本机 Streamable HTTP 端点。其配置弹窗包含 Codex、Claude Code 和 Cursor 的可选快速配置，同时保留无关的客户端设置。
- MCP 页面会立即显示工具，在主窗口显示后于后台启动端点，并在就绪后更新端点状态。内置 MCP 扩展也可关闭；关闭后会撤销访问许可、停止本机端点、移除侧边栏入口，并取消仍由应用管理的 MCP 项目任务。
- 主窗口现在无需等待 Material Theme 扩展状态同步即可显示，同步会在后台继续进行。
- 项目卡片现在会在项目创建完成，以及 VRChat SDK 软件包安装或卸载后立即更新为虚拟形象、世界或未知类型。
- ALCOMD3 现在会跟踪 Unity 项目的启动和就绪状态。打开 Unity 按钮会显示进度、防止重复启动，并可将已就绪的 Unity 项目置于前台。
- 项目列表会在刷新期间保持显示，已保存的软件源显示名称现在会随软件包源一起刷新。

### 安装与升级

- 此版本没有面向用户的安装或升级变化。

### 兼容性与安全

- MCP 仍然默认停用，仅监听本机回环接口，并验证 bearer 认证、主机和来源信息。
- 为旧版 stdio 传输创建的 MCP 客户端配置需要改用 ALCOMD3 显示的端点和令牌，或通过新的快速配置进行替换。
