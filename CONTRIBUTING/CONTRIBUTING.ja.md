# Contributing

言語: [English](../CONTRIBUTING.md) | 日本語 | [简体中文](CONTRIBUTING.zh-CN.md)

### プロジェクト標準

ALCOMD3 は独立したプロジェクトとして保守する。現在のリポジトリ、
ドキュメント、リリース手順、ユーザーに見える挙動を基準にする。

外部の修正は有用な場合があるが、ALCOMD3 の現在のアーキテクチャ、特に
GUI/MCP が共有する操作モデルに合わせて確認し、適用する必要がある。

### 開発範囲

- GUI コードは `vrc-get-gui/` にある。
- VPM package と project management のコードは `vrc-get-vpm/` にある。
- CLI 互換コードは `vrc-get/` にある。
- MCP bridge コードは `alcomd3-mcp/` にある。
- MCP IPC protocol 型は `alcomd3-mcp-protocol/` にある。
- リリースとパッケージング補助は `xtask/` にある。

一部のディレクトリ名や package 名には互換性と履歴上の理由で `vrc-get` が残っている。
互換性移行として明示的に範囲指定されていない限り、リネームしない。

### 環境

推奨ツール:

- Rust stable toolchain。
- Node.js と npm。
- Windows MSVC target をビルドする場合は Windows build tools。
- Windows installer をローカルで扱う場合は Inno Setup。タスクが対応している場合は
  `xtask` にダウンロード / キャッシュさせてもよい。

このリポジトリを直接 clone する:

```bash
git clone https://github.com/ALCOMD3/ALCOMD3.git
```

### ローカル開発

Rust workspace member をビルドしてテストする:

```bash
cargo check
cargo test
```

デスクトップ GUI を development mode で起動する:

```bash
cd vrc-get-gui
npm install
npm run tauri dev
```

### リリース方針

ALCOMD3 は独自のリリース手順を持つ。ALCOMD3 の release automation、
署名 secret、updater metadata、release naming を使う。

Stable release version は `2.0.0`、`2.0.1`、`2.1.0` などの SemVer を使う。
Prerelease build は `2.1.0-beta.1` のような suffix を使ってよい。

Windows release build は scripted release flow を使う。Local artifact
validation には次を使う:

```powershell
cargo xtask release-build --version 2.0.1 --channel stable
```

完全な release workflow は `docs/RELEASE/RELEASE.ja.md` に記載する。Updater key と signature
の詳細は `docs/ALCOMD3_UPDATER/ALCOMD3_UPDATER.ja.md` に記載する。
Agent release procedure は `docs/skills/alcomd3-release/SKILL.md` に記載する。
Maintenance、release、MCP、format、historical documentation を探す場合は
`docs/README/README.ja.md` を documentation index として使う。

### Contributor License Agreement

個人 contributor の pull request を merge する前に、[ALCOMD3 Individual Contributor
License Agreement](../CLA.md) への署名が必要です。

CLA は copyright ownership を譲渡するものではありません。Contributor は自身の
contribution の copyright を保持します。公開 Project に取り込まれた contribution は
ALCOMD3 の現在の main license である `AGPL-3.0-only` で配布されます。同時に CLA は、
closed-source distribution の可能性を含め、contribution の使用、変更、配布、再ライセンス
に関する広範な権利を Maintainer に付与します。

Pull request を作成すると、CLA workflow が署名方法をコメントします。Workflow が示す
正確なコメントを投稿して署名してください。CLA の各 version への署名は一度だけ必要です。

雇用主が成果物の権利を持つ可能性がある場合、または会社・組織を代表して contribution
する場合は、個人 CLA に署名する前に Maintainer へ連絡してください。

### 外部変更の取り込み

他のリポジトリからの変更は merge の標準ではなく、選択的な取り込みとして扱う:

1. 元の commit、pull request、release note、issue を確認する。
2. security、data safety、VRChat/VPM compatibility、user-visible bug に関係するか確認する。
3. 盲目的に merge せず、ALCOMD3 の architecture に合わせて適用する。
4. 影響を受ける Rust、GUI、MCP の code path を検証する。
5. ユーザーに見える重要な変更は `release-notes/` に記録する。

Package operation、project mutation、repository management、operation cancellation、
resource locking、MCP visibility に触れる変更は特に注意する。GUI と MCP bridge は
同じ backend business logic と safety check を共有し続ける必要がある。

### 互換性規則

- Tauri identifier、インストール済み実行ファイル名、protocol name、user data path、
  `vrc-get` compatibility path を安易に変更しない。
- URL、version、color、public path は project config が管理している場合、ハードコードしない。
- ALCOMD3 updater metadata は ALCOMD3 所有の endpoint を指す。
- 別途 design review で明示的に変えない限り、MCP は既定で無効かつ local-only にする。
- 既存の user data と migration path を保持する。

### Pull request の期待値

- PR は focused にする。
- ユーザーに見える挙動変更を説明する。
- 検証結果を含める。検証していない場合は理由を明記する。
- 挙動、互換性、packaging、public configuration が変わる場合は docs と release notes を更新する。
- 無関係な formatting churn を避ける。
