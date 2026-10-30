import type { TranslationTable } from "../i18n-types";

// Existing terms follow the frozen v3 resources; v4-only text is authored for this GUI.
export const actionMessages: TranslationTable = {
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
    "ALCOMD could not complete the package changes. Try again or open Logs for more information.": {
        "zh-CN": "ALCOMD 无法完成软件包更改。请重试，或打开日志查看详情。",
        "ja-JP": "ALCOMDはパッケージの変更を完了できませんでした。再試行するか、ログで詳細を確認してください。"
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
    "Built-in templates cannot be removed.": {
        "zh-CN": "无法删除内置模板。",
        "ja-JP": "組み込みのテンプレートは削除できません。"
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
    "Create template from project": {
        "zh-CN": "从项目创建模板",
        "ja-JP": "プロジェクトからテンプレートを作成"
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
    "Derive from project": {
        "zh-CN": "从项目派生",
        "ja-JP": "プロジェクトから作成"
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
    "Expected registry revision": {
        "zh-CN": "预期注册表修订号",
        "ja-JP": "想定するレジストリのリビジョン"
    },
    "Export": {
        "zh-CN": "导出",
        "ja-JP": "エクスポート"
    },
    "Export {name} as a template bundle.": {
        "zh-CN": "将 {name} 导出为模板包。",
        "ja-JP": "{name}をテンプレートバンドルとしてエクスポートします。"
    },
    "Export target": {
        "zh-CN": "导出路径",
        "ja-JP": "エクスポート先"
    },
    "Export template": {
        "zh-CN": "导出模板",
        "ja-JP": "テンプレートをエクスポート"
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
    "Import template": {
        "zh-CN": "导入模板",
        "ja-JP": "テンプレートをインポート"
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
    "More template creation options": {
        "zh-CN": "更多模板创建选项",
        "ja-JP": "その他のテンプレート作成方法"
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
    "New template ID": {
        "zh-CN": "新模板 ID",
        "ja-JP": "新しいテンプレートID"
    },
    "no change": {
        "zh-CN": "无更改",
        "ja-JP": "変更なし"
    },
    "No package changes are required for this project.": {
        "zh-CN": "此项目无需更改软件包。",
        "ja-JP": "このプロジェクトのパッケージに変更は必要ありません。"
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
    "Replace an existing matching template": {
        "zh-CN": "替换已有的匹配模板",
        "ja-JP": "一致する既存のテンプレートを置き換える"
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
    "Review new template": {
        "zh-CN": "确认新模板",
        "ja-JP": "新しいテンプレートを確認"
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
    "Review template import": {
        "zh-CN": "确认模板导入",
        "ja-JP": "テンプレートのインポートを確認"
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
    "Template actions": {
        "zh-CN": "模板操作",
        "ja-JP": "テンプレートの操作"
    },
    "Template bundle": {
        "zh-CN": "模板包",
        "ja-JP": "テンプレートバンドル"
    },
    "Template exported.": {
        "zh-CN": "已导出模板。",
        "ja-JP": "テンプレートをエクスポートしました。"
    },
    "Template import accepted.": {
        "zh-CN": "已接受模板导入请求。",
        "ja-JP": "テンプレートのインポートを受け付けました。"
    },
    "Template operation accepted.": {
        "zh-CN": "已接受模板操作请求。",
        "ja-JP": "テンプレートの操作を受け付けました。"
    },
    "Template removed.": {
        "zh-CN": "已删除模板。",
        "ja-JP": "テンプレートを削除しました。"
    },
    "Template version": {
        "zh-CN": "模板版本",
        "ja-JP": "テンプレートのバージョン"
    },
    "Template:": {
        "zh-CN": "模板:",
        "ja-JP": "テンプレート:"
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
    "This repository has {count} reported issues. Registration will validate the source again.": {
        "zh-CN": "此存储库报告了 {count} 个问题。添加时将重新验证来源。",
        "ja-JP": "このリポジトリには {count} 件の問題があります。登録時にソースを再検証します。"
    },
    "To": {
        "zh-CN": "到",
        "ja-JP": "変更後"
    },
    "Transitional input: the current registry read interface does not provide its revision. Enter the verified registry revision; it is never inferred from installed extensions.": {
        "zh-CN": "过渡输入：当前注册表读取接口不提供修订号。请填写已验证的注册表修订号，不会根据已安装扩展推测。",
        "ja-JP": "暫定入力：現在のレジストリ読み取りインターフェースはリビジョンを返しません。確認済みのリビジョンを入力してください。インストール済みの拡張機能から推測することはありません。"
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
