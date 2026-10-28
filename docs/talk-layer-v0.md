# Talk Layer v0 — the app's wire protocol

The Aether-Net core (Rust) owns the native L3/L4 stack. While Phase 5/6
hardware drivers and live dissemination are pending, the app ships **talk
layer v0**: a small, fully specified wire protocol over Wi-Fi that works
today, on real phones, and maps 1:1 onto the native stack later (beacons feed
Phase 6 discovery, data packets become L3/L4 payloads, beforenm keys become
link sessions).

## Addressing

A node is its **Ed25519 public key**. Its **AetherNet number** is derived:
`BLAKE2b-256(pubkey || LE64(counter))`, seven free digits `h[0..7] % 10`
behind the tier prefix (`500` standard, `700` elite, `900` prestige) —
implemented identically in `number/src/lib.rs` (Rust), the app (`Identity.kt`),
and `tools/verify_registry.py` (CI).

## Transport

UDP, port **42428**, sent as LAN broadcast (`255.255.255.255`). One socket,
two packet types.

## Beacon — "AEN1" (plaintext, discovery only)

```
offset  size  field
0       4     magic "AEN1" (US-ASCII)
4       32    sender Ed25519 public key
36      1     name length N (max 60)
37      N     display name, UTF-8
```

Every node broadcasts one beacon every ~4 s. A received beacon refreshes the
peer table entry (number → key, address, name, lastSeen). Beacons never carry
payload; they exist so data packets have a route and so the peer table is
self-healing.

## Data — "AEN2" (end-to-end encrypted)

```
offset  size  field
0       4     magic "AEN2" (US-ASCII)
4       32    sender Ed25519 public key
36      24    nonce (XChaCha20-Poly1305-IETF)
60      n     ciphertext = plaintext + 16-byte Poly1305 tag
```

- key     = `crypto_box_beforenm(senderCurvePk, recipientCurveSk)` — the
  standard libsodium X25519+HSalsa20 shared secret; both sides derive the same
  32 bytes, and curve keys are converted from the Ed25519 identity.
- AD (additional data) = `"AEN2" || senderPk || recipientPk` — binds the packet
  to both identities; a relayed/spoofed swap breaks the tag.
- plaintext = UTF-8 JSON: `{"id":"<16 hex>","ts":<ms>,"text":"..."}`

## Store-and-forward

When a data packet has no beacon for its destination, the sender parks the
message as PENDING in the spool. On the destination's next beacon, the relay
service redelivers every parked message. So:

- two nodes on the same LAN: instant, encrypted delivery
- nodes that meet later: delivery on sighting — the gap in coverage delays a
  message, it never silently drops it

This is hop 1 of the mesh. Hops 2..n arrive with the native runtime: the same
packet formats ride multi-node L3 forwarding with per-hop re-encryption, as
already conformance-tested in the core.

## What v0 is honest about

- No offline number→key resolution yet: you can only send to a peer whose
  beacon you have seen. Number→key lookup lands with the registry + name
  resolution integration.
- No relay across subnets yet: v0 speaks within one LAN/broadcast domain.
  Wi-Fi-Direct groups and BLE bridges are the Phase 5 driver work.
- Identity seed is stored app-private, not yet wrapped in an Android Keystore
  key (v0.2 item).
