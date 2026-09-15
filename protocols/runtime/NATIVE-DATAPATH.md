# AetherNet Native Datapath

## Purpose

The native datapath is the execution path for Aether traffic after an authenticated peer session has been established. It does not require IP routing, TCP sockets, UDP semantics, DNS resolution, or Internet gateways.

## Receive path

```text
Authenticated L2 frame
        |
        v
LinkSession decrypt + replay protection
        |
        v
Native Aether L3 packet
        |
        +--> destination is local
        |       |
        |       v
        |   L4 flow demultiplex
        |       |
        |       v
        |   application delivery
        |
        +--> destination is remote
                |
                v
          L3 route lookup
                |
                v
        decrement hop limit
                |
                v
       next-hop peer selection
                |
                v
      LinkSession encryption
                |
                v
             transmit
```

Every hop decrypts and authenticates the frame at its local peer session, processes the native L3 header, and re-encrypts the unchanged end-to-end payload for the selected next hop. The L3 hop limit is decremented exactly once per forwarding hop.

## L4 multiplexing

L4 flow identifiers are carried inside native L3 packets. A runtime node maintains a bounded map of flow identifiers to `AetherL4Session` state. Reliable L4 acknowledgements are emitted as native L4 segments and sent back through the same L3 routing/encrypted-link path.

## Direct routes

A direct peer is represented by an L3 route whose destination and next-hop address are the same native Aether address. This is valid because the route identifies the immediate forwarding peer rather than an IP gateway.

## Integration primitive

`core/src/runtime/native_stack.rs` provides `NativeAetherStack` for:

- authenticated peer-session attachment;
- native L3 route installation;
- native L3 transmission;
- authenticated ingress decryption;
- L3 forwarding and re-encryption;
- bounded L4 flow creation and demultiplexing;
- reliable ACK return through native routing.

The stack is transport-independent at L3/L4. UDP remains only one possible carrier for a platform adapter; it is not part of the Aether packet semantics.

## Conformance coverage

The implementation contains end-to-end tests covering:

1. encrypted Aether L3 traversal across two forwarding hops;
2. local L3 delivery without falsely requiring an L4 payload;
3. reliable L4 flow delivery through native L3;
4. acknowledgement emission through the reverse native route;
5. direct-route operation.

## Security boundary

The datapath assumes the `LinkSession` was created by the authenticated peer/session establishment machinery. It does not treat an arbitrary node identifier as proof of an authenticated session. Route advertisements and provider advertisements still require their respective distributed-control-plane authentication and identity-binding integration.
