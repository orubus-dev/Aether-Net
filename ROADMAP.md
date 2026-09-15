# AetherNet Roadmap

## Phase 1 - Core Protocol Foundation [COMPLETE]
- Native Aether L3/L4 boundary, addressing, security profiles, privacy tokens and replay protection.

## Phase 2 - Native L3 Network Engine [IN PROGRESS]
- [x] Native addressing, forwarding, hop limits, bounded fragmentation/reassembly.
- [ ] Runtime-integrated multi-node forwarding.

## Phase 3 - Native L4 Transport Engine [IN PROGRESS]
- [x] Native segments, datagrams, reliable streams, flow control, congestion control, realtime mode and session lifecycle.
- [ ] Live L4 multiplexing over authenticated L3 and multi-node transport tests.

## Phase 4 - Distributed Routing Engine [IMPLEMENTED - INTEGRATION PENDING]
- [x] Bounded topology, monotonic advertisements, metrics, shortest paths, multipath and loop avoidance.
- [ ] Signed advertisements, live dissemination, L3 integration and failure repair.

## Phase 5 - Physical Underlay Engine [IMPLEMENTED - PLATFORM INTEGRATION PENDING]
- [x] Carrier-neutral adapter model, capabilities, bounded registries, discovery, selection and MTU validation.
- [ ] OS drivers, live L1/L2 transmission and hardware interoperability.

## Phase 6 - Network Formation & Discovery [IMPLEMENTED - RUNTIME INTEGRATION PENDING]
- [x] Bootstrap, formation state machine, bounded peer/service/resource discovery, versions, TTLs and partition generations.
- [ ] Live authenticated dissemination, physical/routing integration and partition/heal tests.

## Phase 7 - Naming & Resource Resolution [IMPLEMENTED - DISTRIBUTED INTEGRATION PENDING]
- [x] Canonical names, hashed identifiers, signed records, version convergence, bounded database/cache and TTLs.
- [ ] Distributed dissemination, delegation, native resolver traffic and Phase 6/8 integration.

## Phase 8 - Distributed Storage & CDN [IMPLEMENTED - DISTRIBUTED INTEGRATION PENDING]
- [x] Content-addressed chunks, bounded storage, integrity verification, deduplication, manifests, provider advertisements, provider selection and repair planning.
- [ ] Native storage messages, live transfer/repair, Phase 6/7 integration, multi-node recovery tests and provider identity binding.

## Phase 9 - Distributed Compute & Services [IMPLEMENTED - EXECUTION INTEGRATION PENDING]
- [x] Workload identifiers, bounded compute nodes/workloads/placements, capacity requirements, versioning, deterministic replica placement and controlled migration.
- [ ] Authenticated capacity adverts, sandboxed execution, resource quotas, attestation/provenance, failure rescheduling/state migration and multi-node execution tests.

## Phase 10 - Aether Hosting [IMPLEMENTED - LIVE SERVING INTEGRATION PENDING]
- [x] Upload model, domain canonicalization, deterministic artifact identity, bounded file validation, artifact verification, site manifest/identity and rollback metadata.
- [x] GitHub source descriptor model.
- [ ] Live GitHub import, isolated builds, signed deployment authorization, Phase 7/8 publish integration, native serving, cache invalidation/rollback and multi-node hosting tests.

## Phase 11 - Aether Connect [IMPLEMENTED - LIVE COMMUNICATION INTEGRATION PENDING]
- [x] Native conversation identity, presence model, media references, bounded groups and offline message queue with expiry.
- [ ] End-to-end encryption/session establishment, live L4 messaging, call negotiation, media transport, relay selection and multi-device synchronization.

## Phase 12 - Internet Interoperability [IMPLEMENTED - GATEWAY RUNTIME INTEGRATION PENDING]
- [x] Bounded gateway directory, protocol/direction modelling, TTL expiry and deterministic lowest-cost selection.
- [ ] Live protocol translation, gateway authorization, egress filtering, accounting, abuse controls and privacy policy enforcement.

## Phase 13 - AetherNet OS & Operatives
- Lightweight infrastructure OS, L0-L5 capability operation, provisioning, updates, recovery and monitoring.

## Phase 14 - Global Hardening
- Adversarial testing, large-scale routing/discovery validation, abuse resistance, protocol upgrade and production certification.
