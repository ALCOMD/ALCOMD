# M7 Official GUI settings/activity/diagnostics threat model

状态：M7 active engineering contract。

| threat | boundary | required response |
|---|---|---|
| malicious or hand-edited settings file | `config/settings.toml` -> application | size/UTF-8/schema/field/type/value validation; unknown or duplicate input fails closed |
| stale GUI overwrites another writer | `settings.update` | checked `expectedRevision`; `revision_conflict`; no automatic retry with a changed value |
| crash during settings replace | daemon filesystem writer | same-directory synced temporary file, recoverable target/backup protocol, deterministic restart recovery |
| raw durable payload leaks into Activity | Event/Operation -> `activity.list` | construct closed safe projection; never parse or return request/result/payload JSON |
| technical details leak into Diagnostics | Operation failure -> `diagnostics.list` | stable code/diagnostic ID/bounded fixed summary only; no raw log source or durable log table |
| permission bypass | all four RPC methods | revalidate exact public permission at application boundary; local-owner grant is not client metadata |
| pagination amplification | list methods | deterministic keyset cursor, maximum 200 items, no unbounded offset scan |
| frontend becomes authority | Tauri/React adapter | typed closed commands only; no direct config file, SQLite, Event join, log file, or daemon socket access |

The redaction denylist includes token, Authorization, raw argv/environment, full private path, raw SQL,
Rust Debug/backtrace, extension-owned values and Portable UI documents/actions/form values. `diagnostics.read`
does not authorize raw diagnostic export; that remains a future, separately approved permission.

The owner-approved Hub location increment makes one explicit settings-payload exception:
`settings.unity.hubPath` and derived `unityHub.path` disclose the requested executable path
to callers already authorized by `settings.read` or the successful `settings.manage` command.
They do not authorize arbitrary file reads/writes or executable launch. The daemon validates
only explicit path changes through its application/platform port; evidence uses bounded,
read-only filesystem inspection and is not persisted. Evidence failure after publication
must not turn a committed preference update into a reported write failure. Raw paths remain
forbidden in Activity, Events, ordinary errors and diagnostics projections. Native selection
only returns a path to the GUI; no Tauri command owns authoritative configuration.
