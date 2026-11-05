# M7 Official GUI settings threat model

状态：M7 active engineering contract。

| threat | boundary | required response |
|---|---|---|
| malicious or hand-edited settings file | `config/settings.toml` -> application | size/UTF-8/schema/field/type/value validation; unknown or duplicate input fails closed |
| stale GUI overwrites another writer | `settings.update` | checked `expectedRevision`; `revision_conflict`; no automatic retry with a changed value |
| crash during settings replace | daemon filesystem writer | same-directory synced temporary file, recoverable target/backup protocol, deterministic restart recovery |
| permission bypass | settings and retained business RPC | revalidate public permissions at the application boundary; client metadata is not authority |
| frontend becomes authority | Tauri/React adapter | typed closed commands only; no direct config file, SQLite, Event join, log file, or daemon socket access |

The owner withdrew the logging, activity and floating reminder systems on 2026-11-05. Their RPCs, permissions, file readers and producers are removed. Business Operations, Events and their existing stable error/privacy boundaries remain; no UI-authored log projection replaces them.

The owner-approved Hub location increment makes one explicit settings-payload exception:
`settings.unity.hubPath` and derived `unityHub.path` disclose the requested executable path
to callers already authorized by `settings.read` or the successful `settings.manage` command.
They do not authorize arbitrary file reads/writes or executable launch. The daemon validates
only explicit path changes through its application/platform port; evidence uses bounded,
read-only filesystem inspection and is not persisted. Evidence failure after publication
must not turn a committed preference update into a reported write failure. Raw paths remain
forbidden in Events and ordinary errors. Native selection
only returns a path to the GUI; no Tauri command owns authoritative configuration.

The owner-approved Config5 default-directory increment extends only that settings-payload
exception to `settings.paths.projectDirectory`, `settings.paths.backupDirectory` and the
same two effective `directories` fields. Explicit directory changes are bounded, absolute,
existing-directory selections validated by Core; defaults are resolved read-only before a
settings write can commit. Reading settings never creates directories or changes project
files. A directory preference authorizes no file read/write by a caller; existing project
and backup use cases retain their own permission, path identity, transaction and recovery
checks. Changing a preference does not relocate existing backups or redirect accepted tasks.

Config8 adds a default-directory discovery worker owned by Core/application. Changing
`projects.autoDiscover`, or changing the project directory while the effective switch is on,
requires `projects.manage` in addition to `settings.manage`; a settings-only principal cannot
use the local-owner worker to obtain registry write authority. Scanning is single-in-flight,
bounded to 4096 immediate directory entries and rejects links/reparse points. Each candidate
rechecks root/child identity and the current settings revision before registry acceptance.
It shares the ProjectCreate lock with project creation, restore, and copy publication to preserve their
preallocated project identities. Disabling or changing the root orders before later old-scan
registrations. Failed scans never delete existing projects or write project files.

Backup defaults are resolved with the destination from one authoritative snapshot, then
frozen in the accepted request. Replaying omitted options uses the accepted values, whereas
explicitly different values conflict. The optional Library inclusion affects only the
standard project-root Library directory; archive path, link, byte/entry and consistency
limits remain mandatory. Existing requests without the flag retain false and their original
fingerprint. GUI default editing and project-list follow-up remain typed RPC clients:
they do not enumerate directories, read caches or perform registry writes themselves.
