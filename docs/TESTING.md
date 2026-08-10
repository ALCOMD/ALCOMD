# ALCOMD3 testing

This document defines the maintained test chain. The release procedure remains
documented separately in [RELEASE.md](./RELEASE.md).

## Test layers

| Layer | Command or workflow | Main coverage |
| --- | --- | --- |
| Rust | `cargo test --workspace --exclude windows-installer-wrapper --locked` | VPM behavior, embedded MCP Streamable HTTP/direct dispatch/Tasks, updater and release tooling |
| GUI unit | `npm test` in `vrc-get-gui` | Tauri command serialization, events, cancellation, errors and navigation state |
| Desktop E2E | `Full-chain desktop smoke` | Real Tauri startup on Windows x64, macOS arm64 and Linux x64; first-run setup, isolated project discovery, restart persistence, MCP 2026/legacy compatibility and Tasks behavior |
| macOS bundles | `Full-chain desktop smoke` | `.app`, updater archive and DMG structure, arm64 binaries, explicit ad-hoc signatures, copied-app launch and cleanup |
| Linux packages | `Full-chain desktop smoke` | AppImage/updater archive parity, DEB metadata, fresh install, launch and purge |
| Windows installer/upgrade | `Full-chain desktop smoke` | Previous stable install, old shared AppId, both historical executable names and legacy shortcuts removed, desktop-shortcut choice preserved, new shortcut targets/new AppId registration, user-data preservation, launch and uninstall |
| Release CLI plan | `release-build`, `release-assemble`, `release-publish` and `release-preflight` with `--dry-run` | Command construction only, without signing, publication or external writes |
| Published updater | Scheduled or manual `Full-chain desktop smoke`, Windows job only | Public stable/beta manifest parity, installer download, exact URL/file binding and Minisign verification; stable also requires the authenticated `release` purpose |
| Formal release chain | `Build release draft` and `Publish updater metadata` | Source-bound native build shards, exact Windows installer upgrade smoke, exact ten-asset allowlist, config-bound macOS ad-hoc signing, three updater Minisign signatures, atomic three-platform metadata and public endpoint |

`Continuous integration` runs the Rust and GUI unit layers on
pull requests and pushes to `main`. `Full-chain desktop smoke` runs its three
platform jobs for relevant pull requests and pushes, every Monday at 03:00
Asia/Shanghai, and on manual dispatch. Public updater checks are a separate step
inside the Windows job and run only for scheduled and manual executions; they
fetch each public manifest, compare it semantically with the checked-out
manifest, and verify the installer referenced by that public response.

## Local verification

Use the Rust version in `alcomd3.config.json` and Node.js 24.

```powershell
cargo fmt --all --check
cargo clippy --workspace --exclude windows-installer-wrapper --all-targets --locked -- -D clippy::correctness
cargo check --workspace --exclude windows-installer-wrapper --locked
cargo test --workspace --exclude windows-installer-wrapper --locked

Push-Location vrc-get-gui
npm.cmd ci
npm.cmd run check
npm.cmd run lint
npm.cmd test
npm.cmd run build
Pop-Location

```

For the real Windows desktop test, build the debug application first:

```powershell
cargo xtask build-alcom --target x86_64-pc-windows-msvc
Push-Location vrc-get-gui
npm.cmd run test:e2e:desktop
Pop-Location
```

Debug desktop E2E sets `ALCOMD3_TEST_LOCAL_DATA_ROOT` to a temporary path and
prewrites an isolated GUI configuration with a reserved MCP port and token. The data-root override is
compiled only when debug assertions are enabled; release builds ignore it.
The MCP scenario checks missing and incorrect tokens, stateless 2026 discovery
and the exact 33-tool catalog, legacy 2025 initialization and ordinary tool
calls, then creates and polls a real project-copy Task. It also verifies the
synchronous fallback for clients without the Tasks capability and that an
authenticated client can still get or cancel an existing Task after data access
is disabled.
The test also disables debug-build system integration. The runner snapshots the
complete current-user `vcc://` registry tree and restores it in `finally` if the
application changes it, including when the test fails. Supplied roots must be empty children of a
temporary, runner-temp or workspace `target` directory.
The runner starts WebdriverIO suspended inside a per-run Windows Job Object;
closing that object stops the complete process tree created by that test only.
The wrapper enforces a 15-minute timeout so a wedged driver still reaches cleanup.

The macOS and Linux jobs build the debug GUI with
`--desktop-e2e-webdriver` and use `npm run test:e2e:desktop:unix`. That build
flag enables a loopback-only embedded WebDriver plugin and is rejected for
non-dev profiles, so it cannot enter release packages. Linux runs the test under
Xvfb. The Unix wrapper owns an isolated process group and applies the same
15-minute completion/result checks as the Windows wrapper.

## Installer and release boundaries

Installer and upgrade smoke tests are deliberately restricted to an ephemeral
GitHub-hosted Windows runner because the installer uses the production AppId
and file associations. The workflow builds an unsigned test installer, upgrades
it over the previous stable installer, verifies that the old shared AppId is
absent from HKCU and both HKLM views, the historical `ALCOM.exe` and
`alcomd3-mcp.exe` are removed, the current installation contains only the GUI executable for MCP, the
old Desktop and Start Menu shortcuts are removed, the previous desktop-shortcut
choice is restored with both new shortcuts targeting `ALCOMD3.exe`, the new
AppId is registered, the new shortcuts, template ProgID, and `vcc://`
registration use the configured Windows AUMID, an unrelated same-named shortcut is preserved, and
existing user data is preserved. It then verifies the
embedded ZIP, launches the application, confirms its `vcc://` command, confirms
that the MCP port is owned by the GUI PID, rejects missing or incorrect bearer
tokens, returns `mcp_disabled` with the correct token, and has no non-loopback
listener on the endpoint port, uninstalls it, and verifies that the new shortcuts are
removed while local application data remains preserved by default.
During the migration window, `legacyWindowsMigrationReleaseTag` in
`alcomd3.config.json` pins the old installer so the baseline cannot advance past
the old AppId after the first new-AppId release.

The macOS package job uses the native `macos-15` arm64 runner and validates an
ad-hoc signed `.app`, updater archive and DMG, including strict signature checks
for the main and extracted updater application and for any future nested executables that are present. The Linux package job uses Ubuntu 22.04
x64 as the AppImage compatibility baseline and validates an AppImage, updater
archive and DEB. It builds the AppImage in `self-updater` mode, then rebuilds the
DEB in `no-self-updater` mode, matching the shared release configuration. Packaged
launch tests isolate `HOME` and XDG directories under `RUNNER_TEMP`, write the
MCP port/token into the isolated GUI configuration, assert that no MCP helper is
packaged and the GUI process owns the loopback listener, and exercise the
disabled/unauthorized boundary before cleanup. The
package smoke helper refuses to run outside an ephemeral GitHub-hosted macOS or
Linux runner.

These `Full-chain desktop smoke` macOS/Linux packages are CI test artifacts
only. They are not uploaded to GitHub Releases and do not claim previous-version
upgrade coverage. Formal multi-platform assets come only from the separate
`Build release draft` DAG. Its Windows shard repeats the upgrade smoke with the
exact setup EXE copied into the source-bound release shard; the historical
baseline phase does not require identity fields introduced by the current
installer. Its macOS app and DMG use the same explicit ad-hoc policy and are not
Apple-notarized. Its Linux build uses the same two configured update modes as
full-chain, so a package-manager install does not replace itself outside the
package manager.

The Draft workflow runs `preflight`, builds source-bound Windows x64, macOS
arm64, and Linux x64 shards on native runners, then uses `release-assemble` to
verify the shards and create three updater Minisign signatures. The published
updater workflow checks all ten Release assets, verifies all three updater
payload/signature pairs, and atomically
generates one manifest containing `windows-x86_64`, `darwin-aarch64`, and
`linux-x86_64`.

The release CLI dry-runs are plan smoke tests; they do not prove updater signing material,
clean/synchronized `main`, or live GitHub Release state. This workflow never reads updater signing secrets, creates a GitHub Release,
publishes updater metadata or pushes a commit. Real signing,
Draft creation and public updater verification remain gates of the documented
release workflows.

Rust integration tests use temporary directories and loopback listeners. Tests must not depend on
or modify a maintainer's existing ALCOMD3, VCC or ALCOM data.
