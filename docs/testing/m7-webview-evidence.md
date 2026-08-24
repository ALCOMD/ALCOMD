# M7 test-only real-WebView evidence

This evidence stage evaluates the `sandboxed_cross_origin_iframe` candidate only. It does not
freeze the production Extension UI container and does not add production GUI wiring, Tauri
commands, capabilities, dependencies, platform APIs, or public contracts.

## Harness boundary

- `apps/alcomd-gui/src-tauri/examples/m7_isolation_probe.rs` is a test-only Tauri executable.
- `tauri.m7-probe.conf.json` grants `core:app:allow-name` only to the probe's host window as a
  positive main-WebView control. The extension documents must not reach that command.
- The only positive Extension UI Bridge method is the test-only `headless.test.ping`. It is an
  in-harness implementation of the already frozen Bridge v1 envelope and is not compiled into a
  production binary.
- Windows runs the probe with WebView2, Ubuntu under Xvfb with WebKitGTK, and macOS directly with
  WKWebView. Xvfb supplies a display only; it does not replace the WebKitGTK engine.
- The harness builds with Tauri's existing `custom-protocol` feature in release asset mode. The
  Windows runner embeds a test-only Common Controls v6 activation manifest with the installed
  Windows SDK `mt.exe`; the manifest is not attached to any production binary.
- The trusted host coordinator is a main-frame-only initialization script. Extension documents do
  not receive it, remain subject to their own CSP, and still must cross the Bridge boundary.
- `isolation_failed`, `harness_unavailable`, malformed evidence, a timeout, a process failure, or
  an unexpected engine fails the hosted job. No probe step uses `continue-on-error`.

## Physical mapping exercised

The probe serves three immutable, `no-store` extension documents through the test-only
`alcomd-extension-ui` scheme:

| Binding | Extension | Package digest purpose |
|---|---|---|
| A/X | `dev.example.m7-probe-a` | baseline package |
| B/X | `dev.example.m7-probe-b` | cross-extension isolation |
| A/Y | `dev.example.m7-probe-a` | replacement-package isolation |

Each document uses its own unguessable session path and an iframe with exactly
`sandbox="allow-scripts"`. The host binds Bridge messages to both `event.source` and the expected
session/logical origin. It rejects B/X confused-deputy messages carrying the A/X session and
invalidates the A/X session before the A/Y positive control completes.

The extension response applies this CSP as both an HTTP response header and a static-document
defense in depth:

```text
default-src 'none'; script-src 'self'; style-src 'none'; connect-src 'none'; img-src 'none';
media-src 'none'; object-src 'none'; child-src 'none'; frame-src 'none'; worker-src 'none';
manifest-src 'none'; form-action 'none'; base-uri 'none';
frame-ancestors https://tauri.localhost tauri://localhost
```

The probe attempts Tauri global/internal/raw invoke, event and channel transport, parent/opener
DOM access, confused-deputy traffic, daemon-socket access, filesystem authority, clipboard,
notification, unrestricted network, object/worker/form loading, popup, and top navigation. A
successful main-only command from any extension document is an isolation failure; merely observing
that a JavaScript object is absent is not treated as sufficient evidence.

## Machine-readable result

Each job prints and validates `target/m7-webview-evidence/<platform>.json`. Required fields are:

```text
platform, webviewEngine, candidateMode, processEnteredMain, webviewCreated,
extensionDocumentLoaded, physicalOrigin, logicalOriginBinding, cspApplied,
bridgeEstablished, tauriGlobalPresent, tauriInternalsPresent, rawInvokeReachable,
eventTransportReachable, channelTransportReachable, parentDomReachable, openerReachable,
topNavigationSucceeded, networkSucceeded, filesystemAuthority, daemonSocketAuthority, result
```

The evidence additionally records the physical scheme, opaque `postMessage` origin, document
origin/URL and user-agent engine evidence for all three bindings, CSP violations,
clipboard/notification/object/worker/form authority, main-only command outcome, cross-origin
isolation, old-session invalidation, and failed checks. `result` is restricted to `pass`,
`isolation_failed`, or `harness_unavailable`. A process that fails before Rust `main` produces
`harness_unavailable` with `processEnteredMain = false` in the runner-generated evidence.

This probe is isolation evidence, not screenshot, accessibility, Windows 10/11 client lifecycle,
installer, or final production-container evidence.
