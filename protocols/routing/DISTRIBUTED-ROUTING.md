# AetherNet Distributed Routing

## Purpose

Phase 4 supplies the control plane that converts authenticated peer topology into dynamic native L3 forwarding routes.

## Control flow

1. A node measures direct neighbor links and creates a `RouteAdvertisement`.
2. The advertisement is signed with the node's Ed25519 identity key.
3. The receiving control plane checks that the signing key is the key already bound to that peer, then verifies the signature.
4. Advertisement sequence numbers provide monotonic convergence and reject older state.
5. Advertisement expiry removes stale topology edges.
6. The distributed routing engine scores loop-free paths by traffic priority and selects the highest-quality path.
7. The control plane installs the resulting destination/next-hop pair into the native L3 forwarding table.
8. On expiry or peer loss, `tick()` removes stale state and recomputes routes, allowing automatic path repair when another authenticated topology path remains.

## Security boundary

A signature alone is not a peer identity system. A signed advertisement is accepted only after its public key matches the key registered for the advertised `NodeId`. Peer registration is expected to be populated from the authenticated peer/session establishment layer.

## Routing semantics

`RouteMetrics::score()` is a quality score: larger is better. The active distributed search therefore uses a max-priority search and sorts completed paths in descending score order. L3 receives a deterministic inverse scalar metric because the L3 table uses lower metric as better.

## Bounds

Topology nodes, links, advertisement neighbors, multipath results and search states are bounded. Dynamic L3 routes are also bounded by the control-plane route-table capacity.

## Runtime status

The routing control plane and corrected distributed path engine are implemented in `core/src/route/`. Native L3/L4 packet forwarding already consumes the L3 table. Live transport of routing advertisements across the physical/runtime event loops remains the next integration layer.
