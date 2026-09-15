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

Phase 2 adds the native forwarding and packet-lifecycle machinery above authenticated Aether links.

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

### Current Phase 2 status

Implemented:

- native 256-bit addressing;
- native L3 packet encoding and decoding;
- bounded route state;
- destination lookup and next-hop selection;
- hop-limit enforcement;
- native fragmentation;
- bounded reassembly;
- conflict/duplicate fragment handling;
- conformance tests for forwarding and reassembly.

Still required before Phase 2 can be marked complete:

- connect L3 forwarding to the authenticated runtime data path;
- exercise forwarding across multiple live Aether nodes;
- add integration tests for multi-hop delivery under real underlays.

## Phase 3 - Native L4 Transport Engine

Phase 3 implements transport semantics above native L3. L4 never requires an IP address, TCP connection, UDP socket, or a specific physical underlay.

### Wire envelope

The L4 segment contains:

```text
magic | version | mode | flags | reserved
flow_id
sequence
acknowledgement
receive_window
payload_length
payload
```

The cumulative acknowledgement identifies the next expected sequence number at the receiver. The receive window advertises bounded receive capacity so a sender can respect both congestion and peer buffering limits.

### Transport modes

`Datagram` provides message delivery without retransmission or ordering guarantees. `Stream` and `ReliableStream` provide ordered delivery with bounded buffering, cumulative acknowledgements, retransmission and congestion control. `Realtime` prefers freshness over delivery guarantees: stale sequence numbers are dropped instead of being retransmitted.

### Reliable session lifecycle

```text
             +-----------+
             |   CLOSED  |
             +-----+-----+
                   |
                 SYN
                   v
             +-----------+
             | SYN-SENT  |
             +-----+-----+
                   |
                  ACK
                   v
             +-----------+
             |ESTABLISHED|
             +--+-----+--+
                |     |
               FIN   RESET
                |     |
                v     v
          +---------+ +-------+
          | FIN-SENT| | RESET|
          +----+----+ +-------+
               |
              ACK
               v
          +---------+
          | CLOSED  |
          +---------+
```

### Reliability and flow control

Reliable sessions retain a bounded set of outstanding segments. A receiver buffers a bounded number of out-of-order segments. Duplicate identical sequence data is harmless; conflicting data for an already-buffered sequence is rejected. Cumulative acknowledgements release outstanding data.

### Congestion control

The sender maintains an explicit congestion window and peer receive window. New data is admitted only when both limits permit it. ACKs increase the congestion window using additive growth, while retransmission loss applies multiplicative reduction. Retransmission timeout is bounded between configured minimum and maximum values.

This is deliberately implemented as an Aether-native transport policy rather than inheriting TCP state or algorithms from the traditional Internet stack.

### Realtime behavior

Realtime flows such as voice, live video, control input and interactive media should not stall behind lost packets. The current L4 engine tracks the newest sequence received and discards stale arrivals rather than scheduling recovery traffic.

### Current Phase 3 status

Implemented:

- native L4 segment encoding/decoding;
- explicit sequence and cumulative acknowledgement fields;
- receive-window advertisement;
- datagram mode;
- ordered stream mode;
- reliable stream mode;
- bounded receive buffers;
- bounded outstanding-send state;
- retransmission timers;
- AIMD congestion response;
- FIN/RESET lifecycle controls;
- flow validation;
- realtime stale-packet suppression;
- unit/conformance tests for the transport state machine.

Still required before Phase 3 can be marked complete:

- integrate session management with the native L3 runtime data path;
- multiplex independent L4 flows through the node runtime;
- add live multi-node transport interoperability tests;
- validate performance and loss behavior on real heterogeneous underlays.

## Later phases

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
