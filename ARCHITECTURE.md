# AetherNet Architecture

## Phase 1 — Core Protocol Foundation

Phase 1 freezes the invariants that every later AetherNet engine depends on.

### Network identity

AetherNet uses cryptographic identities independently from physical network locations. A native L3 address is a fixed-size 256-bit value derived from identity material and is not an IP address.

### Native layers

```text
L1  Physical underlays
L2  Aether link / authenticated peer sessions
L3  Aether network / native addressing + forwarding
L4  Aether transport / streams, datagrams, realtime sessions
L5  Aether services / names, resources, applications
```

TCP/IP is not part of the Aether-native L3/L4 contract. Conventional Internet access is an optional gateway capability.

### Security invariants

1. Aether data is encrypted by default.
2. Peer authentication is required for trusted sessions.
3. Plaintext data-plane operation is disabled by default and is not an accepted production profile.
4. Packet replay is rejected with a bounded sliding sequence window.
5. Stable identity is not required to appear in ordinary data-plane traffic.
6. Privacy material is short-lived, scoped by protocol purpose, and rotatable.
7. Secrets are cleared when owned privacy contexts are dropped.
8. Protocol version compatibility is explicit.

### Privacy / anonymity direction

AetherNet separates stable cryptographic identity from ephemeral network-visible privacy tokens. This prevents the native data plane from requiring a permanent identifier on every packet. This is a privacy primitive, not a claim of perfect network anonymity: traffic analysis, physical observation, compromised relays, and application-level identifiers remain separate threat models and are addressed by later phases.

### Performance invariants

- Native packet structures use fixed-size identifiers where practical.
- Replay protection is allocation-free.
- L3/L4 are transport-independent so physical underlays can change without rewriting applications.
- Hot paths must not depend on centralized services or blocking global locks.
- Resource limits are explicit and bounded to prevent untrusted peers from forcing unbounded allocation.

### Phase 1 acceptance gate

Phase 1 is architecturally complete only when all of the following remain true:

- native L3 address type exists and is stable across underlay changes;
- native L3 packet encoding/decoding is deterministic and bounded;
- native L4 envelope is independent of IP/TCP semantics;
- secure-by-default policy is represented in runtime configuration;
- rotating privacy context and opaque tokens are available;
- replay window rejects duplicates and stale packets;
- protocol versioning and packet limits are explicit;
- unit/conformance tests cover the security primitives;
- later engines can depend on these interfaces without importing traditional Internet semantics.

## Later phases

Phase 2 builds the complete native L3 forwarding engine.
Phase 3 builds the complete native L4 transport engine.
Phase 4 builds distributed routing and route convergence.
Phase 5 expands physical underlays.
Phase 6 builds global peer/discovery formation.
Phase 7 builds distributed naming and resource resolution.
Phase 8 builds distributed storage/content delivery.
Phase 9 builds distributed compute/service infrastructure.
Phase 10 builds Aether Hosting.
Phase 11 builds Aether Connect communications.
Phase 12 builds optional Internet interoperability gateways.
Phase 13 builds AetherNet OS and Operatives.
Phase 14 performs global-scale hardening, adversarial testing, and production certification.
