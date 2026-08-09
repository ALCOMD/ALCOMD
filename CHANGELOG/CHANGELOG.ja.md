# 変更履歴

言語: [English](../CHANGELOG.md) | 日本語 | [簡体中文](./CHANGELOG.zh-CN.md) | [繁体中文](./CHANGELOG.zh-TW.md)

このドキュメントは、英語版の [`../CHANGELOG.md`](../CHANGELOG.md) の日本語版です。
バージョンと変更内容の権威ある情報はトップレベルの `CHANGELOG.md` にあり、本ファイルの
target version entry は GitHub Release の日本語 section に使用されます。

このファイルは [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) の形式に従い、  
本プロジェクトは [Semantic Versioning](https://semver.org/spec/v2.0.0.html) に準拠します。  
GitHub Release 本体は English、日本語、簡体中文の順で、構造が一致する各 target version
entry から生成されます。安定版は直前安定版との差分、プレリリース版は直近の公開版
（安定/プレ）との差分を表します。

開発時は、ユーザー向け変更を同一 PR/変更内で `../CHANGELOG.md` の `Unreleased` に記録します。
Release preparation では target version entry を翻訳し、英語版と date、category order、各
category の bullet count を一致させます。

## [Unreleased]

### Added

- リポジトリに永続的なユーザー定義表示名を設定できるようになりました。
- MCP ツール名について、呼び出し名とローカライズ名を切り替え、もう一方をホバー時に表示できる永続的な設定を追加しました。

### Changed

- MCP のパッケージ用リポジトリ選択をリポジトリ ID に統一し、リポジトリ URL は追加と削除だけで使用するようにしました。
- Windows の新規インストールではデスクトップショートカットの作成を既定で選択し、アップグレード時は以前の選択を維持するようにしました。
- Windows のアンインストール時に、ドキュメント内のプロジェクトやバックアップを残したまま、設定、キャッシュ、その他のローカルアプリデータを削除できるオプションを追加しました。
- MCP ツール一覧とプロジェクトカード一覧をコンテナ幅に応じた最大 3 列表示にし、3 列へ切り替える前の項目幅により余裕を持たせました。

### Fixed

- 補助レコードの表示を切り替えたときも、アクティビティログのテーブル列幅を安定させ、レイアウトのずれを防ぐようにしました。

## [3.2.0] - 2026-08-09

### Added

- 共有リソース管理バックエンド経由で、MCP によるプロジェクトテンプレートの検出と管理を追加しました。
- リリース間の重要な変更を標準化した変更履歴として記録し、正式な基準文書としました。

### Changed

- MCP リポジトリとテンプレートのペイロードを簡素化し、リポジトリ管理を URL ID ベースに統一しました。
- Unity 起動状態ラベルを簡素化しました。
- Unity エディタのフォーカス機能を macOS に拡張し、プロセス照合と Windows のエディタ準備完了キャッシュを改善しました。
- GitHub Release の説明文をこの changelog に統合し、ローカライズ済み updater 要約は構造化された release metadata に移行しました。

### Fixed

- Issue テンプレートとアプリ内の報告リンクで、廃止された `vrc-get-gui` ラベルを要求しないようにしました。
- Windows で環境設定を保存する際、ユーザー設定のパッケージ保存先を保持しました。
- 一致する Unity プロセスが実行中の場合に、それに対応するエディタを検出してフォーカスします。
- パッケージ操作エラーをローカライズし、長いリポジトリ文字列が UI をはみ出さないようにしました。

### Removed

- バージョン別の独立したリリース説明ディレクトリと重複内容を削除しました。

## [3.1.0] - 2026-08-01

### Added

- Bearer トークン保護付きの、ループバック限定 MCP Streamable HTTP エンドポイントと任意クライアントセットアップを追加。
- Unity プロジェクトを開く際の準備完了状態追跡、重複起動防止、エディタへのフォーカス機能を追加。
- MCP から VPM 依存と UnityPackage の添付参照を編集するテンプレート編集を追加。

### Changed

- サイドバー項目が隠れていても拡張ページを表示し続け、Projects/Resources/Settings を常時表示するよう改善し、既存設定で見えなくなった項目を復旧しました。
- MCP 拡張の無効化を可能にし、アクセスを取り消し、エンドポイント停止、サイドバー項目除去、アプリ所有の MCP プロジェクトタスク停止を実装。
- MCP 拡張/エンドポイントの起動処理中でも、メインウィンドウと MCP ツール画面を表示できるようにしました。
- リフレッシュ中もプロジェクトリストを保持し、プロジェクト種別を迅速に更新し、保存済みリポジトリ名を即時反映。
- MCP 設定時に、旧 stdio トランスポート設定の影響を受けない関連設定は維持し、該当クライアントには保護エンドポイントとトークン移行を要求。

### Security

- MCP は既定で無効化され、ループバック専用エンドポイントで Bearer 認証・ホスト・オリジンを検証します。

## [3.1.0-beta.3] - 2026-08-01

### Added

- Unity プロジェクトの開閉状態追跡、重複起動防止、エディタフォーカスを追加。

### Changed

- リフレッシュ中もプロジェクトリストを維持し、保存済みリポジトリ名を更新、内蔵拡張の挙動を明確化。

## [3.1.0-beta.2] - 2026-07-28

### Added

- 内蔵 MCP 拡張を無効化し、アクセスを取り消し、エンドポイントを停止し、サイドバー項目を除去、アプリ所有 MCP タスクをキャンセルする機能を追加。

### Changed

- エンドポイント起動処理がバックグラウンドで続く間も、メインウィンドウと MCP ツールを表示し続けます。
- プロジェクト作成や VRChat SDK パッケージ変更後に、すぐプロジェクト種別を更新。

## [3.1.0-beta.1] - 2026-07-28

### Added

- Bearer トークン保護の MCP Streamable HTTP エンドポイントを追加し、Codex/Claude Code/Cursor 向け任意クライアント設定を提供。未関連クライアント設定は保持。

### Changed

- MCP 設定ダイアログでエンドポイント情報とクライアント設定をまとて表示。
- プロジェクト作成や VRChat SDK パッケージ更新後、すぐにプロジェクト種別を更新。
- 旧 stdio 方式の MCP クライアントは保護エンドポイントへ移行し、トークンを使用するよう変更。

### Security

- MCP は既定で無効。ループバック専用エンドポイントで Bearer 認証、ホスト、オリジンを検証。

## [3.0.1-beta.1] - 2026-07-27

### Added

- 可視化されるインストール済み拡張用の「開く」ボタンを追加。

### Changed

- Projects/Resources/Settings をサイドバーで常時表示し、固定表示の制御を安定化。既存構成で消えた永続項目を復旧。

## [3.0.0] - 2026-07-26

### Added

- 初公開の ALCOMD3 リリースを Windows x64、macOS Apple Silicon、Linux x86_64 向けに公開。

### Security

- ALCOMD3 独自の更新エンドポイント、埋め込み公開鍵、署名付き更新ペイロードを採用。

[Unreleased]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.2.0...HEAD
[3.2.0]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.1.0...v3.2.0
[3.1.0]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.0.0...v3.1.0
[3.1.0-beta.3]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.1.0-beta.2...v3.1.0-beta.3
[3.1.0-beta.2]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.1.0-beta.1...v3.1.0-beta.2
[3.1.0-beta.1]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.0.1-beta.1...v3.1.0-beta.1
[3.0.1-beta.1]: https://github.com/ALCOMD3/ALCOMD3/compare/v3.0.0...v3.0.1-beta.1
[3.0.0]: https://github.com/ALCOMD3/ALCOMD3/releases/tag/v3.0.0
