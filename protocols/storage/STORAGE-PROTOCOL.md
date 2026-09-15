# AetherNet Storage Protocol — Phase 8

## Scope

Phase 8 defines native distributed resource storage above Aether L3/L4. It is content-addressed and provider-independent: a resource is identified by its native `ContentId`, while providers advertise where replicas can be retrieved.

## Storage model

```text
ContentId
   |
   v
Versioned ContentManifest
   |
   +--> Chunk 0 --> ChunkDigest
   +--> Chunk 1 --> ChunkDigest
   +--> ...
   |
   v
Provider advertisements
   |
   +--> Provider A
   +--> Provider B
   +--> Provider C
```

A node may act as an origin, cache, replica provider, or temporary relay. Storage capacity is explicitly bounded so remote peers cannot force unbounded allocation.

## Chunk integrity

Chunks are individually hashed with the domain `aether:chunk:v1:` and include the parent `ContentId`, logical `ChunkId`, and chunk bytes in the digest input. A receiving node verifies the digest before exposing the chunk to callers.

Chunk insertion is deduplicating for the same content/chunk identity. Expired chunks are removed by bounded garbage collection.

## Manifest convergence

A `ContentManifest` describes the ordered chunks and total content size. The Phase 8 `ManifestStore` adds a monotonically increasing version and a validity interval. For a given content identifier, an older manifest cannot replace a newer manifest.

## Provider advertisements

A provider advertisement identifies a content object, provider node, capacity, cached bytes, retrieval cost, reliability, version, and validity interval. The advertisement is signed with Ed25519 and is rejected when its lifetime or resource metrics are invalid.

Provider records converge monotonically by `(content, provider, version)` and expire automatically. Retrieval selection ranks currently valid providers using available capacity, reliability, and cost.

## Replication and repair

The storage controller can build a bounded `RepairPlan` from the currently advertised provider set. The plan records the requested replica count, selected provider nodes, and whether additional repair is required.

The present Phase 8 implementation deliberately stops before automatically moving bytes between live nodes. Automatic transfer, quorum reads/writes, failure-triggered repair, and network request messages belong to the distributed integration gate.

## CDN behavior

The bounded chunk store functions as the native content cache: repeated retrievals are served from verified local chunks, content is deduplicated by native identifiers, and TTL expiry bounds retained data. The provider directory supplies the control-plane information needed for a future distributed CDN selection layer.

## Security boundary

Phase 8 verifies content/chunk integrity locally and verifies provider advertisement signatures. It does not yet claim complete provider identity binding, authorization delegation, encrypted-at-rest storage, Byzantine replica consensus, or live authenticated transport integration. Those are explicit hardening/integration requirements rather than implicit assumptions.

## Implementation status

Implemented in `core/src/storage/distributed.rs`:

- bounded chunk storage;
- chunk digests and read-time integrity validation;
- deduplicating replacement;
- expiry/garbage collection;
- bounded versioned manifest storage;
- signed provider advertisements;
- provider validation and selection;
- replica repair planning.

Still required for Phase 8 completion:

1. Native storage request/response messages over Aether L3/L4.
2. Live replica transfer between authenticated Aether peers.
3. Automatic repair after node/provider loss.
4. Integration with Phase 6 resource discovery.
5. Integration with Phase 7 native naming resolution.
6. Multi-node storage failure/recovery tests.
7. Strong binding between the advertised provider node identity and the signing key.
