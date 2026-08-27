# Aether Peer Establishment Protocol

## State 1 — Discovery

Node A sends:

`HELLO`

Node B validates:

- Aether protocol version
- node identifier
- device identifier
- capability advertisement
- node role

## State 2 — Discovery acknowledgement

Node B responds:

`HELLO_ACK`

This proves that the receiving node can communicate with the advertised
endpoint.

HELLO/HELLO_ACK are discovery messages, not encrypted user payloads.

## State 3 — Authentication challenge

One node generates a fresh random challenge.

The challenge is bound to:

- initiator
- responder
- timestamp

## State 4 — Authentication response

The responder signs a transcript containing:

- responder Node ID
- responder Device ID
- challenge
- ephemeral public key
- timestamp

The signature is checked against the responder's authenticated device public
key.

## State 5 — Key agreement

The peers use ephemeral X25519 material to derive shared session material.

The transcript is included in key derivation so that the negotiated session is
bound to the specific handshake.

## State 6 — Established session

AetherLink provides:

- authenticated encryption
- directional keys
- packet counters
- replay protection
- traffic priority
- rekeying
- keepalive
- close

## State 7 — Rekey

Sessions can transition to a new key epoch without rebuilding the entire peer
relationship.

## State 8 — Failure

Authentication failure, protocol incompatibility, stale sessions or repeated
transport failure must move the peer into a recoverable failure state rather
than leaving undefined session state.

## Security rule

Discovery does not imply trust.

A discovered peer may be reachable without yet being authenticated.

A peer becomes trusted for encrypted AetherLink traffic only after successful
cryptographic authentication.
