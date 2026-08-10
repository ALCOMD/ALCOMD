# ALCOMD3 MCP ガイド

言語: [English](../mcp.md) | 日本語 | [简体中文](mcp.zh-CN.md) | [繁體中文](mcp.zh-TW.md)

このドキュメントは、ALCOMD3 の MCP 接続方法、利用可能な tools、ライフサイクル挙動、トラブルシューティングを説明します。

ALCOMD3 は RMCP 3.1.2 で MCP `2026-07-28` を実装し、`2025-11-25`
client の通常 tool call との互換性も維持します。MCP server は GUI process の一部です。
MCP extension が enabled の間、GUI は implementation name `alcomd3-mcp` で
`127.0.0.1` に local Streamable HTTP endpoint を公開します。helper process、private
IPC listener、endpoint metadata file は存在しません。

## クイックスタート

1. ALCOMD3 を起動し、Extensions page で MCP extension が enabled であることを確認してから、sidebar の MCP page を開きます。
2. MCP を有効化します。
3. 既定では page に表示される MCP Endpoint と Authorization Token を使って手動で
   client を設定します。Windows の Codex、Claude Code、Cursor user は、対応する
   任意のクイック設定 button も選択できます。
4. 手動設定では URL を Streamable HTTP server として追加し、token を
   `Authorization: Bearer <token>` で送信します。
5. ALCOMD3 で MCP を有効にしたまま tool call を実行します。

port を推測せず、GUI に表示される endpoint を使用してください。設定例と
ライフサイクルの詳細は [MCP の有効化と client 設定](#mcp-の有効化と-client-設定)
を参照してください。

## 現在の境界

- MCP は既定で無効です。新しい tool call が ALCOMD3 data を読み書きするには、GUI で手動で有効化する必要があります。
- MCP extension が enabled の間、GUI は local Streamable HTTP endpoint を起動します。MCP page での MCP の有効/無効は新しい tool data access を制御するだけで、endpoint は停止しません。
- Extensions page で MCP extension を disabled にすると、MCP access を取り消し、endpoint を停止し、MCP を sidebar から削除して、GUI が管理中の MCP project task を cancel します。extension を再度 enabled にする switch operation はすぐに完了し、endpoint は background で再起動しますが、MCP page で user が再度有効化するまで MCP access は無効のままです。
- 現在は project、環境レベルの template、repository、package、environment、activity log、technical log の read-only tools と、限定的な write tools を提供します。write tools は project 作成、環境レベルの template の作成/編集/削除、派生 template における直接 VPM 依存関係または UnityPackage 添付参照 1 件の設定/削除、existing project 追加、user VPM repository の追加/削除、registered project の backup、registered project の copy、zip backup からの restore、registered project への package install/uninstall/reinstall です。repository 並べ替え、project 削除などの他の write operation は提供しません。
- MCP extension が enabled の間、GUI が local Streamable HTTP server を起動して管理します。GUI 終了時または extension の無効化時には server も停止します。
- MCP tool の使用中は ALCOMD3 を起動したままにする必要があります。GUI を終了すると public loopback endpoint も停止します。
- MCP が無効な場合、新しい tool call は structured `mcp_disabled` error を返します。endpoint は停止せず、panic もしません。MCP tool result には `isError: true` が付きます。既に開始された project long task の `tasks/get` と `tasks/cancel` は cleanup 例外として、結果確認や cancel ができます。
- embedded server は tool call に local rate limit と concurrency protection を適用します。制限を超えた場合は structured `rate_limited` error を返し、MCP tool result に `isError: true` を付けます。
- GUI MCP page は、既知の tool call 実行中に該当 tool を highlight し、完了または失敗後も短時間 highlight を残します。
- GUI MCP page は tools を read-only、write、log 用途で group 表示し、正確な MCP name を保持します。tool name に hover すると localized readable name を表示します。
- GUI MCP page に表示される client は最近 activity があった client であり、live connection list ではありません。一定時間 activity がない record は自動で隠れます。
- Logs extension が enabled の間、MCP tool call は GUI の local activity log に記録
  されます。record には source、tool name、request id、client summary、
  started/completed/failed/cancelled state、安全処理済み target/details が含まれ、
  ユーザーは GUI の Activity page で Agent が何をしたか確認できます。
- GUI project management page と MCP package tools は backend の GUI-visible package catalog を共有します。pre-release、yanked、hidden repository、hidden local user package、同名 package の source 間 merge、default/user repository priority、Unity compatibility は backend が統一して処理します。
- すべての public MCP tool は GUI の既存 capability に mapping され、`vrc-get-gui/src/backend/` の shared backend service を通して business logic に入ります。MCP dispatch は enabled-state gate、argument parsing、task wrapping、error mapping、activity logging だけを担当し、GUI にない business capability を追加しません。
- Streamable HTTP request には local ALCOMD3 installation 用の bearer token が必要です。
- HTTP server は `Host` と `Origin` を検証し、`127.0.0.1` に厳密に bind します。LAN や public address では listen しません。

Activity record は raw MCP params、token-like field、HTTP header value、query 付き URL、URL userinfo credential を保存しません。local filesystem path は Unity、VPM、非 ASCII path の診断に必要なため完全な値を保持します。MCP access には GUI で MCP を有効化する必要があります。

## アーキテクチャ

```text
MCP Host / Client
        |
        | Streamable HTTP + bearer token
        v
ALCOMD3 GUI process
        |
        +-- embedded alcomd3-mcp HTTP/RMCP service
        +-- direct in-process GUI business dispatch
        +-- shared Tasks, cancellation, locks, and activity state
```

認証済み request はすべて GUI process 内で処理されます。tool handler は GUI と同じ
backend service を直接呼び出し、TCP bridge や JSON-line serialization を使いません。
認証済み local client は 1 つの bearer principal と task namespace を共有します。
client name/version は recent activity と log の表示だけに使います。

## MCP の有効化と client 設定

1. ALCOMD3 を起動します。
2. Extensions page で MCP extension が enabled であることを確認してから、sidebar の MCP page を開きます。
3. MCP を有効化し、MCP tools が ALCOMD3 data を読めるようにします。
4. page の MCP Endpoint と Authorization Token をコピーします。
5. Streamable HTTP MCP server に対応する client で endpoint URL を追加し、token を
   bearer `Authorization` header に設定します。

一般的な設定形は次の通りです。正確な field name は MCP client に従ってください。

```json
{
    "mcpServers": {
        "alcomd3": {
            "url": "http://127.0.0.1:51739/mcp",
            "headers": {
                "Authorization": "Bearer <ALCOMD3 に表示された token>"
            }
        }
    }
}
```

手動設定が既定であり、OS や AI client の設定を自動変更しません。

### Windows での任意の AI client クイック設定

Windows の MCP page には Codex、Claude Code、Cursor それぞれのクイック設定 button
があります。明示的に button をクリックした client だけを変更します。各 button は
現在の token を Windows current user の `ALCOMD3_MCP_BEARER_TOKEN` environment
variable に書き込み、選択した client の ALCOMD3 MCP entry だけを追加または更新します。

- Codex: `$CODEX_HOME/config.toml` を使います。`CODEX_HOME` が未設定の場合は
  `~/.codex/config.toml` を使います。

```toml
[mcp_servers.alcomd3]
url = "http://127.0.0.1:51739/mcp"
bearer_token_env_var = "ALCOMD3_MCP_BEARER_TOKEN"
```

- Claude Code: `$CLAUDE_CONFIG_DIR/.claude.json` を使います。
  `CLAUDE_CONFIG_DIR` が未設定の場合は `~/.claude.json` を使います。

```json
{
    "mcpServers": {
        "alcomd3": {
            "type": "http",
            "url": "http://127.0.0.1:51739/mcp",
            "headers": {
                "Authorization": "Bearer ${ALCOMD3_MCP_BEARER_TOKEN}"
            }
        }
    }
}
```

- Cursor: `~/.cursor/mcp.json` を使います。

```json
{
    "mcpServers": {
        "alcomd3": {
            "type": "http",
            "url": "http://127.0.0.1:51739/mcp",
            "headers": {
                "Authorization": "Bearer ${env:ALCOMD3_MCP_BEARER_TOKEN}"
            }
        }
    }
}
```

選択した client の他の settings と MCP servers は保持されます。environment variable
またはその client の `alcomd3` entry に異なる既存値がある場合、ALCOMD3 は置換前に
確認を求めます。設定後は選択した client を完全に終了して再起動し、user environment
と MCP configuration を再読込してください。

正確な field name は client ごとに異なります。現在の URL と token は GUI からコピーしてください。default port は `51739` です。advanced user は ALCOMD3 起動前に `gui-config.json` の `mcpHttpPort` を変更できます。

endpoint は ALCOMD3 が実行中で MCP extension が enabled の間だけ利用できます。MCP page で access が無効なら、新しい tool call は `mcp_disabled` を返します。MCP を有効化して再試行してください。MCP extension を disabled にすると endpoint が停止し、GUI が管理中の MCP project task は cancel されます。

external HTTP port と bearer token は `gui-config.json` の `mcpHttpPort`、
`mcpHttpToken` に保存されます。token は local secret として扱い、log、screenshot、
shared config に含めないでください。

## Built-in runtime と保存設定

サポート対象の installer/archive は MCP 用に GUI executable だけを含み、配置や起動が必要な
`alcomd3-mcp` helper は含みません。`cargo xtask build-alcom` は GUI だけを build し、
GUI process が HTTP listener と全 tool execution を所有します。

public port と bearer token は `gui-config.json` の `mcpHttpPort`、`mcpHttpToken` に
保存されます。`mcp/endpoint.json` は read/write/migrate されません。
`ALCOMD3_MCP_ENDPOINT_FILE` と internal-listener override は削除されました。
client configuration 用の `ALCOMD3_MCP_BEARER_TOKEN` は引き続き利用できます。

port 変更または token rotation は embedded transport を順番に stop/rebind します。
transport restart 中も shared protocol task state は GUI に残ります。新しい port の bind
に失敗しても GUI の他機能は動作し、MCP page は server not running を表示して technical
log に failure を記録します。

## 利用可能なツール

ALCOMD3 は現在 33 個のツールを公開しています。メインガイドは利用方法と安全境界を簡潔に
説明し、[完全なツールリファレンス](tools.ja.md)は各入力・出力フィールドについて、必須か
条件付きか、省略時の既定値、フィールドの意味を記載します。

| 分類 | 読み取りツール | 書き込みツール |
| --- | --- | --- |
| プロジェクト | 一覧と詳細 | 作成、登録、バックアップ、コピー、復元 |
| テンプレート | 一覧と詳細 | 作成、編集、VPM 依存関係と UnityPackage 参照の設定/削除、テンプレート削除 |
| リポジトリ | リポジトリ一覧 | リモートユーザーリポジトリの追加と削除 |
| パッケージ | 一覧と詳細 | プロジェクトパッケージのインストール、アンインストール、再インストール |
| 環境 | Unity インストール、起動引数、既定パス | なし |
| ログ | 検索、詳細、前後関係、集計 | なし |

ツールリファレンスにはページングの既定値、許可される enum、MCP Task 対応、共通出力型、
エラー形式、および設計上 `ok` なしで詳細オブジェクトを直接返す 2 ツールも記載しています。

### ログ照会ツール

Log tools は activity records と technical logs に分かれています。Agent が 1 つの問題を調査するためにすべての logs を context に取り込む必要を減らします。

- Activity records は user-readable、structured、redacted 済みの operation history です。`alcomd3_search_activity_logs` の default `visibility` は `important` で、write、failure、cancellation、重要な MCP/System behavior を返します。補助 record が必要な場合は `secondary`、`technical`、`all` を明示してください。
- Activity search result は summary fields のみ返します。id、time、source、kind、status、operation、target、duration、error summary が含まれます。detail は `alcomd3_get_activity_log_entry`、周辺 context は `alcomd3_get_activity_log_context` を呼びます。
- Technical logs は diagnostics 用です。default は current process memory の `error` と `warn` だけです。recent files を読むには `"scope": "recent_files"` を渡し、Info/Debug/Trace が必要なら `levels` を明示します。
- Technical log tools は無制限の raw text を返しません。search は `messagePreview` を返し、detail は `max_message_chars` で truncate され、token、secret、authorization、API key、`sk-` values、URL userinfo、query、fragment を redact します。
- Log tools 自体も MCP read activity として記録されます。成功した log read は Secondary、失敗は failed activity として表示されます。

### プロジェクト長時間タスク

ALCOMD3 は RMCP 3.1.2 の実験的な `io.modelcontextprotocol/tasks` extension を
使用します。client ごとに対応状況が異なり、extension は将来変更される可能性があります。

`alcomd3_create_project`、`alcomd3_backup_project`、`alcomd3_copy_project`、`alcomd3_restore_project_from_backup`、`alcomd3_install_project_package`、`alcomd3_uninstall_project_package`、`alcomd3_reinstall_project_package` は、client が `io.modelcontextprotocol/tasks` capability を宣言した場合に task-aware call を使います。

- `tools/call` は `taskId` を含む task handle をすぐに返します。
- `tasks/get` は `working`、`input_required`、`completed`、`failed`、`cancelled` を返し、
  detailed task state に completed result または failure を含めます。
- `tasks/update` は running task が要求した response を提供します。
- `tasks/cancel` は underlying GUI operation を cooperative に cancel し、対応する resource lock を release します。
- `alcomd3_create_project` は formal registration 前に cancel された場合、または package resolve/apply に失敗した場合、MCP が作成した未登録 project directory を cleanup します。
- task 実行中に user が MCP を無効化した場合、新しい tool call と task startup は
  `mcp_disabled` を返します。既存 `taskId` は認証済み `tasks/get` で query し、
  `tasks/cancel` で cancel できます。
- MCP extension 全体を disabled にするか GUI を終了すると、unfinished task を cancel し、
  protocol state を clear します。

この extension は旧 core Tasks の `tasks/list` と `tasks/result` を意図的に提供しません。
completed output は `tasks/get` から読みます。synchronous `tools/call` に
`_meta.progressToken` がある場合は標準 `notifications/progress` を送信し、task-aware call
も backend progress に応じて human-readable status を更新します。

Tasks capability を宣言しない client は従来どおり synchronous `tools/call` と同じ
result shape を受け取ります。

### Path restrictions

`alcomd3_get_project_details`、`alcomd3_backup_project`、`alcomd3_copy_project`、project package install/uninstall/reinstall tools の source project path は、ALCOMD3 database に登録済みの project path のみ受け付けます。MCP client はこれらの tools で任意の local path を read/copy できません。

`alcomd3_get_environment_settings` は ALCOMD3 が保存した local paths を返します。例: Unity executable、default project directory、backup directory。この tool は Unity を起動せず、Unity Hub refresh も行わず、追加 disk path scan もしません。

`alcomd3_backup_project` の `backup_name` は path ではなく、合法な file name 1 つだけを受け付けます。自動追加される `.zip` extension は含めません。archive は常に GUI-configured backup directory に書き込まれ、既存 archive は上書きしません。

`alcomd3_copy_project` の `new_project_path` は absolute path、まだ存在しない directory path、かつ source project 内部ではない必要があります。tool は directory を作成し、project files を copy し、新 project を登録し、失敗時は新 directory を cleanup します。`alcomd3_restore_project_from_backup` の `backup_path` は absolute path で、GUI-configured default project directory にのみ restore します。`project_name` は合法な folder name 1 つだけで、path separator、root path、`..` は使えません。`alcomd3_create_project` の `project_name` も同じ single-folder-name restriction を受けます。明示的な `base_path` は absolute path である必要があります。`base_path` 省略時は GUI default project path を使います。`alcomd3_add_existing_project` の `project_path` は absolute path で、有効な Unity project として load できる必要があります。

### Package visibility and write limits

`alcomd3_list_packages` と `alcomd3_list_repository_packages` は GUI package page と同じ package-state load path を使い、force-refresh path は呼びません。results は GUI の pre-release、hidden repository、hidden local user package、yanked filters に従います。MCP tool call は server-side search を行いません。repository 追加には明示的な `alcomd3_add_repository` call が必要です。list tools は暗黙に repository を追加したり、repository refresh strategy を作り直したりしません。

Repository parameter の役割は分離されています。package read tool と install tool は `alcomd3_list_repositories` が返した `id` で repository を選択します。user repository の add/remove は stored URL を使うため、remove input は add input と直接対応し、built-in default repository は削除対象になりません。duplicate check は stored URL と publisher-declared repository ID の両方に適用されます。GUI の add/remove/reorder も同じ shared URL-based backend を使います。local repository はサポートしません。URL-less user-repository entry は settings の load 時に破棄し、local repository の作成経路も提供しません。

GUI project-management package table は backend が同名 package merge logic から生成します。MCP package lists、package details、project package install selection は同じ backend rules を使います。

- "Show pre-release packages" が off の場合、GUI と MCP の GUI-visible results は pre-release versions を含みません。MCP `latest_gui_visible` も pre-release version を選択できません。underlying cache は pre-release data を保持できますが、setting を再有効化するまで visible results には入りません。
- Yanked package は visible candidate に入りません。installed package version が現在 yanked の場合、project package row は yanked marker を保持します。
- Hidden repositories と hidden local user packages は visible candidate にだけ影響します。hidden source は "existing source" information として表示されることがありますが、latest-version selection には参加しません。
- source 間の同名 packages は project management page で 1 行に merge されます。default repositories、local user packages、user repositories、unregistered repositories は backend order で merge されます。
- project package installation は、GUI-visible かつ project の Unity version と互換性がある candidate だけを選択します。

`alcomd3_install_project_package`、`alcomd3_uninstall_project_package`、`alcomd3_reinstall_project_package` は最初に pending project changes を生成します。結果に dependency conflicts または legacy file/folder deletion が含まれ、`"allow_conflicts": true` が渡されていない場合、tool は `project_package_conflicts` を返し、`error.data.changes` に change summary を含めます。この場合 project には適用されません。確認後、`"allow_conflicts": true` を設定して再試行すると apply します。

Package list tools は discovery/filter 用の summary fields だけを返します: `name`、`displayName`、`version`、`source`。`totalCount` と paging fields は aggregated summary rows から計算され、raw repository version list の length ではありません。description、keywords、dependencies、legacy packages、documentation URL、changelog URL、Unity version requirements を読むには、list から candidate を選び、`alcomd3_get_package_details` を呼びます。

Package list tools の default `offset` は `0`、`limit` は `200` です。`limit` max は `1000` です。paging response は `totalCount`、`offset`、`limit`、`returnedCount`、`hasMore`、`nextOffset` を含みます。complete list を読むには、`hasMore` が `true` の間 `nextOffset` で次ページを request します。package tools は `count` field を返しません。

## Lifecycle and client behavior

GUI は local config の load 後に embedded `alcomd3-mcp` Streamable HTTP server を
bind します。MCP `2026-07-28` request は sessionless で、すべての request に標準 protocol
metadata が必要です。通常の `2025-11-25` client は legacy session を使います。両経路は
GUI state、limiter、task manager、resource locks、activity recorder を共有します。

ALCOMD3 lifecycle boundaries:

- GUI 終了時または MCP extension disabled 時、HTTP listener を停止し、server task の終了を待って unfinished operation を cancel します。
- endpoint URL と bearer token は local installation で stable です。GUI restart 後も client config を変更せず reconnect できます。
- GUI client area は client name/version ごとに recent activity をまとめ、live session list ではありません。tool highlight は現在処理中の call を示します。
- GUI unavailable 中は separate MCP process がないため local endpoint も利用できません。
- GUI available だが MCP disabled の場合、新しい tool call は structured `mcp_disabled` を返します。既存 long task は認証済み `tasks/get` で query、`tasks/cancel` で cancel できます。
- GUI restart 後、configured loopback port に再度 bind し、client request は reconnect できます。data を返すかどうかは GUI で MCP が enabled かに依存します。
- configured port が使用中の場合、MCP page は server not running を表示し、technical log に startup error を記録します。

## Errors and troubleshooting

### `mcp_disabled`

MCP page が disabled です。endpoint が running と表示されることがありますが正常です。MCP を有効化して tool を再実行してください。既に開始された project long tasks は例外で、client は `tasks/get` と `tasks/cancel` で query/cancel できます。

### `rate_limited`

embedded server が短時間に多すぎる tool calls を受信したか、64 tool calls が既に実行中です。1 分間に開始できる tool call は 600 件です。少し待って retry してください。

### The MCP endpoint is unavailable

よくある原因:

- ALCOMD3 GUI が起動していない。
- client URL が GUI に表示される現在の MCP Endpoint と一致しない。
- configured local port が既に使用中。

対応:

1. ALCOMD3 を起動します。
2. MCP page で endpoint running を確認します。
3. MCP Endpoint と Authorization Token を再コピーし、MCP client configuration を更新します。
4. MCP client を再起動します。

Windows で対応 client を使う場合は、そのクイック設定 button を再度クリックし、
必要に応じて置換を確認してから、その client を完全に終了して再起動します。

### HTTP `401 Unauthorized`

bearer token がないか、ALCOMD3 に表示されている token と一致していません。client の
`Authorization` header を更新してください。

### HTTP `403 Forbidden`

request に許可されていない browser `Origin` が含まれています。ALCOMD3 は native MCP
client と同一 loopback origin だけを受け入れ、DNS rebinding や cross-site request による
local server へのアクセスを防ぎます。

### Protocol negotiation errors

MCP `2026-07-28` では request ごとに標準 `MCP-Protocol-Version`、`Mcp-Method`、`_meta`
を送信するか、通常 tool call の前に `2025-11-25` legacy session を initialize してください。
他の protocol version は advertise しません。

## Development smoke test

repository root で embedded MCP service を含む GUI を build します。

```powershell
cargo build -p vrc-get-gui
```

HTTP lifecycle と security smoke tests を実行します。

```powershell
cargo test -p vrc-get-gui mcp::
```

Expected result:

- `initialize` succeeds.
- standard headers/request metadata がある `2026-07-28` の `server/discover` と sessionless request が succeeds.
- `2025-11-25` legacy session が initialize し、通常 tool call を実行できます。
- `tools/list` returns current MCP tools.
- `tools/call` returns readable `ok: false` error and marks the MCP tool result with `isError: true`.
- missing/incorrect bearer token は HTTP `401` を返します。
- disallowed Origin は HTTP `403` を返します。

## Related source

- Embedded HTTP/RMCP service and tools: `vrc-get-gui/src/mcp/server.rs`
- MCP lifecycle, direct dispatch, operations, shared state: `vrc-get-gui/src/mcp/mod.rs`
- Internal MCP data types: `vrc-get-gui/src/mcp/types.rs`
- GUI shared backend services and MCP capability matrix: `vrc-get-gui/src/backend/`
- GUI Tauri commands: `vrc-get-gui/src/commands/mcp.rs`
- GUI MCP page: `vrc-get-gui/app/_main/mcp/index.tsx`
- Packaging logic: `xtask/src/build_alcom.rs`, `xtask/src/bundle_alcom*`

## References

- RMCP 3.1.2: <https://github.com/modelcontextprotocol/rust-sdk/releases/tag/rmcp-v3.1.2>
- MCP Specification `2026-07-28`: <https://modelcontextprotocol.io/specification/2026-07-28>
- MCP Specification `2025-11-25`: <https://modelcontextprotocol.io/specification/2025-11-25>
- Experimental Tasks extension: <https://github.com/modelcontextprotocol/ext-tasks>
