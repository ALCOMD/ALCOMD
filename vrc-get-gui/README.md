# ALCOMD3 GUI

[![Github Release][shields-github-version]][release-alcomd3]

[shields-github-version]: https://img.shields.io/github/v/release/ALCOMD3/ALCOMD3
[release-alcomd3]: https://github.com/ALCOMD3/ALCOMD3/releases

This directory contains the ALCOMD3 desktop application. The GUI is built with
Tauri and React/Vite, and it uses the Rust crates in this workspace for VPM
logic, MCP integration, bundling, and release support.

ALCOMD3 is not an official product of VRChat or VCC.

## Development

Requirements:

- [Node.js] 24
- [npm] bundled with Node.js 24
- [cargo] and a current Rust stable toolchain
- Platform dependencies required by
  [Tauri v2](https://v2.tauri.app/start/prerequisites/#system-dependencies)

[Node.js]: https://nodejs.org/en
[npm]: https://www.npmjs.com
[cargo]: https://doc.rust-lang.org/cargo/

Run the Tauri development app from this directory:

```bash
npm run tauri dev
```

For frontend-only development:

```bash
npm run dev
```

From the repository root, production-oriented GUI builds should use `xtask`:

```bash
cargo xtask build-alcom --release
```

This builds the frontend, the main `ALCOMD3` executable, and the `alcomd3-mcp`
bridge for the current platform. Release and signing details are documented in
[../docs/RELEASE.md](../docs/RELEASE.md) and
[../docs/ALCOMD3_UPDATER.md](../docs/ALCOMD3_UPDATER.md).

## Directory Layout

- `app/`: React route and page code.
- `components/`: reusable frontend components.
- `lib/`: frontend utility code and generated bindings.
- `locales/`: localization resources.
- `src/`: Rust/Tauri backend code.
- `bundle/`: bundle configuration and packaging assets.
- `capabilities/`: Tauri capability configuration.
- `icons/`: application icon assets.
- `project-templates/`: built-in Unity project templates.
- `windows-installer-wrapper/`: helper executable used by the Windows installer.
- `third-party/`: bundled third-party notices and assets.

Generated or local-only directories such as `gen/`, `out/`, and `node_modules/`
are ignored by Git.

## Unity Project State

Project actions distinguish three Unity states:

- `Closed`: Unity can be launched for the project.
- `Opening`: a launch has started, so duplicate launches are blocked while the
  GUI waits for Unity to acquire the project lock and show its editor window.
- `Open`: Unity owns the project's `Temp/UnityLockFile` and its editor is ready.

Opening state is held in the backend, cleared when launching fails, and expires
after a bounded interval if Unity never acquires the lock. Unity processes are
matched to projects by their working directory without converting paths to
lossy strings. After the lock is acquired on Windows, the project remains
`Opening` until the matching Unity process has a visible editor main window;
startup, splash, and utility windows do not count.

Windows caches a shared process and editor-window snapshot for a short interval,
so readiness checks and foreground activation reuse one `EnumWindows` pass. The
cached process and window are validated before activation. Windows restores and
focuses the matching editor, requesting attention if foreground activation is
rejected. macOS activates the matching Unity application through
`NSRunningApplication`. Linux still reports a locked project as open but does
not offer the bring-to-front action.

The backend state and shared platform interface live in `src/state/unity.rs`
and `src/os.rs`; platform behavior and process discovery live in the corresponding
`src/os_*.rs` files and `src/unity_process.rs`. Project-button polling and
presentation live in `components/OpenUnityButton.tsx`.

## Related Documentation

- [Root README](../README.md): project overview, downloads, and repository layout.
- [MCP guide](../docs/mcp.md): MCP usage, tools, lifecycle, and troubleshooting.
- [Maintenance notes](../docs/MAINTENANCE.md): compatibility boundaries and
  maintainer-facing behavior notes.
- [Release workflow](../docs/RELEASE.md): release process and artifacts.
- [Updater signing](../docs/ALCOMD3_UPDATER.md): updater key and signature notes.
- [THIRD-PARTY.md](THIRD-PARTY.md): GUI third-party notices.

## Contribution

GUI changes should follow the repository contribution rules and the local GUI
guidance:

- [../CONTRIBUTING.md](../CONTRIBUTING.md)
- [CONTRIBUTING.md](CONTRIBUTING.md)

## License

ALCOMD3 is licensed under the GNU Affero General Public License v3.0 only
(`AGPL-3.0-only`). See [../LICENSE](../LICENSE) and
[../LICENSE-NOTES.md](../LICENSE-NOTES.md) for more information.
