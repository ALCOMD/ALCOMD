# ALCOMD3 MCP ツールリファレンス

[English](tools.md) | [简体中文](tools.zh-CN.md) | [繁體中文](tools.zh-TW.md)

このリファレンスは、ALCOMD3 が現在公開している 33 個の MCP ツールを説明します。
接続、認証、ライフサイクル、クライアント設定については
[MCP メインガイド](mcp.ja.md)を参照してください。

## このリファレンスの読み方

- 入力名は `snake_case`、出力名は通常 `camelCase` です。
- 「必須」が「はい」のフィールドは省略できません。「いいえ」の場合は省略でき、
  既定値を「意味」欄に記載しています。
- 入力フィールドがないツールにも空オブジェクト `{}` を渡します。
- `string / null` はフィールド自体は存在するものの、値が `null` になり得ることを示します。
- 実行時の `tools/list` は各ツールの `inputSchema` を返します。現在、
  `alcomd3_list_repositories` は厳密な `outputSchema` も返します。
- 結果は MCP `structuredContent` です。ほとんどの成功結果には `ok: true` が含まれますが、
  `alcomd3_get_activity_log_entry` と `alcomd3_get_technical_log_entry` は詳細オブジェクトを
  直接返すため `ok` を含みません。

業務エラーは次の形式で返り、外側の MCP tool result は `isError: true` になります。

```json
{
    "ok": false,
    "error": {
        "code": "error_code",
        "message": "Actionable message",
        "data": {}
    }
}
```

`error.data` は構造化された補足情報が必要な場合だけ現れます。Schema エラー、業務エラー、
プロトコルエラーの違いは
[MCP Tools 仕様](https://modelcontextprotocol.io/specification/2025-11-25/server/tools#error-handling)
を参照してください。

## クイックインデックス

| 分類 | ツール | 動作 | 用途 |
| --- | --- | --- | --- |
| プロジェクト | `alcomd3_list_projects` | 読み取り専用 | 登録済みプロジェクトを一覧表示します。 |
| テンプレート | `alcomd3_list_templates` | 読み取り専用 | 利用可能な環境レベルのテンプレートを一覧表示します。 |
| テンプレート | `alcomd3_get_template` | 読み取り専用 | 環境レベルのテンプレートを 1 件取得します。 |
| テンプレート | `alcomd3_create_template` | 書き込み | 派生テンプレートを作成します。 |
| テンプレート | `alcomd3_edit_template` | 破壊的書き込み | 定義全体を置換して派生テンプレートを編集します。 |
| テンプレート | `alcomd3_set_template_package` | 冪等な書き込み | 直接 VPM 依存関係を 1 件設定します。 |
| テンプレート | `alcomd3_remove_template_package` | 破壊的書き込み | 直接 VPM 依存関係を 1 件削除します。 |
| テンプレート | `alcomd3_set_template_unitypackage` | 冪等な書き込み | UnityPackage 添付参照を 1 件設定します。 |
| テンプレート | `alcomd3_remove_template_unitypackage` | 破壊的書き込み | UnityPackage 添付参照を 1 件削除します。 |
| テンプレート | `alcomd3_remove_template` | 破壊的書き込み | 削除可能なテンプレートをごみ箱へ移動します。 |
| プロジェクト | `alcomd3_get_project_details` | 読み取り専用 | 登録済みプロジェクトの詳細を取得します。 |
| リポジトリ | `alcomd3_list_repositories` | 読み取り専用 | リモートリポジトリと表示設定を一覧表示します。 |
| リポジトリ | `alcomd3_add_repository` | ネットワーク書き込み | リモート VPM リポジトリを追加します。 |
| リポジトリ | `alcomd3_remove_repository` | 破壊的書き込み | URL でユーザーリポジトリを削除します。 |
| パッケージ | `alcomd3_get_package_details` | 読み取り専用 | 表示対象パッケージの詳細を取得します。 |
| パッケージ | `alcomd3_list_packages` | 読み取り専用 | GUI で表示されるパッケージをページングします。 |
| パッケージ | `alcomd3_list_repository_packages` | 読み取り専用 | 1 リポジトリのパッケージをページングします。 |
| 環境 | `alcomd3_get_environment_settings` | 読み取り専用 | Unity インストールと既定パスを取得します。 |
| アクティビティ | `alcomd3_search_activity_logs` | 読み取り専用 | アクティビティ概要を検索します。 |
| アクティビティ | `alcomd3_get_activity_log_entry` | 読み取り専用 | アクティビティを 1 件取得します。 |
| アクティビティ | `alcomd3_summarize_activity_logs` | 読み取り専用 | アクティビティを集計します。 |
| アクティビティ | `alcomd3_get_activity_log_context` | 読み取り専用 | 前後のアクティビティを取得します。 |
| 技術ログ | `alcomd3_search_technical_logs` | 読み取り専用 | 技術ログのプレビューを検索します。 |
| 技術ログ | `alcomd3_get_technical_log_entry` | 読み取り専用 | 技術ログを 1 件取得します。 |
| 技術ログ | `alcomd3_summarize_technical_logs` | 読み取り専用 | 技術ログを集計します。 |
| プロジェクト | `alcomd3_create_project` | 長時間書き込み | Unity プロジェクトを作成・登録します。 |
| プロジェクト | `alcomd3_add_existing_project` | 書き込み | 既存プロジェクトを登録します。 |
| プロジェクト | `alcomd3_backup_project` | 長時間書き込み | zip バックアップを作成します。 |
| プロジェクト | `alcomd3_copy_project` | 長時間書き込み | プロジェクトをコピー・登録します。 |
| プロジェクト | `alcomd3_restore_project_from_backup` | 長時間書き込み | バックアップを復元・登録します。 |
| プロジェクトパッケージ | `alcomd3_install_project_package` | 長時間書き込み | VPM パッケージをインストールします。 |
| プロジェクトパッケージ | `alcomd3_uninstall_project_package` | 破壊的長時間処理 | パッケージをアンインストールします。 |
| プロジェクトパッケージ | `alcomd3_reinstall_project_package` | 長時間書き込み | パッケージを再インストールします。 |

「長時間」ツールは `execution.taskSupport: "optional"` を宣言します。MCP Tasks 対応クライアントは
非同期でポーリングでき、未対応クライアントも通常の同期 `tools/call` を利用できます。詳しくは
[プロジェクト長時間タスク](mcp.ja.md#プロジェクト長時間タスク)を参照してください。

## プロジェクトとテンプレート

### `alcomd3_list_projects`

ALCOMD3 データベースに登録されたプロジェクトを返します。未登録フォルダーは走査しません。

**入力:** フィールドなし。`{}` を渡します。

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `projects` | [`ProjectSummary[]`](#projectsummary) | 常に | 登録済みプロジェクトの概要。正しいパス概要を作れないレコードは除外されます。 |

### `alcomd3_list_templates`

プロジェクト作成に利用できる環境レベルのテンプレートを返します。これらは登録済みプロジェクトが所有するテンプレートデータではありません。テンプレートの保存元パスは公開しません。

**入力:** フィールドなし。`{}` を渡します。

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `templates` | [`TemplateSummary[]`](#templatesummary) | 常に | テンプレート概要と操作可否フラグ。 |

### `alcomd3_get_template`

安定したテンプレート ID で環境レベルのテンプレートを 1 件取得します。この呼び出しは登録済みプロジェクトを調べません。ID は読み取り対象を指定するだけです。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `template_id` | `string` | はい | `alcomd3_list_templates` が返した `id`。前後の空白を除いて空にはできません。 |

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `template` | [`TemplateDetails`](#templatedetails) | 常に | 保存パスを除いた概要と読み取り可能な定義。 |

### `alcomd3_create_template`

派生テンプレートを作成します。安定 ID はバックエンドが生成して永続化します。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `display_name` | `string` | はい | ユーザーに表示するテンプレート名。 |
| `base_template_id` | `string` | はい | `usableAsBase: true` の既存テンプレート ID。 |
| `unity_version_range` | `string` | はい | 解析可能な Unity バージョン範囲。 |
| `vpm_dependencies` | `object<string, string>` | はい | VPM パッケージ名からバージョン範囲への完全なマップ。 |
| `unitypackage_paths` | `string[]` | はい | 存在する絶対 `.unitypackage` 通常ファイルパス。空配列も指定できます。 |

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `template` | [`TemplateDetails`](#templatedetails) | 常に | 永続化された定義と生成 ID。 |

添付ファイルは参照するだけでコピーしません。無効な依存関係・範囲・パス、自己参照、
基底テンプレートの循環は拒否されます。

### `alcomd3_edit_template`

派生テンプレートの編集可能な定義全体を置換し、ID と保存位置は維持します。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `template_id` | `string` | はい | 編集する派生テンプレート ID。 |
| `display_name` | `string` | はい | 置換後の表示名。 |
| `base_template_id` | `string` | はい | 置換後の基底テンプレート ID。 |
| `unity_version_range` | `string` | はい | 置換後の Unity バージョン範囲。 |
| `vpm_dependencies` | `object<string, string>` | はい | 置換後の完全な依存関係マップ。 |
| `unitypackage_paths` | `string[]` | はい | 置換後の完全な添付パス一覧。 |

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `template` | [`TemplateDetails`](#templatedetails) | 常に | 編集後の完全な定義。 |

組み込みテンプレートとプロジェクトアーカイブはフィールド編集できません。定義全体を置換するため、
このツールは destructive としてマークされます。

### `alcomd3_set_template_package`

派生テンプレートに直接 VPM 依存関係を 1 件設定します。パッケージ名とバージョン範囲の宣言だけを保存し、リポジトリの選択、依存関係の解決、ファイルのインストールは行いません。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `template_id` | `string` | はい | 編集可能な派生テンプレート ID。 |
| `package_name` | `string` | はい | 有効な完全 VPM パッケージ名。 |
| `version_range` | `string` | はい | 追加または置換する解析可能な VPM バージョン範囲。 |

**成功出力:** `ok: true` と、`template` 内の最新の完全な [`TemplateDetails`](#templatedetails)。同じパッケージ名と範囲を繰り返し設定しても書き込みません。

### `alcomd3_remove_template_package`

派生テンプレートから直接 VPM 依存関係の宣言を 1 件削除します。既存プロジェクトは変更しません。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `template_id` | `string` | はい | 編集可能な派生テンプレート ID。 |
| `package_name` | `string` | はい | 削除する既存の直接依存関係。 |

**成功出力:** `ok: true` と、`template` 内の最新の完全な [`TemplateDetails`](#templatedetails)。依存関係が存在しない場合は `template_package_not_found` を返します。

### `alcomd3_set_template_unitypackage`

派生テンプレートに UnityPackage 添付参照を 1 件設定します。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `template_id` | `string` | はい | 編集可能な派生テンプレート ID。 |
| `unitypackage_path` | `string` | はい | 存在する絶対 `.unitypackage` 通常ファイルパス。 |

パスを正規化し、ファイルをコピーせず参照だけを保存します。同じ正規パスを繰り返し設定しても書き込みません。

**成功出力:** `ok: true` と、`template` 内の最新の完全な [`TemplateDetails`](#templatedetails)を返します。

### `alcomd3_remove_template_unitypackage`

派生テンプレートから UnityPackage 添付参照を 1 件削除します。パスは `alcomd3_get_template` からコピーしてください。参照先ファイルは削除されず、既に存在しなくても構いません。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `template_id` | `string` | はい | 編集可能な派生テンプレート ID。 |
| `unitypackage_path` | `string` | はい | テンプレート定義に存在する添付パス。 |

**成功出力:** `ok: true` と、`template` 内の最新の完全な [`TemplateDetails`](#templatedetails)を返します。参照が存在しない場合は `template_unitypackage_not_found` を返します。

### `alcomd3_remove_template`

削除可能なテンプレートをシステムのごみ箱へ移動します。組み込みテンプレートは削除できず、
参照先の添付ファイルは削除しません。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `template_id` | `string` | はい | 削除するテンプレート ID。 |

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `template` | [`RemovedTemplate`](#removedtemplate) | 常に | 削除したテンプレートの ID、名前、種類。 |

### `alcomd3_get_project_details`

登録済みプロジェクトの Unity 情報とインストール済みパッケージを取得します。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `project_path` | `string` | はい | ALCOMD3 に登録されたプロジェクトパスと完全に一致する値。 |

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `project` | [`ProjectDetails`](#projectdetails) | 常に | Unity バージョン、解決状態、インストール済みパッケージ。 |

### `alcomd3_create_project`

Unity プロジェクトを作成し、パッケージを解決して登録します。MCP Task を任意で利用できます。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `project_name` | `string` | はい | パス・ルート・`..` ではない有効なディレクトリ名 1 個。 |
| `base_path` | `string` | いいえ | 絶対親ディレクトリ。省略時は GUI の既定プロジェクトディレクトリ。 |
| `template_id` | `string` | いいえ | テンプレート ID。省略時は GUI の現在の選択規則に従います。 |
| `unity_version` | `string` | いいえ | Unity バージョン。省略時は GUI の現在の選択規則に従います。 |

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `projectPath` | `string` | 常に | 新規プロジェクトの絶対パス。 |
| `templateId` | `string` | 常に | 実際に使用したテンプレート ID。 |
| `unityVersion` | `string` | 常に | 実際に選択した Unity バージョン。 |

### `alcomd3_add_existing_project`

既存 Unity プロジェクトをコピーせず ALCOMD3 に登録します。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `project_path` | `string` | はい | 有効な Unity プロジェクトディレクトリへの絶対パス。 |

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `projectPath` | `string` | 常に | 実際に登録したパス。 |

### `alcomd3_backup_project`

登録済みプロジェクトの zip バックアップを作成します。MCP Task を任意で利用できます。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `project_path` | `string` | はい | 登録済みプロジェクトパス。 |
| `backup_name` | `string` | いいえ | `.zip` を含まない有効なファイル名 1 個。省略時は自動生成し、パスは拒否します。 |
| `exclude_vpm_packages` | `boolean` | いいえ | `true` なら VPM パッケージ内容を除外。既定値は `false`。 |

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `backupPath` | `string` | 常に | 作成した zip の絶対パス。 |

バックアップは GUI で設定したバックアップディレクトリに作成され、既存ファイルを上書きしません。

### `alcomd3_copy_project`

登録済みプロジェクトをコピーし、コピー先を登録します。MCP Task を任意で利用できます。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `source_project_path` | `string` | はい | 登録済みコピー元プロジェクトパス。 |
| `new_project_path` | `string` | はい | コピー元内部ではなく、まだ存在しない絶対コピー先ディレクトリ。 |

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `projectPath` | `string` | 常に | コピーして登録したプロジェクトパス。 |

### `alcomd3_restore_project_from_backup`

zip バックアップを GUI の既定プロジェクトディレクトリへ復元し、登録します。
MCP Task を任意で利用できます。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `backup_path` | `string` | はい | ALCOMD3 zip バックアップへの絶対ファイルパス。 |
| `project_name` | `string` | いいえ | 復元先の有効なディレクトリ名 1 個。省略時はバックアップファイル名。 |

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `projectPath` | `string` | 常に | 復元して登録したプロジェクトパス。 |

## リポジトリ、パッケージ、環境

### `alcomd3_list_repositories`

対応するリモートリポジトリとグローバルなパッケージ表示設定を返します。
ローカルリポジトリには対応しません。

**入力:** フィールドなし。`{}` を渡します。

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `repositories` | [`RepositorySummary[]`](#repositorysummary) | 常に | Official、Curated、ユーザーの重複しない正規配列。 |
| `packageVisibility` | `object` | 常に | グローバルなパッケージ表示設定。 |
| `packageVisibility.hideLocalUserPackages` | `boolean` | 常に | ローカルユーザーパッケージを非表示にするか。 |
| `packageVisibility.showPrereleasePackages` | `boolean` | 常に | プレリリースパッケージを表示するか。 |

パッケージの読み取りには返された `id`、ユーザーリポジトリの削除には `url` を使います。

### `alcomd3_add_repository`

リモート VPM リポジトリをダウンロード、検証、追加し、パッケージキャッシュを消去します。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `repository_url` | `string` | はい | 有効なリモート VPM リポジトリ URL。後で削除するときの識別子でもあります。 |
| `headers` | `object<string, string>` | いいえ | ダウンロード時に付ける HTTP header マップ。既定値は空マップ。 |

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `repository` | [`RepositoryMutationSummary`](#repositorymutationsummary) | 常に | 追加したユーザーリポジトリの概要。 |

保存済み URL またはリポジトリ宣言 ID が重複すると拒否されます。アクティビティ記録には
マスク済み URL と header 数だけを保存し、header 値は保存しません。

### `alcomd3_remove_repository`

保存済み URL が完全に一致するユーザーリポジトリを削除し、パッケージキャッシュを消去します。
既定リポジトリは削除できません。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `repository_url` | `string` | はい | `alcomd3_list_repositories` が返したユーザーリポジトリの `url`。ID は受け付けません。 |

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `repository` | [`RepositoryMutationSummary`](#repositorymutationsummary) | 常に | 削除したリポジトリの概要。 |

### `alcomd3_list_packages`

GUI のパッケージ一覧と同じ表示規則で概要をページングします。サーバー側テキスト検索はありません。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `offset` | `integer >= 0` | いいえ | 開始位置。既定値は `0`。 |
| `limit` | `integer >= 0` | いいえ | 要求ページサイズ。既定値 `200`、実際は `1..=1000` に制限。 |

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `totalCount` | `integer` | 常に | 表示フィルターとソース集約後の総数。 |
| `offset` | `integer` | 常に | このページで使った開始位置。 |
| `limit` | `integer` | 常に | 実際のページサイズ。 |
| `returnedCount` | `integer` | 常に | このページの件数。 |
| `hasMore` | `boolean` | 常に | 次ページがあるか。 |
| `nextOffset` | `integer / null` | 常に | 次ページの開始位置。末尾なら `null`。 |
| `packages` | [`PackageSummary[]`](#packagesummary) | 常に | このページのパッケージ概要。 |

### `alcomd3_list_repository_packages`

1 つのリモートリポジトリにある GUI 表示対象パッケージをページングします。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `repository_id` | `string` | はい | `alcomd3_list_repositories` が返した `id`。URL は受け付けません。 |
| `offset` | `integer >= 0` | いいえ | 開始位置。既定値は `0`。 |
| `limit` | `integer >= 0` | いいえ | ページサイズ。既定値 `200`、実際は `1..=1000` に制限。 |

**成功出力:** `alcomd3_list_packages` と同じページングフィールドに加えて次を返します。

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `repository` | [`PackageRepositorySummary`](#packagerepositorysummary) | 常に | 読み取り対象リポジトリの概要。 |
| `packages` | [`PackageSummary[]`](#packagesummary) | 常に | 指定リポジトリだけから返した現在ページ。 |

### `alcomd3_get_package_details`

GUI 表示対象パッケージの詳細メタデータを取得します。絞り込みを省略すると、複数のソースや
バージョンを返す場合があります。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `package_name` | `string` | はい | 空でない完全な VPM パッケージ識別子。 |
| `version` | `string` | いいえ | 完全一致するバージョン文字列。 |
| `repository_id` | `string` | いいえ | 指定リモートリポジトリ ID に限定。URL は受け付けません。 |

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `packages` | [`PackageDetails[]`](#packagedetails) | 常に | 一致する GUI 表示対象パッケージ詳細。1 件以上。 |

### `alcomd3_get_environment_settings`

保存された Unity インストール、起動引数、既定パスを取得します。Unity の起動や追加ディスクの
走査は行いません。

**入力:** フィールドなし。`{}` を渡します。

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `unityInstallations` | [`UnityInstallation[]`](#unityinstallation) | 常に | 登録済み Unity インストール。 |
| `unityLaunchArguments` | `object` | 常に | Unity 起動引数の設定値、既定値、有効値。 |
| `unityLaunchArguments.configured` | `string[] / null` | 常に | ユーザー設定。未設定なら `null`。 |
| `unityLaunchArguments.builtinDefault` | `string[]` | 常に | ALCOMD3 組み込み既定値。 |
| `unityLaunchArguments.effective` | `string[]` | 常に | 現在有効な引数。 |
| `unityLaunchArguments.usesBuiltinDefault` | `boolean` | 常に | 組み込み既定値を使用中か。 |
| `paths` | `object` | 常に | 既定ディレクトリ設定。 |
| `paths.defaultProjectPath` | `string` | 常に | 既定プロジェクトディレクトリ。 |
| `paths.projectBackupPath` | `string` | 常に | プロジェクトバックアップディレクトリ。 |

## アクティビティ記録

### 共通アクティビティフィルター

`alcomd3_search_activity_logs` と `alcomd3_summarize_activity_logs` は次の入力を共有します。

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `search` | `string` | いいえ | operation、summary、target、tool name、client name の大文字小文字を区別しない部分一致。 |
| `sources` | `string[]` | いいえ | `gui`、`mcp`、`deep_link`、`system`。 |
| `kinds` | `string[]` | いいえ | `read`、`write`、`passive`、`open`、`maintenance`。 |
| `statuses` | `string[]` | いいえ | `started`、`succeeded`、`failed`、`cancelled`、`info`。 |
| `visibility` | `string` | いいえ | `important`、`primary`、`secondary`、`technical`、`all`。既定値 `important`。 |
| `operations` | `string[]` | いいえ | 内部 operation 識別子で限定。 |
| `tool_names` | `string[]` | いいえ | MCP ツール名で限定。 |
| `request_id` | `string` | いいえ | MCP request ID で限定。 |
| `target` | `string` | いいえ | 操作対象で限定。 |
| `since` | `RFC 3339 string` | いいえ | 含める最古の時刻。 |
| `until` | `RFC 3339 string` | いいえ | 含める最新の時刻。`since` より前にはできません。 |
| `offset` | `integer >= 0` | いいえ | ページ開始位置。既定値 `0`。 |
| `limit` | `integer >= 0` | いいえ | ページサイズ。既定値 `50`、実際は `1..=200` に制限。 |
| `order` | `string` | いいえ | `newest` または `oldest`。既定値 `newest`。 |

### `alcomd3_search_activity_logs`

共通フィルターでユーザー向けアクティビティ概要をページングします。

**入力:** [共通アクティビティフィルター](#共通アクティビティフィルター)の任意フィールド。
すべて省略できるため `{}` も有効です。

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `totalCount` | `integer` | 常に | 一致したアクティビティ総数。 |
| `offset`, `limit`, `returnedCount`, `hasMore`, `nextOffset` | ページングフィールド | 常に | パッケージ一覧と同じ意味のページ状態。 |
| `entries` | [`ActivityEntrySummary[]`](#activityentrysummary) | 常に | 現在ページの概要。 |

### `alcomd3_get_activity_log_entry`

検索または集計結果の ID で完全なアクティビティを取得します。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `id` | `string` | はい | アクティビティ ID。 |
| `include_details` | `boolean` | いいえ | `details` を含めるか。既定値 `true`。`false` なら空配列。 |

**成功出力:** [`ActivityEntry`](#activityentry) を直接返します。`ok` ラッパーはありません。

### `alcomd3_summarize_activity_logs`

一致するアクティビティを集計し、詳細を読む前に対象範囲を絞り込みます。

**入力:** 共通アクティビティフィルターに加えて次を指定できます。

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `group_by` | `string` | いいえ | `source`、`kind`、`status`、`operation`、`tool_name`、`client_name`、`day`、`hour`。既定値 `source`。 |

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `groupBy` | `string` | 常に | 実際に使用した集計軸。 |
| `totalCount` | `integer` | 常に | 一致したアクティビティ総数。 |
| `totalGroupCount` | `integer` | 常に | ページング前のグループ総数。 |
| `offset`, `limit`, `returnedCount`, `hasMore`, `nextOffset` | ページングフィールド | 常に | グループページの状態。 |
| `groups` | [`ActivitySummaryGroup[]`](#activitysummarygroup) | 常に | 現在ページの集計結果。 |

### `alcomd3_get_activity_log_context`

ログ全体を取得せず、指定アクティビティとその前後を取得します。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `id` | `string` | はい | 中心となるアクティビティ ID。 |
| `before` | `integer >= 0` | いいえ | 前の件数。既定値 `5`、最大 `50`。 |
| `after` | `integer >= 0` | いいえ | 後の件数。既定値 `5`、最大 `50`。 |
| `include_details` | `boolean` | いいえ | 3 グループすべてに詳細を含めるか。既定値 `false`。 |

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `entry` | [`ActivityEntry`](#activityentry) | 常に | 中心のエントリ。 |
| `before` | [`ActivityEntry[]`](#activityentry) | 常に | 中心より前のエントリ。 |
| `after` | [`ActivityEntry[]`](#activityentry) | 常に | 中心より後のエントリ。 |

## 技術ログ

### 共通技術ログフィルター

`alcomd3_search_technical_logs` と `alcomd3_summarize_technical_logs` は次を共有します。

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `search` | `string` | いいえ | target と message の大文字小文字を区別しない部分一致。 |
| `levels` | `string[]` | いいえ | `error`、`warn`、`info`、`debug`、`trace`。既定値は `error` と `warn`。 |
| `targets` | `string[]` | いいえ | target の大文字小文字を区別しない部分一致。 |
| `scope` | `string` | いいえ | `memory` または `recent_files`。既定値 `memory`。 |
| `since` | `RFC 3339 string` | いいえ | 含める最古の時刻。 |
| `until` | `RFC 3339 string` | いいえ | 含める最新の時刻。`since` より前にはできません。 |
| `offset` | `integer >= 0` | いいえ | ページ開始位置。既定値 `0`。 |
| `limit` | `integer >= 0` | いいえ | ページサイズ。既定値 `50`、実際は `1..=100` に制限。 |
| `max_message_chars` | `integer >= 0` | いいえ | 検索プレビューの上限。既定値・最大値とも `300`。集計には本文を含めません。 |

### `alcomd3_search_technical_logs`

マスクされ、長さを制限した技術ログプレビューをページングします。

**入力:** [共通技術ログフィルター](#共通技術ログフィルター)の任意フィールド。すべて省略できます。

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `totalCount` | `integer` | 常に | 一致したログ総数。 |
| `offset`, `limit`, `returnedCount`, `hasMore`, `nextOffset` | ページングフィールド | 常に | 現在ページの状態。 |
| `entries` | [`TechnicalLogEntrySummary[]`](#technicallogentrysummary) | 常に | 現在ページのプレビュー。 |

### `alcomd3_get_technical_log_entry`

検索結果を 1 件取得します。メッセージはマスクしてから切り詰めます。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `id` | `string` | はい | 検索結果が返した技術ログ ID。 |
| `max_message_chars` | `integer >= 0` | いいえ | メッセージ上限。既定値・最大値とも `4000`。 |

**成功出力:** [`TechnicalLogEntryDetails`](#technicallogentrydetails) を直接返します。
`ok` ラッパーはありません。

### `alcomd3_summarize_technical_logs`

一致する技術ログを集計します。

**入力:** 共通技術ログフィルターに加えて次を指定できます。

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `group_by` | `string` | いいえ | `level`、`target`、`file`、`hour`。既定値 `level`。 |

**成功出力:**

| フィールド | 型 | 条件 | 意味 |
| --- | --- | --- | --- |
| `ok` | `boolean` | 常に | 成功時は `true`。 |
| `groupBy` | `string` | 常に | 実際に使用した集計軸。 |
| `totalCount` | `integer` | 常に | 一致したログ総数。 |
| `totalGroupCount` | `integer` | 常に | ページング前のグループ総数。 |
| `offset`, `limit`, `returnedCount`, `hasMore`, `nextOffset` | ページングフィールド | 常に | グループページの状態。 |
| `groups` | [`TechnicalLogSummaryGroup[]`](#technicallogsummarygroup) | 常に | 現在ページの集計結果。 |

## プロジェクトパッケージの書き込み

### `alcomd3_install_project_package`

プロジェクトの Unity バージョンと互換性がある GUI 表示対象候補から 1 パッケージを
インストールします。MCP Task を任意で利用できます。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `project_path` | `string` | はい | 登録済みプロジェクトパス。 |
| `package_name` | `string` | はい | 有効な完全 VPM パッケージ識別子。 |
| `version_selector` | `object` | はい | `{"type":"latest_gui_visible"}` または `{"type":"exact","version":"x.y.z"}`。完全指定でも表示対象かつ互換である必要があります。 |
| `source` | `object` | いいえ | 任意のリモートリポジトリ指定。 |
| `source.repository_id` | `string` | `source` 指定時は必須 | `alcomd3_list_repositories` が返したリポジトリ `id`。URL は受け付けません。 |
| `allow_conflicts` | `boolean` | いいえ | 依存関係の競合または legacy ファイル・ディレクトリ削除を許可。既定値 `false`。 |

**成功出力:** [`ProjectPackageChangeResult`](#projectpackagechangeresult)。

### `alcomd3_uninstall_project_package`

インストール済みパッケージをアンインストールします。任意の MCP Task に対応し、
destructive としてマークされます。

**入力:**

| フィールド | 型 | 必須 | 意味 |
| --- | --- | --- | --- |
| `project_path` | `string` | はい | 登録済みプロジェクトパス。 |
| `package_name` | `string` | はい | プロジェクトにインストール済みの有効な VPM パッケージ識別子。 |
| `allow_conflicts` | `boolean` | いいえ | 競合または legacy 削除を許可。既定値 `false`。 |

**成功出力:** [`ProjectPackageChangeResult`](#projectpackagechangeresult)。

### `alcomd3_reinstall_project_package`

インストール済みパッケージを再インストールします。MCP Task を任意で利用できます。

**入力:** `alcomd3_uninstall_project_package` と同じです。

**成功出力:** [`ProjectPackageChangeResult`](#projectpackagechangeresult)。

3 ツールとも先に変更予定を計算します。`allow_conflicts` が `false` のまま明示的な許可が必要に
なった場合、プロジェクトは変更せず、`project_package_conflicts` と
`error.data.changes` 内の [`PendingChanges`](#pendingchanges) を返します。

## 共通出力型

### `ProjectSummary`

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `name` | `string / null` | プロジェクト表示名。 |
| `path` | `string` | 登録パス。 |
| `projectType` | `string` | バックエンドが判定したプロジェクト種別。 |
| `unity` | `string / null` | Unity バージョン。 |
| `unityRevision` | `string / null` | Unity revision。 |
| `lastModified` | `integer / null` | 最終更新 Unix ミリ秒。 |
| `createdAt` | `integer / null` | 作成 Unix ミリ秒。 |
| `favorite` | `boolean` | お気に入りか。 |
| `exists` | `boolean` | 登録ディレクトリが現在存在するか。 |

### `TemplateSummary`

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `name` | `string` | VPM リポジトリが宣言した元の名前。 |
| `displayName` | `string` | 空でない表示名。初期値は `name` で、ユーザーが編集できます。 |
| `id` | `string` | 安定した管理 ID。 |
| `unityVersions` | `string[]` | プロジェクト作成で選択できる Unity バージョン。 |
| `updateDate` | `string / null` | テンプレート更新日。 |
| `hasUnityPackages` | `boolean` | Unity package を参照しているか。 |
| `hasProjectArchive` | `boolean` | プロジェクトアーカイブを含むか。 |
| `available` | `boolean` | 現在利用可能か。 |
| `kind` | `builtIn / derived / projectArchive` | テンプレート種別。 |
| `editable` | `boolean` | フィールド編集できるか。 |
| `removable` | `boolean` | 削除できるか。 |
| `usableAsBase` | `boolean` | 派生テンプレートの基底にできるか。 |

### `TemplateDetails`

[`TemplateSummary`](#templatesummary) の全フィールドに加えて次を含みます。

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `baseTemplateId` | `string / null` | 派生テンプレートの基底 ID。 |
| `unityVersionRange` | `string / null` | 派生テンプレートの Unity バージョン範囲。 |
| `vpmDependencies` | `object<string, string>` | パッケージ名からバージョン範囲へのマップ。 |
| `unityPackagePaths` | `string[]` | 参照する絶対添付パス。 |

### `RemovedTemplate`

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `id` | `string` | 削除したテンプレート ID。 |
| `displayName` | `string` | 削除したテンプレート名。 |
| `kind` | `builtIn / derived / projectArchive` | 削除したテンプレート種別。 |

### `ProjectDetails`

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `path` | `string` | プロジェクトパス。 |
| `unity.major` | `integer` | Unity メジャーバージョン。 |
| `unity.minor` | `integer` | Unity マイナーバージョン。 |
| `unity.version` | `string` | 完全な Unity バージョン。 |
| `unity.revision` | `string / null` | Unity revision。 |
| `shouldResolve` | `boolean` | パッケージを再解決する必要があるか。 |
| `installedPackages` | `object[]` | インストール済みパッケージ。 |
| `installedPackages[].id` | `string` | プロジェクト依存関係内のパッケージ ID。 |
| `installedPackages[].package` | [`PackageDetails`](#packagedetails) | `source` を含まないインストール済み manifest 概要。 |

### `RepositorySummary`

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `id` | `string` | パッケージ読み取り用の識別子。宣言 ID がなければ URL にフォールバック。 |
| `url` | `string` | リモート URL。ユーザーリポジトリ削除時に使用。 |
| `displayName` | `string` | 表示名。 |
| `kind` | `officialDefault / curatedDefault / user` | 唯一のリポジトリ分類フィールド。 |
| `hidden` | `boolean` | GUI のパッケージ表示設定で現在非表示か。 |

### `RepositoryMutationSummary`

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `id` | `string / null` | リポジトリ宣言 ID。なければ `null`。 |
| `url` | `string` | 追加または削除した URL。 |
| `name` | `string / null` | VPM リポジトリが宣言した元の名前。 |
| `displayName` | `string` | 操作時の空でない表示名。 |
| `kind` | `user` | 常にユーザーリポジトリ。 |

### `PackageRepositorySummary`

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `id` | `string / null` | キャッシュ済みリポジトリ ID。 |
| `url` | `string / null` | キャッシュ済みリポジトリ URL。 |
| `name` | `string` | VPM リポジトリが宣言した元の名前。 |
| `displayName` | `string` | 空でないリポジトリ表示名。 |
| `kind` | `officialDefault / curatedDefault / user` | リポジトリ分類。 |

### `PackageSummary`

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `name` | `string` | 完全な VPM パッケージ識別子。 |
| `displayName` | `string / null` | 表示名。 |
| `version` | `string` | バージョン。 |
| `source` | [`PackageSource`](#packagesource) | パッケージソース。 |

### `PackageSource`

リモートソース:

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `type` | `remote` | リモートリポジトリのパッケージであることを示します。 |
| `kind` | `officialDefault / curatedDefault / userRepository` | リモートリポジトリ分類。 |
| `id` | `string / null` | リポジトリ宣言 ID。 |
| `name` | `string` | VPM リポジトリが宣言した元の名前。 |
| `displayName` | `string` | 空でないリポジトリ表示名。 |
| `url` | `string / null` | リポジトリ URL。 |

ローカルユーザーソース:

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `type` | `localUser` | 登録済みローカルユーザーパッケージディレクトリからのパッケージであることを示します。 |
| `kind` | `localUser` | 固定のローカルユーザー分類。 |
| `isLocalUserPackage` | `true` | ローカルユーザーパッケージであることを明示します。 |

### `PackageDetails`

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `name` | `string` | 完全なパッケージ識別子。 |
| `displayName` | `string / null` | 表示名。 |
| `description` | `string / null` | 説明。 |
| `version` | `string` | バージョン。 |
| `unity` | `object / null` | Unity 要件。存在時は `major` と `minor` を含みます。 |
| `keywords` | `string[]` | キーワード。 |
| `aliases` | `string[]` | パッケージ別名。 |
| `vpmDependencies` | `string[]` | バージョン範囲を除いた依存パッケージ ID。 |
| `legacyPackages` | `string[]` | 置き換え対象の legacy パッケージ。 |
| `changelogUrl` | `string / null` | 変更履歴 URL。 |
| `documentationUrl` | `string / null` | ドキュメント URL。 |
| `isYanked` | `boolean` | このバージョンが yanked か。 |
| `source` | [`PackageSource`](#packagesource) | パッケージ詳細では存在し、インストール済み manifest 概要では省略。 |

### `ActivityEntrySummary`

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `id` | `string` | 詳細・前後関係ツールで使うアクティビティ ID。 |
| `startedAt` | `RFC 3339 string` | 開始時刻。 |
| `finishedAt` | `RFC 3339 string / null` | 完了時刻。未完了なら `null`。 |
| `source` | `string` | `Gui`、`Mcp`、`DeepLink`、`System` などの発生元。 |
| `kind` | `string` | `Read`、`Write`、`Maintenance` などの動作種別。 |
| `status` | `string` | `Started`、`Succeeded`、`Failed` などの現在または最終状態。 |
| `importance` | `string` | `Primary`、`Secondary`、`Technical` の表示レベル。 |
| `operation` | `string` | 安定した内部操作識別子。 |
| `summary` | `string` | ユーザー向けの短い説明。 |
| `target` | `string / null` | 操作対象のリソースまたはパス。 |
| `durationMs` | `integer / null` | 完了した処理の所要時間（ミリ秒）。 |
| `requestId` | `string / null` | 関連する MCP request ID。 |
| `toolName` | `string / null` | 関連する MCP ツール名。 |
| `clientName` | `string / null` | 呼び出した MCP クライアント名。 |
| `detailCount` | `integer` | 完全なエントリに含まれる key/value 詳細数。 |
| `hasError` | `boolean` | 完全なエントリにエラーがあるか。 |
| `errorSummary` | `string / null` | マスクして切り詰めたエラー概要。 |

### `ActivityEntry`

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `id` | `string` | アクティビティ ID。 |
| `source` | `string` | 現在の値は `Gui`、`Mcp`、`DeepLink`、`System`。 |
| `kind` | `string` | 現在の値は `Read`、`Write`、`Passive`、`Open`、`Maintenance`。 |
| `status` | `string` | 現在の値は `Started`、`Succeeded`、`Failed`、`Cancelled`、`Info`。 |
| `importance` | `string` | `Primary`、`Secondary`、`Technical`。 |
| `operation` | `string` | 安定した内部操作識別子。 |
| `summary` | `string` | ユーザー向けのアクティビティ説明。 |
| `target` | `string / null` | 操作対象。 |
| `details` | [`ActivityDetail[]`](#activitydetail) | マスク済みの構造化詳細。`include_details: false` なら空配列。 |
| `requestId` | `string / null` | 関連する MCP request ID。 |
| `toolName` | `string / null` | 関連する MCP ツール名。 |
| `clientName` | `string / null` | MCP クライアント名。 |
| `startedAt` | `RFC 3339 string` | 開始時刻。 |
| `finishedAt` | `RFC 3339 string / null` | 完了時刻。 |
| `durationMs` | `integer / null` | 所要時間（ミリ秒）。 |
| `error` | `string / null` | マスク済みの完全なエラー本文。 |

入力フィルターの enum は小文字の `snake_case`、上表は現在のシリアライズ済み出力値です。

### `ActivityDetail`

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `key` | `string` | 詳細キー。 |
| `value` | `string` | マスク済みの詳細値。 |

### `ActivitySummaryGroup`

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `key` | `string` | グループキー。 |
| `count` | `integer` | レコード数。 |
| `failedCount` | `integer` | 失敗数。 |
| `cancelledCount` | `integer` | キャンセル数。 |
| `latestEntryId` | `string / null` | グループ内の最新 ID。 |
| `latestStartedAt` | `string / null` | グループ内の最新開始時刻。 |

### `TechnicalLogEntrySummary`

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `id` | `string` | 詳細ツールで使う技術ログ ID。 |
| `time` | `RFC 3339 string` | ログ時刻。 |
| `level` | `string` | 現在の値は `Error`、`Warn`、`Info`、`Debug`、`Trace`。 |
| `target` | `string` | ログを出力した Rust target。 |
| `messagePreview` | `string` | マスクし、検索上限で切り詰めたプレビュー。 |
| `truncated` | `boolean` | メッセージが切り詰められたか。 |
| `source` | `memory / file` | 現在のプロセスメモリまたは最近のログファイル。 |
| `fileName` | `string / null` | 元のファイル名。メモリログなら `null`。 |
| `lineNumber` | `integer / null` | 元ファイルの行番号。メモリログなら `null`。 |

### `TechnicalLogEntryDetails`

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `id` | `string` | 技術ログ ID。 |
| `time` | `RFC 3339 string` | ログ時刻。 |
| `level` | `string` | `Error`、`Warn`、`Info`、`Debug`、`Trace`。 |
| `target` | `string` | ログを出力した Rust target。 |
| `message` | `string` | マスクし、詳細要求の上限で切り詰めたメッセージ。 |
| `truncated` | `boolean` | メッセージが切り詰められたか。 |
| `source` | `memory / file` | ログの発生元。 |
| `fileName` | `string / null` | 元のファイル名。メモリログなら `null`。 |
| `lineNumber` | `integer / null` | 元ファイルの行番号。メモリログなら `null`。 |

### `TechnicalLogSummaryGroup`

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `key` | `string` | グループキー。 |
| `count` | `integer` | ログ件数。 |
| `errorCount` | `integer` | Error 件数。 |
| `warnCount` | `integer` | Warn 件数。 |
| `latestEntryId` | `string / null` | グループ内の最新ログ ID。 |
| `latestTime` | `string / null` | グループ内の最新時刻。 |

### `ProjectPackageChangeResult`

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `ok` | `boolean` | 成功時は `true`。 |
| `operation` | `install / uninstall / reinstall` | 実行した操作。 |
| `projectPath` | `string` | 変更したプロジェクトパス。 |
| `packageName` | `string` | 対象パッケージ識別子。 |
| `changes` | [`PendingChanges`](#pendingchanges) | 適用した変更概要。 |

### `PendingChanges`

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `changes_version` | `integer` | バックエンドの変更スナップショットバージョン。 |
| `package_changes` | `[string, PackageChange][]` | パッケージ名とインストール・削除変更のタプル一覧。 |
| `remove_legacy_files` | `string[]` | 削除する legacy ファイル。 |
| `remove_legacy_folders` | `string[]` | 削除する legacy ディレクトリ。 |
| `conflicts` | `[string, ConflictInfo][]` | パッケージ名と競合詳細のタプル一覧。 |

`PackageChange` は `{ "InstallNew": PackageInfo }` または
`{ "Remove": "Requested" / "Legacy" / "Unused" }` です。Remove 値はそれぞれ、
ユーザー要求、他パッケージによる置換、依存関係として不要になったことを示します。

### `PackageInfo`

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `name` | `string` | インストールする完全なパッケージ識別子。 |
| `display_name` | `string / null` | パッケージ表示名。 |
| `description` | `string / null` | パッケージ説明。 |
| `keywords` | `string[]` | 統合したパッケージ別名とキーワード。 |
| `version` | `object` | `major`、`minor`、`patch`、`pre`、`build` を含む構造化 SemVer。 |
| `unity` | `[integer, integer] / null` | 必要な Unity メジャー・マイナーバージョン。 |
| `changelog_url` | `string / null` | 変更履歴 URL。 |
| `documentation_url` | `string / null` | ドキュメント URL。 |
| `vpm_dependencies` | `string[]` | 依存パッケージ識別子。 |
| `legacy_packages` | `string[]` | 置き換え対象の legacy パッケージ。 |
| `is_yanked` | `boolean` | このバージョンが yanked か。 |

### `ConflictInfo`

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `packages` | `string[]` | 対象変更と競合するパッケージ識別子。 |
| `unity_conflict` | `boolean` | Unity バージョンの競合があるか。 |
| `unlocked_names` | `string[]` | 変更適用時にロック解除が必要なパッケージ識別子。 |

### `UnityInstallation`

| フィールド | 型 | 意味 |
| --- | --- | --- |
| `path` | `string` | Unity 実行ファイルパス。 |
| `version` | `string` | 登録された完全な Unity バージョン。 |
| `loadedFromHub` | `boolean` | Unity Hub の記録から読み込んだインストールか。 |
