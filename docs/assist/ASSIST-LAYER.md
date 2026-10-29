# The AetherNet Assist Layer

**Serverless. No backend. No keys on any server.**

The assist layer is AetherNet's internet-assisted fallback and convenience
plane. The mesh itself (L0–L5) works device-to-device with no infrastructure
at all; the assist layer exists for the moments the mesh can't reach — the
person you want is kilometers away with no relay chain between you — and for
convenience paths like claiming a number from a fresh install without a
personal GitHub token.

## The three planes

```text
┌─────────────────────────────┬───────────────────────────────────────────┐
│ Your device                 │ holds the keys. Derives the number.       │
│ (Android / web / desktop)   │ Encrypts everything before it leaves.     │
├─────────────────────────────┼───────────────────────────────────────────┤
│ GitHub (the "VPS of sorts") │ storage of truth:                         │
│ orubus-dev/Aether-Net       │   registry/<tier>/<number>.json  identity │
│                             │   registry/handles/<name>.json   names    │
│                             │   registry/sites/<handle>/       sites    │
│                             │   mail/<box>/<exp>-<id>.json     temp mail │
├─────────────────────────────┼───────────────────────────────────────────┤
│ Assist endpoint             │ stateless pipes only:                     │
│ aether-net.vercel.app       │ validate + route + forward ciphertext.    │
│ (or your own sandbox)       │ No database. No state. No secrets of      │
│                             │ yours — just a GitHub token for writes.   │
└─────────────────────────────┴───────────────────────────────────────────┘
```

## What runs where

| Endpoint | Function | Reads | Writes |
|----------|----------|-------|--------|
| `GET /api/health` | liveness + capabilities | — | — |
| `GET /api/resolve?name=` | number / handle / .ae domain → record | raw GitHub, no token | — |
| `POST /api/claim` | atomic registry claim (derivation re-checked server-side, create-only) | registry | registry |
| `POST /api/mail` | drop encrypted envelope (box = recipient number) | — | `mail/` |
| `GET /api/mail?box=` | list live envelopes | `mail/` | sweeps expired |
| `GET /api/mail?box=&id=` | pickup: fetch + delete | `mail/` | deletes |
| `POST /api/signal` | call signaling envelopes (ring/accept/decline), short TTL | — | `mail/` |
| `GET /api/sites?name=&path=` | serve .ae site packages to any browser | raw GitHub | — |
| `POST /api/sites` | publish a site as ONE atomic git commit | handles + sites | sites |
| `POST /api/sweep` | delete expired envelopes (manual; no cron anywhere) | `mail/` | `mail/` |

## Encryption contract

Everything in `mail/` is opaque ciphertext:

```text
envelope v1 = [1][ephemeral X25519 pk 32][nonce 24][XChaCha20-Poly1305 ct]
shared      = crypto_box_beforenm semantics
              = HSalsa20(X25519(ephSk, recipPk), zero16, sigma)
key         = that 32-byte combiner output
aad         = ephPk || recipPk     (identity-bound, tamper-evident)
```

This is the same primitive pair libsodium ships (`crypto_box_beforenm` +
`crypto_aead_xchacha20poly1305_ietf`), so:

- the Android app unseals with lazysodium — no custom crypto,
- the web client implements HSalsa20 exactly (proven in
  `tests/web_crypto_e2e.mjs`, where a JS-sealed envelope is decrypted by
  raw libsodium in Python),
- the endpoint physically cannot read mail — it holds no keys and the
  combiner output never leaves the two ends.

## Number uniqueness (no double allocation, ever)

1. The number is **derived**: `BLAKE2b-256(pubkey || LE64(counter))`,
   first 7 bytes mod 10, behind the tier prefix — byte-identical in Rust,
   Kotlin, JS and Python.
2. The claim is an **atomic file create** on GitHub; a racing claim for the
   same path loses with 422 → the client bumps its counter, re-derives,
   retries.
3. **CI re-proves every record** on every push (`tools/verify_registry.py`):
   derivation, tier/prefix match, uniqueness, handle rules.

## Own sandbox as VPS

Any machine can become the assist endpoint — same code, same contract:

```bash
GITHUB_TOKEN=github_pat_… GITHUB_REPO=orubus-dev/Aether-Net \
  node api/sandbox-server.js 8787
# → point the app's Settings → Assist endpoint at http://your-box:8787
```

Nothing about AetherNet depends on Vercel. Vercel is just where the public
default lives, because it is free, stateless and close to users.

## Guarantees and limits (honest)

- The endpoint sees **metadata**: who dropped what, when, how big, to which
  box. Encrypted-metadata padding is future work (M3).
- GitHub is a slow mailbox (~seconds of latency), not a live channel — live
  audio/video belongs on mesh L4 sessions; the assist path carries signaling
  and short voice notes until a direct route exists.
- Mail envelopes expire (≤7 days, default 3) and are swept on read; boxes
  cap at 48 live envelopes.
