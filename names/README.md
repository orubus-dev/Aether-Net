# Names

Implemented: handle claims in the registry (`registry/handles/<handle>.json`),
schema in `registry/README.md`; resolution primitives in the core
(`naming.rs`, Phase 7 — canonical names, signed records, TTLs).

## Handles

A handle is `[a-z0-9]([a-z0-9-]{1,30})[a-z0-9]` and is claimed once, atomically,
on the GitHub registry, bound to a claimed number. The address form is
`<handle>@<tier domain>`:

| Tier | Prefix | Address | Site domain |
|---|---|---|---|
| Standard | 500 | `genesis@me.ae` | `genesis.me.ae` |
| Elite | 700 | `genesis@ae` | `genesis.ae` |
| Prestige | 900 | `genesis@ae` | `genesis.ae` (+ relay priority) |

## Resolution

v0 resolves handles through the registry (GitHub raw/file fetch), cached
client-side with the Phase 7 TTL semantics. Native distributed resolution
(signed record dissemination through the mesh) is the Phase 7 "distributed
integration" track already implemented behind the runtime boundary.
