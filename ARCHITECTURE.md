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
```

Hardware-specific identifiers, permissions, socket APIs, radio APIs, and driver details remain below the adapter interface.

### Phase 5 integration gate

Implemented:

- transport-independent physical adapter contract;
- bounded adapter and link registries;
- normalized underlay properties;
- capability aggregation;
- link discovery and per-carrier refresh;
- deterministic active-link selection;
- MTU-aware frame validation;
- platform-neutral carrier descriptors.

Still required before Phase 5 is production-complete:

- OS-specific hardware drivers;
- actual L1/L2 transmission through those drivers;
- platform permission and lifecycle handling;
- hardware interoperability tests;
- physical metrics integrated into distributed routing.

## Phase 6 - Network Formation & Discovery

Phase 6 turns isolated Aether nodes into a self-forming network. It sits between physical discovery and distributed routing/service layers: physical adapters reveal reachable carrier relationships, the formation engine tracks peers and advertised resources, and the routing engine later consumes that topology.

### Formation state machine

```text
             bootstrap / discovery
                  +---------+
                  | Isolated |
                  +----+----+
                       |
                       v
                  +----+----+
                  | Forming  |
                  +----+----+
                       |
                 peer observed
                       |
                       v
                  +----+----+
                  |Connected |
                  +----+----+
                       |
                 partition/failure
                       |
                       v
                  +----+------+
                  |Partitioned|
                  +-----------+
                       |
                  recovery
                       |
                       +----> Forming / Connected
```

`NetworkFormationEngine` owns bounded bootstrap peers, live peer records, service records, and resource records. Each record has an explicit validity interval so stale network state cannot persist forever.

### Formation and discovery controls

Bootstrap peers are optional starting points. Peer, service, and resource records use bounded storage, explicit TTLs, version monotonicity, and partition generations. This makes the discovery plane resilient while limiting hostile input.

### Phase 6 integration gate

Implemented:

- bounded bootstrap-peer configuration;
- formation state machine for isolated/forming/connected/partitioned operation;
- bounded peer discovery state;
- peer record expiration;
- service discovery records with monotonic version replacement;
- resource discovery records with monotonic version replacement;
- partition generation tracking and recovery transitions;
- explicit discovery memory limits;
- discovery conformance tests.

Still required before Phase 6 is production-complete:

- disseminate discovery records through authenticated Aether peers;
- connect physical adapter discovery to the formation engine;
- feed formation/topology state into Phase 4 distributed routing;
- cryptographically authenticate discovery records;
- multi-node partition/heal integration tests.

## Phase 7 - Naming & Resource Resolution

Phase 7 introduces the native naming system above discovery. Aether names resolve directly to Aether resource identifiers rather than requiring a DNS name to become an IP address.

### Name model

A name is normalized into a canonical lowercase representation and then mapped to a fixed-size `NameHash` using domain-separated SHA-256. Names are hierarchical by dot-separated labels, with bounded total and per-label lengths.

```text
school.aeether
      |
      v
canonical name
      |
      v
NameHash (256-bit)
      |
      v
signed NameRecord
      |
      v
ResourceId (256-bit)
```

The name itself is not the location of a server. The resolved `ResourceId` identifies the native resource; later storage/provider phases determine where replicas of that resource can be obtained.

### Signed records

A `NameRecord` contains the name hash, resource identifier, owner's Ed25519 public key, monotonic version, issuance/expiry times, and a 64-byte signature. The signature covers all fields that define the binding. A resolver verifies the signature and validity interval before accepting a record.

### Version convergence

Records are replaced only by an equal-or-newer version for the same name hash. Older records cannot overwrite newer state. This gives distributed caches a deterministic convergence rule.

### Local resolver and cache

`NamingEngine` provides a bounded authoritative/cache store for native name records. Resolution canonicalizes the requested name, checks the bounded cache against the current record version and expiry, then verifies the stored record before returning its resource identifier. Expired records and cache entries are removable through bounded garbage collection.

### Security boundary

Name records are cryptographically authenticated before publication or resolution. Naming therefore does not trust a central resolver or an unverified network response. Ownership/delegation policy is intentionally separate from the record primitive and is a later integration concern.

### Phase 7 integration gate

Implemented:

- canonical name validation and normalization;
- fixed-size name hashing;
- signed name-to-resource records;
- Ed25519 signature verification;
- monotonic version replacement;
- bounded naming database;
- bounded resolver cache;
- TTL/expiry validation and cleanup;
- native name/resource identifiers;
- naming conformance tests.

Still required before Phase 7 is production-complete:

- distributed record dissemination across live Aether nodes;
- delegation and authority hierarchy;
- secure ownership transfer/revocation protocol;
- native resolver messages over Aether L3/L4;
- integration with Phase 6 discovery and Phase 8 distributed storage/provider selection.

## Later phases

Phase 8 builds distributed storage/content delivery.
Phase 9 builds distributed compute/service infrastructure.
Phase 10 builds Aether Hosting.
Phase 11 builds Aether Connect communications.
Phase 12 builds optional Internet interoperability gateways.
Phase 13 builds AetherNet OS and Operatives.
Phase 14 performs global-scale hardening, adversarial testing, and production certification.
