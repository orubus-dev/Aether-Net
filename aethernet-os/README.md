# AetherNet OS

AetherNet OS is the native node operating environment for AetherNet. It exposes the Aether protocol stack directly and treats conventional Internet access as an optional gateway capability.

## Transfer model

Large resources are transferred as bounded Aether data packets. Each node can independently configure an upload bandwidth ceiling, download bandwidth ceiling, maximum packet payload, and maximum concurrent transfers.

The default implementation uses a 2,000,000 bit/s upload ceiling and a 10,000,000 bit/s download ceiling. These are configurable per node; they are scheduler limits, not claims about the physical carrier's actual capacity.

A transfer therefore behaves like:

```text
ContentId
   |
   v
Manifest / chunks
   |
   v
Aether data packets
   |
   +--> upload governor / download governor
   |
   v
Native L3 -> L4 -> physical carrier
```

A 2 Mb/s upload setting and a 10 Mb/s download setting are separate budgets. The sender's upload budget and receiver's download budget both constrain an actual transfer.

## Capability levels

L0 through L5 describe node capability, not different network protocols. Higher levels can expose greater relay, storage, compute and infrastructure capacity.

## Current implementation

`core/src/operative.rs` implements bandwidth limits, packet budgets, transfer identifiers, transfer state and packet scheduling. `core/src/operative_os.rs` implements L0-L5 capability state plus boot, health, degraded and recovery lifecycle primitives.

These are protocol/runtime primitives. Persistent disk management, real hardware drivers, signed OS update bootstrapping and full process/service supervision remain platform-integration work.
