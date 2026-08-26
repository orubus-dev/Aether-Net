# Aether Core — M0

M0 establishes the complete Core protocol foundation.

## L0 — Aether Physical

Provides the platform-neutral physical transport abstraction:

- Ethernet
- Wi-Fi
- Wi-Fi Direct
- Wi-Fi Aware
- Bluetooth LE
- UWB
- LoRa
- Cellular
- Satellite

Hardware-specific adapters are provided by platform layers.

## L1 — Aether Mesh

Provides:

- neighbors
- link metrics
- topology
- relay eligibility
- battery policy
- thermal policy
- mesh power policy

Relay participation stops at the configured battery threshold unless
charging or an explicit emergency policy permits participation.

## L2 — AetherLink

Provides:

- authenticated peer sessions
- X25519 key agreement
- Ed25519 identity authentication
- HKDF-SHA-256 session derivation
- ChaCha20-Poly1305 authenticated encryption
- directional keys
- replay protection
- counters
- session epochs
- rekeying
- session expiration
- keepalive/control messages
- graceful close

## L3 — Aether Network

Provides:

- node identifiers
- node registry
- peer protocol
- protocol versions
- packet format
- service advertisements
- service discovery

## L4 — AetherRoute

Provides:

- route metrics
- latency
- jitter
- packet loss
- reliability
- bandwidth
- hop cost
- energy cost
- route ranking
- gateway representation

## L5 — Aether Services

Provides the runtime service abstraction on which:

- Aether Connect
- Aether Line
- Aether Mail
- Aether OAuth
- Aether Name
- Aether Number
- CDN
- storage
- games
- third-party applications

will operate.

## Identity

Aether identity is separated into:

- Aether ID — cryptographic identity
- Aether Name — human-readable identity
- Aether Number — communication address
- Device ID — cryptographic endpoint identifier

Private identity keys never belong in Aether network packets.

## M0 completion criteria

M0 is complete only when:

1. all Core modules compile
2. central tests pass
3. Clippy passes with warnings denied
4. secure identity storage round-trips
5. device lifecycle works
6. recovery signatures verify
7. AetherLink encrypts/decrypts authenticated frames
8. replayed frames are rejected
9. rekeying succeeds
10. L0-L5 interfaces integrate with Core
11. Core lifecycle initializes, starts and stops
12. all production files contain actual implementations
