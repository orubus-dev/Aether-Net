# Number

Implemented: `number/src/lib.rs` (crate `aether-number`, in the workspace).

AetherNet numbers are ten-digit identities allocated from tiered ranges:
`500` standard, `700` elite, `900` prestige. Numbers are **derived from the
device's Ed25519 public key** — never drawn from a pool:

```
BLAKE2b-256(pubkey || LE64(counter)) -> seven free digits h[0..7] % 10
number = <tier prefix><7 digits>        e.g. 7001234567
display = 700 1234 567                  (3-4-3 grouping)
```

- deterministic: the same key + counter gives the same number on every
  platform (Rust crate, Kotlin app, and CI verifier implement it identically)
- collision-safe: the atomic GitHub registry claim (`registry/README.md`)
  makes a duplicate allocation impossible; the loser bumps its counter and
  re-derives (`derive_with_retry`)
- self-proving: every registry record stores pubkey+counter and CI recomputes
  the number from them on every push
- tier upgrade = registry-side re-claim; the seven free digits stay identical
  for the same key across tiers

The Android app uses the same derivation on-device, so a phone is on the
network (offline or not) the second it creates its identity.
