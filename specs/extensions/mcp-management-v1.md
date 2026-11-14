# MCP management Host interface v1

Status: approved by the owner on 2026-11-14; implementation evidence is tracked in the M8 ExecPlan.

The exact additive interface is `alcomd:mcp-management/clients@1.0.0`; the new world
`alcomd:mcp-management/management-v1@1.0.0` includes the frozen extension-v1 world.
The old WIT files, errors and Portable UI protocol are unchanged. Unsupported required imports
fail negotiation; old guests remain supported. Neither first-party provenance nor a Manifest
declaration grants authority.

The six WIT functions and exact records are in `wit/mcp-management-v1/management.wit`.
The approved limits, error semantics, CLI boundary and dependency versions are defined in the
M8 ExecPlan's approved management contract. Private capability names are
`host-mcp-clients.list/grants/projects/create/change-grants/revoke`; they are not public Core RPCs.
Each request and response is limited to 65536 serialized UTF-8 bytes, beneath existing WIT/frame limits.
Cursor maximum: 512 ASCII bytes. Keys: existing 1–128 ASCII-byte idempotency keys.

The owner additionally approved `client-page.can-manage` on 2026-11-14. This current two-sided
permission/lease/UI projection controls presentation only; it never authorizes a subsequent write.
State35, approved at the same time, adds only the necessary extension-grant CHECK combinations;
see `../storage/state-v35.md`. No old migration is edited and original user data is not upgraded.

Read permissions: `mcp.clients.read` for list/grants, exact `projects.read` intersection for projects.
Writes require `mcp.clients.manage` on both the interactive Client and Extension. Extension grants
use `Global/*` for the existing MCP permissions and `Project/<ProjectId>` for projects.read.
MCP client grants retain the existing lowercase `global/project` kinds. These are distinct records.
Render can only read; action can read/write; background and close cannot invoke this interface.
The Store acceptance transaction revalidates the Core-issued lease, current grants, UI authority
and scoped UI use before inspecting/replaying or accepting writes. The actual Extension Principal
is the idempotency actor. No owner substitution, new persistent query session or Config change;
the only State increment is the separately approved State35 grant CHECK migration above.

Create has no grants. Change-grants merges at most 128 additions and 128 removals atomically, verifies
the final 4096/64-permission bounds and increments one revision. New project grants require registered
projects in the two-sided read intersection. Removing an orphan or inaccessible grant remains possible.
Revoke is permanent. Neither UI close nor protocol cancellation rolls back accepted changes.
Unknown write results do not trigger automatic retries. No credential issuance or secret is exposed
through this interface, Portable UI, data storage or diagnostics. Local-owner CLI issuance uses the
existing one-time-return RPC and explicit rotation/redirect confirmation.

The generated Rust Component needs its guest table and the encoder's canonical-call trampoline
table. The Host permits these two core tables with one shared aggregate 10,000-element budget;
memory (64 MiB), instance count, fuel, wall time and concurrency quotas remain unchanged. Component
preparation does not consume the first render export's two-second budget; the session's original
idle/absolute deadlines still cap the invocation. No WASI or OS capability is added.
