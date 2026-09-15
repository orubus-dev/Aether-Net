# AetherNet Native Naming & Resource Resolution Protocol

Phase 7 defines the native naming plane used to bind human-readable Aether names to content/resource identifiers and to resolve those bindings without depending on Internet DNS.

## Identity and canonical names

Names are canonicalized to lowercase ASCII labels, with a maximum total length of 253 bytes and a maximum label length of 63 bytes. The canonical name is hashed with the `aether:name:v1:` domain to produce a 256-bit `NameHash`.

A `NameRecord` binds `NameHash` to a 256-bit `ResourceId`, an Ed25519 owner public key, a monotonic version, an issue/expiry interval, and an Ed25519 signature. The receiver verifies the signature before accepting the record.

## Native control messages

The Phase 7 wire protocol uses magic `AEN1` and version 1.

- **NameQuery**: 16-byte request identifier plus 32-byte `NameHash`.
- **NameResponse**: request identifier plus either a bounded negative answer or a canonical name and complete signed `NameRecord`.
- **NameAnnouncement**: canonical name plus complete signed `NameRecord` for gossip/publication.
- **DelegationAnnouncement**: authenticated signed delegation for a name namespace.

All decoding is length-bounded. Responses and announcements verify that the canonical name hashes to the embedded record's `NameHash`.

## Delegation

A `NameDelegation` binds a name hash to a parent name hash, an owner `NodeId`, a version, and expiry. `SignedNameDelegation` adds the Ed25519 public key and signature and is self-certifying: the device ID and node ID must be derivable from the advertised public key.

Delegations are monotonic per name and bounded by the resolution engine's configured delegation capacity.

## Resolution

The local resolution engine checks its authoritative records and bounded cache first. A runtime query obtains a request identifier with an expiry and can be fulfilled by a peer response. A response must match the pending request and requested name hash before it is installed.

A missing local record is a normal negative resolution result rather than an instruction to fall back to Internet DNS. Internet interoperation belongs to the later gateway phase.

## Runtime API

The node runtime exposes native operations for publishing a signed name, resolving a local name, creating a bounded remote query, and consuming a response. These operations use the Phase 7 resolver attached to `NodeState`.

## Security and resource rules

Implementations must:

1. canonicalize and hash names deterministically;
2. verify `NameRecord` signatures before installation;
3. enforce monotonic record versions;
4. reject expired records and delegations;
5. enforce bounded record, cache, delegation, and pending-query state;
6. bind responses to their request identifier and requested name hash;
7. reject name/hash mismatches;
8. avoid treating naming data as authorization for the resource itself;
9. leave provider/content integrity to the consuming storage/service protocol.

## Phase 7 completion boundary

Phase 7 is functionally complete when names can be canonically hashed, signed records can be published and resolved, authenticated delegations can be represented and validated, native query/response/announcement messages can be encoded and decoded with bounds, and the node runtime exposes the resolver without requiring an Internet DNS dependency.

Long-duration propagation tests, adversarial fuzzing, clock-skew tests, namespace policy, and independent security review remain part of production certification.