# AetherNet Compute Protocol — Phase 9

Phase 9 defines the native distributed compute control plane above Aether L3/L4. It places bounded workloads on nodes according to declared capacity, reliability and cost, with replica count and migration policy expressed by the workload itself.

## Model

```text
WorkloadSpec
   |
   v
DistributedCompute
   |
   +--> capacity discovery
   +--> placement scoring
   +--> replica placement
   +--> migration
   +--> expiry / cleanup
```

A compute node advertises CPU, memory, storage and bandwidth capacity plus reliability and cost. A workload declares its required resources, service identity, desired replicas and whether migration is permitted.

## Safety boundaries

All node, workload and placement collections have explicit bounds. Workloads and node advertisements expire. Placement never occurs unless a candidate reports sufficient available capacity. The scheduler does not execute arbitrary code itself; execution isolation, sandboxing, quotas and attestation remain separate runtime concerns.

## Current status

Implemented: bounded node/workload registries, deterministic capacity-aware placement, replica selection, migration checks, monotonic workload versions, and expiry cleanup.

Still required: live workload transport, authenticated capacity advertisements, sandbox execution, quota enforcement, node attestation, failure-triggered rescheduling, state migration, and multi-node compute integration tests.
