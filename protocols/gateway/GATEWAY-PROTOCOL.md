# Aether Gateway Protocol — Phase 12

Internet interoperability is an optional boundary, not part of the Aether-native L3/L4 contract.

A gateway advertises the external protocol it can translate, direction, opaque endpoint hint, price and expiry. A bounded directory selects the lowest-cost currently valid gateway for a requested protocol and direction.

Gateway descriptors do not themselves grant permission to bridge arbitrary traffic. Authorization, abuse controls, accounting, protocol translation, egress filtering and privacy policy must be enforced by the gateway runtime. The core Aether network remains functional without gateways.

Current implementation: bounded gateway registry, TTL expiry, deterministic selection, protocol/direction modelling and tests.
