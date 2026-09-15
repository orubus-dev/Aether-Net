# Aether Native Naming & Resource Resolution — Phase 7

Phase 7 defines the native name layer above discovery and below application services.

## Model

```text
Human / application name
        |
        v
Canonical Aether name
        |
        v
SHA-256 name hash (domain-separated)
        |
        v
Signed NameRecord
        |
        v
ResourceId
        |
        +--> storage / hosting / service / other resource
```

Names are canonicalized to lowercase ASCII labels. A trailing dot is accepted and removed. Labels are bounded to 63 bytes and the complete name to 253 bytes. `_` is permitted for service-oriented names.

## Ownership and updates

A `NameRecord` contains a name hash, resource identifier, owner public key, monotonic version, validity interval, and Ed25519 signature. The signature covers the complete record fields except the signature itself.

A node accepts an update only when the signature is valid and its version is not older than the currently stored version. Replacing a record invalidates the cached resolution for that name so an older resource cannot be returned after a successful update.

## Resolution

`NamingEngine` provides bounded authoritative records and a bounded cache. Resolution always re-validates the backing record's version and expiry before trusting a cache entry. Expired records and cache entries are garbage-collected.

`NameResolutionEngine` adds a bounded delegation table so the name namespace can later be distributed across roots, operators, and subdomains without changing the resource-resolution API.

## Distributed operation

Phase 7's local resolver is deliberately independent of the carrier. Phase 6 discovery supplies reachable peers; Phase 4 routing supplies paths; native L3/L4 carry future name query/update messages. This keeps naming independent from the physical and legacy Internet carriers.

The next integration step is a signed native name-query protocol carried over Aether L4, followed by distributed replication, quorum/consistency policy, and bootstrap/root namespace distribution.

## Security properties

- Names are domain-separated from L3 addresses.
- Records are cryptographically authenticated.
- Version rollback is rejected.
- Expiry is enforced during lookup and garbage collection.
- Record/cache capacities are bounded to prevent unbounded memory growth.
- Canonicalization prevents equivalent spellings from creating different hashes.

## Production boundary

Phase 7 is implemented as a protocol/runtime primitive. Global root governance, distributed replication, anti-equivocation proofs, persistent name databases, and end-to-end native query transport remain integration/hardening work for production certification.
