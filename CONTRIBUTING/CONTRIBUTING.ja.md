# ALCOMD3 への貢献

言語: [English](../CONTRIBUTING.md) | 日本語 | [简体中文](CONTRIBUTING.zh-CN.md)

ALCOMD3 の改善にご協力いただきありがとうございます。バグ報告、機能提案、ドキュメント改善、
翻訳、テスト、コード変更を歓迎します。

## 変更を始める前に

- 既存の [Issue](https://github.com/ALCOMD/ALCOMD/issues) と
  [Discussion](https://github.com/ALCOMD/ALCOMD/discussions) を先に検索してください。
- バグ報告と機能提案には
  [Issue フォーム](https://github.com/ALCOMD/ALCOMD/issues/new/choose) を使用し、
  質問は Discussions に投稿してください。
- 小さな修正は直接提出できます。大きな機能、互換性変更、アーキテクチャ変更は、
  実装前に相談してください。
- 敬意を持って建設的に話し合ってください。
- セキュリティ脆弱性を公開しないでください。
  [github@cqmhv.com](mailto:github@cqmhv.com) へ報告してください。

## 開発環境

`alcomd3.config.json` で指定された Rust ツールチェーン、Node.js 24、および
[Tauri v2 が対象プラットフォームに要求する依存関係](https://v2.tauri.app/start/prerequisites/)
が必要です。

Fork を clone した後、GUI の依存関係をインストールしてアプリを起動します。

```bash
cd vrc-get-gui
npm ci
npm run tauri dev
```

## 変更時の注意

- 一つの変更範囲を明確にし、既存のコードスタイルに従ってください。
- 動作を変更する場合は、テストを追加または更新してください。
- ユーザー向けテキストはローカライズ機構を通して追加してください。詳細は
  [GUI 貢献ガイド](../vrc-get-gui/CONTRIBUTING/CONTRIBUTING.ja.md) を参照してください。
- 動作または公開設定を変更する場合は、関連ドキュメントを更新してください。
- 重要なユーザー向け変更またはリリースに影響する変更は、`CHANGELOG.md` の適切な
  `Unreleased` 分類に追加してください。内部リファクタリング、テスト、書式変更、CI のみの
  変更は追加しません。
- 一部の `vrc-get` という名前は互換性のために残っています。通常の整理として変更しないで
  ください。詳細は [MAINTENANCE.md](../docs/MAINTENANCE/MAINTENANCE.ja.md) を参照してください。

## 検証

変更に関係するチェックを実行してください。完全な説明は
[TESTING.md](../docs/TESTING.md) にあります。

Rust の変更:

```bash
cargo fmt --all --check
cargo clippy --workspace --exclude windows-installer-wrapper --all-targets --locked -- -D clippy::correctness
cargo check --workspace --exclude windows-installer-wrapper --locked
cargo test --workspace --exclude windows-installer-wrapper --locked
```

GUI の変更は `vrc-get-gui/` で実行します。

```bash
npm run check
npm run lint
npm test
npm run build
```

必要なチェックを実行できない場合は、Pull Request に記載してください。

## Pull Request と CLA

Pull Request には、問題と解決方法、関連する Issue、実行したチェックを記載し、画面に見える
UI 変更にはスクリーンショットを添付してください。無関係な変更を同じ Pull Request に含めないで
ください。

個人コントリビューターの Pull Request をマージする前に、[Contributor License
Agreement](../CLA.md) への署名が必要です。署名方法は CLA ワークフローが案内します。雇用主が
貢献物の権利を持つ可能性がある場合、または組織を代表して貢献する場合は、署名前に
[github@cqmhv.com](mailto:github@cqmhv.com) へ連絡してください。
