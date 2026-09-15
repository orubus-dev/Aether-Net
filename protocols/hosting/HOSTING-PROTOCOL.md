# AetherNet Hosting Protocol — Phase 10

Phase 10 defines native hosting as a deployment workflow built on Aether storage and naming rather than conventional server sockets.

## Deployment flow

```text
Upload files OR GitHub source
          |
          v
   validate + canonicalize
          |
          v
 deterministic artifact hash
          |
          v
  ContentId / distributed storage
          |
          v
 signed site manifest
          |
          v
 native domain -> resource binding
          |
          v
 replicas + cache + rollback
```

The current deployment primitive accepts uploaded files, canonicalizes the native domain, rejects unsafe paths and oversized files, and deterministically derives an artifact `ContentId`. A previous artifact may be retained as a rollback target.

GitHub source is already represented by `DeploymentSource::GitHub`; actual repository fetching/build execution remains an integration layer so the core protocol does not depend on GitHub availability.

## Security boundaries

Deployment input is bounded and path traversal is rejected. Artifact verification re-hashes the exact input set and refuses a mismatch. Domain normalization uses the native naming layer. Code execution during builds is not performed by the core storage module; a future isolated builder must provide sandboxing, resource limits, dependency controls and provenance/attestation.

## Current status

Implemented: deterministic upload packaging, bounded file validation, artifact identity, verification, domain canonicalization, site manifest generation and rollback metadata.

Still required: native GitHub source import, isolated builds, signed deployment authorization, publish/resolve integration with Phase 7, replica orchestration with Phase 8, live serving over Aether L3/L4, cache invalidation, automatic rollback and multi-node hosting tests.
