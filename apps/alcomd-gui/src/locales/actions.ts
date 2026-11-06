import type { TranslationTable } from "../i18n-types";

// Existing terms follow the frozen v3 resources; v4-only text is authored for this GUI.
export const actionMessages: TranslationTable = {
    "Network": { "zh-CN": "网络", "ja-JP": "ネットワーク" },
    "Global proxy": { "zh-CN": "全局代理", "ja-JP": "グローバルプロキシ" },
    "Follow system proxy": { "zh-CN": "跟随系统代理", "ja-JP": "システムプロキシに従う" },
    "Do not use a proxy": { "zh-CN": "不使用代理", "ja-JP": "プロキシを使用しない" },
    "Custom proxy": { "zh-CN": "自定义代理", "ja-JP": "カスタムプロキシ" },
    "Proxy address": { "zh-CN": "代理地址", "ja-JP": "プロキシアドレス" },
    "Enter an HTTP or HTTPS proxy address, such as http://127.0.0.1:7890.": { "zh-CN": "请输入 HTTP 或 HTTPS 代理地址，例如 http://127.0.0.1:7890。", "ja-JP": "http://127.0.0.1:7890 のような HTTP または HTTPS プロキシアドレスを入力してください。" },
    "VCC links": { "zh-CN": "VCC 链接", "ja-JP": "VCC リンク" },
    "Handle VCC links": { "zh-CN": "处理 VCC 链接", "ja-JP": "VCC リンクを処理する" },
    "Register ALCOMD for VCC links": { "zh-CN": "注册 ALCOMD 处理 VCC 链接", "ja-JP": "VCC リンクの処理に ALCOMD を登録" },
    "ALCOMD was registered for VCC links.": { "zh-CN": "已注册 ALCOMD 处理 VCC 链接。", "ja-JP": "VCC リンクの処理に ALCOMD を登録しました。" },
    "Could not register ALCOMD for VCC links.": { "zh-CN": "无法注册 ALCOMD 处理 VCC 链接。", "ja-JP": "VCC リンクの処理に ALCOMD を登録できませんでした。" },
    "Authentication": { "zh-CN": "认证", "ja-JP": "認証" },
    "Repository authentication": { "zh-CN": "存储库认证", "ja-JP": "リポジトリの認証" },
    "Uses authentication": { "zh-CN": "使用认证", "ja-JP": "認証を使用" },
    "Header name": { "zh-CN": "请求头名称", "ja-JP": "ヘッダー名" },
    "Request header names must be unique, regardless of case.": { "zh-CN": "请求头名称不区分大小写，不能重复。", "ja-JP": "大文字と小文字を区別せず、同じヘッダー名は重複できません。" },
    "Header value": { "zh-CN": "请求头值", "ja-JP": "ヘッダー値" },
    "Add request header": { "zh-CN": "添加请求头", "ja-JP": "リクエストヘッダーを追加" },
    "Remove request header": { "zh-CN": "移除请求头", "ja-JP": "リクエストヘッダーを削除" },
    "Clear authentication": { "zh-CN": "清除认证", "ja-JP": "認証を解除" },
    "No template": { "zh-CN": "不使用模板", "ja-JP": "テンプレートを使用しない" },
    "Edit template": { "zh-CN": "编辑模板", "ja-JP": "テンプレートを編集" },
    "Save template": { "zh-CN": "保存模板", "ja-JP": "テンプレートを保存" },
    "Create template from project": { "zh-CN": "从项目创建模板", "ja-JP": "プロジェクトからテンプレートを作成" },
    "Import template": { "zh-CN": "导入模板", "ja-JP": "テンプレートをインポート" },
    "Export template": { "zh-CN": "导出模板", "ja-JP": "テンプレートをエクスポート" },
    "Replace template": { "zh-CN": "替换模板", "ja-JP": "テンプレートを置き換え" },
    "More template creation options": { "zh-CN": "更多模板创建选项", "ja-JP": "テンプレート作成のその他の操作" },
    "Template source": { "zh-CN": "模板来源", "ja-JP": "テンプレートのソース" },
    "Template name": { "zh-CN": "模板名称", "ja-JP": "テンプレート名" },
    "User template": { "zh-CN": "用户模板", "ja-JP": "ユーザーテンプレート" },
    "Template file": { "zh-CN": "模板文件", "ja-JP": "テンプレートファイル" },
    "Choose file": { "zh-CN": "选择文件", "ja-JP": "ファイルを選択" },
    "File name": { "zh-CN": "文件名", "ja-JP": "ファイル名" },
    "Destination file": { "zh-CN": "目标文件", "ja-JP": "保存先ファイル" },
    "Include project assets": { "zh-CN": "包含项目资产", "ja-JP": "プロジェクトのアセットを含める" },
    "Lock package versions": { "zh-CN": "锁定软件包版本", "ja-JP": "パッケージのバージョンを固定" },
    "Replace the contents of {name}?": { "zh-CN": "替换 {name} 的内容？", "ja-JP": "{name} の内容を置き換えますか？" },
    "For example, com.example.my-template": { "zh-CN": "例如 com.example.my-template", "ja-JP": "例：com.example.my-template" },
    "Package ID": { "zh-CN": "软件包 ID", "ja-JP": "パッケージ ID" },
    "Version range": { "zh-CN": "版本范围", "ja-JP": "バージョン範囲" },
    "Add package": { "zh-CN": "添加软件包", "ja-JP": "パッケージを追加" },
    "Remove package": { "zh-CN": "移除软件包", "ja-JP": "パッケージを削除" },
    "The selected template changed. Select it again.": { "zh-CN": "所选模板已变化，请重新选择。", "ja-JP": "選択したテンプレートが変更されました。選び直してください。" },
    "Retry with the same details to recover the original creation request.": { "zh-CN": "使用相同信息重试，可找回原创建请求。", "ja-JP": "元の作成リクエストを取得するには、同じ内容で再試行してください。" },
    "Publisher confirmation required": { "zh-CN": "请确认扩展发布者", "ja-JP": "拡張機能の発行元を確認してください" },
    "Error code": { "zh-CN": "错误标识", "ja-JP": "エラーコード" },
    "Requested permissions": { "zh-CN": "申请的权限", "ja-JP": "要求する権限" },
    "Required permissions": { "zh-CN": "必需权限", "ja-JP": "必須の権限" },
    "Optional permissions": { "zh-CN": "可选权限", "ja-JP": "任意の権限" },
    "Allowed permissions": { "zh-CN": "已允许的权限", "ja-JP": "許可済みの権限" },
    "No permissions requested": { "zh-CN": "未申请权限", "ja-JP": "要求する権限はありません" },
    "No permissions granted": { "zh-CN": "尚未授予权限", "ja-JP": "許可済みの権限はありません" },
    "Run in background": { "zh-CN": "后台运行", "ja-JP": "バックグラウンドで実行" },
    "Read projects": { "zh-CN": "读取项目", "ja-JP": "プロジェクトの読み取り" },
    "Unknown permission: {permission}": { "zh-CN": "未知权限：{permission}", "ja-JP": "不明な権限：{permission}" },
    "This extension": { "zh-CN": "此扩展", "ja-JP": "この拡張機能" },
    "Select project": { "zh-CN": "选择项目", "ja-JP": "プロジェクトを選択" },
    "Project {id}": { "zh-CN": "项目 {id}", "ja-JP": "プロジェクト {id}" },
    "Permission details are unavailable from this Core.": { "zh-CN": "当前 Core 无法提供权限详情。", "ja-JP": "現在のCoreから権限の詳細を取得できません。" },
    "The permission change may have completed. Refresh permissions before trying again.": { "zh-CN": "权限变更可能已完成。请先刷新权限，再决定是否重试。", "ja-JP": "権限の変更は完了している可能性があります。再試行の前に権限を更新してください。" },
    "Recover extension": { "zh-CN": "恢复扩展", "ja-JP": "拡張機能を復旧" },
    "This extension is quarantined. Recovery rechecks its package and permissions before enabling it.": { "zh-CN": "此扩展已被隔离。恢复时会重新验证扩展包与权限，然后启用。", "ja-JP": "この拡張機能は隔離されています。復旧時にパッケージと権限を再検証してから有効にします。" },
    "Operation failed": { "zh-CN": "任务失败", "ja-JP": "操作に失敗しました" },
    "Open project": { "zh-CN": "打开项目", "ja-JP": "プロジェクトを開く" },
    "New list file name": { "zh-CN": "新列表文件名", "ja-JP": "新しい一覧ファイル名" },
    "Enter a file name without path separators or reserved characters.": { "zh-CN": "请输入文件名，不含路径分隔符或保留字符。", "ja-JP": "パス区切りや予約文字を含まないファイル名を入力してください。" },
    "Choose the destination folder. A new file with this name will be created there.": { "zh-CN": "请选择保存文件夹，应用会在其中创建上述新文件。", "ja-JP": "保存先フォルダーを選んでください。その中に指定した名前で新規ファイルを作成します。" },
    "Adding repositories…": { "zh-CN": "正在添加存储库…", "ja-JP": "リポジトリを追加中…" },
    "Repository list actions": { "zh-CN": "存储库列表操作", "ja-JP": "リポジトリ一覧の操作" },
    "Import list": { "zh-CN": "导入列表", "ja-JP": "一覧をインポート" },
    "Export list": { "zh-CN": "导出列表", "ja-JP": "一覧をエクスポート" },
    "Append repositories from a UTF-8 list. One HTTP(S) URL per line; # starts a comment. Existing repositories are kept.": { "zh-CN": "从 UTF-8 列表追加存储库。每行一个 HTTP(S) 网址，# 后为注释。保留现有存储库。", "ja-JP": "UTF-8の一覧から追加します。1行に1つのHTTP(S) URLを記載し、#以降はコメントです。既存の登録は保持します。" },
    "{count} entries · {ignored} blank or comment lines": { "zh-CN": "{count} 条记录 · {ignored} 行空白或注释", "ja-JP": "{count}件 · 空行・コメント{ignored}行" },
    "Added: {added} · Failed: {failed} · Unknown: {unknown} · Not processed: {pending}": { "zh-CN": "已添加：{added} · 失败：{failed} · 未知：{unknown} · 未处理：{pending}", "ja-JP": "追加済み：{added} · 失敗：{failed} · 不明：{unknown} · 未処理：{pending}" },
    "Completed additions are kept. Check the repository list before importing again.": { "zh-CN": "已添加的存储库会保留。再次导入前请核对存储库列表。", "ja-JP": "追加済みのリポジトリは保持されます。再インポートの前にリポジトリ一覧を確認してください。" },
    "Line {line}": { "zh-CN": "第 {line} 行", "ja-JP": "{line}行目" },
    "To import": { "zh-CN": "待导入", "ja-JP": "インポート対象" },
    "Already added": { "zh-CN": "已存在", "ja-JP": "追加済み" },
    "Duplicate": { "zh-CN": "重复", "ja-JP": "重複" },
    "Invalid": { "zh-CN": "无效", "ja-JP": "無効" },
    "Unsupported": { "zh-CN": "不支持", "ja-JP": "未対応" },
    "Not processed": { "zh-CN": "未处理", "ja-JP": "未処理" },
    "Failed": { "zh-CN": "失败", "ja-JP": "失敗" },
    "Result unknown": { "zh-CN": "结果未知", "ja-JP": "結果不明" },
    "Repository list progress": { "zh-CN": "存储库列表进度", "ja-JP": "リポジトリ一覧の進行状況" },
    "{count} requests completed": { "zh-CN": "已完成 {count} 个请求", "ja-JP": "{count}件のリクエストが完了" },
    "Stop after current request": { "zh-CN": "当前请求后停止", "ja-JP": "現在のリクエスト後に停止" },
    "Stop import": { "zh-CN": "停止导入", "ja-JP": "インポートを停止" },
    "Stopping import…": { "zh-CN": "正在停止导入…", "ja-JP": "インポートを停止中…" },
    "Choose list file": { "zh-CN": "选择列表文件", "ja-JP": "一覧ファイルを選択" },
    "Add {count} repositories": { "zh-CN": "添加 {count} 个存储库", "ja-JP": "{count}件のリポジトリを追加" },
    "The file already exists and was preserved. Choose another file name.": { "zh-CN": "文件已存在并已保留，请选择其他文件名。", "ja-JP": "既存のファイルは保持されました。別のファイル名を選んでください。" },
    "The repository snapshot changed. Preview again before saving.": { "zh-CN": "存储库快照已变化，请重新预览后保存。", "ja-JP": "リポジトリのスナップショットが変わりました。再度プレビューしてから保存してください。" },
    "{count} repositories can be exported · {excluded} excluded · {bytes} bytes": { "zh-CN": "可导出 {count} 个存储库 · 排除 {excluded} 个 · {bytes} 字节", "ja-JP": "エクスポート可能：{count}件 · 除外：{excluded}件 · {bytes}バイト" },
    "This file can contain repository credentials in plain text, including authentication headers. Keep it private.": { "zh-CN": "文件可能包含明文存储库凭据，包括认证请求头。请妥善保管。", "ja-JP": "ファイルには認証ヘッダーなどのリポジトリ認証情報が平文で含まれる場合があります。取り扱いにご注意ください。" },
    "Only a new file can be created. Existing files are preserved; choose a different name if the destination exists.": { "zh-CN": "仅创建新文件。保留现有文件；目标已存在时请另选名称。", "ja-JP": "新規ファイルのみ作成できます。既存ファイルは保持するため、同名ファイルがある場合は別名を選んでください。" },
    "I have reviewed the export scope and understand the file can contain credentials.": { "zh-CN": "我已核对导出范围，并了解文件可能包含凭据。", "ja-JP": "エクスポート範囲を確認し、ファイルに認証情報が含まれる可能性があることを理解しました。" },
    "Saved {count} repositories ({bytes} bytes).": { "zh-CN": "已保存 {count} 个存储库（{bytes} 字节）。", "ja-JP": "{count}件のリポジトリ（{bytes}バイト）を保存しました。" },
    "Preview again": { "zh-CN": "重新预览", "ja-JP": "再プレビュー" },
    "Save new list file": { "zh-CN": "保存新列表文件", "ja-JP": "新しい一覧ファイルを保存" },
    "Local source cannot be represented by this URL list.": { "zh-CN": "该网址列表无法表达本地来源。", "ja-JP": "このURL一覧ではローカルソースを表現できません。" },
    "Authentication settings cannot be represented by this URL list.": { "zh-CN": "该网址列表无法表达认证设置。", "ja-JP": "このURL一覧では認証設定を表現できません。" },
    "This source cannot be represented by this URL list.": { "zh-CN": "该网址列表无法表达此来源。", "ja-JP": "このURL一覧ではこのソースを表現できません。" },
    "Check the repository URL or manifest path.": {
        "zh-CN": "请检查存储库网址或清单路径。",
        "ja-JP": "リポジトリのURLまたはマニフェストのパスを確認してください。"
    },
    "Repository refresh failed": {
        "zh-CN": "存储库刷新失败",
        "ja-JP": "リポジトリの更新に失敗しました"
    },
    "The last available packages are still shown. Check the source and retry.": {
        "zh-CN": "仍保留上次可用的软件包。请检查来源后重试。",
        "ja-JP": "前回取得したパッケージを引き続き表示しています。取得元を確認して再試行してください。"
    },
    "Refresh list": {
        "zh-CN": "刷新列表",
        "ja-JP": "一覧を更新"
    },
    "Favorite could not be updated": {
        "zh-CN": "无法更新收藏",
        "ja-JP": "お気に入りを更新できませんでした"
    },
    "User Package enrolled.": {
        "zh-CN": "已登记用户软件包。",
        "ja-JP": "ユーザーパッケージを登録しました。"
    },
    "User Package enrollment failed": {
        "zh-CN": "用户软件包登记失败",
        "ja-JP": "ユーザーパッケージの登録に失敗しました"
    },
    "User Package refreshed.": {
        "zh-CN": "已刷新用户软件包。",
        "ja-JP": "ユーザーパッケージを更新しました。"
    },
    "Enrollment removed. The source folder was not deleted.": {
        "zh-CN": "已取消登记，源目录未删除。",
        "ja-JP": "登録を解除しました。元のフォルダーは削除されていません。"
    },
    "User Package refresh failed": {
        "zh-CN": "用户软件包刷新失败",
        "ja-JP": "ユーザーパッケージの更新に失敗しました"
    },
    "Hide details": {
        "zh-CN": "收起详情",
        "ja-JP": "詳細を隠す"
    },
    "The request could not be completed. Review the details before trying again.": {
        "zh-CN": "未能完成请求。请查看详情后重试。",
        "ja-JP": "リクエストを完了できませんでした。詳細を確認して再試行してください。"
    },
    " · Diagnostic ID {id}": {
        "zh-CN": " · 诊断 ID {id}",
        "ja-JP": " · 診断ID {id}"
    },
    "{title} progress": {
        "zh-CN": "{title}进度",
        "ja-JP": "{title}の進捗"
    },
    "{version} → Removed": {
        "zh-CN": "{version} → 已删除",
        "ja-JP": "{version} → 削除"
    },
    "Action:": {
        "zh-CN": "操作：",
        "ja-JP": "操作："
    },
    "Additional arguments": {
        "zh-CN": "附加参数",
        "ja-JP": "追加の引数"
    },
    "ALCOMD could not complete the package changes. Try again.": {
        "zh-CN": "ALCOMD 无法完成软件包更改，请重试。",
        "ja-JP": "ALCOMDはパッケージの変更を完了できませんでした。再試行してください。"
    },
    "ALCOMD will inspect the selected root and add it to the per-user registry.": {
        "zh-CN": "ALCOMD 将检查所选路径，并将项目添加到当前用户的项目列表。",
        "ja-JP": "ALCOMDは選択したパスを検証し、現在のユーザーのプロジェクト一覧に追加します。"
    },
    "Apply changes": {
        "zh-CN": "应用更改",
        "ja-JP": "変更を適用"
    },
    "Apply package changes?": {
        "zh-CN": "应用软件包更改？",
        "ja-JP": "パッケージの変更を適用しますか？"
    },
    "Apply reviewed plan": {
        "zh-CN": "应用已确认的计划",
        "ja-JP": "確認済みのプランを適用"
    },
    "approve for extension": {
        "zh-CN": "为此扩展批准",
        "ja-JP": "この拡張機能で承認"
    },
    "Approve this publisher for this extension": {
        "zh-CN": "为此扩展批准该发布者",
        "ja-JP": "この拡張機能でこの発行元を承認する"
    },
    "Back up {name}.": {
        "zh-CN": "备份 {name}。",
        "ja-JP": "{name}をバックアップします。"
    },
    "Browse…": {
        "zh-CN": "浏览…",
        "ja-JP": "参照…"
    },
    "Cancel operation": {
        "zh-CN": "取消任务",
        "ja-JP": "タスクをキャンセル"
    },
    "Cancellation is cooperative. The operation remains authoritative until the daemon reports a terminal state.": {
        "zh-CN": "取消请求将由任务协作处理。在后台服务报告终止状态之前，任务仍可能继续运行。",
        "ja-JP": "キャンセルはタスクの処理状況に従います。バックグラウンドサービスが終了状態を報告するまで、タスクは実行を継続する場合があります。"
    },
    "Cancellation requested.": {
        "zh-CN": "已请求取消。",
        "ja-JP": "キャンセルを要求しました。"
    },
    "cancelled": {
        "zh-CN": "已取消",
        "ja-JP": "キャンセル済み"
    },
    "cancelling": {
        "zh-CN": "正在取消",
        "ja-JP": "キャンセル中"
    },
    "Change version": {
        "zh-CN": "更改版本",
        "ja-JP": "バージョンを変更"
    },
    "china variant change": {
        "zh-CN": "中国版与国际版切换",
        "ja-JP": "中国版と国際版の変更"
    },
    "Choose a project": {
        "zh-CN": "选择项目",
        "ja-JP": "プロジェクトを選択"
    },
    "Choose an installation": {
        "zh-CN": "选择安装",
        "ja-JP": "インストールを選択"
    },
    "Choose the Unity installation for this launch.": {
        "zh-CN": "选择本次启动使用的 Unity。",
        "ja-JP": "今回の起動に使用するUnityを選択してください。"
    },
    "Choose the Unity installation to use for this launch.": {
        "zh-CN": "选择本次启动使用的 Unity。",
        "ja-JP": "今回の起動に使用するUnityを選択してください。"
    },
    "Choose Unity {version} installation": {
        "zh-CN": "选择 Unity {version} 安装",
        "ja-JP": "Unity {version}のインストールを選択"
    },
    "Choose Unity for this launch": {
        "zh-CN": "选择本次启动使用的 Unity",
        "ja-JP": "今回の起動に使用するUnityを選択"
    },
    "Classification": {
        "zh-CN": "分类",
        "ja-JP": "分類"
    },
    "Clear launch arguments": {
        "zh-CN": "清除启动参数",
        "ja-JP": "起動引数を消去"
    },
    "Closing this dialog does not cancel the backup task.": {
        "zh-CN": "关闭此对话框不会取消备份任务。",
        "ja-JP": "このダイアログを閉じてもバックアップはキャンセルされません。"
    },
    "Closing this dialog does not cancel the restore task.": {
        "zh-CN": "关闭此对话框不会取消恢复任务。",
        "ja-JP": "このダイアログを閉じても復元はキャンセルされません。"
    },
    "Create install plan": {
        "zh-CN": "创建安装计划",
        "ja-JP": "インストールプランを作成"
    },
    "create project": {
        "zh-CN": "新建项目",
        "ja-JP": "プロジェクトを作成"
    },
    "Create template": {
        "zh-CN": "创建模板",
        "ja-JP": "テンプレートを作成"
    },
    "Create uninstall plan": {
        "zh-CN": "创建卸载计划",
        "ja-JP": "アンインストールプランを作成"
    },
    "Current · {version}": {
        "zh-CN": "当前 · {version}",
        "ja-JP": "現在 · {version}"
    },
    "Data": {
        "zh-CN": "数据",
        "ja-JP": "データ"
    },
    "delete data": {
        "zh-CN": "删除数据",
        "ja-JP": "データを削除"
    },
    "Delete extension-owned data during uninstall": {
        "zh-CN": "卸载时删除扩展自有数据",
        "ja-JP": "アンインストール時に拡張機能のデータを削除する"
    },
    "derive": {
        "zh-CN": "派生",
        "ja-JP": "作成"
    },
    "Description": {
        "zh-CN": "描述",
        "ja-JP": "説明"
    },
    "Disable": {
        "zh-CN": "停用",
        "ja-JP": "無効化"
    },
    "Discard plan": {
        "zh-CN": "放弃计划",
        "ja-JP": "プランを破棄"
    },
    "Discover and refresh": {
        "zh-CN": "查找并刷新",
        "ja-JP": "検出して更新"
    },
    "Display name": {
        "zh-CN": "显示名称",
        "ja-JP": "表示名"
    },
    "downgrade": {
        "zh-CN": "降级",
        "ja-JP": "ダウングレード"
    },
    "Enable": {
        "zh-CN": "启用",
        "ja-JP": "有効化"
    },
    "Exclude VPM packages": {
        "zh-CN": "排除 VPM 软件包",
        "ja-JP": "VPMパッケージを除外"
    },
    "Export": {
        "zh-CN": "导出",
        "ja-JP": "エクスポート"
    },
    "Export target": {
        "zh-CN": "导出路径",
        "ja-JP": "エクスポート先"
    },
    "Extension": {
        "zh-CN": "扩展",
        "ja-JP": "拡張機能"
    },
    "Extension disabled.": {
        "zh-CN": "已停用扩展。",
        "ja-JP": "拡張機能を無効にしました。"
    },
    "Extension enabled.": {
        "zh-CN": "已启用扩展。",
        "ja-JP": "拡張機能を有効にしました。"
    },
    "Extension install accepted.": {
        "zh-CN": "已接受扩展安装请求。",
        "ja-JP": "拡張機能のインストールを受け付けました。"
    },
    "Extension install input": {
        "zh-CN": "扩展安装信息",
        "ja-JP": "拡張機能のインストール情報"
    },
    "Extension management": {
        "zh-CN": "扩展管理",
        "ja-JP": "拡張機能の管理"
    },
    "Extension package": {
        "zh-CN": "扩展包",
        "ja-JP": "拡張機能パッケージ"
    },
    "Extension permissions": {
        "zh-CN": "扩展权限",
        "ja-JP": "拡張機能の権限"
    },
    "Extension uninstall accepted.": {
        "zh-CN": "已接受扩展卸载请求。",
        "ja-JP": "拡張機能のアンインストールを受け付けました。"
    },
    "failed": {
        "zh-CN": "失败",
        "ja-JP": "失敗"
    },
    "Fast": {
        "zh-CN": "快速",
        "ja-JP": "高速"
    },
    "Favorite {name}": {
        "zh-CN": "收藏 {name}",
        "ja-JP": "{name}をお気に入りに追加"
    },
    "Favorite updated.": {
        "zh-CN": "已更新收藏状态。",
        "ja-JP": "お気に入りを更新しました。"
    },
    "From": {
        "zh-CN": "从",
        "ja-JP": "変更前"
    },
    "Go back": {
        "zh-CN": "返回",
        "ja-JP": "戻る"
    },
    "Grant": {
        "zh-CN": "授予",
        "ja-JP": "付与"
    },
    "import": {
        "zh-CN": "导入",
        "ja-JP": "インポート"
    },
    "Installation to remove": {
        "zh-CN": "要删除的安装记录",
        "ja-JP": "削除する登録"
    },
    "interrupted": {
        "zh-CN": "已中断",
        "ja-JP": "中断"
    },
    "Load more projects": {
        "zh-CN": "加载更多项目",
        "ja-JP": "プロジェクトをさらに読み込む"
    },
    "Loading launch arguments…": {
        "zh-CN": "正在加载启动参数…",
        "ja-JP": "起動引数を読み込み中…"
    },
    "Loading projects…": {
        "zh-CN": "正在加载项目…",
        "ja-JP": "プロジェクトを読み込み中…"
    },
    "Local manifest": {
        "zh-CN": "本地清单",
        "ja-JP": "ローカルマニフェスト"
    },
    "Local manifest path": {
        "zh-CN": "本地清单路径",
        "ja-JP": "ローカルマニフェストのパス"
    },
    "major downgrade": {
        "zh-CN": "主版本降级",
        "ja-JP": "メジャーバージョンのダウングレード"
    },
    "major upgrade": {
        "zh-CN": "主版本升级",
        "ja-JP": "メジャーバージョンの更新"
    },
    "Manage permissions": {
        "zh-CN": "管理权限",
        "ja-JP": "権限を管理"
    },
    "Maximum": {
        "zh-CN": "最大压缩",
        "ja-JP": "最大圧縮"
    },
    "Migrate Project Unity version": {
        "zh-CN": "迁移项目 Unity 版本",
        "ja-JP": "プロジェクトのUnityバージョンを移行"
    },
    "Migrate Project…": {
        "zh-CN": "迁移项目…",
        "ja-JP": "プロジェクトを移行…"
    },
    "Migration completed, but no exact Unity {version} installation is available.": {
        "zh-CN": "迁移已完成，但未找到版本完全匹配的 Unity {version} 安装。",
        "ja-JP": "移行は完了しましたが、完全に一致するUnity {version}が見つかりません。"
    },
    "Migration completed. Choose the Unity installation for this launch.": {
        "zh-CN": "迁移已完成，请选择本次启动使用的 Unity。",
        "ja-JP": "移行が完了しました。今回の起動に使用するUnityを選択してください。"
    },
    "Migration completed. Unity launch {state}.": {
        "zh-CN": "迁移已完成。Unity 启动状态：{state}。",
        "ja-JP": "移行が完了しました。Unityの起動状態：{state}。"
    },
    "More actions for {name}": {
        "zh-CN": "{name} 的更多操作",
        "ja-JP": "{name}のその他の操作"
    },
    "More than one source provides this package version. Choose a source and try again.": {
        "zh-CN": "多个来源提供此软件包版本。请选择来源后重试。",
        "ja-JP": "このパッケージのバージョンは複数のソースにあります。ソースを選択して再試行してください。"
    },
    "New directory": {
        "zh-CN": "新目录",
        "ja-JP": "新しいフォルダー"
    },
    "New directory name": {
        "zh-CN": "新目录名称",
        "ja-JP": "新しいフォルダー名"
    },
    "no change": {
        "zh-CN": "无更改",
        "ja-JP": "変更なし"
    },
    "No package changes are required for this project.": {
        "zh-CN": "此项目无需更改软件包。",
        "ja-JP": "このプロジェクトのパッケージに変更は必要ありません。"
    },
    "The following legacy files and folders will be removed.": {
        "zh-CN": "应用后将移除以下旧文件和文件夹。",
        "ja-JP": "適用すると、次の旧ファイルとフォルダーが削除されます。"
    },
    "No package resolve is required.": {
        "zh-CN": "无需解析软件包依赖。",
        "ja-JP": "パッケージの依存関係の解決は不要です。"
    },
    "No registered projects available.": {
        "zh-CN": "没有已添加的项目。",
        "ja-JP": "登録済みのプロジェクトはありません。"
    },
    "Not included in this update:": {
        "zh-CN": "本次更新不包含：",
        "ja-JP": "今回の更新に含まれない項目："
    },
    "official": {
        "zh-CN": "官方",
        "ja-JP": "Official"
    },
    "One argument per line. The daemon validates forbidden arguments.": {
        "zh-CN": "每行填写一个参数。后台服务将检查禁止使用的参数。",
        "ja-JP": "引数は1行に1つ入力してください。使用できない引数はバックグラウンドサービスが検証します。"
    },
    "Only the ALCOMD registry entry is removed. The editor remains installed.": {
        "zh-CN": "仅删除 ALCOMD 中的安装记录，不会卸载编辑器。",
        "ja-JP": "ALCOMDの登録情報のみを削除します。エディターはアンインストールされません。"
    },
    "open": {
        "zh-CN": "已打开",
        "ja-JP": "起動済み"
    },
    "opening": {
        "zh-CN": "正在打开",
        "ja-JP": "起動中"
    },
    "Operation control": {
        "zh-CN": "任务控制",
        "ja-JP": "タスクの操作"
    },
    "Package changes started.": {
        "zh-CN": "已开始应用软件包更改。",
        "ja-JP": "パッケージの変更を開始しました。"
    },
    "Package changes were not applied": {
        "zh-CN": "软件包更改未应用",
        "ja-JP": "パッケージの変更は適用されませんでした"
    },
    "Packages already installed in projects stay installed.": {
        "zh-CN": "已安装到项目中的软件包将保留。",
        "ja-JP": "プロジェクトにインストール済みのパッケージは保持されます。"
    },
    "Packages are up to date": {
        "zh-CN": "软件包已是最新版本",
        "ja-JP": "パッケージは最新です"
    },
    "patch or minor downgrade": {
        "zh-CN": "补丁或次版本降级",
        "ja-JP": "パッチまたはマイナーバージョンのダウングレード"
    },
    "patch or minor upgrade": {
        "zh-CN": "补丁或次版本升级",
        "ja-JP": "パッチまたはマイナーバージョンの更新"
    },
    "Permission": {
        "zh-CN": "权限",
        "ja-JP": "権限"
    },
    "Permission granted.": {
        "zh-CN": "已授予权限。",
        "ja-JP": "権限を付与しました。"
    },
    "Permission revoked.": {
        "zh-CN": "已撤销权限。",
        "ja-JP": "権限を取り消しました。"
    },
    "Plan fingerprint:": {
        "zh-CN": "计划指纹：",
        "ja-JP": "プランのフィンガープリント："
    },
    "Progress is temporarily unavailable. ALCOMD will keep following the same task.": {
        "zh-CN": "暂时无法读取进度。ALCOMD 将继续跟踪此任务。",
        "ja-JP": "現在、進捗を取得できません。ALCOMDは引き続き同じタスクを追跡します。"
    },
    "Project already uses Unity {version}.": {
        "zh-CN": "此项目已使用 Unity {version}。",
        "ja-JP": "このプロジェクトはすでにUnity {version}を使用しています。"
    },
    "Project folder name": {
        "zh-CN": "项目文件夹名称",
        "ja-JP": "プロジェクトフォルダー名"
    },
    "Project refreshed.": {
        "zh-CN": "已刷新项目。",
        "ja-JP": "プロジェクトを更新しました。"
    },
    "Project registered at revision {revision}.": {
        "zh-CN": "项目已添加，修订号为 {revision}。",
        "ja-JP": "プロジェクトを登録しました（リビジョン {revision}）。"
    },
    "Project root": {
        "zh-CN": "项目路径",
        "ja-JP": "プロジェクトのパス"
    },
    "Project Unity version: {version}": {
        "zh-CN": "项目 Unity 版本：{version}",
        "ja-JP": "プロジェクトのUnityバージョン：{version}"
    },
    "queued": {
        "zh-CN": "排队中",
        "ja-JP": "待機中"
    },
    "Reading progress…": {
        "zh-CN": "正在读取进度…",
        "ja-JP": "進捗を読み込み中…"
    },
    "recovering": {
        "zh-CN": "恢复中",
        "ja-JP": "復旧中"
    },
    "Refresh repository": {
        "zh-CN": "刷新存储库",
        "ja-JP": "リポジトリを更新"
    },
    "Refreshing repository": {
        "zh-CN": "正在刷新存储库",
        "ja-JP": "リポジトリを更新中"
    },
    "Register": {
        "zh-CN": "添加",
        "ja-JP": "登録"
    },
    "Register project": {
        "zh-CN": "添加已有项目",
        "ja-JP": "既存のプロジェクトを追加"
    },
    "Remote URL": {
        "zh-CN": "远程 URL",
        "ja-JP": "リモートURL"
    },
    "Remove {name} from favorites": {
        "zh-CN": "取消收藏 {name}",
        "ja-JP": "{name}をお気に入りから削除"
    },
    "Remove {name} from the list?": {
        "zh-CN": "从列表删除 {name}？",
        "ja-JP": "{name}を一覧から削除しますか？"
    },
    "Remove {name} from your sources?": {
        "zh-CN": "从来源中删除 {name}？",
        "ja-JP": "ソースから{name}を削除しますか？"
    },
    "Remove {name} from your templates?": {
        "zh-CN": "从模板中删除 {name}？",
        "ja-JP": "テンプレートから{name}を削除しますか？"
    },
    "Remove favorite": {
        "zh-CN": "取消收藏",
        "ja-JP": "お気に入りから削除"
    },
    "Remove installation": {
        "zh-CN": "删除安装记录",
        "ja-JP": "登録を削除"
    },
    "Remove repository": {
        "zh-CN": "删除存储库",
        "ja-JP": "リポジトリを削除"
    },
    "Remove template": {
        "zh-CN": "删除模板",
        "ja-JP": "テンプレートを削除"
    },
    "Remove this repository?": {
        "zh-CN": "删除此存储库？",
        "ja-JP": "このリポジトリを削除しますか？"
    },
    "Remove Unity installation": {
        "zh-CN": "删除 Unity 安装记录",
        "ja-JP": "Unityの登録を削除"
    },
    "Repository added.": {
        "zh-CN": "已添加存储库。",
        "ja-JP": "リポジトリを追加しました。"
    },
    "Repository refreshed.": {
        "zh-CN": "已刷新存储库。",
        "ja-JP": "リポジトリを更新しました。"
    },
    "Repository removed.": {
        "zh-CN": "已删除存储库。",
        "ja-JP": "リポジトリを削除しました。"
    },
    "Repository URL": {
        "zh-CN": "存储库 URL",
        "ja-JP": "リポジトリのURL"
    },
    "Request cancellation?": {
        "zh-CN": "请求取消？",
        "ja-JP": "キャンセルを要求しますか？"
    },
    "Resource ID": {
        "zh-CN": "资源 ID",
        "ja-JP": "リソースID"
    },
    "Resource kind": {
        "zh-CN": "资源类型",
        "ja-JP": "リソースの種類"
    },
    "retain data": {
        "zh-CN": "保留数据",
        "ja-JP": "データを保持"
    },
    "Review": {
        "zh-CN": "确认",
        "ja-JP": "確認"
    },
    "Review backup restore": {
        "zh-CN": "确认恢复备份",
        "ja-JP": "バックアップの復元内容を確認"
    },
    "Review extension install": {
        "zh-CN": "确认扩展安装",
        "ja-JP": "拡張機能のインストールを確認"
    },
    "Review extension uninstall": {
        "zh-CN": "确认扩展卸载",
        "ja-JP": "拡張機能のアンインストールを確認"
    },
    "Review import": {
        "zh-CN": "确认导入",
        "ja-JP": "インポート内容を確認"
    },
    "Review migration": {
        "zh-CN": "确认迁移",
        "ja-JP": "移行内容を確認"
    },
    "Review project creation": {
        "zh-CN": "确认创建项目",
        "ja-JP": "プロジェクトの作成を確認"
    },
    "Review registration": {
        "zh-CN": "确认添加",
        "ja-JP": "登録内容を確認"
    },
    "Review removal": {
        "zh-CN": "确认删除",
        "ja-JP": "削除内容を確認"
    },
    "Review repository": {
        "zh-CN": "确认存储库",
        "ja-JP": "リポジトリを確認"
    },
    "Review the changes ALCOMD will make to this project.": {
        "zh-CN": "请确认 ALCOMD 将对此项目进行的更改。",
        "ja-JP": "ALCOMDがこのプロジェクトに適用する変更を確認してください。"
    },
    "Review the removal before applying it.": {
        "zh-CN": "请在应用前确认删除内容。",
        "ja-JP": "適用前に削除内容を確認してください。"
    },
    "Review Unity migration": {
        "zh-CN": "确认 Unity 迁移",
        "ja-JP": "Unityの移行内容を確認"
    },
    "Revoke": {
        "zh-CN": "撤销",
        "ja-JP": "取り消し"
    },
    "same version": {
        "zh-CN": "相同版本",
        "ja-JP": "同じバージョン"
    },
    "Save launch arguments": {
        "zh-CN": "保存启动参数",
        "ja-JP": "起動引数を保存"
    },
    "Select an installation": {
        "zh-CN": "选择安装",
        "ja-JP": "インストールを選択"
    },
    "Source type": {
        "zh-CN": "来源类型",
        "ja-JP": "ソースの種類"
    },
    "Specify the exact permission and resource scope. Changes take effect only after Grant or Revoke.": {
        "zh-CN": "请填写准确的权限名称和资源范围。仅在点击授予或撤销后生效。",
        "ja-JP": "正確な権限とリソース範囲を指定してください。「付与」または「取り消し」を実行した後に変更が有効になります。"
    },
    "Store": {
        "zh-CN": "不压缩",
        "ja-JP": "無圧縮"
    },
    "succeeded": {
        "zh-CN": "已完成",
        "ja-JP": "完了"
    },
    "Supported": {
        "zh-CN": "支持",
        "ja-JP": "対応"
    },
    "Target must be absent": {
        "zh-CN": "目标必须不存在",
        "ja-JP": "作成先が存在しないこと"
    },
    "Target parent": {
        "zh-CN": "目标父目录",
        "ja-JP": "作成先の親フォルダー"
    },
    "Target Unity installation": {
        "zh-CN": "目标 Unity 安装",
        "ja-JP": "移行先のUnity"
    },
    "Target Unity version": {
        "zh-CN": "目标 Unity 版本",
        "ja-JP": "移行先のUnityバージョン"
    },
    "The connected daemon did not negotiate {capability}.": {
        "zh-CN": "当前连接的后台服务不支持 {capability}。",
        "ja-JP": "接続中のバックグラウンドサービスは{capability}に対応していません。"
    },
    "The daemon validates and owns this path.": {
        "zh-CN": "此路径由后台服务验证和管理。",
        "ja-JP": "このパスはバックグラウンドサービスが検証・管理します。"
    },
    "The daemon will revalidate this frozen plan before applying it.": {
        "zh-CN": "应用前，后台服务将重新验证此已冻结的计划。",
        "ja-JP": "適用前に、バックグラウンドサービスが確定済みのプランを再検証します。"
    },
    "The daemon will revalidate this frozen plan before importing.": {
        "zh-CN": "导入前，后台服务将重新验证此已冻结的计划。",
        "ja-JP": "インポート前に、バックグラウンドサービスが確定済みのプランを再検証します。"
    },
    "The daemon will revalidate this frozen plan. A stale plan fails instead of being silently replaced.": {
        "zh-CN": "后台服务将重新验证此已冻结的计划。过期计划会失败，不会被静默替换。",
        "ja-JP": "バックグラウンドサービスが確定済みのプランを再検証します。古いプランは自動で置き換えず、エラーになります。"
    },
    "The daemon will revalidate this install plan before applying it.": {
        "zh-CN": "应用前，后台服务将重新验证此安装计划。",
        "ja-JP": "適用前に、バックグラウンドサービスがインストールプランを再検証します。"
    },
    "The daemon will revalidate this uninstall plan before applying it.": {
        "zh-CN": "应用前，后台服务将重新验证此卸载计划。",
        "ja-JP": "適用前に、バックグラウンドサービスがアンインストールプランを再検証します。"
    },
    "The package information changed or is no longer available. Refresh the package list and try again.": {
        "zh-CN": "软件包信息发生变化或已不可用。请刷新软件包列表后重试。",
        "ja-JP": "パッケージ情報が変更されたか、利用できなくなりました。パッケージ一覧を更新して再試行してください。"
    },
    "The selected package actions conflict. Review the selection and try again.": {
        "zh-CN": "所选软件包操作存在冲突。请检查选择后重试。",
        "ja-JP": "選択したパッケージの操作が競合しています。選択内容を確認して再試行してください。"
    },
    "The Unity migration did not complete.": {
        "zh-CN": "Unity 迁移未完成。",
        "ja-JP": "Unityの移行が完了しませんでした。"
    },
    "This package is not currently installed in the project.": {
        "zh-CN": "项目中尚未安装此软件包。",
        "ja-JP": "このパッケージはプロジェクトにインストールされていません。"
    },
    "This project changed while the package changes were being prepared. Refresh the package list and try again.": {
        "zh-CN": "准备软件包更改时项目发生了变化。请刷新软件包列表后重试。",
        "ja-JP": "パッケージの変更を準備中にプロジェクトが変更されました。パッケージ一覧を更新して再試行してください。"
    },
    "This project requires Unity {version}. No exact matching Unity installation was found.": {
        "zh-CN": "此项目需要 Unity {version}，未找到版本完全匹配的 Unity 安装。",
        "ja-JP": "このプロジェクトにはUnity {version}が必要ですが、完全に一致するバージョンが見つかりません。"
    },
    "This removes only the ALCOMD registry entry. It does not delete the Unity project.": {
        "zh-CN": "仅删除 ALCOMD 中的项目记录，不会删除 Unity 项目文件。",
        "ja-JP": "ALCOMDの登録情報のみを削除します。Unityプロジェクトのファイルは削除しません。"
    },
    "this repository": {
        "zh-CN": "此存储库",
        "ja-JP": "このリポジトリ"
    },
    "This repository has {count} reported issues.": {
        "zh-CN": "此存储库报告了 {count} 个问题。",
        "ja-JP": "このリポジトリには {count} 件の問題があります。"
    },
    "To": {
        "zh-CN": "到",
        "ja-JP": "変更後"
    },
    "Unable to open Unity: {code}": {
        "zh-CN": "无法打开 Unity：{code}",
        "ja-JP": "Unityを開けません：{code}"
    },
    "Uninstall extension": {
        "zh-CN": "卸载扩展",
        "ja-JP": "拡張機能をアンインストール"
    },
    "Unity {version} is required. No exact matching Unity installation was found.": {
        "zh-CN": "需要 Unity {version}，未找到版本完全匹配的 Unity 安装。",
        "ja-JP": "Unity {version}が必要ですが、完全に一致するバージョンが見つかりません。"
    },
    "Unity action failed: {code}": {
        "zh-CN": "Unity 操作失败：{code}",
        "ja-JP": "Unityの操作に失敗しました：{code}"
    },
    "Unity actions": {
        "zh-CN": "Unity 操作",
        "ja-JP": "Unityの操作"
    },
    "Unity executable": {
        "zh-CN": "Unity 可执行文件",
        "ja-JP": "Unityの実行ファイル"
    },
    "Unity installation for this launch": {
        "zh-CN": "本次启动使用的 Unity",
        "ja-JP": "今回の起動に使用するUnity"
    },
    "Unity installation registered.": {
        "zh-CN": "已添加 Unity。",
        "ja-JP": "Unityを登録しました。"
    },
    "Unity installation removed from the list.": {
        "zh-CN": "已从列表删除 Unity。",
        "ja-JP": "Unityを一覧から削除しました。"
    },
    "Unity installation required": {
        "zh-CN": "需要安装 Unity",
        "ja-JP": "Unityのインストールが必要です"
    },
    "Unity Editor version could not be verified.": {
        "zh-CN": "无法验证 Unity Editor 版本。",
        "ja-JP": "Unity Editor のバージョンを確認できませんでした。"
    },
    "Unity installation could not be validated.": {
        "zh-CN": "无法验证 Unity 安装。",
        "ja-JP": "Unity のインストールを検証できませんでした。"
    },
    "Unity installations refreshed.": {
        "zh-CN": "已刷新 Unity 安装列表。",
        "ja-JP": "Unityのインストール一覧を更新しました。"
    },
    "Unity launch {state}.": {
        "zh-CN": "Unity 启动状态：{state}。",
        "ja-JP": "Unityの起動状態：{state}。"
    },
    "Unity launch arguments": {
        "zh-CN": "Unity 启动参数",
        "ja-JP": "Unityの起動引数"
    },
    "Unity launch arguments cleared.": {
        "zh-CN": "已清除 Unity 启动参数。",
        "ja-JP": "Unityの起動引数を消去しました。"
    },
    "Unity launch arguments updated.": {
        "zh-CN": "已更新 Unity 启动参数。",
        "ja-JP": "Unityの起動引数を更新しました。"
    },
    "Unity migration accepted.": {
        "zh-CN": "已接受 Unity 迁移请求。",
        "ja-JP": "Unityの移行リクエストを受け付けました。"
    },
    "Unnamed repository": {
        "zh-CN": "未命名存储库",
        "ja-JP": "名前のないリポジトリ"
    },
    "Unregister": {
        "zh-CN": "从列表删除",
        "ja-JP": "一覧から削除"
    },
    "Unregister this project?": {
        "zh-CN": "从列表删除此项目？",
        "ja-JP": "このプロジェクトを一覧から削除しますか？"
    },
    "user approved for extension": {
        "zh-CN": "用户为此扩展批准",
        "ja-JP": "ユーザーがこの拡張機能で承認"
    },
    "Version {version}": {
        "zh-CN": "版本 {version}",
        "ja-JP": "バージョン {version}"
    },
    "View template": {
        "zh-CN": "查看模板",
        "ja-JP": "テンプレートを表示"
    },
    "VPM packages require a separate resolve after restoration.": {
        "zh-CN": "恢复后需要单独解析 VPM 软件包依赖。",
        "ja-JP": "復元後にVPMパッケージの依存関係を別途解決する必要があります。"
    }
};
