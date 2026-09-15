# AetherNet Roadmap

## Phase 1 - Core Protocol Foundation [COMPLETE]
- Native Aether L3/L4 boundary, addressing, secure-by-default profiles, privacy tokens and replay protection.

## Phase 2 - Native L3 Network Engine [COMPLETE - RUNTIME DATAPATH INTEGRATED]
- [x] Native addressing, forwarding, hop limits and bounded fragmentation/reassembly.
- [x] Authenticated link-session ingress decrypts native L3 packets.
- [x] L3 performs destination classification and next-hop selection in the runtime stack.
- [x] Forwarded packets consume exactly one hop and are re-encrypted for the next authenticated peer.
- [x] Direct next-hop routes are supported.
- [x] Two-hop authenticated multi-node forwarding conformance test.

## Phase 3 - Native L4 Transport Engine [COMPLETE - RUNTIME MULTIPLEXING INTEGRATED]
- [x] Native segments, datagrams, reliable streams, flow control, congestion control, realtime mode and lifecycle.
- [x] Flow identifiers multiplex L4 sessions inside native L3 packets.
- [x] Locally delivered L4 payloads are demultiplexed into bounded session state.
- [x] Reliable ACKs are emitted back through native L3 routing and authenticated link encryption.
- [x] End-to-end L4 multi-node conformance test.

## Phase 4 - Distributed Routing Engine [IMPLEMENTED - INTEGRATION PENDING]
- [x] Bounded topology, monotonic advertisements, metrics, shortest paths, multipath and loop avoidance.
- [ ] Signed advertisements, live dissemination, automatic L3 route-table synchronization and failure repair.

## Phase 5 - Physical Underlay Engine [IMPLEMENTED - PLATFORM INTEGRATION PENDING]
- [x] Carrier-neutral adapter model, capabilities, bounded registries, discovery, selection and MTU validation.
- [ ] OS drivers, live L1/L2 transmission, hardware interoperability and physical metrics feeding routing.

## Phase 6 - Network Formation & Discovery [IMPLEMENTED - RUNTIME INTEGRATION PENDING]
- [x] Bootstrap, formation state machine, bounded peer/service/resource discovery, versions, TTLs and partition generations.
- [ ] Live authenticated dissemination, physical/routing integration and partition/heal tests.

## Phase 7 - Naming & Resource Resolution [IMPLEMENTED - DISTRIBUTED INTEGRATION PENDING]
- [x] Canonical names, hashed identifiers, signed records, version convergence, bounded database/cache and TTLs.
- [ ] Distributed dissemination, delegation, native resolver traffic and Phase 6/8 integration.

## Phase 8 - Distributed Storage & CDN [IMPLEMENTED - DISTRIBUTED INTEGRATION PENDING]
- [x] Content-addressed chunks, bounded storage, integrity verification, deduplication, manifests, provider advertisements, provider selection and repair planning.
- [ ] Native storage messages, live packetized transfer/automatic repair, Phase 6/7 integration, multi-node recovery tests and provider identity binding.

## Phase 9 - Distributed Compute & Services [IMPLEMENTED - EXECUTION INTEGRATION PENDING]
- [x] Workload identifiers, bounded compute nodes/workloads/placements, capacity requirements, versioning, deterministic placement and controlled migration.
- [ ] Authenticated capacity advertisements, sandboxed execution, resource quotas, attestation/provenance, failure rescheduling/state migration and multi-node execution tests.

## Phase 10 - Aether Hosting [IMPLEMENTED - LIVE SERVING INTEGRATION PENDING]
- [x] Upload model, domain canonicalization, deterministic artifact identity, bounded file validation, artifact verification, site identity and rollback metadata.
- [x] GitHub source descriptor model.
- [ ] Live GitHub import, isolated builds, signed deployment authorization, Phase 7/8 publish integration, native serving, cache invalidation/rollback and multi-node hosting tests.

## Phase 11 - Aether Connect [IMPLEMENTED - LIVE COMMUNICATION INTEGRATION PENDING]
- [x] Native conversation identity, presence model, media references, bounded groups and offline message queue with expiry.
- [ ] End-to-end encryption/session establishment, live L4 messaging, call negotiation, media transport, relay selection and multi-device synchronization.

## Phase 12 - Internet Interoperability [IMPLEMENTED - GATEWAY RUNTIME INTEGRATION PENDING]
- [x] Bounded gateway directory, protocol/direction modelling, TTL expiry and deterministic lowest-cost selection.
- [ ] Live protocol translation, authorization, egress filtering, accounting, abuse controls and privacy policy enforcement.

## Phase 13 - AetherNet OS & Operatives [IMPLEMENTED - PLATFORM INTEGRATION PENDING]
- [x] L0-L5 capability model and node resource profile.
- [x] Operative upload/download bandwidth governors with asymmetric limits.
- [x] Packet-size, transfer-size and concurrent-transfer bounds.
- [x] Transfer lifecycle and native data-packet scheduling primitives.
- [x] OS boot/health/degraded/recovery lifecycle primitives.
- [ ] Persistent disk management and native storage service.
- [ ] Real hardware drivers and physical adapter integration.
- [ ] Signed OS update chain and secure bootstrapping.
- [ ] Full service/process supervision and provisioning.
- [ ] Device telemetry and production monitoring.

## Phase 14 - Global Hardening
- Large-scale simulation, adversarial routing/storage/compute testing, abuse resistance, protocol upgrades, security auditing and production certification.
