# AetherNet Storage Protocol — Phase 8

## Scope

Phase 8 defines native distributed resource storage and CDN behavior above the existing Aether L3/L4 datapath. Objects are content-addressed, chunks are individually integrity-protected, and providers advertise where replicas can be retrieved.

The implementation is intentionally provider-independent: an origin device, cache, replica node, relay, or dedicated infrastructure node can participate without the storage layer being tied to a particular cloud provider.

## Storage model

```text
ContentId
   |
   v
Versioned ObjectManifest
   |
   +--> Chunk 0 --> ChunkDigest
   +--> Chunk 1 --> ChunkDigest
   +--> ...
   |
   v
Signed provider advertisements
   |
   +--> Provider A
   +--> Provider B
   +--> Provider C
```

A node can originate, cache, replicate, relay, or serve an object. Capacity is explicitly bounded so remote peers cannot force unbounded memory allocation.

## Content addressing and chunk integrity

`ContentId` is a SHA-256 identifier of the complete object. Each stored chunk has a domain-separated `ChunkDigest` containing the parent `ContentId`, logical `ChunkId`, and chunk bytes.

The object store verifies chunk digests on insertion and retrieval, verifies the assembled object against its `ContentId`, deduplicates `(content, chunk)` identities, and removes expired data through garbage collection.

Objects are represented by ordered manifests containing total size, chunk size, chunk offsets, chunk lengths, per-chunk digests, a version, and a validity interval. Manifest validation rejects gaps, reordered chunks, inconsistent totals, expired records, and out-of-range chunk sizes.

## Bounded local storage / CDN cache

`ObjectStore` is bounded by maximum bytes, chunks, and manifests. When capacity is reached it evicts least-recently-accessed chunks before accepting new data. TTL expiration removes stale chunks and manifests.

This makes the local object store the native CDN/cache substrate: repeated reads can terminate locally without contacting the origin while preserving content-addressed integrity.

## Native storage protocol

Storage control/data frames use magic `AES1` and protocol version `1`.

Supported messages are:

- `GetManifest` — request a bounded manifest page;
- `ManifestPage` — return a bounded page of ordered chunk descriptors;
- `GetChunk` — request a bounded chunk fragment with the expected digest;
- `Chunk` — return a fragment from an existing object;
- `PutChunk` — stream a replica fragment into the receiving store;
- `Ack` — acknowledge acceptance/rejection of a storage operation;
- `ProviderAnnouncement` — advertise a signed replica provider.

Frames have strict length validation. Manifest pages are bounded to 32 descriptors and transfer fragments to 16 KiB. A receiving assembler accepts out-of-order fragments, tolerates identical retransmission, rejects conflicting overlapping data, and verifies the final digest before committing a chunk.

## Transfer control

Per-peer token buckets independently constrain upload and download traffic. The default runtime policy is:

- 2 MiB/s upload per peer;
- 10 MiB/s download per peer;
- 256 KiB burst allowance.

The storage runtime applies these limits before emitting or accepting chunk payloads, so storage traffic cannot silently bypass the node's configured transfer budget.

## Provider advertisements

A provider advertisement identifies:

- the content object;
- provider `NodeId`;
- monotonically increasing provider version;
- total capacity and cached bytes;
- retrieval cost;
- reliability estimate;
- observation timestamp and expiry;
- Ed25519 public key and signature.

The runtime verifies that the signing public key deterministically derives the advertised `NodeId` and verifies the signature before accepting the provider into the directory. Provider records converge by version and expire automatically.

Provider selection considers free capacity, reliability, and advertised retrieval cost, while remaining bounded by the requested replica count.

## Phase 6 discovery integration

Each local provider advertisement is also published as a Phase 6 `ResourceRecord` using the storage resource kind `8`. Storage announcements are propagated to known peers with a bounded fanout.

Provider expiry and runtime maintenance therefore feed back into the normal network formation lifecycle rather than maintaining a parallel discovery system.

## Phase 7 naming integration

The node runtime exposes a content-resolution bridge through `resolve_content_name`: a Phase 7 `ResourceId` can be interpreted as the corresponding native `ContentId` without introducing a separate application-layer storage naming system.

## Replication and repair

Replication targets are configurable; the default runtime policy requests three replicas including the local copy.

On each maintenance cycle the storage runtime:

1. examines locally hosted content;
2. counts currently valid remote providers;
3. identifies eligible known peers when the replica target is not satisfied;
4. creates bounded replication jobs;
5. streams manifest pages followed by chunk fragments;
6. rate-limits generated frames through the transfer governor;
7. resets completed jobs after a retry interval so an unavailable provider can be replaced;
8. removes expired provider state and stale transfer assemblies.

A `RepairPlan` exposes the current replica count, selected provider nodes, and whether additional replicas are required. Provider expiry therefore causes normal maintenance to schedule replacement replication.

## Failure and recovery behavior

A replica is not considered valid merely because a peer once advertised it. Provider advertisements have bounded lifetimes, and stale records are discarded. When the advertised provider set falls below the configured target, the runtime can schedule another peer for replication.

A receiving node only commits a replicated chunk after the complete fragment stream passes digest verification. A manifest is only retained after its structure is validated. This prevents corrupted transfer data from becoming an authoritative local object.

## Security boundary

Phase 8 includes content/chunk integrity verification, signed provider advertisements, provider `NodeId`/key binding in the runtime acceptance path, bounded allocations, bounded message sizes, and transfer rate limiting.

The current implementation is not a claim of production cryptographic certification. In particular, the object store is an in-memory store rather than a crash-safe disk backend; there is no Byzantine replica consensus, encrypted-at-rest key management, or distributed quorum transaction protocol. Those belong to production hardening rather than being hidden behind the Phase 8 API.

## Implementation status

Phase 8 runtime functionality is implemented across:

- `core/src/storage/content.rs` — native content identifiers;
- `core/src/storage/object_store.rs` — bounded content-addressed object store, manifests, LRU eviction and TTL GC;
- `core/src/storage/distributed.rs` — provider directory and repair planning foundation;
- `core/src/storage/transfer.rs` — transfer budgets and resumable fragment assembly;
- `core/src/storage/protocol.rs` — native storage wire codec;
- `core/src/runtime/storage.rs` — live storage serving, receiving, replication scheduling, and repair;
- `core/src/runtime/node/state.rs` — storage state attached to every node runtime;
- `core/src/runtime/node/event_loop_fixed.rs` — storage discovery, provider gossip, transfers, and maintenance integration;
- `core/src/runtime/node/runtime.rs` — public storage APIs.

The remaining work for production certification is intentionally separated from this functional phase: durable storage backend, operational observability, larger-scale multi-node stress testing, authenticated native-L3/L4 integration review, and security audit.
