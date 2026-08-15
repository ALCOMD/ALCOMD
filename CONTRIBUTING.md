# Contributing to ALCOMD3

Languages: English | [日本語](CONTRIBUTING/CONTRIBUTING.ja.md) |
[简体中文](CONTRIBUTING/CONTRIBUTING.zh-CN.md)

Thank you for helping improve ALCOMD3. Bug reports, feature ideas,
documentation improvements, translations, tests, and code changes are welcome.

## Before making changes

- Search existing [issues](https://github.com/ALCOMD/ALCOMD/issues) and
  [discussions](https://github.com/ALCOMD/ALCOMD/discussions) first.
- Use the [issue forms](https://github.com/ALCOMD/ALCOMD/issues/new/choose)
  for bug reports and feature requests. Use Discussions for questions.
- Small fixes may be submitted directly. Please discuss large features,
  compatibility changes, or architectural changes before implementing them.
- Keep discussions respectful and constructive.
- Do not report security vulnerabilities publicly. Email
  [github@cqmhv.com](mailto:github@cqmhv.com) instead.

## Development setup

You need the Rust toolchain declared in `alcomd3.config.json`, Node.js 24, and
the [platform prerequisites required by Tauri
v2](https://v2.tauri.app/start/prerequisites/).

After cloning your fork, install the GUI dependencies and start the application:

```bash
cd vrc-get-gui
npm ci
npm run tauri dev
```

## Making changes

- Keep each change focused and follow the existing code style.
- Add or update tests when behavior changes.
- Add user-facing text through the localization system. See the
  [GUI contribution guide](vrc-get-gui/CONTRIBUTING.md).
- Update relevant documentation when behavior or public configuration changes.
- Add important user-visible or release-related changes to the appropriate
  `Unreleased` section in `CHANGELOG.md`. Do not add entries for internal-only
  refactoring, tests, formatting, or CI changes.
- Some `vrc-get` names remain for compatibility and should not be renamed as
  ordinary cleanup. See [MAINTENANCE.md](docs/MAINTENANCE.md).

## Checks

Run the checks relevant to your change. The complete guidance is in
[TESTING.md](docs/TESTING.md).

For Rust changes:

```bash
cargo fmt --all --check
cargo clippy --workspace --exclude windows-installer-wrapper --all-targets --locked -- -D clippy::correctness
cargo check --workspace --exclude windows-installer-wrapper --locked
cargo test --workspace --exclude windows-installer-wrapper --locked
```

For GUI changes, from `vrc-get-gui/`:

```bash
npm run check
npm run lint
npm test
npm run build
```

If you cannot run a relevant check, mention that in the pull request.

## Pull requests and the CLA

In your pull request, explain the problem and solution, link related issues,
list the checks you ran, and include screenshots for visible UI changes. Keep
unrelated changes out of the same pull request.

Individual contributors must sign the [Contributor License Agreement](CLA.md)
before a pull request can be merged. The CLA workflow will provide the signing
instructions. If an employer may own your contribution, or if you contribute
for an organization, contact [github@cqmhv.com](mailto:github@cqmhv.com) before
signing.
