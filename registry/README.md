# AetherNet Number Registry

The registry is the second guard behind key-derived numbers. Numbers are
**derived, not drawn**: a device computes `BLAKE2b-256(pubkey || LE64(counter))`
and takes the first seven bytes mod 10 as its free digits behind the tier
prefix (`500` standard, `700` elite, `900` prestige). Two devices can therefore
never "generate the same number twice" in any practical sense — and this
directory makes that provable:

- **Atomic claims.** A claim is a file create through the GitHub Contents API.
  GitHub rejects a create when the path already exists, so a race between two
  devices produces exactly one winner; the loser bumps its counter, re-derives,
  and claims again. This is the `derive_with_retry` loop in `number/src/lib.rs`.
- **Self-proving records.** Every record stores `pubkey` + `counter`, and CI
  (`tools/verify_registry.py`, run on every push) recomputes the number from
  them. A published number that its key does not derive breaks the build.
- **Uniqueness enforced twice.** Once by atomic create, once by the verifier's
  duplicate scan across all tier directories.

## Layout

```
registry/500/<500xxxxxxx>.json   — standard tier number claims
registry/700/<700xxxxxxx>.json   — elite tier number claims
registry/900/<900xxxxxxx>.json   — prestige tier number claims
registry/handles/<handle>.json   — handle claims (genesis@me.ae)
```

## Record shape (schema 1)

```json
{
  "number": "7001234567",
  "tier": "elite",
  "pubkey": "<base64 Ed25519 public key>",
  "counter": 0,
  "handle": "genesis",
  "domain": "genesis.ae",
  "created_at": "2026-10-28T20:30:00Z",
  "schema": 1
}
```

Tier domain rules: standard handles live under `.me.ae`; elite and prestige
handles earn the bare `.ae`. The verifier rejects a record whose domain
disagrees with its tier.

## Claiming from the app

The app's Settings screen accepts a GitHub fine-grained token with
`contents: write` on this repository. On first registration the app:

1. generates the Ed25519 identity keypair on-device,
2. derives the number at counter 0 and displays it immediately (the device is
   on the network from that second, offline or not),
3. when connectivity exists, attempts the atomic claim; on `422 Already
   Exists` it bumps the counter, re-derives, and retries — the registry is
   authoritative, the on-device number never forks from it.

Handles are claimed the same way into `registry/handles/`.
