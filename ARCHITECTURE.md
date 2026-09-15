# AetherNet Architecture

## Phase 1 - Core Protocol Foundation

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

## Phase 2 - Native L3 Network Engine

Phase 2 adds the first native network-plane machinery above authenticated Aether links.

### Native forwarding model

```text
Aether source address
        |
        v
+-----------------------+
| Native L3 packet      |
| destination address   |
| flow + sequence       |
| hop limit + QoS       |
+-----------+-----------+
            |
            v
+-----------------------+
| L3 forwarding engine  |
| bounded route table   |
| destination lookup    |
| next-hop selection    |
| hop-limit decrement   |
+-----------+-----------+
            |
       local? / forward
```

`AetherL3Engine` owns native route state and is independent of IP addresses, sockets, TCP, UDP, or any particular physical underlay. Route table capacity is explicitly bounded.

### Packet lifecycle

1. A packet is created with native source/destination addresses.
2. The local L3 engine classifies it as local delivery or forwarding.
3. Forwarding requires a valid route and a hop limit greater than one.
4. Exactly one hop is consumed before forwarding.
5. Expired routes are ignored and can be purged.
6. No-route and expired-hop conditions are explicit forwarding errors.

### Fragmentation / reassembly

Native L3 fragmentation uses the packet flow identifier plus bounded fragment metadata carried inside fragmented payloads. The reassembly table has explicit entry, byte, and timeout limits. Duplicate identical fragments are harmless; conflicting fragments for the same flow/index are rejected.

## Phase 3 - Native L4 Transport Engine

Phase 3 provides transport semantics above native L3 without importing TCP/IP assumptions.

### Transport contract

The native L4 segment carries a flow identifier, sequence number, explicit cumulative acknowledgement number, advertised receive window, transport mode, flags, and bounded payload. This separates data sequencing from acknowledgement state and allows different transport modes to share one envelope.

### Session modes

```text
Datagram       best-effort message delivery
Stream         ordered message sequencing with reliability machinery
ReliableStream ordered reliable delivery with retransmission
Realtime       freshness-first delivery; stale frames are discarded
```

Reliable modes use bounded in-flight state, cumulative acknowledgements, receive-window flow control, retransmission timers, and AIMD congestion control. Realtime mode intentionally avoids reliable retransmission so late media does not consume transport capacity after it becomes obsolete.

## Phase 4 - Distributed Routing Engine

Phase 4 turns local L3 forwarding state into a distributed route-convergence system.

### Topology model

```text
       advertisement
 Node A ----------------> Node B
   |                        |
   | metrics                | metrics
   v                        v
 Node C ----------------> Node D
             alternate path
```

Each routing node maintains a bounded directed topology graph. A `RouteAdvertisement` describes an origin node, a monotonically increasing sequence number, validity interval, and bounded neighbor list. Newer advertisements replace older state for the same origin; stale advertisements are ignored. Expired links are removed by garbage collection.

### Metric composition

Per-link metrics include latency, jitter, packet loss, bandwidth, reliability, monetary cost, energy cost, and hop count. Multi-hop routes combine these with network-safe composition rules:

- latency and jitter add;
- bandwidth follows the bottleneck minimum;
- monetary and energy costs add;
- reliability compounds;
- end-to-end loss compounds;
- hop count adds.

Priority-aware scoring then selects routes according to the traffic class used by the existing Aether route metric system.

### Convergence and failure behavior

Route advertisements are accepted only with valid lifetimes, bounded neighbor counts, valid metrics, and monotonic origin sequence numbers. This prevents old topology information from overwriting newer state and bounds hostile routing input.

### Multipath and loop avoidance

The engine can return a bounded set of alternate paths rather than only one route. Path expansion rejects nodes already present in the candidate path, preventing routing loops during computation. A global search-state budget prevents a large or adversarial topology from causing unbounded route calculation.

### Phase 4 integration gate

Implemented:

- distributed topology database;
- bounded node/link/advertisement state;
- monotonic route-advertisement convergence;
- topology expiration and garbage collection;
- native metric aggregation;
- priority-aware route selection;
- bounded alternate-path discovery;
- loop-free candidate paths;
- bounded routing computation.

Still required before Phase 4 is production-complete:

- authenticate/sign route advertisements using the established peer security layer;
- disseminate advertisements through the live node runtime;
- connect route convergence directly to Phase 2's native L3 forwarding table;
- implement live failure-triggered repair and multi-node integration tests.

## Phase 5 - Physical Underlay Engine

Phase 5 defines the carrier boundary underneath authenticated Aether links. L3 and L4 never address a physical technology directly. Every carrier is represented by the same adapter contract and exposes only the properties the network engine needs.

### Underlay boundary

```text
                    Aether L2
                        |
                +-------+-------+
                | PhysicalEngine |
                +-------+-------+
                        |
       +----------------+----------------+
       |                |                |
    Ethernet          Wi-Fi          Bluetooth LE
       |                |                |
   hardware         hardware         hardware

       Future carriers remain interchangeable:
       Wi-Fi Direct / Wi-Fi Aware / UWB / LoRa /
       cellular / satellite / direct radio / custom links
```

The physical layer is therefore an implementation boundary, not a network-addressing layer. Hardware-specific identifiers, device permissions, socket APIs, radio APIs, and driver details remain below the adapter interface.

### Adapter contract

Every physical adapter exposes:

- its carrier type;
- supported physical capabilities;
- discovery of currently visible/connected physical links;
- normalized underlay properties;
- bounded error reporting.

The normalized `UnderlayProperties` contract includes MTU, estimated bandwidth, latency, energy cost, and broadcast/multicast support. This lets the routing and link layers reason about heterogeneous carriers without knowing their implementation.

### Physical link state

A `PhysicalLink` represents one discovered carrier relationship and reports:

- stable link identifier within the local physical subsystem;
- carrier type;
- estimated latency;
- estimated bandwidth;
- packet loss in parts-per-million;
- signal quality;
- active/inactive state.

Measurements are validated at the physical boundary so malformed or impossible values cannot propagate upward into route selection.

### Engine behavior

`PhysicalEngine` provides a bounded registry for adapters and links. It supports full discovery, per-carrier refresh, deterministic active-link selection, aggregate capability reporting, and MTU-aware frame admission.

Resource limits are explicit:

- maximum adapter count;
- maximum physical link count;
- bounded discovery results;
- no unbounded remote-controlled allocations.

The engine selects links deterministically using loss first, latency second, bandwidth third, and link identifier as a stable tie-breaker. That keeps selection reproducible while allowing later routing phases to apply richer policy.

### Carrier descriptors

Platform-neutral descriptors currently exist for:

- Ethernet;
- Wi-Fi;
- Wi-Fi Direct;
- Bluetooth LE;
- UWB/direct links.

These descriptors intentionally do not pretend to be hardware drivers. They provide the production interface that real OS-specific implementations will satisfy in the platform layer.

### Phase 5 integration gate

Implemented:

- transport-independent physical adapter contract;
- bounded adapter registry;
- bounded link registry;
- normalized underlay properties;
- capability aggregation;
- link discovery and per-carrier refresh;
- deterministic active-link selection;
- MTU-aware frame validation;
- carrier descriptors for Ethernet, Wi-Fi, Wi-Fi Direct, Bluetooth LE, and UWB/direct links;
- physical-layer validation tests.

Still required before Phase 5 is production-complete:

- OS-specific hardware drivers;
- actual L1/L2 transmission through those drivers;
- platform permission and lifecycle handling;
- interoperability tests using real Ethernet, Wi-Fi, Bluetooth, and radio hardware;
- integration of measured physical metrics into the distributed routing engine.

## Later phases

Phase 6 builds global peer/discovery formation and partition recovery.
Phase 7 builds distributed naming and resource resolution.
Phase 8 builds distributed storage/content delivery.
Phase 9 builds distributed compute/service infrastructure.
Phase 10 builds Aether Hosting.
Phase 11 builds Aether Connect communications.
Phase 12 builds optional Internet interoperability gateways.
Phase 13 builds AetherNet OS and Operatives.
Phase 14 performs global-scale hardening, adversarial testing, and production certification.
