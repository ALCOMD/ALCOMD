# State45 extension background integration grants

`0045_extension_local_ipc.sql` rebuilds `extension_grants` in one immediate transaction and copies every existing row. It grants neither new permission automatically.

| Permission | Resource kind | Resource ID |
| --- | --- | --- |
| `unity.sessions.read` | `Global` | `*` |
| `local-ipc.connect` | `LocalIpcTarget` | 64 lowercase hexadecimal characters identifying the signed target descriptor |

Extension instance, package, and namespace storage is unchanged. Connections and Unity session samples are ephemeral. Invalid local IPC scope syntax fails the database constraint. Core verifies the requested target against signed installed package content before granting it, and checks the active lease, background grant, and exact target scope when using a channel.
