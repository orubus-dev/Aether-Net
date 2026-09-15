# AetherNet Discovery & Network Formation Protocol

Phase 6 defines how Aether nodes find one another, establish bounded network membership, disseminate discovery state, and recover from partitions.

## 1. Discovery layers

Discovery has two separate functions:

- **Liveness/bootstrap:** the existing control carrier exchanges HELLO and HELLO_ACK messages so nodes can learn reachable endpoints and maintain transport-level peer liveness.
- **Network membership/state:** signed discovery advertisements are the authoritative mechanism for adding peer, service, and resource records to formation state.

An unsigned HELLO is therefore not sufficient by itself to create authenticated discovery membership.

The current runtime uses the configured UDP endpoint as a bootstrap/control carrier. This is a carrier for formation traffic, not the semantic definition of the Aether L3/L4 network.

## 2. Self-certifying identity

Each signed advertisement carries:

- NodeId
- DeviceId
- Ed25519 public key
- monotonically increasing origin sequence
- issue and expiry timestamps
- bounded peer/service/resource records
- Ed25519 signature over the canonical advertisement bytes

The receiver derives DeviceId from the advertised public key using the device identity domain and derives NodeId from the same public key using the node identity domain. A mismatch rejects the advertisement.

## 3. Advertisement bounds

The wire codec is bounded to 32 peer records, 32 service records, and 32 resource records per advertisement. Advertisement origins are tracked in a bounded replay/sequence table with a default maximum of 16,384 origins.

Records and advertisements have explicit expiries. Expired state is garbage-collected every five seconds by the runtime formation coordinator.

## 4. Monotonic dissemination

Each origin owns a sequence space. A receiver accepts a new advertisement only when its sequence is greater than the last accepted sequence for that origin.

The complete advertisement is signature-verified and its embedded records are validated before the origin sequence is advanced. This avoids advancing replay state for a rejected advertisement.

Accepted advertisements are relayed to a bounded fanout of known peers. The default relay fanout is 32 peers.

## 5. Bootstrap and formation

Nodes may be configured with bootstrap endpoint hints. Bootstrap configuration places the node into `Forming` state. Once a valid signed advertisement supplies a live peer record, the node enters `Connected` state.

The formation states are:

- `Isolated`: no active peers and no active bootstrap formation attempt.
- `Forming`: bootstrap/join activity exists, but no authenticated peer membership is currently active.
- `Connected`: at least one valid peer record is active.
- `Partitioned`: a previously connected formation has lost all live peer membership.

When a connected formation expires its final peer, the runtime marks the formation `Partitioned` and increments the formation generation. A valid newer signed advertisement restores the formation and increments the generation during recovery.

## 6. Service and resource discovery

Service and resource records are propagated inside the same signed origin advertisement. The origin signature authenticates the statement as a discovery claim made by that origin.

Provider-specific authorization remains the responsibility of the service/storage protocols that consume those records; discovery does not replace provider-level capability or content verification.

## 7. Liveness

HELLO messages are emitted every ten seconds. Runtime peer entries become stale after fifteen seconds and expire after thirty seconds. Formation records use their own discovery TTLs and are independently garbage-collected.

A liveness timeout removes the corresponding formation peer record. When the final formation peer disappears, partition handling is triggered automatically.

## 8. Recovery

Recovery does not trust stale cached state. The first valid newer signed advertisement received after a partition re-establishes the active membership view and moves the node back to `Connected`.

This allows a partitioned segment to rejoin another segment after the underlying connectivity path returns, without requiring process restart.

## 9. Security and resource rules

Implementations must:

1. verify advertisement identity bindings before accepting records;
2. verify the Ed25519 signature before applying state;
3. reject expired or malformed timestamps;
4. enforce all record and origin bounds;
5. enforce monotonic origin sequences;
6. never allocate based on untrusted record counts beyond configured protocol caps;
7. avoid unbounded gossip fanout;
8. treat discovery claims as hints until the consuming subsystem performs its own authorization/integrity checks.

## 10. Phase 6 completion boundary

Phase 6 is considered functionally complete when bootstrap discovery, signed peer membership, bounded advertisement propagation, service/resource discovery, liveness expiry, automatic partition detection, and partition recovery operate through the runtime without requiring manual state manipulation.

Production certification still requires long-duration network tests, adversarial fuzzing, clock-skew testing, multi-partition simulation, and independent security review.
