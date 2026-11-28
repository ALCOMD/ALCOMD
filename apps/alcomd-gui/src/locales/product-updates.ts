import type { TranslationTable } from "../i18n-types";

export const productUpdateMessages: TranslationTable = {
    "Command-line integration": { "zh-CN": "命令行集成", "ja-JP": "コマンドライン連携" },
    "Enable CLI command": { "zh-CN": "启用 CLI 命令", "ja-JP": "CLI コマンドを有効にする" },
    "Repair CLI command": { "zh-CN": "修复 CLI 命令", "ja-JP": "CLI コマンドを修復する" },
    "Remove CLI command": { "zh-CN": "移除 CLI 命令", "ja-JP": "CLI コマンドを削除する" },
    "Enable the alcomd-cli command for this account.": { "zh-CN": "为当前账号启用 alcomd-cli 命令。", "ja-JP": "このアカウントで alcomd-cli コマンドを有効にできます。" },
    "The alcomd-cli command is available in ~/.local/bin.": { "zh-CN": "alcomd-cli 命令已安装在 ~/.local/bin。", "ja-JP": "alcomd-cli コマンドは ~/.local/bin にあります。" },
    "The command needs repair. If the AppImage moved, reopen it from its current location.": {
        "zh-CN": "命令需要修复。如果 AppImage 已移动，请从当前位置重新打开它。",
        "ja-JP": "コマンドの修復が必要です。AppImage を移動した場合は、現在の場所から開き直してください。",
    },
    "An existing file conflicts with CLI integration. ALCOMD preserved it.": {
        "zh-CN": "现有文件与命令行集成冲突。ALCOMD 已保留该文件。",
        "ja-JP": "既存のファイルがコマンドライン連携と競合しています。ALCOMD はそのファイルを保持しました。",
    },
    "Updating CLI integration…": { "zh-CN": "正在更新命令行集成…", "ja-JP": "コマンドライン連携を更新しています…" },
    "CLI integration could not be changed. Reopen the AppImage and retry.": {
        "zh-CN": "无法更改命令行集成。请重新打开 AppImage 后重试。",
        "ja-JP": "コマンドライン連携を変更できませんでした。AppImage を開き直して再試行してください。",
    },
    "Product updates": { "zh-CN": "产品更新", "ja-JP": "製品の更新" },
    "Check for updates": { "zh-CN": "检查更新", "ja-JP": "更新を確認" },
    "Checking for updates…": { "zh-CN": "正在检查更新…", "ja-JP": "更新を確認しています…" },
    "You are using the latest version.": { "zh-CN": "当前已是最新版本。", "ja-JP": "最新バージョンを使用しています。" },
    "Version {version} is available ({size} MiB).": { "zh-CN": "版本 {version} 可供下载（{size} MiB）。", "ja-JP": "バージョン {version}（{size} MiB）をダウンロードできます。" },
    "Download update": { "zh-CN": "下载更新", "ja-JP": "更新をダウンロード" },
    "Downloading the signed update…": { "zh-CN": "正在下载已签名的更新…", "ja-JP": "署名済みの更新をダウンロードしています…" },
    "Version {version} is ready to install.": { "zh-CN": "版本 {version} 已准备好安装。", "ja-JP": "バージョン {version} をインストールできます。" },
    "Install and restart": { "zh-CN": "安装并重启", "ja-JP": "インストールして再起動" },
    "Resume and restart": { "zh-CN": "恢复并重启", "ja-JP": "更新を再開して再起動" },
    "ALCOMD will close to complete the update. Close any other ALCOMD clients before continuing.": {
        "zh-CN": "ALCOMD 将退出以完成更新。请先关闭其他 ALCOMD 客户端。",
        "ja-JP": "更新を完了するため ALCOMD を終了します。続行する前に、ほかの ALCOMD クライアントを閉じてください。",
    },
    "The update could not be prepared. Check again to retry.": { "zh-CN": "更新准备失败。请重新检查后重试。", "ja-JP": "更新を準備できませんでした。もう一度更新を確認してください。" },
    "The update could not finish. Close other ALCOMD clients and check again to retry.": {
        "zh-CN": "更新未能完成。请关闭其他 ALCOMD 客户端，再检查更新并重试。",
        "ja-JP": "更新を完了できませんでした。ほかの ALCOMD クライアントを閉じて、もう一度更新を確認してください。",
    },
    "An interrupted update is ready to resume.": { "zh-CN": "有一项中断的更新可以恢复。", "ja-JP": "中断された更新を再開できます。" },
    "Restarting to complete the update…": { "zh-CN": "正在重启以完成更新…", "ja-JP": "更新を完了するため再起動しています…" },
};
