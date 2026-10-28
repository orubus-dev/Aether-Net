# Aether

A decentralized carrier network where people's devices and independently operated infrastructure nodes form the network itself.

## Architecture

- L0 — Aether Physical
- L1 — Aether Mesh
- L2 — AetherLink
- L3 — Aether Network
- L4 — AetherRoute
- L5 — Aether Services

## Philosophy

Take What Exists. Make It Better.

Aether uses proven networking, cryptographic, distributed-system, operating-system,
and Internet technologies as foundations rather than reinventing mature primitives.

## Core Principles

- Encryption by default
- Identity without unnecessary exposure
- User-controlled devices
- Consent-based participation
- Distributed infrastructure
- Efficient routing
- Native communication without carrier dependency
- Paid Internet access where upstream costs exist
- Open APIs and SDKs
- Modular architecture
- Signed updates
- Automatic health checks and rollback
- Minimal infrastructure overhead
- Developer-first ecosystem

## Implemented milestones

Phase 1 through Phase 5 establish the native security, L3, L4, routing, and physical
underlay foundations.

Phase 6 adds runtime network formation: bounded peer/service/resource discovery,
formation state transitions, generation tracking, expiry, partition recovery, and
integration with the node event loop.

The `aetherd` host daemon is now part of the workspace. It provides a service-oriented
host boundary with persistent state/config/runtime/log directories, restart policy
tracking, clean shutdown signaling, and a Linux systemd unit. Windows host integration
uses the same platform boundary and is the next platform-certification track.

## The app

`app-android/` — the AetherNet client (Android, Liquid Glass UI, icy-blue accent):

- **Identity**: Ed25519 keypair generated on-device; the tiered number
  (500/700/900) is *derived from the key*, not allocated —
  `number/src/lib.rs`, the app, and the registry verifier implement the same
  derivation byte-for-byte.
- **Registry**: atomic GitHub claims (`registry/`) — two devices can never
  hold the same number; on the astronomically rare collision the app bumps
  its counter and re-derives. CI re-proves every published number from its
  key on every push.
- **Talk layer v0** (`docs/talk-layer-v0.md`): encrypted LAN beacons +
  XChaCha20-Poly1305-IETF direct messages with store-and-forward spooling.
- **Relay**: a foreground service keeps the phone acting as a node — your
  phone is the tower.

Windows and Linux clients follow the same identity/number/registry contracts.

## Build the app

CI does it on every push to `app-android/**` (`.github/workflows/android.yml`),
signs with the release keystore, and publishes the APK to Releases:

```bash
gradle -p app-android assembleRelease
```

## Production boundary

The network protocol core is intentionally separated from OS-specific mechanisms.
Linux currently provides the reference raw-packet and interface substrate; Windows,
macOS and other platforms require native adapters and validation before production
certification. Security review, interoperability testing, crash recovery testing,
upgrade/rollback testing and real-hardware soak tests are still required.
